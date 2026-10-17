//! Small, keyboard-first QuickChat views. Only one chat is ever painted
//! in panel mode; no contact list, chat preview or archive goes to an IPC peer.

use crate::app::App;
use crate::model::{Action, Page};
use crate::quickchat::{self, Hit, Kind, Mode};

const SEARCH_SIZE: [f32; 2] = [560.0, 470.0];
const PANEL_SIZE: [f32; 2] = [490.0, 760.0];
const SEARCH_FIELD: &str = "quickchat-name-search";

fn close(app: &mut App) {
    // Leave QuickChat's mode intact until window_gone() so the window's
    // temporary geometry is never persisted as the ordinary window geometry.
    app.hide_intent = true;
    app.actions.push(Action::CloseWindow);
}

fn place_window(app: &mut App, ctx: &egui::Context) {
    if !std::mem::take(&mut app.quickchat.resize_pending) {
        return;
    }
    let panel = app.quickchat.mode == Mode::Panel;
    let [width, height] = if panel { PANEL_SIZE } else { SEARCH_SIZE };
    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(width, height)));
    ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
        egui::WindowLevel::AlwaysOnTop,
    ));
    // This is a first-pass placement on the primary display. A real NSPanel
    // geometry pilot will handle multiple monitors, Spaces and menu-bar insets.
    if let Some(monitor) = ctx.input(|i| i.viewport().monitor_size) {
        let left = if panel {
            (monitor.x - width - 22.0).max(0.0)
        } else {
            ((monitor.x - width) / 2.0).max(0.0)
        };
        let top = if panel {
            42.0
        } else {
            ((monitor.y - height) / 4.0).max(30.0)
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(left, top)));
    }
    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
}

fn choose(app: &mut App, hit: Hit) {
    app.quickchat.panel();
    app.actions.push(Action::SwitchAccount(hit.account));
    app.page = Page::Chats;
    if hit.existing {
        app.actions.push(Action::OpenChat(hit.chat));
    } else {
        app.actions.push(Action::StartChat {
            id: hit.chat,
            name: hit.name,
        });
    }
    app.focus_composer = true;
}

fn suffix(chat: &str) -> String {
    let local = chat.split('@').next().unwrap_or("");
    let mut end: Vec<_> = local.chars().rev().take(4).collect();
    end.reverse();
    end.into_iter().collect()
}

fn launcher(app: &mut App, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    let (up, down, enter) = ctx.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
        )
    });
    let mut chosen = None;
    egui::CentralPanel::default().show(ui, |ui| {
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.heading("QuickChat");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Close").clicked() {
                    close(app);
                }
                if ui.small_button("All Chats").clicked() {
                    app.actions.push(Action::ShowAllChats);
                }
            });
        });
        ui.label(egui::RichText::new("Search WhatsApp people and groups by name").weak());
        ui.add_space(10.0);
        let response = ui.add(
            egui::TextEdit::singleline(&mut app.quickchat.query)
                .id(egui::Id::new(SEARCH_FIELD))
                .hint_text("Name or group…")
                .desired_width(f32::INFINITY),
        );
        if std::mem::take(&mut app.quickchat.focus_search) {
            response.request_focus();
        }
        if response.changed() {
            app.quickchat.selected = 0;
        }

        let hits = quickchat::search(&app.accounts, &app.quickchat.query, 16);
        if !hits.is_empty() {
            if down {
                app.quickchat.selected = (app.quickchat.selected + 1).min(hits.len() - 1);
            }
            if up {
                app.quickchat.selected = app.quickchat.selected.saturating_sub(1);
            }
            if enter {
                chosen = hits.get(app.quickchat.selected).cloned();
            }
        }
        ui.add_space(8.0);
        egui::ScrollArea::vertical()
            .max_height(305.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if hits.is_empty() {
                    // Immediately after pairing, the device is connected but
                    // WhatsApp may not have delivered chats/contacts yet.
                    let loading = app.accounts.iter().any(|account| {
                        account.syncing
                            || (account.chats.is_empty()
                                && account.contacts.is_empty()
                                && matches!(account.link, crate::backend::LinkStatus::Connected))
                    });
                    if loading {
                        ui.label("WhatsApp linked. Syncing contacts and conversations…");
                    } else {
                        ui.label("No matching person or group.");
                    }
                }
                for (index, hit) in hits.iter().enumerate() {
                    let kind = match hit.kind {
                        Kind::Person => "Person",
                        Kind::Group => "Group",
                    };
                    let account = if app.accounts.len() > 1 {
                        format!(
                            " · Account {}",
                            app.accounts
                                .iter()
                                .position(|a| a.id == hit.account)
                                .unwrap_or(0)
                                + 1
                        )
                    } else {
                        String::new()
                    };
                    let hint = format!("{kind} · …{}{account}", suffix(&hit.chat));
                    let title = if hit.unread > 0 {
                        format!("{} ({})", hit.name, hit.unread)
                    } else {
                        hit.name.clone()
                    };
                    ui.horizontal(|ui| {
                        let button = ui.selectable_label(index == app.quickchat.selected, title);
                        if button.clicked() {
                            chosen = Some(hit.clone());
                        }
                        ui.label(egui::RichText::new(hint).weak().small());
                    });
                }
            });
        ui.add_space(5.0);
        ui.label(
            egui::RichText::new("↑ ↓  Select     Enter  Open chat     Esc  Hide")
                .weak()
                .small(),
        );
        #[cfg(all(target_os = "macos", feature = "quickchat"))]
        ui.label(egui::RichText::new(super::hotkey::status()).weak().small());
    });
    if let Some(hit) = chosen {
        choose(app, hit);
    }
}

fn panel(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        if ui.small_button("All Chats").clicked() {
            app.actions.push(Action::ShowAllChats);
        }
    });
    egui::CentralPanel::default().show(ui, |ui| {
        if app.open_chat.is_none() {
            ui.label("Select a conversation to start.");
            if ui.button("Search").clicked() {
                app.quickchat.search();
            }
        } else {
            crate::ui::conversation::show(app, ui);
        }
    });
}

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    place_window(app, &ctx);
    // Quick Search bypasses the ordinary keyboard shortcut handler.
    // Keep standard macOS exit and window-close shortcuts available.
    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Q)) {
        app.actions.push(Action::Quit);
        return;
    }
    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::W)) {
        close(app);
        return;
    }
    let plain_escape_can_hide = match app.quickchat.mode {
        Mode::Search => true,
        Mode::Panel => {
            app.dialog.is_none()
                && app.picker.is_none()
                && app.recording.is_none()
                && app.selection.is_none()
                && app.image_preview.is_none()
                && !app.video_expanded
                && app.reply_to.is_none()
                && app.editing.is_none()
                && app.pending.is_empty()
                && !egui::Popup::is_any_open(&ctx)
        }
        Mode::Normal => false,
    };
    if plain_escape_can_hide
        && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
    {
        close(app);
        return;
    }
    match app.quickchat.mode {
        Mode::Search => launcher(app, ui),
        Mode::Panel => panel(app, ui),
        Mode::Normal => {}
    }
}
