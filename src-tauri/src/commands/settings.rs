use std::collections::HashMap;

use rusqlite::{params, Connection};
use tauri::State;

use crate::db::Db;

pub fn get_all(conn: &Connection) -> Result<HashMap<String, String>, String> {
    let mut stmt = conn.prepare("SELECT key, value FROM setting").map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<HashMap<_, _>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

/// Reads one setting, if present.
pub fn get(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    match conn.query_row("SELECT value FROM setting WHERE key = ?1", params![key], |r| r.get(0)) {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

fn validate(key: &str, value: &str) -> Result<(), String> {
    match key {
        // 0 = Sunday .. 6 = Saturday (matches date-fns `weekStartsOn`).
        "week_start" => match value.parse::<u8>() {
            Ok(n) if n <= 6 => Ok(()),
            _ => Err("week_start must be 0 (Sunday) to 6 (Saturday)".into()),
        },
        "default_meat_serves" => match value.parse::<u32>() {
            Ok(n) if n >= 1 => Ok(()),
            _ => Err("Default portions must be a whole number of at least 1".into()),
        },
        "planner_view" => match value {
            "1w" | "2w" | "month" | "scroll" => Ok(()),
            _ => Err("planner_view must be one of: 1w, 2w, month, scroll".into()),
        },
        _ => Err(format!("Unknown setting: {key}")),
    }
}

pub fn set(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    validate(key, value)?;
    conn.execute(
        "INSERT INTO setting (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn settings_get(db: State<Db>) -> Result<HashMap<String, String>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    get_all(&conn)
}

#[tauri::command]
pub fn settings_set(db: State<Db>, key: String, value: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    set(&conn, &key, &value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_conn;

    #[test]
    fn seeded_defaults() {
        let conn = test_conn();
        let all = get_all(&conn).unwrap();
        assert_eq!(all["default_meat_serves"], "3");
        assert_eq!(all["week_start"], "1");
    }

    #[test]
    fn set_updates_and_validates() {
        let conn = test_conn();
        set(&conn, "week_start", "0").unwrap();
        assert_eq!(get(&conn, "week_start").unwrap().as_deref(), Some("0"));
        assert!(set(&conn, "week_start", "7").is_err());
        assert!(set(&conn, "default_meat_serves", "0").is_err());
        assert!(set(&conn, "default_meat_serves", "abc").is_err());
        assert!(set(&conn, "nonsense", "1").is_err());
        set(&conn, "planner_view", "month").unwrap();
        assert_eq!(get(&conn, "planner_view").unwrap().as_deref(), Some("month"));
        assert!(set(&conn, "planner_view", "3w").is_err());
        assert_eq!(get(&conn, "nonsense").unwrap(), None);
    }
}
