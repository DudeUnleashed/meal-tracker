use std::fs;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tauri::State;

use super::settings;
use crate::db::Db;

/// Written into new backups. Version 1 files (before slots, live templates and the takeaway
/// token) still import: the missing fields default, and `import_json` fills in the rest.
const VERSION: u32 = 2;
const OLDEST_READABLE: u32 = 1;

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
    #[serde(default)]
    sample: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct TemplateRow {
    id: i64,
    name: String,
    #[serde(default)]
    sample: bool,
    #[serde(default = "meal")]
    item_type: String,
    #[serde(default)]
    colour: Option<String>,
    /// The template's chip. Absent in version 1 backups; restored by `ensure_items`.
    #[serde(default)]
    item_id: Option<i64>,
}

fn meal() -> String {
    "meal".into()
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct TemplateOptionRow {
    id: i64,
    line_id: i64,
    ingredient_id: i64,
    quantity: Option<f64>,
    position: i64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct PlanPickRow {
    id: i64,
    entry_id: i64,
    line_id: Option<i64>,
    ingredient_id: i64,
    quantity: f64,
    position: i64,
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
    #[serde(default)]
    multi: bool,
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
    #[serde(default)]
    sample: bool,
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
    /// Older backups predate the size multiplier.
    #[serde(default = "one")]
    scale: f64,
    /// Older backups only had dinners.
    #[serde(default = "dinner")]
    slot: String,
    #[serde(default)]
    label: Option<String>,
}

fn dinner() -> String {
    "dinner".into()
}

fn one() -> f64 {
    1.0
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct DayInfoRow {
    date: String,
    note: Option<String>,
    leftovers: bool,
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
    #[serde(default)]
    template_options: Vec<TemplateOptionRow>,
    items: Vec<ItemRow>,
    item_ingredients: Vec<ItemIngredientRow>,
    plan_entries: Vec<PlanEntryRow>,
    #[serde(default)]
    plan_picks: Vec<PlanPickRow>,
    /// Absent in backups made before day notes existed.
    #[serde(default)]
    day_info: Vec<DayInfoRow>,
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
        ingredients: rows(conn, "SELECT id, name, category_id, unit, archived, sample FROM ingredient ORDER BY id", |r| {
            Ok(IngredientRow {
                id: r.get(0)?,
                name: r.get(1)?,
                category_id: r.get(2)?,
                unit: r.get(3)?,
                archived: r.get::<_, i64>(4)? != 0,
                sample: r.get::<_, i64>(5)? != 0,
            })
        })?,
        templates: rows(conn, "SELECT id, name, sample, item_type, colour, item_id FROM template ORDER BY id", |r| {
            Ok(TemplateRow {
                id: r.get(0)?,
                name: r.get(1)?,
                sample: r.get::<_, i64>(2)? != 0,
                item_type: r.get(3)?,
                colour: r.get(4)?,
                item_id: r.get(5)?,
            })
        })?,
        template_options: rows(
            conn,
            "SELECT id, line_id, ingredient_id, quantity, position FROM template_option ORDER BY id",
            |r| {
                Ok(TemplateOptionRow {
                    id: r.get(0)?,
                    line_id: r.get(1)?,
                    ingredient_id: r.get(2)?,
                    quantity: r.get(3)?,
                    position: r.get(4)?,
                })
            },
        )?,
        plan_picks: rows(
            conn,
            "SELECT id, entry_id, line_id, ingredient_id, quantity, position FROM plan_entry_pick ORDER BY id",
            |r| {
                Ok(PlanPickRow {
                    id: r.get(0)?,
                    entry_id: r.get(1)?,
                    line_id: r.get(2)?,
                    ingredient_id: r.get(3)?,
                    quantity: r.get(4)?,
                    position: r.get(5)?,
                })
            },
        )?,
        template_lines: rows(
            conn,
            "SELECT id, template_id, label, ingredient_id, category_id, quantity, unit_hint, position, multi
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
                    multi: r.get::<_, i64>(8)? != 0,
                })
            },
        )?,
        items: rows(
            conn,
            "SELECT id, name, type, colour, notes, template_id, archived, sample FROM item ORDER BY id",
            |r| {
                Ok(ItemRow {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    item_type: r.get(2)?,
                    colour: r.get(3)?,
                    notes: r.get(4)?,
                    template_id: r.get(5)?,
                    archived: r.get::<_, i64>(6)? != 0,
                    sample: r.get::<_, i64>(7)? != 0,
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
            "SELECT id, date, item_id, serves, note, position, scale, slot, label FROM plan_entry ORDER BY id",
            |r| {
                Ok(PlanEntryRow {
                    id: r.get(0)?,
                    date: r.get(1)?,
                    item_id: r.get(2)?,
                    serves: r.get(3)?,
                    note: r.get(4)?,
                    position: r.get(5)?,
                    scale: r.get(6)?,
                    slot: r.get(7)?,
                    label: r.get(8)?,
                })
            },
        )?,
        day_info: rows(conn, "SELECT date, note, leftovers FROM day_info ORDER BY date", |r| {
            Ok(DayInfoRow { date: r.get(0)?, note: r.get(1)?, leftovers: r.get::<_, i64>(2)? != 0 })
        })?,
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
    if !(OLDEST_READABLE..=VERSION).contains(&b.version) {
        return Err(format!(
            "Unsupported backup version {} (this app reads versions {OLDEST_READABLE}-{VERSION})",
            b.version
        ));
    }

    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    // Children before parents so foreign keys hold while clearing.
    // Templates and items point at each other (chip / "started from"); cut those links first.
    tx.execute("UPDATE template SET item_id = NULL", []).map_err(|e| e.to_string())?;
    tx.execute("UPDATE item SET template_id = NULL", []).map_err(|e| e.to_string())?;
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
            "INSERT INTO ingredient (id, name, category_id, unit, archived, sample)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![i.id, i.name, i.category_id, i.unit, i.archived, i.sample],
        )
        .map_err(|e| err("ingredients", e))?;
    }
    for t in &b.templates {
        tx.execute(
            "INSERT INTO template (id, name, sample, item_type, colour) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![t.id, t.name, t.sample, t.item_type, t.colour],
        )
        .map_err(|e| err("templates", e))?;
    }
    for l in &b.template_lines {
        tx.execute(
            "INSERT INTO template_line
               (id, template_id, label, ingredient_id, category_id, quantity, unit_hint, position, multi)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![l.id, l.template_id, l.label, l.ingredient_id, l.category_id, l.quantity, l.unit_hint, l.position, l.multi],
        )
        .map_err(|e| err("template lines", e))?;
    }
    for o in &b.template_options {
        tx.execute(
            "INSERT INTO template_option (id, line_id, ingredient_id, quantity, position)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![o.id, o.line_id, o.ingredient_id, o.quantity, o.position],
        )
        .map_err(|e| err("template options", e))?;
    }
    for i in &b.items {
        tx.execute(
            "INSERT INTO item (id, name, type, colour, notes, template_id, archived, sample)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![i.id, i.name, i.item_type, i.colour, i.notes, i.template_id, i.archived, i.sample],
        )
        .map_err(|e| err("items", e))?;
    }
    for t in &b.templates {
        if let Some(item_id) = t.item_id {
            tx.execute("UPDATE template SET item_id = ?1 WHERE id = ?2", params![item_id, t.id])
                .map_err(|e| err("template chips", e))?;
        }
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
            "INSERT INTO plan_entry (id, date, item_id, serves, note, position, scale, slot, label)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![p.id, p.date, p.item_id, p.serves, p.note, p.position, p.scale, p.slot, p.label],
        )
        .map_err(|e| err("the plan", e))?;
    }
    for k in &b.plan_picks {
        tx.execute(
            "INSERT INTO plan_entry_pick (id, entry_id, line_id, ingredient_id, quantity, position)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![k.id, k.entry_id, k.line_id, k.ingredient_id, k.quantity, k.position],
        )
        .map_err(|e| err("what was picked each night", e))?;
    }
    for d in &b.day_info {
        tx.execute(
            "INSERT INTO day_info (date, note, leftovers) VALUES (?1, ?2, ?3)",
            params![d.date, d.note, d.leftovers],
        )
        .map_err(|e| err("day notes", e))?;
    }
    for s in &b.settings {
        settings::set(&tx, &s.key, &s.value)?;
    }

    // Backups from before live templates have no chips, and none of the newer shopping
    // categories; bring those up to date so a restored library behaves like a current one.
    super::categories::ensure_defaults(&tx)?;
    super::templates::ensure_items(&tx)?;

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

/// Writes a timestamped backup into `dir` (created if needed) and returns its path. Used as a
/// safety net before destructive actions such as wiping data.
pub fn auto_backup(conn: &Connection, dir: &std::path::Path, reason: &str) -> Result<std::path::PathBuf, String> {
    fs::create_dir_all(dir).map_err(|e| format!("Couldn't create {}: {e}", dir.display()))?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let path = dir.join(format!("{reason}-{stamp}.json"));
    fs::write(&path, export_json(conn)?).map_err(|e| format!("Couldn't write {}: {e}", path.display()))?;
    Ok(path)
}

#[tauri::command]
pub fn backup_export(db: State<Db>, path: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let json = export_json(&conn)?;
    fs::write(&path, json).map_err(|e| format!("Couldn't write {path}: {e}"))
}

#[tauri::command]
pub fn backup_import(
    db: State<Db>,
    undo: State<super::undo::UndoStack>,
    path: String,
) -> Result<ImportSummary, String> {
    let json = fs::read_to_string(&path).map_err(|e| format!("Couldn't read {path}: {e}"))?;
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let summary = import_json(&conn, &json)?;
    undo.clear(); // old snapshots refer to data that no longer exists
    Ok(summary)
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
        assert_eq!(summary.items, 10);
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
    fn day_info_scale_and_sample_flags_survive_a_restore() {
        let source = lived_in();
        let steak = plan::list(&source, "2026-10-06", "2026-10-06").unwrap();
        let chips = steak.iter().find(|e| e.item_name == "Chips").unwrap().id;
        plan::update_with(&source, chips, None, None, Some(1.5)).unwrap();
        crate::commands::dayinfo::set(&source, "2026-10-07", Some("Alex working late"), Some(true)).unwrap();
        source.execute("UPDATE item SET sample = 0 WHERE name = 'Chips'", []).unwrap();

        let target = test_conn();
        import_json(&target, &export_json(&source).unwrap()).unwrap();
        let day = crate::commands::dayinfo::list(&target, "2026-10-07", "2026-10-07").unwrap();
        assert_eq!((day[0].note.as_deref(), day[0].leftovers), (Some("Alex working late"), true));
        let e = plan::list(&target, "2026-10-06", "2026-10-06").unwrap().into_iter().find(|e| e.item_name == "Chips").unwrap();
        assert_eq!(e.scale, 1.5);
        let sample: i64 = target.query_row("SELECT sample FROM item WHERE name = 'Chips'", [], |r| r.get(0)).unwrap();
        assert_eq!(sample, 0);
        assert_eq!(comparable(&source), comparable(&target));
    }

    #[test]
    fn backups_from_before_these_features_still_import() {
        let source = lived_in();
        let mut v: serde_json::Value = serde_json::from_str(&export_json(&source).unwrap()).unwrap();
        // Strip what older versions never wrote.
        v.as_object_mut().unwrap().remove("day_info");
        for e in v["plan_entries"].as_array_mut().unwrap() {
            e.as_object_mut().unwrap().remove("scale");
        }
        for table in ["items", "ingredients", "templates"] {
            for row in v[table].as_array_mut().unwrap() {
                row.as_object_mut().unwrap().remove("sample");
            }
        }
        let target = test_conn();
        import_json(&target, &v.to_string()).unwrap();
        let e = plan::list(&target, "2026-10-06", "2026-10-06").unwrap();
        assert_eq!(e[0].scale, 1.0);
        let samples: i64 = target.query_row("SELECT COUNT(*) FROM item WHERE sample = 1", [], |r| r.get(0)).unwrap();
        assert_eq!(samples, 0, "old backups have no sample flags");
    }

    #[test]
    fn auto_backup_writes_a_restorable_file() {
        let source = lived_in();
        let dir = std::env::temp_dir().join(format!("meal-planner-test-{}", std::process::id()));
        let path = auto_backup(&source, &dir, "before-test").unwrap();
        assert!(path.file_name().unwrap().to_string_lossy().starts_with("before-test-"));
        let target = test_conn();
        import_json(&target, &fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(comparable(&source), comparable(&target));
        let _ = fs::remove_dir_all(&dir);
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
