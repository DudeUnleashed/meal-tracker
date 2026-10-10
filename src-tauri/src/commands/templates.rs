use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::db::Db;

const UNITS: [&str; 3] = ["g", "ml", "each"];

#[derive(Serialize, Debug)]
pub struct TemplateSummary {
    pub id: i64,
    pub name: String,
    pub item_type: String,
    pub fixed_count: i64,
    pub slot_count: i64,
}

/// A fixed line has an ingredient; a slot (`ingredient_id` = None) is filled in per meal.
#[derive(Serialize, Debug)]
pub struct TemplateLine {
    pub id: i64,
    pub label: String,
    pub ingredient_id: Option<i64>,
    pub ingredient_name: Option<String>,
    pub ingredient_unit: Option<String>,
    /// Filters the ingredient picker for slots (e.g. Meat).
    pub category_id: Option<i64>,
    pub category_name: Option<String>,
    pub quantity: Option<f64>,
    /// Unit the default quantity is in, for slots.
    pub unit_hint: Option<String>,
    /// A slot you can tick several ingredients on in one night (steamed veg).
    pub multi: bool,
    /// Candidate ingredients for a slot; empty means "anything in the category".
    pub options: Vec<TemplateOption>,
}

#[derive(Serialize, Debug)]
pub struct TemplateOption {
    pub id: i64,
    pub ingredient_id: i64,
    pub ingredient_name: String,
    pub ingredient_unit: String,
    /// Default amount, in the ingredient's own unit.
    pub quantity: Option<f64>,
}

#[derive(Serialize, Debug)]
pub struct Template {
    pub id: i64,
    pub name: String,
    /// What the template's chip is: meal, side or sweet.
    pub item_type: String,
    pub colour: Option<String>,
    /// The library chip for this template (a linked item).
    pub item_id: Option<i64>,
    pub category_id: Option<i64>,
    pub lines: Vec<TemplateLine>,
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct OptionInput {
    pub ingredient_id: i64,
    pub quantity: Option<f64>,
}

#[derive(Deserialize, Default, Clone)]
pub struct LineInput {
    /// Existing line to keep (so nights that picked from it stay attached); omit for a new line.
    #[serde(default)]
    pub id: Option<i64>,
    pub label: String,
    pub ingredient_id: Option<i64>,
    pub category_id: Option<i64>,
    pub quantity: Option<f64>,
    pub unit_hint: Option<String>,
    #[serde(default)]
    pub multi: bool,
    #[serde(default)]
    pub options: Vec<OptionInput>,
}

#[derive(Deserialize)]
pub struct TemplateInput {
    pub name: String,
    #[serde(default = "meal")]
    pub item_type: String,
    #[serde(default)]
    pub colour: Option<String>,
    #[serde(default)]
    pub category_id: Option<i64>,
    pub lines: Vec<LineInput>,
}

fn meal() -> String {
    "meal".into()
}

const TYPES: [&str; 3] = ["meal", "side", "sweet"];

#[derive(Deserialize)]
pub struct SlotFill {
    pub line_id: i64,
    pub ingredient_id: i64,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct DraftLine {
    pub ingredient_id: i64,
    pub ingredient_name: String,
    pub unit: String,
    /// None when the template's default can't be trusted (unit mismatch) or was never set.
    pub quantity: Option<f64>,
    /// The template line this came from, e.g. "Protein".
    pub label: String,
}

/// What "start from template" hands the item editor: an unsaved, fully editable meal.
#[derive(Serialize, Debug, PartialEq)]
pub struct ItemDraft {
    pub name: String,
    pub template_id: i64,
    pub lines: Vec<DraftLine>,
}

/// Validates and normalises the input: trimmed name/labels, fixed lines drop slot-only fields.
fn clean(input: &TemplateInput) -> Result<(String, Vec<LineInput>), String> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err("Name is required".into());
    }
    if !TYPES.contains(&input.item_type.as_str()) {
        return Err(format!("Type must be one of: {}", TYPES.join(", ")));
    }
    let mut lines = Vec::new();
    for l in &input.lines {
        let label = l.label.trim().to_string();
        if label.is_empty() {
            return Err("Every line needs a label".into());
        }
        let positive = |q: f64| q.is_finite() && q > 0.0;
        match l.ingredient_id {
            Some(_) => {
                if !l.quantity.is_some_and(positive) {
                    return Err(format!("\"{label}\" needs a quantity greater than zero"));
                }
                lines.push(LineInput {
                    id: l.id,
                    label,
                    ingredient_id: l.ingredient_id,
                    category_id: None,
                    quantity: l.quantity,
                    unit_hint: None,
                    multi: false,
                    options: vec![],
                });
            }
            None => {
                if l.quantity.is_some_and(|q| !positive(q)) {
                    return Err(format!("\"{label}\" has an invalid default quantity"));
                }
                let hint = l.unit_hint.as_deref().map(str::trim).filter(|h| !h.is_empty());
                if hint.is_some_and(|h| !UNITS.contains(&h)) {
                    return Err(format!("Unit hint must be one of: {}", UNITS.join(", ")));
                }
                if l.options.iter().any(|o| o.quantity.is_some_and(|q| !positive(q))) {
                    return Err(format!("\"{label}\" has an option with an invalid amount"));
                }
                let mut seen = Vec::new();
                for o in &l.options {
                    if seen.contains(&o.ingredient_id) {
                        return Err(format!("\"{label}\" lists the same ingredient twice"));
                    }
                    seen.push(o.ingredient_id);
                }
                lines.push(LineInput {
                    id: l.id,
                    label,
                    ingredient_id: None,
                    category_id: l.category_id,
                    quantity: l.quantity,
                    unit_hint: hint.map(str::to_string),
                    multi: l.multi,
                    options: l.options.clone(),
                });
            }
        }
    }
    Ok((name, lines))
}

