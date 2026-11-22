; rustmud – Windows-Installer (NSIS)
;
; Bauen:  makensis /DVERSION=0.0.1 /DEXE=..\target\release\rustmud.exe installer\rustmud.nsi
;
; Installiert pro Benutzer nach %LOCALAPPDATA%\Programs\rustmud (keine Adminrechte nötig).
; Parameter für Updates aus dem Programm: /S (still) und /RUN (danach rustmud starten).

Unicode true
SetCompressor /SOLID lzma

!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!ifndef EXE
  !define EXE "..\target\release\rustmud.exe"
!endif
!ifndef OUTFILE
  !define OUTFILE "rustmud-setup-x64.exe"
!endif

!define APPNAME "rustmud"
!define PUBLISHER "areadox"
!define URL "https://github.com/areadox/Rustmud"
!define UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\rustmud"

!include "MUI2.nsh"
!include "FileFunc.nsh"

Name "${APPNAME} ${VERSION}"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\Programs\rustmud"
InstallDirRegKey HKCU "Software\rustmud" "InstallDir"
RequestExecutionLevel user
BrandingText "${APPNAME} ${VERSION}"

VIProductVersion "${VERSION}.0"
VIAddVersionKey /LANG=1031 "ProductName" "${APPNAME}"
VIAddVersionKey /LANG=1031 "CompanyName" "${PUBLISHER}"
VIAddVersionKey /LANG=1031 "FileDescription" "${APPNAME} Setup"
VIAddVersionKey /LANG=1031 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=1031 "ProductVersion" "${VERSION}"
VIAddVersionKey /LANG=1031 "LegalCopyright" "(c) 2026 areadox - MIT"

!define MUI_ICON "..\assets\rustmud.ico"
!define MUI_UNICON "..\assets\rustmud.ico"
!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN "$INSTDIR\rustmud.exe"
!define MUI_FINISHPAGE_LINK "rustmud auf GitHub"
!define MUI_FINISHPAGE_LINK_LOCATION "${URL}"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "..\LICENSE"
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "German"
!insertmacro MUI_LANGUAGE "English"

LangString DESC_Main ${LANG_GERMAN} "Das Programm rustmud (erforderlich)."
LangString DESC_Main ${LANG_ENGLISH} "The rustmud program (required)."
LangString DESC_Start ${LANG_GERMAN} "Verknüpfung im Startmenü."
LangString DESC_Start ${LANG_ENGLISH} "Start menu shortcut."
LangString DESC_Desk ${LANG_GERMAN} "Verknüpfung auf dem Desktop."
LangString DESC_Desk ${LANG_ENGLISH} "Desktop shortcut."
LangString NAME_Start ${LANG_GERMAN} "Startmenü-Eintrag"
LangString NAME_Start ${LANG_ENGLISH} "Start menu entry"
LangString NAME_Desk ${LANG_GERMAN} "Desktop-Verknüpfung"
LangString NAME_Desk ${LANG_ENGLISH} "Desktop shortcut"

Function .onInit
  ; Bei Updates aus dem Programm kurz warten, bis rustmud beendet ist
  ${GetParameters} $R0
  ClearErrors
  ${GetOptions} $R0 "/RUN" $R1
  IfErrors +2
    Sleep 2000
FunctionEnd

Section "rustmud" SecMain
  SectionIn RO
  SetOutPath "$INSTDIR"
  ; Falls rustmud.exe noch läuft (z. B. beim Update), einige Sekunden warten statt abzubrechen
  SetOverwrite try
  StrCpy $R2 0
  retry:
    ClearErrors
    File "/oname=rustmud.exe" "${EXE}"
    IfErrors 0 copied
    IntOp $R2 $R2 + 1
    IntCmp $R2 15 locked wait locked
  wait:
    Sleep 1000
    Goto retry
  locked:
    MessageBox MB_OK|MB_ICONSTOP "rustmud.exe ist noch geöffnet. Bitte rustmud beenden und das Setup erneut starten." /SD IDOK
    Abort
  copied:
  SetOverwrite on
  File "..\README.md"
  File "..\LICENSE"
  File "..\CHANGELOG.md"
  SetOutPath "$INSTDIR\beispiele"
  File "..\beispiele\*.lua"
  SetOutPath "$INSTDIR"

  WriteUninstaller "$INSTDIR\uninstall.exe"
  WriteRegStr HKCU "Software\rustmud" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayName" "${APPNAME}"
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTKEY}" "Publisher" "${PUBLISHER}"
  WriteRegStr HKCU "${UNINSTKEY}" "URLInfoAbout" "${URL}"
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayIcon" "$INSTDIR\rustmud.exe"
  WriteRegStr HKCU "${UNINSTKEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTKEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "${UNINSTKEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKCU "${UNINSTKEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTKEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU "${UNINSTKEY}" "EstimatedSize" "$0"
SectionEnd

Section "$(NAME_Start)" SecStart
  CreateShortcut "$SMPROGRAMS\rustmud.lnk" "$INSTDIR\rustmud.exe"
SectionEnd

Section /o "$(NAME_Desk)" SecDesk
  CreateShortcut "$DESKTOP\rustmud.lnk" "$INSTDIR\rustmud.exe"
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecMain} $(DESC_Main)
  !insertmacro MUI_DESCRIPTION_TEXT ${SecStart} $(DESC_Start)
  !insertmacro MUI_DESCRIPTION_TEXT ${SecDesk} $(DESC_Desk)
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Function .onInstSuccess
  ; Nach einem stillen Update rustmud wieder starten
  ${GetParameters} $R0
  ClearErrors
  ${GetOptions} $R0 "/RUN" $R1
  IfErrors +2
    Exec '"$INSTDIR\rustmud.exe"'
FunctionEnd

Section "Uninstall"
  Delete "$INSTDIR\rustmud.exe"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\CHANGELOG.md"
  Delete "$INSTDIR\beispiele\*.lua"
  RMDir "$INSTDIR\beispiele"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\rustmud.lnk"
  Delete "$DESKTOP\rustmud.lnk"
  DeleteRegKey HKCU "${UNINSTKEY}"
  DeleteRegKey HKCU "Software\rustmud"
  ; Welten und Logs in %APPDATA%\areadox\rustmud bleiben erhalten
SectionEnd
