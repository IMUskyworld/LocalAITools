@echo off
REM LocalMind dev startup script
REM Kills stale vite processes, then starts fresh

echo [LocalMind Dev] Killing stale vite/node processes...
taskkill /f /im node.exe 2>nul >nul
timeout /t 2 /nobreak >nul

echo [LocalMind Dev] Starting Vite dev server...
start /b "" "cmd /c npx vite --port 1420 --strictPort"
