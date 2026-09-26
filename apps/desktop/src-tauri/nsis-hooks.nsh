!macro NSIS_HOOK_POSTINSTALL
  CreateShortCut "$DESKTOP\TeleCloud.lnk" "$INSTDIR\TeleCloud.exe" "" "$INSTDIR\TeleCloud.exe" 0
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  Delete "$DESKTOP\TeleCloud.lnk"
!macroend
