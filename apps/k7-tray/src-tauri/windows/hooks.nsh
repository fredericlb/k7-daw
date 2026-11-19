; hooks.nsh — NSIS hooks of K7 DAW's installer: the windowless tray this app replaces is uninstalled first, so the
; two never sit side by side (docs/desktop.md).

!macro NSIS_HOOK_PREINSTALL
  ; The tray's own uninstaller, found by the key its productName gave it, run silently and waited for (`_?=` keeps it
  ; from copying itself away and returning at once). Its login entry stays: the app's first start moves it.
  ReadRegStr $R1 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\k7-tray" "InstallLocation"
  ${If} $R1 != ""
  ${AndIf} ${FileExists} "$R1\uninstall.exe"
    ExecWait '"$R1\uninstall.exe" /S _?=$R1'
    Delete "$R1\uninstall.exe"
    RMDir "$R1"
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\k7-tray"
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
!macroend

!macro NSIS_HOOK_PREUNINSTALL
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
!macroend
