use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tauri::State;

use super::ingredients::friendly;
use crate::db::Db;

const TYPES: [&str; 3] = ["meal", "meat", "side"];

#[derive(Serialize, Debug)]
pub struct ItemSummary {
    pub id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub item_type: String,
    pub colour: Option<String>,
    pub archived: bool,
    pub ingredient_count: i64,
}

#[derive(Serialize, Debug)]
pub struct ItemLine {
    pub ingredient_id: i64,
    pub ingredient_name: String,
    pub unit: String,
    /// Per serve for meat items; the fixed amount otherwise.
    pub quantity: f64,
}

#[derive(Serialize, Debug)]
pub struct Item {
    pub id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub item_type: String,
    pub colour: Option<String>,
    pub notes: Option<String>,
    pub template_id: Option<i64>,
    pub archived: bool,
    pub lines: Vec<ItemLine>,
}

#[derive(Deserialize)]
pub struct LineInput {
    pub ingredient_id: i64,
    pub quantity: f64,
}

#[derive(Deserialize)]
pub struct ItemInput {
    pub name: String,
    #[serde(rename = "type")]
    pub item_type: String,
    pub colour: Option<String>,
    pub notes: Option<String>,
    pub template_id: Option<i64>,
    pub lines: Vec<LineInput>,
}

fn validate(input: &ItemInput) -> Result<String, String> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err("Name is required".into());
    }
    if !TYPES.contains(&input.item_type.as_str()) {
        return Err(format!("Type must be one of: {}", TYPES.join(", ")));
    }
    if input.lines.iter().any(|l| !(l.quantity.is_finite() && l.quantity > 0.0)) {
        return Err("Every ingredient quantity must be greater than zero".into());
    }
    Ok(name)
}

