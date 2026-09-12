; LocalMind NSIS installer script
; Build: makensis installer.nsi
; Produces: LocalMindSetup.exe
; Installs LocalMind.exe + scripts\make_doc.exe to %LOCALAPPDATA%\LocalMind
;
; 重要：本安装包【不再内置任何 API Key】。
; 用户首次启动后在「设置」页填写自己的 DeepSeek API Key（保存到 auth.json）。

Unicode true
!include "MUI2.nsh"
!include "FileFunc.nsh"
!include "WinMessages.nsh"

; ---------- Product info ----------
!define APP_NAME "LocalMind"
!define APP_VERSION "0.3.0"
!define APP_ID "com.localmind.app"
!define INSTALL_DIR "$LOCALAPPDATA\LocalMind"
!define DATA_DIR "$APPDATA\LocalMind"

!ifndef STAGE_DIR
  !define STAGE_DIR "..\LocalMindSetupBuild"
!endif

Name "${APP_NAME}"
OutFile "LocalMindSetup.exe"
InstallDir "${INSTALL_DIR}"
RequestExecutionLevel user
SetCompressor /SOLID lzma
CRCCheck on

; ---------- Modern UI 2 ----------
!define MUI_ICON "${STAGE_DIR}\icon.ico"
!define MUI_UNICON "${STAGE_DIR}\icon.ico"
!define MUI_ABORTWARNING

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES

!define MUI_FINISHPAGE_RUN "$INSTDIR\LocalMind.exe"
!define MUI_FINISHPAGE_RUN_TEXT "立即运行 LocalMind"
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "SimpChinese"
!insertmacro MUI_LANGUAGE "English"

; ---------- Install ----------
Section "Install" SecMain
    ; ---- 每次安装都从干净状态开始（用户要求：安装后创建新的记忆文档）----
    ; 清理上一次安装残留的会话/记忆/API Key 配置，避免旧数据污染新版本
    Delete "${DATA_DIR}\auth.json"
    Delete "${DATA_DIR}\localmind.db"
    Delete "${DATA_DIR}\localmind.db-wal"
    Delete "${DATA_DIR}\localmind.db-shm"
    RMDir /r "${DATA_DIR}\traces"

    SetOutPath "${INSTALL_DIR}"

    ; Main executable
    File "${STAGE_DIR}\LocalMind.exe"

    ; Document generator (scripts subfolder - matches find_doc_exe lookup)
    SetOutPath "${INSTALL_DIR}\scripts"
    File "${STAGE_DIR}\scripts\make_doc.exe"

    ; Pydantic AI Agent service (onedir - recursive)
    SetOutPath "${INSTALL_DIR}\scripts\localmind-agent"
    File /r "${STAGE_DIR}\scripts\localmind-agent\*.*"

    ; ---- 清理历史版本写入的环境变量（旧安装包曾把 key 写进注册表）----
    ; 现在改为用户在设置页自填，安装包不再注入任何 key。
    DeleteRegValue HKCU "Environment" "LOCALMIND_DEEPSEEK_KEY"
    SendMessage ${HWND_BROADCAST} ${WM_WININICHANGE} 0 "STR:Environment" /TIMEOUT=5000

    ; Shortcuts
    SetOutPath "${INSTALL_DIR}"
    CreateShortCut "$DESKTOP\${APP_NAME}.lnk" "${INSTALL_DIR}\LocalMind.exe"
    CreateDirectory "$SMPROGRAMS\${APP_NAME}"
    CreateShortCut "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk" "${INSTALL_DIR}\LocalMind.exe"
    CreateShortCut "$SMPROGRAMS\${APP_NAME}\Uninstall.lnk" "${INSTALL_DIR}\uninstall.exe"

    ; Uninstaller
    WriteUninstaller "${INSTALL_DIR}\uninstall.exe"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "DisplayName" "${APP_NAME}"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "DisplayVersion" "${APP_VERSION}"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "Publisher" "LocalMind"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "DisplayIcon" "${INSTALL_DIR}\LocalMind.exe"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "UninstallString" "${INSTALL_DIR}\uninstall.exe"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "InstallLocation" "${INSTALL_DIR}"
    ${GetSize} "${INSTALL_DIR}" "/S=0K" $0 $1 $2
    IntFmt $0 "0x%08X" $0
    WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "EstimatedSize" "$0"
SectionEnd

; ---------- Uninstall ----------
Section "Uninstall"
    ; Remove shortcuts
    Delete "$DESKTOP\${APP_NAME}.lnk"
    RMDir /r "$SMPROGRAMS\${APP_NAME}"

    ; Remove installed files
    Delete "${INSTALL_DIR}\LocalMind.exe"
    Delete "${INSTALL_DIR}\scripts\make_doc.exe"
    RMDir /r "${INSTALL_DIR}\scripts\localmind-agent"
    RMDir "${INSTALL_DIR}\scripts"
    Delete "${INSTALL_DIR}\uninstall.exe"
    RMDir "${INSTALL_DIR}"

    ; ---- 删除应用数据（记忆文档 / 会话 / API key 配置）----
    ; auth.json、localmind.db（Session/Message/Memory 落库）、traces/
    Delete "${DATA_DIR}\auth.json"
    Delete "${DATA_DIR}\localmind.db"
    Delete "${DATA_DIR}\localmind.db-wal"
    Delete "${DATA_DIR}\localmind.db-shm"
    RMDir /r "${DATA_DIR}\traces"
    RMDir "${DATA_DIR}"

    ; Remove the legacy injected API key from environment (历史的旧安装包写过)
    DeleteRegValue HKCU "Environment" "LOCALMIND_DEEPSEEK_KEY"
    SendMessage ${HWND_BROADCAST} ${WM_WININICHANGE} 0 "STR:Environment" /TIMEOUT=5000

    ; Remove registry
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}"
SectionEnd
