use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

fn default_product_type() -> String {
    "LGD".into()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LocalPaths {
    #[serde(default, rename = "LGD")]
    pub lgd: String,
    #[serde(default, rename = "NATURAL")]
    pub natural: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FtpConfig {
    pub protocol: String,
    pub host: String,
    #[serde(default)]
    pub port: String,
    pub user: String,
    pub password: String,
    #[serde(default = "default_product_type")]
    pub product_type: String,
    #[serde(default)]
    pub local_path: String,
    #[serde(default)]
    pub local_paths: LocalPaths,
    #[serde(default)]
    pub watching: bool,
}

impl FtpConfig {
    pub fn normalized_product_type(&self) -> String {
        match self.product_type.to_uppercase().as_str() {
            "NATURAL" => "NATURAL".into(),
            _ => "LGD".into(),
        }
    }

    pub fn normalize(&mut self) {
        self.product_type = self.normalized_product_type();
        if self.local_paths.lgd.is_empty()
            && self.local_paths.natural.is_empty()
            && !self.local_path.is_empty()
        {
            if self.product_type == "NATURAL" {
                self.local_paths.natural = self.local_path.clone();
            } else {
                self.local_paths.lgd = self.local_path.clone();
            }
        } else if !self.local_path.is_empty() {
            self.sync_current_path();
        }
        self.local_path = if self.product_type == "NATURAL" {
            self.local_paths.natural.clone()
        } else {
            self.local_paths.lgd.clone()
        };
    }

    pub fn sync_current_path(&mut self) {
        if self.product_type == "NATURAL" {
            if !self.local_path.is_empty() {
                self.local_paths.natural = self.local_path.clone();
            }
        } else if !self.local_path.is_empty() {
            self.local_paths.lgd = self.local_path.clone();
        }
    }

    /// (local path, product type) pairs that should be watched.
    pub fn watch_targets(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        if !self.local_paths.lgd.is_empty() {
            out.push((self.local_paths.lgd.clone(), "LGD".into()));
        }
        if !self.local_paths.natural.is_empty() {
            out.push((self.local_paths.natural.clone(), "NATURAL".into()));
        }
        if out.is_empty() && !self.local_path.is_empty() {
            out.push((self.local_path.clone(), self.normalized_product_type()));
        }
        out
    }
}

fn config_file(app: &AppHandle) -> PathBuf {
    let dir = app
        .path()
        .app_config_dir()
        .expect("could not resolve app config dir");
    fs::create_dir_all(&dir).ok();
    dir.join("config.json")
}

fn legacy_config_paths() -> Vec<PathBuf> {
    let mut out = Vec::new();
    #[cfg(target_os = "macos")]
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        out.push(
            home.join("Library/Application Support/LGD FTP DATA SYNC/config.json"),
        );
        out.push(
            home.join("Library/Application Support/com.diamonds.ftpuploader/config.json"),
        );
    }
    #[cfg(target_os = "windows")]
    if let Some(appdata) = std::env::var_os("APPDATA") {
        out.push(PathBuf::from(appdata).join("LGD FTP DATA SYNC").join("config.json"));
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            out.push(
                PathBuf::from(home)
                    .join(".config/LGD FTP DATA SYNC/config.json"),
            );
        }
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            out.push(PathBuf::from(xdg).join("LGD FTP DATA SYNC/config.json"));
        }
    }
    out
}

fn parse_config(data: &str) -> Option<FtpConfig> {
    let mut cfg: FtpConfig = serde_json::from_str(data).ok()?;
    cfg.normalize();
    Some(cfg)
}

pub fn load_config(app: &AppHandle) -> Option<FtpConfig> {
    let dest = config_file(app);
    if let Ok(data) = fs::read_to_string(&dest) {
        if let Some(cfg) = parse_config(&data) {
            return Some(cfg);
        }
    }
    // Software update / Electron → Tauri: reuse the previous config so the
    // user does not have to re-enter host, user, or password.
    for legacy in legacy_config_paths() {
        if legacy == dest {
            continue;
        }
        if let Ok(data) = fs::read_to_string(&legacy) {
            if let Some(cfg) = parse_config(&data) {
                save_config(app, &cfg);
                return Some(cfg);
            }
        }
    }
    None
}

pub fn save_config(app: &AppHandle, config: &FtpConfig) {
    let mut to_save = config.clone();
    to_save.normalize();
    if let Ok(data) = serde_json::to_string_pretty(&to_save) {
        let _ = fs::write(config_file(app), data);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> FtpConfig {
        FtpConfig {
            protocol: "ftp".into(),
            host: "127.0.0.1".into(),
            port: "21".into(),
            user: "u".into(),
            password: "p".into(),
            product_type: "lgd".into(),
            local_path: "/tmp/lgd.csv".into(),
            local_paths: LocalPaths::default(),
            watching: false,
        }
    }

    #[test]
    fn normalize_copies_legacy_path_into_product_slot() {
        let mut cfg = sample();
        cfg.normalize();
        assert_eq!(cfg.product_type, "LGD");
        assert_eq!(cfg.local_paths.lgd, "/tmp/lgd.csv");
        assert!(cfg.local_paths.natural.is_empty());
        assert_eq!(cfg.watch_targets(), vec![("/tmp/lgd.csv".into(), "LGD".into())]);
    }

    #[test]
    fn watch_targets_include_both_product_types() {
        let mut cfg = sample();
        cfg.local_path = "/tmp/lgd".into();
        cfg.local_paths.lgd = "/tmp/lgd".into();
        cfg.local_paths.natural = "/tmp/natural".into();
        cfg.normalize();
        let targets = cfg.watch_targets();
        assert_eq!(targets.len(), 2);
        assert!(targets.contains(&("/tmp/lgd".into(), "LGD".into())));
        assert!(targets.contains(&("/tmp/natural".into(), "NATURAL".into())));
    }

    #[test]
    fn old_config_json_without_product_type_still_parses() {
        let json = r#"{
            "protocol":"ftp","host":"h","port":"21","user":"u",
            "password":"p","localPath":"/old.csv","watching":true
        }"#;
        let cfg = parse_config(json).expect("legacy config");
        assert_eq!(cfg.product_type, "LGD");
        assert_eq!(cfg.local_path, "/old.csv");
        assert_eq!(cfg.local_paths.lgd, "/old.csv");
        assert!(cfg.watching);
    }
}

/// Merge a UI payload with the on-disk config so an update / auto-save never
/// drops the other product type's path or forces the user to re-enter FTP details.
pub fn merge_with_saved(app: &AppHandle, incoming: &FtpConfig, watching: Option<bool>) -> FtpConfig {
    let mut merged = incoming.clone();
    merged.normalize();
    if let Some(old) = load_config(app) {
        if merged.protocol.is_empty() {
            merged.protocol = old.protocol;
        }
        if merged.host.is_empty() {
            merged.host = old.host;
        }
        if merged.port.is_empty() {
            merged.port = old.port;
        }
        if merged.user.is_empty() {
            merged.user = old.user;
        }
        if merged.password.is_empty() {
            merged.password = old.password;
        }
        merged.watching = watching.unwrap_or(old.watching);
        merged.normalize();
    } else if let Some(flag) = watching {
        merged.watching = flag;
    }
    merged
}
