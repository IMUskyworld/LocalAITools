# LocalMind production build (executable only, no NSIS installer)
# 路径基于本脚本所在目录（$PSScriptRoot）推导，适用于任意 checkout 位置。
# 原生工具链（Rust/CMake/MSVC/libclang）按下面变量配置，请按本机实际路径调整。
$ErrorActionPreference = "Stop"

# ---- 原生工具链路径（可用同名环境变量覆盖；CONTEXT.md 记录 E:\APP / E:\Visual Studio）----
$env:LIBCLANG_PATH = if ($env:LIBCLANG_PATH) { $env:LIBCLANG_PATH } else { "E:\APP\Lib\site-packages\clang\native" }
$CargoBin = if ($env:LOCALMIND_CARGO_BIN) { $env:LOCALMIND_CARGO_BIN } else { "C:\Users\world\.cargo\bin" }
$CmakeBin = if ($env:LOCALMIND_CMAKE_BIN) { $env:LOCALMIND_CMAKE_BIN } else { "E:\APP\CLion 2026.2.1\bin\cmake\win\x64\bin" }
$MsvcBin  = if ($env:LOCALMIND_MSVC_BIN)  { $env:LOCALMIND_MSVC_BIN }  else { "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC\14.44.35207\bin\Hostx64\x64" }
$env:Path  = "$CargoBin;$CmakeBin;$MsvcBin;$env:Path"
if (-not $env:TEMP) { $env:TEMP = (Join-Path $env:USERPROFILE "AppData\Local\Temp") }
$env:TMP = $env:TEMP

# ---- 关键：注入 DeepSeek key（编译期烘焙进 exe，仓库源码不含明文 key）----
if ([string]::IsNullOrEmpty($env:LOCALMIND_DEEPSEEK_KEY)) {
    $env:LOCALMIND_DEEPSEEK_KEY = [Environment]::GetEnvironmentVariable("LOCALMIND_DEEPSEEK_KEY", "User")
}
if ([string]::IsNullOrEmpty($env:LOCALMIND_DEEPSEEK_KEY)) {
    throw "LOCALMIND_DEEPSEEK_KEY 未设置。请先运行：[Environment]::SetEnvironmentVariable('LOCALMIND_DEEPSEEK_KEY','<你的key>','User')"
}
Write-Host "LOCALMIND_DEEPSEEK_KEY 已注入（长度 $($env:LOCALMIND_DEEPSEEK_KEY.Length)；仅构建期读取，不进源码）"

# ---- 路径（基于脚本位置）----
$ProjectRoot = Join-Path $PSScriptRoot "localmind"
$CrateDir    = Join-Path $ProjectRoot "src-tauri"
$ReleaseDir  = Join-Path $CrateDir "target\release"
$OutputExe   = Join-Path $PSScriptRoot "LocalMind.exe"
$ScriptsDir  = Join-Path $PSScriptRoot "LocalMindScripts"

Write-Host "=== Step 2: Build production executable through Tauri CLI ==="
$env:CARGO_TARGET_DIR = Join-Path $CrateDir "target"
Write-Host "CARGO_TARGET_DIR: $env:CARGO_TARGET_DIR"
Set-Location $ProjectRoot
# Important: raw `cargo build --release` embeds build.devUrl and makes the EXE
# navigate to localhost:1420. Tauri CLI sets the production build environment.
npx tauri build --no-bundle
if ($LASTEXITCODE -ne 0) { throw "Tauri production build failed" }

Write-Host "=== Step 3: Publish executable ==="
$BuiltExe = Join-Path $ReleaseDir "localmind.exe"
if (-not (Test-Path -LiteralPath $BuiltExe)) {
    $LegacyName = Join-Path $ReleaseDir "LocalMind.exe"
    if (Test-Path -LiteralPath $LegacyName) { $BuiltExe = $LegacyName }
}
if (-not (Test-Path -LiteralPath $BuiltExe)) { throw "EXE not found under $ReleaseDir (expected localmind.exe or LocalMind.exe)" }
Copy-Item -LiteralPath $BuiltExe -Destination $OutputExe -Force
$size = (Get-Item -LiteralPath $OutputExe).Length / 1MB
Write-Host "exe size: $([math]::Round($size, 1)) MB"
Write-Host "OK: $OutputExe"

Write-Host ""
Write-Host "=== Step 4: Publish make_doc.exe (document generator, no Python needed) ==="
$DocExe = Join-Path $ProjectRoot "scripts\dist\make_doc.exe"
if (-not (Test-Path -LiteralPath $DocExe)) { throw "make_doc.exe not found: $DocExe（先运行 python scripts/pack_doc_exe.py）" }
if (-not (Test-Path -LiteralPath $ScriptsDir)) { New-Item -ItemType Directory -Path $ScriptsDir | Out-Null }
Copy-Item -LiteralPath $DocExe -Destination (Join-Path $ScriptsDir "make_doc.exe") -Force
$docSize = (Get-Item -LiteralPath (Join-Path $ScriptsDir "make_doc.exe")).Length / 1MB
Write-Host "make_doc.exe size: $([math]::Round($docSize, 1)) MB"
Write-Host "OK: $ScriptsDir\make_doc.exe"

Write-Host ""
Write-Host "=== Step 5: Publish localmind-agent (Pydantic AI agent service, onedir) ==="
$AgentDir = Join-Path $ProjectRoot "src-tauri\scripts\localmind-agent"
$AgentExe = Join-Path $AgentDir "localmind-agent.exe"
if (-not (Test-Path -LiteralPath $AgentExe)) { throw "localmind-agent.exe not found: $AgentExe（先运行 python scripts/pack_agent_exe.py）" }
if (-not (Test-Path -LiteralPath $ScriptsDir)) { New-Item -ItemType Directory -Path $ScriptsDir | Out-Null }
$DestAgent = Join-Path $ScriptsDir "localmind-agent"
if (Test-Path -LiteralPath $DestAgent) { Remove-Item -LiteralPath $DestAgent -Recurse -Force }
Copy-Item -LiteralPath $AgentDir -Destination $DestAgent -Recurse -Force
$agentSize = (Get-Item -LiteralPath $AgentExe).Length / 1MB
Write-Host "localmind-agent.exe size: $([math]::Round($agentSize, 1)) MB"
Write-Host "OK: $DestAgent"

Write-Host ""
Write-Host "=== 发布文件夹内容 ==="
Write-Host "  $OutputExe"
Write-Host "  $ScriptsDir\make_doc.exe"
Write-Host "  $ScriptsDir\localmind-agent\"
Write-Host "把 LocalMind.exe + LocalMindScripts\ 一起压缩发给别人，解压即用（无需安装 Python）"
Write-Host "=== DONE ==="
