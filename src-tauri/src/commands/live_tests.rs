//! Tests for the slots / live templates / blank token work, and for carrying older data forward.

use rusqlite::{params, Connection};

use super::plan::{self, PickInput};
use super::templates::{self, LineInput, OptionInput, TemplateInput};
use super::{backup, summary, undo::UndoStack};
use crate::db::{self, test_conn};

fn ing(conn: &Connection, name: &str) -> i64 {
    conn.query_row("SELECT id FROM ingredient WHERE name = ?1", params![name], |r| r.get(0)).unwrap()
}

fn item(conn: &Connection, name: &str) -> i64 {
    conn.query_row("SELECT id FROM item WHERE name = ?1", params![name], |r| r.get(0)).unwrap()
}

fn count(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get(0)).unwrap()
}

/// "Steamed veg": a side with one multi-pick slot offering carrot / pumpkin-ish / potato.
fn steamed_veg(conn: &mut Connection) -> templates::Template {
    let opt = |name: &str, q: f64| OptionInput { ingredient_id: ing(conn, name), quantity: Some(q) };
    let options = vec![opt("Carrot", 2.0), opt("Potato", 400.0), opt("Broccoli", 1.0)];
    let input = TemplateInput {
        name: "Steamed veggies".into(),
        item_type: "side".into(),
        colour: Some("#16a34a".into()),
        category_id: None,
        lines: vec![LineInput { label: "Veg".into(), multi: true, options, ..Default::default() }],
    };
    templates::create(conn, &input).unwrap()
}

fn pick(conn: &Connection, line: i64, name: &str) -> PickInput {
    PickInput { line_id: Some(line), ingredient_id: ing(conn, name), quantity: None }
}

// ---- templates -------------------------------------------------------------------------

