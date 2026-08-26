// commands.rs — the Tauri #[command] handlers, matching the old preload.js
// / main.js IPC surface (load-config, choose-path, test-ftp, start-watch,
// stop-watch) so src/renderer.js (via src/tauri-bridge.js) needs no changes.
use crate::config::{self, FtpConfig};
use crate::{autostart, tray, uploader, watcher};
use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

#[derive(Serialize)]
pub struct ApiResult {
    ok: bool,
    message: String,
}

pub(crate) fn set_autostart(app: &AppHandle, enabled: bool) {
    let result = if enabled {
        autostart::enable()
    } else {
        autostart::disable()
    };
    match result {
        Ok(()) if enabled => watcher::send_log(
            app,
            "ok",
            "Will start automatically after shutdown or restart (while watching is on).",
        ),
        Ok(()) => {}
        Err(err) => watcher::send_log(
            app,
            "error",
            format!("Could not update auto-start: {}", err),
        ),
    }
}

pub(crate) fn resume_watch_if_needed(app: &AppHandle) {
    let Some(cfg) = config::load_config(app) else {
        return;
    };
    if !cfg.watching || cfg.watch_targets().is_empty() {
        return;
    }

    set_autostart(app, true);

    if try_resume(app, &cfg) {
        return;
    }

    watcher::send_log(
        app,
        "info",
        "Watch folder not ready yet after startup — retrying…",
    );
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        for secs in [3_u64, 8, 15, 30] {
            tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
            let Some(cfg) = config::load_config(&handle) else {
                return;
            };
            if !cfg.watching {
                return;
            }
            if watcher::is_watching(&handle) {
                return;
            }
            if try_resume(&handle, &cfg) {
                watcher::send_log(
                    &handle,
                    "ok",
                    "Watch mode resumed after system startup.",
                );
                return;
            }
        }
        watcher::send_log(
            &handle,
            "error",
            "Could not resume watching after startup. Open the window and click Start Watching.",
        );
        tray::update_tray(&handle, false);
        watcher::emit_watch_state(&handle, false);
    });
}

fn try_resume(app: &AppHandle, cfg: &FtpConfig) -> bool {
    if watcher::start_watching(app, cfg).is_ok() {
        tray::update_tray(app, true);
        watcher::emit_watch_state(app, true);
        true
    } else {
        false
    }
}

#[tauri::command]
pub fn load_config(app: AppHandle) -> Option<FtpConfig> {
    config::load_config(&app)
}

#[tauri::command]
pub async fn choose_path(app: AppHandle, kind: String) -> Option<String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let send = move |path: Option<tauri_plugin_dialog::FilePath>| {
        let _ = tx.send(
            path.and_then(|p| p.into_path().ok())
                .map(|p| p.to_string_lossy().into_owned()),
        );
    };
    match kind.as_str() {
        "folder" => {
            app.dialog().file().pick_folder(send);
        }
        // CSV / Excel file picker — the LGD FTP server only accepts these.
        _ => {
            app.dialog()
                .file()
                .add_filter("CSV & Excel", &["csv", "xls", "xlsx"])
                .pick_file(send);
        }
    }
    rx.await.ok().flatten()
}

#[tauri::command]
pub fn save_settings(app: AppHandle, config: FtpConfig) -> bool {
    let merged = config::merge_with_saved(&app, &config, None);
    config::save_config(&app, &merged);
    true
}

#[tauri::command]
pub async fn test_ftp(app: AppHandle, config: FtpConfig) -> ApiResult {
    let merged = config::merge_with_saved(&app, &config, None);
    match uploader::test_connection(merged.clone()).await {
        Ok(message) => {
            config::save_config(&app, &merged);
            ApiResult {
                ok: true,
                message: format!("{} — settings saved.", message),
            }
        }
        Err(err) => ApiResult { ok: false, message: err },
    }
}

#[tauri::command]
pub fn start_watch(app: AppHandle, config: FtpConfig) -> bool {
    let merged = config::merge_with_saved(&app, &config, Some(false));
    let started = watcher::start_watching(&app, &merged).is_ok();
    let mut to_save = merged;
    to_save.watching = started;
    config::save_config(&app, &to_save);
    set_autostart(&app, started);
    tray::update_tray(&app, started);
    watcher::emit_watch_state(&app, started);
    started
}

#[tauri::command]
pub fn stop_watch(app: AppHandle) -> bool {
    persist_stop_watch(&app);
    true
}

// Shared by the Stop button (stop_watch command) and the tray's "Stop
// Watching" menu item.
pub fn persist_stop_watch(app: &AppHandle) {
    watcher::stop_watching(app);
    if let Some(mut cfg) = config::load_config(app) {
        cfg.watching = false;
        config::save_config(app, &cfg);
    }
    set_autostart(app, false);
    tray::update_tray(app, false);
    watcher::emit_watch_state(app, false);
}
