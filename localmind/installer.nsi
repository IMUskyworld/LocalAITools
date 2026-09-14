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
!define APP_VERSION "0.3.2"
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
    ; ---- 安装【不再】清空用户数据 ----
    ; 历史版本在这里删掉过 auth.json 与 localmind.db，导致「覆盖安装 = 丢 API Key +
    ; 丢全部会话记录 + 丢记忆文档」。旧数据污染的根因（安装包把 key 写进注册表）
    ; 已经修掉，数据库也有 migration，所以升级必须保留用户数据。
    ; 记忆文档本身是「缺了才创建」，卸载时才删除（见 Uninstall 段）。

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
    ; 长期记忆文档（含整理前备份）：用户要求卸载时一并删除
    Delete "${DATA_DIR}\memory.md"
    Delete "${DATA_DIR}\memory.md.bak"
    RMDir /r "${DATA_DIR}\traces"
    ; 用 /r 兜底：否则目录里只要还有任何文件（例如未来的新文件）就删不掉，
    ; 会出现「卸载后 %APPDATA%\LocalMind 残留」
    RMDir /r "${DATA_DIR}"

    ; Remove the legacy injected API key from environment (历史的旧安装包写过)
    DeleteRegValue HKCU "Environment" "LOCALMIND_DEEPSEEK_KEY"
    SendMessage ${HWND_BROADCAST} ${WM_WININICHANGE} 0 "STR:Environment" /TIMEOUT=5000

    ; Remove registry
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}"
SectionEnd
