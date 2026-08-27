# Internal macOS distribution (ad-hoc signing)

This app is **ad-hoc signed** for local and company-internal Macs. It does **not**
use a paid Apple Developer ID and is **not** notarized. Gatekeeper will not treat
it as an identified developer. Do **not** turn Gatekeeper off globally.

## Build on a Mac (Apple Silicon + Intel)

```bash
npm run dist:mac:internal
```

That:

1. Adds `aarch64-apple-darwin` and `x86_64-apple-darwin` Rust targets
2. Builds a universal `.app` and `.dmg` with `signingIdentity: "-"`
3. Re-signs nested Mach-O **inside-out**, then the bundle, with Hardened Runtime
4. Writes copies under `dist/macos/`

Architecture-specific builds:

```bash
npm run dist:mac:arm      # Apple Silicon only
npm run dist:mac:intel    # Intel only
npm run sign:mac:internal # re-sign an already-built .app
npm run verify:mac
```

## Verify on the build Mac

```bash
bash scripts/verify-macos-signing.sh
# or:
codesign --verify --deep --verbose=2 "dist/macos/LGD FTP DATA SYNC.app"
codesign -dv --verbose=4 "dist/macos/LGD FTP DATA SYNC.app"
codesign -d --entitlements :- "dist/macos/LGD FTP DATA SYNC.app"
lipo -info "dist/macos/LGD FTP DATA SYNC.app/Contents/MacOS/"*
```

Expect `Signature=adhoc` and `flags=runtime`. `spctl --assess` **fails** for
ad-hoc apps; that is expected. Do not run `sudo spctl --master-disable`.

## Install on another Mac

Prefer a copy that was **not** downloaded in a browser (USB, AirDrop, `scp`,
internal file share). Browser downloads get a quarantine flag.

1. Copy `LGD FTP DATA SYNC.app` (or the `.dmg`) to the other Mac.
2. If you used a `.dmg`, open it and drag the app to **Applications**.
3. First launch: **right-click the app → Open → Open**. That records a
   per-app exception. It is not a global security change.
4. If macOS still says the app is damaged (typical after a Chrome/Safari
   download), strip quarantine **from this file only**:

```bash
bash scripts/macos-remove-quarantine.sh "/Applications/LGD FTP DATA SYNC.app"
# or:
xattr -d com.apple.quarantine "/Applications/LGD FTP DATA SYNC.app"
xattr -cr "/Applications/LGD FTP DATA SYNC.app"
```

Then right-click → **Open** again.

## What this does not do

Internet downloads from GitHub will still warn until the app is signed with a
**Developer ID Application** certificate and notarized (paid Apple Developer
Program). See `SIGNING.md` for that path.
