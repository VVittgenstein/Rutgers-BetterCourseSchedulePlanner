@echo off
setlocal
cd /d "%~dp0"
echo Starting BetterCourseSchedulePlanner (web UI)...
where node >nul 2>&1
if errorlevel 1 (
  echo Node.js is not installed or not on PATH. Opening the download page...
  start "" "https://nodejs.org/en/download/"
  echo Please install Node.js, reopen this window, and run the launcher again.
  pause
  exit /b 1
)
node "scripts\\oneclick_start.js"
if errorlevel 1 (
  echo Launcher exited with an error. Scroll up for details.
  pause
  exit /b %errorlevel%
)
echo Launcher stopped. Press any key to close this window.
pause
