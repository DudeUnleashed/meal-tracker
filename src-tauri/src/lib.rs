mod commands;
mod db;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let db = db::init(app.handle())?;
            app.manage(db);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::categories::categories_list,
            commands::ingredients::ingredients_list,
            commands::ingredients::ingredients_create,
            commands::ingredients::ingredients_update,
            commands::ingredients::ingredients_archive,
            commands::items::items_list,
            commands::items::items_get,
            commands::items::items_create,
            commands::items::items_update,
            commands::items::items_duplicate,
            commands::items::items_archive,
            commands::summary::summary_get,
            commands::plan::plan_list,
            commands::plan::plan_add,
            commands::plan::plan_remove,
            commands::settings::settings_get,
            commands::settings::settings_set,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
