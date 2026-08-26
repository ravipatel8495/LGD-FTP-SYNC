# Code signing (firewalls / SmartScreen / Gatekeeper)

Unsigned `.exe`, `.dmg`, and `.app` files are blocked by many company firewalls,
Windows SmartScreen, and macOS Gatekeeper. This project signs installers.

## What is free vs paid

There is **no free Apple or Microsoft certificate** that removes Gatekeeper or
SmartScreen. Those require a paid Developer ID / EV-OV cert.

This repo uses every **free** signing option that actually exists:

| Method | Cost | What it does |
|---|---|---|
| Augmont self-signed `.p12` (already in GitHub secrets) | Free | File is marked signed; Windows/Mac OS still warn |
| `SHA256SUMS.txt` on each GitHub Release | Free | Check the file was not corrupted |
| [GitHub artifact attestations](https://docs.github.com/en/actions/security-for-github-actions/using-artifact-attestations) (Sigstore) | Free | Proves the installer was built by this repo’s Actions |
| Apple Developer ID + notarize | $99/year | Removes the Mac “could not verify” dialog |
| Windows EV/OV code signing | Paid | Reduces/removes SmartScreen |

GitHub Releases pack the Augmont CA **inside** the Windows `.exe` (setup
installs it for the current user) and the macOS `.dmg` (file is in the disk
image and inside the `.app`). You do not need a separate `.crt` download.

Verify a download:


```bash
# checksums from the Release page
shasum -a 256 -c SHA256SUMS.txt

# free GitHub provenance
gh attestation verify "LGD FTP DATA SYNC_1.0.1_x64-setup.exe" --repo ravipatel8495/LGD-FTP-SYNC

# or
bash scripts/verify-release.sh path/to/installer SHA256SUMS.txt
```


## Files (created on this Mac, not committed)

| File | Use |
|---|---|
| `certs/augmont-ca.crt` | Give to IT / install on each PC so Windows trusts the signed EXE |
| `certs/augmont-codesign.p12` | Signing key (macOS + Windows). Keep secret. |
| `certs/PASSWORD.txt` | Password for the `.p12` |
| `certs/windows-thumbprint.txt` | Thumbprint for Tauri on Windows |
| `certs/augmont-codesign.p12.base64.txt` | Paste into GitHub secret `WINDOWS_CERTIFICATE` / `APPLE_CERTIFICATE` |

## Sign the macOS build

```bash
bash scripts/create-self-signed-certs.sh
bash scripts/sign-macos.sh
```

## Windows PC (so the firewall accepts the EXE)

1. Copy `certs/augmont-ca.crt` (also in `dist/macos/Install-this-CA-on-PCs.crt`).
2. Double-click → **Install Certificate** → **Local Machine** →
   **Place all certificates in the following store** → **Trusted Root Certification Authorities**.
3. Repeat the same install into **Trusted Publishers**.
4. Then install the signed `.exe`.

To **build** a signed Windows installer, run `npm run dist:win` on Windows after
importing `augmont-codesign.pfx` into `Cert:\CurrentUser\My`, then set
`bundle.windows.certificateThumbprint` in `tauri.conf.json` to the value in
`certs/windows-thumbprint.txt`.

## GitHub Actions (auto-release, self-signed)

Each successful GitHub build publishes a **new versioned Release**
(`v1.0.1`, `v1.0.2`, …). Older versions stay on the Releases page.

If `v1.0.1` already exists, the next build becomes `v1.0.2`. You can also
type a version in **Run workflow**, or push a tag such as `v1.0.3`.

**Repo → Settings → Secrets and variables → Actions** — add:

| Secret | Value |
|---|---|
| `WINDOWS_CERTIFICATE` | contents of `certs/augmont-codesign.p12.base64.txt` |
| `WINDOWS_CERTIFICATE_PASSWORD` | contents of `certs/PASSWORD.txt` |
| `APPLE_CERTIFICATE` | same as `WINDOWS_CERTIFICATE` (self-signed `.p12` base64) |
| `APPLE_CERTIFICATE_PASSWORD` | same as `WINDOWS_CERTIFICATE_PASSWORD` |
| `APPLE_SIGNING_IDENTITY` | `Augmont Goldtech` |

Then push to `main`, or **Actions → Release installers → Run workflow**.
Each run adds a new version page under **Releases**. Previous versions stay.

`APPLE_ID` / `APPLE_PASSWORD` / `APPLE_TEAM_ID` are only for **Apple notarization**
(paid Apple Developer). Self-sign still shows Gatekeeper “could not verify”
until notarized.

## GitHub Actions secrets (for signed Windows + Apple notarized Mac)

| Secret | Value |
|---|---|
| `WINDOWS_CERTIFICATE` | contents of `augmont-codesign.p12.base64.txt` |
| `WINDOWS_CERTIFICATE_PASSWORD` | contents of `PASSWORD.txt` |
| `APPLE_CERTIFICATE` | same p12 base64 (self-signed) **or** your Apple Developer ID `.p12` |
| `APPLE_CERTIFICATE_PASSWORD` | p12 password |
| `APPLE_SIGNING_IDENTITY` | `Augmont Goldtech` (self-signed) or `Developer ID Application: Your Name (TEAMID)` |
| `APPLE_ID` / `APPLE_PASSWORD` / `APPLE_TEAM_ID` | only for **Apple notarization** (paid Apple Developer) |

## Recommended for production (no warnings)

1. **Windows:** buy an OV or EV **code signing** certificate (DigiCert, Sectigo, SSL.com). SSL certs do not work.
2. **macOS:** Apple Developer Program, then Developer ID Application + notarize.

Replace the self-signed `.p12` with those purchased certificates and keep the same GitHub secret names.
