# QuickChat macOS Pilot

## Scope

QuickChat is built from this repository with `--features quickchat`. Without
the feature the same source builds ZapFast unchanged. `src/identity.rs` holds
every name, folder, keyring service and instance slot that differs, and
`src/quickchat/` holds the launcher; `app.rs` only calls into it.

QuickChat is an independent macOS WhatsApp companion fork of ZapFast. It does
not reuse ZapFast/FastsApp's app bundle identifier, account data directory,
archive Keychain service or single-instance slot. No archive/phone data was
inspected to develop this feature. ZapFast remains an unofficial WhatsApp
client; using an unofficial client has account-suspension risk.

Current pilot includes:

- **First-use pairing gate:** before WhatsApp has linked, the shortcut
  opens the full onboarding/QR screen, not an empty Quick Search panel. A
  transient transport `Connecting` state alone does not count as paired.
  If QR/pairing-code delivery stalls, a visible retry option appears after
  15 seconds, with a phone-number pairing option where applicable.
- **Control+Option+W** opens the built-in name search while QuickChat runs
  as a bundled app. The bundle is a menu-bar utility without a Dock icon.
  Settings > System > Quick Search shortcut picks another combination
  (⌃⌥Space, ⌥Space, ⌃⌥K) or turns it off; the change applies at once and
  the menu-bar entry names the current one. When another app holds the
  chosen keys, the launcher's footer says so. Settings hides ZapFast's update
  switches, since QuickChat turns updates off.
- **`quickchat --quick`** opens the same search, including from Raycast.
- **`quickchat --quick-query "Ahmet"`** opens it with a name filter already
  filled. The query is never converted to an automatic recipient or message.
- Search over synced person names, group titles and available contacts; matches
  Turkish accented names case-insensitively.
- From two typed characters the launcher also lists matching messages
  under **Messages**, newest first, with the chat, the sender in a group and
  the matching line. It is the same local archive search as the chat list's,
  run in every linked account; locked chats stay out. Enter opens the panel
  at that message. Nothing leaves the Mac.
- Locked conversations are excluded; unresolved privacy-ID-only contacts are
  not offered as new chats.
- With an empty field the launcher lists recent chats, unread ones first,
  then pinned and favourite ones, then the rest, each newest first; arrow keys wrap
  around the list and Enter opens the selected conversation in a narrow
  panel at the right of the screen.
- **Tab writes to the selected chat without opening it.** The launcher shows
  the recipient's name and picture over a message field: Enter sends and
  closes, Shift+Enter starts a new line, Escape goes back to the list and
  keeps the text (picking another chat drops it, so it never goes to the
  wrong person). Only existing chats that take your messages offer this; a
  contact without a chat, or an admins-only group, opens in the panel as
  before. The app lock blocks it like any other send, and the open chat's
  draft or an edit under way there is left alone.
- The launcher and the panel open on the screen the pointer is on, inside
  the area the menu bar and the Dock leave free.
- In the launcher, Escape clears a typed name first and closes on the next
  press. In the panel it cancels whatever is in progress (a reply, a
  dialog, a selection) and otherwise hides the panel; the chat header's
  Back arrow returns to the search. The WhatsApp connection stays resident.
- Clicking the menu-bar icon opens its menu: Quick Search, Open QuickChat
  (the full window, brought to the front), Lock and Quit. While the full
  window is open QuickChat has a Dock icon and appears in Cmd-Tab; with only
  the launcher, the panel or no window it is a menu-bar utility again.
- The menu-bar icon carries a dot while any chat is unread, and its tooltip
  gives the count: QuickChat has no Dock badge while it is a menu-bar utility.
- A click on a macOS message notification requests the panel for that message.
- Automatic/manual vendor application update installation is disabled in the fork.

## Build without touching a real WhatsApp account

Requirements: macOS 11+, Apple Silicon, Rust 1.98+, CMake, Xcode command line
tools. Initial build of the original pinned dependencies needs disk space.

```sh
cd ~/CODE/Ram/quickchat
brew install cmake
cargo fmt --all
cargo test --lib --features quickchat quickchat::
cargo test --lib single_instance::tests
cargo test --locked --all-targets
```

Do not run an actual WhatsApp pairing until the above gates pass. For
synthetic-only UI inspection, use the upstream demo data. The `waiting` page
reproduces the original QR-loading state without a network connection;
the manual retry control appears after 15 seconds:

