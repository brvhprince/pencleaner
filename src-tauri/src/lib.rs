mod care;
mod commands;
mod login;
#[cfg(target_os = "macos")]
mod menu;
mod menubar;
mod notify;
#[cfg(target_os = "macos")]
mod trash_watch;

use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let background = std::env::args().any(|a| a == login::BACKGROUND_FLAG);
    // Open at Login's agent starting while PenCleaner is already open: nothing to do.
    if !login::claim_single_instance() && background {
        return;
    }
    pencleaner_core::fsutil::set_observer_io_policy();
    // An enabled watcher must call this copy of PenCleaner, wherever it now lives.
    #[cfg(target_os = "macos")]
    std::thread::spawn(|| {
        if let Ok(exe) = std::env::current_exe() {
            let _ = pencleaner_core::protection::watch::repoint(&exe);
        }
    });
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .setup(move |app| {
            menubar::install(app.handle())?;
            // Started at login: menu bar only, no window and no Dock icon until it's opened.
            if background {
                #[cfg(target_os = "macos")]
                let _ = app.handle().set_activation_policy(tauri::ActivationPolicy::Accessory);
            } else {
                menubar::show_main(app.handle());
            }
            #[cfg(target_os = "macos")]
            trash_watch::start(app.handle().clone());
            care::start(app.handle().clone());
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
        });
    // The macOS app menu (About, Settings…, Hide, Quit). Windows apps don't have one.
    #[cfg(target_os = "macos")]
    let builder = builder.menu(menu::build).on_menu_event(|app, event| {
        // About and Settings are part of the UI; the window shows them.
        if event.id() == menu::ABOUT {
            let _ = tauri::Emitter::emit(app, "open-about", ());
        } else if event.id() == menu::SETTINGS {
            let _ = tauri::Emitter::emit(app, "open-settings", ());
        }
    });
    builder
        .invoke_handler(tauri::generate_handler![
            commands::system_info,
            commands::about_info,
            commands::volumes_list,
            #[cfg(target_os = "macos")]
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
            #[cfg(target_os = "macos")]
            commands::apps_scan,
            #[cfg(target_os = "macos")]
            commands::apps_remove,
            #[cfg(target_os = "macos")]
            commands::app_icon,
            #[cfg(target_os = "macos")]
            commands::perf_scan,
            #[cfg(target_os = "macos")]
            commands::perf_memory,
            #[cfg(target_os = "macos")]
            commands::perf_run_tasks,
            #[cfg(target_os = "macos")]
            commands::perf_full_items,
            #[cfg(target_os = "macos")]
            commands::perf_set_item,
            #[cfg(target_os = "macos")]
            commands::perf_remove_item,
            #[cfg(target_os = "macos")]
            commands::perf_delete_snapshots,
            #[cfg(target_os = "macos")]
            commands::open_login_items_settings,
            commands::activity_list,
            commands::activity_clear,
            #[cfg(target_os = "macos")]
            commands::updates_scan,
            #[cfg(target_os = "macos")]
            commands::updates_install,
            #[cfg(target_os = "macos")]
            commands::open_app_store_updates,
            #[cfg(target_os = "macos")]
            commands::protection_scan,
            #[cfg(target_os = "macos")]
            commands::protection_cancel,
            #[cfg(target_os = "macos")]
            commands::protection_remove,
            #[cfg(target_os = "macos")]
            commands::privacy_clean,
            #[cfg(target_os = "macos")]
            commands::permission_revoke,
            #[cfg(target_os = "macos")]
            commands::open_privacy_settings,
            #[cfg(target_os = "macos")]
            commands::watcher_set,
            #[cfg(target_os = "macos")]
            commands::watcher_clear_alerts,
            #[cfg(target_os = "macos")]
            commands::watcher_status,
            commands::settings_get,
            commands::settings_save,
            commands::ignore_add,
            commands::ignore_remove,
            #[cfg(target_os = "macos")]
            commands::vitals,
            commands::show_main,
            #[cfg(target_os = "macos")]
            commands::empty_trash,
            #[cfg(target_os = "macos")]
            commands::trashed_app_remove,
            #[cfg(target_os = "macos")]
            commands::trashed_app_dismiss,
            commands::login_item_status,
            commands::care_status,
            commands::care_done,
            commands::login_item_set,
            commands::open_login_items_for_app,
        ])
        .build(tauri::generate_context!())
        .expect("error while building PenCleaner")
        .run(|_app, _event| {
            // Clicking the Dock icon with the window closed brings it back.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = _event {
                menubar::show_main(_app);
            }
        });
}
