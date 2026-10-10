use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tauri::State;

use super::ingredients::friendly;
use crate::db::Db;

// "blank" (the takeaway token) is a system item and is never created or edited by hand.
const TYPES: [&str; 4] = ["meal", "meat", "side", "sweet"];

#[derive(Serialize, Debug)]
pub struct ItemSummary {
    pub id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub item_type: String,
    pub colour: Option<String>,
    pub archived: bool,
    pub ingredient_count: i64,
    /// Set for a template's chip: it is edited in Library → Templates.
    pub live_template_id: Option<i64>,
    /// The dish category it is filed under (Beef, Soups...), if any.
    pub category_id: Option<i64>,
    pub category_name: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct ItemLine {
    pub ingredient_id: i64,
    pub ingredient_name: String,
    pub unit: String,
    /// Per serve for meat items; the fixed amount otherwise.
    pub quantity: f64,
}

#[derive(Serialize, Debug)]
pub struct Item {
    pub id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub item_type: String,
    pub colour: Option<String>,
    pub notes: Option<String>,
    pub template_id: Option<i64>,
    pub archived: bool,
    /// Set for a template's chip: it is edited in Library → Templates.
    pub live_template_id: Option<i64>,
    pub category_id: Option<i64>,
    pub lines: Vec<ItemLine>,
}

#[derive(Deserialize)]
pub struct LineInput {
    pub ingredient_id: i64,
    pub quantity: f64,
}

#[derive(Deserialize)]
pub struct ItemInput {
    pub name: String,
    #[serde(rename = "type")]
    pub item_type: String,
    pub colour: Option<String>,
    pub notes: Option<String>,
    pub template_id: Option<i64>,
    #[serde(default)]
    pub category_id: Option<i64>,
    pub lines: Vec<LineInput>,
}

fn validate(input: &ItemInput) -> Result<String, String> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err("Name is required".into());
    }
    if !TYPES.contains(&input.item_type.as_str()) {
        return Err(format!("Type must be one of: {}", TYPES.join(", ")));
    }
    if input.lines.iter().any(|l| !(l.quantity.is_finite() && l.quantity > 0.0)) {
        return Err("Every ingredient quantity must be greater than zero".into());
    }
    Ok(name)
}

