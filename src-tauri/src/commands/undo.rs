use std::sync::Mutex;

use rusqlite::{params, Connection};
use tauri::State;

use crate::db::Db;

/// How many "whoops" steps are remembered (in memory only; cleared when the app closes).
const MAX_STEPS: usize = 30;

struct PlanRow {
    id: i64,
    date: String,
    item_id: i64,
    serves: Option<i64>,
    note: Option<String>,
    position: i64,
    scale: f64,
}

struct DayRow {
    date: String,
    note: Option<String>,
    leftovers: bool,
}

/// Everything undo can bring back: the dinner plan and the per-day notes / leftovers.
/// Items, ingredients and templates are not part of it — only the planning.
struct Snapshot {
    label: String,
    plan: Vec<PlanRow>,
    days: Vec<DayRow>,
}

#[derive(Default)]
pub struct UndoStack(Mutex<Vec<Snapshot>>);

fn take_snapshot(conn: &Connection, label: String) -> Result<Snapshot, String> {
    let mut stmt = conn
        .prepare("SELECT id, date, item_id, serves, note, position, scale FROM plan_entry ORDER BY id")
        .map_err(|e| e.to_string())?;
    let plan = stmt
        .query_map([], |r| {
            Ok(PlanRow {
                id: r.get(0)?,
                date: r.get(1)?,
                item_id: r.get(2)?,
                serves: r.get(3)?,
                note: r.get(4)?,
                position: r.get(5)?,
                scale: r.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT date, note, leftovers FROM day_info ORDER BY date")
        .map_err(|e| e.to_string())?;
    let days = stmt
        .query_map([], |r| {
            Ok(DayRow { date: r.get(0)?, note: r.get(1)?, leftovers: r.get::<_, i64>(2)? != 0 })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(Snapshot { label, plan, days })
}

fn restore(conn: &Connection, s: &Snapshot) -> Result<(), String> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM plan_entry", []).map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM day_info", []).map_err(|e| e.to_string())?;
    for p in &s.plan {
        tx.execute(
            "INSERT INTO plan_entry (id, date, item_id, serves, note, position, scale)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![p.id, p.date, p.item_id, p.serves, p.note, p.position, p.scale],
        )
        .map_err(|e| e.to_string())?;
    }
    for d in &s.days {
        tx.execute(
            "INSERT INTO day_info (date, note, leftovers) VALUES (?1, ?2, ?3)",
            params![d.date, d.note, d.leftovers],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())
}

impl UndoStack {
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Vec<Snapshot>>, String> {
        self.0.lock().map_err(|e| e.to_string())
    }

    /// Remembers the current plan, labelled with what is about to change.
    pub fn record(&self, conn: &Connection, label: String) -> Result<(), String> {
        let snap = take_snapshot(conn, label)?;
        let mut stack = self.lock()?;
        stack.push(snap);
        if stack.len() > MAX_STEPS {
            stack.remove(0);
        }
        Ok(())
    }

    /// Drops the newest step (the change it guarded failed, so there is nothing to undo).
    pub fn discard_last(&self) {
        if let Ok(mut stack) = self.lock() {
            stack.pop();
        }
    }

    /// What Undo would revert, e.g. "Removed Steak".
    pub fn peek(&self) -> Result<Option<String>, String> {
        Ok(self.lock()?.last().map(|s| s.label.clone()))
    }

    /// Forget everything — used when data is wiped or restored, since old snapshots would
    /// refer to items that no longer exist.
    pub fn clear(&self) {
        if let Ok(mut stack) = self.lock() {
            stack.clear();
        }
    }

    /// Reverts the newest step and returns its label, or `None` if there is nothing to undo.
    pub fn undo(&self, conn: &Connection) -> Result<Option<String>, String> {
        let mut stack = self.lock()?;
        let Some(snap) = stack.last() else { return Ok(None) };
        restore(conn, snap)?;
        Ok(stack.pop().map(|s| s.label))
    }
}

/// Runs a plan change so it can be undone: snapshots first, and forgets the snapshot if the
/// change fails. `label` is computed before the change (so it can look up names).
pub fn tracked<T>(
    db: &Db,
    undo: &UndoStack,
    label: impl FnOnce(&Connection) -> String,
    change: impl FnOnce(&Connection) -> Result<T, String>,
) -> Result<T, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    undo.record(&conn, label(&conn))?;
    change(&conn).inspect_err(|_| undo.discard_last())
}

#[tauri::command]
pub fn plan_undo(db: State<Db>, undo: State<UndoStack>) -> Result<Option<String>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    undo.undo(&conn)
}

#[tauri::command]
pub fn plan_undo_peek(undo: State<UndoStack>) -> Result<Option<String>, String> {
    undo.peek()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{dayinfo, plan};
    use crate::db::test_conn;

    fn item_id(conn: &Connection, name: &str) -> i64 {
        conn.query_row("SELECT id FROM item WHERE name = ?1", params![name], |r| r.get(0)).unwrap()
    }

    fn names(conn: &Connection) -> Vec<String> {
        plan::list(conn, "2026-01-01", "2026-12-31").unwrap().into_iter().map(|e| e.item_name).collect()
    }

    #[test]
    fn undo_restores_a_removed_entry_exactly() {
        let conn = test_conn();
        let undo = UndoStack::default();
        let e = plan::add(&conn, item_id(&conn, "Steak"), "2026-10-06", Some(4)).unwrap();
        plan::update(&conn, e.id, None, Some("use rump")).unwrap();

        undo.record(&conn, "Removed Steak".into()).unwrap();
        plan::remove(&conn, e.id).unwrap();
        assert!(names(&conn).is_empty());

        assert_eq!(undo.peek().unwrap().as_deref(), Some("Removed Steak"));
        assert_eq!(undo.undo(&conn).unwrap().as_deref(), Some("Removed Steak"));
        let back = plan::list(&conn, "2026-10-06", "2026-10-06").unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!((back[0].id, back[0].serves, back[0].note.as_deref()), (e.id, Some(4), Some("use rump")));
        assert_eq!(undo.peek().unwrap(), None);
        assert_eq!(undo.undo(&conn).unwrap(), None, "nothing left to undo");
    }

    #[test]
    fn undo_steps_back_one_change_at_a_time() {
        let conn = test_conn();
        let undo = UndoStack::default();
        undo.record(&conn, "Added Chips".into()).unwrap();
        let chips = plan::add(&conn, item_id(&conn, "Chips"), "2026-10-06", None).unwrap();
        undo.record(&conn, "Moved Chips".into()).unwrap();
        plan::move_entry(&conn, chips.id, "2026-10-09", 0).unwrap();

        undo.undo(&conn).unwrap();
        assert_eq!(plan::list(&conn, "2026-10-06", "2026-10-06").unwrap().len(), 1, "move undone");
        undo.undo(&conn).unwrap();
        assert!(names(&conn).is_empty(), "add undone");
    }

    #[test]
    fn undo_covers_day_notes_and_leftovers() {
        let conn = test_conn();
        let undo = UndoStack::default();
        dayinfo::set(&conn, "2026-10-06", Some("old"), Some(true)).unwrap();
        undo.record(&conn, "Changed the note".into()).unwrap();
        dayinfo::set(&conn, "2026-10-06", Some("new"), Some(false)).unwrap();
        undo.undo(&conn).unwrap();
        let d = dayinfo::list(&conn, "2026-10-06", "2026-10-06").unwrap();
        assert_eq!((d[0].note.as_deref(), d[0].leftovers), (Some("old"), true));
    }

    #[test]
    fn a_failed_change_leaves_nothing_to_undo() {
        let conn = test_conn();
        let undo = UndoStack::default();
        undo.record(&conn, "Added nothing".into()).unwrap();
        // Emulates `tracked` when the change returns an error.
        let result = plan::add(&conn, 9999, "2026-10-06", None);
        assert!(result.is_err());
        undo.discard_last();
        assert_eq!(undo.peek().unwrap(), None);
    }

    #[test]
    fn only_the_most_recent_steps_are_kept() {
        let conn = test_conn();
        let undo = UndoStack::default();
        for i in 0..(MAX_STEPS + 5) {
            undo.record(&conn, format!("step {i}")).unwrap();
        }
        let mut count = 0;
        while undo.undo(&conn).unwrap().is_some() {
            count += 1;
        }
        assert_eq!(count, MAX_STEPS);
    }

    #[test]
    fn clear_forgets_everything() {
        let conn = test_conn();
        let undo = UndoStack::default();
        undo.record(&conn, "x".into()).unwrap();
        undo.clear();
        assert_eq!(undo.peek().unwrap(), None);
    }
}