#[test]
fn every_template_gets_a_chip_and_fixed_lines_become_its_ingredients() {
    let conn = test_conn();
    let (item_id, ty, sample): (i64, String, i64) = conn
        .query_row(
            "SELECT i.id, i.type, i.sample FROM template t JOIN item i ON i.id = t.item_id WHERE t.name = 'Risotto'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!((ty.as_str(), sample), ("meal", 1));
    assert_eq!(count(&conn, &format!("SELECT COUNT(*) FROM item_ingredient WHERE item_id = {item_id}")), 3);
}

#[test]
fn creating_a_template_makes_its_chip_and_editing_keeps_line_ids() {
    let mut conn = test_conn();
    let t = steamed_veg(&mut conn);
    let chip = t.item_id.expect("chip");
    let (name, ty, colour): (String, String, Option<String>) = conn
        .query_row("SELECT name, type, colour FROM item WHERE id = ?1", params![chip], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .unwrap();
    assert_eq!((name.as_str(), ty.as_str(), colour.as_deref()), ("Steamed veggies", "side", Some("#16a34a")));
    assert!(t.lines[0].multi);
    assert_eq!(t.lines[0].options.len(), 3);
    assert_eq!(t.lines[0].options[1].ingredient_name, "Potato");

    // Edit: rename, add an option. The slot keeps its id so planned nights stay attached.
    let line = &t.lines[0];
    let mut options: Vec<OptionInput> =
        line.options.iter().map(|o| OptionInput { ingredient_id: o.ingredient_id, quantity: o.quantity }).collect();
    options.push(OptionInput { ingredient_id: ing(&conn, "Zucchini"), quantity: Some(1.0) });
    let edited = TemplateInput {
        name: "Steamed veg".into(),
        item_type: "side".into(),
        colour: None,
        category_id: None,
        lines: vec![LineInput { id: Some(line.id), label: "Veg".into(), multi: true, options, ..Default::default() }],
    };
    let t2 = templates::update(&mut conn, t.id, &edited).unwrap();
    assert_eq!(t2.lines[0].id, line.id);
    assert_eq!(t2.lines[0].options.len(), 4);
    assert_eq!(t2.item_id, Some(chip), "same chip");
    let new_name: String = conn.query_row("SELECT name FROM item WHERE id = ?1", params![chip], |r| r.get(0)).unwrap();
    assert_eq!(new_name, "Steamed veg");

    // A line id from some other template is refused.
    let bad = TemplateInput {
        name: "x".into(),
        item_type: "side".into(),
        colour: None,
        category_id: None,
        lines: vec![LineInput { id: Some(999_999), label: "Veg".into(), ..Default::default() }],
    };
    assert!(templates::update(&mut conn, t.id, &bad).is_err());
}

#[test]
fn the_chip_cannot_be_edited_as_a_plain_item() {
    let mut conn = test_conn();
    let t = steamed_veg(&mut conn);
    let err = super::items::update(
        &mut conn,
        t.item_id.unwrap(),
        &super::items::ItemInput {
            name: "x".into(),
            item_type: "side".into(),
            colour: None,
            notes: None,
            template_id: None,
            category_id: None,
            lines: vec![],
        },
    )
    .unwrap_err();
    assert!(err.contains("Templates"), "{err}");
}

#[test]
fn deleting_a_planned_template_archives_its_chip_otherwise_removes_it() {
    let mut conn = test_conn();
    let planned = steamed_veg(&mut conn);
    plan::add(&conn, planned.item_id.unwrap(), "2026-10-06", None).unwrap();
    templates::delete(&mut conn, planned.id).unwrap();
    let archived: i64 =
        conn.query_row("SELECT archived FROM item WHERE id = ?1", params![planned.item_id.unwrap()], |r| r.get(0)).unwrap();
    assert_eq!(archived, 1, "past weeks still render");

    let unused = steamed_veg(&mut conn);
    let chip = unused.item_id.unwrap();
    templates::delete(&mut conn, unused.id).unwrap();
    assert_eq!(count(&conn, &format!("SELECT COUNT(*) FROM item WHERE id = {chip}")), 0);
}

// ---- picks -----------------------------------------------------------------------------

#[test]
fn a_night_picks_its_own_veg_and_the_summary_counts_only_those() {
    let mut conn = test_conn();
    let t = steamed_veg(&mut conn);
    let line = t.lines[0].id;
    let mon = plan::add(&conn, t.item_id.unwrap(), "2026-10-05", None).unwrap();
    let tue = plan::add(&conn, t.item_id.unwrap(), "2026-10-06", None).unwrap();
    assert_eq!(mon.template_id, Some(t.id));
    assert!(mon.picks.is_empty());

    plan::set_picks(&conn, mon.id, &[pick(&conn, line, "Carrot")]).unwrap();
    let tue = plan::set_picks(&conn, tue.id, &[pick(&conn, line, "Carrot"), pick(&conn, line, "Potato")]).unwrap();
    // Amounts default from the template's option for that ingredient.
    let got: Vec<(&str, f64)> = tue.picks.iter().map(|p| (p.ingredient_name.as_str(), p.quantity)).collect();
    assert_eq!(got, [("Carrot", 2.0), ("Potato", 400.0)]);

    let s = summary::summary(&conn, "2026-10-05", "2026-10-11").unwrap();
    let row = |n: &str| s.rows.iter().find(|r| r.ingredient_name == n).map(|r| r.quantity);
    assert_eq!(row("Carrot"), Some(4.0));
    assert_eq!(row("Potato"), Some(400.0));
    assert_eq!(row("Broccoli"), None, "an option nobody picked adds nothing");

    // Size multiplies picks like any other meal line, and the pick shows in the sources.
    plan::update_with(&conn, tue.id, None, None, Some(2.0)).unwrap();
    let s = summary::summary(&conn, "2026-10-05", "2026-10-11").unwrap();
    assert_eq!(s.rows.iter().find(|r| r.ingredient_name == "Potato").unwrap().quantity, 800.0);
    let carrot = s.rows.iter().find(|r| r.ingredient_name == "Carrot").unwrap();
    assert_eq!(carrot.sources.iter().map(|x| x.date.as_str()).collect::<Vec<_>>(), ["2026-10-05", "2026-10-06"]);
}

#[test]
fn picks_are_snapshots_so_template_edits_do_not_change_planned_nights() {
    let mut conn = test_conn();
    let t = steamed_veg(&mut conn);
    let line = t.lines[0].id;
    let e = plan::add(&conn, t.item_id.unwrap(), "2026-10-06", None).unwrap();
    plan::set_picks(&conn, e.id, &[pick(&conn, line, "Potato")]).unwrap();

    // The template now suggests 900 g of potato and drops the slot entirely.
    let edited = TemplateInput { name: "Steamed veggies".into(), item_type: "side".into(), colour: None, category_id: None, lines: vec![] };
    templates::update(&mut conn, t.id, &edited).unwrap();

    let back = plan::list(&conn, "2026-10-06", "2026-10-06").unwrap();
    assert_eq!(back[0].picks.len(), 1);
    assert_eq!((back[0].picks[0].quantity, back[0].picks[0].line_id), (400.0, None));
}

#[test]
fn bad_picks_are_rejected() {
    let mut conn = test_conn();
    let t = steamed_veg(&mut conn);
    let line = t.lines[0].id;
    let e = plan::add(&conn, t.item_id.unwrap(), "2026-10-06", None).unwrap();
    // No default for an ingredient that is not one of the options.
    assert!(plan::set_picks(&conn, e.id, &[pick(&conn, line, "Onion")]).unwrap_err().contains("Set an amount"));
    // ...but an explicit amount is fine.
    let ok = PickInput { line_id: Some(line), ingredient_id: ing(&conn, "Onion"), quantity: Some(1.0) };
    assert!(plan::set_picks(&conn, e.id, &[ok]).is_ok());
    // Plain meals have nothing to pick.
    let plain = plan::add(&conn, item(&conn, "Chow mein"), "2026-10-06", None).unwrap();
    assert!(plan::set_picks(&conn, plain.id, &[]).is_err());
    // A slot from another template.
    let risotto_line: i64 = conn.query_row("SELECT id FROM template_line WHERE label = 'Protein' LIMIT 1", [], |r| r.get(0)).unwrap();
    let foreign = PickInput { line_id: Some(risotto_line), ingredient_id: ing(&conn, "Carrot"), quantity: None };
    assert!(plan::set_picks(&conn, e.id, &[foreign]).is_err());
    // A failed save leaves the earlier picks alone.
    assert_eq!(plan::list(&conn, "2026-10-06", "2026-10-06").unwrap()[0].picks.len(), 1);
}

#[test]
fn risotto_style_single_slot_uses_the_line_default_when_units_match() {
    let conn = test_conn();
    let risotto = templates::list(&conn).unwrap().into_iter().find(|t| t.name == "Risotto").unwrap();
    let t = templates::get(&conn, risotto.id).unwrap();
    let protein = t.lines.iter().find(|l| l.label == "Protein").unwrap().id;
    let veg = t.lines.iter().find(|l| l.label == "Veg 1").unwrap().id;
    let e = plan::add(&conn, t.item_id.unwrap(), "2026-10-06", None).unwrap();
    let e = plan::set_picks(&conn, e.id, &[pick(&conn, protein, "Chicken thigh"), pick(&conn, veg, "Mushroom")]).unwrap();
    assert_eq!(e.picks.iter().map(|p| p.quantity).collect::<Vec<_>>(), [500.0, 200.0]);
    // Carrot is counted "each" but the slot defaults to grams: needs an explicit amount.
    assert!(plan::set_picks(&conn, e.id, &[pick(&conn, veg, "Carrot")]).is_err());
}

#[test]
fn copying_a_week_copies_picks_slots_and_labels() {
    let mut conn = test_conn();
    let t = steamed_veg(&mut conn);
    let line = t.lines[0].id;
    let e = plan::add(&conn, t.item_id.unwrap(), "2026-10-06", None).unwrap();
    plan::set_picks(&conn, e.id, &[pick(&conn, line, "Carrot")]).unwrap();
    plan::add_at(&conn, item(&conn, "Chips"), "2026-10-07", Some("lunch"), None, None).unwrap();
    plan::add_blank(&conn, "2026-10-08", Some("dinner"), Some("Thai takeaway"), None).unwrap();

    plan::copy_range(&conn, "2026-10-05", "2026-10-11", "2026-10-12").unwrap();
    let next = plan::list(&conn, "2026-10-12", "2026-10-18").unwrap();
    assert_eq!(next.len(), 3);
    assert_eq!(next[0].picks.len(), 1);
    assert_eq!(next[1].slot, "lunch");
    assert_eq!(next[2].label.as_deref(), Some("Thai takeaway"));
}

#[test]
fn undo_brings_back_picks() {
    let mut conn = test_conn();
    let t = steamed_veg(&mut conn);
    let line = t.lines[0].id;
    let e = plan::add(&conn, t.item_id.unwrap(), "2026-10-06", None).unwrap();
    plan::set_picks(&conn, e.id, &[pick(&conn, line, "Carrot")]).unwrap();
    let undo = UndoStack::default();
    undo.record(&conn, "Removed".into()).unwrap();
    plan::remove(&conn, e.id).unwrap();
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM plan_entry_pick"), 0, "picks go with the entry");
    undo.undo(&conn).unwrap();
    let back = plan::list(&conn, "2026-10-06", "2026-10-06").unwrap();
    assert_eq!(back[0].picks.len(), 1);
}

// ---- slots and the blank token -----------------------------------------------------------

#[test]
fn slots_are_numbered_separately_and_entries_can_move_between_them() {
    let conn = test_conn();
    let d = "2026-10-06";
    let a = plan::add_at(&conn, item(&conn, "Chips"), d, Some("lunch"), None, None).unwrap();
    let b = plan::add_at(&conn, item(&conn, "Garden salad"), d, Some("lunch"), None, None).unwrap();
    let c = plan::add_at(&conn, item(&conn, "Chow mein"), d, None, None, None).unwrap();
    assert_eq!((a.slot.as_str(), a.position, b.position), ("lunch", 0, 1));
    assert_eq!((c.slot.as_str(), c.position), ("dinner", 0), "dinner starts at 0 despite lunch entries");

    // Move the first lunch into dinner, first place: lunch renumbers, dinner shifts.
    plan::move_to(&conn, a.id, d, "dinner", 0).unwrap();
    let pos = |id: i64| -> (String, i64) {
        conn.query_row("SELECT slot, position FROM plan_entry WHERE id = ?1", params![id], |r| Ok((r.get(0)?, r.get(1)?))).unwrap()
    };
    assert_eq!(pos(a.id), ("dinner".into(), 0));
    assert_eq!(pos(c.id), ("dinner".into(), 1));
    assert_eq!(pos(b.id), ("lunch".into(), 0));

    // A plain move keeps the entry's slot.
    plan::move_entry(&conn, b.id, "2026-10-07", 0).unwrap();
    assert_eq!(plan::list(&conn, "2026-10-07", "2026-10-07").unwrap()[0].slot, "lunch");
    assert!(plan::move_to(&conn, b.id, d, "elevenses", 0).is_err());
    assert!(plan::add_at(&conn, item(&conn, "Chips"), d, Some("brunch"), None, None).is_err());
}

#[test]
fn sweet_savory_items_default_to_the_snack_slot() {
    let mut conn = test_conn();
    let butter = ing(&conn, "Butter");
    let cookies = super::items::create(
        &mut conn,
        &super::items::ItemInput {
            name: "Choc chip cookies".into(),
            item_type: "sweet".into(),
            colour: None,
            notes: None,
            template_id: None,
            category_id: None,
            lines: vec![super::items::LineInput { ingredient_id: butter, quantity: 125.0 }],
        },
    )
    .unwrap();
    let e = plan::add(&conn, cookies.id, "2026-10-06", None).unwrap();
    assert_eq!((e.slot.as_str(), e.serves), ("snack", None));
    let explicit = plan::add_at(&conn, cookies.id, "2026-10-06", Some("dinner"), None, None).unwrap();
    assert_eq!(explicit.slot, "dinner");
    // Its ingredients reach the shopping list like any meal's.
    let s = summary::summary(&conn, "2026-10-06", "2026-10-06").unwrap();
    assert_eq!(s.rows.iter().find(|r| r.ingredient_name == "Butter").unwrap().quantity, 250.0);
}

#[test]
fn the_blank_token_has_a_title_a_hidden_item_and_no_shopping() {
    let conn = test_conn();
    let e = plan::add_blank(&conn, "2026-10-06", None, None, None).unwrap();
    assert_eq!((e.item_type.as_str(), e.label.as_deref(), e.slot.as_str()), ("blank", None, "dinner"));
    let e = plan::set_label(&conn, e.id, "  Thai takeaway ").unwrap();
    assert_eq!(e.label.as_deref(), Some("Thai takeaway"));
    assert_eq!(plan::set_label(&conn, e.id, " ").unwrap().label, None, "blank clears it");

    // One shared system item, never listed with the real ones.
    plan::add_blank(&conn, "2026-10-07", Some("lunch"), Some("Dinner out"), None).unwrap();
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM item WHERE type = 'blank'"), 1);
    let listed = super::items::list(&conn, None, None, true).unwrap();
    assert!(listed.iter().all(|i| i.item_type != "blank"));

    // Counts as a planned entry but adds nothing to the list; real meals can't take a label.
    let s = summary::summary(&conn, "2026-10-06", "2026-10-07").unwrap();
    assert_eq!((s.entry_count, s.rows.len()), (2, 0));
    let meal = plan::add(&conn, item(&conn, "Chow mein"), "2026-10-06", None).unwrap();
    assert!(plan::set_label(&conn, meal.id, "x").is_err());
}

// ---- carrying existing data forward ---------------------------------------------------------

/// A database exactly as the previous version of the app left it (migrations 1-3), with a
/// lived-in plan, then upgraded with the real migrations.
#[test]
fn upgrading_a_lived_in_database_keeps_everything() {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    db::migrations().to_version(&mut conn, 3).unwrap();

    // Old-version content: a custom meal made from a template, a plan with notes/serves/size,
    // leftovers, and a shopping order the user rearranged.
    conn.execute_batch(
        "INSERT INTO item (name, type, template_id) VALUES ('My risotto', 'meal', (SELECT id FROM template WHERE name = 'Risotto'));
         INSERT INTO item_ingredient (item_id, ingredient_id, quantity, position)
           VALUES ((SELECT id FROM item WHERE name = 'My risotto'), (SELECT id FROM ingredient WHERE name = 'Arborio rice'), 300, 0);
         INSERT INTO plan_entry (date, item_id, serves, note, position, scale)
           VALUES ('2026-10-06', (SELECT id FROM item WHERE name = 'Steak'), 4, 'rump', 0, 1.0),
                  ('2026-10-06', (SELECT id FROM item WHERE name = 'My risotto'), NULL, NULL, 1, 1.5);
         INSERT INTO day_info (date, note, leftovers) VALUES ('2026-10-07', 'out late', 1);
         UPDATE ingredient_category SET sort_order = 10 WHERE name = 'Meat';",
    )
    .unwrap();
    let before_items = count(&conn, "SELECT COUNT(*) FROM item");

    db::migrate(&mut conn).unwrap();

    // Plan entries are intact, all dinners, in the same order.
    let plan = plan::list(&conn, "2026-10-01", "2026-10-31").unwrap();
    assert_eq!(plan.len(), 2);
    assert_eq!((plan[0].item_name.as_str(), plan[0].serves, plan[0].note.as_deref()), ("Steak", Some(4), Some("rump")));
    assert_eq!((plan[1].item_name.as_str(), plan[1].scale), ("My risotto", 1.5));
    assert!(plan.iter().all(|e| e.slot == "dinner" && e.picks.is_empty() && e.label.is_none()));
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM day_info"), 1);

    // The meal made from a template is still a static meal with its link and lines.
    let (ty, tid): (String, Option<i64>) = conn
        .query_row("SELECT type, template_id FROM item WHERE name = 'My risotto'", [], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap();
    assert_eq!(ty, "meal");
    assert!(tid.is_some());
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM item_ingredient WHERE item_id = (SELECT id FROM item WHERE name = 'My risotto')"), 1);

    // Both templates now have chips (two more items), and the library's other items are untouched.
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM item"), before_items + 2);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM template WHERE item_id IS NULL"), 0);

    // New categories sit after the user's order; frozen sample foods moved there.
    let order: Vec<String> = {
        let mut stmt = conn.prepare("SELECT name FROM ingredient_category ORDER BY sort_order, name").unwrap();
        stmt.query_map([], |r| r.get(0)).unwrap().collect::<Result<_, _>>().unwrap()
    };
    assert_eq!(order[order.len() - 3..], ["Meat", "Bakery", "Frozen"]);
    let cat = |n: &str| -> String {
        conn.query_row(
            "SELECT c.name FROM ingredient g JOIN ingredient_category c ON c.id = g.category_id WHERE g.name = ?1",
            params![n],
            |r| r.get(0),
        )
        .unwrap()
    };
    assert_eq!((cat("Frozen peas").as_str(), cat("Frozen chips").as_str()), ("Frozen", "Frozen"));
    // The shopping summary still works on old entries.
    assert!(summary::summary(&conn, "2026-10-06", "2026-10-06").unwrap().rows.iter().any(|r| r.ingredient_name == "Steak"));
}

/// Takes today's export and strips it back to the version-1 layout the previous release wrote.
fn as_version_1(json: &str) -> String {
    let mut v: serde_json::Value = serde_json::from_str(json).unwrap();
    let o = v.as_object_mut().unwrap();
    o.insert("version".into(), 1.into());
    o.remove("template_options");
    o.remove("plan_picks");
    for t in o["templates"].as_array_mut().unwrap() {
        let t = t.as_object_mut().unwrap();
        for k in ["item_type", "colour", "item_id"] {
            t.remove(k);
        }
    }
    for l in o["template_lines"].as_array_mut().unwrap() {
        l.as_object_mut().unwrap().remove("multi");
    }
    for p in o["plan_entries"].as_array_mut().unwrap() {
        let p = p.as_object_mut().unwrap();
        p.remove("slot");
        p.remove("label");
    }
    // The previous release had none of the new items or categories.
    let chips: Vec<i64> = {
        let ids: Vec<i64> = o["items"].as_array().unwrap().iter().filter(|i| i["name"] == "Risotto" || i["name"] == "Stir fry").map(|i| i["id"].as_i64().unwrap()).collect();
        ids
    };
    o["items"].as_array_mut().unwrap().retain(|i| !chips.contains(&i["id"].as_i64().unwrap()));
    o["item_ingredients"].as_array_mut().unwrap().retain(|i| !chips.contains(&i["item_id"].as_i64().unwrap()));
    o["categories"].as_array_mut().unwrap().retain(|c| c["name"] != "Bakery" && c["name"] != "Frozen");
    // ...and the frozen foods were a Veg and a Pantry item then.
    let cat = |o: &serde_json::Map<String, serde_json::Value>, n: &str| {
        o["categories"].as_array().unwrap().iter().find(|c| c["name"] == n).unwrap()["id"].clone()
    };
    let (veg, pantry) = (cat(o, "Veg"), cat(o, "Pantry"));
    for i in o["ingredients"].as_array_mut().unwrap() {
        if i["name"] == "Frozen peas" {
            i["category_id"] = veg.clone();
        }
        if i["name"] == "Frozen chips" {
            i["category_id"] = pantry.clone();
        }
    }
    serde_json::to_string(&v).unwrap()
}

#[test]
fn a_version_1_backup_imports_into_the_new_app() {
    let mut source = test_conn();
    let t = steamed_veg(&mut source);
    let _ = t;
    plan::add(&source, item(&source, "Steak"), "2026-10-06", Some(4)).unwrap();
    plan::add(&source, item(&source, "Chow mein"), "2026-10-07", None).unwrap();
    plan::update(&source, 1, None, Some("a note")).unwrap();
    let v1 = as_version_1(&backup::export_json(&source).unwrap());
    assert!(!v1.contains("plan_picks") && !v1.contains("\"slot\""));

    let target = test_conn();
    let summary = backup::import_json(&target, &v1).unwrap();
    assert_eq!(summary.plan_entries, 2);

    let plan = plan::list(&target, "2026-10-01", "2026-10-31").unwrap();
    assert_eq!(plan.len(), 2);
    assert!(plan.iter().all(|e| e.slot == "dinner"));
    assert_eq!(plan[0].note.as_deref(), Some("a note"));

    // Chips were recreated for the templates in the file; categories topped up.
    assert_eq!(count(&target, "SELECT COUNT(*) FROM template WHERE item_id IS NULL"), 0);
    let templates = templates::list(&target).unwrap();
    assert!(templates.iter().any(|t| t.name == "Risotto"));
    assert_eq!(count(&target, "SELECT COUNT(*) FROM ingredient_category WHERE name IN ('Bakery','Frozen')"), 2);
    assert_eq!(count(&target, "SELECT COUNT(*) FROM pragma_foreign_key_check"), 0);

    // Imported templates are live straight away.
    let risotto = templates.iter().find(|t| t.name == "Risotto").unwrap();
    let full = templates::get(&target, risotto.id).unwrap();
    let e = plan::add(&target, full.item_id.unwrap(), "2026-10-08", None).unwrap();
    let protein = full.lines.iter().find(|l| l.label == "Protein").unwrap().id;
    assert!(plan::set_picks(&target, e.id, &[pick(&target, protein, "Pork mince")]).is_ok());
}

#[test]
fn a_current_backup_round_trips_picks_slots_and_options() {
    let mut source = test_conn();
    let t = steamed_veg(&mut source);
    let line = t.lines[0].id;
    let e = plan::add(&source, t.item_id.unwrap(), "2026-10-06", None).unwrap();
    plan::set_picks(&source, e.id, &[pick(&source, line, "Carrot"), pick(&source, line, "Potato")]).unwrap();
    plan::add_at(&source, item(&source, "Chips"), "2026-10-06", Some("lunch"), None, None).unwrap();
    plan::add_blank(&source, "2026-10-07", Some("snack"), Some("Bakery run"), None).unwrap();

    let json = backup::export_json(&source).unwrap();
    let target = test_conn();
    backup::import_json(&target, &json).unwrap();

    let a = plan::list(&source, "2026-10-01", "2026-10-31").unwrap();
    let b = plan::list(&target, "2026-10-01", "2026-10-31").unwrap();
    assert_eq!(format!("{a:?}"), format!("{b:?}"));
    let opts = |c: &Connection| count(c, "SELECT COUNT(*) FROM template_option");
    assert_eq!(opts(&source), opts(&target));
    assert_eq!(
        summary::summary(&source, "2026-10-05", "2026-10-11").unwrap().rows.len(),
        summary::summary(&target, "2026-10-05", "2026-10-11").unwrap().rows.len()
    );
}

#[test]
fn the_sample_wipe_takes_the_sample_templates_and_their_chips_but_keeps_yours() {
    let mut conn = test_conn();
    let mine = steamed_veg(&mut conn);
    let e = plan::add(&conn, mine.item_id.unwrap(), "2026-10-06", None).unwrap();
    plan::set_picks(&conn, e.id, &[pick(&conn, mine.lines[0].id, "Carrot")]).unwrap();
    plan::add(&conn, item(&conn, "Spag bol"), "2026-10-06", None).unwrap();

    let r = super::data::remove_sample(&conn).unwrap();
    assert_eq!((r.templates, r.items), (2, 10));
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM template"), 1);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM item"), 1, "just your chip");
    // Your planned night and its pick survive, and so do the options' ingredients.
    let plan = plan::list(&conn, "2026-10-06", "2026-10-06").unwrap();
    assert_eq!((plan.len(), plan[0].picks.len()), (1, 1));
    assert!(count(&conn, "SELECT COUNT(*) FROM ingredient WHERE name = 'Potato'") == 1);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM pragma_foreign_key_check"), 0);
}

