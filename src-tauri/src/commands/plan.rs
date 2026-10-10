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
    /// breakfast, lunch, dinner or snack.
    pub slot: String,
    /// Title of a Blank (takeaway etc.) entry.
    pub label: Option<String>,
    /// The template this chip comes from, for template chips.
    pub template_id: Option<i64>,
    /// What this night chose for the template's slots.
    pub picks: Vec<Pick>,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Pick {
    pub line_id: Option<i64>,
    pub ingredient_id: i64,
    pub ingredient_name: String,
    pub unit: String,
    pub quantity: f64,
}

pub const SLOTS: [&str; 4] = ["breakfast", "lunch", "dinner", "snack"];

fn check_slot(slot: &str) -> Result<(), String> {
    if SLOTS.contains(&slot) {
        Ok(())
    } else {
        Err(format!("Slot must be one of: {}", SLOTS.join(", ")))
    }
}

fn parse_date(s: &str) -> Result<String, String> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map(|d| d.format("%Y-%m-%d").to_string())
        .map_err(|_| format!("Invalid date: {s}"))
}

const SELECT: &str = "
    SELECT pe.id, pe.date, pe.item_id, i.name, i.type, i.colour, i.archived,
           pe.serves, pe.note, pe.position, pe.scale, pe.slot, pe.label, t.id
    FROM plan_entry pe
    JOIN item i ON i.id = pe.item_id
    LEFT JOIN template t ON t.item_id = i.id";

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
        slot: r.get(11)?,
        label: r.get(12)?,
        template_id: r.get(13)?,
        picks: vec![],
    })
}

/// Fills in each entry's picks (one query for the lot).
fn attach_picks(conn: &Connection, entries: &mut [PlanEntry]) -> Result<(), String> {
    let Some(first) = entries.first() else { return Ok(()) };
    let (mut lo, mut hi) = (first.date.clone(), first.date.clone());
    for e in entries.iter() {
        if e.date < lo {
            lo = e.date.clone();
        }
        if e.date > hi {
            hi = e.date.clone();
        }
    }
    let mut stmt = conn
        .prepare(
            "SELECT k.entry_id, k.line_id, k.ingredient_id, g.name, g.unit, k.quantity
             FROM plan_entry_pick k
             JOIN plan_entry pe ON pe.id = k.entry_id
             JOIN ingredient g ON g.id = k.ingredient_id
             WHERE pe.date BETWEEN ?1 AND ?2 ORDER BY k.entry_id, k.position, k.id",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![lo, hi], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                Pick {
                    line_id: r.get(1)?,
                    ingredient_id: r.get(2)?,
                    ingredient_name: r.get(3)?,
                    unit: r.get(4)?,
                    quantity: r.get(5)?,
                },
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    for (entry_id, pick) in rows {
        if let Some(e) = entries.iter_mut().find(|e| e.id == entry_id) {
            e.picks.push(pick);
        }
    }
    Ok(())
}

/// Entries dated `start..=end`, ordered by day then position (the app groups them by slot).
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
    let mut rows = rows;
    attach_picks(conn, &mut rows)?;
    Ok(rows)
}

fn get(conn: &Connection, id: i64) -> Result<PlanEntry, String> {
    let mut e = conn
        .query_row(&format!("{SELECT} WHERE pe.id = ?1"), params![id], from_row)
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => "Plan entry not found".to_string(),
            e => e.to_string(),
        })?;
    attach_picks(conn, std::slice::from_mut(&mut e))?;
    Ok(e)
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
    add_at(conn, item_id, date, None, serves, None)
}

/// Like [`add`], but into a chosen `slot` (default: dinner, or snack for Sweet/Savory items)
/// and at `position` in that slot (0-based, clamped) instead of the end, so dropping onto a
/// chip is a single step.
pub fn add_at(
    conn: &Connection,
    item_id: i64,
    date: &str,
    slot: Option<&str>,
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
    let slot = match slot {
        Some(s) => s,
        None if item_type == "sweet" => "snack",
        None => "dinner",
    };
    check_slot(slot)?;

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
            "SELECT COALESCE(MAX(position) + 1, 0) FROM plan_entry WHERE date = ?1 AND slot = ?2",
            params![date, slot],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO plan_entry (date, item_id, serves, position, slot) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![date, item_id, serves, end_position, slot],
    )
    .map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    match position {
        Some(p) => move_to(conn, id, &date, slot, p),
        None => get(conn, id),
    }
}

