use chrono::NaiveDate;
use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::State;

use super::settings;
use crate::db::Db;

#[derive(Serialize, Debug)]
pub struct PlanEntry {
    pub id: i64,
    pub date: String,
    pub item_id: i64,
    pub item_name: String,
    pub item_type: String,
    pub item_colour: Option<String>,
    pub item_archived: bool,
    /// Present for meat entries only.
    pub serves: Option<i64>,
    pub note: Option<String>,
    pub position: i64,
}

fn parse_date(s: &str) -> Result<String, String> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map(|d| d.format("%Y-%m-%d").to_string())
        .map_err(|_| format!("Invalid date: {s}"))
}

const SELECT: &str = "
    SELECT pe.id, pe.date, pe.item_id, i.name, i.type, i.colour, i.archived,
           pe.serves, pe.note, pe.position
    FROM plan_entry pe
    JOIN item i ON i.id = pe.item_id";

fn from_row(r: &rusqlite::Row) -> rusqlite::Result<PlanEntry> {
    Ok(PlanEntry {
        id: r.get(0)?,
        date: r.get(1)?,
        item_id: r.get(2)?,
        item_name: r.get(3)?,
        item_type: r.get(4)?,
        item_colour: r.get(5)?,
        item_archived: r.get::<_, i64>(6)? != 0,
        serves: r.get(7)?,
        note: r.get(8)?,
        position: r.get(9)?,
    })
}

/// Entries dated `start..=end`, ordered by day then position within the day.
pub fn list(conn: &Connection, start: &str, end: &str) -> Result<Vec<PlanEntry>, String> {
    let (start, end) = (parse_date(start)?, parse_date(end)?);
    let mut stmt = conn
        .prepare(&format!(
            "{SELECT} WHERE pe.date BETWEEN ?1 AND ?2 ORDER BY pe.date, pe.position, pe.id"
        ))
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![start, end], from_row)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

fn get(conn: &Connection, id: i64) -> Result<PlanEntry, String> {
    conn.query_row(&format!("{SELECT} WHERE pe.id = ?1"), params![id], from_row)
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => "Plan entry not found".to_string(),
            e => e.to_string(),
        })
}

/// Puts an item on a day, at the end of that day's entries.
///
/// Meat entries take `serves`, falling back to the `default_meat_serves` setting.
/// Meals and sides never carry serves.
pub fn add(
    conn: &Connection,
    item_id: i64,
    date: &str,
    serves: Option<i64>,
) -> Result<PlanEntry, String> {
    let date = parse_date(date)?;
    let (item_type, archived): (String, bool) = conn
        .query_row("SELECT type, archived FROM item WHERE id = ?1", params![item_id], |r| {
            Ok((r.get(0)?, r.get::<_, i64>(1)? != 0))
        })
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => "Item not found".to_string(),
            e => e.to_string(),
        })?;
    if archived {
        return Err("Archived items can't be added to the plan".into());
    }

    let serves = if item_type == "meat" {
        let n = match serves {
            Some(n) => n,
            None => settings::get(conn, "default_meat_serves")?
                .and_then(|v| v.parse().ok())
                .unwrap_or(3),
        };
        if n < 1 {
            return Err("Serves must be at least 1".into());
        }
        Some(n)
    } else {
        None
    };

    let position: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(position) + 1, 0) FROM plan_entry WHERE date = ?1",
            params![date],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO plan_entry (date, item_id, serves, position) VALUES (?1, ?2, ?3, ?4)",
        params![date, item_id, serves, position],
    )
    .map_err(|e| e.to_string())?;
    get(conn, conn.last_insert_rowid())
}

pub fn remove(conn: &Connection, id: i64) -> Result<(), String> {
    let changed = conn
        .execute("DELETE FROM plan_entry WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    if changed == 0 {
        return Err("Plan entry not found".into());
    }
    Ok(())
}

#[tauri::command]
pub fn plan_list(db: State<Db>, start: String, end: String) -> Result<Vec<PlanEntry>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    list(&conn, &start, &end)
}

#[tauri::command]
pub fn plan_add(
    db: State<Db>,
    item_id: i64,
    date: String,
    serves: Option<i64>,
) -> Result<PlanEntry, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    add(&conn, item_id, &date, serves)
}

