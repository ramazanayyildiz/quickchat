//! QuickChat's name-only launcher index. No archive access and no message text.
//! The resident process already holds contacts/chats for each linked account.

use std::collections::{HashMap, HashSet};

#[cfg(all(target_os = "macos", feature = "quickchat"))]
mod dock;
mod hooks;
#[cfg(all(target_os = "macos", feature = "quickchat"))]
pub mod hotkey;
pub mod ui;

pub(crate) use hooks::tray_config;

use crate::account::Account;
use crate::model::{AccountId, Chat, ChatKind, Contact};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Person,
    Group,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    pub account: AccountId,
    pub chat: String,
    pub name: String,
    pub kind: Kind,
    pub existing: bool,
    pub unread: u32,
    pub last_activity: i64,
    rank: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Search,
    Panel,
}

#[derive(Debug)]
pub struct State {
    pub mode: Mode,
    pub query: String,
    pub selected: usize,
    pub focus_search: bool,
    pub resize_pending: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            mode: Mode::Normal,
            query: String::new(),
            selected: 0,
            focus_search: false,
            resize_pending: false,
        }
    }
}

impl State {
    pub fn search(&mut self) {
        self.search_with_query("");
    }

    /// Raycast and native search share the same UI. A supplied name is a
    /// search filter only: never interpret it as an exact recipient or send.
    pub fn search_with_query(&mut self, query: &str) {
        self.mode = Mode::Search;
        self.query = query.to_owned();
        self.selected = 0;
        self.focus_search = true;
        self.resize_pending = true;
    }

    pub fn panel(&mut self) {
        self.mode = Mode::Panel;
        self.resize_pending = true;
    }

    pub fn hide(&mut self) {
        self.mode = Mode::Normal;
        self.query.clear();
        self.selected = 0;
        self.focus_search = false;
        self.resize_pending = false;
    }
}

fn key(value: &str) -> String {
    // ZapFast already strips combining accents, but Turkish dotless ı
    // needs an additional fold for Latin-keyboard name searches.
    crate::util::search_key(value).replace('ı', "i")
}

/// Whether two names read the same to the search.
pub fn same_name(a: &str, b: &str) -> bool {
    key(a) == key(b)
}

fn relevance(name: &str, words: &[&str], needle: &str) -> Option<u8> {
    let normalized = key(name);
    if words.is_empty() {
        return Some(4);
    }
    if normalized == needle {
        return Some(0);
    }
    if normalized.starts_with(needle) {
        return Some(1);
    }
    if normalized
        .split_whitespace()
        .any(|word| word.starts_with(needle))
    {
        return Some(2);
    }
    words
        .iter()
        .all(|word| normalized.contains(word))
        .then_some(3)
}

/// Name matching is intentionally account-scoped; ids have no meaning across
/// accounts. Never surface a locked chat via its contact alias.
pub fn search_one(
    account: &AccountId,
    chats: &[Chat],
    contacts: &HashMap<String, Contact>,
    query: &str,
) -> Vec<Hit> {
    let needle = key(query.trim());
    let words: Vec<&str> = needle.split_whitespace().collect();
    let locked: HashSet<&str> = chats
        .iter()
        .filter(|chat| chat.locked)
        .map(|chat| chat.id.as_str())
        .collect();
    let mut seen = HashSet::<String>::new();
    let mut matches = Vec::new();

    for chat in chats {
        if chat.locked || chat.left || chat.kind == ChatKind::Broadcast {
            continue;
        }
        let name = if chat.kind == ChatKind::Group {
            chat.name.clone()
        } else {
            contacts
                .get(&chat.id)
                .and_then(Contact::display_name)
                .map(str::to_owned)
                .unwrap_or_else(|| chat.name.clone())
        };
        let Some(rank) = relevance(&name, &words, &needle) else {
            continue;
        };
        if !seen.insert(chat.id.clone()) {
            continue;
        }
        matches.push(Hit {
            account: account.clone(),
            chat: chat.id.clone(),
            name,
            kind: if chat.kind == ChatKind::Group {
                Kind::Group
            } else {
                Kind::Person
            },
            existing: true,
            unread: chat.unread,
            last_activity: chat.last_activity,
            rank,
        });
    }

    // The contact list also includes people without an existing conversation.
    // Never expose a contact whose same-id chat is locked, even if it matches.
    if !words.is_empty() {
        for contact in contacts.values() {
            if seen.contains(&contact.id)
                || locked.contains(contact.id.as_str())
                // Privacy LIDs require canonical mapping. Do not suggest
                // a new @lid conversation until the worker resolves it.
                || !contact.id.ends_with("@s.whatsapp.net")
            {
                continue;
            }
            let Some(name) = contact.display_name() else {
                continue;
            };
            let Some(rank) = relevance(name, &words, &needle) else {
                continue;
            };
            matches.push(Hit {
                account: account.clone(),
                chat: contact.id.clone(),
                name: name.to_owned(),
                kind: Kind::Person,
                existing: false,
                unread: 0,
                last_activity: 0,
                rank,
            });
        }
    }
    matches
}

