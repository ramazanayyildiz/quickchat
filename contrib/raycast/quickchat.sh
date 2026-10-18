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

# A running QuickChat takes the request whichever copy sends it, so the
# order only picks what starts when none runs: the installed app first
# (release.sh puts it in ~/Applications), the development bundle last.
# Never a bare target/ binary: without the quickchat feature that is ZapFast.
if [ -n "${1:-}" ]; then
  args=(--quick-query "$1")
else
  args=(--quick)
fi

for bin in "$app" "$system_app" "$project_app"; do
  if [ -x "$bin" ]; then
    # QuickChat is a resident app: Raycast must not wait for the GUI process.
    # If an instance is already running, this child forwards the request to it.
    nohup "$bin" "${args[@]}" </dev/null >/dev/null 2>&1 &
    exit 0
  fi
done

echo "QuickChat is not built. See docs/quickchat-local.md." >&2
exit 1
