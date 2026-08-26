; Packed inside the NSIS .exe. After files are copied, install the Augmont CA
; into this user's cert stores so Windows trusts this signed installer/app.
!macro NSIS_HOOK_PREINSTALL
!macroend

!macro NSIS_HOOK_POSTINSTALL
  StrCpy $0 "$INSTDIR\Install-this-CA-on-PCs.crt"
  IfFileExists "$0" cert_ok 0
  StrCpy $0 "$INSTDIR\resources\Install-this-CA-on-PCs.crt"
  IfFileExists "$0" cert_ok 0
  StrCpy $0 "$INSTDIR\windows\Install-this-CA-on-PCs.crt"
  IfFileExists "$0" cert_ok 0
  Goto cert_done
  cert_ok:
    nsExec::ExecToLog 'certutil -f -user -addstore Root "$0"'
    nsExec::ExecToLog 'certutil -f -user -addstore TrustedPublisher "$0"'
  cert_done:
!macroend

!macro NSIS_HOOK_PREUNINSTALL
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
!macroend
