// tray.rs — system tray icon + context menu, mirroring updateTray() in the
// old main.js: a disabled status label, Open / Stop Watching / Quit.
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

pub struct TrayHandles {
    pub status_item: MenuItem<tauri::Wry>,
    pub stop_item: MenuItem<tauri::Wry>,
}

pub fn show_main_window(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    {
        let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
        let _ = app.set_dock_visibility(true);
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_skip_taskbar(false);
        let _ = window.unminimize();
        if let Err(err) = window.show() {
            crate::log_startup(&format!("show() failed: {err}"));
        }
        if let Err(err) = window.set_focus() {
            crate::log_startup(&format!("set_focus() failed: {err}"));
        }
    } else {
        crate::log_startup("show_main_window: main window missing");
    }
}

pub fn hide_to_tray(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
        let _ = window.set_skip_taskbar(true);
    }
    #[cfg(target_os = "macos")]
    {
        let _ = app.set_dock_visibility(false);
        let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
    }
}

pub fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let status_item = MenuItem::with_id(app, "status", "⚪ Not watching", false, None::<&str>)?;
    let open_item = MenuItem::with_id(app, "open", "Open LGD FTP DATA SYNC", true, None::<&str>)?;
    let stop_item = MenuItem::with_id(app, "stop", "Stop Watching", false, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[
            &status_item,
            &sep1,
            &open_item,
            &stop_item,
            &sep2,
            &quit_item,
        ],
    )?;

    let icon_bytes: &[u8] = {
        #[cfg(target_os = "macos")]
        {
            include_bytes!("../../assets/trayTemplate.png")
        }
        #[cfg(target_os = "windows")]
        {
            include_bytes!("../icons/icon.ico")
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            include_bytes!("../../assets/tray.png")
        }
    };
    let icon = tauri::image::Image::from_bytes(icon_bytes)?;

    TrayIconBuilder::with_id("tray")
        .icon(icon)
        .icon_as_template(cfg!(target_os = "macos"))
        .menu(&menu)
        // macOS: left-click opens the menu. Windows/Linux: left-click reopens
        // the window; the menu stays on right-click.
        .show_menu_on_left_click(cfg!(target_os = "macos"))
        .tooltip("LGD FTP DATA SYNC — not watching")
        .on_tray_icon_event(|tray, event| {
            if !cfg!(target_os = "macos") {
                match event {
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    }
                    | TrayIconEvent::DoubleClick {
                        button: MouseButton::Left,
                        ..
                    } => show_main_window(tray.app_handle()),
                    _ => {}
                }
            }
        })
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main_window(app),
            "stop" => crate::commands::persist_stop_watch(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    app.manage(TrayHandles {
        status_item,
        stop_item,
    });
    Ok(())
}

pub fn update_tray(app: &AppHandle, watching: bool) {
    if let Some(handles) = app.try_state::<TrayHandles>() {
        let _ = handles
            .status_item
            .set_text(if watching { "🟢 Watching" } else { "⚪ Not watching" });
        let _ = handles.stop_item.set_enabled(watching);
    }
    if let Some(tray) = app.tray_by_id("tray") {
        let tip = if watching {
            "LGD FTP DATA SYNC — watching"
        } else {
            "LGD FTP DATA SYNC — not watching"
        };
        let _ = tray.set_tooltip(Some(tip));
    }
}
