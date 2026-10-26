#!/usr/bin/env bash
set -euo pipefail
# Run under dbus-run-session and Xvfb in CI; do not touch a real media library.
dbus-update-activation-environment DISPLAY XAUTHORITY
task_log=$(mktemp)
flatpak run --env=LIBGL_ALWAYS_SOFTWARE=1 --env=MEDIA_MANAGER_STATE_DIR=/tmp/media-manager-smoke com.tvshowmanager.app >"$task_log" 2>&1 &
task_pid=$!
trap 'kill "$task_pid" 2>/dev/null || true' EXIT
for task_attempt in $(seq 1 30); do
  if ! kill -0 "$task_pid" 2>/dev/null; then
    cat "$task_log"
    echo 'Flatpak exited before opening its window' >&2
    exit 1
  fi
  if xdotool search --onlyvisible --name '^Media Manager$' >/dev/null 2>&1; then
    # Let the splash transition and WebKit initialization complete as well.
    sleep 5
    kill -0 "$task_pid"
    task_portal=$(gdbus introspect --session --dest org.freedesktop.portal.Desktop --object-path /org/freedesktop/portal/desktop)
    if ! grep -q 'org.freedesktop.portal.FileChooser' <<<"$task_portal"; then
      cat "$task_log"
      echo 'The desktop file chooser portal is unavailable' >&2
      exit 1
    fi
    cat "$task_log"
    echo 'Media Manager opened successfully inside the managed GNOME runtime; file chooser portal is available'
    exit 0
  fi
  sleep 1
done
cat "$task_log"
echo 'Flatpak did not open a visible application window' >&2
exit 1
