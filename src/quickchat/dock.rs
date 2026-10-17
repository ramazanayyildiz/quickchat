//! QuickChat is a menu-bar utility while it shows the launcher or nothing,
//! and an ordinary app (Dock icon, Cmd-Tab, menu bar) while the full window
//! is open, so that window can always be found and brought forward.

use std::sync::atomic::{AtomicU8, Ordering};

use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};

/// 0 = not set yet, 1 = accessory, 2 = regular.
static POLICY: AtomicU8 = AtomicU8::new(0);

/// Shows the Dock icon while `full_window` is on screen, and hides it
/// otherwise. Acts only on a change, on the main thread.
pub fn sync(full_window: bool) {
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
