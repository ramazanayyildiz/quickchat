#!/bin/bash
# @raycast.schemaVersion 1
# @raycast.title QuickChat
# @raycast.mode silent
# @raycast.packageName QuickChat
# @raycast.icon 💬
# @raycast.description Find a WhatsApp contact or group in QuickChat
# @raycast.argument1 { "type": "text", "placeholder": "Contact / group name", "optional": true }

set -euo pipefail

# Raycast invokes the same trusted local IPC command as the native hotkey.
# No contacts, message bodies, or authentication secrets travel through Raycast.
app="$HOME/Applications/QuickChat.app/Contents/MacOS/quickchat"
system_app="/Applications/QuickChat.app/Contents/MacOS/quickchat"
project_app="$HOME/CODE/Ram/quickchat/QuickChat.app/Contents/MacOS/quickchat"
built="$HOME/CODE/Ram/quickchat/target/debug/quickchat"

# Development build first, so an older installed copy cannot swallow the
# new --quick-query argument during testing.
if [ -n "${1:-}" ]; then
  args=(--quick-query "$1")
else
  args=(--quick)
fi

for bin in "$project_app" "$app" "$system_app" "$built"; do
  if [ -x "$bin" ]; then
    # QuickChat is a resident app: Raycast must not wait for the GUI process.
    # If an instance is already running, this child forwards the request to it.
    nohup "$bin" "${args[@]}" </dev/null >/dev/null 2>&1 &
    exit 0
  fi
done

echo "QuickChat is not built. See docs/quickchat-local.md." >&2
exit 1
