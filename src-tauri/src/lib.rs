mod commands;

use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    pencleaner_core::fsutil::set_observer_io_policy();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::system_info,
            commands::volumes_list,
            commands::open_full_disk_access_settings,
            commands::reveal_in_finder,
            commands::space_lens_scan,
            commands::space_lens_cancel,
            commands::space_lens_list,
            commands::space_lens_discard,
            commands::remove_items,
            commands::cleanup_scan,
            commands::cleanup_cancel,
            commands::cleanup_clean,
            commands::clutter_scan,
            commands::clutter_cancel,
            commands::clutter_remove,
        ])
        .run(tauri::generate_context!())
        .expect("error while running PenCleaner");
}
