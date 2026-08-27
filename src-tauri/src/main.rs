#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod autostart;
mod commands;
mod config;
mod tray;
mod uploader;
mod watcher;

use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Manager;
use tauri::webview::PageLoadEvent;

/// Set when the user closes the window (hide to tray). Windows otherwise
/// can fire ExitRequested at startup while the window is still hidden,
/// which used to leave the app running with no UI.
static USER_HID_WINDOW: AtomicBool = AtomicBool::new(false);

pub(crate) fn log_startup(msg: &str) {
    let line = format!(
        "[{}] {}\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        msg
    );
    let dir = {
        #[cfg(windows)]
        {
            std::env::var_os("APPDATA").map(|p| {
                std::path::PathBuf::from(p).join("com.diamonds.ftpuploader")
            })
        }
        #[cfg(not(windows))]
        {
            std::env::var_os("HOME").map(|p| {
                std::path::PathBuf::from(p).join("Library/Logs/com.diamonds.ftpuploader")
            })
        }
    };
    if let Some(dir) = dir {
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("startup.log"))
        {
            use std::io::Write;
            let _ = f.write_all(line.as_bytes());
        }
    }
}

#[cfg(windows)]
fn win_error_box(msg: &str) {
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::null_mut;
    #[link(name = "user32")]
    extern "system" {
        fn MessageBoxW(
            hwnd: *mut core::ffi::c_void,
            text: *const u16,
            caption: *const u16,
            ty: u32,
        ) -> i32;
    }
    fn wide(s: &str) -> Vec<u16> {
        std::ffi::OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }
    let text = wide(msg);
    let caption = wide("LGD FTP DATA SYNC");
    unsafe {
        MessageBoxW(null_mut(), text.as_ptr(), caption.as_ptr(), 0x10);
    }
}

// macOS silently kills idle/backgrounded "foreground" apps via Automatic
// Termination + Sudden Termination once their window is occluded/unfocused
// for a while — no crash, no log, it just vanishes. A background tray app
// like this one must opt out or it can be reaped mid-session with no user
// interaction at all.
#[cfg(target_os = "macos")]
fn disable_macos_auto_termination() {
    use objc2_foundation::{NSProcessInfo, NSString};
    let info = NSProcessInfo::processInfo();
    let reason = NSString::from_str("Watches folders and uploads in the background");
    info.disableAutomaticTermination(&reason);
    info.disableSuddenTermination();
}

fn main() {
    #[cfg(target_os = "macos")]
    disable_macos_auto_termination();
    log_startup("process start");
    std::panic::set_hook(Box::new(|info| {
        let msg = format!("The app crashed:\n{info}");
        log_startup(&msg);
        #[cfg(windows)]
        win_error_box(&msg);
    }));

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            USER_HID_WINDOW.store(false, Ordering::SeqCst);
            tray::show_main_window(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .manage(watcher::WatchState::default())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                USER_HID_WINDOW.store(true, Ordering::SeqCst);
                tray::hide_to_tray(window.app_handle());
            }
        })
        .on_page_load(|webview, payload| {
            if payload.event() == PageLoadEvent::Finished {
                let app = webview.app_handle();
                watcher::emit_watch_state(app, watcher::is_watching(app));
            }
        })
        .setup(|app| {
            let handle = app.handle().clone();
            if let Err(err) = tray::build_tray(&handle) {
                log_startup(&format!("tray icon failed (window will still open): {err}"));
            }

            let start_hidden = std::env::args().any(|a| a == "--hidden");
            if start_hidden {
                USER_HID_WINDOW.store(true, Ordering::SeqCst);
                tray::hide_to_tray(&handle);
                log_startup("started with --hidden (tray)");
            } else {
                USER_HID_WINDOW.store(false, Ordering::SeqCst);
                tray::show_main_window(&handle);
                log_startup("showing main window");
                // Windows can emit a fake "exit" at startup and leave the
                // process running with no window. If that happens, bring it
                // back; if it still is not visible, tell the user why.
                let watchdog = handle.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    if USER_HID_WINDOW.load(Ordering::SeqCst) {
                        return;
                    }
                    let visible = watchdog
                        .get_webview_window("main")
                        .and_then(|w| w.is_visible().ok())
                        .unwrap_or(false);
                    if !visible {
                        log_startup("watchdog: window was not visible, forcing show");
                        tray::show_main_window(&watchdog);
                        #[cfg(windows)]
                        win_error_box(
                            "The app started but the window was hidden.\n\nClick OK, then look at the taskbar or system tray (near the clock).\n\nIf it still does not appear, install Microsoft Edge WebView2 Runtime and try again.",
                        );
                    } else {
                        log_startup("watchdog: window is visible");
                    }
                });
            }

            commands::resume_watch_if_needed(&handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::load_config,
            commands::choose_path,
            commands::save_settings,
            commands::test_ftp,
            commands::start_watch,
            commands::stop_watch,
        ])
        .build(tauri::generate_context!());

    let app = match app {
        Ok(app) => app,
        Err(err) => {
            let msg = format!(
                "Could not start LGD FTP DATA SYNC.\n\n{err}\n\nIf this is a new PC, install Microsoft Edge WebView2 Runtime, then try again."
            );
            log_startup(&msg);
            #[cfg(windows)]
            win_error_box(&msg);
            std::process::exit(1);
        }
    };

    app.run(|app, event| match event {
        tauri::RunEvent::ExitRequested { api, code, .. } => {
            // Only stay in the tray after the user actually closed the window.
            // Preventing every Windows "exit" event left a hidden process that
            // blocked later Start-menu clicks (single-instance).
            if code.is_none() && USER_HID_WINDOW.load(Ordering::SeqCst) {
                api.prevent_exit();
                tray::hide_to_tray(app);
            }
        }
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Reopen {
            has_visible_windows,
            ..
        } => {
            if !has_visible_windows {
                USER_HID_WINDOW.store(false, Ordering::SeqCst);
                tray::show_main_window(app);
            }
        }
        _ => {}
    });
}
