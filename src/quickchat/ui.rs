//! Small, keyboard-first QuickChat views. Only one chat is ever painted
//! in panel mode; no contact list, chat preview or archive goes to an IPC peer.

use egui::{Align, Layout, Rect, Sense, UiBuilder, Vec2, pos2, vec2};

use crate::app::App;
use crate::model::{AccountId, Action, Contact, Message, Page};
use crate::quickchat::{self, Hit, Kind, Mode};
use crate::theme::{self, Icon};
use crate::ui::widgets;

const SEARCH_SIZE: [f32; 2] = [640.0, 440.0];
/// Narrower than the app's narrow layout, so the chat header clears the
/// traffic lights and offers Back, which returns to the search.
const PANEL_SIZE: [f32; 2] = [500.0, 760.0];
const SEARCH_FIELD: &str = "quickchat-name-search";
const MESSAGE_FIELD: &str = "quickchat-message";
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
    if let Some(area) = screen_area(ctx) {
        let left = if panel {
            area.right() - width - 22.0
        } else {
            area.center().x - width / 2.0
        };
        let top = if panel {
            area.top() + 12.0
        } else {
            area.top() + ((area.height() - height) / 4.0).max(30.0)
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos2(
            left.max(area.left()),
            top,
        )));
    }
    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
}

