//! Small, keyboard-first QuickChat views. Only one chat is ever painted
//! in panel mode; no contact list, chat preview or archive goes to an IPC peer.

use egui::{Align, Layout, Rect, Sense, UiBuilder, Vec2, pos2, vec2};

use crate::app::App;
use crate::model::{Action, Page};
use crate::quickchat::{self, Hit, Kind, Mode};
use crate::theme::{self, Icon};
use crate::ui::widgets;

const SEARCH_SIZE: [f32; 2] = [640.0, 440.0];
/// Narrower than the app's narrow layout, so the chat header clears the
/// traffic lights and offers Back, which returns to the search.
const PANEL_SIZE: [f32; 2] = [500.0, 760.0];
const SEARCH_FIELD: &str = "quickchat-name-search";
const HIT_HEIGHT: f32 = 56.0;
const AVATAR: f32 = 36.0;
const LIMIT: usize = 30;

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
    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(vec2(width, height)));
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
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos2(left, top)));
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
            dismiss_dialog: true,
        });
    }
    app.focus_composer = true;
}

/// The last digits of a number, to tell apart two people with one name.
fn suffix(chat: &str) -> String {
    let local = chat.split('@').next().unwrap_or("");
    let mut end: Vec<_> = local.chars().rev().take(4).collect();
    end.reverse();
    end.into_iter().collect()
}

/// The quiet line under a hit's name.
fn detail(app: &App, hit: &Hit, hits: &[Hit]) -> String {
    let mut parts = vec![match (hit.kind, hit.existing) {
        (Kind::Group, _) => "Group".to_owned(),
        (Kind::Person, true) => "Chat".to_owned(),
        (Kind::Person, false) => "Contact, no chat yet".to_owned(),
    }];
    let namesake = hits
        .iter()
        .filter(|other| quickchat::same_name(&other.name, &hit.name))
        .count()
        > 1;
    if namesake && hit.kind == Kind::Person {
        parts.push(format!("…{}", suffix(&hit.chat)));
    }
    if app.accounts.len() > 1
        && let Some(account) = app.accounts.iter().find(|a| a.id == hit.account)
    {
        parts.push(account.display_label(app.locale));
    }
    parts.join(" · ")
}

/// The top bar: the window's buttons, the search field, and All Chats.
fn search_bar(app: &mut App, ui: &mut egui::Ui) -> egui::Response {
    let palette = app.palette;
    let (bar, _) = ui.allocate_exact_size(
        vec2(ui.available_width(), theme::TOP_BAR_HEIGHT),
        Sense::hover(),
    );
    // The empty edges move the window, as a title bar would.
    crate::ui::titlebar_drag(ui, bar);
    let left = bar.left() + theme::traffic_light_inset(ui.ctx()).max(16.0);
    let button = 32.0;
    let right = bar.right() - 16.0 - button - 8.0;
    let field = Rect::from_min_max(
        pos2(left, bar.center().y - 17.0),
        pos2(right, bar.center().y + 17.0),
    );
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(field)
            .layout(Layout::left_to_right(Align::Center)),
    );
    let response = widgets::search_field(
        &mut child,
        &palette,
        egui::Id::new(SEARCH_FIELD),
        &mut app.quickchat.query,
        "Search people and groups",
        field.width(),
    );
    let all = Rect::from_center_size(
        pos2(bar.right() - 16.0 - button / 2.0, bar.center().y),
        Vec2::splat(button),
    );
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(all)
            .layout(Layout::centered_and_justified(egui::Direction::LeftToRight)),
    );
    if theme::icon_button(
        &mut child,
        Icon::PanelLeft,
        18.0,
        palette.secondary,
        palette.text,
        "Open all chats",
    )
    .clicked()
    {
        app.actions.push(Action::ShowAllChats);
    }
    response
}

