mod commands;
mod menu;
mod menubar;

use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    pencleaner_core::fsutil::set_observer_io_policy();
    // An enabled watcher must call this copy of PenCleaner, wherever it now lives.
    std::thread::spawn(|| {
        if let Ok(exe) = std::env::current_exe() {
            let _ = pencleaner_core::protection::watch::repoint(&exe);
        }
    });
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .setup(|app| {
            menubar::install(app.handle())?;
            Ok(())
        })
        .on_window_event(|win, event| {
            // Closing the main window keeps PenCleaner in the menu bar; the Dock icon
            // or the popover's "Open PenCleaner" brings it back.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if win.label() == "main" {
                    api.prevent_close();
                    let _ = win.hide();
                }
            }
        })
        .menu(menu::build)
        .on_menu_event(|app, event| {
            // About and Settings are part of the UI; the window shows them.
            if event.id() == menu::ABOUT {
                let _ = tauri::Emitter::emit(app, "open-about", ());
            } else if event.id() == menu::SETTINGS {
                let _ = tauri::Emitter::emit(app, "open-settings", ());
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::system_info,
            commands::about_info,
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
            commands::apps_scan,
            commands::apps_remove,
            commands::app_icon,
            commands::perf_scan,
            commands::perf_memory,
            commands::perf_run_tasks,
            commands::perf_full_items,
            commands::perf_set_item,
            commands::perf_remove_item,
            commands::perf_delete_snapshots,
            commands::open_login_items_settings,
            commands::activity_list,
            commands::activity_clear,
            commands::updates_scan,
            commands::updates_install,
            commands::open_app_store_updates,
            commands::protection_scan,
            commands::protection_cancel,
            commands::protection_remove,
            commands::privacy_clean,
            commands::permission_revoke,
            commands::open_privacy_settings,
            commands::watcher_set,
            commands::watcher_clear_alerts,
            commands::watcher_status,
            commands::settings_get,
            commands::settings_save,
            commands::ignore_add,
            commands::ignore_remove,
            commands::vitals,
            commands::show_main,
            commands::empty_trash,
        ])
        .build(tauri::generate_context!())
        .expect("error while building PenCleaner")
        .run(|app, event| {
            if let tauri::RunEvent::Reopen { .. } = event {
                menubar::show_main(app);
            }
        });
}