```sh
cargo run --features demo,quickchat -- --demo --quick
cargo run --features demo,quickchat -- --demo --quick --demo-page waiting
```

If the demo renders correctly, build a **synthetic-data** macOS bundle
to test the native shortcut, position and close behavior without pairing a
WhatsApp account:

```sh
cargo build --locked --features demo,quickchat
packaging/macos/quickchat/bundle.sh target/debug/zapfast QuickChat.app 0.19.0
open QuickChat.app --args --demo --quick
```

With QuickChat open, check the status text at the bottom of the search window.
It reports whether the hotkey registered or received a key press. Focus Finder
and try Control+Option+W. If it fails, use the QuickChat menu-bar icon and
select Quick Search as a reliable alternative. Choose a synthetic contact; the
right panel should open. Press Escape: the panel should close without exiting
QuickChat. The native shortcut in this test is opt-in, because demo runs without
`--quick` never register the hotkey. This synthetic run deliberately skips the
running-instance socket and any account/session loading.

Only after this gate passes, **exit the demo process** and test the actual
standalone app with its own linked-device session, not the existing ZapFast
session:

```sh
open QuickChat.app
# From another terminal, while the real QuickChat instance runs:
./QuickChat.app/Contents/MacOS/quickchat --quick
```

Bundled macOS notifications require permissions and a signed .app. Ad-hoc
signing in the bundle script is suitable for a local pilot only. No notification
or WhatsApp delivery result is claimed by the synthetic demo.

For a local shortcut without a separate UI, run `quickchat --quick` through
macOS Shortcuts. The Raycast Script Command is at
`contrib/raycast/quickchat.sh`. Mark it executable with
`chmod +x contrib/raycast/quickchat.sh` and add `contrib/raycast` as a Raycast
Script Commands directory. In Raycast, assign its command alias `w`;
typing `w ahmet` and pressing Enter opens QuickChat with `ahmet` in the
local search field. This is a launcher, **not** an extension that lists contacts
inside Raycast. It neither reads the archive nor outputs the contact list.
No remote service is required.

## Everyday build

For daily use, build the optimized app and install it in `~/Applications`
(quit a running QuickChat first; the script refuses otherwise):

```sh
packaging/macos/quickchat/release.sh
open ~/Applications/QuickChat.app
```

It signs with the keychain's Developer ID Application identity when there
is one (or `CODESIGN_IDENTITY`), else ad hoc. With a stable signature the
Keychain approval ("Always Allow") and the notification permission survive
rebuilds; an ad-hoc signature changes with every build, so macOS asks again.
The first launch after switching signatures asks once more. The bundle is
not notarized: that matters only for copies downloaded on other Macs.

The Raycast script prefers this installed copy when QuickChat is not
running; a running QuickChat takes the request whichever copy sends it.

## Upgrading from the first pilot

The pilot used the bundle identifier `app.quickchat.desktop`; QuickChat now
uses `io.github.ramazanayyildiz.quickchat`. Your linked session, archive and
Keychain key are unchanged (they never depended on it), but macOS treats it
as a new app: allow notifications again in System Settings. If you had
turned on start at login, delete
`~/Library/LaunchAgents/app.quickchat.desktop.plist` and turn the setting
on again.

## Open the full conversations window

QuickChat has two presentation modes, the small name search / one-chat panel
and the original full WhatsApp-style conversation list. While a compact
window is open, choose **All Chats** from its top bar, or choose
**Open All Chats** from the QuickChat menu-bar icon. In a terminal:

```sh
./QuickChat.app/Contents/MacOS/quickchat --chats
```

Launching `open QuickChat.app` without `--quick` also opens the full
conversations window when QuickChat is already running. The resident app
closes and recreates its compact window at the normal saved dimensions rather
than leaving an always-on-top floating panel stretched across the screen.
This is independent of Raycast and never sends any message.

Before the first WhatsApp pairing, both modes show the full linking screen,
not the conversation list. Complete pairing on the phone first.

## Rust disk usage: measure before deleting

The repository uses mise with optional mr-boxington (`mbx`) to share Cargo
build outputs. Repeated debug tests, feature variants, Clippy, and builds
can create many gigabytes of compiler artifacts. Some are managed *outside*
the repository, so absence of a local `target/` does not mean the shared
build store is empty.

These commands only inspect disk usage:

