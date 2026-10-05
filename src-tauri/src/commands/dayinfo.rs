use chrono::NaiveDate;
use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::State;

use super::undo::{tracked, UndoStack};
use crate::db::Db;

/// Things about a day itself (not a dinner): a note and/or a Leftovers marker.
#[derive(Serialize, Debug, PartialEq, Clone)]
pub struct DayInfo {
    pub date: String,
    pub note: Option<String>,
    pub leftovers: bool,
}

fn parse_date(s: &str) -> Result<String, String> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map(|d| d.format("%Y-%m-%d").to_string())
        .map_err(|_| format!("Invalid date: {s}"))
}

fn from_row(r: &rusqlite::Row) -> rusqlite::Result<DayInfo> {
    Ok(DayInfo { date: r.get(0)?, note: r.get(1)?, leftovers: r.get::<_, i64>(2)? != 0 })
}

/// Days dated `start..=end` that have a note or the Leftovers marker.
pub fn list(conn: &Connection, start: &str, end: &str) -> Result<Vec<DayInfo>, String> {
    let (start, end) = (parse_date(start)?, parse_date(end)?);
    let mut stmt = conn
        .prepare("SELECT date, note, leftovers FROM day_info WHERE date BETWEEN ?1 AND ?2 ORDER BY date")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![start, end], from_row)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

fn get(conn: &Connection, date: &str) -> Result<Option<DayInfo>, String> {
    match conn.query_row("SELECT date, note, leftovers FROM day_info WHERE date = ?1", params![date], from_row) {
        Ok(d) => Ok(Some(d)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Partial update: `None` leaves a field alone; a blank note clears it. The row disappears
/// once it has neither a note nor the marker. Returns the day's info afterwards (if any).
pub fn set(
    conn: &Connection,
    date: &str,
    note: Option<&str>,
    leftovers: Option<bool>,
) -> Result<Option<DayInfo>, String> {
    let date = parse_date(date)?;
    let current = get(conn, &date)?;
    let new_note = match note {
        Some(n) => Some(n.trim().to_string()).filter(|n| !n.is_empty()),
        None => current.as_ref().and_then(|c| c.note.clone()),
    };
    let new_leftovers = leftovers.unwrap_or_else(|| current.as_ref().is_some_and(|c| c.leftovers));

    if new_note.is_none() && !new_leftovers {
        conn.execute("DELETE FROM day_info WHERE date = ?1", params![date]).map_err(|e| e.to_string())?;
        return Ok(None);
    }
    conn.execute(
        "INSERT INTO day_info (date, note, leftovers) VALUES (?1, ?2, ?3)
         ON CONFLICT(date) DO UPDATE SET note = excluded.note, leftovers = excluded.leftovers",
        params![date, new_note, new_leftovers],
    )
    .map_err(|e| e.to_string())?;
    get(conn, &date)
}

/// Moves the Leftovers marker from one day to another (a note stays with its day).
pub fn move_leftovers(conn: &Connection, from: &str, to: &str) -> Result<(), String> {
    let (from, to) = (parse_date(from)?, parse_date(to)?);
    if !get(conn, &from)?.is_some_and(|d| d.leftovers) {
        return Err("There are no leftovers on that day".into());
    }
    if from == to {
        return Ok(());
    }
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    set(&tx, &from, None, Some(false))?;
    set(&tx, &to, None, Some(true))?;
    tx.commit().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn day_info_list(db: State<Db>, start: String, end: String) -> Result<Vec<DayInfo>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    list(&conn, &start, &end)
}

#[tauri::command]
pub fn day_info_set(
    db: State<Db>,
    undo: State<UndoStack>,
    date: String,
    note: Option<String>,
    leftovers: Option<bool>,
) -> Result<Option<DayInfo>, String> {
    let label = match (&note, leftovers) {
        (_, Some(true)) => format!("Added leftovers on {date}"),
        (_, Some(false)) => format!("Removed leftovers on {date}"),
        _ => format!("Changed the note for {date}"),
    };
    tracked(&db, &undo, |_| label, |conn| set(conn, &date, note.as_deref(), leftovers))
}

#[tauri::command]
pub fn day_leftovers_move(
    db: State<Db>,
    undo: State<UndoStack>,
    from: String,
    to: String,
) -> Result<(), String> {
    tracked(&db, &undo, |_| "Moved leftovers".to_string(), |conn| move_leftovers(conn, &from, &to))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_conn;

    #[test]
    fn note_set_update_and_clear() {
        let conn = test_conn();
        let d = set(&conn, "2026-10-06", Some("  Out for Sam's birthday "), None).unwrap().unwrap();
        assert_eq!((d.note.as_deref(), d.leftovers), (Some("Out for Sam's birthday"), false));
        let d = set(&conn, "2026-10-06", Some("Out for dinner"), None).unwrap().unwrap();
        assert_eq!(d.note.as_deref(), Some("Out for dinner"));
        assert_eq!(set(&conn, "2026-10-06", Some("   "), None).unwrap(), None, "blank note clears it");
        assert!(list(&conn, "2026-10-01", "2026-10-31").unwrap().is_empty(), "row is gone");
    }

    #[test]
    fn leftovers_and_note_are_independent() {
        let conn = test_conn();
        set(&conn, "2026-10-07", None, Some(true)).unwrap();
        let d = set(&conn, "2026-10-07", Some("Alex working late"), None).unwrap().unwrap();
        assert_eq!((d.note.as_deref(), d.leftovers), (Some("Alex working late"), true));
        // Clearing the note keeps the marker, and vice versa.
        let d = set(&conn, "2026-10-07", Some(""), None).unwrap().unwrap();
        assert_eq!((d.note, d.leftovers), (None, true));
        set(&conn, "2026-10-07", Some("again"), None).unwrap();
        let d = set(&conn, "2026-10-07", None, Some(false)).unwrap().unwrap();
        assert_eq!((d.note.as_deref(), d.leftovers), (Some("again"), false));
        assert_eq!(set(&conn, "2026-10-07", Some(""), None).unwrap(), None);
    }

    #[test]
    fn list_is_inclusive_ordered_and_validates() {
        let conn = test_conn();
        for d in ["2026-10-12", "2026-10-05", "2026-10-04", "2026-10-13"] {
            set(&conn, d, Some("x"), None).unwrap();
        }
        let dates: Vec<String> = list(&conn, "2026-10-05", "2026-10-12").unwrap().into_iter().map(|d| d.date).collect();
        assert_eq!(dates, ["2026-10-05", "2026-10-12"]);
        assert!(list(&conn, "nope", "2026-10-12").is_err());
        assert!(set(&conn, "06/10/2026", Some("x"), None).is_err());
    }

    #[test]
    fn moving_leftovers_leaves_notes_behind() {
        let conn = test_conn();
        set(&conn, "2026-10-07", Some("had a big lunch"), Some(true)).unwrap();
        move_leftovers(&conn, "2026-10-07", "2026-10-08").unwrap();
        let days = list(&conn, "2026-10-07", "2026-10-08").unwrap();
        assert_eq!(days.len(), 2);
        assert_eq!((days[0].note.as_deref(), days[0].leftovers), (Some("had a big lunch"), false));
        assert_eq!((days[1].note.as_deref(), days[1].leftovers), (None, true));
        assert!(move_leftovers(&conn, "2026-10-07", "2026-10-09").is_err(), "nothing to move");
    }
}
