use std::collections::HashMap;

use chrono::NaiveDate;
use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::State;

use crate::db::Db;

/// One plan entry's contribution to an ingredient total.
#[derive(Serialize, Debug, PartialEq)]
pub struct Source {
    pub item_id: i64,
    pub item_name: String,
    pub date: String,
    /// Serves on the entry (meat items only).
    pub serves: Option<i64>,
    /// Size multiplier on a meal or side entry; only present when it isn't the usual 1.
    pub scale: Option<f64>,
    /// Quantity this entry adds, in the ingredient's unit (already scaled by serves).
    pub quantity: f64,
}

#[derive(Serialize, Debug)]
pub struct SummaryRow {
    pub ingredient_id: i64,
    pub ingredient_name: String,
    pub category_id: i64,
    pub category_name: String,
    pub unit: String,
    /// Total quantity; `each` totals are rounded up to whole items.
    pub quantity: f64,
    pub sources: Vec<Source>,
}

#[derive(Serialize, Debug)]
pub struct Summary {
    pub start: String,
    pub end: String,
    /// Plan entries in the range (including ones whose item has no ingredients).
    pub entry_count: i64,
    /// Distinct days in the range that have at least one entry.
    pub dinner_days: i64,
    /// Sorted by category sort order, then ingredient name.
    pub rows: Vec<SummaryRow>,
}

fn parse_date(s: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| format!("Invalid date: {s}"))
}

/// Totals every ingredient across plan entries dated `start..=end`.
///
/// Meat lines are per serve and scale by the entry's serves; meal and side lines
/// are fixed. Archived items and ingredients still count so past ranges stay accurate.
pub fn summary(conn: &Connection, start: &str, end: &str) -> Result<Summary, String> {
    let (s, e) = (parse_date(start)?, parse_date(end)?);
    if s > e {
        return Err("Start date must be on or before the end date".into());
    }
    // Normalise so string comparison in SQL matches the ISO format we store.
    let (start, end) = (s.format("%Y-%m-%d").to_string(), e.format("%Y-%m-%d").to_string());

    let (entry_count, dinner_days) = conn
        .query_row(
            "SELECT COUNT(*), COUNT(DISTINCT date) FROM plan_entry WHERE date BETWEEN ?1 AND ?2",
            params![start, end],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
        )
        .map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT g.id, g.name, g.unit, c.id, c.name, c.sort_order,
                    i.id, i.name, i.type, pe.date, pe.serves, ii.quantity, pe.scale,
                    pe.position, pe.id, 0, ii.position, ii.id
             FROM plan_entry pe
             JOIN item i ON i.id = pe.item_id
             JOIN item_ingredient ii ON ii.item_id = i.id
             JOIN ingredient g ON g.id = ii.ingredient_id
             JOIN ingredient_category c ON c.id = g.category_id
             WHERE pe.date BETWEEN ?1 AND ?2
             UNION ALL
             -- What each template chip picked for its slots on that night.
             SELECT g.id, g.name, g.unit, c.id, c.name, c.sort_order,
                    i.id, i.name, i.type, pe.date, pe.serves, k.quantity, pe.scale,
                    pe.position, pe.id, 1, k.position, k.id
             FROM plan_entry pe
             JOIN item i ON i.id = pe.item_id
             JOIN plan_entry_pick k ON k.entry_id = pe.id
             JOIN ingredient g ON g.id = k.ingredient_id
             JOIN ingredient_category c ON c.id = g.category_id
             WHERE pe.date BETWEEN ?1 AND ?2
             ORDER BY 10, 14, 15, 16, 17, 18",
        )
        .map_err(|e| e.to_string())?;

    let mut rows: Vec<SummaryRow> = Vec::new();
    let mut sort_orders: Vec<i64> = Vec::new(); // parallel to `rows`
    let mut index: HashMap<i64, usize> = HashMap::new();

    let mut query = stmt.query(params![start, end]).map_err(|e| e.to_string())?;
    while let Some(r) = query.next().map_err(|e| e.to_string())? {
        let ingredient_id: i64 = r.get(0).map_err(|e| e.to_string())?;
        let item_type: String = r.get(8).map_err(|e| e.to_string())?;
        let serves: Option<i64> = r.get(10).map_err(|e| e.to_string())?;
        let line_quantity: f64 = r.get(11).map_err(|e| e.to_string())?;
        let entry_scale: f64 = r.get(12).map_err(|e| e.to_string())?;

        let is_meat = item_type == "meat";
        // Meat scales by serves (required; fall back to 1 so bad data can't silently drop a
        // line). Meals and sides are one night's amount times the entry's size multiplier.
        let factor = if is_meat { serves.unwrap_or(1) as f64 } else { entry_scale };
        let quantity = line_quantity * factor;
        let shown_scale = (!is_meat && (entry_scale - 1.0).abs() > f64::EPSILON).then_some(entry_scale);

        let slot = match index.get(&ingredient_id) {
            Some(&i) => i,
            None => {
                rows.push(SummaryRow {
                    ingredient_id,
                    ingredient_name: r.get(1).map_err(|e| e.to_string())?,
                    category_id: r.get(3).map_err(|e| e.to_string())?,
                    category_name: r.get(4).map_err(|e| e.to_string())?,
                    unit: r.get(2).map_err(|e| e.to_string())?,
                    quantity: 0.0,
                    sources: Vec::new(),
                });
                sort_orders.push(r.get(5).map_err(|e| e.to_string())?);
                index.insert(ingredient_id, rows.len() - 1);
                rows.len() - 1
            }
        };

        rows[slot].quantity += quantity;
        rows[slot].sources.push(Source {
            item_id: r.get(6).map_err(|e| e.to_string())?,
            item_name: r.get(7).map_err(|e| e.to_string())?,
            date: r.get(9).map_err(|e| e.to_string())?,
            serves: if is_meat { serves } else { None },
            scale: shown_scale,
            quantity,
        });
    }

    for row in &mut rows {
        if row.unit == "each" {
            // Small epsilon so float noise (e.g. 3.0000000000000004) doesn't round up.
            row.quantity = (row.quantity - 1e-9).ceil();
        }
    }

    let mut order: Vec<usize> = (0..rows.len()).collect();
    order.sort_by(|&a, &b| {
        sort_orders[a]
            .cmp(&sort_orders[b])
            .then_with(|| rows[a].ingredient_name.to_lowercase().cmp(&rows[b].ingredient_name.to_lowercase()))
    });
    let mut slots: Vec<Option<SummaryRow>> = rows.into_iter().map(Some).collect();
    let rows = order.into_iter().map(|i| slots[i].take().unwrap()).collect();

    Ok(Summary { start, end, entry_count, dinner_days, rows })
}

