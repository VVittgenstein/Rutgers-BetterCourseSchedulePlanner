@echo off
setlocal
cd /d "%~dp0"
echo Starting BetterCourseSchedulePlanner (web UI)...
node "scripts\\oneclick_start.js"
if errorlevel 1 (
  echo Launcher exited with an error. Scroll up for details.
  pause
  exit /b %errorlevel%
)
echo Launcher stopped. Press any key to close this window.
pause