/// The Blank token (takeaway, eating out…): a plan entry with a typed title instead of a meal.
/// It rides on one hidden system item, created the first time it's needed.
pub fn add_blank(
    conn: &Connection,
    date: &str,
    slot: Option<&str>,
    label: Option<&str>,
    position: Option<i64>,
) -> Result<PlanEntry, String> {
    let existing: Option<i64> = conn
        .query_row("SELECT id FROM item WHERE type = 'blank' LIMIT 1", [], |r| r.get(0))
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            e => Err(e),
        })
        .map_err(|e| e.to_string())?;
    let item_id = match existing {
        Some(id) => id,
        None => {
            conn.execute("INSERT INTO item (name, type) VALUES ('Blank', 'blank')", [])
                .map_err(|e| e.to_string())?;
            conn.last_insert_rowid()
        }
    };
    let entry = add_at(conn, item_id, date, Some(slot.unwrap_or("dinner")), None, position)?;
    match label {
        Some(l) => set_label(conn, entry.id, l),
        None => Ok(entry),
    }
}

/// Sets (or, when blank, clears) the title of a Blank entry.
pub fn set_label(conn: &Connection, id: i64, label: &str) -> Result<PlanEntry, String> {
    let entry = get(conn, id)?;
    if entry.item_type != "blank" {
        return Err("Only Blank entries have a title".into());
    }
    let label = label.trim();
    conn.execute(
        "UPDATE plan_entry SET label = ?1 WHERE id = ?2",
        params![if label.is_empty() { None } else { Some(label) }, id],
    )
    .map_err(|e| e.to_string())?;
    get(conn, id)
}

/// Moves an entry to `date` at `position` (0-based; clamped to the end of that day).
///
/// Works within a day (reorder) and across days. Both affected days are renumbered 0..n
/// so positions stay contiguous.
pub fn move_entry(conn: &Connection, id: i64, date: &str, position: i64) -> Result<PlanEntry, String> {
    let slot = get(conn, id)?.slot;
    move_to(conn, id, date, &slot, position)
}

/// [`move_entry`] into a particular slot of the day. Each slot is numbered 0..n on its own.
pub fn move_to(conn: &Connection, id: i64, date: &str, slot: &str, position: i64) -> Result<PlanEntry, String> {
    let date = parse_date(date)?;
    check_slot(slot)?;
    let (old_date, old_slot): (String, String) = conn
        .query_row("SELECT date, slot FROM plan_entry WHERE id = ?1", params![id], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => "Plan entry not found".to_string(),
            e => e.to_string(),
        })?;

    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let mut ids = day_ids(&tx, &date, slot, Some(id))?;
    let at = position.clamp(0, ids.len() as i64) as usize;
    ids.insert(at, id);
    tx.execute("UPDATE plan_entry SET date = ?1, slot = ?2 WHERE id = ?3", params![date, slot, id])
        .map_err(|e| e.to_string())?;
    renumber(&tx, &ids)?;
    if old_date != date || old_slot != slot {
        let rest = day_ids(&tx, &old_date, &old_slot, None)?;
        renumber(&tx, &rest)?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    get(conn, id)
}

