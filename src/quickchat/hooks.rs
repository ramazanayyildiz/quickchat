//! Where QuickChat plugs into the app: its tray menu, the shortcut, and
//! when the launcher may show. `app.rs` calls these and keeps no logic of
//! its own for them.

use crate::app::{App, TRAY_LOCK, TRAY_QUIT};
use crate::backend::LinkStatus;
use crate::model::Action;
use crate::settings::QuickShortcut;

use super::Mode;

pub(crate) const TRAY_QUICK: &str = "quick-search";
pub(crate) const TRAY_ALL_CHATS: &str = "all-chats";

/// The tray's launcher entry, naming the shortcut when there is one.
fn quick_label(shortcut: QuickShortcut) -> String {
    match shortcut {
        QuickShortcut::Off => "Quick Search".to_owned(),
        other => format!("Quick Search ({})", other.symbols()),
    }
}

/// The tray menu, with a launcher entry for when the shortcut is taken by
/// another app.
pub(crate) fn tray_config(lockable: bool, shortcut: QuickShortcut) -> fastframe_tray::Config {
    use fastframe_tray::MenuItem;
    let mut config = crate::app::tray_config(lockable);
    config.id = "quickchat";
    config.title = "QuickChat".into();
    config.menu = vec![
        MenuItem::action(TRAY_QUICK, quick_label(shortcut)),
        MenuItem::action(TRAY_ALL_CHATS, "Open QuickChat"),
        MenuItem::Separator,
        MenuItem::action(TRAY_LOCK, "Lock QuickChat").visible(lockable),
        MenuItem::action(TRAY_QUIT, "Quit QuickChat"),
    ];
    config
}

/// Whether an account has linked, and so has chats and an archive to search.
fn account_ready(account: &crate::account::Account) -> bool {
    match account.link {
        LinkStatus::Connected => true,
        LinkStatus::Connecting | LinkStatus::Disconnected { .. } => {
            account.me.is_some() || !account.chats.is_empty()
        }
        LinkStatus::Starting
        | LinkStatus::Unlinked { .. }
        | LinkStatus::LoggedOut
        | LinkStatus::Failed(_) => false,
    }
}

impl App {
    /// Turns on QuickChat's launcher and turns off ZapFast's updater.
    pub fn enable_quickchat_fork(&mut self) {
        self.fork_mode = true;
        self.settings.check_for_updates = false;
        self.settings.download_updates_automatically = false;
    }

    /// Opt-in demo menu for checking the macOS launcher with synthetic data.
    /// No real account or on-disk session is loaded.
    pub fn enable_quickchat_demo_tray(&mut self) {
        self.enable_quickchat_fork();
        if self.tray.is_none() {
            let wake = self.waker.clone();
            self.tray = fastframe_tray::Tray::spawn(
                tray_config(self.tray_lockable, self.settings.quickchat_shortcut),
                move || wake.wake(),
            );
        }
    }

    #[cfg(all(target_os = "macos", feature = "quickchat"))]
    pub(crate) fn ensure_quickchat_hotkey(&mut self) {
        // A test must never take a shortcut from the desktop it runs on.
        if self.fork_mode && self.quickchat_hotkey.is_none() && !cfg!(test) {
            self.quickchat_hotkey =
                super::hotkey::register(&self.waker, self.settings.quickchat_shortcut);
        }
    }

    /// Asks every account's archive for messages matching the launcher's
    /// search. The answers land in each account's `quick_hits`.
    pub(crate) fn search_quick_messages(&mut self, query: String) {
        self.quickchat.searched = query.clone();
        for account in &mut self.accounts {
            account.quick_hits.clear();
            // An account not linked yet has no archive open to ask.
            if !query.is_empty() && account_ready(account) {
                account
                    .backend
                    .send(crate::backend::Command::SearchMessages {
                        query: query.clone(),
                    });
            }
        }
    }

