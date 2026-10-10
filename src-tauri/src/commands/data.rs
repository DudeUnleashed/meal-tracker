use std::path::PathBuf;

use rusqlite::Connection;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use super::backup;
use super::undo::UndoStack;
use crate::db::Db;

/// How much demo content is still in the app.
#[derive(Serialize, Debug, PartialEq)]
pub struct SampleCounts {
    pub items: i64,
    pub ingredients: i64,
    pub templates: i64,
    /// Planned dinners that use a sample item (these go with it).
    pub planned_entries: i64,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Removed {
    pub items: i64,
    pub templates: i64,
    pub ingredients: i64,
    pub plan_entries: i64,
    /// Sample ingredients kept because one of your own meals or templates uses them.
    pub ingredients_kept: i64,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Erased {
    pub items: i64,
    pub ingredients: i64,
    pub templates: i64,
    pub plan_entries: i64,
    pub day_notes: i64,
}

/// What a wipe did, plus where the automatic safety backup went.
#[derive(Serialize, Debug)]
pub struct Report<T> {
    pub result: T,
    pub backup_path: String,
}

fn count(conn: &Connection, sql: &str) -> Result<i64, String> {
    conn.query_row(sql, [], |r| r.get(0)).map_err(|e| e.to_string())
}

pub fn sample_counts(conn: &Connection) -> Result<SampleCounts, String> {
    Ok(SampleCounts {
        items: count(conn, "SELECT COUNT(*) FROM item WHERE sample = 1")?,
        ingredients: count(conn, "SELECT COUNT(*) FROM ingredient WHERE sample = 1")?,
        templates: count(conn, "SELECT COUNT(*) FROM template WHERE sample = 1")?,
        planned_entries: count(
            conn,
            "SELECT COUNT(*) FROM plan_entry WHERE item_id IN (SELECT id FROM item WHERE sample = 1)",
        )?,
    })
}

/// Removes the demo meals, templates and ingredients that shipped with the app, and any
/// dinners planned with them. Anything you made yourself stays, including sample ingredients
/// that your own meals or templates still use.
pub fn remove_sample(conn: &Connection) -> Result<Removed, String> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let run = |sql: &str| -> Result<i64, String> {
        tx.execute(sql, []).map(|n| n as i64).map_err(|e| e.to_string())
    };

    let plan_entries =
        run("DELETE FROM plan_entry WHERE item_id IN (SELECT id FROM item WHERE sample = 1)")?;
    // Sample templates and their chips go together. The chip is an item, so it is counted with
    // the items; the template must be deleted first because it points at the chip.
    run("UPDATE item SET template_id = NULL WHERE template_id IN (SELECT id FROM template WHERE sample = 1)")?;
    let templates = run("DELETE FROM template WHERE sample = 1")?;
    // Lines go with their item / template (ON DELETE CASCADE).
    let items = run("DELETE FROM item WHERE sample = 1")?;
    let ingredients = run(
        "DELETE FROM ingredient WHERE sample = 1
           AND id NOT IN (SELECT ingredient_id FROM item_ingredient)
           AND id NOT IN (SELECT ingredient_id FROM template_line WHERE ingredient_id IS NOT NULL)
           AND id NOT IN (SELECT ingredient_id FROM template_option)
           AND id NOT IN (SELECT ingredient_id FROM plan_entry_pick)",
    )?;
    // What's left is in use by the user's own things: it's theirs now.
    let ingredients_kept = run("UPDATE ingredient SET sample = 0 WHERE sample = 1")?;

    tx.commit().map_err(|e| e.to_string())?;
    Ok(Removed { items, templates, ingredients, plan_entries, ingredients_kept })
}

/// Deletes everything you've entered — meals, ingredients, templates, the whole plan and day
/// notes — leaving the ingredient categories and settings.
pub fn erase_all(conn: &Connection) -> Result<Erased, String> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let erased = Erased {
        items: count(&tx, "SELECT COUNT(*) FROM item")?,
        ingredients: count(&tx, "SELECT COUNT(*) FROM ingredient")?,
        templates: count(&tx, "SELECT COUNT(*) FROM template")?,
        plan_entries: count(&tx, "SELECT COUNT(*) FROM plan_entry")?,
        day_notes: count(&tx, "SELECT COUNT(*) FROM day_info")?,
    };
    // Templates and items point at each other (chip / "started from"); cut those links first.
    tx.execute("UPDATE template SET item_id = NULL", []).map_err(|e| e.to_string())?;
    tx.execute("UPDATE item SET template_id = NULL", []).map_err(|e| e.to_string())?;
    // Children before parents so foreign keys hold.
    for table in [
        "plan_entry_pick",
        "plan_entry",
        "day_info",
        "item_ingredient",
        "template_option",
        "template_line",
        "item",
        "template",
        "ingredient",
    ] {
        tx.execute(&format!("DELETE FROM {table}"), []).map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(erased)
}

fn backups_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_data_dir().map_err(|e| e.to_string())?.join("backups"))
}

#[tauri::command]
pub fn data_sample_counts(db: State<Db>) -> Result<SampleCounts, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    sample_counts(&conn)
}

#[tauri::command]
pub fn data_remove_sample(
    app: AppHandle,
    db: State<Db>,
    undo: State<UndoStack>,
) -> Result<Report<Removed>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let backup_path = backup::auto_backup(&conn, &backups_dir(&app)?, "before-removing-sample-data")?;
    let result = remove_sample(&conn)?;
    undo.clear();
    Ok(Report { result, backup_path: backup_path.display().to_string() })
}

