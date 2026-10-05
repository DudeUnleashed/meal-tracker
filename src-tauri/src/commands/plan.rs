use chrono::NaiveDate;
use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::State;

use super::settings;
use super::undo::{tracked, UndoStack};
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
    /// Size multiplier for meal and side entries (1.0 = the usual one night). Meat uses serves.
    pub scale: f64,
}

fn parse_date(s: &str) -> Result<String, String> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map(|d| d.format("%Y-%m-%d").to_string())
        .map_err(|_| format!("Invalid date: {s}"))
}

const SELECT: &str = "
    SELECT pe.id, pe.date, pe.item_id, i.name, i.type, i.colour, i.archived,
           pe.serves, pe.note, pe.position, pe.scale
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
        scale: r.get(10)?,
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
#[cfg_attr(not(test), allow(dead_code))]
pub fn add(
    conn: &Connection,
    item_id: i64,
    date: &str,
    serves: Option<i64>,
) -> Result<PlanEntry, String> {
    add_at(conn, item_id, date, serves, None)
}

/// Like [`add`], but puts the entry at `position` in the day (0-based, clamped) instead of
/// the end, so dropping onto a chip is a single step.
pub fn add_at(
    conn: &Connection,
    item_id: i64,
    date: &str,
    serves: Option<i64>,
    position: Option<i64>,
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

    let end_position: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(position) + 1, 0) FROM plan_entry WHERE date = ?1",
            params![date],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO plan_entry (date, item_id, serves, position) VALUES (?1, ?2, ?3, ?4)",
        params![date, item_id, serves, end_position],
    )
    .map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    match position {
        Some(p) => move_entry(conn, id, &date, p),
        None => get(conn, id),
    }
}

/// Moves an entry to `date` at `position` (0-based; clamped to the end of that day).
///
/// Works within a day (reorder) and across days. Both affected days are renumbered 0..n
/// so positions stay contiguous.
pub fn move_entry(conn: &Connection, id: i64, date: &str, position: i64) -> Result<PlanEntry, String> {
    let date = parse_date(date)?;
    let old_date: String = conn
        .query_row("SELECT date FROM plan_entry WHERE id = ?1", params![id], |r| r.get(0))
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => "Plan entry not found".to_string(),
            e => e.to_string(),
        })?;

    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let mut ids = day_ids(&tx, &date, Some(id))?;
    let at = position.clamp(0, ids.len() as i64) as usize;
    ids.insert(at, id);
    tx.execute("UPDATE plan_entry SET date = ?1 WHERE id = ?2", params![date, id])
        .map_err(|e| e.to_string())?;
    renumber(&tx, &ids)?;
    if old_date != date {
        let rest = day_ids(&tx, &old_date, None)?;
        renumber(&tx, &rest)?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    get(conn, id)
}

