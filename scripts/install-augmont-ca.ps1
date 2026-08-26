# Run in PowerShell as Administrator on each Windows PC
# (or deploy via Group Policy) so the company firewall / SmartScreen
# accepts Augmont-signed LGD FTP DATA SYNC installers.
$ErrorActionPreference = "Stop"
$cert = Join-Path $PSScriptRoot "Install-this-CA-on-PCs.crt"
if (-not (Test-Path $cert)) {
  Write-Error "Put Install-this-CA-on-PCs.crt next to this script."
}
Import-Certificate -FilePath $cert -CertStoreLocation Cert:\LocalMachine\Root | Out-Null
Import-Certificate -FilePath $cert -CertStoreLocation Cert:\LocalMachine\TrustedPublisher | Out-Null
Write-Host "Augmont CA installed. You can now run the signed LGD FTP DATA SYNC installer."