/// Empty / whitespace-only optional text is stored as NULL.
fn blank_to_none(s: &Option<String>) -> Option<String> {
    s.as_ref().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

pub fn list(
    conn: &Connection,
    item_type: Option<&str>,
    search: Option<&str>,
    include_archived: bool,
) -> Result<Vec<ItemSummary>, String> {
    let search = search.map(str::trim).filter(|s| !s.is_empty());
    let mut stmt = conn
        .prepare(
            "SELECT i.id, i.name, i.type, i.colour, i.archived,
                    (SELECT COUNT(*) FROM item_ingredient ii WHERE ii.item_id = i.id),
                    (SELECT t.id FROM template t WHERE t.item_id = i.id),
                    i.category_id, c.name
             FROM item i
             LEFT JOIN item_category c ON c.id = i.category_id
             WHERE i.type != 'blank'
               AND (?1 IS NULL OR i.type = ?1)
               AND (?2 IS NULL OR i.name LIKE '%' || ?2 || '%')
               AND (?3 OR i.archived = 0)
             ORDER BY i.name COLLATE NOCASE",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![item_type, search, include_archived], |r| {
            Ok(ItemSummary {
                id: r.get(0)?,
                name: r.get(1)?,
                item_type: r.get(2)?,
                colour: r.get(3)?,
                archived: r.get::<_, i64>(4)? != 0,
                ingredient_count: r.get(5)?,
                live_template_id: r.get(6)?,
                category_id: r.get(7)?,
                category_name: r.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(rows)
}

pub fn get(conn: &Connection, id: i64) -> Result<Item, String> {
    let mut item = conn
        .query_row(
            "SELECT id, name, type, colour, notes, template_id, archived,
                    (SELECT t.id FROM template t WHERE t.item_id = item.id), category_id
             FROM item WHERE id = ?1",
            params![id],
            |r| {
                Ok(Item {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    item_type: r.get(2)?,
                    colour: r.get(3)?,
                    notes: r.get(4)?,
                    template_id: r.get(5)?,
                    archived: r.get::<_, i64>(6)? != 0,
                    live_template_id: r.get(7)?,
                    category_id: r.get(8)?,
                    lines: vec![],
                })
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => "Item not found".to_string(),
            e => e.to_string(),
        })?;

    let mut stmt = conn
        .prepare(
            "SELECT ii.ingredient_id, g.name, g.unit, ii.quantity
             FROM item_ingredient ii
             JOIN ingredient g ON g.id = ii.ingredient_id
             WHERE ii.item_id = ?1
             ORDER BY ii.position, ii.id",
        )
        .map_err(|e| e.to_string())?;
    item.lines = stmt
        .query_map(params![id], |r| {
            Ok(ItemLine {
                ingredient_id: r.get(0)?,
                ingredient_name: r.get(1)?,
                unit: r.get(2)?,
                quantity: r.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(item)
}

fn write_lines(conn: &Connection, item_id: i64, lines: &[LineInput]) -> Result<(), String> {
    for (pos, l) in lines.iter().enumerate() {
        conn.execute(
            "INSERT INTO item_ingredient (item_id, ingredient_id, quantity, position)
             VALUES (?1, ?2, ?3, ?4)",
            params![item_id, l.ingredient_id, l.quantity, pos as i64],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn create(conn: &mut Connection, input: &ItemInput) -> Result<Item, String> {
    let name = validate(input)?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO item (name, type, colour, notes, template_id, category_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            name,
            input.item_type,
            blank_to_none(&input.colour),
            blank_to_none(&input.notes),
            input.template_id,
            input.category_id
        ],
    )
    .map_err(|e| e.to_string())?;
    let id = tx.last_insert_rowid();
    write_lines(&tx, id, &input.lines)?;
    tx.commit().map_err(|e| e.to_string())?;
    get(conn, id)
}

/// Replaces the item's fields and its whole ingredient list.
pub fn update(conn: &mut Connection, id: i64, input: &ItemInput) -> Result<Item, String> {
    let name = validate(input)?;
    let is_chip: bool = conn
        .query_row("SELECT EXISTS (SELECT 1 FROM template WHERE item_id = ?1)", params![id], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if is_chip {
        return Err("This is a template's chip - edit it in Library → Templates".into());
    }
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let old_type: Option<String> = tx
        .query_row("SELECT type FROM item WHERE id = ?1", params![id], |r| r.get(0))
        .ok();
    let changed = tx
        .execute(
            "UPDATE item SET name = ?1, type = ?2, colour = ?3, notes = ?4, template_id = ?5, category_id = ?6
             WHERE id = ?7",
            params![
                name,
                input.item_type,
                blank_to_none(&input.colour),
                blank_to_none(&input.notes),
                input.template_id,
                input.category_id,
                id
            ],
        )
        .map_err(|e| e.to_string())?;
    if changed == 0 {
        return Err("Item not found".into());
    }
    tx.execute("DELETE FROM item_ingredient WHERE item_id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    write_lines(&tx, id, &input.lines)?;

    // Only meat entries carry serves. Keep already-planned entries consistent when the type
    // changes: a meal becoming meat needs a serves count to adjust; the reverse must drop it.
    let now_meat = input.item_type == "meat";
    if now_meat && old_type.as_deref() != Some("meat") {
        let default: i64 = super::settings::get(&tx, "default_meat_serves")?
            .and_then(|v| v.parse().ok())
            .unwrap_or(3);
        tx.execute(
            "UPDATE plan_entry SET serves = ?1 WHERE item_id = ?2 AND serves IS NULL",
            params![default, id],
        )
        .map_err(|e| e.to_string())?;
    } else if !now_meat && old_type.as_deref() == Some("meat") {
        tx.execute("UPDATE plan_entry SET serves = NULL WHERE item_id = ?1", params![id])
            .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    get(conn, id)
}

/// Copies an item (and its lines) as "<name> (copy)" for quick variants.
pub fn duplicate(conn: &mut Connection, id: i64) -> Result<Item, String> {
    let src = get(conn, id)?;
    let input = ItemInput {
        name: format!("{} (copy)", src.name),
        item_type: src.item_type,
        colour: src.colour,
        notes: src.notes,
        template_id: src.template_id,
        category_id: src.category_id,
        lines: src
            .lines
            .iter()
            .map(|l| LineInput { ingredient_id: l.ingredient_id, quantity: l.quantity })
            .collect(),
    };
    create(conn, &input)
}

/// Archives (or restores) rather than deleting, so past plans still render.
pub fn archive(conn: &Connection, id: i64, archived: bool) -> Result<Item, String> {
    let changed = conn
        .execute("UPDATE item SET archived = ?1 WHERE id = ?2", params![archived, id])
        .map_err(|e| friendly(e, "Item"))?;
    if changed == 0 {
        return Err("Item not found".into());
    }
    get(conn, id)
}

#[tauri::command]
pub fn items_list(
    db: State<Db>,
    item_type: Option<String>,
    search: Option<String>,
    include_archived: Option<bool>,
) -> Result<Vec<ItemSummary>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    list(&conn, item_type.as_deref(), search.as_deref(), include_archived.unwrap_or(false))
}

#[tauri::command]
pub fn items_get(db: State<Db>, id: i64) -> Result<Item, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    get(&conn, id)
}

#[tauri::command]
pub fn items_create(db: State<Db>, input: ItemInput) -> Result<Item, String> {
    let mut conn = db.0.lock().map_err(|e| e.to_string())?;
    create(&mut conn, &input)
}

#[tauri::command]
pub fn items_update(db: State<Db>, id: i64, input: ItemInput) -> Result<Item, String> {
    let mut conn = db.0.lock().map_err(|e| e.to_string())?;
    update(&mut conn, id, &input)
}

#[tauri::command]
pub fn items_duplicate(db: State<Db>, id: i64) -> Result<Item, String> {
    let mut conn = db.0.lock().map_err(|e| e.to_string())?;
    duplicate(&mut conn, id)
}

#[tauri::command]
pub fn items_archive(db: State<Db>, id: i64, archived: bool) -> Result<Item, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    archive(&conn, id, archived)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_conn;

    fn ingredient_id(conn: &Connection, name: &str) -> i64 {
        conn.query_row("SELECT id FROM ingredient WHERE name = ?1", params![name], |r| r.get(0))
            .unwrap()
    }

    fn input(conn: &Connection, name: &str, t: &str) -> ItemInput {
        ItemInput {
            name: name.into(),
            item_type: t.into(),
            colour: Some("  ".into()),
            notes: None,
            template_id: None,
            category_id: None,
            lines: vec![
                LineInput { ingredient_id: ingredient_id(conn, "Pork mince"), quantity: 500.0 },
                LineInput { ingredient_id: ingredient_id(conn, "Onion"), quantity: 1.0 },
            ],
        }
    }

    #[test]
    fn list_filters_by_type_and_search() {
        let conn = test_conn();
        assert_eq!(list(&conn, None, None, false).unwrap().len(), 10);
        let meats = list(&conn, Some("meat"), None, false).unwrap();
        assert_eq!(meats.len(), 3);
        let found = list(&conn, None, Some(" spag "), false).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "Spag bol");
        assert_eq!(found[0].ingredient_count, 5);
    }

    #[test]
    fn create_get_update_roundtrip() {
        let mut conn = test_conn();
        let i = input(&conn, "Pork stir fry", "meal");
        let created = create(&mut conn, &i).unwrap();
        assert_eq!(created.lines.len(), 2);
        assert_eq!(created.lines[0].ingredient_name, "Pork mince");
        assert_eq!(created.colour, None, "blank colour is stored as NULL");

        let mut edit = input(&conn, "Pork stir fry v2", "meal");
        edit.lines.reverse();
        edit.lines.pop();
        let updated = update(&mut conn, created.id, &edit).unwrap();
        assert_eq!(updated.name, "Pork stir fry v2");
        assert_eq!(updated.lines.len(), 1);
        assert_eq!(updated.lines[0].ingredient_name, "Onion");
    }

    #[test]
    fn meat_quantities_are_stored_per_serve() {
        let conn = test_conn();
        let steak = list(&conn, Some("meat"), Some("Steak"), false).unwrap().remove(0);
        let steak = get(&conn, steak.id).unwrap();
        assert_eq!(steak.lines[0].quantity, 250.0);
    }

    fn as_input(conn: &Connection, id: i64, item_type: &str) -> ItemInput {
        let it = get(conn, id).unwrap();
        ItemInput {
            name: it.name,
            item_type: item_type.into(),
            colour: it.colour,
            notes: it.notes,
            template_id: None,
            category_id: None,
            lines: it.lines.iter().map(|l| LineInput { ingredient_id: l.ingredient_id, quantity: l.quantity }).collect(),
        }
    }

    #[test]
    fn changing_type_keeps_planned_entries_consistent() {
        use crate::commands::{plan, settings};
        let mut conn = test_conn();
        let bol = list(&conn, None, Some("Spag bol"), false).unwrap().remove(0).id;
        let steak = list(&conn, None, Some("Steak"), false).unwrap().remove(0).id;
        let bol_entry = plan::add(&conn, bol, "2026-10-06", None).unwrap();
        let steak_entry = plan::add(&conn, steak, "2026-10-06", Some(4)).unwrap();
        assert_eq!((bol_entry.serves, steak_entry.serves), (None, Some(4)));

        // meal -> meat: entries get the default serves so they can be adjusted.
        settings::set(&conn, "default_meat_serves", "5").unwrap();
        let to_meat = as_input(&conn, bol, "meat");
        update(&mut conn, bol, &to_meat).unwrap();
        let day = plan::list(&conn, "2026-10-06", "2026-10-06").unwrap();
        assert_eq!(day.iter().find(|e| e.item_id == bol).unwrap().serves, Some(5));

        // meat -> meal: stale serves are dropped.
        let to_meal = as_input(&conn, steak, "meal");
        update(&mut conn, steak, &to_meal).unwrap();
        let day = plan::list(&conn, "2026-10-06", "2026-10-06").unwrap();
        assert_eq!(day.iter().find(|e| e.item_id == steak).unwrap().serves, None);

        // Edits that don't change the type leave serves alone.
        plan::update(&conn, bol_entry.id, Some(2), None).unwrap();
        let same = as_input(&conn, bol, "meat");
        update(&mut conn, bol, &same).unwrap();
        assert_eq!(plan::list(&conn, "2026-10-06", "2026-10-06").unwrap().iter().find(|e| e.item_id == bol).unwrap().serves, Some(2));
    }

    #[test]
    fn duplicate_copies_lines() {
        let mut conn = test_conn();
        let bol = list(&conn, None, Some("Spag bol"), false).unwrap().remove(0);
        let copy = duplicate(&mut conn, bol.id).unwrap();
        assert_eq!(copy.name, "Spag bol (copy)");
        assert_eq!(copy.lines.len(), 5);
        assert_ne!(copy.id, bol.id);
    }

    #[test]
    fn archive_hides_from_default_list() {
        let mut conn = test_conn();
        let temp = input(&conn, "Temp", "side");
        let created = create(&mut conn, &temp).unwrap();
        archive(&conn, created.id, true).unwrap();
        assert!(!list(&conn, None, Some("Temp"), false).unwrap().iter().any(|i| i.id == created.id));
        assert!(list(&conn, None, Some("Temp"), true).unwrap().iter().any(|i| i.id == created.id));
        // Still readable by id so past plans can render it.
        assert!(get(&conn, created.id).unwrap().archived);
    }

    #[test]
    fn rejects_bad_input() {
        let mut conn = test_conn();
        let blank = input(&conn, " ", "meal");
        let bad_type = input(&conn, "X", "dessert");
        assert!(create(&mut conn, &blank).is_err());
        assert!(create(&mut conn, &bad_type).is_err());
        let mut zero = input(&conn, "X", "meal");
        zero.lines[0].quantity = 0.0;
        assert!(create(&mut conn, &zero).is_err());
        let ok = input(&conn, "X", "meal");
        assert_eq!(update(&mut conn, 9999, &ok).unwrap_err(), "Item not found");
    }
}