/// Where the window may go: the screen the pointer is on, on macOS, so the
/// launcher follows the person across displays; elsewhere the window's own.
fn screen_area(ctx: &egui::Context) -> Option<Rect> {
    #[cfg(all(target_os = "macos", feature = "quickchat"))]
    if let Some(area) = super::screen::under_pointer() {
        return Some(area);
    }
    ctx.input(|i| i.viewport().monitor_size)
        .map(|size| Rect::from_min_size(egui::Pos2::ZERO, size))
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

/// Whether the launcher can send to `hit` without opening it: only an
/// existing chat that takes messages from us. A contact without a chat,
/// or an admins-only group, opens as before.
fn can_write(app: &App, hit: &Hit) -> bool {
    hit.existing
        && app
            .accounts
            .iter()
            .find(|account| account.id == hit.account)
            .and_then(|account| account.chats.iter().find(|chat| chat.id == hit.chat))
            .is_some_and(crate::model::Chat::can_send)
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
fn footer(app: &App, ui: &mut egui::Ui, hints: &str) {
    let palette = app.palette;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.top(),
        egui::Stroke::new(1.0, palette.outline),
    );
    let galley = ui
        .painter()
        .layout_no_wrap(hints.to_owned(), theme::regular(12.0), palette.dim);
    ui.painter().galley(
        pos2(rect.left() + 20.0, rect.center().y - galley.size().y / 2.0),
        galley,
        palette.dim,
    );
    #[cfg(all(target_os = "macos", feature = "quickchat"))]
    if let Some(problem) = super::hotkey::problem(app.settings.quickchat_shortcut) {
        let galley = ui
            .painter()
            .layout_no_wrap(problem, theme::regular(12.0), palette.warning);
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

/// One line of the launcher's list: a chat or contact by name, or a message
/// whose text matched.
#[derive(Clone)]
enum Row {
    Name(Hit),
    Message(AccountId, Box<Message>),
}

/// The fewest characters a message search runs for: one letter matches
/// nearly every message.
const MESSAGE_SEARCH_MIN: usize = 2;
const MESSAGE_LIMIT: usize = 20;

/// The newest matching messages across accounts, outside locked chats.
fn message_hits(app: &App) -> Vec<(AccountId, Message)> {
    let mut hits: Vec<_> = app
        .accounts
        .iter()
        .flat_map(|account| {
            account
                .quick_hits
                .iter()
                .filter(|message| {
                    account
                        .chats
                        .iter()
                        .find(|chat| chat.id == message.chat)
                        .is_none_or(|chat| !chat.locked)
                })
                .map(|message| (account.id.clone(), message.clone()))
        })
        .collect();
    hits.sort_by_key(|(_, message)| std::cmp::Reverse(message.timestamp));
    hits.truncate(MESSAGE_LIMIT);
    hits
}

/// What the launcher lists: names, then the messages the last search found.
fn rows(app: &App) -> Vec<Row> {
    let mut rows: Vec<Row> = quickchat::search(&app.accounts, &app.quickchat.query, LIMIT)
        .into_iter()
        .map(Row::Name)
        .collect();
    // Only the answer to what was asked: hits from an earlier search stay
    // out of a shorter one that asked for none.
    if !app.quickchat.searched.is_empty() {
        rows.extend(
            message_hits(app)
                .into_iter()
                .map(|(account, message)| Row::Message(account, Box::new(message))),
        );
    }
    rows
}

fn launcher(app: &mut App, ui: &mut egui::Ui) {
    if app.quickchat.compose.is_some() {
        compose(app, ui);
        return;
    }
    let ctx = ui.ctx().clone();
    let palette = app.palette;
    // Messages are searched as the name is typed, once per change.
    let wanted = app.quickchat.query.trim();
    let wanted = if wanted.chars().count() >= MESSAGE_SEARCH_MIN {
        wanted.to_owned()
    } else {
        String::new()
    };
    if wanted != app.quickchat.searched {
        app.quickchat.searched = wanted.clone();
        app.actions.push(Action::QuickSearchMessages(wanted));
    }
    // Tab writes to the selected chat. With nothing to write to it is left
    // to move the focus, so All Chats stays reachable from the keyboard.
    let writable = {
        let rows = rows(app);
        match rows.get(app.quickchat.selected.min(rows.len().saturating_sub(1))) {
            Some(Row::Name(hit)) if can_write(app, hit) => Some(hit.clone()),
            _ => None,
        }
    };
    let hints = if writable.is_some() {
        "↑↓ Select    ↵ Open    ⇥ Write    esc Close"
    } else {
        "↑↓ Select    ↵ Open    esc Close"
    };
    // `consume_key(NONE, Tab)` would take Shift+Tab too, which moves the
    // focus back: look at the event's modifiers instead.
    let tab = |input: &mut egui::InputState| {
        let mut taken = false;
        input.events.retain(|event| {
            let plain_tab = !taken
                && matches!(
                    event,
                    egui::Event::Key {
                        key: egui::Key::Tab,
                        pressed: true,
                        modifiers,
                        ..
                    } if modifiers.is_none()
                );
            taken |= plain_tab;
            !plain_tab
        });
        taken
    };
    if let Some(hit) = writable
        && ctx.input_mut(tab)
    {
        app.quickchat.compose(hit);
        return;
    }
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
            let rows = rows(app);
            let names = rows
                .iter()
                .filter(|row| matches!(row, Row::Name(_)))
                .count();
            let moved = !rows.is_empty() && (up || down);
            if !rows.is_empty() {
                let last = rows.len() - 1;
                let at = app.quickchat.selected.min(last);
                app.quickchat.selected = match (up, down) {
                    (true, false) if at == 0 => last,
                    (true, false) => at - 1,
                    (false, true) if at == last => 0,
                    (false, true) => at + 1,
                    _ => at,
                };
                if enter {
                    chosen = rows.get(app.quickchat.selected).cloned();
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
            if rows.is_empty() {
                empty(app, &mut child);
            } else {
                let hits: Vec<Hit> = rows
                    .iter()
                    .filter_map(|row| match row {
                        Row::Name(hit) => Some(hit.clone()),
                        Row::Message(..) => None,
                    })
                    .collect();
                let query = app.quickchat.query.trim().to_owned();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(&mut child, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for (index, row) in rows.iter().enumerate() {
                            if index == 0 && names > 0 {
                                heading(
                                    app,
                                    ui,
                                    if query.is_empty() {
                                        "Recent"
                                    } else {
                                        "Results"
                                    },
                                );
                            }
                            if index == names {
                                heading(app, ui, "Messages");
                            }
                            let selected = index == app.quickchat.selected;
                            let top = ui.cursor().top();
                            let clicked = match row {
                                Row::Name(hit) => {
                                    let detail = detail(app, hit, &hits);
                                    hit_row(app, ui, hit, &detail, selected)
                                }
                                Row::Message(account, message) => {
                                    message_row(app, ui, account, message, &query, selected)
                                }
                            };
                            if clicked {
                                chosen = Some(row.clone());
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
            footer(app, ui, hints);
        });
    match chosen {
        Some(Row::Name(hit)) => choose(app, hit),
        Some(Row::Message(account, message)) => open_message(app, account, *message),
        None => {}
    }
}

/// A quiet label over a group of rows.
fn heading(app: &App, ui: &mut egui::Ui, label: &str) {
    egui::Frame::new()
        .inner_margin(egui::Margin {
            left: 20,
            right: 20,
            top: 8,
            bottom: 6,
        })
        .show(ui, |ui| {
            theme::text(ui, label, theme::semibold(12.0), app.palette.secondary);
        });
}

/// Opens the panel at a message the search found, in its own account.
fn open_message(app: &mut App, account: AccountId, message: Message) {
    app.quickchat.panel();
    app.actions.push(Action::SwitchAccount(account));
    app.page = Page::Chats;
    app.actions.push(Action::OpenMessage {
        chat: message.chat,
        message: message.id,
    });
}

/// A message the search found: the chat's picture and name, then the line
/// that matched, with its sender in a group.
fn message_row(
    app: &mut App,
    ui: &mut egui::Ui,
    account: &AccountId,
    message: &Message,
    query: &str,
    selected: bool,
) -> bool {
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
    let Some(owner) = app.accounts.iter_mut().find(|owner| &owner.id == account) else {
        return false;
    };
    let title = quickchat::chat_name(owner, &message.chat);
    let picture = owner.avatar(&message.chat);
    let sender = if message.from_me {
        Some("You".to_owned())
    } else if crate::model::ChatKind::from_id(&message.chat) == crate::model::ChatKind::Group {
        Some(
            owner
                .contacts
                .get(&message.sender)
                .and_then(Contact::display_name)
                .map(str::to_owned)
                .or_else(|| message.sender_name.clone())
                .unwrap_or_else(|| quickchat::chat_name(owner, &message.sender)),
        )
    } else {
        None
    };
    let avatar = Rect::from_center_size(
        pos2(rect.left() + 20.0 + AVATAR / 2.0, rect.center().y),
        Vec2::splat(AVATAR),
    );
    widgets::paint_avatar(
        ui,
        &palette,
        avatar,
        &title,
        &message.chat,
        picture.as_deref(),
    );
    let left = avatar.right() + 12.0;
    let right = rect.right() - 20.0;
    let stamp = ui.painter().layout_no_wrap(
        crate::util::chat_stamp(app.locale, message.timestamp),
        theme::regular(11.5),
        palette.dim,
    );
    let width = (right - stamp.size().x - 8.0 - left).max(0.0);
    let name = widgets::line(ui, &title, theme::medium(14.5), palette.text, width, 1);
    let text = message
        .text_matching(query)
        .unwrap_or_else(|| message.summary());
    let text = crate::markup::plain(&text, &[]);
    let text = match sender {
        Some(sender) => format!("{sender}: {text}"),
        None => text,
    };
    let quiet = widgets::line(
        ui,
        &text,
        theme::regular(12.5),
        palette.secondary,
        (right - left).max(0.0),
        1,
    );
    let top = rect.center().y - (name.size().y + 2.0 + quiet.size().y) / 2.0;
    name.paint(ui, pos2(left, top), palette.text);
    ui.painter()
        .galley(pos2(right - stamp.size().x, top + 1.0), stamp, palette.dim);
    quiet.paint(ui, pos2(left, top + name.size().y + 2.0), palette.secondary);
    response.clicked()
}

/// Writing to one chat from the launcher: Enter sends and closes, Shift+Enter
/// starts a new line, Escape goes back to the list with the text kept.
fn compose(app: &mut App, ui: &mut egui::Ui) {
    let Some(hit) = app.quickchat.compose.clone() else {
        return;
    };
    let ctx = ui.ctx().clone();
    let palette = app.palette;
    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        app.quickchat.stop_composing();
        return;
    }
    let field = egui::Id::new(MESSAGE_FIELD);
    // `consume_key(NONE, Enter)` also matches Shift+Enter, which must reach
    // the field as a new line: look at the events' modifiers instead.
    let send = ctx.memory(|memory| memory.has_focus(field))
        && ctx.input_mut(|input| {
            let mut sent = false;
            input.events.retain(|event| {
                let is_send = !sent
                    && matches!(
                        event,
                        egui::Event::Key {
                            key: egui::Key::Enter,
                            pressed: true,
                            modifiers,
                            ..
                        } if !modifiers.shift && !modifiers.alt
                    );
                sent |= is_send;
                !is_send
            });
            sent
        });
    if send && !app.quickchat.message.trim().is_empty() {
        app.actions.push(Action::QuickSend {
            account: hit.account.clone(),
            chat: hit.chat.clone(),
            text: std::mem::take(&mut app.quickchat.message),
        });
        close(app);
        return;
    }
    let mut back = false;
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(palette.window))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            back = compose_bar(app, ui, &hit);
            let footer_height = 34.0;
            let body_height = (ui.available_height() - footer_height).max(0.0);
            let (body, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), body_height), Sense::hover());
            let inner = body.shrink2(vec2(20.0, 12.0));
            let mut child = ui.new_child(
                UiBuilder::new()
                    .max_rect(inner)
                    .layout(Layout::top_down(Align::Min)),
            );
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(&mut child, |ui| {
                    let response = ui.add(
                        egui::TextEdit::multiline(&mut app.quickchat.message)
                            .id(field)
                            .font(theme::regular(15.0))
                            .text_color(palette.text)
                            .hint_text(format!("Message {}", hit.name))
                            .frame(egui::Frame::NONE)
                            .desired_width(inner.width())
                            .desired_rows(8),
                    );
                    if std::mem::take(&mut app.quickchat.focus_message) {
                        response.request_focus();
                    }
                });
            footer(app, ui, "↵ Send    ⇧↵ New line    esc Back");
        });
    if back {
        app.quickchat.stop_composing();
    }
}

/// The message view's top bar: Back, then whom the message goes to, as
/// large as the name in the list so the recipient is never in doubt.
fn compose_bar(app: &mut App, ui: &mut egui::Ui, hit: &Hit) -> bool {
    let palette = app.palette;
    let (bar, _) = ui.allocate_exact_size(
        vec2(ui.available_width(), theme::TOP_BAR_HEIGHT),
        Sense::hover(),
    );
    crate::ui::titlebar_drag(ui, bar);
    ui.painter().hline(
        bar.x_range(),
        bar.bottom(),
        egui::Stroke::new(1.0, palette.outline),
    );
    let left = bar.left() + theme::traffic_light_inset(ui.ctx()).max(16.0);
    let button = Rect::from_center_size(pos2(left + 16.0, bar.center().y), Vec2::splat(32.0));
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(button)
            .layout(Layout::centered_and_justified(egui::Direction::LeftToRight)),
    );
    let back = theme::icon_button(
        &mut child,
        Icon::ArrowLeft,
        18.0,
        palette.secondary,
        palette.text,
        "Back to the list",
    )
    .clicked();
    let avatar = Rect::from_center_size(
        pos2(button.right() + 8.0 + 16.0, bar.center().y),
        Vec2::splat(32.0),
    );
    let picture = app
        .accounts
        .iter_mut()
        .find(|account| account.id == hit.account)
        .and_then(|account| account.avatar(&hit.chat));
    widgets::paint_avatar(
        ui,
        &palette,
        avatar,
        &hit.name,
        &hit.chat,
        picture.as_deref(),
    );
    let text_left = avatar.right() + 10.0;
    let width = (bar.right() - 20.0 - text_left).max(0.0);
    let name = widgets::line(ui, &hit.name, theme::semibold(15.0), palette.text, width, 1);
    name.paint(
        ui,
        pos2(text_left, bar.center().y - name.size().y / 2.0),
        palette.text,
    );
    back
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

    /// Typing two characters asks for matching messages; fewer clears them.
    #[test]
    fn the_launcher_searches_messages_from_two_characters() {
        let root = tempfile::tempdir().unwrap();
        let mut app = app(root.path());
        let ctx = egui::Context::default();
        app.attach(&ctx);
        app.quickchat.search_with_query("a");
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| launcher(&mut app, ui));
        output.textures_delta.clear();
        assert!(
            !app.actions
                .iter()
                .any(|action| matches!(action, Action::QuickSearchMessages(_)))
        );
        app.quickchat.query = " ad ".into();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| launcher(&mut app, ui));
        output.textures_delta.clear();
        assert!(
            app.actions
                .contains(&Action::QuickSearchMessages("ad".into()))
        );
        app.actions.clear();
        app.quickchat.query = "a".into();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| launcher(&mut app, ui));
        output.textures_delta.clear();
        assert!(
            app.actions
                .contains(&Action::QuickSearchMessages(String::new()))
        );
    }

    fn message(chat: &str, id: &str, text: &str) -> crate::model::Message {
        crate::model::Message {
            id: id.into(),
            chat: chat.into(),
            sender: chat.into(),
            sender_name: None,
            from_me: false,
            timestamp: 1,
            content: crate::model::Content::text(text),
            status: crate::model::Delivery::None,
            delivered_at: None,
            read_at: None,
            quoted: None,
            reactions: Vec::new(),
            history_order: None,
            edited: false,
            mentions: Vec::new(),
            forwarded: false,
            thumbnail: None,
        }
    }

    /// Hits an earlier search left behind never show under a query too
    /// short to search, as after closing the launcher and typing one letter.
    #[test]
    fn earlier_message_hits_stay_out_of_a_short_query() {
        let root = tempfile::tempdir().unwrap();
        let mut app = app(root.path());
        app.quick_hits = vec![message("15550001111@s.whatsapp.net", "hit", "meet up")];
        app.quickchat.search_with_query("meet");
        app.quickchat.searched = "meet".into();
        assert!(rows(&app).iter().any(|row| matches!(row, Row::Message(..))));
        app.quickchat.hide();
        app.quickchat.search_with_query("m");
        let ctx = egui::Context::default();
        app.attach(&ctx);
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| launcher(&mut app, ui));
        output.textures_delta.clear();
        assert!(!rows(&app).iter().any(|row| matches!(row, Row::Message(..))));
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
