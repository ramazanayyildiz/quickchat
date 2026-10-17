//! QuickChat is a menu-bar utility while it shows the launcher or nothing,
//! and an ordinary app (Dock icon, Cmd-Tab, menu bar) while the full window
//! is open, so that window can always be found and brought forward.

use std::sync::atomic::{AtomicU8, Ordering};

use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};

/// 0 = not set yet, 1 = accessory, 2 = regular.
static POLICY: AtomicU8 = AtomicU8::new(0);

/// What is on screen.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Window {
    /// No window: a menu-bar utility.
    None,
    /// The launcher or the panel: the policy is left as it is, since
    /// dropping the Dock icon under a key window takes its keyboard focus.
    Compact,
    /// The full window: an ordinary app, in the Dock and Cmd-Tab.
    Full,
}

/// Shows the Dock icon with the full window and drops it with the last
/// window. Acts only on a change, on the main thread.
pub fn sync(window: Window) {
    let full_window = match window {
        Window::Full => true,
        Window::None => false,
        Window::Compact => return,
    };
    let wanted = if full_window { 2 } else { 1 };
    if POLICY.load(Ordering::Relaxed) == wanted {
        return;
    }
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
    let policy = if full_window {
        NSApplicationActivationPolicy::Regular
    } else {
        NSApplicationActivationPolicy::Accessory
    };
    if app.setActivationPolicy(policy) {
        POLICY.store(wanted, Ordering::Relaxed);
        if full_window {
            // A new Dock icon does not bring the app forward by itself.
            #[allow(deprecated, reason = "its replacement needs macOS 14")]
            app.activateIgnoringOtherApps(true);
        }
    }
}