#[test]
fn erase_all_handles_chips_and_picks() {
    let mut conn = test_conn();
    let t = steamed_veg(&mut conn);
    let e = plan::add(&conn, t.item_id.unwrap(), "2026-10-06", None).unwrap();
    plan::set_picks(&conn, e.id, &[pick(&conn, t.lines[0].id, "Carrot")]).unwrap();
    plan::add_blank(&conn, "2026-10-06", None, Some("x"), None).unwrap();
    super::data::erase_all(&conn).unwrap();
    for table in ["item", "template", "template_option", "plan_entry", "plan_entry_pick"] {
        assert_eq!(count(&conn, &format!("SELECT COUNT(*) FROM {table}")), 0, "{table}");
    }
    // The takeaway token comes back by itself.
    assert!(plan::add_blank(&conn, "2026-10-06", None, Some("x"), None).is_ok());
}

/// Run by hand against a copy of a real database:
/// `MEAL_DB=path/to/copy.db cargo test real_database -- --ignored --nocapture`
#[test]
#[ignore]
fn real_database_upgrades_cleanly() {
    let path = std::env::var("MEAL_DB").expect("set MEAL_DB to a COPY of meal_planner.db");
    let mut conn = Connection::open(&path).unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    let n = |c: &Connection, t: &str| count(c, &format!("SELECT COUNT(*) FROM {t}"));
    let before = (n(&conn, "item"), n(&conn, "plan_entry"), n(&conn, "ingredient"), n(&conn, "day_info"));
    db::migrate(&mut conn).unwrap();
    let after = (n(&conn, "item"), n(&conn, "plan_entry"), n(&conn, "ingredient"), n(&conn, "day_info"));
    let templates = n(&conn, "template");
    println!("before {before:?}  after {after:?}  templates {templates}");
    assert_eq!((before.1, before.2, before.3), (after.1, after.2, after.3));
    assert_eq!(after.0, before.0 + templates);
    // And a full export/import cycle of the upgraded data works.
    let json = backup::export_json(&conn).unwrap();
    backup::import_json(&test_conn(), &json).unwrap();
}

