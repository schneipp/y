use std::collections::HashMap;

use super::action::Action;
use super::key::KeyCombo;
use crate::mode::Mode;

#[derive(Debug)]
pub enum DispatchResult {
    Executed(Action),
    Pending,
    AwaitingChar(Action),
    Unbound,
}

#[derive(Debug)]
enum TrieNode {
    Prefix(HashMap<KeyCombo, TrieNode>),
    Leaf(Action),
}

#[derive(Debug)]
pub struct KeybindingRegistry {
    /// Per-mode binding trie. Single keys are just depth-1 tries.
    modes: HashMap<ModeKey, HashMap<KeyCombo, TrieNode>>,
}

/// Simplified mode key for the registry (avoids needing all Mode variants).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ModeKey {
    Normal,
    Insert,
    Visual,
    VisualLine,
    Completion,
    Normie,
}

impl ModeKey {
    pub fn from_mode(mode: &Mode) -> Option<Self> {
        match mode {
            Mode::Normal => Some(ModeKey::Normal),
            Mode::Insert => Some(ModeKey::Insert),
            Mode::Visual => Some(ModeKey::Visual),
            Mode::VisualLine => Some(ModeKey::VisualLine),
            Mode::Normie => Some(ModeKey::Normie),
            // Command, Search, FuzzyFinder are text-entry modes with hardcoded handling
            _ => None,
        }
    }
}

impl KeybindingRegistry {
    pub fn new() -> Self {
        Self {
            modes: HashMap::new(),
        }
    }

    /// Bind a key sequence to an action for a given mode.
    pub fn bind(&mut self, mode: ModeKey, keys: Vec<KeyCombo>, action: Action) {
        let mode_map = self.modes.entry(mode).or_insert_with(HashMap::new);

        if keys.is_empty() {
            return;
        }

        if keys.len() == 1 {
            mode_map.insert(keys.into_iter().next().unwrap(), TrieNode::Leaf(action));
            return;
        }

        // Multi-key sequence: build trie
        let mut keys_iter = keys.into_iter();
        let first = keys_iter.next().unwrap();
        let remaining: Vec<KeyCombo> = keys_iter.collect();

        let node = mode_map
            .entry(first)
            .or_insert_with(|| TrieNode::Prefix(HashMap::new()));

        // Ensure the first node is a Prefix
        if let TrieNode::Leaf(_) = node {
            // Overwrite single-key binding with a prefix (sequence takes priority)
            *node = TrieNode::Prefix(HashMap::new());
        }

        Self::insert_into_trie(node, &remaining, action);
    }

    fn insert_into_trie(node: &mut TrieNode, keys: &[KeyCombo], action: Action) {
        if keys.is_empty() {
            return;
        }

        if let TrieNode::Prefix(ref mut children) = node {
            if keys.len() == 1 {
                children.insert(keys[0].clone(), TrieNode::Leaf(action));
            } else {
                let child = children
                    .entry(keys[0].clone())
                    .or_insert_with(|| TrieNode::Prefix(HashMap::new()));
                Self::insert_into_trie(child, &keys[1..], action);
            }
        }
    }

    /// Resolve a key event given current pending sequence state.
    /// `pending` is modified in place: extended on Pending, cleared on Executed/Unbound.
    pub fn resolve(
        &self,
        mode: &ModeKey,
        key: KeyCombo,
        pending: &mut Vec<KeyCombo>,
    ) -> DispatchResult {
        let mode_map = match self.modes.get(mode) {
            Some(m) => m,
            None => return DispatchResult::Unbound,
        };

        if pending.is_empty() {
            // Direct single-key lookup
            match mode_map.get(&key) {
                Some(TrieNode::Leaf(action)) => {
                    let action = action.clone();
                    if matches!(action, Action::FindCharForward | Action::FindCharBackward) {
                        return DispatchResult::AwaitingChar(action);
                    }
                    DispatchResult::Executed(action)
                }
                Some(TrieNode::Prefix(_)) => {
                    pending.push(key);
                    DispatchResult::Pending
                }
                None => DispatchResult::Unbound,
            }
        } else {
            // Walk the trie following the pending sequence
            let mut current_map = mode_map;

            for pending_key in pending.iter() {
                match current_map.get(pending_key) {
                    Some(TrieNode::Prefix(children)) => {
                        current_map = children;
                    }
                    _ => {
                        pending.clear();
                        return DispatchResult::Unbound;
                    }
                }
            }

            // Now look up the new key in the current position
            match current_map.get(&key) {
                Some(TrieNode::Leaf(action)) => {
                    let action = action.clone();
                    pending.clear();
                    if matches!(action, Action::FindCharForward | Action::FindCharBackward) {
                        return DispatchResult::AwaitingChar(action);
                    }
                    DispatchResult::Executed(action)
                }
                Some(TrieNode::Prefix(_)) => {
                    pending.push(key);
                    DispatchResult::Pending
                }
                None => {
                    pending.clear();
                    DispatchResult::Unbound
                }
            }
        }
    }
}
