# LGD FTP DATA SYNC

Augmont desktop app that watches a file or folder and automatically uploads any
new or changed file to your server over **FTP, FTPS (explicit), FTPS (implicit), or SFTP**.

- Runs in the **background** (system tray / macOS menu bar) — closing the window
  does not stop uploads.
- **Survives restarts** — while watch mode is on, the app starts automatically
  at login and resumes watching by itself.
- Subfolders are recreated on the server automatically.

---

## Installation

### Option A — Install from GitHub Releases (for end users)

Download the Windows `.exe` or macOS `.dmg` from
[Releases](https://github.com/ravipatel8495/LGD-FTP-SYNC/releases).
The Augmont CA is packed inside those files (Windows setup installs it automatically).

Local copies are also in the `dist/` folder after a release build.

| OS | Minimum version | File | How to install |
|----|-----------------|------|----------------|
| **Windows** | Windows **10** (1809+) or **11**, 64-bit | `LGD FTP DATA SYNC_1.0.1_x64-setup.exe` | Double-click the installer |
| **macOS** | **10.15 Catalina** and later (Intel + Apple Silicon) | `LGD FTP DATA SYNC_1.0.1_universal.dmg` | Open the DMG → drag the app to **Applications** |
| **Ubuntu / Linux** | Ubuntu **22.04** or **24.04** (or other distros with WebKitGTK 4.1) | `.deb` or `.AppImage` | `sudo dpkg -i …deb` or `chmod +x` the AppImage and run it |

**Windows 7, 8, and 8.1 are not supported.** This app uses WebView2, which Microsoft only provides for Windows 10 and 11.

> **Windows:** the `.exe` is signed and contains `Install-this-CA-on-PCs.crt`;
> setup installs that CA for the current user. See [SIGNING.md](SIGNING.md).
>
> **macOS:** the `.dmg` contains the same CA inside the app. If macOS still
> blocks the first open, right-click the app → **Open**. Apple notarization
> (paid Developer ID) is the only way to skip that prompt.

### Option B — Run from source (for developers)

Requires [Node.js](https://nodejs.org) 18 or newer and [Rust](https://rustup.rs) (the app is built with [Tauri](https://tauri.app), not Electron).

```bash
cd LGD-FTP-SYNC
npm install
npm start
```

---

## How to use

1. Choose **Connection Type** (FTP / FTPS / SFTP) — the port fills in automatically.
2. Enter **Host Name**, **User Name**, and **Password**.
3. Click **Test FTP** — if the connection works, all details are **saved
   automatically** and reload the next time you open the app.
4. Click **Browse…** and pick the **local folder** to watch
   (on macOS you can also pick a single file; on Windows/Linux type a file path
   into the box by hand if you want to watch one file).
5. Click **Start Watching** — existing files are uploaded first, then any file
  inside the watched path that is added or updated is uploaded automatically
  (subfolders are recreated on the server too). The Activity Log shows every upload.

Files are uploaded to the FTP account's **default directory** on the server.

### Background mode

Closing the window does **not** quit the app — it keeps running in the system
tray (menu bar on macOS) and keeps watching and uploading. On a Mac, Cmd+Q
also hides to the tray. Click the tray icon to reopen the window; quit from
the tray menu (**Quit**).

### Auto-start after shutdown / restart

Once **Start Watching** has been clicked one time, the app registers itself to
start automatically when you log in — after every shutdown or restart it opens
again in the tray (hidden in the background) and resumes watching by itself.
**Stop Watching** removes that login entry. To quit for this session only
(and still come back after a reboot), use the tray **Quit** while watching is on.

Auto-start happens at **user login**. If the machine must recover from a power
cut with nobody present, also enable automatic login in the OS:

- **Windows:** `Win + R` → `netplwiz` → untick *"Users must enter a user name
  and password"*.
- **macOS:** System Settings → Users & Groups → *Automatically log in as* (not
  available while FileVault is on).

---

## FAQ

**Which connection type should I pick?**
Whatever your hosting provider supports: FTP (port 21) is the classic default,
FTPS explicit (21) / implicit (990) add encryption, SFTP (22) runs over SSH.
When in doubt, ask your provider or try SFTP first — it is the most secure.

**Does it upload existing files when I click Start Watching?**
Yes. Existing files are uploaded when watching starts, and files that are
added or changed afterward are uploaded automatically. Hidden dot-files are
skipped.

**Are deleted files removed from the server?**
No — the app only uploads. It never deletes anything on the server.

**Are hidden files uploaded?**
No. Files and folders starting with a dot (`.git`, `.DS_Store`, …) are skipped.

**What if a file changes while it is being uploaded?**
The app waits until a file has stopped changing for 1.5 seconds before
uploading, and uploads one file at a time in a queue — so half-written files
are not sent.

**What if an upload fails (server down, no internet)?**
The failure is shown in the Activity Log. The file is uploaded again the next
time it changes. The app keeps watching — one failure does not stop it.

**Can two copies of the app run at the same time?**
No — launching it a second time just opens the window of the copy that is
already running, so files are never uploaded twice.

**Where are my settings saved? Is the password safe?**
In `config.json` inside the app's user-data folder
(Windows: `%APPDATA%/LGD FTP DATA SYNC`, macOS: `~/Library/Application Support/LGD FTP DATA SYNC`,
Linux: `~/.config/LGD FTP DATA SYNC`). **The password is stored in plain text** on
your own computer — only use the app on a machine you trust.

**How do I completely exit the app?**
Right-click the tray icon (menu bar icon on macOS) → **Quit**. Closing the
window only hides it.

**How do I uninstall?**
Windows: *Settings → Apps → LGD FTP DATA SYNC → Uninstall*. macOS: drag the app from
Applications to the Trash. Linux: delete the AppImage file. Also remove the
auto-start entry if one was created (Windows: Task Manager → Startup apps;
macOS: System Settings → General → Login Items).

---

## Troubleshooting

| Problem | What to check |
|---|---|
| **Test FTP fails: timeout** | Host name / port wrong, or a firewall blocks the port. Try the other connection types. |
| **Test FTP fails: 530 / authentication** | User name or password wrong. Some hosts need the user as `user@domain.com`. |
| **FTPS fails but FTP works** | The server may only support one FTPS mode — try *explicit* and *implicit* both. |
| **Uploads are slow to start** | Normal: the app waits 1.5 s after the last change so half-written files are not sent. |
| **Nothing uploads** | Is the watch state 🟢? Check the Activity Log for the connection or upload error. Hidden dot-files are skipped. |
| **App doesn't auto-start after reboot** | Auto-start is registered the first time you click **Start Watching** — click it once, then reboot. On macOS check System Settings → General → Login Items. |
| **Two icons / app already running** | Only one copy runs; the second launch just opens the existing window. |

Every upload, error, and state change is listed in the **Activity Log** at the
bottom of the window — check it first when something looks wrong. There is also
a built-in **Help & FAQ** section below the log.

---

## Build installers

Tauri can only produce an installer for the OS you build on (plus a universal
macOS DMG on an Apple Silicon Mac).

```bash
npm run dist:mac                    # this Mac: universal .app + .dmg  →  dist/macos/
npm run dist:win                    # run on Windows 10/11: NSIS .exe + MSI
npm run dist:linux                  # run on Ubuntu 22.04+: .deb + AppImage
```

To get **Windows + macOS + Ubuntu** in one go, use GitHub Actions **manually**:
**Actions → Release installers → Run workflow**. That builds all three
installers and publishes a **new** GitHub Release (`v1.0.1`, then `v1.0.2`, …).
Older version releases are **kept**. Pushing to `main` does **not** deploy.

1. Repo → **Settings → Secrets and variables → Actions** — add secrets from [SIGNING.md](SIGNING.md).
2. **Actions → Release installers → Run workflow** (optional version, or leave empty for next patch).
3. Open **Releases** — each version is a separate page with its Windows / Mac / Ubuntu files.

Output from a local build is also under `src-tauri/target/*/release/bundle/`.

## Android / iOS

Tauri (this app) does not run on phones. Also, iOS and Android do not allow an app to
watch the file system in the background the way a desktop can, so a true
"auto-upload on file change" app is not possible on phones. Realistic options:

- **Manual mobile app**: a small Flutter or React Native app where you pick a
  file and tap Upload (works on both Android and iOS). Ask and I can build it.
- **Android only**: apps like FolderSync already do watched-folder FTP sync.
- **Best answer for phones**: run this desktop app (or a small Node script) on
  a server/PC, and have the phone save files to a synced folder (Google Drive,
  Dropbox, Syncthing) that the watcher picks up.
