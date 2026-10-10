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
            app.manage(commands::undo::UndoStack::default());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::categories::categories_list,
            commands::categories::categories_reorder,
            commands::backup::backup_export,
            commands::backup::backup_import,
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
            commands::templates::templates_list,
            commands::templates::templates_get,
            commands::templates::templates_create,
            commands::templates::templates_update,
            commands::templates::templates_delete,
            commands::templates::templates_instantiate,
            commands::usage::items_usage,
            commands::usage::items_history,
            commands::plan::plan_list,
            commands::plan::plan_add,
            commands::plan::plan_add_blank,
            commands::plan::plan_set_picks,
            commands::plan::plan_move,
            commands::plan::plan_update,
            commands::plan::plan_copy_range,
            commands::plan::plan_remove,
            commands::undo::plan_undo,
            commands::undo::plan_undo_peek,
            commands::dayinfo::day_info_list,
            commands::dayinfo::day_info_set,
            commands::dayinfo::day_leftovers_move,
            commands::data::data_sample_counts,
            commands::data::data_remove_sample,
            commands::data::data_erase_all,
            commands::settings::settings_get,
            commands::settings::settings_set,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
