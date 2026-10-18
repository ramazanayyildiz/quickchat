//! QuickChat's global launcher shortcut on macOS, Control+Option+W unless
//! Settings picks another. The callback only sets a flag and wakes the
//! resident application; it never reads user data and never touches egui
//! from Carbon's callback.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
    hotkey::{Code, HotKey, Modifiers},
};

use crate::backend::Waker;
use crate::settings::QuickShortcut;

static OPEN_REQUESTED: AtomicBool = AtomicBool::new(false);
/// The registered shortcut's id, 0 for none.
static CURRENT: AtomicU32 = AtomicU32::new(0);
/// Whether the chosen shortcut could not be registered.
static TAKEN: AtomicBool = AtomicBool::new(false);

/// Why the shortcut does not work, when it does not.
pub fn problem(shortcut: QuickShortcut) -> Option<String> {
    TAKEN.load(Ordering::Acquire).then(|| {
        format!(
            "{} is taken: pick another in Settings, or use the menu bar icon",
            shortcut.symbols()
        )
    })
}

fn hotkey(shortcut: QuickShortcut) -> Option<HotKey> {
    let control_option = Some(Modifiers::CONTROL | Modifiers::ALT);
    Some(match shortcut {
        QuickShortcut::ControlOptionW => HotKey::new(control_option, Code::KeyW),
        QuickShortcut::ControlOptionSpace => HotKey::new(control_option, Code::Space),
        QuickShortcut::OptionSpace => HotKey::new(Some(Modifiers::ALT), Code::Space),
        QuickShortcut::ControlOptionK => HotKey::new(control_option, Code::KeyK),
        QuickShortcut::Off => return None,
    })
}

/// Keeps the Carbon registration alive until the process exits.
pub struct Registration {
    manager: GlobalHotKeyManager,
    current: Option<HotKey>,
}

impl Registration {
    /// Swaps the shortcut. One another app holds leaves none, and says so
    /// through [`problem`]; WhatsApp keeps working either way.
    pub fn set(&mut self, shortcut: QuickShortcut) {
        if let Some(old) = self.current.take() {
            let _ = self.manager.unregister(old);
        }
        CURRENT.store(0, Ordering::Release);
        TAKEN.store(false, Ordering::Release);
        let Some(key) = hotkey(shortcut) else {
            return;
        };
        match self.manager.register(key) {
            Ok(()) => {
                CURRENT.store(key.id(), Ordering::Release);
                self.current = Some(key);
            }
            Err(error) => {
                log::warn!(
                    "QuickChat shortcut {} is unavailable: {error}",
                    shortcut.symbols()
                );
                TAKEN.store(true, Ordering::Release);
            }
        }
    }
}

pub fn register(waker: &Waker, shortcut: QuickShortcut) -> Option<Registration> {
    let manager = match GlobalHotKeyManager::new() {
        Ok(manager) => manager,
        Err(error) => {
            log::warn!("QuickChat hotkey cannot initialize: {error}");
            TAKEN.store(true, Ordering::Release);
            return None;
        }
    };
    let wake = waker.clone();
    GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
        let current = CURRENT.load(Ordering::Acquire);
        if current != 0 && event.id == current && event.state == HotKeyState::Pressed {
            OPEN_REQUESTED.store(true, Ordering::Release);
            wake.wake();
        }
    }));
    let mut registration = Registration {
        manager,
        current: None,
    };
    registration.set(shortcut);
    Some(registration)
}

/// Called only on the resident app's main processing path.
pub fn take() -> bool {
    OPEN_REQUESTED.swap(false, Ordering::AcqRel)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every choice but Off is a distinct shortcut.
    #[test]
    fn each_choice_is_its_own_shortcut() {
        let ids: std::collections::HashSet<_> = QuickShortcut::ALL
            .into_iter()
            .filter_map(hotkey)
            .map(|key| key.id())
            .collect();
        assert_eq!(ids.len(), QuickShortcut::ALL.len() - 1);
        assert!(hotkey(QuickShortcut::Off).is_none());
    }
}
