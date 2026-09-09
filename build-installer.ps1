# LocalMind NSIS installer build (setup.exe)
# 路径基于本脚本所在目录（$PSScriptRoot）推导。NSIS 通过 -DSTAGE_DIR 把暂存目录传给 installer.nsi。
$ErrorActionPreference = "Stop"
$ProjectRoot = Join-Path $PSScriptRoot "localmind"
$NSIS = if ($env:LOCALMIND_NSIS_BIN) { $env:LOCALMIND_NSIS_BIN } else { "C:\Users\world\AppData\Local\tauri\nsis-3.11\Bin\makensis.exe" }
$StageDir    = Join-Path $PSScriptRoot "LocalMindSetupBuild"
$OutputSetup = Join-Path $PSScriptRoot "LocalMindSetup.exe"
$Standalone  = Join-Path $PSScriptRoot "LocalMind.exe"
$ScriptsDir  = Join-Path $PSScriptRoot "LocalMindScripts"

Write-Host "=== Step 1: Verify artifacts ==="
if (-not (Test-Path -LiteralPath $Standalone)) { throw "LocalMind.exe not found - run build-localmind.ps1 first" }
if (-not (Test-Path -LiteralPath (Join-Path $ScriptsDir "make_doc.exe"))) { throw "make_doc.exe not found - run pack_doc_exe.py first" }
if (-not (Test-Path -LiteralPath (Join-Path $ScriptsDir "localmind-agent\localmind-agent.exe"))) { throw "localmind-agent not found - run pack_agent_exe.py + build-localmind.ps1 first" }
if (-not (Test-Path -LiteralPath $NSIS)) { throw "makensis not found at $NSIS（可设环境变量 LOCALMIND_NSIS_BIN 覆盖）" }

Write-Host "=== Step 2: Stage files ==="
if (Test-Path -LiteralPath $StageDir) { Remove-Item -LiteralPath $StageDir -Recurse -Force }
New-Item -ItemType Directory -Path (Join-Path $StageDir "scripts") -Force | Out-Null
Copy-Item -LiteralPath $Standalone -Destination (Join-Path $StageDir "LocalMind.exe") -Force
Copy-Item -LiteralPath (Join-Path $ScriptsDir "make_doc.exe") -Destination (Join-Path $StageDir "scripts\make_doc.exe") -Force
Copy-Item -LiteralPath (Join-Path $ScriptsDir "localmind-agent") -Destination (Join-Path $StageDir "localmind-agent") -Recurse -Force
Copy-Item -LiteralPath (Join-Path $ProjectRoot "src-tauri\icons\icon.ico") -Destination (Join-Path $StageDir "icon.ico") -Force
Write-Host "Staged LocalMind.exe + scripts/make_doc.exe + localmind-agent + icon.ico"

Write-Host "=== Step 3: Compile NSIS installer ==="
& $NSIS "-DSTAGE_DIR=$StageDir" (Join-Path $ProjectRoot "installer.nsi")
if ($LASTEXITCODE -ne 0) { throw "makensis failed with exit code $LASTEXITCODE" }

Write-Host "=== Step 4: Publish setup.exe ==="
Copy-Item -LiteralPath (Join-Path $ProjectRoot "LocalMindSetup.exe") -Destination $OutputSetup -Force
$size = (Get-Item -LiteralPath $OutputSetup).Length / 1MB
Write-Host ("OK: " + $OutputSetup + " (" + [math]::Round($size, 1) + " MB)")

Write-Host ""
Write-Host "=== Artifacts ==="
Write-Host ("  Installer: " + $OutputSetup + " (send this to others)")
Write-Host "  Standalone exe: $Standalone"
Write-Host "=== DONE ==="