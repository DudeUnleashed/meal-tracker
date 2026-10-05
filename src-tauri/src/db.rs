use std::fs;
use std::sync::Mutex;

use rusqlite::Connection;
use rusqlite_migration::{Migrations, M};
use tauri::{AppHandle, Manager};

pub struct Db(pub Mutex<Connection>);

fn migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(include_str!("../migrations/001_init.sql")),
        M::up(include_str!("../migrations/002_seed.sql")),
    ])
}

/// Applies all migrations (schema + seed) to a connection.
pub fn migrate(conn: &mut Connection) -> Result<(), rusqlite_migration::Error> {
    migrations().to_latest(conn)
}

/// Opens (creating if needed) the database in the app data dir and migrates it.
pub fn init(app: &AppHandle) -> Result<Db, Box<dyn std::error::Error>> {
    let dir = app.path().app_data_dir()?;
    fs::create_dir_all(&dir)?;
    let mut conn = Connection::open(dir.join("meal_planner.db"))?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrate(&mut conn)?;
    Ok(Db(Mutex::new(conn)))
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
        assert_eq!(count("SELECT COUNT(*) FROM ingredient_category"), 7);
        assert_eq!(count("SELECT COUNT(*) FROM ingredient"), 34);
        assert_eq!(count("SELECT COUNT(*) FROM item"), 8);
        assert_eq!(count("SELECT COUNT(*) FROM item_ingredient"), 21);
        assert_eq!(count("SELECT COUNT(*) FROM template"), 2);
        assert_eq!(count("SELECT COUNT(*) FROM template_line"), 9);
        // Every seed join resolved: no item lacks ingredients.
        assert_eq!(
            count("SELECT COUNT(*) FROM item WHERE id NOT IN (SELECT item_id FROM item_ingredient)"),
            0
        );
    }
}
