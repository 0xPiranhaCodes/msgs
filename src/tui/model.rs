use crate::domain::{ChatEntry, Identity, InstanceId, NetworkEvent, UserAction};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::{
    collections::{BTreeMap, VecDeque},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_ENTRIES: usize = 1_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Compose,
    Navigate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pane {
    Peers,
    Conversation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overlay {
    Help,
}

#[derive(Clone, Debug)]
pub enum InputEvent {
    Key(KeyEvent),
    Network(NetworkEvent),
    Scroll(i16),
}

pub struct Model {
    pub local_identity: Identity,
    pub mode: Mode,
    pub pane: Pane,
    pub overlay: Option<Overlay>,
    pub peers_visible: bool,
    pub peers: BTreeMap<InstanceId, Identity>,
    pub entries: VecDeque<ChatEntry>,
    pub composer: String,
    pub status: String,
    pub scroll: usize,
    pub should_quit: bool,
}

impl Model {
    pub fn new(local_identity: Identity) -> Self {
        Self {
            local_identity,
            mode: Mode::Compose,
            pane: Pane::Conversation,
            overlay: None,
            peers_visible: true,
            peers: BTreeMap::new(),
            entries: VecDeque::new(),
            composer: String::new(),
            status: "joined room automatically".into(),
            scroll: 0,
            should_quit: false,
        }
    }

    pub fn update(&mut self, event: InputEvent) -> Vec<UserAction> {
        match event {
            InputEvent::Network(event) => {
                self.network(event);
                Vec::new()
            }
            InputEvent::Scroll(delta) => {
                self.scroll = self.scroll.saturating_add_signed(delta as isize);
                Vec::new()
            }
            InputEvent::Key(key) => self.key(key),
        }
    }

    fn key(&mut self, key: KeyEvent) -> Vec<UserAction> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return vec![UserAction::Quit];
        }
        if self.overlay == Some(Overlay::Help) {
            if matches!(key.code, KeyCode::Esc | KeyCode::Char('?')) {
                self.overlay = None;
            }
            return Vec::new();
        }
        match self.mode {
            Mode::Compose => self.compose_key(key),
            Mode::Navigate => self.navigate_key(key),
        }
    }

    fn compose_key(&mut self, key: KeyEvent) -> Vec<UserAction> {
        match key.code {
            KeyCode::Esc => self.mode = Mode::Navigate,
            KeyCode::Backspace => {
                self.composer.pop();
            }
            KeyCode::Char(character)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && self.composer.len() < 8 * 1024 =>
            {
                self.composer.push(character);
            }
            KeyCode::Enter => {
                let text = self.composer.trim().to_owned();
                self.composer.clear();
                if text.is_empty() {
                    return Vec::new();
                }
                if text == "/quit" {
                    self.should_quit = true;
                    return vec![UserAction::Quit];
                }
                if text == "/who" {
                    return vec![UserAction::ListPeers];
                }
                self.push_entry(ChatEntry::Message {
                    id: InstanceId([0; 16]),
                    identity: self.local_identity.clone(),
                    text: text.clone(),
                    timestamp: now(),
                });
                return vec![UserAction::Send(text)];
            }
            _ => {}
        }
        Vec::new()
    }

    fn navigate_key(&mut self, key: KeyEvent) -> Vec<UserAction> {
        match key.code {
            KeyCode::Char('i') => self.mode = Mode::Compose,
            KeyCode::Char('q') => {
                self.should_quit = true;
                return vec![UserAction::Quit];
            }
            KeyCode::Char('k') | KeyCode::Up => self.scroll = self.scroll.saturating_add(1),
            KeyCode::Char('j') | KeyCode::Down => self.scroll = self.scroll.saturating_sub(1),
            KeyCode::PageUp => self.scroll = self.scroll.saturating_add(10),
            KeyCode::PageDown => self.scroll = self.scroll.saturating_sub(10),
            KeyCode::Char('p') => self.peers_visible = !self.peers_visible,
            KeyCode::Char('?') => self.overlay = Some(Overlay::Help),
            KeyCode::Tab => {
                self.pane = match self.pane {
                    Pane::Peers => Pane::Conversation,
                    Pane::Conversation => Pane::Peers,
                }
            }
            _ => {}
        }
        Vec::new()
    }

    fn network(&mut self, event: NetworkEvent) {
        match event {
            NetworkEvent::PeerJoined { id, identity } => {
                self.peers.insert(id, identity.clone());
                self.push_entry(ChatEntry::Presence {
                    identity,
                    joined: true,
                    timestamp: now(),
                });
            }
            NetworkEvent::PeerLeft { id, identity } => {
                self.peers.remove(&id);
                self.push_entry(ChatEntry::Presence {
                    identity,
                    joined: false,
                    timestamp: now(),
                });
            }
            NetworkEvent::Message {
                id,
                identity,
                text,
                timestamp,
            } => self.push_entry(ChatEntry::Message {
                id,
                identity,
                text,
                timestamp,
            }),
            NetworkEvent::Status(status) => self.status = status,
        }
    }

    fn push_entry(&mut self, entry: ChatEntry) {
        self.entries.push_back(entry);
        while self.entries.len() > MAX_ENTRIES {
            self.entries.pop_front();
        }
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> Model {
        Model::new(Identity {
            name: "Ada".into(),
            email: "ada@example.com".into(),
        })
    }

    #[test]
    fn starts_in_compose_and_enter_sends() {
        let mut model = model();
        assert_eq!(model.mode, Mode::Compose);
        for character in "hello".chars() {
            model.update(InputEvent::Key(KeyEvent::new(
                KeyCode::Char(character),
                KeyModifiers::NONE,
            )));
        }
        let actions = model.update(InputEvent::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        )));
        assert_eq!(actions, vec![UserAction::Send("hello".into())]);
        assert!(model.composer.is_empty());
    }

    #[test]
    fn switches_modes_and_handles_navigation_keys() {
        let mut model = model();
        model.update(InputEvent::Key(KeyEvent::new(
            KeyCode::Esc,
            KeyModifiers::NONE,
        )));
        assert_eq!(model.mode, Mode::Navigate);
        model.update(InputEvent::Key(KeyEvent::new(
            KeyCode::Char('k'),
            KeyModifiers::NONE,
        )));
        assert_eq!(model.scroll, 1);
        model.update(InputEvent::Key(KeyEvent::new(
            KeyCode::Char('p'),
            KeyModifiers::NONE,
        )));
        assert!(!model.peers_visible);
        model.update(InputEvent::Key(KeyEvent::new(
            KeyCode::Char('?'),
            KeyModifiers::NONE,
        )));
        assert_eq!(model.overlay, Some(Overlay::Help));
        model.update(InputEvent::Key(KeyEvent::new(
            KeyCode::Esc,
            KeyModifiers::NONE,
        )));
        assert_eq!(model.overlay, None);
        model.update(InputEvent::Key(KeyEvent::new(
            KeyCode::Char('i'),
            KeyModifiers::NONE,
        )));
        assert_eq!(model.mode, Mode::Compose);
    }

    #[test]
    fn slash_commands_and_quit_keys_emit_actions() {
        let mut model = model();
        model.composer = "/who".into();
        assert_eq!(
            model.update(InputEvent::Key(KeyEvent::new(
                KeyCode::Enter,
                KeyModifiers::NONE
            ))),
            vec![UserAction::ListPeers]
        );
        let actions = model.update(InputEvent::Key(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
        )));
        assert_eq!(actions, vec![UserAction::Quit]);
        assert!(model.should_quit);
    }

    #[test]
    fn network_entries_are_bounded_and_peers_are_tracked() {
        let mut model = model();
        let identity = Identity {
            name: "Grace".into(),
            email: "grace@example.com".into(),
        };
        model.update(InputEvent::Network(NetworkEvent::PeerJoined {
            id: InstanceId([2; 16]),
            identity: identity.clone(),
        }));
        assert_eq!(model.peers.len(), 1);
        for index in 0..1_010 {
            model.update(InputEvent::Network(NetworkEvent::Message {
                id: InstanceId([2; 16]),
                identity: identity.clone(),
                text: index.to_string(),
                timestamp: index,
            }));
        }
        assert_eq!(model.entries.len(), 1_000);
        model.update(InputEvent::Network(NetworkEvent::PeerLeft {
            id: InstanceId([2; 16]),
            identity,
        }));
        assert!(model.peers.is_empty());
    }
}