fn hit_row(app: &mut App, ui: &mut egui::Ui, hit: &Hit, detail: &str, selected: bool) -> bool {
    let palette = app.palette;
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), HIT_HEIGHT), Sense::click());
    if !ui.is_rect_visible(rect) {
        return response.clicked();
    }
    if selected {
        widgets::row_highlight(ui, &palette, rect, palette.surface_active);
    } else if response.hovered() {
        widgets::row_highlight(ui, &palette, rect, palette.surface_hover);
    }
    let avatar = Rect::from_center_size(
        pos2(rect.left() + 20.0 + AVATAR / 2.0, rect.center().y),
        Vec2::splat(AVATAR),
    );
    // Only chats ask WhatsApp for a picture, as the chat list does; a
    // contact without one shows a picture only if it is already here.
    let picture = app
        .accounts
        .iter_mut()
        .find(|account| account.id == hit.account)
        .and_then(|account| {
            if hit.existing {
                account.avatar(&hit.chat)
            } else {
                account.avatars.get(&hit.chat).cloned().flatten()
            }
        });
    widgets::paint_avatar(
        ui,
        &palette,
        avatar,
        &hit.name,
        &hit.chat,
        picture.as_deref(),
    );
    let left = avatar.right() + 12.0;
    let mut right = rect.right() - 20.0;
    if hit.unread > 0 {
        let width = widgets::badge(
            ui,
            &palette,
            pos2(right - 10.0, rect.center().y),
            hit.unread,
            false,
        );
        right -= width + 10.0;
    } else if selected {
        let enter = ui
            .painter()
            .layout_no_wrap("↵".to_owned(), theme::regular(14.0), palette.dim);
        ui.painter().galley(
            pos2(
                right - enter.size().x,
                rect.center().y - enter.size().y / 2.0,
            ),
            enter.clone(),
            palette.dim,
        );
        right -= enter.size().x + 10.0;
    }
    let width = (right - left).max(0.0);
    let name = widgets::line(ui, &hit.name, theme::medium(14.5), palette.text, width, 1);
    let quiet = widgets::line(
        ui,
        detail,
        theme::regular(12.5),
        palette.secondary,
        width,
        1,
    );
    let top = rect.center().y - (name.size().y + 2.0 + quiet.size().y) / 2.0;
    name.paint(ui, pos2(left, top), palette.text);
    quiet.paint(ui, pos2(left, top + name.size().y + 2.0), palette.secondary);
    response.clicked()
}

/// Keyboard hints along the bottom, and the shortcut's state only when it
/// failed.
fn footer(app: &App, ui: &mut egui::Ui) {
    let palette = app.palette;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.top(),
        egui::Stroke::new(1.0, palette.outline),
    );
    let hints = "↑↓ Select    ↵ Open    esc Close";
    let galley = ui
        .painter()
        .layout_no_wrap(hints.to_owned(), theme::regular(12.0), palette.dim);
    ui.painter().galley(
        pos2(rect.left() + 20.0, rect.center().y - galley.size().y / 2.0),
        galley,
        palette.dim,
    );
    #[cfg(all(target_os = "macos", feature = "quickchat"))]
    if let Some(problem) = super::hotkey::problem() {
        let galley =
            ui.painter()
                .layout_no_wrap(problem.to_owned(), theme::regular(12.0), palette.warning);
        ui.painter().galley(
            pos2(
                rect.right() - 20.0 - galley.size().x,
                rect.center().y - galley.size().y / 2.0,
            ),
            galley,
            palette.warning,
        );
    }
}

fn launcher(app: &mut App, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    let palette = app.palette;
    let (up, down, enter, escape) = ctx.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
        )
    });
    if escape {
        // A typed name clears first, as in Spotlight; an empty field closes.
        if app.quickchat.query.is_empty() {
            close(app);
            return;
        }
        app.quickchat.query.clear();
        app.quickchat.selected = 0;
    }
    let mut chosen = None;
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(palette.window))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let response = search_bar(app, ui);
            if std::mem::take(&mut app.quickchat.focus_search) {
                response.request_focus();
            }
            if response.changed() {
                app.quickchat.selected = 0;
            }
            let hits = quickchat::search(&app.accounts, &app.quickchat.query, LIMIT);
            let moved = !hits.is_empty() && (up || down);
            if !hits.is_empty() {
                let last = hits.len() - 1;
                let at = app.quickchat.selected.min(last);
                app.quickchat.selected = match (up, down) {
                    (true, false) if at == 0 => last,
                    (true, false) => at - 1,
                    (false, true) if at == last => 0,
                    (false, true) => at + 1,
                    _ => at,
                };
                if enter {
                    chosen = hits.get(app.quickchat.selected).cloned();
                }
            }
            let footer_height = 34.0;
            let list_height = (ui.available_height() - footer_height).max(0.0);
            let (list, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), list_height), Sense::hover());
            let mut child = ui.new_child(
                UiBuilder::new()
                    .max_rect(list)
                    .layout(Layout::top_down(Align::Min)),
            );
            if hits.is_empty() {
                empty(app, &mut child);
            } else {
                let heading = if app.quickchat.query.trim().is_empty() {
                    "Recent"
                } else {
                    "Results"
                };
                egui::Frame::new()
                    .inner_margin(egui::Margin {
                        left: 20,
                        right: 20,
                        top: 4,
                        bottom: 6,
                    })
                    .show(&mut child, |ui| {
                        theme::text(ui, heading, theme::semibold(12.0), palette.secondary);
                    });
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(&mut child, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for (index, hit) in hits.iter().enumerate() {
                            let selected = index == app.quickchat.selected;
                            let detail = detail(app, hit, &hits);
                            let top = ui.cursor().top();
                            if hit_row(app, ui, hit, &detail, selected) {
                                chosen = Some(hit.clone());
                            }
                            if selected && moved {
                                let rect = Rect::from_min_size(
                                    pos2(ui.max_rect().left(), top),
                                    vec2(ui.max_rect().width(), HIT_HEIGHT),
                                );
                                ui.scroll_to_rect(rect, None);
                            }
                        }
                    });
            }
            footer(app, ui);
        });
    if let Some(hit) = chosen {
        choose(app, hit);
    }
}

