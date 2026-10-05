use std::fs;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tauri::State;

use super::settings;
use crate::db::Db;

/// Bump when the backup layout changes in a way old files can't be read.
const VERSION: u32 = 1;

// One struct per table, keeping original ids so every reference survives a restore.

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct CategoryRow {
    id: i64,
    name: String,
    sort_order: i64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct IngredientRow {
    id: i64,
    name: String,
    category_id: i64,
    unit: String,
    archived: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct TemplateRow {
    id: i64,
    name: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct TemplateLineRow {
    id: i64,
    template_id: i64,
    label: String,
    ingredient_id: Option<i64>,
    category_id: Option<i64>,
    quantity: Option<f64>,
    unit_hint: Option<String>,
    position: i64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct ItemRow {
    id: i64,
    name: String,
    #[serde(rename = "type")]
    item_type: String,
    colour: Option<String>,
    notes: Option<String>,
    template_id: Option<i64>,
    archived: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct ItemIngredientRow {
    id: i64,
    item_id: i64,
    ingredient_id: i64,
    quantity: f64,
    position: i64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct PlanEntryRow {
    id: i64,
    date: String,
    item_id: i64,
    serves: Option<i64>,
    note: Option<String>,
    position: i64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct SettingRow {
    key: String,
    value: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Backup {
    version: u32,
    exported_at: String,
    categories: Vec<CategoryRow>,
    ingredients: Vec<IngredientRow>,
    templates: Vec<TemplateRow>,
    template_lines: Vec<TemplateLineRow>,
    items: Vec<ItemRow>,
    item_ingredients: Vec<ItemIngredientRow>,
    plan_entries: Vec<PlanEntryRow>,
    settings: Vec<SettingRow>,
}

/// What an import brought in, for the confirmation message.
#[derive(Serialize, Debug, PartialEq)]
pub struct ImportSummary {
    pub items: usize,
    pub ingredients: usize,
    pub templates: usize,
    pub plan_entries: usize,
}

fn rows<T>(
    conn: &Connection,
    sql: &str,
    map: impl FnMut(&rusqlite::Row) -> rusqlite::Result<T>,
) -> Result<Vec<T>, String> {
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let out = stmt
        .query_map([], map)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(out)
}

fn collect(conn: &Connection) -> Result<Backup, String> {
    Ok(Backup {
        version: VERSION,
        exported_at: chrono::Utc::now().to_rfc3339(),
        categories: rows(conn, "SELECT id, name, sort_order FROM ingredient_category ORDER BY id", |r| {
            Ok(CategoryRow { id: r.get(0)?, name: r.get(1)?, sort_order: r.get(2)? })
        })?,
        ingredients: rows(conn, "SELECT id, name, category_id, unit, archived FROM ingredient ORDER BY id", |r| {
            Ok(IngredientRow {
                id: r.get(0)?,
                name: r.get(1)?,
                category_id: r.get(2)?,
                unit: r.get(3)?,
                archived: r.get::<_, i64>(4)? != 0,
            })
        })?,
        templates: rows(conn, "SELECT id, name FROM template ORDER BY id", |r| {
            Ok(TemplateRow { id: r.get(0)?, name: r.get(1)? })
        })?,
        template_lines: rows(
            conn,
            "SELECT id, template_id, label, ingredient_id, category_id, quantity, unit_hint, position
             FROM template_line ORDER BY id",
            |r| {
                Ok(TemplateLineRow {
                    id: r.get(0)?,
                    template_id: r.get(1)?,
                    label: r.get(2)?,
                    ingredient_id: r.get(3)?,
                    category_id: r.get(4)?,
                    quantity: r.get(5)?,
                    unit_hint: r.get(6)?,
                    position: r.get(7)?,
                })
            },
        )?,
        items: rows(
            conn,
            "SELECT id, name, type, colour, notes, template_id, archived FROM item ORDER BY id",
            |r| {
                Ok(ItemRow {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    item_type: r.get(2)?,
                    colour: r.get(3)?,
                    notes: r.get(4)?,
                    template_id: r.get(5)?,
                    archived: r.get::<_, i64>(6)? != 0,
                })
            },
        )?,
        item_ingredients: rows(
            conn,
            "SELECT id, item_id, ingredient_id, quantity, position FROM item_ingredient ORDER BY id",
            |r| {
                Ok(ItemIngredientRow {
                    id: r.get(0)?,
                    item_id: r.get(1)?,
                    ingredient_id: r.get(2)?,
                    quantity: r.get(3)?,
                    position: r.get(4)?,
                })
            },
        )?,
        plan_entries: rows(
            conn,
            "SELECT id, date, item_id, serves, note, position FROM plan_entry ORDER BY id",
            |r| {
                Ok(PlanEntryRow {
                    id: r.get(0)?,
                    date: r.get(1)?,
                    item_id: r.get(2)?,
                    serves: r.get(3)?,
                    note: r.get(4)?,
                    position: r.get(5)?,
                })
            },
        )?,
        settings: rows(conn, "SELECT key, value FROM setting ORDER BY key", |r| {
            Ok(SettingRow { key: r.get(0)?, value: r.get(1)? })
        })?,
    })
}

/// The whole database as pretty-printed JSON.
pub fn export_json(conn: &Connection) -> Result<String, String> {
    serde_json::to_string_pretty(&collect(conn)?).map_err(|e| e.to_string())
}

/// Replaces **all** data with the backup's. All-or-nothing: if anything in the file is
/// invalid (wrong version, bad values, dangling references) nothing is changed.
pub fn import_json(conn: &Connection, json: &str) -> Result<ImportSummary, String> {
    let b: Backup = serde_json::from_str(json).map_err(|e| format!("Not a valid backup file: {e}"))?;
    if b.version != VERSION {
        return Err(format!("Unsupported backup version {} (this app reads version {VERSION})", b.version));
    }

    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    // Children before parents so foreign keys hold while clearing.
    for table in [
        "plan_entry",
        "item_ingredient",
        "template_line",
        "item",
        "template",
        "ingredient",
        "ingredient_category",
        "setting",
    ] {
        tx.execute(&format!("DELETE FROM {table}"), []).map_err(|e| e.to_string())?;
    }

    let err = |what: &str, e: rusqlite::Error| format!("Couldn't restore {what}: {e}");
    for c in &b.categories {
        tx.execute(
            "INSERT INTO ingredient_category (id, name, sort_order) VALUES (?1, ?2, ?3)",
            params![c.id, c.name, c.sort_order],
        )
        .map_err(|e| err("categories", e))?;
    }
    for i in &b.ingredients {
        tx.execute(
            "INSERT INTO ingredient (id, name, category_id, unit, archived) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![i.id, i.name, i.category_id, i.unit, i.archived],
        )
        .map_err(|e| err("ingredients", e))?;
    }
    for t in &b.templates {
        tx.execute("INSERT INTO template (id, name) VALUES (?1, ?2)", params![t.id, t.name])
            .map_err(|e| err("templates", e))?;
    }
    for l in &b.template_lines {
        tx.execute(
            "INSERT INTO template_line
               (id, template_id, label, ingredient_id, category_id, quantity, unit_hint, position)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![l.id, l.template_id, l.label, l.ingredient_id, l.category_id, l.quantity, l.unit_hint, l.position],
        )
        .map_err(|e| err("template lines", e))?;
    }
    for i in &b.items {
        tx.execute(
            "INSERT INTO item (id, name, type, colour, notes, template_id, archived)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![i.id, i.name, i.item_type, i.colour, i.notes, i.template_id, i.archived],
        )
        .map_err(|e| err("items", e))?;
    }
    for l in &b.item_ingredients {
        tx.execute(
            "INSERT INTO item_ingredient (id, item_id, ingredient_id, quantity, position)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![l.id, l.item_id, l.ingredient_id, l.quantity, l.position],
        )
        .map_err(|e| err("item ingredients", e))?;
    }
    for p in &b.plan_entries {
        tx.execute(
            "INSERT INTO plan_entry (id, date, item_id, serves, note, position)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![p.id, p.date, p.item_id, p.serves, p.note, p.position],
        )
        .map_err(|e| err("the plan", e))?;
    }
    for s in &b.settings {
        settings::set(&tx, &s.key, &s.value)?;
    }

    // Foreign keys are checked per statement above, but be explicit that nothing dangles.
    let dangling: i64 = tx
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if dangling > 0 {
        return Err("The backup has entries that point at missing data".into());
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(ImportSummary {
        items: b.items.len(),
        ingredients: b.ingredients.len(),
        templates: b.templates.len(),
        plan_entries: b.plan_entries.len(),
    })
}

#[tauri::command]
pub fn backup_export(db: State<Db>, path: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let json = export_json(&conn)?;
    fs::write(&path, json).map_err(|e| format!("Couldn't write {path}: {e}"))
}

#[tauri::command]
pub fn backup_import(db: State<Db>, path: String) -> Result<ImportSummary, String> {
    let json = fs::read_to_string(&path).map_err(|e| format!("Couldn't read {path}: {e}"))?;
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    import_json(&conn, &json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{plan, templates};
    use crate::db::test_conn;

    /// Export with the volatile timestamp blanked, for comparing two databases.
    fn comparable(conn: &Connection) -> Backup {
        let mut b = collect(conn).unwrap();
        b.exported_at = String::new();
        b
    }

    fn item_id(conn: &Connection, name: &str) -> i64 {
        conn.query_row("SELECT id FROM item WHERE name = ?1", params![name], |r| r.get(0)).unwrap()
    }

    /// A seeded database with some plan history, notes and edited settings.
    fn lived_in() -> Connection {
        let conn = test_conn();
        let e = plan::add(&conn, item_id(&conn, "Steak"), "2026-10-06", Some(4)).unwrap();
        plan::update(&conn, e.id, None, Some("use rump")).unwrap();
        plan::add(&conn, item_id(&conn, "Chips"), "2026-10-06", None).unwrap();
        settings::set(&conn, "week_start", "0").unwrap();
        settings::set(&conn, "planner_view", "month").unwrap();
        conn.execute("UPDATE item SET archived = 1 WHERE name = 'Sausages'", []).unwrap();
        conn
    }

    #[test]
    fn export_then_import_into_a_fresh_database_is_identical() {
        let source = lived_in();
        let json = export_json(&source).unwrap();

        let target = test_conn(); // different data: just the seed
        let summary = import_json(&target, &json).unwrap();
        assert_eq!(summary.plan_entries, 2);
        assert_eq!(summary.items, 8);
        assert_eq!(comparable(&source), comparable(&target));
    }

    #[test]
    fn import_replaces_existing_data() {
        let source = lived_in();
        let json = export_json(&source).unwrap();

        let target = test_conn();
        plan::add(&target, item_id(&target, "Spag bol"), "2026-11-01", None).unwrap();
        conn_exec(&target, "INSERT INTO ingredient (name, category_id, unit) VALUES ('Leek', 3, 'each')");
        import_json(&target, &json).unwrap();

        let leeks: i64 = target.query_row("SELECT COUNT(*) FROM ingredient WHERE name = 'Leek'", [], |r| r.get(0)).unwrap();
        assert_eq!(leeks, 0, "data not in the backup is gone");
        assert!(plan::list(&target, "2026-11-01", "2026-11-01").unwrap().is_empty());
        assert_eq!(plan::list(&target, "2026-10-06", "2026-10-06").unwrap().len(), 2);
    }

    fn conn_exec(conn: &Connection, sql: &str) {
        conn.execute(sql, []).unwrap();
    }

    #[test]
    fn restore_keeps_notes_serves_archives_templates_and_settings() {
        let source = lived_in();
        // Make a template-linked item too.
        let tid: i64 = source.query_row("SELECT id FROM template WHERE name = 'Risotto'", [], |r| r.get(0)).unwrap();
        conn_exec(&source, &format!("UPDATE item SET template_id = {tid} WHERE name = 'Chow mein'"));
        let json = export_json(&source).unwrap();

        let target = test_conn();
        import_json(&target, &json).unwrap();
        let day = plan::list(&target, "2026-10-06", "2026-10-06").unwrap();
        assert_eq!((day[0].serves, day[0].note.as_deref()), (Some(4), Some("use rump")));
        let archived: bool =
            target.query_row("SELECT archived FROM item WHERE name = 'Sausages'", [], |r| r.get::<_, i64>(0).map(|v| v != 0)).unwrap();
        assert!(archived);
        let linked: Option<i64> =
            target.query_row("SELECT template_id FROM item WHERE name = 'Chow mein'", [], |r| r.get(0)).unwrap();
        assert_eq!(linked, Some(tid));
        assert_eq!(settings::get(&target, "week_start").unwrap().as_deref(), Some("0"));
        assert_eq!(templates::list(&target).unwrap().len(), 2);
    }

    #[test]
    fn a_bad_file_changes_nothing() {
        let conn = lived_in();
        let before = comparable(&conn);

        assert!(import_json(&conn, "not json").unwrap_err().starts_with("Not a valid backup file"));
        assert_eq!(comparable(&conn), before);

        let mut wrong_version: serde_json::Value = serde_json::from_str(&export_json(&conn).unwrap()).unwrap();
        wrong_version["version"] = 99.into();
        let err = import_json(&conn, &wrong_version.to_string()).unwrap_err();
        assert!(err.contains("Unsupported backup version 99"), "{err}");
        assert_eq!(comparable(&conn), before);
    }

    #[test]
    fn invalid_contents_roll_back_everything() {
        let conn = lived_in();
        let before = comparable(&conn);
        let good = export_json(&conn).unwrap();

        let tamper = |edit: &dyn Fn(&mut serde_json::Value)| {
            let mut v: serde_json::Value = serde_json::from_str(&good).unwrap();
            edit(&mut v);
            v.to_string()
        };
        // A plan entry pointing at an item that doesn't exist.
        let dangling = tamper(&|v| v["plan_entries"][0]["item_id"] = 99999.into());
        assert!(import_json(&conn, &dangling).is_err());
        assert_eq!(comparable(&conn), before, "failed import left the data alone");

        // An ingredient with a unit the app doesn't allow.
        let bad_unit = tamper(&|v| v["ingredients"][0]["unit"] = "kg".into());
        assert!(import_json(&conn, &bad_unit).is_err());
        assert_eq!(comparable(&conn), before);

        // A setting with an invalid value.
        let bad_setting = tamper(&|v| v["settings"][0]["value"] = "banana".into());
        assert!(import_json(&conn, &bad_setting).is_err());
        assert_eq!(comparable(&conn), before);
    }

    #[test]
    fn missing_sections_are_rejected_not_silently_empty() {
        let conn = lived_in();
        let before = comparable(&conn);
        let mut v: serde_json::Value = serde_json::from_str(&export_json(&conn).unwrap()).unwrap();
        v.as_object_mut().unwrap().remove("items");
        assert!(import_json(&conn, &v.to_string()).is_err());
        assert_eq!(comparable(&conn), before);
    }
}
