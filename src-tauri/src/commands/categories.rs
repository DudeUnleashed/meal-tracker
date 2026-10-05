use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::State;

use crate::db::Db;

#[derive(Serialize, Debug)]
pub struct IngredientCategory {
    pub id: i64,
    pub name: String,
    pub sort_order: i64,
}

pub fn list(conn: &Connection) -> Result<Vec<IngredientCategory>, String> {
    let mut stmt = conn
        .prepare("SELECT id, name, sort_order FROM ingredient_category ORDER BY sort_order, name")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(IngredientCategory {
                id: r.get(0)?,
                name: r.get(1)?,
                sort_order: r.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

/// Sets the order categories appear in the shopping summary. `ids` must list every
/// category exactly once, in the new order.
pub fn reorder(conn: &Connection, ids: &[i64]) -> Result<Vec<IngredientCategory>, String> {
    let mut existing: Vec<i64> = list(conn)?.into_iter().map(|c| c.id).collect();
    let mut wanted = ids.to_vec();
    existing.sort_unstable();
    wanted.sort_unstable();
    if existing != wanted {
        return Err("The new order must list every category exactly once".into());
    }
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    for (pos, id) in ids.iter().enumerate() {
        tx.execute(
            "UPDATE ingredient_category SET sort_order = ?1 WHERE id = ?2",
            params![pos as i64 + 1, id],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    list(conn)
}

#[tauri::command]
pub fn categories_list(db: State<Db>) -> Result<Vec<IngredientCategory>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    list(&conn)
}

#[tauri::command]
pub fn categories_reorder(db: State<Db>, ids: Vec<i64>) -> Result<Vec<IngredientCategory>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    reorder(&conn, &ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_conn;

    fn names(conn: &Connection) -> Vec<String> {
        list(conn).unwrap().into_iter().map(|c| c.name).collect()
    }

    #[test]
    fn reorder_changes_the_order() {
        let conn = test_conn();
        assert_eq!(names(&conn), ["Meat", "Seafood", "Veg", "Fruit", "Dairy", "Pantry", "Other"]);
        // Put Veg first and Meat last.
        let mut ids: Vec<i64> = list(&conn).unwrap().into_iter().map(|c| c.id).collect();
        let veg = ids.remove(2);
        let meat = ids.remove(0);
        ids.insert(0, veg);
        ids.push(meat);
        let after = reorder(&conn, &ids).unwrap();
        assert_eq!(
            after.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
            ["Veg", "Seafood", "Fruit", "Dairy", "Pantry", "Other", "Meat"]
        );
    }

    #[test]
    fn reorder_requires_every_category_once() {
        let conn = test_conn();
        let ids: Vec<i64> = list(&conn).unwrap().into_iter().map(|c| c.id).collect();
        assert!(reorder(&conn, &ids[1..]).is_err(), "missing one");
        let mut dup = ids.clone();
        dup[0] = dup[1];
        assert!(reorder(&conn, &dup).is_err(), "duplicate");
        let mut unknown = ids.clone();
        unknown[0] = 9999;
        assert!(reorder(&conn, &unknown).is_err(), "unknown id");
        assert_eq!(names(&conn)[0], "Meat", "failed reorders change nothing");
    }
}
