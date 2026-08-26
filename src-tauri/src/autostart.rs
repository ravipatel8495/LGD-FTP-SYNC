// Login autostart that survives shutdown/reboot. The Tauri plugin writes an
// unquoted Windows Run-key command, which breaks because this app's install
// path contains spaces ("LGD FTP DATA SYNC").
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[cfg(not(target_os = "macos"))]
const APP_NAME: &str = "LGD FTP DATA SYNC";
const LAUNCH_LABEL: &str = "com.diamonds.ftpuploader";
const HIDDEN_ARG: &str = "--hidden";

fn current_exe() -> Result<PathBuf, String> {
    env::current_exe()
        .map_err(|e| e.to_string())?
        .canonicalize()
        .map_err(|e| e.to_string())
}

#[cfg(target_os = "macos")]
fn macos_bundle_path(exe: &Path) -> Option<PathBuf> {
    let s = exe.to_string_lossy();
    let idx = s.find(".app")?;
    Some(PathBuf::from(&s[..=idx + 3]))
}

#[cfg(target_os = "macos")]
fn launch_agents_dir() -> Result<PathBuf, String> {
    let home = env::var_os("HOME").ok_or("HOME is not set")?;
    let dir = PathBuf::from(home).join("Library/LaunchAgents");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

#[cfg(target_os = "macos")]
fn plist_path() -> Result<PathBuf, String> {
    Ok(launch_agents_dir()?.join(format!("{LAUNCH_LABEL}.plist")))
}

#[cfg(target_os = "linux")]
fn desktop_path() -> Result<PathBuf, String> {
    let base = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .ok_or("HOME is not set")?;
    let dir = base.join("autostart");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("lgd-ftp-data-sync.desktop"))
}

pub fn enable() -> Result<(), String> {
    let exe = current_exe()?;
    #[cfg(target_os = "windows")]
    {
        enable_windows(&exe)
    }
    #[cfg(target_os = "macos")]
    {
        enable_macos(&exe)
    }
    #[cfg(target_os = "linux")]
    {
        enable_linux(&exe)
    }
}

pub fn disable() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        disable_windows()
    }
    #[cfg(target_os = "macos")]
    {
        disable_macos()
    }
    #[cfg(target_os = "linux")]
    {
        disable_linux()
    }
}

#[cfg(target_os = "windows")]
fn quoted_command(exe: &Path) -> String {
    format!("\"{}\" {HIDDEN_ARG}", exe.display())
}

#[cfg(target_os = "windows")]
fn enable_windows(exe: &Path) -> Result<(), String> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};
    use winreg::{RegKey, RegValue, enums::RegType::REG_BINARY};

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run = hkcu
        .open_subkey_with_flags(
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
            KEY_SET_VALUE,
        )
        .map_err(|e| e.to_string())?;
    let cmd = quoted_command(exe);
    run.set_value(APP_NAME, &cmd).map_err(|e| e.to_string())?;
    // Clear a leftover key written by tauri-plugin-autostart (unquoted path).
    let _ = run.delete_value("lgd-ftp-data-sync");

    const ENABLED: [u8; 12] = [0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    if let Ok(approved) = hkcu.open_subkey_with_flags(
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run",
        KEY_SET_VALUE,
    ) {
        let _ = approved.set_raw_value(
            APP_NAME,
            &RegValue {
                vtype: REG_BINARY,
                bytes: ENABLED.to_vec(),
            },
        );
        let _ = approved.delete_value("lgd-ftp-data-sync");
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn disable_windows() -> Result<(), String> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(run) = hkcu.open_subkey_with_flags(
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
        KEY_SET_VALUE,
    ) {
        let _ = run.delete_value(APP_NAME);
        let _ = run.delete_value("lgd-ftp-data-sync");
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn enable_macos(exe: &Path) -> Result<(), String> {
    let args_xml = if let Some(bundle) = macos_bundle_path(exe) {
        format!(
            "<string>/usr/bin/open</string>\
             <string>-g</string>\
             <string>-a</string>\
             <string>{}</string>\
             <string>--args</string>\
             <string>{HIDDEN_ARG}</string>",
            bundle.display()
        )
    } else {
        format!(
            "<string>{}</string><string>{HIDDEN_ARG}</string>",
            exe.display()
        )
    };

    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
  <dict>
    <key>Label</key>
    <string>{LAUNCH_LABEL}</string>
    <key>ProgramArguments</key>
    <array>{args_xml}</array>
    <key>RunAtLoad</key>
    <true/>
    <key>ProcessType</key>
    <string>Interactive</string>
  </dict>
</plist>
"#
    );

    let dest = plist_path()?;
    fs::File::create(&dest)
        .and_then(|mut f| f.write_all(plist.as_bytes()))
        .map_err(|e| e.to_string())?;

    // Remove the plist written by tauri-plugin-autostart (wrong label / exe).
    if let Ok(dir) = launch_agents_dir() {
        let _ = fs::remove_file(dir.join("lgd-ftp-data-sync.plist"));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn disable_macos() -> Result<(), String> {
    if let Ok(path) = plist_path() {
        let _ = fs::remove_file(path);
    }
    if let Ok(dir) = launch_agents_dir() {
        let _ = fs::remove_file(dir.join("lgd-ftp-data-sync.plist"));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn enable_linux(exe: &Path) -> Result<(), String> {
    let body = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name={APP_NAME}\n\
         Exec=\"{}\" {HIDDEN_ARG}\n\
         X-GNOME-Autostart-enabled=true\n\
         Hidden=false\n",
        exe.display()
    );
    fs::write(desktop_path()?, body).map_err(|e| e.to_string())
}

#[cfg(target_os = "linux")]
fn disable_linux() -> Result<(), String> {
    if let Ok(path) = desktop_path() {
        let _ = fs::remove_file(path);
    }
    Ok(())
}