/// `BACKUP=path/to/backup.json cargo test real_backup -- --ignored --nocapture`
#[test]
#[ignore]
fn real_backup_imports_and_works() {
    let json = std::fs::read_to_string(std::env::var("BACKUP").expect("set BACKUP")).unwrap();
    let conn = test_conn();
    let s = backup::import_json(&conn, &json).unwrap();
    println!("imported {s:?}");
    let n = |t: &str| count(&conn, &format!("SELECT COUNT(*) FROM {t}"));
    println!("items {} (chips {}), templates {}, entries {}, categories {}", n("item"), count(&conn, "SELECT COUNT(*) FROM template WHERE item_id IS NOT NULL"), n("template"), n("plan_entry"), n("ingredient_category"));
    assert_eq!(n("template"), count(&conn, "SELECT COUNT(*) FROM template WHERE item_id IS NOT NULL"));
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM plan_entry WHERE slot != 'dinner'"), 0);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM pragma_foreign_key_check"), 0);
    let sum = summary::summary(&conn, "2000-01-01", "2100-01-01").unwrap();
    for r in &sum.rows {
        println!("TOTAL\t{}\t{}\t{}", r.ingredient_name, r.unit, r.quantity);
    }
    // Re-export and re-import is stable.
    let again = test_conn();
    backup::import_json(&again, &backup::export_json(&conn).unwrap()).unwrap();
    assert_eq!(
        format!("{:?}", plan::list(&conn, "2000-01-01", "2100-01-01").unwrap()),
        format!("{:?}", plan::list(&again, "2000-01-01", "2100-01-01").unwrap())
    );
}