/// What the list shows with nothing to list.
fn empty(app: &App, ui: &mut egui::Ui) {
    let palette = app.palette;
    // Immediately after pairing, the device is connected but WhatsApp may
    // not have delivered chats/contacts yet.
    let loading = app.accounts.iter().any(|account| {
        account.syncing
            || (account.chats.is_empty()
                && account.contacts.is_empty()
                && matches!(account.link, crate::backend::LinkStatus::Connected))
    });
    if loading {
        ui.add_space(64.0);
        ui.vertical_centered(|ui| {
            theme::spinner(ui, 22.0, palette.accent);
            ui.add_space(10.0);
            theme::text(
                ui,
                "Syncing your chats and contacts…",
                theme::regular(13.5),
                palette.secondary,
            );
        });
    } else {
        widgets::empty_state(
            ui,
            &palette,
            Icon::Search,
            "No matches",
            "Try another name, or open all chats.",
        );
    }
}

fn panel(app: &mut App, ui: &mut egui::Ui) {
    if app.open_chat.is_none() {
        // Back in the chat header closes the chat: return to the search.
        app.quickchat.search();
        return;
    }
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ui, |ui| crate::ui::conversation::show(app, ui));
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
    match app.quickchat.mode {
        Mode::Search => launcher(app, ui),
        Mode::Panel => panel(app, ui),
        Mode::Normal => {}
    }
}

/// Runs the chat's own keys in the panel. Escape there cancels the topmost
/// state (a dialog, a reply, a selection) as in the full window; only the
/// last step, which would close the chat, hides the panel instead.
pub fn panel_keys(app: &mut App, ctx: &egui::Context) {
    let from = app.actions.len();
    let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape) && i.modifiers.is_none());
    crate::ui::keys::handle(app, ctx);
    if escape
        && let Some(at) = app.actions[from..]
            .iter()
            .position(|action| matches!(action, Action::CloseChat))
    {
        app.actions.remove(from + at);
        close(app);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::AppDirs;
    use crate::settings::Settings;

    fn app(root: &std::path::Path) -> App {
        let mut app = App::headless(AppDirs::under(root), Settings::default()).0;
        app.enable_quickchat_fork();
        app
    }

    /// One frame with Escape pressed.
    fn run(ctx: &egui::Context, frame: impl FnMut(&mut egui::Ui)) {
        let mut output = ctx.run_ui(escape(), frame);
        output.textures_delta.clear();
    }

    fn escape() -> egui::RawInput {
        egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        }
    }

    /// Escape in the panel would close the chat in the full window; here it
    /// hides the panel and keeps the chat, so the next shortcut finds it.
    #[test]
    fn escape_in_the_panel_hides_it_instead_of_closing_the_chat() {
        let root = tempfile::tempdir().unwrap();
        let mut app = app(root.path());
        app.quickchat.panel();
        app.open_chat = Some("15550001111@s.whatsapp.net".into());
        let ctx = egui::Context::default();
        app.attach(&ctx);
        run(&ctx, |ui| panel_keys(&mut app, ui.ctx()));
        assert!(!app.actions.contains(&Action::CloseChat));
        assert!(app.actions.contains(&Action::CloseWindow));
        assert!(app.hide_intent);
    }

    /// A reply being written is cancelled first, as in the full window.
    #[test]
    fn escape_in_the_panel_cancels_a_reply_first() {
        let root = tempfile::tempdir().unwrap();
        let mut app = app(root.path());
        app.quickchat.panel();
        app.open_chat = Some("15550001111@s.whatsapp.net".into());
        app.reply_to = Some(crate::model::ReplyTarget {
            id: "message".into(),
            chat: None,
        });
        let ctx = egui::Context::default();
        app.attach(&ctx);
        run(&ctx, |ui| panel_keys(&mut app, ui.ctx()));
        assert!(app.actions.contains(&Action::CancelReply));
        assert!(!app.actions.contains(&Action::CloseWindow));
    }

    /// The first Escape clears a typed name, the second closes the window.
    #[test]
    fn escape_in_the_launcher_clears_the_name_then_closes() {
        let root = tempfile::tempdir().unwrap();
        let mut app = app(root.path());
        app.quickchat.search_with_query("Ada");
        let ctx = egui::Context::default();
        app.attach(&ctx);
        run(&ctx, |ui| launcher(&mut app, ui));
        assert!(app.quickchat.query.is_empty());
        assert!(!app.actions.contains(&Action::CloseWindow));
        run(&ctx, |ui| launcher(&mut app, ui));
        assert!(app.actions.contains(&Action::CloseWindow));
    }
}
