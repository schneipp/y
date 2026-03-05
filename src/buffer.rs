#[derive(Debug, Clone)]
pub struct YBuffer {
    pub lines: Vec<YLine>,
}

impl YBuffer {
    pub fn new() -> Self {
        Self { lines: Vec::new() }
    }
    pub fn from(lines: Vec<YLine>) -> Self {
        Self { lines }
    }
}

#[derive(Debug, Clone)]
pub struct YLine {
    pub text: String,
}

impl YLine {
    pub fn new() -> Self {
        Self {
            text: String::new(),
        }
    }
    pub fn from(text: String) -> Self {
        Self { text }
    }
    pub fn with_style(self) -> Self {
        self
    }

    /// Returns the number of characters (not bytes) in the line.
    pub fn char_count(&self) -> usize {
        self.text.chars().count()
    }

    /// Converts a character index to a byte index.
    /// Returns the byte position of the nth character, or the string length if char_idx >= char_count.
    pub fn char_to_byte(&self, char_idx: usize) -> usize {
        self.text
            .char_indices()
            .nth(char_idx)
            .map(|(byte_idx, _)| byte_idx)
            .unwrap_or(self.text.len())
    }

    /// Computes the visual display column for a given character index,
    /// expanding tabs to `tab_width` spaces aligned to tab stops.
    pub fn visual_col(&self, char_idx: usize, tab_width: usize) -> usize {
        let mut vcol = 0;
        for (i, ch) in self.text.chars().enumerate() {
            if i >= char_idx {
                break;
            }
            if ch == '\t' {
                vcol += tab_width - (vcol % tab_width);
            } else {
                vcol += 1;
            }
        }
        vcol
    }

    /// Expands tabs to spaces, returning the display string.
    pub fn expanded_text(&self, tab_width: usize) -> String {
        let mut result = String::with_capacity(self.text.len());
        let mut vcol = 0;
        for ch in self.text.chars() {
            if ch == '\t' {
                let spaces = tab_width - (vcol % tab_width);
                for _ in 0..spaces {
                    result.push(' ');
                }
                vcol += spaces;
            } else {
                result.push(ch);
                vcol += 1;
            }
        }
        result
    }

    /// Inserts a character at the given character index (not byte index).
    pub fn insert_char_at(&mut self, char_idx: usize, c: char) {
        let byte_idx = self.char_to_byte(char_idx);
        self.text.insert(byte_idx, c);
    }

    /// Removes the character at the given character index (not byte index).
    /// Returns the removed character, or None if index is out of bounds.
    pub fn remove_char_at(&mut self, char_idx: usize) -> Option<char> {
        let byte_idx = self.char_to_byte(char_idx);
        if byte_idx < self.text.len() {
            Some(self.text.remove(byte_idx))
        } else {
            None
        }
    }

    /// Truncates the line at the given character index (not byte index).
    pub fn truncate_at_char(&mut self, char_idx: usize) {
        let byte_idx = self.char_to_byte(char_idx);
        self.text.truncate(byte_idx);
    }

    /// Splits the line at the given character index, returning the portion after the split.
    pub fn split_at_char(&mut self, char_idx: usize) -> String {
        let byte_idx = self.char_to_byte(char_idx);
        self.text.split_off(byte_idx)
    }

    /// Returns a substring from start_char to end_char (character indices, not bytes).
    pub fn slice_chars(&self, start_char: usize, end_char: usize) -> &str {
        let start_byte = self.char_to_byte(start_char);
        let end_byte = self.char_to_byte(end_char);
        &self.text[start_byte..end_byte]
    }
}

pub type BufferId = usize;

#[derive(Debug)]
pub struct BufferEntry {
    pub buffer: YBuffer,
    pub filename: Option<String>,
    pub modified: bool,
}

#[derive(Debug)]
pub struct BufferPool {
    buffers: Vec<BufferEntry>,
}

impl BufferPool {
    pub fn new() -> Self {
        Self {
            buffers: Vec::new(),
        }
    }

    pub fn add(&mut self, buffer: YBuffer) -> BufferId {
        let id = self.buffers.len();
        self.buffers.push(BufferEntry {
            buffer,
            filename: None,
            modified: false,
        });
        id
    }

    pub fn add_with_filename(&mut self, buffer: YBuffer, filename: Option<String>) -> BufferId {
        let id = self.buffers.len();
        self.buffers.push(BufferEntry {
            buffer,
            filename,
            modified: false,
        });
        id
    }

    pub fn get(&self, id: BufferId) -> &YBuffer {
        &self.buffers[id].buffer
    }

    pub fn get_mut(&mut self, id: BufferId) -> &mut YBuffer {
        &mut self.buffers[id].buffer
    }

    pub fn get_entry(&self, id: BufferId) -> &BufferEntry {
        &self.buffers[id]
    }

    pub fn get_entry_mut(&mut self, id: BufferId) -> &mut BufferEntry {
        &mut self.buffers[id]
    }

    pub fn len(&self) -> usize {
        self.buffers.len()
    }

    /// Get list of (BufferId, display_name) for all buffers
    pub fn buffer_list(&self) -> Vec<(BufferId, String)> {
        self.buffers
            .iter()
            .enumerate()
            .map(|(id, entry)| {
                let name = entry
                    .filename
                    .as_deref()
                    .unwrap_or("[No Name]")
                    .to_string();
                let display = if entry.modified {
                    format!("{} [+]", name)
                } else {
                    name
                };
                (id, display)
            })
            .collect()
    }

    /// Find buffer by filename, returns None if not found
    pub fn find_by_filename(&self, filename: &str) -> Option<BufferId> {
        self.buffers.iter().position(|entry| {
            entry.filename.as_deref() == Some(filename)
        })
    }
}