// ---- dish categories (Beef, Chicken, Soups...) -----------------------------------------------

use super::dish_categories as cats;

fn cat(conn: &Connection, name: &str) -> i64 {
    conn.query_row("SELECT id FROM item_category WHERE name = ?1", params![name], |r| r.get(0)).unwrap()
}

fn dish_in(conn: &mut Connection, name: &str, category: Option<i64>) -> super::items::Item {
    let flour = ing(conn, "Butter");
    super::items::create(
        conn,
        &super::items::ItemInput {
            name: name.into(),
            item_type: "meal".into(),
            colour: None,
            notes: None,
            template_id: None,
            category_id: category,
            lines: vec![super::items::LineInput { ingredient_id: flour, quantity: 1.0 }],
        },
    )
    .unwrap()
}

#[test]
fn a_fresh_library_starts_with_the_usual_categories_and_nothing_is_filed() {
    let conn = test_conn();
    let names: Vec<String> = cats::list(&conn).unwrap().into_iter().map(|c| c.name).collect();
    assert_eq!(names, ["Beef", "Chicken", "Lamb", "Pork", "Seafood", "Vegetarian"]);
    assert!(super::items::list(&conn, None, None, true).unwrap().iter().all(|i| i.category_id.is_none()));
}

#[test]
fn a_dish_is_filed_by_choice_and_the_list_shows_the_category_name() {
    let mut conn = test_conn();
    let beef = cat(&conn, "Beef");
    let rissoles = dish_in(&mut conn, "Beef rissoles", Some(beef));
    let carbonara = dish_in(&mut conn, "Carbonara", None);
    assert_eq!(rissoles.category_id, Some(beef));
    let all = super::items::list(&conn, None, None, false).unwrap();
    let find = |id: i64| all.iter().find(|i| i.id == id).unwrap();
    assert_eq!(find(rissoles.id).category_name.as_deref(), Some("Beef"));
    assert_eq!(find(carbonara.id).category_id, None, "nothing is categorised automatically");
    assert_eq!(cats::list(&conn).unwrap().iter().find(|c| c.name == "Beef").unwrap().item_count, 1);

    // Duplicating keeps the category; editing can clear it.
    let copy = super::items::duplicate(&mut conn, rissoles.id).unwrap();
    assert_eq!(copy.category_id, Some(beef));
    let mut input = super::items::ItemInput {
        name: "Beef rissoles".into(),
        item_type: "meal".into(),
        colour: None,
        notes: None,
        template_id: None,
        category_id: None,
        lines: vec![],
    };
    input.lines.push(super::items::LineInput { ingredient_id: ing(&conn, "Butter"), quantity: 1.0 });
    assert_eq!(super::items::update(&mut conn, rissoles.id, &input).unwrap().category_id, None);
}

