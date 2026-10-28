//! The menu bar companion: an icon in the menu bar (macOS) or the taskbar corner
//! (Windows) and a borderless popover window ("menubar") that opens beside it and hides
//! when it loses focus.

use tauri::image::Image;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, LogicalSize, Manager, PhysicalPosition, Rect, Runtime, WindowEvent};

/// When the popover last hid itself on losing focus (ms since the epoch).
static HIDDEN_AT: AtomicU64 = AtomicU64::new(0);

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

pub const WINDOW: &str = "menubar";
const TRAY: &str = "pencleaner";
/// Full popover, and the compact card used for "you trashed an app".
pub const FULL_HEIGHT: f64 = 590.0;
#[cfg(target_os = "macos")]
pub const COMPACT_HEIGHT: f64 = 236.0;

pub fn install<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    // macOS tints a black template glyph to suit the menu bar; Windows shows the colour
    // app icon, as tray icons there do.
    #[cfg(target_os = "macos")]
    let icon = Image::from_bytes(include_bytes!("../icons/tray-template.png"))?;
    #[cfg(not(target_os = "macos"))]
    let icon = match app.default_window_icon() {
        Some(i) => i.clone(),
        None => Image::from_bytes(include_bytes!("../icons/32x32.png"))?,
    };
    TrayIconBuilder::with_id(TRAY)
        .icon(icon)
        .icon_as_template(cfg!(target_os = "macos"))
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
                HIDDEN_AT.store(now_ms(), Ordering::Relaxed);
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
    // Clicking the icon to close the popover first takes focus from it (it hides itself),
    // then arrives here: that click meant "close", not "open again".
    if now_ms().saturating_sub(HIDDEN_AT.load(Ordering::Relaxed)) < 300 {
        return;
    }
    let _ = win.set_size(LogicalSize::new(360.0, FULL_HEIGHT));
    place_and_show(&win, icon);
    let _ = tauri::Emitter::emit_to(&win, WINDOW, "menubar-shown", ());
}

/// Opens the popover under the menu bar icon at `height`, without a click.
#[cfg(target_os = "macos")]
pub fn pop_up<R: Runtime>(app: &AppHandle<R>, height: f64) -> Option<()> {
    let win = app.get_webview_window(WINDOW)?;
    let icon = app.tray_by_id(TRAY)?.rect().ok()??;
    let _ = win.set_size(LogicalSize::new(360.0, height));
    place_and_show(&win, icon);
    Some(())
}

fn place_and_show<R: Runtime>(win: &tauri::WebviewWindow<R>, icon: Rect) {
    // Centre the popover on the icon, kept on screen: below it for a menu bar at the top,
    // above it for a taskbar at the bottom (Windows' default).
    let scale = win.scale_factor().unwrap_or(1.0);
    let pos = icon.position.to_physical::<f64>(scale);
    let size = icon.size.to_physical::<f64>(scale);
    let (width, height) = win.outer_size().map_or((360.0 * scale, FULL_HEIGHT * scale), |s| (s.width as f64, s.height as f64));
    let mut x = pos.x + size.width / 2.0 - width / 2.0;
    let mut y = pos.y + size.height + 4.0 * scale;
    if let Ok(Some(m)) = win.current_monitor().or_else(|_| win.primary_monitor()) {
        let (left, top) = (m.position().x as f64, m.position().y as f64);
        let right = left + m.size().width as f64;
        x = x.min(right - width - 8.0 * scale).max(left + 8.0 * scale);
        if pos.y > top + m.size().height as f64 / 2.0 {
            y = pos.y - height - 8.0 * scale;
        }
    }
    let _ = win.set_position(PhysicalPosition::new(x, y));
    let _ = win.show();
    let _ = win.set_focus();
}

pub fn show_main<R: Runtime>(app: &AppHandle<R>) {
    // Back in the Dock if it started at login as menu bar only.
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
    if let Some(m) = app.get_webview_window("main") {
        let _ = m.show();
        let _ = m.unminimize();
        let _ = m.set_focus();
    }
}