#[tauri::command]
pub fn plan_remove(db: State<Db>, id: i64) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    remove(&conn, id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_conn;

    fn item_id(conn: &Connection, name: &str) -> i64 {
        conn.query_row("SELECT id FROM item WHERE name = ?1", params![name], |r| r.get(0)).unwrap()
    }

    #[test]
    fn meat_defaults_to_setting_and_others_have_no_serves() {
        let conn = test_conn();
        let steak = add(&conn, item_id(&conn, "Steak"), "2026-10-06", None).unwrap();
        assert_eq!(steak.serves, Some(3));
        assert_eq!(steak.item_type, "meat");

        settings::set(&conn, "default_meat_serves", "5").unwrap();
        let saus = add(&conn, item_id(&conn, "Sausages"), "2026-10-06", None).unwrap();
        assert_eq!(saus.serves, Some(5));

        let kiev = add(&conn, item_id(&conn, "Chicken Kiev"), "2026-10-06", Some(2)).unwrap();
        assert_eq!(kiev.serves, Some(2));

        let chips = add(&conn, item_id(&conn, "Chips"), "2026-10-06", Some(4)).unwrap();
        assert_eq!(chips.serves, None);
        let bol = add(&conn, item_id(&conn, "Spag bol"), "2026-10-06", None).unwrap();
        assert_eq!(bol.serves, None);
    }

    #[test]
    fn entries_append_in_order_within_a_day() {
        let conn = test_conn();
        let a = add(&conn, item_id(&conn, "Steak"), "2026-10-06", None).unwrap();
        let b = add(&conn, item_id(&conn, "Chips"), "2026-10-06", None).unwrap();
        let other = add(&conn, item_id(&conn, "Chips"), "2026-10-07", None).unwrap();
        assert_eq!((a.position, b.position, other.position), (0, 1, 0));

        let day = list(&conn, "2026-10-06", "2026-10-06").unwrap();
        let names: Vec<&str> = day.iter().map(|e| e.item_name.as_str()).collect();
        assert_eq!(names, ["Steak", "Chips"]);
    }

    #[test]
    fn list_is_inclusive_and_ordered_by_date() {
        let conn = test_conn();
        add(&conn, item_id(&conn, "Chips"), "2026-10-12", None).unwrap();
        add(&conn, item_id(&conn, "Chips"), "2026-10-05", None).unwrap();
        add(&conn, item_id(&conn, "Chips"), "2026-10-11", None).unwrap();
        add(&conn, item_id(&conn, "Chips"), "2026-10-04", None).unwrap();
        let dates: Vec<String> =
            list(&conn, "2026-10-05", "2026-10-12").unwrap().into_iter().map(|e| e.date).collect();
        assert_eq!(dates, ["2026-10-05", "2026-10-11", "2026-10-12"]);
    }

    #[test]
    fn remove_deletes_the_entry() {
        let conn = test_conn();
        let e = add(&conn, item_id(&conn, "Chips"), "2026-10-06", None).unwrap();
        remove(&conn, e.id).unwrap();
        assert!(list(&conn, "2026-10-06", "2026-10-06").unwrap().is_empty());
        assert_eq!(remove(&conn, e.id).unwrap_err(), "Plan entry not found");
    }

    #[test]
    fn rejects_bad_input() {
        let conn = test_conn();
        let chips = item_id(&conn, "Chips");
        assert!(add(&conn, chips, "06/10/2026", None).is_err());
        assert!(add(&conn, 9999, "2026-10-06", None).is_err());
        assert!(add(&conn, item_id(&conn, "Steak"), "2026-10-06", Some(0)).is_err());
        conn.execute("UPDATE item SET archived = 1 WHERE id = ?1", params![chips]).unwrap();
        assert!(add(&conn, chips, "2026-10-06", None).is_err());
        assert!(list(&conn, "nope", "2026-10-06").is_err());
    }

    #[test]
    fn archived_items_still_appear_in_past_plans() {
        let conn = test_conn();
        let e = add(&conn, item_id(&conn, "Chips"), "2026-10-06", None).unwrap();
        conn.execute("UPDATE item SET archived = 1 WHERE name = 'Chips'", []).unwrap();
        let day = list(&conn, "2026-10-06", "2026-10-06").unwrap();
        assert_eq!(day[0].id, e.id);
        assert!(day[0].item_archived);
    }
}