/// Entry ids on a day in display order, optionally leaving one out.
fn day_ids(conn: &Connection, date: &str, exclude: Option<i64>) -> Result<Vec<i64>, String> {
    let mut stmt = conn
        .prepare("SELECT id FROM plan_entry WHERE date = ?1 AND id != ?2 ORDER BY position, id")
        .map_err(|e| e.to_string())?;
    let ids = stmt
        .query_map(params![date, exclude.unwrap_or(-1)], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<i64>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(ids)
}

fn renumber(conn: &Connection, ids: &[i64]) -> Result<(), String> {
    for (pos, id) in ids.iter().enumerate() {
        conn.execute("UPDATE plan_entry SET position = ?1 WHERE id = ?2", params![pos as i64, id])
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Changes serves (meat entries only) and/or the entry's note.
///
/// `note: Some("")` (or whitespace) clears the note; `None` leaves it unchanged.
#[cfg_attr(not(test), allow(dead_code))]
pub fn update(
    conn: &Connection,
    id: i64,
    serves: Option<i64>,
    note: Option<&str>,
) -> Result<PlanEntry, String> {
    update_with(conn, id, serves, note, None)
}

/// Largest "make it bigger" multiplier we accept (a sanity bound, not a real limit).
const MAX_SCALE: f64 = 10.0;

/// [`update`] plus the size multiplier for meal and side entries (`None` leaves it unchanged).
pub fn update_with(
    conn: &Connection,
    id: i64,
    serves: Option<i64>,
    note: Option<&str>,
    scale: Option<f64>,
) -> Result<PlanEntry, String> {
    let entry = get(conn, id)?;
    if let Some(sc) = scale {
        if entry.item_type == "meat" {
            return Err("Meat entries are sized by serves, not a multiplier".into());
        }
        if !(sc.is_finite() && sc > 0.0 && sc <= MAX_SCALE) {
            return Err(format!("Size must be more than 0 and at most {MAX_SCALE}"));
        }
        conn.execute("UPDATE plan_entry SET scale = ?1 WHERE id = ?2", params![sc, id])
            .map_err(|e| e.to_string())?;
    }
    if let Some(n) = serves {
        if entry.item_type != "meat" {
            return Err("Only meat entries have serves".into());
        }
        if n < 1 {
            return Err("Serves must be at least 1".into());
        }
        conn.execute("UPDATE plan_entry SET serves = ?1 WHERE id = ?2", params![n, id])
            .map_err(|e| e.to_string())?;
    }
    if let Some(note) = note {
        let note = note.trim();
        conn.execute(
            "UPDATE plan_entry SET note = ?1 WHERE id = ?2",
            params![if note.is_empty() { None } else { Some(note) }, id],
        )
        .map_err(|e| e.to_string())?;
    }
    get(conn, id)
}

#[derive(Serialize, Debug, PartialEq)]
pub struct CopyResult {
    pub copied: i64,
    /// Entries left out because their item has been archived since.
    pub skipped_archived: i64,
}

/// Copies every entry dated `src_start..=src_end` so the range starts on `dest_start` instead
/// (the same weekday pattern, shifted). Serves and size are kept, and so are Leftovers markers
/// (so "lasagna, then leftovers" repeats); notes are not, since a note is about one particular night. Copies go to the end of their day, after anything already there.
pub fn copy_range(
    conn: &Connection,
    src_start: &str,
    src_end: &str,
    dest_start: &str,
) -> Result<CopyResult, String> {
    let (start, end) = (parse_date(src_start)?, parse_date(src_end)?);
    let dest = parse_date(dest_start)?;
    let parse = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|e| e.to_string());
    let offset = parse(&dest)?.signed_duration_since(parse(&start)?);
    if parse(&start)? > parse(&end)? {
        return Err("Start date must be on or before the end date".into());
    }
    if offset.num_days() == 0 {
        return Err("Pick a different destination than the source".into());
    }

    let entries = list(conn, &start, &end)?;
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let mut result = CopyResult { copied: 0, skipped_archived: 0 };
    for e in entries {
        if e.item_archived {
            result.skipped_archived += 1;
            continue;
        }
        let new_date = (parse(&e.date)? + offset).format("%Y-%m-%d").to_string();
        let position: i64 = tx
            .query_row(
                "SELECT COALESCE(MAX(position) + 1, 0) FROM plan_entry WHERE date = ?1",
                params![new_date],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        tx.execute(
            "INSERT INTO plan_entry (date, item_id, serves, position, scale) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![new_date, e.item_id, e.serves, position, e.scale],
        )
        .map_err(|e| e.to_string())?;
        result.copied += 1;
    }
    for day in super::dayinfo::list(&tx, &start, &end)? {
        if day.leftovers {
            let new_date = (parse(&day.date)? + offset).format("%Y-%m-%d").to_string();
            super::dayinfo::set(&tx, &new_date, None, Some(true))?;
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(result)
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

fn entry_label(conn: &Connection, verb: &str, id: i64) -> String {
    match get(conn, id) {
        Ok(e) => format!("{verb} {}", e.item_name),
        Err(_) => format!("{verb} a dinner"),
    }
}

#[tauri::command]
pub fn plan_list(db: State<Db>, start: String, end: String) -> Result<Vec<PlanEntry>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    list(&conn, &start, &end)
}

#[tauri::command]
pub fn plan_add(
    db: State<Db>,
    undo: State<UndoStack>,
    item_id: i64,
    date: String,
    serves: Option<i64>,
    position: Option<i64>,
) -> Result<PlanEntry, String> {
    tracked(
        &db,
        &undo,
        |c| {
            let name: Result<String, _> =
                c.query_row("SELECT name FROM item WHERE id = ?1", params![item_id], |r| r.get(0));
            format!("Added {}", name.unwrap_or_else(|_| "a dinner".into()))
        },
        |c| add_at(c, item_id, &date, serves, position),
    )
}

#[tauri::command]
pub fn plan_move(
    db: State<Db>,
    undo: State<UndoStack>,
    id: i64,
    date: String,
    position: i64,
) -> Result<PlanEntry, String> {
    tracked(&db, &undo, |c| entry_label(c, "Moved", id), |c| move_entry(c, id, &date, position))
}

#[tauri::command]
pub fn plan_update(
    db: State<Db>,
    undo: State<UndoStack>,
    id: i64,
    serves: Option<i64>,
    note: Option<String>,
    scale: Option<f64>,
) -> Result<PlanEntry, String> {
    tracked(
        &db,
        &undo,
        |c| entry_label(c, "Changed", id),
        |c| update_with(c, id, serves, note.as_deref(), scale),
    )
}

#[tauri::command]
pub fn plan_copy_range(
    db: State<Db>,
    undo: State<UndoStack>,
    src_start: String,
    src_end: String,
    dest_start: String,
) -> Result<CopyResult, String> {
    tracked(
        &db,
        &undo,
        |_| "Copied dinners".to_string(),
        |c| copy_range(c, &src_start, &src_end, &dest_start),
    )
}

#[tauri::command]
pub fn plan_remove(db: State<Db>, undo: State<UndoStack>, id: i64) -> Result<(), String> {
    tracked(&db, &undo, |c| entry_label(c, "Removed", id), |c| remove(c, id))
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

    fn names(conn: &Connection, date: &str) -> Vec<String> {
        list(conn, date, date).unwrap().into_iter().map(|e| e.item_name).collect()
    }

    fn positions(conn: &Connection, date: &str) -> Vec<i64> {
        list(conn, date, date).unwrap().iter().map(|e| e.position).collect()
    }

    /// Steak, Chips, Garden salad on 6 Oct.
    fn setup_day(conn: &Connection) -> Vec<PlanEntry> {
        ["Steak", "Chips", "Garden salad"]
            .iter()
            .map(|n| add(conn, item_id(conn, n), "2026-10-06", None).unwrap())
            .collect()
    }

    #[test]
    fn move_reorders_within_a_day() {
        let conn = test_conn();
        let e = setup_day(&conn);
        move_entry(&conn, e[2].id, "2026-10-06", 0).unwrap();
        assert_eq!(names(&conn, "2026-10-06"), ["Garden salad", "Steak", "Chips"]);
        move_entry(&conn, e[2].id, "2026-10-06", 99).unwrap(); // clamps to the end
        assert_eq!(names(&conn, "2026-10-06"), ["Steak", "Chips", "Garden salad"]);
        move_entry(&conn, e[0].id, "2026-10-06", 1).unwrap();
        assert_eq!(names(&conn, "2026-10-06"), ["Chips", "Steak", "Garden salad"]);
        assert_eq!(positions(&conn, "2026-10-06"), [0, 1, 2]);
    }

    #[test]
    fn move_across_days_renumbers_both() {
        let conn = test_conn();
        let e = setup_day(&conn);
        let other = add(&conn, item_id(&conn, "Spag bol"), "2026-10-07", None).unwrap();

        let moved = move_entry(&conn, e[0].id, "2026-10-07", 0).unwrap();
        assert_eq!(moved.date, "2026-10-07");
        assert_eq!(names(&conn, "2026-10-07"), ["Steak", "Spag bol"]);
        assert_eq!(names(&conn, "2026-10-06"), ["Chips", "Garden salad"]);
        assert_eq!(positions(&conn, "2026-10-06"), [0, 1], "source day has no gap");
        assert_eq!(get(&conn, other.id).unwrap().position, 1);
        assert_eq!(moved.serves, Some(3), "serves survive a move");
    }

    #[test]
    fn move_to_an_empty_day_and_errors() {
        let conn = test_conn();
        let e = setup_day(&conn);
        let m = move_entry(&conn, e[1].id, "2026-12-25", 5).unwrap();
        assert_eq!((m.date.as_str(), m.position), ("2026-12-25", 0));
        assert_eq!(move_entry(&conn, 9999, "2026-10-06", 0).unwrap_err(), "Plan entry not found");
        assert!(move_entry(&conn, e[0].id, "bad", 0).is_err());
    }

    #[test]
    fn update_serves_and_note() {
        let conn = test_conn();
        let steak = add(&conn, item_id(&conn, "Steak"), "2026-10-06", None).unwrap();
        let u = update(&conn, steak.id, Some(5), Some("  use rump  ")).unwrap();
        assert_eq!((u.serves, u.note.as_deref()), (Some(5), Some("use rump")));
        // None leaves fields alone.
        let u = update(&conn, steak.id, None, None).unwrap();
        assert_eq!((u.serves, u.note.as_deref()), (Some(5), Some("use rump")));
        // Blank clears the note.
        let u = update(&conn, steak.id, None, Some("   ")).unwrap();
        assert_eq!(u.note, None);

        assert!(update(&conn, steak.id, Some(0), None).is_err());
        let chips = add(&conn, item_id(&conn, "Chips"), "2026-10-06", None).unwrap();
        assert_eq!(update(&conn, chips.id, Some(2), None).unwrap_err(), "Only meat entries have serves");
        assert!(update(&conn, chips.id, None, Some("crispy")).is_ok());
        assert_eq!(update(&conn, 9999, None, None).unwrap_err(), "Plan entry not found");
    }

    #[test]
    fn copy_range_shifts_dates_and_keeps_serves_but_not_notes() {
        let conn = test_conn();
        let steak = add(&conn, item_id(&conn, "Steak"), "2026-10-05", Some(4)).unwrap();
        update(&conn, steak.id, None, Some("use rump")).unwrap();
        add(&conn, item_id(&conn, "Chips"), "2026-10-05", None).unwrap();
        add(&conn, item_id(&conn, "Spag bol"), "2026-10-07", None).unwrap();
        add(&conn, item_id(&conn, "Chow mein"), "2026-10-12", None).unwrap(); // outside the source

        let r = copy_range(&conn, "2026-10-05", "2026-10-11", "2026-10-12").unwrap();
        assert_eq!(r, CopyResult { copied: 3, skipped_archived: 0 });

        let monday = list(&conn, "2026-10-12", "2026-10-12").unwrap();
        // The pre-existing Chow mein stays first; copies are appended in their original order.
        let names: Vec<&str> = monday.iter().map(|e| e.item_name.as_str()).collect();
        assert_eq!(names, ["Chow mein", "Steak", "Chips"]);
        assert_eq!(monday[1].serves, Some(4));
        assert_eq!(monday[1].note, None, "notes are not copied");
        assert_eq!(positions(&conn, "2026-10-12"), [0, 1, 2]);
        assert_eq!(names_on(&conn, "2026-10-14"), ["Spag bol"]);
        // The source is untouched.
        assert_eq!(names_on(&conn, "2026-10-05"), ["Steak", "Chips"]);
    }

    fn names_on(conn: &Connection, date: &str) -> Vec<String> {
        names(conn, date)
    }

    #[test]
    fn copy_range_can_go_backwards_and_skips_archived_items() {
        let conn = test_conn();
        add(&conn, item_id(&conn, "Chips"), "2026-10-13", None).unwrap();
        add(&conn, item_id(&conn, "Steak"), "2026-10-14", None).unwrap();
        conn.execute("UPDATE item SET archived = 1 WHERE name = 'Chips'", []).unwrap();
        let r = copy_range(&conn, "2026-10-12", "2026-10-18", "2026-10-05").unwrap();
        assert_eq!(r, CopyResult { copied: 1, skipped_archived: 1 });
        assert_eq!(names_on(&conn, "2026-10-07"), ["Steak"]);
        assert!(names_on(&conn, "2026-10-06").is_empty());
    }

    #[test]
    fn copy_range_rejects_bad_input() {
        let conn = test_conn();
        assert!(copy_range(&conn, "2026-10-05", "2026-10-11", "2026-10-05").is_err(), "same destination");
        assert!(copy_range(&conn, "2026-10-11", "2026-10-05", "2026-10-12").is_err(), "reversed range");
        assert!(copy_range(&conn, "x", "2026-10-05", "2026-10-12").is_err());
        // Copying an empty range is fine and does nothing.
        assert_eq!(
            copy_range(&conn, "2026-10-05", "2026-10-11", "2026-10-12").unwrap(),
            CopyResult { copied: 0, skipped_archived: 0 }
        );
    }

    #[test]
    fn add_at_inserts_at_the_given_position() {
        let conn = test_conn();
        add(&conn, item_id(&conn, "Steak"), "2026-10-06", None).unwrap();
        add(&conn, item_id(&conn, "Chips"), "2026-10-06", None).unwrap();
        let e = add_at(&conn, item_id(&conn, "Garden salad"), "2026-10-06", None, Some(1)).unwrap();
        assert_eq!(e.position, 1);
        assert_eq!(names(&conn, "2026-10-06"), ["Steak", "Garden salad", "Chips"]);
        assert_eq!(positions(&conn, "2026-10-06"), [0, 1, 2]);
        add_at(&conn, item_id(&conn, "Steamed veg"), "2026-10-06", None, Some(99)).unwrap();
        assert_eq!(names(&conn, "2026-10-06")[3], "Steamed veg", "clamped to the end");
    }

    #[test]
    fn scale_applies_to_meals_and_sides_only() {
        let conn = test_conn();
        let bol = add(&conn, item_id(&conn, "Spag bol"), "2026-10-06", None).unwrap();
        assert_eq!(bol.scale, 1.0, "every meal defaults to one night's size");
        let u = update_with(&conn, bol.id, None, None, Some(1.5)).unwrap();
        assert_eq!(u.scale, 1.5);
        assert_eq!(update_with(&conn, bol.id, None, Some("x"), None).unwrap().scale, 1.5, "unchanged when omitted");

        for bad in [0.0, -1.0, 10.5, f64::NAN] {
            assert!(update_with(&conn, bol.id, None, None, Some(bad)).is_err(), "{bad}");
        }
        let steak = add(&conn, item_id(&conn, "Steak"), "2026-10-06", None).unwrap();
        assert!(update_with(&conn, steak.id, None, None, Some(2.0)).is_err(), "meat is sized by serves");
        let chips = add(&conn, item_id(&conn, "Chips"), "2026-10-06", None).unwrap();
        assert_eq!(update_with(&conn, chips.id, None, None, Some(2.0)).unwrap().scale, 2.0);
    }

    #[test]
    fn copy_range_keeps_size_and_leftovers() {
        use crate::commands::dayinfo;
        let conn = test_conn();
        let bol = add(&conn, item_id(&conn, "Spag bol"), "2026-10-05", None).unwrap();
        update_with(&conn, bol.id, None, None, Some(2.0)).unwrap();
        dayinfo::set(&conn, "2026-10-06", Some("Alex working late"), Some(true)).unwrap();
        dayinfo::set(&conn, "2026-10-08", Some("just a note"), None).unwrap();

        copy_range(&conn, "2026-10-05", "2026-10-11", "2026-10-12").unwrap();
        assert_eq!(list(&conn, "2026-10-12", "2026-10-12").unwrap()[0].scale, 2.0);
        let days = dayinfo::list(&conn, "2026-10-12", "2026-10-18").unwrap();
        assert_eq!(days.len(), 1, "only the leftovers marker is copied, not notes");
        assert_eq!((days[0].date.as_str(), days[0].leftovers, days[0].note.as_deref()), ("2026-10-13", true, None));
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
