; ReviewGlass's additions to Tauri's NSIS installer (P7, adr.rg.038).
;
; Tauri's own template already installs the two collectors beside the app (they are the
; crate's other binaries), removes them on uninstall, and gives the Start Menu shortcut
; the app's identifier as its AppUserModelID, which is the app's own toast identity.
; What it cannot know, these hooks add:
;
; - the Start Menu entry "Stop ReviewGlass": taskkill on the app and its children (its
;   WebView2 processes), the way out of an app whose tray no longer answers;
; - on uninstall, reviewglass.exe --remove-integration: ReviewGlass's entries leave the
;   user's Claude Code settings and the collectors leave ~/.reviewglass/bin. The app
;   handles the flag before Tauri starts, so a running copy does not stand in the way.
;   Skipped when the uninstaller runs with /UPDATE. Tauri's installer also offers to
;   uninstall an older version first without /UPDATE, so a connected ReviewGlass
;   leaves a marker, and the new installer's --restore-integration puts the connection
;   back within the hour.
;
; The installer writes nothing into the Claude Code settings itself: Connect in
; Settings > Claude Code does, where the user sees the change first.

!macro NSIS_HOOK_POSTINSTALL
  ; Put back a connection the old version's uninstaller took down within the hour (an
  ; upgrade that uninstalled first). A first install does nothing here.
  ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --restore-integration'
  CreateShortcut "$SMPROGRAMS\Stop ${PRODUCTNAME}.lnk" "$SYSDIR\taskkill.exe" "/IM ${MAINBINARYNAME}.exe /F /T" "$INSTDIR\${MAINBINARYNAME}.exe" 0 SW_SHOWMINIMIZED "" "Stops ${PRODUCTNAME} and its web views when it no longer responds"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ${If} $UpdateMode <> 1
    ${If} ${FileExists} "$INSTDIR\${MAINBINARYNAME}.exe"
      ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --remove-integration'
    ${EndIf}
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    Delete "$SMPROGRAMS\Stop ${PRODUCTNAME}.lnk"
  ${EndIf}
!macroend
