; IDM Master — NSIS Installer Script
; Build: makensis /DAPP_VERSION=0.1.0 installer.nsi

!include "MUI2.nsh"
!include "FileFunc.nsh"

; ── Configuration ──
!define APP_NAME "IDM Master"
!define APP_EXE "idm-master-tauri.exe"
!define APP_PUBLISHER "IDM Master"
!define APP_URL "https://github.com/BrotherCao/IDM-Master"
!define APP_VERSION "0.1.0"
!define APP_BUILD_DIR "..\src-tauri\target\release"
!define EXTENSION_DIR "..\extension"

Name "${APP_NAME} ${APP_VERSION}"
OutFile "..\dist\IDM-Master-Setup-${APP_VERSION}.exe"
InstallDir "$PROGRAMFILES64\${APP_NAME}"
RequestExecutionLevel admin
SetCompressor /SOLID lzma

; ── Modern UI ──
!define MUI_ICON "${APP_BUILD_DIR}\..\icons\icon.ico"
!define MUI_UNICON "${APP_BUILD_DIR}\..\icons\icon.ico"
!define MUI_ABORTWARNING

; Pages
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "..\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "SimpChinese"

; ── Install Section ──
Section "IDM Master" SecMain
  SetOutPath "$INSTDIR"

  ; Main executable
  File "${APP_BUILD_DIR}\${APP_EXE}"
  File "${APP_BUILD_DIR}\*.dll"

  ; WebView2 loader (Tauri 依赖)
  SetOutPath "$INSTDIR\Microsoft.Web.WebView2"
  File /nonfatal "${APP_BUILD_DIR}\WebView2Loader.dll"

  ; Chrome Extension
  SetOutPath "$INSTDIR\extension"
  File /r "${EXTENSION_DIR}\*.*"

  ; Create shortcuts
  CreateDirectory "$SMPROGRAMS\${APP_NAME}"
  CreateShortCut "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}"
  CreateShortCut "$SMPROGRAMS\${APP_NAME}\卸载 IDM Master.lnk" "$INSTDIR\uninstall.exe"
  CreateShortCut "$DESKTOP\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}"

  ; Write uninstaller
  WriteUninstaller "$INSTDIR\uninstall.exe"

  ; Registry for Add/Remove Programs
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" \
    "DisplayName" "${APP_NAME}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" \
    "UninstallString" "$\"$INSTDIR\uninstall.exe$\""
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" \
    "DisplayIcon" "$\"$INSTDIR\${APP_EXE}$\""
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" \
    "Publisher" "${APP_PUBLISHER}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" \
    "URLInfoAbout" "${APP_URL}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" \
    "DisplayVersion" "${APP_VERSION}"
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" \
    "NoModify" 1
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" \
    "NoRepair" 1

  ; Estimate size
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" \
    "EstimatedSize" "$0"
SectionEnd

; ── Uninstall Section ──
Section "Uninstall"
  ; Remove shortcuts
  Delete "$DESKTOP\${APP_NAME}.lnk"
  RMDir /r "$SMPROGRAMS\${APP_NAME}"

  ; Remove files
  RMDir /r "$INSTDIR\extension"
  Delete "$INSTDIR\${APP_EXE}"
  Delete "$INSTDIR\*.dll"
  RMDir /r "$INSTDIR\Microsoft.Web.WebView2"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"

  ; Remove registry
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}"

  ; Remove user data (optional — ask user)
  MessageBox MB_YESNO "是否删除所有下载历史和设置？" IDNO skipUserData
    RMDir /r "$APPDATA\IDM-Master"
  skipUserData:
SectionEnd