    /// Marks the menu-bar icon while any chat is unread, as the Dock badge
    /// would if QuickChat kept one, and names the count in its tooltip.
    pub(crate) fn sync_quickchat_tray(&mut self) {
        if !self.fork_mode {
            return;
        }
        let unread = self.unread_chat_count_everywhere();
        let shown = self.quickchat.tray_unread.replace(unread);
        if shown == Some(unread) {
            return;
        }
        let Some(tray) = self.tray.as_mut() else {
            return;
        };
        if shown.is_none_or(|shown| (shown > 0) != (unread > 0)) {
            let template: fastframe_tray::DrawIcon = if unread > 0 {
                crate::util::tray_template_unread_rgba
            } else {
                crate::util::tray_template_rgba
            };
            tray.set_icon(crate::util::app_icon_rgba, Some(template));
        }
        tray.set_tooltip(match unread {
            0 => "QuickChat".to_owned(),
            1 => "QuickChat: 1 unread chat".to_owned(),
            count => format!("QuickChat: {count} unread chats"),
        });
    }

    /// Saves the launcher shortcut and puts it into effect at once, along
    /// with the tray entry that names it.
    pub(crate) fn set_quickchat_shortcut(&mut self, shortcut: QuickShortcut) {
        self.settings.quickchat_shortcut = shortcut;
        self.mark_settings_dirty();
        // Only a running registration changes: the window registers the
        // shortcut when it starts, and tests never do.
        #[cfg(all(target_os = "macos", feature = "quickchat"))]
        if let Some(registration) = self.quickchat_hotkey.as_mut() {
            registration.set(shortcut);
        }
        if self.fork_mode
            && let Some(tray) = self.tray.as_mut()
        {
            tray.set_label(TRAY_QUICK, quick_label(shortcut));
        }
    }

    /// Opens the launcher with `query` in its field, or the window in
    /// ZapFast, which has no launcher.
    pub(crate) fn open_quick_search(&mut self, query: &str) {
        if self.fork_mode {
            self.quickchat.search_with_query(query);
        }
        self.actions.push(Action::ShowWindow);
    }

    /// Keeps the Dock icon while, and only while, the full window is open.
    #[cfg(all(target_os = "macos", feature = "quickchat"))]
    pub(crate) fn sync_quickchat_dock(&self) {
        use super::dock::Window;
        if self.fork_mode {
            super::dock::sync(if self.window_hidden {
                Window::None
            } else if self.quickchat_compact() {
                Window::Compact
            } else {
                Window::Full
            });
        }
    }

    /// Handles QuickChat's own tray entries; false leaves `event` to the app.
    pub(crate) fn quickchat_tray_event(&mut self, event: &fastframe_tray::Event) -> bool {
        match event {
            fastframe_tray::Event::Menu(TRAY_QUICK) => self.open_quick_search(""),
            fastframe_tray::Event::Menu(TRAY_ALL_CHATS) => self.actions.push(Action::ShowAllChats),
            _ => return false,
        }
        true
    }

    /// QuickChat may offer search only after WhatsApp has actually linked.
    /// A fresh protocol connection reports Connecting before the phone has
    /// approved any device, so is_linked() alone is not sufficient here.
    /// Previous linked accounts with cached identity/chats can be searched
    /// while reconnecting or temporarily offline.
    pub fn quickchat_ready(&self) -> bool {
        self.accounts.iter().any(account_ready)
    }

    /// First-run and unlinked accounts stay on the full pairing page until
    /// a confirmed linked identity is available for Quick Search.
    pub fn quickchat_needs_link(&self) -> bool {
        self.fork_mode && !self.quickchat_ready()
    }

    /// Whether the compact launcher or panel draws instead of the full app.
    pub fn quickchat_compact(&self) -> bool {
        self.fork_mode && self.quickchat.mode != Mode::Normal && self.quickchat_ready()
    }

    pub fn is_quickchat_fork(&self) -> bool {
        self.fork_mode
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_menu_has_a_launcher_for_when_the_shortcut_is_taken() {
        use fastframe_tray::MenuItem;
        let config = tray_config(false, QuickShortcut::default());
        assert_eq!(config.id, "quickchat");
        assert_eq!(config.title, "QuickChat");
        assert_eq!(
            config.menu.first(),
            Some(&MenuItem::action(TRAY_QUICK, "Quick Search (⌃⌥W)"))
        );
        assert_eq!(
            config.menu.get(1),
            Some(&MenuItem::action(TRAY_ALL_CHATS, "Open QuickChat"))
        );
    }
}
