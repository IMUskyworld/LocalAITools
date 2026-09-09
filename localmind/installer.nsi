; LocalMind NSIS installer script
; Build: makensis installer.nsi
; Produces: LocalMindSetup.exe
; Installs LocalMind.exe + scripts\make_doc.exe to %LOCALAPPDATA%\LocalMind

Unicode true
!include "MUI2.nsh"
!include "FileFunc.nsh"
!include "WinMessages.nsh"

; ---------- Product info ----------
!define APP_NAME "LocalMind"
!define APP_VERSION "1.0.0"
!define APP_ID "com.localmind.app"
!define INSTALL_DIR "$LOCALAPPDATA\LocalMind"

; ---------- DeepSeek API key（随包分发，开箱即用）----------
; 说明：Rust 端 deepseek_api_key() 优先读运行时环境变量 LOCALMIND_DEEPSEEK_KEY，
; 未设置才回退编译期烘焙值。这里在安装时写入「用户环境变量」，
; 因此无需重新编译 Rust 即可让任何机器装完即用。
; 安全警示：此 key 以明文编入安装包，任何拿到 LocalMindSetup.exe 的人都可提取。
; 请仅使用低额度备用 key；若需更换，改这一行后重新执行 build-installer.ps1 即可。
; 若本文件会 push 到公开仓库，请先作废该 key。
!ifndef DEEPSEEK_KEY
  !define DEEPSEEK_KEY "sk-ee34da1ded9c46cdb920b43e45fa3a0a"
!endif

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

; 完成页「立即运行」：安装进程已注入 LOCALMIND_DEEPSEEK_KEY 环境变量，
; 由此启动的 LocalMind 会继承它，装完即可直接对话。
!define MUI_FINISHPAGE_RUN "$INSTDIR\LocalMind.exe"
!define MUI_FINISHPAGE_RUN_TEXT "立即运行 LocalMind"
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "SimpChinese"
!insertmacro MUI_LANGUAGE "English"

; ---------- Install ----------
Section "Install" SecMain
    SetOutPath "${INSTALL_DIR}"

    ; Main executable
    File "${STAGE_DIR}\LocalMind.exe"

    ; Document generator (scripts subfolder - matches find_doc_exe lookup)
    SetOutPath "${INSTALL_DIR}\scripts"
    File "${STAGE_DIR}\scripts\make_doc.exe"

    ; Pydantic AI Agent service (onedir - recursive)
    ; 装在 scripts\localmind-agent 下，命中 find_agent_exe 的 scripts/ 候选路径
    SetOutPath "${INSTALL_DIR}\scripts\localmind-agent"
    File /r "${STAGE_DIR}\scripts\localmind-agent\*.*"

    ; ---- 注入 DeepSeek key（三重保险，确保任何启动方式都读得到）----
    ; 1) 写入用户级环境变量：永久生效，重启/重登录后所有进程均可见
    WriteRegExpandStr HKCU "Environment" "LOCALMIND_DEEPSEEK_KEY" "${DEEPSEEK_KEY}"

    ; 2) 广播 WM_WININICHANGE：通知 Explorer 等已运行进程刷新环境，
    ;    这样从桌面 / 开始菜单快捷方式启动也能立刻读到，无需注销
    SendMessage ${HWND_BROADCAST} ${WM_WININICHANGE} 0 "STR:Environment" /TIMEOUT=5000

    ; 3) 设置安装进程自身环境：完成页「立即运行」启动的 LocalMind 直接继承
    System::Call 'kernel32::SetEnvironmentVariable(t "LOCALMIND_DEEPSEEK_KEY", t "${DEEPSEEK_KEY}")'

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

    ; Remove the injected API key (卸载时不残留凭证)
    DeleteRegValue HKCU "Environment" "LOCALMIND_DEEPSEEK_KEY"
    SendMessage ${HWND_BROADCAST} ${WM_WININICHANGE} 0 "STR:Environment" /TIMEOUT=5000

    ; Remove registry
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}"
SectionEnd