pub fn list(conn: &Connection) -> Result<Vec<TemplateSummary>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT t.id, t.name, t.item_type,
                    COUNT(tl.id) FILTER (WHERE tl.ingredient_id IS NOT NULL),
                    COUNT(tl.id) FILTER (WHERE tl.ingredient_id IS NULL)
             FROM template t LEFT JOIN template_line tl ON tl.template_id = t.id
             GROUP BY t.id ORDER BY t.name COLLATE NOCASE",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(TemplateSummary {
                id: r.get(0)?,
                name: r.get(1)?,
                item_type: r.get(2)?,
                fixed_count: r.get(3)?,
                slot_count: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

pub fn get(conn: &Connection, id: i64) -> Result<Template, String> {
    let (name, item_type, colour, item_id, category_id): (String, String, Option<String>, Option<i64>, Option<i64>) = conn
        .query_row("SELECT name, item_type, colour, item_id, category_id FROM template WHERE id = ?1", params![id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => "Template not found".to_string(),
            e => e.to_string(),
        })?;
    let mut stmt = conn
        .prepare(
            "SELECT tl.id, tl.label, tl.ingredient_id, g.name, g.unit,
                    tl.category_id, c.name, tl.quantity, tl.unit_hint, tl.multi
             FROM template_line tl
             LEFT JOIN ingredient g ON g.id = tl.ingredient_id
             LEFT JOIN ingredient_category c ON c.id = tl.category_id
             WHERE tl.template_id = ?1 ORDER BY tl.position, tl.id",
        )
        .map_err(|e| e.to_string())?;
    let lines = stmt
        .query_map(params![id], |r| {
            Ok(TemplateLine {
                id: r.get(0)?,
                label: r.get(1)?,
                ingredient_id: r.get(2)?,
                ingredient_name: r.get(3)?,
                ingredient_unit: r.get(4)?,
                category_id: r.get(5)?,
                category_name: r.get(6)?,
                quantity: r.get(7)?,
                unit_hint: r.get(8)?,
                multi: r.get::<_, i64>(9)? != 0,
                options: vec![],
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let mut lines = lines;
    let mut stmt = conn
        .prepare(
            "SELECT o.id, o.ingredient_id, g.name, g.unit, o.quantity
             FROM template_option o JOIN ingredient g ON g.id = o.ingredient_id
             WHERE o.line_id = ?1 ORDER BY o.position, o.id",
        )
        .map_err(|e| e.to_string())?;
    for line in &mut lines {
        line.options = stmt
            .query_map(params![line.id], |r| {
                Ok(TemplateOption {
                    id: r.get(0)?,
                    ingredient_id: r.get(1)?,
                    ingredient_name: r.get(2)?,
                    ingredient_unit: r.get(3)?,
                    quantity: r.get(4)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
    }
    Ok(Template { id, name, item_type, colour, item_id, category_id, lines })
}

/// Inserts or updates each line (keeping the id of lines that already exist, so nights that
/// picked from them stay attached), removes lines that are gone, and rewrites their options.
fn write_lines(conn: &Connection, template_id: i64, lines: &[LineInput]) -> Result<(), String> {
    let existing: Vec<i64> = {
        let mut stmt = conn
            .prepare("SELECT id FROM template_line WHERE template_id = ?1")
            .map_err(|e| e.to_string())?;
        let ids = stmt
            .query_map(params![template_id], |r| r.get(0))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<i64>, _>>()
            .map_err(|e| e.to_string())?;
        ids
    };
    for id in &existing {
        if !lines.iter().any(|l| l.id == Some(*id)) {
            conn.execute("DELETE FROM template_line WHERE id = ?1", params![id]).map_err(|e| e.to_string())?;
        }
    }
    for (pos, l) in lines.iter().enumerate() {
        let line_id = match l.id {
            Some(id) if existing.contains(&id) => {
                conn.execute(
                    "UPDATE template_line SET label = ?1, ingredient_id = ?2, category_id = ?3, quantity = ?4,
                            unit_hint = ?5, multi = ?6, position = ?7 WHERE id = ?8",
                    params![l.label, l.ingredient_id, l.category_id, l.quantity, l.unit_hint, l.multi, pos as i64, id],
                )
                .map_err(|e| e.to_string())?;
                id
            }
            Some(_) => return Err("That line isn't part of this template".into()),
            None => {
                conn.execute(
                    "INSERT INTO template_line
                       (template_id, label, ingredient_id, category_id, quantity, unit_hint, multi, position)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![template_id, l.label, l.ingredient_id, l.category_id, l.quantity, l.unit_hint, l.multi, pos as i64],
                )
                .map_err(|e| e.to_string())?;
                conn.last_insert_rowid()
            }
        };
        conn.execute("DELETE FROM template_option WHERE line_id = ?1", params![line_id])
            .map_err(|e| e.to_string())?;
        for (opos, o) in l.options.iter().enumerate() {
            conn.execute(
                "INSERT INTO template_option (line_id, ingredient_id, quantity, position) VALUES (?1, ?2, ?3, ?4)",
                params![line_id, o.ingredient_id, o.quantity, opos as i64],
            )
            .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Keeps the template's chip (a linked item) in step with the template: creates it if missing,
/// copies the name, type and colour, and mirrors the fixed lines as its ingredients. Which
/// ingredients a slot adds on a given night is stored on the plan entry instead.
pub fn sync_item(conn: &Connection, template_id: i64) -> Result<i64, String> {
    let (name, item_type, colour, item_id, sample, category_id): (String, String, Option<String>, Option<i64>, i64, Option<i64>) = conn
        .query_row(
            "SELECT name, item_type, colour, item_id, sample, category_id FROM template WHERE id = ?1",
            params![template_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .map_err(|e| e.to_string())?;
    let item_id = match item_id {
        Some(id) => {
            conn.execute(
                "UPDATE item SET name = ?1, type = ?2, colour = ?3, category_id = ?4 WHERE id = ?5",
                params![name, item_type, colour, category_id, id],
            )
            .map_err(|e| e.to_string())?;
            id
        }
        None => {
            conn.execute(
                "INSERT INTO item (name, type, colour, sample, category_id) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![name, item_type, colour, sample, category_id],
            )
            .map_err(|e| e.to_string())?;
            let id = conn.last_insert_rowid();
            conn.execute("UPDATE template SET item_id = ?1 WHERE id = ?2", params![id, template_id])
                .map_err(|e| e.to_string())?;
            id
        }
    };
    conn.execute("DELETE FROM item_ingredient WHERE item_id = ?1", params![item_id]).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO item_ingredient (item_id, ingredient_id, quantity, position)
         SELECT ?1, ingredient_id, quantity, position FROM template_line
         WHERE template_id = ?2 AND ingredient_id IS NOT NULL AND quantity IS NOT NULL
         ORDER BY position, id",
        params![item_id, template_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(item_id)
}

/// Gives every template a chip. Runs after migrations and after a backup is restored, which is
/// how templates that predate live templates get theirs.
pub fn ensure_items(conn: &Connection) -> Result<(), String> {
    let ids: Vec<i64> = {
        let mut stmt = conn
            .prepare("SELECT id FROM template WHERE item_id IS NULL")
            .map_err(|e| e.to_string())?;
        let ids = stmt
            .query_map([], |r| r.get(0))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<i64>, _>>()
            .map_err(|e| e.to_string())?;
        ids
    };
    for id in ids {
        sync_item(conn, id)?;
    }
    Ok(())
}

pub fn create(conn: &mut Connection, input: &TemplateInput) -> Result<Template, String> {
    let (name, lines) = clean(input)?;
    let colour = input.colour.as_deref().map(str::trim).filter(|c| !c.is_empty());
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO template (name, item_type, colour, category_id) VALUES (?1, ?2, ?3, ?4)",
        params![name, input.item_type, colour, input.category_id],
    )
    .map_err(|e| e.to_string())?;
    let id = tx.last_insert_rowid();
    write_lines(&tx, id, &lines)?;
    sync_item(&tx, id)?;
    tx.commit().map_err(|e| e.to_string())?;
    get(conn, id)
}

/// Replaces the template's name, type, colour and lines, and updates its chip. Nights already
/// planned keep what they picked; items made *from* the template earlier are copies and unaffected.
pub fn update(conn: &mut Connection, id: i64, input: &TemplateInput) -> Result<Template, String> {
    let (name, lines) = clean(input)?;
    let colour = input.colour.as_deref().map(str::trim).filter(|c| !c.is_empty());
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let changed = tx
        .execute(
            "UPDATE template SET name = ?1, item_type = ?2, colour = ?3, category_id = ?4 WHERE id = ?5",
            params![name, input.item_type, colour, input.category_id, id],
        )
        .map_err(|e| e.to_string())?;
    if changed == 0 {
        return Err("Template not found".into());
    }
    write_lines(&tx, id, &lines)?;
    sync_item(&tx, id)?;
    tx.commit().map_err(|e| e.to_string())?;
    get(conn, id)
}

/// Deletes a template. Items made from it keep their ingredients and just lose the link. Its
/// chip is removed too, or archived instead if it has been planned (so past weeks stay intact).
pub fn delete(conn: &mut Connection, id: i64) -> Result<(), String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let item_id: Option<i64> = tx
        .query_row("SELECT item_id FROM template WHERE id = ?1", params![id], |r| r.get(0))
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => "Template not found".to_string(),
            e => e.to_string(),
        })?;
    tx.execute("UPDATE item SET template_id = NULL WHERE template_id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM template WHERE id = ?1", params![id]).map_err(|e| e.to_string())?;
    if let Some(item_id) = item_id {
        let planned: i64 = tx
            .query_row("SELECT COUNT(*) FROM plan_entry WHERE item_id = ?1", params![item_id], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        if planned > 0 {
            tx.execute("UPDATE item SET archived = 1 WHERE id = ?1", params![item_id]).map_err(|e| e.to_string())?;
        } else {
            tx.execute("DELETE FROM item WHERE id = ?1", params![item_id]).map_err(|e| e.to_string())?;
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// Builds an item draft from a template: fixed lines are copied, filled slots become lines,
/// unfilled slots are skipped. The default quantity is only trusted when the chosen
/// ingredient's unit matches the slot's unit hint (200 g shouldn't become 200 carrots).
pub fn instantiate(conn: &Connection, template_id: i64, fills: &[SlotFill]) -> Result<ItemDraft, String> {
    let template = get(conn, template_id)?;

    for f in fills {
        match template.lines.iter().find(|l| l.id == f.line_id) {
            None => return Err("That slot isn't part of this template".into()),
            Some(l) if l.ingredient_id.is_some() => {
                return Err(format!("\"{}\" is a fixed line, not a slot", l.label));
            }
            Some(_) => {}
        }
    }

    let mut lines = Vec::new();
    let mut filled_names = Vec::new();
    for l in &template.lines {
        if let (Some(id), Some(name), Some(unit)) = (l.ingredient_id, &l.ingredient_name, &l.ingredient_unit) {
            lines.push(DraftLine {
                ingredient_id: id,
                ingredient_name: name.clone(),
                unit: unit.clone(),
                quantity: l.quantity,
                label: l.label.clone(),
            });
            continue;
        }
        let Some(fill) = fills.iter().find(|f| f.line_id == l.id) else { continue };
        let (name, unit): (String, String) = conn
            .query_row(
                "SELECT name, unit FROM ingredient WHERE id = ?1",
                params![fill.ingredient_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|_| "Ingredient not found".to_string())?;
        let quantity = match &l.unit_hint {
            Some(hint) if *hint != unit => None,
            _ => l.quantity,
        };
        filled_names.push(name.to_lowercase());
        lines.push(DraftLine {
            ingredient_id: fill.ingredient_id,
            ingredient_name: name,
            unit,
            quantity,
            label: l.label.clone(),
        });
    }

    let name = if filled_names.is_empty() {
        template.name.clone()
    } else {
        format!("{} – {}", template.name, filled_names.join(" & "))
    };
    Ok(ItemDraft { name, template_id, lines })
}

#[tauri::command]
pub fn templates_list(db: State<Db>) -> Result<Vec<TemplateSummary>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    list(&conn)
}

#[tauri::command]
pub fn templates_get(db: State<Db>, id: i64) -> Result<Template, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    get(&conn, id)
}

#[tauri::command]
pub fn templates_create(db: State<Db>, input: TemplateInput) -> Result<Template, String> {
    let mut conn = db.0.lock().map_err(|e| e.to_string())?;
    create(&mut conn, &input)
}

#[tauri::command]
pub fn templates_update(db: State<Db>, id: i64, input: TemplateInput) -> Result<Template, String> {
    let mut conn = db.0.lock().map_err(|e| e.to_string())?;
    update(&mut conn, id, &input)
}

#[tauri::command]
pub fn templates_delete(db: State<Db>, id: i64) -> Result<(), String> {
    let mut conn = db.0.lock().map_err(|e| e.to_string())?;
    delete(&mut conn, id)
}

#[tauri::command]
pub fn templates_instantiate(
    db: State<Db>,
    template_id: i64,
    slot_fills: Vec<SlotFill>,
) -> Result<ItemDraft, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    instantiate(&conn, template_id, &slot_fills)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_conn;

    fn ing(conn: &Connection, name: &str) -> i64 {
        conn.query_row("SELECT id FROM ingredient WHERE name = ?1", params![name], |r| r.get(0)).unwrap()
    }

    fn template_id(conn: &Connection, name: &str) -> i64 {
        conn.query_row("SELECT id FROM template WHERE name = ?1", params![name], |r| r.get(0)).unwrap()
    }

    fn slot(t: &Template, label: &str) -> i64 {
        t.lines.iter().find(|l| l.label == label).unwrap().id
    }

    fn fixed(label: &str, ingredient_id: i64, quantity: f64) -> LineInput {
        LineInput { label: label.into(), ingredient_id: Some(ingredient_id), quantity: Some(quantity), ..Default::default() }
    }

    fn slot_input(label: &str, category_id: Option<i64>, quantity: Option<f64>, hint: Option<&str>) -> LineInput {
        LineInput { label: label.into(), category_id, quantity, unit_hint: hint.map(Into::into), ..Default::default() }
    }

    #[test]
    fn list_counts_fixed_and_slot_lines() {
        let conn = test_conn();
        let all = list(&conn).unwrap();
        let risotto = all.iter().find(|t| t.name == "Risotto").unwrap();
        assert_eq!((risotto.fixed_count, risotto.slot_count), (3, 2));
        let stir = all.iter().find(|t| t.name == "Stir fry").unwrap();
        assert_eq!((stir.fixed_count, stir.slot_count), (1, 3));
    }

    #[test]
    fn get_resolves_ingredient_and_category_names() {
        let conn = test_conn();
        let t = get(&conn, template_id(&conn, "Risotto")).unwrap();
        let rice = &t.lines[0];
        assert_eq!((rice.label.as_str(), rice.ingredient_name.as_deref(), rice.quantity), ("Arborio rice", Some("Arborio rice"), Some(300.0)));
        let protein = t.lines.iter().find(|l| l.label == "Protein").unwrap();
        assert_eq!(protein.ingredient_id, None);
        assert_eq!(protein.category_name.as_deref(), Some("Meat"));
        assert_eq!(protein.unit_hint.as_deref(), Some("g"));
    }

    #[test]
    fn create_update_delete_roundtrip() {
        let mut conn = test_conn();
        let meat = 1;
        let input = TemplateInput {
            item_type: "meal".into(),
            colour: None,
            category_id: None,
            name: "  Pasta bake ".into(),
            lines: vec![fixed("Pasta", ing(&conn, "Spaghetti"), 400.0), slot_input("Protein", Some(meat), Some(300.0), Some("g"))],
        };
        let t = create(&mut conn, &input).unwrap();
        assert_eq!(t.name, "Pasta bake");
        assert_eq!(t.lines.len(), 2);

        let edited = TemplateInput {
            item_type: "meal".into(),
            colour: None,
            category_id: None,
            name: "Pasta bake v2".into(),
            lines: vec![slot_input("Veg", None, None, None)],
        };
        let t2 = update(&mut conn, t.id, &edited).unwrap();
        assert_eq!((t2.name.as_str(), t2.lines.len()), ("Pasta bake v2", 1));
        assert_eq!(t2.lines[0].quantity, None);

        delete(&mut conn, t.id).unwrap();
        assert_eq!(get(&conn, t.id).unwrap_err(), "Template not found");
        assert_eq!(delete(&mut conn, t.id).unwrap_err(), "Template not found");
    }

    #[test]
    fn deleting_a_template_keeps_items_made_from_it() {
        let mut conn = test_conn();
        let tid = template_id(&conn, "Risotto");
        conn.execute("INSERT INTO item (name, type, template_id) VALUES ('My risotto', 'meal', ?1)", params![tid]).unwrap();
        delete(&mut conn, tid).unwrap();
        let linked: Option<i64> =
            conn.query_row("SELECT template_id FROM item WHERE name = 'My risotto'", [], |r| r.get(0)).unwrap();
        assert_eq!(linked, None);
    }

    #[test]
    fn rejects_bad_templates() {
        let mut conn = test_conn();
        let rice = ing(&conn, "Arborio rice");
        let bad = |lines: Vec<LineInput>| TemplateInput { name: "T".into(), item_type: "meal".into(), colour: None, category_id: None, lines };
        assert!(create(&mut conn, &TemplateInput { name: " ".into(), item_type: "meal".into(), colour: None, category_id: None, lines: vec![] }).is_err());
        assert!(create(&mut conn, &TemplateInput { name: "T".into(), item_type: "meat".into(), colour: None, category_id: None, lines: vec![] }).is_err(), "templates can't be meat");
        assert!(create(&mut conn, &bad(vec![fixed(" ", rice, 1.0)])).is_err(), "blank label");
        assert!(create(&mut conn, &bad(vec![fixed("Rice", rice, 0.0)])).is_err(), "fixed line needs a quantity");
        assert!(create(&mut conn, &bad(vec![slot_input("P", None, Some(-1.0), None)])).is_err());
        assert!(create(&mut conn, &bad(vec![slot_input("P", None, None, Some("kg"))])).is_err());
        assert!(update(&mut conn, 9999, &bad(vec![])).is_err());
        // Slots without a default quantity or unit hint are fine.
        assert!(create(&mut conn, &bad(vec![slot_input("P", None, None, None)])).is_ok());
    }

    #[test]
    fn instantiate_copies_fixed_lines_and_filled_slots() {
        let conn = test_conn();
        let tid = template_id(&conn, "Risotto");
        let t = get(&conn, tid).unwrap();
        let draft = instantiate(
            &conn,
            tid,
            &[
                SlotFill { line_id: slot(&t, "Protein"), ingredient_id: ing(&conn, "Chicken thigh") },
                SlotFill { line_id: slot(&t, "Veg 1"), ingredient_id: ing(&conn, "Mushroom") },
            ],
        )
        .unwrap();
        assert_eq!(draft.name, "Risotto – chicken thigh & mushroom");
        assert_eq!(draft.template_id, tid);
        let got: Vec<(&str, Option<f64>)> = draft.lines.iter().map(|l| (l.ingredient_name.as_str(), l.quantity)).collect();
        assert_eq!(
            got,
            [
                ("Arborio rice", Some(300.0)),
                ("Chicken stock", Some(1000.0)),
                ("Parmesan", Some(50.0)),
                ("Chicken thigh", Some(500.0)),
                ("Mushroom", Some(200.0)),
            ]
        );
        assert_eq!(draft.lines[3].label, "Protein");
    }

    #[test]
    fn unfilled_slots_are_skipped_and_name_falls_back() {
        let conn = test_conn();
        let tid = template_id(&conn, "Stir fry");
        let draft = instantiate(&conn, tid, &[]).unwrap();
        assert_eq!(draft.name, "Stir fry");
        assert_eq!(draft.lines.len(), 1, "only the fixed soy sauce line");

        let t = get(&conn, tid).unwrap();
        let one = instantiate(&conn, tid, &[SlotFill { line_id: slot(&t, "Protein"), ingredient_id: ing(&conn, "Prawns") }]).unwrap();
        assert_eq!(one.name, "Stir fry – prawns");
        assert_eq!(one.lines.len(), 2);
    }

    #[test]
    fn default_quantity_is_dropped_when_the_unit_differs_from_the_hint() {
        let conn = test_conn();
        let tid = template_id(&conn, "Risotto");
        let t = get(&conn, tid).unwrap();
        // Veg 1 defaults to 200 g, but carrots are counted "each".
        let draft = instantiate(&conn, tid, &[SlotFill { line_id: slot(&t, "Veg 1"), ingredient_id: ing(&conn, "Carrot") }]).unwrap();
        let carrot = draft.lines.iter().find(|l| l.ingredient_name == "Carrot").unwrap();
        assert_eq!((carrot.unit.as_str(), carrot.quantity), ("each", None));

        // Stir fry's Veg slots are hinted "each" and keep their default for a matching unit.
        let sid = template_id(&conn, "Stir fry");
        let st = get(&conn, sid).unwrap();
        let d = instantiate(&conn, sid, &[SlotFill { line_id: slot(&st, "Veg 1"), ingredient_id: ing(&conn, "Carrot") }]).unwrap();
        assert_eq!(d.lines.iter().find(|l| l.ingredient_name == "Carrot").unwrap().quantity, Some(2.0));
    }

    #[test]
    fn instantiate_rejects_bad_fills() {
        let conn = test_conn();
        let tid = template_id(&conn, "Risotto");
        let t = get(&conn, tid).unwrap();
        let carrot = ing(&conn, "Carrot");
        assert!(instantiate(&conn, tid, &[SlotFill { line_id: slot(&t, "Parmesan"), ingredient_id: carrot }]).is_err(), "fixed line");
        assert!(instantiate(&conn, tid, &[SlotFill { line_id: 99999, ingredient_id: carrot }]).is_err(), "unknown line");
        assert!(instantiate(&conn, tid, &[SlotFill { line_id: slot(&t, "Protein"), ingredient_id: 99999 }]).is_err(), "unknown ingredient");
        assert_eq!(instantiate(&conn, 9999, &[]).unwrap_err(), "Template not found");
    }
}