```sh
cd ~/CODE/Ram/quickchat
df -h .
du -sh ./target ./QuickChat.app "$HOME/.cargo/registry" "$HOME/.cargo/git" 2>/dev/null
if command -v mbx >/dev/null 2>&1; then
  mbx doctor
  mbx cache stats
  mbx gc --dry-run
  du -sh "$(dirname "$(mbx cache dir)")"
fi
```

Do not remove `~/Library/Application Support`, the app's device/session
database, or its SQLCipher archive. Do not repeatedly run `cargo clean`:
it discards reusable workspace artifacts and may make the next build bigger
again. If the *mbx* dry run reports disposable cache objects, `mbx gc`
can collect the eligible ones; review the output first. When mbx is absent,
check whether Cargo is running through `mise exec -- cargo ...` before
building again. No deletion is performed by QuickChat.

## Live WhatsApp validation, separate opt-in

Do not attempt this while the synthetic demo process is still open. This
fork's own `QuickChat.app` must be rebuilt without demo features and launched
as a **separate linked device**. None of these commands copy, inspect or migrate
existing ZapFast account archives.

```sh
cd ~/CODE/Ram/quickchat
cargo fmt --all
cargo build --locked --features quickchat
packaging/macos/quickchat/bundle.sh target/debug/zapfast QuickChat.app 0.19.0
open QuickChat.app
```

On the first launch, QuickChat shows the full onboarding view until the
phone has actually linked. The QR code comes from the WhatsApp connection,
not from the keyboard shortcut. On your own phone, open WhatsApp → Settings
(Android: menu) → Linked devices → Link a device, then scan QuickChat's QR
code yourself. If the code does not appear, wait for the recovery text and
click **Retry WhatsApp connection**. Phone-number pairing is offered as an
alternative where available; neither action deletes the old session or
imports an existing ZapFast account.

After the phone successfully links, the previously requested Quick Search
opens automatically with any prefilled name preserved. This is an *unofficial*
WhatsApp client, which may violate WhatsApp's terms or risk an account
restriction. Pairing and real message sending are manual user acceptance gates.

After linking, verify a contact and a group name with
`./QuickChat.app/Contents/MacOS/quickchat --quick-query "sample"`.
The test should open the correct QuickChat search, allow manual selection,
and show the chat, with no unsolicited message sent.

For a notification test, enable QuickChat notifications in macOS System
Settings, then have a consenting contact send an ordinary test message.
Click its notification while QuickChat has no visible window; confirm the
right account and **exact** message appear, not the full conversation list.
Repeat with the app lock enabled and with two linked accounts, if applicable.
No linked device, notification click, or real send has been exercised by
our automated tests.

## Acceptance gates

1. **Isolation:** ZapFast keeps its original session, bundle ID, archive and
   process; neither QuickChat install nor close causes data migration from it.
2. **Launcher:** Control+Option+W works from a different foreground app, finds
   person/group names, and opens only the selected chat.
3. **Privacy:** locked and hidden chats never appear in results; two matching
   names remain two distinct choices; a draft is never sent by navigation.
4. **Panel:** on-screen placement works on the current screen and does not
   persist as the main-window geometry; Esc returns to the prior app.
5. **Notifications:** click targets account + chat + message, including when
   the process has no open window or is app-locked.
6. **Recovery:** QuickChat closes to the menu bar, resumes connection and
   leaves draft state intact. No duplicate WhatsApp sessions.
7. **Regression:** focused tests, demo snapshots and upstream unit suite pass.

## Verified checks and remaining acceptance work

- Live macOS window focus, screen selection (including displays with
  different scaling), panel sizing and native shortcut still require an
  interactive desktop run.
- Live WhatsApp account linking and notification click have not been exercised.
- On this Mac, `cargo test --locked --all-targets --all-features`
  passed with 1227 passing tests, zero failures and 8 explicitly ignored.
  `cargo clippy --locked --all-targets --all-features -- -D warnings`
  also passed. These checks do not prove native notification, linked-device
  delivery or multi-monitor behavior. Run `cargo fmt --all --check` after
  formatting the final source.
- Raycast accepts an optional name and prefills native search; it does not
  show a contact-result list within Raycast or read chat messages.
- No MCP server, AI history search, background summaries or sending API yet.
  Sending exists only in the launcher's own window, never over the local
  command channel.
- A complete signed/notarized release and telemetry-free
  performance benchmarks remain future gates.

The current source is a pilot and must **not** be considered release-ready
until the listed acceptance gates are verified.
