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

GitHub Actions **macOS** builds are **ad-hoc signed** (`signingIdentity: "-"`).
A self-signed “Augmont Goldtech” identity cannot be resolved on GitHub’s Mac
runners (`failed to resolve signing identity`). That does not remove Gatekeeper.
Paid Apple Developer ID + notarization is still required to skip Open Anyway.

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

## GitHub Actions (manual release)

Each successful GitHub build publishes a **new versioned Release**
(`v1.0.1`, `v1.0.2`, …). Older versions stay on the Releases page.

If `v1.0.1` already exists, the next build becomes `v1.0.2`. You can also
type a version in **Run workflow**. Pushing to `main` does **not** start a release.

**Repo → Settings → Secrets and variables → Actions** — add:

| Secret | Value |
|---|---|
| `WINDOWS_CERTIFICATE` | contents of `certs/augmont-codesign.p12.base64.txt` |
| `WINDOWS_CERTIFICATE_PASSWORD` | contents of `certs/PASSWORD.txt` |
| `APPLE_*` | **Developer ID** values — see section below. Do not use the self-signed p12. |

Then **Actions → Release installers → Run workflow**.
Each run adds a new version page under **Releases**. Previous versions stay.

## Apple Developer ID + notarize (Mac: no Open Anyway)

This is the only way a GitHub-downloaded `.dmg` opens with no extra clicks.
Cost: [Apple Developer Program](https://developer.apple.com/programs/) **$99/year**
(use Augmont’s company account if it already exists).

Do **not** reuse the self-signed `Augmont Goldtech` `.p12` for Mac. Replace
`APPLE_CERTIFICATE` with a **Developer ID Application** certificate.

### 1. Enroll

1. Open https://developer.apple.com/programs/ and enroll (company account preferred).
2. Wait until the membership is **Active**.
3. Copy **Team ID** (10 characters) from https://developer.apple.com/account → **Membership details**.

### 2. Create a CSR on a Mac

1. Open **Keychain Access**.
2. Menu: **Keychain Access → Certificate Assistant → Request a Certificate From a Certificate Authority…**
3. User Email: your Apple ID email. Common Name: `Augmont Goldtech` (or the legal company name).
4. Select **Saved to disk**. Save `CertificateSigningRequest.certSigningRequest`.

### 3. Create Developer ID Application cert

1. https://developer.apple.com/account/resources/certificates/list → **+**
2. Choose **Developer ID Application** (not Apple Development, not Mac App Store).
3. Continue → upload the CSR → download `developerID_application.cer`.
4. Double-click the `.cer` so it appears in Keychain Access (login keychain).

### 4. Export `.p12`

1. In Keychain Access, under **My Certificates**, find  
   `Developer ID Application: Your Name (TEAMID)`.
2. Expand it, select the cert **and** the private key, right-click → **Export**.
3. Format: **Personal Information Exchange (.p12)**. Set a password. Save `developer-id.p12`.
4. Confirm the identity string (Terminal):

```bash
security find-identity -v -p codesigning | grep "Developer ID Application"
```

Copy the full quoted name, for example:

`Developer ID Application: Augmont Goldtech Private Limited (AB12CD34EF)`

### 5. App-specific password (for notarization)

1. https://appleid.apple.com → **Sign-In and Security → App-Specific Passwords**.
2. Generate one named `github-notary`. Copy the `xxxx-xxxx-xxxx-xxxx` password.
   This is **not** your Apple ID login password.

### 6. GitHub secrets

Repo → **Settings → Secrets and variables → Actions**. **Replace** any old
self-signed Apple secrets:

| Secret | Value |
|---|---|
| `APPLE_CERTIFICATE` | `base64 -i developer-id.p12 \| tr -d '\n'` (one line) |
| `APPLE_CERTIFICATE_PASSWORD` | password you set on the `.p12` |
| `APPLE_SIGNING_IDENTITY` | exact `Developer ID Application: … (TEAMID)` string from step 4 |
| `APPLE_ID` | Apple ID email |
| `APPLE_PASSWORD` | app-specific password from step 5 |
| `APPLE_TEAM_ID` | 10-character Team ID |

Encode the p12:

```bash
base64 -i developer-id.p12 | tr -d '\n' | pbcopy
```

Paste into `APPLE_CERTIFICATE`. Keep `developer-id.p12` off git.

### 7. Release

**Actions → Release installers → Run workflow**. The Mac log must show
`Apple Developer ID + notarize`. After it finishes, download the new `.dmg`
from Releases — Gatekeeper should allow it with a normal double-click.

If the log still says `Ad-hoc Mac signing`, one of the six secrets is empty.

## Recommended for production (no warnings)

1. **Windows:** buy an OV or EV **code signing** certificate (DigiCert, Sectigo, SSL.com). SSL certs do not work.
2. **macOS:** Apple Developer Program, then Developer ID Application + notarize.

Replace the self-signed `.p12` with those purchased certificates and keep the same GitHub secret names.
