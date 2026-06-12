; NSIS installer for the R-Shell command-line tool (Windows x64).
;
; R-Shell is a CLI, so the installer copies r-shell.exe into Program Files and
; adds the install directory to the system PATH, then registers an uninstaller.
;
; Expected /D defines passed by CI:
;   APP_VERSION       human-readable version, e.g. 2.1.0
;   APP_VERSION_QUAD  4-part version, e.g. 2.1.0.0
;   STAGE_DIR         directory containing r-shell.exe, README.md, LICENSE
;   OUTPUT_EXE        full path of the installer to produce

Unicode true
SetCompressor /SOLID lzma

!ifndef APP_VERSION
  !define APP_VERSION "0.0.0"
!endif
!ifndef APP_VERSION_QUAD
  !define APP_VERSION_QUAD "0.0.0.0"
!endif
!ifndef STAGE_DIR
  !error "STAGE_DIR must be defined"
!endif
!ifndef OUTPUT_EXE
  !define OUTPUT_EXE "r-shell-windows-x64-installer.exe"
!endif

!define APP_NAME "R-Shell"
!define APP_PUBLISHER "MageGojo"
!define APP_EXE "r-shell.exe"
!define APP_REGKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\R-Shell"

!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "WinMessages.nsh"

; PATH editing uses the built-in registry writes plus a WM_WININICHANGE
; broadcast below, so no external plugin is required.

Name "${APP_NAME} ${APP_VERSION}"
OutFile "${OUTPUT_EXE}"
InstallDir "$PROGRAMFILES64\${APP_NAME}"
InstallDirRegKey HKLM "Software\${APP_NAME}" "InstallDir"
RequestExecutionLevel admin

VIProductVersion "${APP_VERSION_QUAD}"
VIAddVersionKey "ProductName" "${APP_NAME}"
VIAddVersionKey "Filedescription" "${APP_NAME} command-line SSH tool"
VIAddVersionKey "FileVersion" "${APP_VERSION}"
VIAddVersionKey "ProductVersion" "${APP_VERSION}"
VIAddVersionKey "CompanyName" "${APP_PUBLISHER}"
VIAddVersionKey "LegalCopyright" "MIT License"

!define MUI_ABORTWARNING

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "SimpChinese"

Section "Install"
  SetOutPath "$INSTDIR"
  File "${STAGE_DIR}\${APP_EXE}"
  File /nonfatal "${STAGE_DIR}\README.md"
  File /nonfatal "${STAGE_DIR}\LICENSE"

  WriteRegStr HKLM "Software\${APP_NAME}" "InstallDir" "$INSTDIR"

  ; Uninstaller + Add/Remove Programs entry.
  WriteUninstaller "$INSTDIR\uninstall.exe"
  WriteRegStr   HKLM "${APP_REGKEY}" "DisplayName"     "${APP_NAME}"
  WriteRegStr   HKLM "${APP_REGKEY}" "DisplayVersion"  "${APP_VERSION}"
  WriteRegStr   HKLM "${APP_REGKEY}" "Publisher"       "${APP_PUBLISHER}"
  WriteRegStr   HKLM "${APP_REGKEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr   HKLM "${APP_REGKEY}" "UninstallString" "$\"$INSTDIR\uninstall.exe$\""
  WriteRegDWORD HKLM "${APP_REGKEY}" "NoModify" 1
  WriteRegDWORD HKLM "${APP_REGKEY}" "NoRepair" 1

  ; Append install dir to the system PATH (machine scope) if not present.
  ReadRegStr $0 HKLM "SYSTEM\CurrentControlSet\Control\Session Manager\Environment" "Path"
  ${If} $0 == ""
    StrCpy $1 "$INSTDIR"
  ${Else}
    ; Crude contains-check: only append when not already there.
    Push "$0"
    Push "$INSTDIR"
    Call StrContains
    Pop $2
    ${If} $2 == "found"
      StrCpy $1 "$0"
    ${Else}
      StrCpy $1 "$0;$INSTDIR"
    ${EndIf}
  ${EndIf}
  WriteRegExpandStr HKLM "SYSTEM\CurrentControlSet\Control\Session Manager\Environment" "Path" "$1"
  ; Broadcast the environment change so new shells pick it up.
  SendMessage ${HWND_BROADCAST} ${WM_WININICHANGE} 0 "STR:Environment" /TIMEOUT=5000
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\${APP_EXE}"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"

  DeleteRegKey HKLM "${APP_REGKEY}"
  DeleteRegKey HKLM "Software\${APP_NAME}"
  ; Note: the PATH entry is left in place to avoid corrupting a manually edited
  ; PATH; users can remove "$INSTDIR" from PATH manually if desired.
SendMessage ${HWND_BROADCAST} ${WM_WININICHANGE} 0 "STR:Environment" /TIMEOUT=5000
SectionEnd

; StrContains: sets $R0 to "found" if needle (top) is a substring of haystack.
; Stack in:  haystack, needle      Stack out: result
Function StrContains
  Exch $R1 ; needle
  Exch
  Exch $R2 ; haystack
  Push $R3
  Push $R4
  Push $R5
  StrLen $R3 $R1
  StrCpy $R4 0
  StrCpy $R0 "not found"
  loop:
    StrCpy $R5 $R2 $R3 $R4
    StrCmp $R5 "" done
    StrCmp $R5 $R1 found
    IntOp $R4 $R4 + 1
    Goto loop
  found:
    StrCpy $R0 "found"
  done:
    Pop $R5
    Pop $R4
    Pop $R3
    Pop $R2
    Pop $R1
    Exch $R0
FunctionEnd
