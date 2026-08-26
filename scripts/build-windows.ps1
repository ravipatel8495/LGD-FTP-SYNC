# Build Windows installers on a Windows 10/11 PC, then copy them into dist\windows.
# Requires: Node.js 20+ (https://nodejs.org) and Rust (https://rustup.rs)
# Run in PowerShell from the project folder:
#   powershell -ExecutionPolicy Bypass -File scripts\build-windows.ps1

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

if (-not (Get-Command npm -ErrorAction SilentlyContinue)) {
  throw "Install Node.js 20+ from https://nodejs.org then run this script again."
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
  throw "Install Rust from https://rustup.rs then close and reopen PowerShell."
}

Write-Host "Installing npm packages..."
npm install

Write-Host "Building Windows NSIS installer + MSI..."
npm run dist:win

$bundle = Join-Path $Root "src-tauri\target\release\bundle"
$dest = Join-Path $Root "dist\windows"
New-Item -ItemType Directory -Force -Path $dest | Out-Null

Get-ChildItem -Path $bundle -Recurse -Include *.exe, *.msi | ForEach-Object {
  Copy-Item $_.FullName -Destination $dest -Force
  Write-Host "Copied $($_.Name)"
}

Write-Host ""
Write-Host "Windows installers are in dist\windows\"
Get-ChildItem $dest | Format-Table Name, Length