#[tauri::command]
pub fn summary_get(db: State<Db>, start: String, end: String) -> Result<Summary, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    summary(&conn, &start, &end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::items::{self, ItemInput, LineInput};
    use crate::db::test_conn;

    fn item_id(conn: &Connection, name: &str) -> i64 {
        conn.query_row("SELECT id FROM item WHERE name = ?1", params![name], |r| r.get(0)).unwrap()
    }

    fn ingredient_id(conn: &Connection, name: &str) -> i64 {
        conn.query_row("SELECT id FROM ingredient WHERE name = ?1", params![name], |r| r.get(0))
            .unwrap()
    }

    fn plan(conn: &Connection, date: &str, item: &str, serves: Option<i64>) {
        let position: i64 = conn
            .query_row("SELECT COUNT(*) FROM plan_entry WHERE date = ?1", params![date], |r| r.get(0))
            .unwrap();
        conn.execute(
            "INSERT INTO plan_entry (date, item_id, serves, position) VALUES (?1, ?2, ?3, ?4)",
            params![date, item_id(conn, item), serves, position],
        )
        .unwrap();
    }

    fn row<'a>(s: &'a Summary, name: &str) -> &'a SummaryRow {
        s.rows.iter().find(|r| r.ingredient_name == name).unwrap_or_else(|| panic!("no row {name}"))
    }

    const WEEK: (&str, &str) = ("2026-10-05", "2026-10-11");

    #[test]
    fn empty_range_has_no_rows() {
        let conn = test_conn();
        let s = summary(&conn, WEEK.0, WEEK.1).unwrap();
        assert!(s.rows.is_empty());
        assert_eq!((s.entry_count, s.dinner_days), (0, 0));
    }

    #[test]
    fn meal_is_fixed_quantity() {
        let conn = test_conn();
        plan(&conn, "2026-10-06", "Spag bol", None);
        let s = summary(&conn, WEEK.0, WEEK.1).unwrap();
        assert_eq!(row(&s, "Beef mince").quantity, 500.0);
        assert_eq!(row(&s, "Spaghetti").quantity, 400.0);
        assert_eq!(s.rows.len(), 5);
    }

    #[test]
    fn side_is_fixed_quantity_and_ignores_serves() {
        let conn = test_conn();
        // Serves should never be set on a side, but must not scale it if it is.
        plan(&conn, "2026-10-06", "Chips", Some(4));
        let s = summary(&conn, WEEK.0, WEEK.1).unwrap();
        assert_eq!(row(&s, "Frozen chips").quantity, 600.0);
        assert_eq!(row(&s, "Frozen chips").sources[0].serves, None);
    }

    #[test]
    fn meat_scales_by_serves() {
        let conn = test_conn();
        plan(&conn, "2026-10-06", "Steak", Some(3));
        plan(&conn, "2026-10-07", "Sausages", Some(2));
        let s = summary(&conn, WEEK.0, WEEK.1).unwrap();
        assert_eq!(row(&s, "Steak").quantity, 750.0);
        assert_eq!(row(&s, "Sausages").quantity, 4.0);
        assert_eq!(row(&s, "Steak").sources[0].serves, Some(3));
    }

    #[test]
    fn multiple_entries_on_one_day() {
        let conn = test_conn();
        plan(&conn, "2026-10-06", "Steak", Some(3));
        plan(&conn, "2026-10-06", "Chips", None);
        plan(&conn, "2026-10-06", "Garden salad", None);
        let s = summary(&conn, WEEK.0, WEEK.1).unwrap();
        assert_eq!((s.entry_count, s.dinner_days), (3, 1));
        assert_eq!(row(&s, "Steak").quantity, 750.0);
        assert_eq!(row(&s, "Frozen chips").quantity, 600.0);
        assert_eq!(row(&s, "Lettuce").quantity, 1.0);
    }

    #[test]
    fn entry_size_multiplies_meals_and_sides_but_not_meat() {
        let conn = test_conn();
        let bol = crate::commands::plan::add(&conn, item_id(&conn, "Spag bol"), "2026-10-06", None).unwrap();
        crate::commands::plan::update_with(&conn, bol.id, None, None, Some(1.5)).unwrap();
        let chips = crate::commands::plan::add(&conn, item_id(&conn, "Chips"), "2026-10-06", None).unwrap();
        crate::commands::plan::update_with(&conn, chips.id, None, None, Some(2.0)).unwrap();
        plan(&conn, "2026-10-07", "Spag bol", None); // a normal-sized one too

        let s = summary(&conn, WEEK.0, WEEK.1).unwrap();
        assert_eq!(row(&s, "Beef mince").quantity, 1250.0); // 500*1.5 + 500
        assert_eq!(row(&s, "Frozen chips").quantity, 1200.0);
        let beef = row(&s, "Beef mince");
        assert_eq!(beef.sources[0].scale, Some(1.5));
        assert_eq!(beef.sources[1].scale, None, "usual size isn't called out");
        // each totals still round up after scaling: 1 onion * 1.5 + 1 = 2.5 -> 3
        assert_eq!(row(&s, "Onion").quantity, 3.0);
    }

    #[test]
    fn same_ingredient_across_item_types_is_combined() {
        let conn = test_conn();
        plan(&conn, "2026-10-05", "Chow mein", None); // carrot 2
        plan(&conn, "2026-10-06", "Spag bol", None); // carrot 1
        plan(&conn, "2026-10-07", "Garden salad", None); // carrot 1
        plan(&conn, "2026-10-08", "Steamed veg", None); // carrot 2
        let s = summary(&conn, WEEK.0, WEEK.1).unwrap();
        let carrot = row(&s, "Carrot");
        assert_eq!(carrot.quantity, 6.0);
        assert_eq!(carrot.sources.len(), 4);
        assert_eq!(row(&s, "Beef mince").quantity, 1000.0);
        let names: Vec<&str> = row(&s, "Beef mince").sources.iter().map(|s| s.item_name.as_str()).collect();
        assert_eq!(names, ["Chow mein", "Spag bol"]);
    }

    #[test]
    fn same_item_on_two_days_counts_twice() {
        let conn = test_conn();
        plan(&conn, "2026-10-05", "Spag bol", None);
        plan(&conn, "2026-10-09", "Spag bol", None);
        let s = summary(&conn, WEEK.0, WEEK.1).unwrap();
        assert_eq!(row(&s, "Beef mince").quantity, 1000.0);
        assert_eq!(row(&s, "Beef mince").sources.len(), 2);
    }

    #[test]
    fn each_totals_round_up_but_weights_do_not() {
        let mut conn = test_conn();
        let input = ItemInput {
            name: "Half sausage".into(),
            item_type: "meat".into(),
            colour: None,
            notes: None,
            template_id: None,
            lines: vec![
                LineInput { ingredient_id: ingredient_id(&conn, "Sausages"), quantity: 1.5 },
                LineInput { ingredient_id: ingredient_id(&conn, "Beef mince"), quantity: 62.5 },
            ],
        };
        items::create(&mut conn, &input).unwrap();
        plan(&conn, "2026-10-06", "Half sausage", Some(3));
        let s = summary(&conn, WEEK.0, WEEK.1).unwrap();
        assert_eq!(row(&s, "Sausages").quantity, 5.0); // 4.5 -> 5
        assert_eq!(row(&s, "Beef mince").quantity, 187.5); // g left as-is
        // The per-entry source keeps the exact figure.
        assert_eq!(row(&s, "Sausages").sources[0].quantity, 4.5);
    }

    #[test]
    fn exact_each_totals_do_not_round_up_from_float_noise() {
        let mut conn = test_conn();
        let input = ItemInput {
            name: "Tenths".into(),
            item_type: "meat".into(),
            colour: None,
            notes: None,
            template_id: None,
            lines: vec![LineInput { ingredient_id: ingredient_id(&conn, "Carrot"), quantity: 0.1 }],
        };
        items::create(&mut conn, &input).unwrap();
        plan(&conn, "2026-10-06", "Tenths", Some(30)); // 0.1 * 30 = 3.0000000000000004
        let s = summary(&conn, WEEK.0, WEEK.1).unwrap();
        assert_eq!(row(&s, "Carrot").quantity, 3.0);
    }

    #[test]
    fn range_is_inclusive_and_excludes_outside_dates() {
        let conn = test_conn();
        plan(&conn, "2026-10-04", "Chips", None); // day before
        plan(&conn, "2026-10-05", "Steak", Some(1)); // first day
        plan(&conn, "2026-10-11", "Steak", Some(1)); // last day
        plan(&conn, "2026-10-12", "Steak", Some(1)); // day after
        let s = summary(&conn, WEEK.0, WEEK.1).unwrap();
        assert_eq!(row(&s, "Steak").quantity, 500.0);
        assert!(s.rows.iter().all(|r| r.ingredient_name != "Frozen chips"));
        assert_eq!(s.entry_count, 2);
        // A single-day range works.
        let one = summary(&conn, "2026-10-05", "2026-10-05").unwrap();
        assert_eq!(row(&one, "Steak").quantity, 250.0);
    }

    #[test]
    fn rows_sorted_by_category_then_name() {
        let conn = test_conn();
        plan(&conn, "2026-10-05", "Chow mein", None);
        plan(&conn, "2026-10-06", "Steak", Some(1));
        let s = summary(&conn, WEEK.0, WEEK.1).unwrap();
        let order: Vec<(&str, &str)> =
            s.rows.iter().map(|r| (r.category_name.as_str(), r.ingredient_name.as_str())).collect();
        assert_eq!(
            order,
            [
                ("Meat", "Beef mince"),
                ("Meat", "Steak"),
                ("Veg", "Cabbage"),
                ("Veg", "Carrot"),
                ("Veg", "Onion"),
                ("Pantry", "Chow mein noodles"),
                ("Pantry", "Soy sauce"),
            ]
        );
    }

    #[test]
    fn archived_items_and_ingredients_still_count() {
        let conn = test_conn();
        plan(&conn, "2026-10-06", "Steak", Some(2));
        conn.execute("UPDATE item SET archived = 1 WHERE name = 'Steak'", []).unwrap();
        conn.execute("UPDATE ingredient SET archived = 1 WHERE name = 'Steak'", []).unwrap();
        let s = summary(&conn, WEEK.0, WEEK.1).unwrap();
        assert_eq!(row(&s, "Steak").quantity, 500.0);
    }

    #[test]
    fn item_edits_apply_to_past_entries() {
        let mut conn = test_conn();
        plan(&conn, "2026-09-01", "Spag bol", None);
        let bol = item_id(&conn, "Spag bol");
        let mut edit = items::get(&conn, bol).unwrap();
        edit.lines.iter_mut().find(|l| l.ingredient_name == "Beef mince").unwrap().quantity = 750.0;
        let input = ItemInput {
            name: edit.name,
            item_type: edit.item_type,
            colour: edit.colour,
            notes: edit.notes,
            template_id: edit.template_id,
            lines: edit
                .lines
                .iter()
                .map(|l| LineInput { ingredient_id: l.ingredient_id, quantity: l.quantity })
                .collect(),
        };
        items::update(&mut conn, bol, &input).unwrap();
        let s = summary(&conn, "2026-09-01", "2026-09-07").unwrap();
        assert_eq!(row(&s, "Beef mince").quantity, 750.0);
    }

    #[test]
    fn rejects_bad_ranges() {
        let conn = test_conn();
        assert!(summary(&conn, "2026-10-11", "2026-10-05").is_err());
        assert!(summary(&conn, "not-a-date", "2026-10-05").is_err());
        assert!(summary(&conn, "2026-02-30", "2026-03-05").is_err());
    }
}
