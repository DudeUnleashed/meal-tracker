use std::collections::HashMap;

use chrono::NaiveDate;
use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::State;

use crate::db::Db;

/// When an item was last had / is next planned, plus the most recent note left on it.
/// Items never planned have no row.
#[derive(Serialize, Debug, Default, PartialEq)]
pub struct ItemUsage {
    pub item_id: i64,
    /// Most recent date strictly before today.
    pub last_had: Option<String>,
    /// Earliest date today or later.
    pub next_planned: Option<String>,
    /// Note from the most recently dated entry that has one.
    pub latest_note: Option<String>,
    pub latest_note_date: Option<String>,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct HistoryEntry {
    pub entry_id: i64,
    pub date: String,
    pub serves: Option<i64>,
    pub note: Option<String>,
}

fn parse_date(s: &str) -> Result<String, String> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map(|d| d.format("%Y-%m-%d").to_string())
        .map_err(|_| format!("Invalid date: {s}"))
}

/// Usage for every item that appears in the plan. `today` is the caller's local date,
/// so "had" vs "planned" matches what the user sees.
pub fn usage(conn: &Connection, today: &str) -> Result<Vec<ItemUsage>, String> {
    let today = parse_date(today)?;
    let mut by_item: HashMap<i64, ItemUsage> = HashMap::new();

    let mut stmt = conn
        .prepare(
            "SELECT item_id,
                    MAX(CASE WHEN date <  ?1 THEN date END),
                    MIN(CASE WHEN date >= ?1 THEN date END)
             FROM plan_entry GROUP BY item_id",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![today], |r| {
            Ok(ItemUsage {
                item_id: r.get(0)?,
                last_had: r.get(1)?,
                next_planned: r.get(2)?,
                ..Default::default()
            })
        })
        .map_err(|e| e.to_string())?;
    for row in rows {
        let u = row.map_err(|e| e.to_string())?;
        by_item.insert(u.item_id, u);
    }

    // Oldest first, so the last write per item is the most recent note.
    let mut stmt = conn
        .prepare("SELECT item_id, note, date FROM plan_entry WHERE note IS NOT NULL ORDER BY date, id")
        .map_err(|e| e.to_string())?;
    let notes = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))
        .map_err(|e| e.to_string())?;
    for n in notes {
        let (item_id, note, date) = n.map_err(|e| e.to_string())?;
        if let Some(u) = by_item.get_mut(&item_id) {
            u.latest_note = Some(note);
            u.latest_note_date = Some(date);
        }
    }

    let mut out: Vec<ItemUsage> = by_item.into_values().collect();
    out.sort_by_key(|u| u.item_id);
    Ok(out)
}

/// Every plan entry for an item, newest first (future dates included).
pub fn history(conn: &Connection, item_id: i64) -> Result<Vec<HistoryEntry>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, date, serves, note FROM plan_entry
             WHERE item_id = ?1 ORDER BY date DESC, id DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![item_id], |r| {
            Ok(HistoryEntry { entry_id: r.get(0)?, date: r.get(1)?, serves: r.get(2)?, note: r.get(3)? })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

#[tauri::command]
pub fn items_usage(db: State<Db>, today: String) -> Result<Vec<ItemUsage>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    usage(&conn, &today)
}

