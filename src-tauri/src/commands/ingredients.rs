use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::db::Db;

const UNITS: [&str; 3] = ["g", "ml", "each"];

#[derive(Serialize, Debug)]
pub struct Ingredient {
    pub id: i64,
    pub name: String,
    pub category_id: i64,
    pub category_name: String,
    pub unit: String,
    pub archived: bool,
    /// Number of items that use this ingredient.
    pub used_in_count: i64,
}

#[derive(Deserialize)]
pub struct IngredientInput {
    pub name: String,
    pub category_id: i64,
    pub unit: String,
}

/// Turns a UNIQUE-constraint failure into a readable message.
pub(crate) fn friendly(e: rusqlite::Error, what: &str) -> String {
    let msg = e.to_string();
    if msg.contains("UNIQUE") {
        format!("{what} already exists")
    } else {
        msg
    }
}

fn validate(input: &IngredientInput) -> Result<String, String> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err("Name is required".into());
    }
    if !UNITS.contains(&input.unit.as_str()) {
        return Err(format!("Unit must be one of: {}", UNITS.join(", ")));
    }
    Ok(name)
}

/// Names are unique ignoring case: "carrot" and "Carrot" would otherwise split a shopping total.
fn ensure_name_free(conn: &Connection, name: &str, except_id: i64) -> Result<(), String> {
    let taken: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM ingredient WHERE name = ?1 COLLATE NOCASE AND id != ?2)",
            params![name, except_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if taken {
        return Err("An ingredient with that name already exists".into());
    }
    Ok(())
}

const SELECT: &str = "
    SELECT i.id, i.name, i.category_id, c.name, i.unit, i.archived,
           (SELECT COUNT(DISTINCT ii.item_id) FROM item_ingredient ii WHERE ii.ingredient_id = i.id)
    FROM ingredient i
    JOIN ingredient_category c ON c.id = i.category_id";

fn from_row(r: &rusqlite::Row) -> rusqlite::Result<Ingredient> {
    Ok(Ingredient {
        id: r.get(0)?,
        name: r.get(1)?,
        category_id: r.get(2)?,
        category_name: r.get(3)?,
        unit: r.get(4)?,
        archived: r.get::<_, i64>(5)? != 0,
        used_in_count: r.get(6)?,
    })
}

pub fn list(conn: &Connection, include_archived: bool) -> Result<Vec<Ingredient>, String> {
    let sql = format!(
        "{SELECT} WHERE (?1 OR i.archived = 0) ORDER BY c.sort_order, i.name COLLATE NOCASE"
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![include_archived], from_row)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

fn get(conn: &Connection, id: i64) -> Result<Ingredient, String> {
    conn.query_row(&format!("{SELECT} WHERE i.id = ?1"), params![id], from_row)
        .map_err(|e| e.to_string())
}

pub fn create(conn: &Connection, input: &IngredientInput) -> Result<Ingredient, String> {
    let name = validate(input)?;
    ensure_name_free(conn, &name, -1)?;
    conn.execute(
        "INSERT INTO ingredient (name, category_id, unit) VALUES (?1, ?2, ?3)",
        params![name, input.category_id, input.unit],
    )
    .map_err(|e| friendly(e, "An ingredient with that name"))?;
    get(conn, conn.last_insert_rowid())
}

pub fn update(conn: &Connection, id: i64, input: &IngredientInput) -> Result<Ingredient, String> {
    let name = validate(input)?;
    ensure_name_free(conn, &name, id)?;
    let changed = conn
        .execute(
            "UPDATE ingredient SET name = ?1, category_id = ?2, unit = ?3 WHERE id = ?4",
            params![name, input.category_id, input.unit, id],
        )
        .map_err(|e| friendly(e, "An ingredient with that name"))?;
    if changed == 0 {
        return Err("Ingredient not found".into());
    }
    get(conn, id)
}

/// Archives (or restores) rather than deleting, so past plans still render.
pub fn archive(conn: &Connection, id: i64, archived: bool) -> Result<Ingredient, String> {
    let changed = conn
        .execute(
            "UPDATE ingredient SET archived = ?1 WHERE id = ?2",
            params![archived, id],
        )
        .map_err(|e| e.to_string())?;
    if changed == 0 {
        return Err("Ingredient not found".into());
    }
    get(conn, id)
}

#[tauri::command]
pub fn ingredients_list(db: State<Db>, include_archived: Option<bool>) -> Result<Vec<Ingredient>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    list(&conn, include_archived.unwrap_or(false))
}

#[tauri::command]
pub fn ingredients_create(db: State<Db>, input: IngredientInput) -> Result<Ingredient, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    create(&conn, &input)
}

#[tauri::command]
pub fn ingredients_update(db: State<Db>, id: i64, input: IngredientInput) -> Result<Ingredient, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    update(&conn, id, &input)
}

#[tauri::command]
pub fn ingredients_archive(db: State<Db>, id: i64, archived: bool) -> Result<Ingredient, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    archive(&conn, id, archived)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_conn;

    fn input(name: &str, unit: &str) -> IngredientInput {
        IngredientInput { name: name.into(), category_id: 3, unit: unit.into() }
    }

    #[test]
    fn create_update_archive() {
        let conn = test_conn();
        let created = create(&conn, &input("  Leek ", "each")).unwrap();
        assert_eq!(created.name, "Leek");
        assert_eq!(created.category_name, "Veg");
        assert_eq!(created.used_in_count, 0);

        let updated = update(&conn, created.id, &input("Leeks", "g")).unwrap();
        assert_eq!((updated.name.as_str(), updated.unit.as_str()), ("Leeks", "g"));

        archive(&conn, created.id, true).unwrap();
        assert!(!list(&conn, false).unwrap().iter().any(|i| i.id == created.id));
        assert!(list(&conn, true).unwrap().iter().any(|i| i.id == created.id));
    }

    #[test]
    fn rejects_bad_input() {
        let conn = test_conn();
        assert!(create(&conn, &input("   ", "g")).is_err());
        assert!(create(&conn, &input("Leek", "kg")).is_err());
        assert_eq!(
            create(&conn, &input("Carrot", "each")).unwrap_err(),
            "An ingredient with that name already exists"
        );
    }

    #[test]
    fn names_are_unique_ignoring_case() {
        let conn = test_conn();
        let msg = "An ingredient with that name already exists";
        assert_eq!(create(&conn, &input("carrot", "each")).unwrap_err(), msg);
        assert_eq!(create(&conn, &input("  CARROT ", "each")).unwrap_err(), msg);

        let leek = create(&conn, &input("Leek", "each")).unwrap();
        assert_eq!(update(&conn, leek.id, &input("zucchini", "each")).unwrap_err(), msg);
        // Renaming to a different case of its own name is fine.
        assert_eq!(update(&conn, leek.id, &input("LEEK", "each")).unwrap().name, "LEEK");
    }

    #[test]
    fn used_in_count_counts_items() {
        let conn = test_conn();
        let carrot = list(&conn, false).unwrap().into_iter().find(|i| i.name == "Carrot").unwrap();
        // Seed: Chow mein, Spag bol, Garden salad, Steamed veg
        assert_eq!(carrot.used_in_count, 4);
    }
}
