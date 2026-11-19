; hooks.nsh — NSIS hooks of K7 DAW's installer: the windowless tray this app replaces is uninstalled first, and its
; login entry removed, so the two never sit side by side (docs/desktop.md).

!macro NSIS_HOOK_PREINSTALL
  ; The tray's own uninstaller, found by the key its productName gave it, run silently.
  ReadRegStr $R0 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\k7-tray" "QuietUninstallString"
  ${If} $R0 != ""
    ExecWait '$R0'
  ${EndIf}
  ; K7 DAW registers its own entry; the tray's would start a second app at login.
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "k7-tray"
!macroend

!macro NSIS_HOOK_POSTINSTALL
!macroend

!macro NSIS_HOOK_PREUNINSTALL
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
!macroend
