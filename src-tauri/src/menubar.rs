//! The menu bar companion: a template icon in the menu bar and a borderless popover
//! window ("menubar") that opens under it and hides when it loses focus.

use tauri::image::Image;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, PhysicalPosition, Rect, Runtime, WindowEvent};

pub const WINDOW: &str = "menubar";

pub fn install<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let icon = Image::from_bytes(include_bytes!("../icons/tray-template.png"))?;
    TrayIconBuilder::with_id("pencleaner")
        .icon(icon)
        .icon_as_template(true)
        .tooltip("PenCleaner")
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, rect, .. } = event {
                toggle(tray.app_handle(), rect);
            }
        })
        .build(app)?;

    if let Some(win) = app.get_webview_window(WINDOW) {
        let w = win.clone();
        win.on_window_event(move |e| {
            // A popover closes as soon as you click anywhere else.
            if let WindowEvent::Focused(false) = e {
                let _ = w.hide();
            }
        });
    }
    Ok(())
}

fn toggle<R: Runtime>(app: &AppHandle<R>, icon: Rect) {
    let Some(win) = app.get_webview_window(WINDOW) else { return };
    if win.is_visible().unwrap_or(false) {
        let _ = win.hide();
        return;
    }
    // Centre the popover under the icon, kept on screen.
    let scale = win.scale_factor().unwrap_or(1.0);
    let pos = icon.position.to_physical::<f64>(scale);
    let size = icon.size.to_physical::<f64>(scale);
    let width = win.outer_size().map(|s| s.width as f64).unwrap_or(360.0 * scale);
    let mut x = pos.x + size.width / 2.0 - width / 2.0;
    if let Ok(Some(m)) = win.current_monitor().or_else(|_| win.primary_monitor()) {
        let right = m.position().x as f64 + m.size().width as f64;
        x = x.min(right - width - 8.0 * scale).max(m.position().x as f64 + 8.0 * scale);
    }
    let y = pos.y + size.height + 4.0 * scale;
    let _ = win.set_position(PhysicalPosition::new(x, y));
    let _ = win.show();
    let _ = win.set_focus();
    let _ = tauri::Emitter::emit_to(&win, WINDOW, "menubar-shown", ());
}

pub fn show_main<R: Runtime>(app: &AppHandle<R>) {
    if let Some(m) = app.get_webview_window("main") {
        let _ = m.show();
        let _ = m.unminimize();
        let _ = m.set_focus();
    }
}