/// Entry ids in one slot of a day in display order, optionally leaving one out.
fn day_ids(conn: &Connection, date: &str, slot: &str, exclude: Option<i64>) -> Result<Vec<i64>, String> {
    let mut stmt = conn
        .prepare("SELECT id FROM plan_entry WHERE date = ?1 AND slot = ?2 AND id != ?3 ORDER BY position, id")
        .map_err(|e| e.to_string())?;
    let ids = stmt
        .query_map(params![date, slot, exclude.unwrap_or(-1)], |r| r.get(0))
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
                "SELECT COALESCE(MAX(position) + 1, 0) FROM plan_entry WHERE date = ?1 AND slot = ?2",
                params![new_date, e.slot],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        tx.execute(
            "INSERT INTO plan_entry (date, item_id, serves, position, scale, slot, label)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![new_date, e.item_id, e.serves, position, e.scale, e.slot, e.label],
        )
        .map_err(|e| e.to_string())?;
        let new_id = tx.last_insert_rowid();
        // What the night picked comes along (the amounts were copied in when picked).
        tx.execute(
            "INSERT INTO plan_entry_pick (entry_id, line_id, ingredient_id, quantity, position)
             SELECT ?1, line_id, ingredient_id, quantity, position FROM plan_entry_pick WHERE entry_id = ?2",
            params![new_id, e.id],
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

#[derive(serde::Deserialize, Clone, Debug)]
pub struct PickInput {
    pub line_id: Option<i64>,
    pub ingredient_id: i64,
    /// `None` uses the template's default for this ingredient.
    pub quantity: Option<f64>,
}

/// The amount a template suggests for an ingredient picked on a line: the line's candidate
/// amount, else the line's default amount when its unit hint matches the ingredient's unit.
fn default_quantity(conn: &Connection, line_id: i64, ingredient_id: i64, unit: &str) -> Result<Option<f64>, String> {
    let option: Option<Option<f64>> = conn
        .query_row(
            "SELECT quantity FROM template_option WHERE line_id = ?1 AND ingredient_id = ?2",
            params![line_id, ingredient_id],
            |r| r.get(0),
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            e => Err(e),
        })
        .map_err(|e| e.to_string())?;
    if let Some(Some(q)) = option {
        return Ok(Some(q));
    }
    let (q, hint): (Option<f64>, Option<String>) = conn
        .query_row("SELECT quantity, unit_hint FROM template_line WHERE id = ?1", params![line_id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .map_err(|e| e.to_string())?;
    Ok(match hint {
        Some(h) if h != unit => None,
        _ => q,
    })
}

/// Replaces what a template chip picked for its slots on this night.
pub fn set_picks(conn: &Connection, entry_id: i64, picks: &[PickInput]) -> Result<PlanEntry, String> {
    let entry = get(conn, entry_id)?;
    let Some(template_id) = entry.template_id else {
        return Err("Only template chips have ingredient choices".into());
    };
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM plan_entry_pick WHERE entry_id = ?1", params![entry_id]).map_err(|e| e.to_string())?;
    let mut seen: Vec<(Option<i64>, i64)> = Vec::new();
    for (pos, p) in picks.iter().enumerate() {
        if seen.contains(&(p.line_id, p.ingredient_id)) {
            continue; // the same ingredient ticked twice is one pick
        }
        seen.push((p.line_id, p.ingredient_id));
        let (name, unit): (String, String) = tx
            .query_row("SELECT name, unit FROM ingredient WHERE id = ?1", params![p.ingredient_id], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .map_err(|_| "Ingredient not found".to_string())?;
        let quantity = match (p.quantity, p.line_id) {
            (Some(q), _) => Some(q),
            (None, Some(line)) => {
                let owner: Option<i64> = tx
                    .query_row("SELECT template_id FROM template_line WHERE id = ?1", params![line], |r| r.get(0))
                    .ok();
                if owner != Some(template_id) {
                    return Err("That slot isn't part of this template".into());
                }
                default_quantity(&tx, line, p.ingredient_id, &unit)?
            }
            (None, None) => None,
        };
        let Some(q) = quantity.filter(|q| q.is_finite() && *q > 0.0) else {
            return Err(format!("Set an amount for {name}"));
        };
        tx.execute(
            "INSERT INTO plan_entry_pick (entry_id, line_id, ingredient_id, quantity, position)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![entry_id, p.line_id, p.ingredient_id, q, pos as i64],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    get(conn, entry_id)
}

fn entry_label(conn: &Connection, verb: &str, id: i64) -> String {
    match get(conn, id) {
        Ok(e) => format!("{verb} {}", e.label.as_deref().unwrap_or(&e.item_name)),
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
    slot: Option<String>,
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
        |c| add_at(c, item_id, &date, slot.as_deref(), serves, position),
    )
}

#[tauri::command]
pub fn plan_add_blank(
    db: State<Db>,
    undo: State<UndoStack>,
    date: String,
    slot: Option<String>,
    label: Option<String>,
    position: Option<i64>,
) -> Result<PlanEntry, String> {
    tracked(
        &db,
        &undo,
        |_| "Added a blank".to_string(),
        |c| add_blank(c, &date, slot.as_deref(), label.as_deref(), position),
    )
}

#[tauri::command]
pub fn plan_set_picks(
    db: State<Db>,
    undo: State<UndoStack>,
    id: i64,
    picks: Vec<PickInput>,
) -> Result<PlanEntry, String> {
    tracked(&db, &undo, |c| entry_label(c, "Changed", id), |c| set_picks(c, id, &picks))
}

#[tauri::command]
pub fn plan_move(
    db: State<Db>,
    undo: State<UndoStack>,
    id: i64,
    date: String,
    slot: Option<String>,
    position: i64,
) -> Result<PlanEntry, String> {
    tracked(&db, &undo, |c| entry_label(c, "Moved", id), |c| match slot.as_deref() {
        Some(slot) => move_to(c, id, &date, slot, position),
        None => move_entry(c, id, &date, position),
    })
}

#[tauri::command]
pub fn plan_update(
    db: State<Db>,
    undo: State<UndoStack>,
    id: i64,
    serves: Option<i64>,
    note: Option<String>,
    scale: Option<f64>,
    label: Option<String>,
) -> Result<PlanEntry, String> {
    tracked(
        &db,
        &undo,
        |c| entry_label(c, "Changed", id),
        |c| {
            update_with(c, id, serves, note.as_deref(), scale)?;
            match label.as_deref() {
                Some(l) => set_label(c, id, l),
                None => get(c, id),
            }
        },
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
        let e = add_at(&conn, item_id(&conn, "Garden salad"), "2026-10-06", None, None, Some(1)).unwrap();
        assert_eq!(e.position, 1);
        assert_eq!(names(&conn, "2026-10-06"), ["Steak", "Garden salad", "Chips"]);
        assert_eq!(positions(&conn, "2026-10-06"), [0, 1, 2]);
        add_at(&conn, item_id(&conn, "Steamed veg"), "2026-10-06", None, None, Some(99)).unwrap();
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
