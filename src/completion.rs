use serde_json::Value;

#[derive(Debug, Clone)]
pub struct CompletionItem {
    pub label: String,
    pub detail: Option<String>,
    pub insert_text: Option<String>,
    pub kind: Option<CompletionKind>,
    pub sort_text: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum CompletionKind {
    Text,
    Method,
    Function,
    Constructor,
    Field,
    Variable,
    Class,
    Interface,
    Module,
    Property,
    Unit,
    Value,
    Enum,
    Keyword,
    Snippet,
    Color,
    File,
    Reference,
    Folder,
    EnumMember,
    Constant,
    Struct,
    Event,
    Operator,
    TypeParameter,
}

impl CompletionKind {
    pub fn from_lsp(kind: u64) -> Self {
        match kind {
            1 => Self::Text,
            2 => Self::Method,
            3 => Self::Function,
            4 => Self::Constructor,
            5 => Self::Field,
            6 => Self::Variable,
            7 => Self::Class,
            8 => Self::Interface,
            9 => Self::Module,
            10 => Self::Property,
            11 => Self::Unit,
            12 => Self::Value,
            13 => Self::Enum,
            14 => Self::Keyword,
            15 => Self::Snippet,
            16 => Self::Color,
            17 => Self::File,
            18 => Self::Reference,
            19 => Self::Folder,
            20 => Self::EnumMember,
            21 => Self::Constant,
            22 => Self::Struct,
            23 => Self::Event,
            24 => Self::Operator,
            25 => Self::TypeParameter,
            _ => Self::Text,
        }
    }

    pub fn short_label(&self) -> &str {
        match self {
            Self::Text => "tx",
            Self::Method => "me",
            Self::Function => "fn",
            Self::Constructor => "co",
            Self::Field => "fd",
            Self::Variable => "vr",
            Self::Class => "cl",
            Self::Interface => "if",
            Self::Module => "md",
            Self::Property => "pr",
            Self::Unit => "un",
            Self::Value => "vl",
            Self::Enum => "en",
            Self::Keyword => "kw",
            Self::Snippet => "sn",
            Self::Color => "co",
            Self::File => "fi",
            Self::Reference => "rf",
            Self::Folder => "dr",
            Self::EnumMember => "em",
            Self::Constant => "ct",
            Self::Struct => "st",
            Self::Event => "ev",
            Self::Operator => "op",
            Self::TypeParameter => "tp",
        }
    }
}

impl CompletionItem {
    pub fn from_lsp(value: &Value) -> Option<Self> {
        let label = value.get("label")?.as_str()?.to_string();
        let detail = value.get("detail").and_then(|v| v.as_str()).map(|s| s.to_string());
        let insert_text = value
            .get("textEdit")
            .and_then(|te| te.get("newText"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                value
                    .get("insertText")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            });
        let kind = value
            .get("kind")
            .and_then(|v| v.as_u64())
            .map(CompletionKind::from_lsp);
        let sort_text = value
            .get("sortText")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        Some(Self {
            label,
            detail,
            insert_text,
            kind,
            sort_text,
        })
    }

    pub fn text_to_insert(&self) -> &str {
        self.insert_text.as_deref().unwrap_or(&self.label)
    }
}

pub struct CompletionState {
    pub items: Vec<CompletionItem>,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub active: bool,
    pub trigger_col: usize,
    pub trigger_row: usize,
    pub prefix: String,
    pub pending_request_id: Option<i64>,
    pub document_version: i64,
    /// Cached ghost text: the suffix of the first filtered item beyond the prefix.
    ghost_text_cache: Option<String>,
}

impl CompletionState {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            filtered: Vec::new(),
            selected: 0,
            active: false,
            trigger_col: 0,
            trigger_row: 0,
            prefix: String::new(),
            pending_request_id: None,
            document_version: 1,
            ghost_text_cache: None,
        }
    }

    pub fn activate(
        &mut self,
        items: Vec<CompletionItem>,
        trigger_row: usize,
        trigger_col: usize,
        prefix: &str,
    ) {
        self.items = items;
        self.trigger_row = trigger_row;
        self.trigger_col = trigger_col;
        self.prefix = prefix.to_string();
        self.selected = 0;
        self.filter();
        self.active = !self.filtered.is_empty();
    }

    pub fn dismiss(&mut self) {
        self.active = false;
        self.items.clear();
        self.filtered.clear();
        self.pending_request_id = None;
        self.ghost_text_cache = None;
    }

    pub fn update_prefix(&mut self, prefix: &str) {
        self.prefix = prefix.to_string();
        self.filter();
        if self.filtered.is_empty() {
            self.active = false;
        }
    }

    fn filter(&mut self) {
        let prefix_lower = self.prefix.to_lowercase();
        self.filtered = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                if prefix_lower.is_empty() {
                    true
                } else {
                    fuzzy_match(&item.label.to_lowercase(), &prefix_lower)
                }
            })
            .map(|(i, _)| i)
            .collect();

        if self.selected >= self.filtered.len() {
            self.selected = 0;
        }

        // Update ghost text cache
        self.ghost_text_cache = self.filtered.first().and_then(|&idx| {
            let item = &self.items[idx];
            let full = strip_snippets(item.text_to_insert());
            if full.len() > self.prefix.len() && full.to_lowercase().starts_with(&prefix_lower) {
                Some(full[self.prefix.len()..].to_string())
            } else if full.len() > self.prefix.len() {
                // Fuzzy match — show the full text minus prefix length as hint
                Some(full[self.prefix.len()..].to_string())
            } else {
                None
            }
        });
    }

    pub fn navigate(&mut self, delta: i32) {
        if self.filtered.is_empty() {
            return;
        }
        let len = self.filtered.len() as i32;
        self.selected = ((self.selected as i32 + delta).rem_euclid(len)) as usize;
    }

    pub fn selected_item(&self) -> Option<&CompletionItem> {
        self.filtered
            .get(self.selected)
            .and_then(|&idx| self.items.get(idx))
    }

    /// Get the ghost text suffix for the first filtered completion item.
    /// Returns the portion of the completion text that extends beyond the current prefix.
    pub fn ghost_text(&self) -> Option<&str> {
        self.ghost_text_cache.as_deref()
    }

    pub fn bump_version(&mut self) -> i64 {
        self.document_version += 1;
        self.document_version
    }
}

/// Simple fuzzy match: all chars of pattern appear in order in text.
fn fuzzy_match(text: &str, pattern: &str) -> bool {
    let mut text_chars = text.chars();
    for p in pattern.chars() {
        loop {
            match text_chars.next() {
                Some(t) if t == p => break,
                Some(_) => continue,
                None => return false,
            }
        }
    }
    true
}

/// Strip LSP snippet placeholders ($0, $1, ${0:text} etc.) from insert text.
pub fn strip_snippets(text: &str) -> String {
    let mut result = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' {
            match chars.peek() {
                Some(&'{') => {
                    chars.next();
                    // Skip digits
                    while let Some(&c) = chars.peek() {
                        if c.is_ascii_digit() {
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    if chars.peek() == Some(&':') {
                        chars.next();
                        // Keep text until closing '}'
                        let mut depth = 1;
                        while let Some(c) = chars.next() {
                            if c == '{' {
                                depth += 1;
                            }
                            if c == '}' {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            result.push(c);
                        }
                    } else if chars.peek() == Some(&'}') {
                        chars.next();
                    }
                }
                Some(&c) if c.is_ascii_digit() => {
                    chars.next();
                }
                _ => result.push('$'),
            }
        } else {
            result.push(c);
        }
    }
    result
}
