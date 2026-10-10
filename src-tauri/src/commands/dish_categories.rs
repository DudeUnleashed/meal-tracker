use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::State;

use crate::db::Db;

/// What a fresh library (or a restored backup that predates categories) starts with.
pub const DEFAULTS: [&str; 6] = ["Beef", "Chicken", "Lamb", "Pork", "Seafood", "Vegetarian"];

/// A way to file dishes in the library lists (Beef, Chicken, Soups...). Not to be confused with
/// the ingredient categories that group the shopping list.
#[derive(Serialize, Debug, PartialEq)]
pub struct DishCategory {
    pub id: i64,
    pub name: String,
    /// Dishes (and templates) filed under it.
    pub item_count: i64,
}

fn clean(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Give the category a name".into());
    }
    Ok(name.to_string())
}

fn exists(conn: &Connection, name: &str, except: Option<i64>) -> Result<bool, String> {
    conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM item_category WHERE name = ?1 AND id != ?2)",
        params![name, except.unwrap_or(-1)],
        |r| r.get(0),
    )
    .map_err(|e| e.to_string())
}

pub fn list(conn: &Connection) -> Result<Vec<DishCategory>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT c.id, c.name,
                    (SELECT COUNT(*) FROM item i WHERE i.category_id = c.id AND i.type != 'blank')
             FROM item_category c ORDER BY c.name COLLATE NOCASE",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| Ok(DishCategory { id: r.get(0)?, name: r.get(1)?, item_count: r.get(2)? }))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

pub fn create(conn: &Connection, name: &str) -> Result<DishCategory, String> {
    let name = clean(name)?;
    if exists(conn, &name, None)? {
        return Err(format!("There is already a category called \"{name}\""));
    }
    conn.execute("INSERT INTO item_category (name) VALUES (?1)", params![name]).map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    Ok(DishCategory { id, name, item_count: 0 })
}

pub fn rename(conn: &Connection, id: i64, name: &str) -> Result<(), String> {
    let name = clean(name)?;
    if exists(conn, &name, Some(id))? {
        return Err(format!("There is already a category called \"{name}\""));
    }
    let changed = conn
        .execute("UPDATE item_category SET name = ?1 WHERE id = ?2", params![name, id])
        .map_err(|e| e.to_string())?;
    if changed == 0 {
        return Err("Category not found".into());
    }
    Ok(())
}

/// Deletes a category; dishes filed under it simply become uncategorised.
pub fn delete(conn: &Connection, id: i64) -> Result<(), String> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute("UPDATE item SET category_id = NULL WHERE category_id = ?1", params![id]).map_err(|e| e.to_string())?;
    tx.execute("UPDATE template SET category_id = NULL WHERE category_id = ?1", params![id]).map_err(|e| e.to_string())?;
    let changed = tx.execute("DELETE FROM item_category WHERE id = ?1", params![id]).map_err(|e| e.to_string())?;
    if changed == 0 {
        return Err("Category not found".into());
    }
    tx.commit().map_err(|e| e.to_string())
}

/// Puts the starting categories back when there are none (a restored backup that predates them).
pub fn ensure_defaults(conn: &Connection) -> Result<(), String> {
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM item_category", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    if n == 0 {
        for name in DEFAULTS {
            conn.execute("INSERT INTO item_category (name) VALUES (?1)", params![name]).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn dish_categories_list(db: State<Db>) -> Result<Vec<DishCategory>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    list(&conn)
}

#[tauri::command]
pub fn dish_categories_create(db: State<Db>, name: String) -> Result<DishCategory, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    create(&conn, &name)
}

#[tauri::command]
pub fn dish_categories_rename(db: State<Db>, id: i64, name: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    rename(&conn, id, &name)
}

#[tauri::command]
pub fn dish_categories_delete(db: State<Db>, id: i64) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    delete(&conn, id)
}
