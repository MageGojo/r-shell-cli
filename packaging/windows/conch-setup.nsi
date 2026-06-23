; NSIS 安装器 — Conch 桌面版(Windows x64)。
;
; 打包整个 Flutter Release 目录(Conch.exe + flutter/rust DLL + 自带 VC++ 运行时 +
; data/),装到 Program Files\Conch,建开始菜单/桌面快捷方式,注册卸载项。
; 因为 Release 已自带运行时,装完「双击即用」,无需任何额外环境。
;
; CI / 脚本通过 /D 传入:
;   STAGE_DIR    含 Conch.exe 的 Release 目录(必填)
;   OUTPUT_EXE   要产出的安装器完整路径
;   ICON_PATH    安装器与卸载器图标(.ico,可选)
;   APP_VERSION  版本号,如 1.0.0

Unicode true
SetCompressor /SOLID lzma

!ifndef STAGE_DIR
  !error "STAGE_DIR must be defined"
!endif
!ifndef OUTPUT_EXE
  !define OUTPUT_EXE "Conch-windows-x64-setup.exe"
!endif
!ifndef APP_VERSION
  !define APP_VERSION "1.0.0"
!endif

!define APP_NAME "Conch"
!define APP_PUBLISHER "MageGojo"
!define APP_EXE "Conch.exe"
!define APP_REGKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\Conch"

!include "MUI2.nsh"

!ifdef ICON_PATH
  !define MUI_ICON "${ICON_PATH}"
  !define MUI_UNICON "${ICON_PATH}"
!endif

Name "${APP_NAME} ${APP_VERSION}"
OutFile "${OUTPUT_EXE}"
InstallDir "$PROGRAMFILES64\${APP_NAME}"
InstallDirRegKey HKLM "Software\${APP_NAME}" "InstallDir"
RequestExecutionLevel admin

VIProductVersion "${APP_VERSION}.0"
VIAddVersionKey "ProductName" "${APP_NAME}"
VIAddVersionKey "FileDescription" "${APP_NAME} — AI-native SSH/ADB terminal"
VIAddVersionKey "FileVersion" "${APP_VERSION}"
VIAddVersionKey "ProductVersion" "${APP_VERSION}"
VIAddVersionKey "CompanyName" "${APP_PUBLISHER}"
VIAddVersionKey "LegalCopyright" "MIT License"

!define MUI_ABORTWARNING
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\${APP_EXE}"
!define MUI_FINISHPAGE_RUN_TEXT "立即启动 ${APP_NAME}"
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "SimpChinese"
!insertmacro MUI_LANGUAGE "English"

Section "Install"
  SetOutPath "$INSTDIR"
  ; 递归打包整个 Release(含运行时 DLL 与 data/)。
  File /r "${STAGE_DIR}\*.*"

  WriteRegStr HKLM "Software\${APP_NAME}" "InstallDir" "$INSTDIR"

  CreateDirectory "$SMPROGRAMS\${APP_NAME}"
  CreateShortcut "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}"
  CreateShortcut "$DESKTOP\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}"

  WriteUninstaller "$INSTDIR\uninstall.exe"
  WriteRegStr   HKLM "${APP_REGKEY}" "DisplayName"     "${APP_NAME}"
  WriteRegStr   HKLM "${APP_REGKEY}" "DisplayVersion"  "${APP_VERSION}"
  WriteRegStr   HKLM "${APP_REGKEY}" "Publisher"       "${APP_PUBLISHER}"
  WriteRegStr   HKLM "${APP_REGKEY}" "DisplayIcon"     "$INSTDIR\${APP_EXE}"
  WriteRegStr   HKLM "${APP_REGKEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr   HKLM "${APP_REGKEY}" "UninstallString" "$\"$INSTDIR\uninstall.exe$\""
  WriteRegDWORD HKLM "${APP_REGKEY}" "NoModify" 1
  WriteRegDWORD HKLM "${APP_REGKEY}" "NoRepair" 1
SectionEnd

Section "Uninstall"
  Delete "$DESKTOP\${APP_NAME}.lnk"
  Delete "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk"
  RMDir  "$SMPROGRAMS\${APP_NAME}"
  RMDir /r "$INSTDIR"
  DeleteRegKey HKLM "${APP_REGKEY}"
  DeleteRegKey HKLM "Software\${APP_NAME}"
SectionEnd