#[test]
fn categories_can_be_added_renamed_and_deleted_without_losing_dishes() {
    let mut conn = test_conn();
    let soups = cats::create(&conn, "  Soups ").unwrap();
    assert_eq!(soups.name, "Soups");
    assert!(cats::create(&conn, "soups").is_err(), "names are unique ignoring case");
    assert!(cats::create(&conn, "  ").is_err());
    let dish = dish_in(&mut conn, "Pumpkin soup", Some(soups.id));

    cats::rename(&conn, soups.id, "Soups & stews").unwrap();
    assert_eq!(super::items::list(&conn, None, None, false).unwrap().iter().find(|i| i.id == dish.id).unwrap().category_name.as_deref(), Some("Soups & stews"));
    assert!(cats::rename(&conn, soups.id, "beef").is_err());
    assert!(cats::rename(&conn, 9999, "x").is_err());

    cats::delete(&conn, soups.id).unwrap();
    assert_eq!(super::items::get(&conn, dish.id).unwrap().category_id, None, "dish stays, just uncategorised");
    assert!(cats::delete(&conn, soups.id).is_err());
}

#[test]
fn a_templates_chip_follows_its_category() {
    let mut conn = test_conn();
    let soups = cats::create(&conn, "Soups").unwrap();
    let t = steamed_veg(&mut conn);
    let input = TemplateInput {
        name: "Veg soup".into(),
        item_type: "meal".into(),
        colour: None,
        category_id: Some(soups.id),
        lines: vec![],
    };
    let t = templates::update(&mut conn, t.id, &input).unwrap();
    assert_eq!(t.category_id, Some(soups.id));
    assert_eq!(super::items::get(&conn, t.item_id.unwrap()).unwrap().category_id, Some(soups.id));
    cats::delete(&conn, soups.id).unwrap();
    assert_eq!(templates::get(&conn, t.id).unwrap().category_id, None);
}