pub fn search(accounts: &[Account], query: &str, limit: usize) -> Vec<Hit> {
    let mut result = Vec::new();
    for account in accounts {
        result.extend(search_one(
            &account.id,
            &account.chats,
            &account.contacts,
            query,
        ));
    }
    result.sort_by(|a, b| {
        a.rank
            .cmp(&b.rank)
            .then_with(|| b.existing.cmp(&a.existing))
            .then_with(|| b.last_activity.cmp(&a.last_activity))
            .then_with(|| key(&a.name).cmp(&key(&b.name)))
            .then_with(|| a.account.0.cmp(&b.account.0))
            .then_with(|| a.chat.cmp(&b.chat))
    });
    result.truncate(limit.min(50));
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contact(id: &str, name: &str) -> Contact {
        Contact {
            id: id.to_owned(),
            full_name: Some(name.to_owned()),
            ..Contact::default()
        }
    }

    fn fixture() -> (Vec<Chat>, HashMap<String, Contact>) {
        let a = "1111111111@s.whatsapp.net";
        let b = "2222222222@s.whatsapp.net";
        let locked_id = "3333333333@s.whatsapp.net";
        let mut chats = vec![
            Chat::new(a.into(), "1111111111".into()),
            Chat::new("98765@g.us".into(), "Ürün Tasarım Ekibi".into()),
            Chat::new(locked_id.into(), "Gizli Proje".into()),
        ];
        chats[2].locked = true;
        let contacts = [
            contact(a, "Ahmet Yılmaz"),
            contact(b, "Özge Işık"),
            contact(locked_id, "Kilitli Ahmet"),
        ]
        .into_iter()
        .map(|c| (c.id.clone(), c))
        .collect();
        (chats, contacts)
    }

    #[test]
    fn name_query_prefills_only_search_and_reset_is_explicit() {
        let mut state = State::default();
        state.search_with_query("Ahmet Yılmaz");
        assert_eq!(state.mode, Mode::Search);
        assert_eq!(state.query, "Ahmet Yılmaz");
        assert_eq!(state.selected, 0);
        assert!(state.focus_search);
        state.panel();
        assert_eq!(state.query, "Ahmet Yılmaz");
        state.search();
        assert!(state.query.is_empty());
        assert_eq!(state.mode, Mode::Search);
    }

    #[test]
    fn name_search_uses_contacts_without_phone_input() {
        let (chats, contacts) = fixture();
        let hits = search_one(&AccountId::first(), &chats, &contacts, "ahmet");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "Ahmet Yılmaz");
        assert!(hits[0].existing);
    }

    #[test]
    fn group_and_unsaved_conversation_are_searchable() {
        let (chats, contacts) = fixture();
        let groups = search_one(&AccountId::first(), &chats, &contacts, "tasarim");
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].kind, Kind::Group);
        let person = search_one(&AccountId::first(), &chats, &contacts, "ozge");
        assert_eq!(person.len(), 1);
        assert!(!person[0].existing);
    }

    #[test]
    fn turkish_case_and_dotted_letters_are_searchable() {
        let (chats, contacts) = fixture();
        assert_eq!(
            search_one(&AccountId::first(), &chats, &contacts, "ISIK")[0].name,
            "Özge Işık"
        );
    }

    #[test]
    fn locked_conversations_cannot_leak_through_contacts() {
        let (chats, contacts) = fixture();
        assert!(search_one(&AccountId::first(), &chats, &contacts, "kilitli").is_empty());
        assert!(search_one(&AccountId::first(), &chats, &contacts, "gizli").is_empty());
    }

    #[test]
    fn empty_query_only_lists_existing_unlocked_chats() {
        let (chats, contacts) = fixture();
        let hits = search_one(&AccountId::first(), &chats, &contacts, "");
        assert_eq!(hits.len(), 2);
        assert!(hits.iter().all(|hit| hit.existing));
    }

    #[test]
    fn identically_named_people_are_not_arbitrarily_resolved() {
        let (mut chats, mut contacts) = fixture();
        let id = "4444444444@s.whatsapp.net";
        chats.push(Chat::new(id.into(), "4444444444".into()));
        contacts.insert(id.into(), contact(id, "Ahmet Yılmaz"));
        let hits = search_one(&AccountId::first(), &chats, &contacts, "ahmet");
        assert_eq!(hits.len(), 2);
        assert_ne!(hits[0].chat, hits[1].chat);
    }
}