/// Empty / whitespace-only optional text is stored as NULL.
fn blank_to_none(s: &Option<String>) -> Option<String> {
    s.as_ref().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

pub fn list(
    conn: &Connection,
    item_type: Option<&str>,
    search: Option<&str>,
    include_archived: bool,
) -> Result<Vec<ItemSummary>, String> {
    let search = search.map(str::trim).filter(|s| !s.is_empty());
    let mut stmt = conn
        .prepare(
            "SELECT i.id, i.name, i.type, i.colour, i.archived,
                    (SELECT COUNT(*) FROM item_ingredient ii WHERE ii.item_id = i.id)
             FROM item i
             WHERE (?1 IS NULL OR i.type = ?1)
               AND (?2 IS NULL OR i.name LIKE '%' || ?2 || '%')
               AND (?3 OR i.archived = 0)
             ORDER BY i.name COLLATE NOCASE",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![item_type, search, include_archived], |r| {
            Ok(ItemSummary {
                id: r.get(0)?,
                name: r.get(1)?,
                item_type: r.get(2)?,
                colour: r.get(3)?,
                archived: r.get::<_, i64>(4)? != 0,
                ingredient_count: r.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

pub fn get(conn: &Connection, id: i64) -> Result<Item, String> {
    let mut item = conn
        .query_row(
            "SELECT id, name, type, colour, notes, template_id, archived FROM item WHERE id = ?1",
            params![id],
            |r| {
                Ok(Item {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    item_type: r.get(2)?,
                    colour: r.get(3)?,
                    notes: r.get(4)?,
                    template_id: r.get(5)?,
                    archived: r.get::<_, i64>(6)? != 0,
                    lines: vec![],
                })
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => "Item not found".to_string(),
            e => e.to_string(),
        })?;

    let mut stmt = conn
        .prepare(
            "SELECT ii.ingredient_id, g.name, g.unit, ii.quantity
             FROM item_ingredient ii
             JOIN ingredient g ON g.id = ii.ingredient_id
             WHERE ii.item_id = ?1
             ORDER BY ii.position, ii.id",
        )
        .map_err(|e| e.to_string())?;
    item.lines = stmt
        .query_map(params![id], |r| {
            Ok(ItemLine {
                ingredient_id: r.get(0)?,
                ingredient_name: r.get(1)?,
                unit: r.get(2)?,
                quantity: r.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(item)
}

fn write_lines(conn: &Connection, item_id: i64, lines: &[LineInput]) -> Result<(), String> {
    for (pos, l) in lines.iter().enumerate() {
        conn.execute(
            "INSERT INTO item_ingredient (item_id, ingredient_id, quantity, position)
             VALUES (?1, ?2, ?3, ?4)",
            params![item_id, l.ingredient_id, l.quantity, pos as i64],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn create(conn: &mut Connection, input: &ItemInput) -> Result<Item, String> {
    let name = validate(input)?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO item (name, type, colour, notes, template_id) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            name,
            input.item_type,
            blank_to_none(&input.colour),
            blank_to_none(&input.notes),
            input.template_id
        ],
    )
    .map_err(|e| e.to_string())?;
    let id = tx.last_insert_rowid();
    write_lines(&tx, id, &input.lines)?;
    tx.commit().map_err(|e| e.to_string())?;
    get(conn, id)
}

/// Replaces the item's fields and its whole ingredient list.
pub fn update(conn: &mut Connection, id: i64, input: &ItemInput) -> Result<Item, String> {
    let name = validate(input)?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let changed = tx
        .execute(
            "UPDATE item SET name = ?1, type = ?2, colour = ?3, notes = ?4, template_id = ?5
             WHERE id = ?6",
            params![
                name,
                input.item_type,
                blank_to_none(&input.colour),
                blank_to_none(&input.notes),
                input.template_id,
                id
            ],
        )
        .map_err(|e| e.to_string())?;
    if changed == 0 {
        return Err("Item not found".into());
    }
    tx.execute("DELETE FROM item_ingredient WHERE item_id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    write_lines(&tx, id, &input.lines)?;
    tx.commit().map_err(|e| e.to_string())?;
    get(conn, id)
}

/// Copies an item (and its lines) as "<name> (copy)" for quick variants.
pub fn duplicate(conn: &mut Connection, id: i64) -> Result<Item, String> {
    let src = get(conn, id)?;
    let input = ItemInput {
        name: format!("{} (copy)", src.name),
        item_type: src.item_type,
        colour: src.colour,
        notes: src.notes,
        template_id: src.template_id,
        lines: src
            .lines
            .iter()
            .map(|l| LineInput { ingredient_id: l.ingredient_id, quantity: l.quantity })
            .collect(),
    };
    create(conn, &input)
}

/// Archives (or restores) rather than deleting, so past plans still render.
pub fn archive(conn: &Connection, id: i64, archived: bool) -> Result<Item, String> {
    let changed = conn
        .execute("UPDATE item SET archived = ?1 WHERE id = ?2", params![archived, id])
        .map_err(|e| friendly(e, "Item"))?;
    if changed == 0 {
        return Err("Item not found".into());
    }
    get(conn, id)
}

#[tauri::command]
pub fn items_list(
    db: State<Db>,
    item_type: Option<String>,
    search: Option<String>,
    include_archived: Option<bool>,
) -> Result<Vec<ItemSummary>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    list(&conn, item_type.as_deref(), search.as_deref(), include_archived.unwrap_or(false))
}

#[tauri::command]
pub fn items_get(db: State<Db>, id: i64) -> Result<Item, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    get(&conn, id)
}

#[tauri::command]
pub fn items_create(db: State<Db>, input: ItemInput) -> Result<Item, String> {
    let mut conn = db.0.lock().map_err(|e| e.to_string())?;
    create(&mut conn, &input)
}

#[tauri::command]
pub fn items_update(db: State<Db>, id: i64, input: ItemInput) -> Result<Item, String> {
    let mut conn = db.0.lock().map_err(|e| e.to_string())?;
    update(&mut conn, id, &input)
}

#[tauri::command]
pub fn items_duplicate(db: State<Db>, id: i64) -> Result<Item, String> {
    let mut conn = db.0.lock().map_err(|e| e.to_string())?;
    duplicate(&mut conn, id)
}

#[tauri::command]
pub fn items_archive(db: State<Db>, id: i64, archived: bool) -> Result<Item, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    archive(&conn, id, archived)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_conn;

    fn ingredient_id(conn: &Connection, name: &str) -> i64 {
        conn.query_row("SELECT id FROM ingredient WHERE name = ?1", params![name], |r| r.get(0))
            .unwrap()
    }

    fn input(conn: &Connection, name: &str, t: &str) -> ItemInput {
        ItemInput {
            name: name.into(),
            item_type: t.into(),
            colour: Some("  ".into()),
            notes: None,
            template_id: None,
            lines: vec![
                LineInput { ingredient_id: ingredient_id(conn, "Pork mince"), quantity: 500.0 },
                LineInput { ingredient_id: ingredient_id(conn, "Onion"), quantity: 1.0 },
            ],
        }
    }

    #[test]
    fn list_filters_by_type_and_search() {
        let conn = test_conn();
        assert_eq!(list(&conn, None, None, false).unwrap().len(), 8);
        let meats = list(&conn, Some("meat"), None, false).unwrap();
        assert_eq!(meats.len(), 3);
        let found = list(&conn, None, Some(" spag "), false).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "Spag bol");
        assert_eq!(found[0].ingredient_count, 5);
    }

    #[test]
    fn create_get_update_roundtrip() {
        let mut conn = test_conn();
        let i = input(&conn, "Pork stir fry", "meal");
        let created = create(&mut conn, &i).unwrap();
        assert_eq!(created.lines.len(), 2);
        assert_eq!(created.lines[0].ingredient_name, "Pork mince");
        assert_eq!(created.colour, None, "blank colour is stored as NULL");

        let mut edit = input(&conn, "Pork stir fry v2", "meal");
        edit.lines.reverse();
        edit.lines.pop();
        let updated = update(&mut conn, created.id, &edit).unwrap();
        assert_eq!(updated.name, "Pork stir fry v2");
        assert_eq!(updated.lines.len(), 1);
        assert_eq!(updated.lines[0].ingredient_name, "Onion");
    }

    #[test]
    fn meat_quantities_are_stored_per_serve() {
        let conn = test_conn();
        let steak = list(&conn, Some("meat"), Some("Steak"), false).unwrap().remove(0);
        let steak = get(&conn, steak.id).unwrap();
        assert_eq!(steak.lines[0].quantity, 250.0);
    }

    #[test]
    fn duplicate_copies_lines() {
        let mut conn = test_conn();
        let bol = list(&conn, None, Some("Spag bol"), false).unwrap().remove(0);
        let copy = duplicate(&mut conn, bol.id).unwrap();
        assert_eq!(copy.name, "Spag bol (copy)");
        assert_eq!(copy.lines.len(), 5);
        assert_ne!(copy.id, bol.id);
    }

    #[test]
    fn archive_hides_from_default_list() {
        let mut conn = test_conn();
        let temp = input(&conn, "Temp", "side");
        let created = create(&mut conn, &temp).unwrap();
        archive(&conn, created.id, true).unwrap();
        assert!(!list(&conn, None, Some("Temp"), false).unwrap().iter().any(|i| i.id == created.id));
        assert!(list(&conn, None, Some("Temp"), true).unwrap().iter().any(|i| i.id == created.id));
        // Still readable by id so past plans can render it.
        assert!(get(&conn, created.id).unwrap().archived);
    }

    #[test]
    fn rejects_bad_input() {
        let mut conn = test_conn();
        let blank = input(&conn, " ", "meal");
        let bad_type = input(&conn, "X", "dessert");
        assert!(create(&mut conn, &blank).is_err());
        assert!(create(&mut conn, &bad_type).is_err());
        let mut zero = input(&conn, "X", "meal");
        zero.lines[0].quantity = 0.0;
        assert!(create(&mut conn, &zero).is_err());
        let ok = input(&conn, "X", "meal");
        assert_eq!(update(&mut conn, 9999, &ok).unwrap_err(), "Item not found");
    }
}
