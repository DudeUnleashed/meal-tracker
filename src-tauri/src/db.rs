use std::fs;
use std::sync::Mutex;

use rusqlite::Connection;
use rusqlite_migration::{Migrations, M};
use tauri::{AppHandle, Manager};

pub struct Db(pub Mutex<Connection>);

const LATEST_VERSION: usize = 4;

pub(crate) fn migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(include_str!("../migrations/001_init.sql")),
        M::up(include_str!("../migrations/002_seed.sql")),
        M::up(include_str!("../migrations/003_day_info_scale_sample.sql")),
        M::up(include_str!("../migrations/004_slots_live_templates.sql")),
    ])
}

/// Applies all migrations (schema + seed) to a connection, then makes sure every template has
/// its chip. Foreign keys are off while migrating: 004 rebuilds the `item` table, and a pragma
/// can't be changed inside the transaction the migrations run in. They are checked afterwards.
pub fn migrate(conn: &mut Connection) -> Result<(), Box<dyn std::error::Error>> {
    conn.pragma_update(None, "foreign_keys", "OFF")?;
    let result = migrations().to_latest(conn);
    conn.pragma_update(None, "foreign_keys", "ON")?;
    result?;
    let dangling: i64 = conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r.get(0))?;
    if dangling > 0 {
        return Err(format!("Migration left {dangling} rows pointing at missing data").into());
    }
    crate::commands::templates::ensure_items(conn)?;
    Ok(())
}

/// Opens (creating if needed) the database in the app data dir and migrates it.
pub fn init(app: &AppHandle) -> Result<Db, Box<dyn std::error::Error>> {
    let dir = app.path().app_data_dir()?;
    fs::create_dir_all(&dir)?;
    let path = dir.join("meal_planner.db");
    keep_copy_before_upgrade(&path)?;
    let mut conn = Connection::open(&path)?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrate(&mut conn)?;
    Ok(Db(Mutex::new(conn)))
}

/// An existing database that is about to be upgraded is copied first, so a bad migration can
/// never cost anyone their library. Kept as `meal_planner.before-vN.db` next to the original.
fn keep_copy_before_upgrade(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    if !path.exists() {
        return Ok(());
    }
    let version: usize = {
        let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        conn.query_row("PRAGMA user_version", [], |r| r.get(0))?
    };
    if version > 0 && version < LATEST_VERSION {
        let copy = path.with_file_name(format!("meal_planner.before-v{LATEST_VERSION}.db"));
        if !copy.exists() {
            fs::copy(path, copy)?;
        }
    }
    Ok(())
}

/// Fresh in-memory database with schema and seed data, for unit tests.
#[cfg(test)]
pub fn test_conn() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    migrate(&mut conn).unwrap();
    conn
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_are_valid_and_seed_loads() {
        migrations().validate().unwrap();

        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        migrate(&mut conn).unwrap();

        let count = |sql: &str| -> i64 { conn.query_row(sql, [], |r| r.get(0)).unwrap() };
        assert_eq!(count("SELECT COUNT(*) FROM ingredient_category"), 9);
        assert_eq!(count("SELECT COUNT(*) FROM ingredient"), 34);
        // 8 sample meals plus the chip each sample template gets.
        assert_eq!(count("SELECT COUNT(*) FROM item"), 10);
        assert_eq!(count("SELECT COUNT(*) FROM item_ingredient"), 25);
        assert_eq!(count("SELECT COUNT(*) FROM template"), 2);
        assert_eq!(count("SELECT COUNT(*) FROM template_line"), 9);
        // Every seed join resolved: no item lacks ingredients.
        assert_eq!(
            count("SELECT COUNT(*) FROM item WHERE id NOT IN (SELECT item_id FROM item_ingredient)"),
            0
        );
    }
}
