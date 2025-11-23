#!/usr/bin/env bash
cd "$(cd "$(dirname "$0")" && pwd)" || exit 1
echo "Starting BetterCourseSchedulePlanner (web UI)..."
node "./scripts/oneclick_start.js"
status=$?
if [ "$status" -ne 0 ]; then
  echo "Launcher exited with an error. See logs above (exit code $status)."
fi
exit "$status"
