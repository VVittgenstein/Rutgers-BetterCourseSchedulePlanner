#!/usr/bin/env bash
cd "$(cd "$(dirname "$0")" && pwd)" || exit 1
echo "Starting BetterCourseSchedulePlanner (web UI)..."
if ! command -v node >/dev/null 2>&1; then
  echo "Node.js is not installed or not on PATH. Opening the download page..."
  if command -v open >/dev/null 2>&1; then
    open "https://nodejs.org/en/download/"
  elif command -v xdg-open >/dev/null 2>&1; then
    xdg-open "https://nodejs.org/en/download/"
  else
    echo "Please visit https://nodejs.org/en/download/ to install Node.js."
  fi
  echo "Install Node.js, reopen this window, and rerun the launcher."
  exit 1
fi
node "./scripts/oneclick_start.js"
status=$?
if [ "$status" -ne 0 ]; then
  echo "Launcher exited with an error. See logs above (exit code $status)."
fi
exit "$status"
