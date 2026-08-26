// watcher.rs — Rust port of the chokidar-based watch+queue logic in the old
// main.js: watches a file or folder, skips hidden dot-files, waits for a
// file to stop changing (debounce) before uploading, and uploads one file
// at a time in a queue.
use crate::config::FtpConfig;
use crate::uploader;
use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind, Debouncer};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc;

const FILE_SETTLE: Duration = Duration::from_millis(1500);
const UPLOAD_DELAY: Duration = Duration::from_secs(30);

#[derive(Clone, Serialize)]
struct LogEntry {
    #[serde(rename = "type")]
    kind: String,
    message: String,
    time: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    percent: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    key: Option<String>,
}

fn now() -> String {
    chrono::Local::now().format("%I:%M:%S %p").to_string()
}

pub(crate) fn send_log(app: &AppHandle, kind: &str, message: impl Into<String>) {
    send_log_full(app, kind, message, None, None);
}

fn send_log_full(
    app: &AppHandle,
    kind: &str,
    message: impl Into<String>,
    percent: Option<u8>,
    key: Option<String>,
) {
    let _ = app.emit(
        "log",
        LogEntry {
            kind: kind.to_string(),
            message: message.into(),
            time: now(),
            percent,
            key,
        },
    );
}

fn send_watch_state(app: &AppHandle, watching: bool) {
    let _ = app.emit("watch-state", watching);
}

pub(crate) fn is_hidden(relative: &Path) -> bool {
    relative
        .components()
        .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
}

#[cfg(test)]
pub(crate) fn collect_existing_files_for_test(
    base_dir: &Path,
    dir: &Path,
    out: &mut Vec<PathBuf>,
) {
    collect_existing_files(base_dir, dir, out);
}

fn collect_existing_files(base_dir: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let relative = path.strip_prefix(base_dir).unwrap_or(&path);
        if is_hidden(relative) {
            continue;
        }
        if path.is_dir() {
            collect_existing_files(base_dir, &path, out);
        } else if path.is_file() {
            out.push(path);
        }
    }
}

pub struct WatchHandle {
    debouncers: Vec<Debouncer<notify::RecommendedWatcher>>,
    watching: bool,
    pending: PendingMap,
}

pub struct WatchState(pub Mutex<WatchHandle>);

type QueueItem = (PathBuf, String, String);
type PendingMap = Arc<Mutex<HashMap<String, tauri::async_runtime::JoinHandle<()>>>>;