#[tauri::command]
pub fn items_history(db: State<Db>, id: i64) -> Result<Vec<HistoryEntry>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    history(&conn, id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::plan;
    use crate::db::test_conn;

    fn item_id(conn: &Connection, name: &str) -> i64 {
        conn.query_row("SELECT id FROM item WHERE name = ?1", params![name], |r| r.get(0)).unwrap()
    }

    fn plan_on(conn: &Connection, item: &str, date: &str) -> i64 {
        plan::add(conn, item_id(conn, item), date, None).unwrap().id
    }

    fn for_item(conn: &Connection, today: &str, item: &str) -> Option<ItemUsage> {
        let id = item_id(conn, item);
        usage(conn, today).unwrap().into_iter().find(|u| u.item_id == id)
    }

    #[test]
    fn never_planned_items_have_no_usage() {
        let conn = test_conn();
        assert!(usage(&conn, "2026-10-05").unwrap().is_empty());
        assert_eq!(for_item(&conn, "2026-10-05", "Chips"), None);
    }

    #[test]
    fn last_had_is_most_recent_past_and_next_planned_is_earliest_upcoming() {
        let conn = test_conn();
        for d in ["2026-09-01", "2026-09-28", "2026-10-05", "2026-10-09", "2026-10-20"] {
            plan_on(&conn, "Spag bol", d);
        }
        let u = for_item(&conn, "2026-10-05", "Spag bol").unwrap();
        assert_eq!(u.last_had.as_deref(), Some("2026-09-28"));
        // Today counts as planned, not yet had.
        assert_eq!(u.next_planned.as_deref(), Some("2026-10-05"));

        let u = for_item(&conn, "2026-10-06", "Spag bol").unwrap();
        assert_eq!(u.last_had.as_deref(), Some("2026-10-05"));
        assert_eq!(u.next_planned.as_deref(), Some("2026-10-09"));
    }

    #[test]
    fn only_future_or_only_past() {
        let conn = test_conn();
        plan_on(&conn, "Chips", "2026-10-20");
        plan_on(&conn, "Steak", "2026-08-01");
        let chips = for_item(&conn, "2026-10-05", "Chips").unwrap();
        assert_eq!((chips.last_had, chips.next_planned.as_deref()), (None, Some("2026-10-20")));
        let steak = for_item(&conn, "2026-10-05", "Steak").unwrap();
        assert_eq!((steak.last_had.as_deref(), steak.next_planned), (Some("2026-08-01"), None));
    }

    #[test]
    fn latest_note_is_from_the_most_recently_dated_entry_with_a_note() {
        let conn = test_conn();
        let a = plan_on(&conn, "Chow mein", "2026-09-01");
        let b = plan_on(&conn, "Chow mein", "2026-09-20");
        let c = plan_on(&conn, "Chow mein", "2026-09-25"); // no note
        plan::update(&conn, a, None, Some("too salty")).unwrap();
        plan::update(&conn, b, None, Some("try satay sauce")).unwrap();
        let _ = c;
        let u = for_item(&conn, "2026-10-05", "Chow mein").unwrap();
        assert_eq!(u.latest_note.as_deref(), Some("try satay sauce"));
        assert_eq!(u.latest_note_date.as_deref(), Some("2026-09-20"));

        // Clearing the newest note falls back to the older one.
        plan::update(&conn, b, None, Some("")).unwrap();
        let u = for_item(&conn, "2026-10-05", "Chow mein").unwrap();
        assert_eq!(u.latest_note.as_deref(), Some("too salty"));
    }

    #[test]
    fn usage_is_per_item() {
        let conn = test_conn();
        let e = plan_on(&conn, "Chips", "2026-09-01");
        plan::update(&conn, e, None, Some("crispier")).unwrap();
        plan_on(&conn, "Garden salad", "2026-09-02");
        let salad = for_item(&conn, "2026-10-05", "Garden salad").unwrap();
        assert_eq!(salad.latest_note, None);
    }

    #[test]
    fn history_lists_all_entries_newest_first_with_notes() {
        let conn = test_conn();
        let a = plan_on(&conn, "Steak", "2026-09-01");
        plan_on(&conn, "Steak", "2026-10-20");
        plan_on(&conn, "Chips", "2026-09-15");
        plan::update(&conn, a, Some(4), Some("rare was better")).unwrap();
        let h = history(&conn, item_id(&conn, "Steak")).unwrap();
        let dates: Vec<&str> = h.iter().map(|e| e.date.as_str()).collect();
        assert_eq!(dates, ["2026-10-20", "2026-09-01"]);
        assert_eq!((h[1].serves, h[1].note.as_deref()), (Some(4), Some("rare was better")));
        assert!(history(&conn, item_id(&conn, "Garden salad")).unwrap().is_empty());
    }

    #[test]
    fn rejects_bad_date() {
        let conn = test_conn();
        assert!(usage(&conn, "yesterday").is_err());
    }
}
