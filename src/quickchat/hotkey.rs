//! Native macOS global Quick Search shortcut (Control + Option + W).
//! The callback only sets a flag and wakes the resident application; it
//! never reads user data and never touches egui from Carbon's callback.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
    hotkey::{Code, HotKey, Modifiers},
};

use crate::backend::Waker;

static OPEN_REQUESTED: AtomicBool = AtomicBool::new(false);
// 0 = not attempted, 1 = active, 2 = received a press, 3 = failed.
static STATUS: AtomicU8 = AtomicU8::new(0);

pub fn status() -> &'static str {
    match STATUS.load(Ordering::Acquire) {
        1 => "Global shortcut: registered (Ctrl+Option+W)",
        2 => "Global shortcut: key press detected",
        3 => "Global shortcut unavailable: use menu bar > Quick Search",
        _ => "Global shortcut not initialized",
    }
}

/// Keeps the Carbon registration alive until the process exits.
pub struct Registration {
    _manager: GlobalHotKeyManager,
}

pub fn register(waker: &Waker) -> Option<Registration> {
    let manager = match GlobalHotKeyManager::new() {
        Ok(manager) => manager,
        Err(error) => {
            log::warn!("QuickChat hotkey cannot initialize: {error}");
            STATUS.store(3, Ordering::Release);
            return None;
        }
    };
    let shortcut = HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyW);
    if let Err(error) = manager.register(shortcut) {
        // A conflict with another app must not prevent WhatsApp from working.
        log::warn!("QuickChat shortcut Control+Option+W is unavailable: {error}");
        STATUS.store(3, Ordering::Release);
        return None;
    }
    STATUS.store(1, Ordering::Release);
    let id = shortcut.id();
    let wake = waker.clone();
    GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
        if event.id == id && event.state == HotKeyState::Pressed {
            OPEN_REQUESTED.store(true, Ordering::Release);
            STATUS.store(2, Ordering::Release);
            wake.wake();
        }
    }));
    Some(Registration { _manager: manager })
}

/// Called only on the resident app's main processing path.
pub fn take() -> bool {
    OPEN_REQUESTED.swap(false, Ordering::AcqRel)
}
