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