impl Default for WatchHandle {
    fn default() -> Self {
        Self {
            debouncers: Vec::new(),
            watching: false,
            pending: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for WatchState {
    fn default() -> Self {
        WatchState(Mutex::new(WatchHandle::default()))
    }
}

async fn process_queue(
    app: AppHandle,
    config: FtpConfig,
    mut rx: mpsc::UnboundedReceiver<QueueItem>,
) {
    while let Some((local_file, relative_path, product_type)) = rx.recv().await {
        let size = std::fs::metadata(&local_file)
            .map(|m| m.len())
            .unwrap_or(0);
        let key = format!("{}::{relative_path}", product_type);
        send_log_full(
            &app,
            "progress",
            format!(
                "Uploading [{}] {} — 0% ({})",
                product_type,
                relative_path,
                uploader::format_bytes(size)
            ),
            Some(0),
            Some(key.clone()),
        );
        let started = Instant::now();
        let app_p = app.clone();
        let key_p = key.clone();
        let rel_p = relative_path.clone();
        let pt_p = product_type.clone();
        let last = std::sync::atomic::AtomicU8::new(255);
        let on_progress: uploader::ProgressFn = Arc::new(move |sent, total| {
            let pct = if total == 0 {
                100
            } else {
                ((sent.saturating_mul(100)) / total).min(100) as u8
            };
            if last.swap(pct, std::sync::atomic::Ordering::Relaxed) == pct {
                return;
            }
            send_log_full(
                &app_p,
                "progress",
                format!(
                    "Uploading [{}] {} — {}% ({}/{})",
                    pt_p,
                    rel_p,
                    pct,
                    uploader::format_bytes(sent),
                    uploader::format_bytes(total)
                ),
                Some(pct),
                Some(key_p.clone()),
            );
        });
        match uploader::upload_file_as(
            config.clone(),
            local_file,
            relative_path.clone(),
            &product_type,
            Some(on_progress),
        )
        .await
        {
            Ok(remote) => {
                let secs = started.elapsed().as_secs_f32();
                send_log_full(
                    &app,
                    "ok",
                    format!(
                        "Uploaded [{}] {} → {} ({} — 100% in {:.1}s)",
                        product_type,
                        relative_path,
                        remote,
                        uploader::format_bytes(size),
                        secs
                    ),
                    Some(100),
                    Some(key),
                );
            }
            Err(err) => send_log_full(
                &app,
                "error",
                format!("Upload failed for {}: {}", relative_path, err),
                None,
                Some(key),
            ),
        }
    }
}

fn enqueue_existing(
    target: &Path,
    product_type: &str,
    tx: &mpsc::UnboundedSender<QueueItem>,
) {
    let is_dir = target.is_dir();
    let base_dir = if is_dir {
        target.to_path_buf()
    } else {
        target.parent().map(Path::to_path_buf).unwrap_or_default()
    };
    let mut existing = Vec::new();
    if is_dir {
        collect_existing_files(&base_dir, target, &mut existing);
    } else if target.is_file() {
        existing.push(target.to_path_buf());
    }
    for path in existing {
        let relative = path
            .strip_prefix(&base_dir)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        let _ = tx.send((path, relative, product_type.to_string()));
    }
}

fn schedule_upload(
    app: AppHandle,
    tx: mpsc::UnboundedSender<QueueItem>,
    pending: PendingMap,
    path: PathBuf,
    relative: String,
    product_type: String,
) {
    let key = format!("{}::{}", product_type, path.to_string_lossy());
    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    {
        let mut map = pending.lock().unwrap();
        if let Some(prev) = map.remove(&key) {
            prev.abort();
            send_log(
                &app,
                "info",
                format!(
                    "Change again [{}] {} — 30s timer reset",
                    product_type, relative
                ),
            );
        } else {
            send_log(
                &app,
                "info",
                format!(
                    "Change detected [{}] {} ({}) — upload starts in 30 seconds",
                    product_type,
                    relative,
                    uploader::format_bytes(size)
                ),
            );
        }
    }

    let app2 = app.clone();
    let pending2 = pending.clone();
    let key2 = key.clone();
    let handle = tauri::async_runtime::spawn(async move {
        tokio::time::sleep(UPLOAD_DELAY).await;
        send_log(
            &app2,
            "info",
            format!("Starting upload of [{}] {} …", product_type, relative),
        );
        let _ = tx.send((path, relative, product_type));
        if let Ok(mut map) = pending2.lock() {
            map.remove(&key2);
        }
    });
    pending.lock().unwrap().insert(key, handle);
}

fn watch_target(
    target: PathBuf,
    product_type: String,
    tx: mpsc::UnboundedSender<QueueItem>,
    app: AppHandle,
    pending: PendingMap,
) -> Result<Debouncer<notify::RecommendedWatcher>, String> {
    let is_dir = target.is_dir();
    let base_dir = if is_dir {
        target.clone()
    } else {
        target.parent().map(Path::to_path_buf).unwrap_or_default()
    };

    let mut debouncer = new_debouncer(
        FILE_SETTLE,
        move |res: notify_debouncer_mini::DebounceEventResult| {
            let Ok(events) = res else { return };
            for event in events {
                if event.kind != DebouncedEventKind::Any {
                    continue;
                }
                let path = event.path;
                if !path.is_file() {
                    continue;
                }
                let relative = path.strip_prefix(&base_dir).unwrap_or(&path).to_path_buf();
                if is_hidden(&relative) {
                    continue;
                }
                schedule_upload(
                    app.clone(),
                    tx.clone(),
                    pending.clone(),
                    path,
                    relative.to_string_lossy().to_string(),
                    product_type.clone(),
                );
            }
        },
    )
    .map_err(|e| e.to_string())?;

    debouncer
        .watcher()
        .watch(&target, RecursiveMode::Recursive)
        .map_err(|e| e.to_string())?;

    Ok(debouncer)
}

pub fn start_watching(app: &AppHandle, config: &FtpConfig) -> Result<(), String> {
    let mut cfg = config.clone();
    cfg.normalize();
    let targets = cfg.watch_targets();
    if targets.is_empty() {
        send_log(app, "error", "Select a file or folder for LGD or NATURAL.");
        return Err("Select a file or folder for LGD or NATURAL.".into());
    }

    let mut existing_targets: Vec<(PathBuf, String)> = Vec::new();
    for (path, product_type) in targets {
        let target = PathBuf::from(&path);
        if path.is_empty() || !target.exists() {
            send_log(
                app,
                "error",
                format!("[{}] local path not found: {}", product_type, path),
            );
            continue;
        }
        existing_targets.push((target, product_type));
    }
    if existing_targets.is_empty() {
        return Err("Local path not found for the selected product type.".into());
    }

    stop_watching_inner(app, false);

    let (tx, rx) = mpsc::unbounded_channel::<QueueItem>();
    tauri::async_runtime::spawn(process_queue(app.clone(), cfg.clone(), rx));

    let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));
    let mut debouncers = Vec::new();
    let mut watched_labels = Vec::new();
    for (target, product_type) in existing_targets {
        enqueue_existing(&target, &product_type, &tx);
        let label = format!("{} → /{}", product_type, product_type);
        watched_labels.push(format!("{} ({})", label, target.display()));
        debouncers.push(watch_target(
            target,
            product_type,
            tx.clone(),
            app.clone(),
            pending.clone(),
        )?);
    }

    let state = app.state::<WatchState>();
    {
        let mut handle = state.0.lock().unwrap();
        handle.debouncers = debouncers;
        handle.watching = true;
        handle.pending = pending;
    }

    send_log(
        app,
        "ok",
        format!(
            "Watching {} — changed files upload 30 seconds after the last save. Progress shows in this log.",
            watched_labels.join("; ")
        ),
    );
    Ok(())
}

fn stop_watching_inner(app: &AppHandle, log: bool) {
    let state = app.state::<WatchState>();
    let mut handle = state.0.lock().unwrap();
    if !handle.debouncers.is_empty() && log {
        send_log(app, "info", "Stopped watching.");
    }
    for (_, task) in handle.pending.lock().unwrap().drain() {
        task.abort();
    }
    handle.debouncers.clear();
    handle.watching = false;
}

pub fn stop_watching(app: &AppHandle) {
    stop_watching_inner(app, true);
}

pub fn is_watching(app: &AppHandle) -> bool {
    app.state::<WatchState>()
        .0
        .lock()
        .map(|h| h.watching)
        .unwrap_or(false)
}

pub fn emit_watch_state(app: &AppHandle, watching: bool) {
    send_watch_state(app, watching);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn hidden_dotfiles_and_dot_folders_are_skipped() {
        assert!(is_hidden(Path::new(".hidden.csv")));
        assert!(is_hidden(Path::new(".DS_Store")));
        assert!(is_hidden(Path::new("sub/.secret/a.csv")));
        assert!(!is_hidden(Path::new("visible.csv")));
        assert!(!is_hidden(Path::new("batch/inner.csv")));
    }

    #[test]
    fn collect_existing_skips_hidden_and_walks_subfolders() {
        let root = std::env::temp_dir().join("lgd-collect-test");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("keep")).unwrap();
        fs::create_dir_all(root.join(".skip")).unwrap();
        fs::write(root.join("keep/ok.csv"), "a").unwrap();
        fs::write(root.join(".skip/no.csv"), "b").unwrap();
        fs::write(root.join(".dot.csv"), "c").unwrap();
        let mut out = Vec::new();
        collect_existing_files(&root, &root, &mut out);
        let mut rels: Vec<String> = out
            .iter()
            .map(|p| p.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/"))
            .collect();
        rels.sort();
        assert_eq!(rels, vec!["keep/ok.csv".to_string()]);
        let _ = fs::remove_dir_all(&root);
    }
}
