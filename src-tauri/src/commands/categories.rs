use serde::Serialize;
use tauri::State;

use crate::db::Db;

#[derive(Serialize)]
pub struct IngredientCategory {
    pub id: i64,
    pub name: String,
    pub sort_order: i64,
}

#[tauri::command]
pub fn categories_list(db: State<Db>) -> Result<Vec<IngredientCategory>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
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