#[test]
fn categories_and_filing_survive_a_backup_and_older_backups_get_the_starting_list() {
    let mut source = test_conn();
    let soups = cats::create(&source, "Soups").unwrap();
    cats::delete(&source, cat(&source, "Lamb")).unwrap();
    dish_in(&mut source, "Pumpkin soup", Some(soups.id));
    let json = backup::export_json(&source).unwrap();

    let target = test_conn();
    backup::import_json(&target, &json).unwrap();
    let names: Vec<String> = cats::list(&target).unwrap().into_iter().map(|c| c.name).collect();
    assert!(names.contains(&"Soups".to_string()) && !names.contains(&"Lamb".to_string()), "{names:?}");
    let dish = super::items::list(&target, None, Some("Pumpkin"), false).unwrap();
    assert_eq!(dish[0].category_name.as_deref(), Some("Soups"));

    // A backup from before categories existed has none of it: the starting list comes back.
    let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let o = v.as_object_mut().unwrap();
    o.remove("item_categories");
    for i in o["items"].as_array_mut().unwrap() {
        i.as_object_mut().unwrap().remove("category_id");
    }
    for t in o["templates"].as_array_mut().unwrap() {
        t.as_object_mut().unwrap().remove("category_id");
    }
    let older = test_conn();
    cats::delete(&older, cat(&older, "Beef")).unwrap();
    backup::import_json(&older, &v.to_string()).unwrap();
    assert_eq!(cats::list(&older).unwrap().len(), 6);
    assert!(super::items::list(&older, None, None, true).unwrap().iter().all(|i| i.category_id.is_none()));
    assert_eq!(count(&older, "SELECT COUNT(*) FROM pragma_foreign_key_check"), 0);
}

#[test]
fn erasing_everything_keeps_your_category_list() {
    let mut conn = test_conn();
    cats::create(&conn, "Soups").unwrap();
    let soups = cat(&conn, "Soups");
    dish_in(&mut conn, "Pumpkin soup", Some(soups));
    super::data::erase_all(&conn).unwrap();
    assert_eq!(cats::list(&conn).unwrap().len(), 7);
}