#[tauri::command]
pub fn data_erase_all(
    app: AppHandle,
    db: State<Db>,
    undo: State<UndoStack>,
) -> Result<Report<Erased>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let backup_path = backup::auto_backup(&conn, &backups_dir(&app)?, "before-erasing-everything")?;
    let result = erase_all(&conn)?;
    undo.clear();
    Ok(Report { result, backup_path: backup_path.display().to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{items, plan, templates};
    use crate::db::test_conn;
    use rusqlite::params;

    fn id(conn: &Connection, table: &str, name: &str) -> i64 {
        conn.query_row(&format!("SELECT id FROM {table} WHERE name = ?1"), params![name], |r| r.get(0)).unwrap()
    }

    fn total(conn: &Connection, table: &str) -> i64 {
        count(conn, &format!("SELECT COUNT(*) FROM {table}")).unwrap()
    }

    #[test]
    fn seed_data_is_flagged_as_sample() {
        let conn = test_conn();
        let c = sample_counts(&conn).unwrap();
        assert_eq!((c.items, c.ingredients, c.templates, c.planned_entries), (10, 34, 2, 0));
        // Everything seeded is sample, so counts match the totals.
        assert_eq!((total(&conn, "item"), total(&conn, "ingredient"), total(&conn, "template")), (10, 34, 2));
    }

    #[test]
    fn removing_sample_data_keeps_what_you_made() {
        let mut conn = test_conn();
        // Your own meal that uses sample ingredients, plus your own ingredient.
        let leek = conn
            .execute("INSERT INTO ingredient (name, category_id, unit) VALUES ('Leek', 3, 'each')", [])
            .map(|_| conn.last_insert_rowid())
            .unwrap();
        let mince = id(&conn, "ingredient", "Beef mince");
        let input = items::ItemInput {
            name: "Nan's pie".into(),
            item_type: "meal".into(),
            colour: None,
            notes: None,
            template_id: None,
            lines: vec![
                items::LineInput { ingredient_id: mince, quantity: 400.0, group_by: false },
                items::LineInput { ingredient_id: leek, quantity: 2.0, group_by: false },
            ],
        };
        let pie = items::create(&mut conn, &input).unwrap();
        plan::add(&conn, pie.id, "2026-10-06", None).unwrap();
        plan::add(&conn, id(&conn, "item", "Steak"), "2026-10-06", Some(2)).unwrap(); // sample
        plan::add(&conn, id(&conn, "item", "Chips"), "2026-10-07", None).unwrap(); // sample

        assert_eq!(sample_counts(&conn).unwrap().planned_entries, 2);
        let r = remove_sample(&conn).unwrap();
        assert_eq!((r.items, r.templates, r.plan_entries), (10, 2, 2));
        assert_eq!(r.ingredients_kept, 1, "Beef mince is used by Nan's pie");
        assert_eq!(r.ingredients, 33);

        let names: Vec<String> =
            items::list(&conn, None, None, true).unwrap().into_iter().map(|i| i.name).collect();
        assert_eq!(names, ["Nan's pie"]);
        assert_eq!(plan::list(&conn, "2026-01-01", "2026-12-31").unwrap().len(), 1, "your own dinner stays");
        let kept: Vec<String> = conn
            .prepare("SELECT name FROM ingredient ORDER BY name").unwrap()
            .query_map([], |r| r.get(0)).unwrap().map(|n| n.unwrap()).collect();
        assert_eq!(kept, ["Beef mince", "Leek"]);
        assert_eq!(templates::list(&conn).unwrap().len(), 0);
        // The leftover sample ingredient is yours now, so a second removal does nothing more.
        assert_eq!(sample_counts(&conn).unwrap().ingredients, 0);
    }

    #[test]
    fn sample_ingredients_used_by_your_template_survive() {
        let mut conn = test_conn();
        let input = templates::TemplateInput {
            name: "Mine".into(),
            item_type: "meal".into(),
            colour: None,
            lines: vec![templates::LineInput {
                label: "Rice".into(),
                ingredient_id: Some(id(&conn, "ingredient", "Arborio rice")),
                quantity: Some(200.0),
                ..Default::default()
            }],
        };
        templates::create(&mut conn, &input).unwrap();
        // Sample templates are removed, and so is Risotto's link, but not the shared ingredient.
        let r = remove_sample(&conn).unwrap();
        assert_eq!(r.ingredients_kept, 1);
        assert_eq!(templates::list(&conn).unwrap().len(), 1);
    }

    #[test]
    fn erase_all_clears_everything_but_categories_and_settings() {
        let conn = test_conn();
        plan::add(&conn, id(&conn, "item", "Chips"), "2026-10-06", None).unwrap();
        crate::commands::dayinfo::set(&conn, "2026-10-06", Some("note"), Some(true)).unwrap();

        let e = erase_all(&conn).unwrap();
        assert_eq!((e.items, e.ingredients, e.templates, e.plan_entries, e.day_notes), (10, 34, 2, 1, 1));
        for t in ["item", "item_ingredient", "ingredient", "template", "template_line", "plan_entry", "day_info"] {
            assert_eq!(total(&conn, t), 0, "{t}");
        }
        assert_eq!(total(&conn, "ingredient_category"), 9);
        assert_eq!(total(&conn, "setting"), 2);
    }
}
