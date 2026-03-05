use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::plugins::{Plugin, PluginContext, PluginRenderContext};

#[derive(Debug, Clone)]
struct TreeNode {
    name: String,
    path: PathBuf,
    is_dir: bool,
    depth: usize,
    git_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
enum InputMode {
    Normal,
    Filter,
    CreateFile,
    CreateDir,
    Rename,
    ConfirmDelete,
}

pub struct PendingOpen {
    pub path: String,
}

pub struct FileTreePlugin {
    active: bool,
    root: PathBuf,
    visible_nodes: Vec<TreeNode>,
    expanded: HashSet<PathBuf>,
    selected: usize,
    scroll_offset: usize,
    git_statuses: std::collections::HashMap<String, String>,
    input_mode: InputMode,
    input_buffer: String,
    filter_query: String,
    pub pending_open: Option<PendingOpen>,
    width: u16,
    colors: TreeColors,
}

struct TreeColors {
    border: Color,
    bg: Color,
    fg: Color,
    selected_bg: Color,
    dir_fg: Color,
    file_fg: Color,
    git_modified: Color,
    git_untracked: Color,
    git_staged: Color,
    accent: Color,
    dim: Color,
}

impl Default for TreeColors {
    fn default() -> Self {
        Self {
            border: Color::Cyan,
            bg: Color::Black,
            fg: Color::White,
            selected_bg: Color::DarkGray,
            dir_fg: Color::Blue,
            file_fg: Color::White,
            git_modified: Color::Yellow,
            git_untracked: Color::Green,
            git_staged: Color::Cyan,
            accent: Color::Cyan,
            dim: Color::DarkGray,
        }
    }
}

impl FileTreePlugin {
    pub fn new() -> Self {
        let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut plugin = Self {
            active: false,
            root: root.clone(),
            visible_nodes: Vec::new(),
            expanded: HashSet::new(),
            selected: 0,
            scroll_offset: 0,
            git_statuses: std::collections::HashMap::new(),
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            filter_query: String::new(),
            pending_open: None,
            width: 35,
            colors: TreeColors::default(),
        };
        plugin.expanded.insert(root);
        plugin
    }

    pub fn set_colors(
        &mut self,
        border: Color,
        bg: Color,
        fg: Color,
        selected_bg: Color,
        accent: Color,
        dim: Color,
    ) {
        self.colors.border = border;
        self.colors.bg = bg;
        self.colors.fg = fg;
        self.colors.selected_bg = selected_bg;
        self.colors.accent = accent;
        self.colors.dim = dim;
    }

    pub fn activate_tree(&mut self) {
        self.active = true;
        self.input_mode = InputMode::Normal;
        self.input_buffer.clear();
        self.filter_query.clear();
        self.refresh_git_status();
        self.rebuild_visible();
    }

    fn refresh_git_status(&mut self) {
        self.git_statuses.clear();
        let output = std::process::Command::new("git")
            .args(["status", "--porcelain=v1"])
            .current_dir(&self.root)
            .output();

        if let Ok(out) = output {
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines() {
                if line.len() < 4 {
                    continue;
                }
                let index_char = line.chars().next().unwrap_or(' ');
                let worktree_char = line.chars().nth(1).unwrap_or(' ');
                let path = line[3..].trim_matches('"').to_string();

                let status = if index_char == '?' {
                    "?".to_string()
                } else if index_char != ' ' && worktree_char != ' ' {
                    format!("{}{}", index_char, worktree_char)
                } else if index_char != ' ' {
                    format!("{}", index_char)
                } else {
                    format!("{}", worktree_char)
                };

                self.git_statuses.insert(path, status);
            }
        }
    }

    fn rebuild_visible(&mut self) {
        self.visible_nodes.clear();
        self.build_tree(&self.root.clone(), 0);

        if !self.filter_query.is_empty() {
            let query = self.filter_query.to_lowercase();
            self.visible_nodes.retain(|node| {
                node.is_dir || node.name.to_lowercase().contains(&query)
            });
            // Also remove dirs that have no matching children below them
            self.prune_empty_dirs();
        }

        if self.selected >= self.visible_nodes.len() {
            self.selected = self.visible_nodes.len().saturating_sub(1);
        }
    }

    fn prune_empty_dirs(&mut self) {
        // Iterate from bottom up and remove dirs that have no children after them
        let mut i = self.visible_nodes.len();
        while i > 0 {
            i -= 1;
            if self.visible_nodes[i].is_dir {
                let depth = self.visible_nodes[i].depth;
                let has_children = if i + 1 < self.visible_nodes.len() {
                    self.visible_nodes[i + 1].depth > depth
                } else {
                    false
                };
                if !has_children && !self.filter_query.is_empty() {
                    self.visible_nodes.remove(i);
                }
            }
        }
    }

    fn build_tree(&mut self, dir: &Path, depth: usize) {
        let mut entries: Vec<_> = match std::fs::read_dir(dir) {
            Ok(entries) => entries.filter_map(|e| e.ok()).collect(),
            Err(_) => return,
        };

        entries.sort_by(|a, b| {
            let a_is_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let b_is_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
            match (a_is_dir, b_is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.file_name().cmp(&b.file_name()),
            }
        });

        for entry in entries {
            let name = entry.file_name().to_string_lossy().to_string();
            let path = entry.path();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);

            // Skip hidden files and common large directories
            if name.starts_with('.') || name == "node_modules" || name == "target" || name == "__pycache__" {
                continue;
            }

            let rel_path = path.strip_prefix(&self.root).unwrap_or(&path);
            let rel_str = rel_path.to_string_lossy().to_string();
            let git_status = self.git_statuses.get(&rel_str).cloned();

            // For directories, check if any child has git status
            let dir_git_status = if is_dir && git_status.is_none() {
                let prefix = format!("{}/", rel_str);
                if self.git_statuses.keys().any(|k| k.starts_with(&prefix)) {
                    Some("M".to_string())
                } else {
                    None
                }
            } else {
                git_status
            };

            self.visible_nodes.push(TreeNode {
                name,
                path: path.clone(),
                is_dir,
                depth,
                git_status: dir_git_status,
            });

            if is_dir && self.expanded.contains(&path) {
                self.build_tree(&path, depth + 1);
            }
        }
    }

    fn toggle_expand(&mut self) {
        if let Some(node) = self.visible_nodes.get(self.selected) {
            if node.is_dir {
                let path = node.path.clone();
                if self.expanded.contains(&path) {
                    self.expanded.remove(&path);
                } else {
                    self.expanded.insert(path);
                }
                self.rebuild_visible();
            }
        }
    }

    fn open_selected(&mut self, ctx: &mut PluginContext) {
        if let Some(node) = self.visible_nodes.get(self.selected) {
            if node.is_dir {
                self.toggle_expand();
            } else {
                let path = node.path.to_string_lossy().to_string();
                self.pending_open = Some(PendingOpen { path });
                self.active = false;
                *ctx.mode = ctx.default_mode.clone();
            }
        }
    }

    fn collapse_or_parent(&mut self) {
        if let Some(node) = self.visible_nodes.get(self.selected) {
            if node.is_dir && self.expanded.contains(&node.path) {
                let path = node.path.clone();
                self.expanded.remove(&path);
                self.rebuild_visible();
            } else {
                // Jump to parent directory
                if let Some(parent) = node.path.parent() {
                    let parent = parent.to_path_buf();
                    for (i, n) in self.visible_nodes.iter().enumerate() {
                        if n.path == parent {
                            self.selected = i;
                            break;
                        }
                    }
                }
            }
        }
    }

    fn expand_or_enter(&mut self, ctx: &mut PluginContext) {
        if let Some(node) = self.visible_nodes.get(self.selected) {
            if node.is_dir {
                if !self.expanded.contains(&node.path) {
                    let path = node.path.clone();
                    self.expanded.insert(path);
                    self.rebuild_visible();
                } else {
                    // Already expanded, move into it
                    if self.selected + 1 < self.visible_nodes.len() {
                        self.selected += 1;
                    }
                }
            } else {
                self.open_selected(ctx);
            }
        }
    }

    fn create_file(&mut self) {
        if self.input_buffer.trim().is_empty() {
            self.input_mode = InputMode::Normal;
            return;
        }

        let parent = self.get_current_dir();
        let new_path = parent.join(&self.input_buffer);

        if let Some(dir) = new_path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::File::create(&new_path);

        self.input_buffer.clear();
        self.input_mode = InputMode::Normal;
        self.refresh_git_status();
        self.rebuild_visible();
    }

    fn create_dir(&mut self) {
        if self.input_buffer.trim().is_empty() {
            self.input_mode = InputMode::Normal;
            return;
        }

        let parent = self.get_current_dir();
        let new_path = parent.join(&self.input_buffer);
        let _ = std::fs::create_dir_all(&new_path);

        self.input_buffer.clear();
        self.input_mode = InputMode::Normal;
        self.refresh_git_status();
        self.rebuild_visible();
    }

    fn delete_selected(&mut self) {
        if let Some(node) = self.visible_nodes.get(self.selected) {
            let path = node.path.clone();
            if node.is_dir {
                let _ = std::fs::remove_dir_all(&path);
            } else {
                let _ = std::fs::remove_file(&path);
            }
        }
        self.input_mode = InputMode::Normal;
        self.refresh_git_status();
        self.rebuild_visible();
    }

    fn rename_selected(&mut self) {
        if self.input_buffer.trim().is_empty() {
            self.input_mode = InputMode::Normal;
            return;
        }

        if let Some(node) = self.visible_nodes.get(self.selected) {
            if let Some(parent) = node.path.parent() {
                let new_path = parent.join(&self.input_buffer);
                let _ = std::fs::rename(&node.path, &new_path);
            }
        }

        self.input_buffer.clear();
        self.input_mode = InputMode::Normal;
        self.refresh_git_status();
        self.rebuild_visible();
    }

    fn get_current_dir(&self) -> PathBuf {
        if let Some(node) = self.visible_nodes.get(self.selected) {
            if node.is_dir {
                node.path.clone()
            } else {
                node.path.parent().unwrap_or(&self.root).to_path_buf()
            }
        } else {
            self.root.clone()
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent, ctx: &mut PluginContext) -> bool {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.active = false;
                self.filter_query.clear();
                *ctx.mode = ctx.default_mode.clone();
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if self.selected + 1 < self.visible_nodes.len() {
                    self.selected += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
            }
            KeyCode::Enter => {
                self.open_selected(ctx);
            }
            KeyCode::Char('l') | KeyCode::Right => {
                self.expand_or_enter(ctx);
            }
            KeyCode::Char('h') | KeyCode::Left => {
                self.collapse_or_parent();
            }
            KeyCode::Char('a') => {
                self.input_mode = InputMode::CreateFile;
                self.input_buffer.clear();
            }
            KeyCode::Char('A') => {
                self.input_mode = InputMode::CreateDir;
                self.input_buffer.clear();
            }
            KeyCode::Char('d') => {
                if !self.visible_nodes.is_empty() {
                    self.input_mode = InputMode::ConfirmDelete;
                }
            }
            KeyCode::Char('r') => {
                if let Some(node) = self.visible_nodes.get(self.selected) {
                    self.input_buffer = node.name.clone();
                    self.input_mode = InputMode::Rename;
                }
            }
            KeyCode::Char('/') => {
                self.input_mode = InputMode::Filter;
                self.filter_query.clear();
                self.input_buffer.clear();
            }
            KeyCode::Char('R') => {
                self.refresh_git_status();
                self.rebuild_visible();
            }
            KeyCode::Char('g') if key.modifiers == KeyModifiers::NONE => {
                self.selected = 0;
            }
            KeyCode::Char('G') => {
                self.selected = self.visible_nodes.len().saturating_sub(1);
            }
            KeyCode::Char('x') => {
                // Collapse all
                let root = self.root.clone();
                self.expanded.clear();
                self.expanded.insert(root);
                self.selected = 0;
                self.rebuild_visible();
            }
            KeyCode::Char('o') => {
                // Expand all (one level deep from current)
                let dirs: Vec<PathBuf> = self.visible_nodes.iter()
                    .filter(|n| n.is_dir)
                    .map(|n| n.path.clone())
                    .collect();
                for d in dirs {
                    self.expanded.insert(d);
                }
                self.rebuild_visible();
            }
            KeyCode::Char('.') => {
                // Toggle hidden files - for now just refresh
                self.rebuild_visible();
            }
            KeyCode::Char('y') => {
                // Yank path to clipboard (copy path)
                if let Some(node) = self.visible_nodes.get(self.selected) {
                    let path_str = node.path.to_string_lossy().to_string();
                    // Try xclip or xsel
                    let _ = std::process::Command::new("xclip")
                        .args(["-selection", "clipboard"])
                        .stdin(std::process::Stdio::piped())
                        .spawn()
                        .and_then(|mut child| {
                            use std::io::Write;
                            if let Some(ref mut stdin) = child.stdin {
                                stdin.write_all(path_str.as_bytes())?;
                            }
                            child.wait()
                        });
                }
            }
            _ => return true,
        }
        true
    }

    fn handle_input_key(&mut self, key: KeyEvent, ctx: &mut PluginContext) -> bool {
        match key.code {
            KeyCode::Esc => {
                if self.input_mode == InputMode::Filter {
                    self.filter_query.clear();
                    self.rebuild_visible();
                }
                self.input_mode = InputMode::Normal;
                self.input_buffer.clear();
            }
            KeyCode::Enter => {
                match self.input_mode {
                    InputMode::CreateFile => self.create_file(),
                    InputMode::CreateDir => self.create_dir(),
                    InputMode::Rename => self.rename_selected(),
                    InputMode::ConfirmDelete => {
                        self.input_mode = InputMode::Normal;
                        self.input_buffer.clear();
                    }
                    InputMode::Filter => {
                        self.filter_query = self.input_buffer.clone();
                        self.input_buffer.clear();
                        self.input_mode = InputMode::Normal;
                        self.rebuild_visible();
                    }
                    _ => {}
                }
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
                if self.input_mode == InputMode::Filter {
                    self.filter_query = self.input_buffer.clone();
                    self.rebuild_visible();
                }
            }
            KeyCode::Char(c) => {
                if self.input_mode == InputMode::ConfirmDelete {
                    if c == 'y' || c == 'Y' {
                        self.delete_selected();
                    } else {
                        self.input_mode = InputMode::Normal;
                    }
                    self.input_buffer.clear();
                    return true;
                }
                self.input_buffer.push(c);
                if self.input_mode == InputMode::Filter {
                    self.filter_query = self.input_buffer.clone();
                    self.rebuild_visible();
                }
            }
            _ => {}
        }
        let _ = ctx;
        true
    }

    fn render_tree(&self, area: Rect, buf: &mut Buffer) {
        if !self.active {
            return;
        }

        let colors = &self.colors;
        let sidebar_width = self.width.min(area.width);

        let sidebar_area = Rect {
            x: area.x,
            y: area.y,
            width: sidebar_width,
            height: area.height,
        };

        Clear.render(sidebar_area, buf);

        let root_name = self.root.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| self.root.to_string_lossy().to_string());

        let title = format!(" {} ", root_name);
        let block = Block::default()
            .title(title.as_str())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(colors.border))
            .style(Style::default().bg(colors.bg).fg(colors.fg));

        let inner = block.inner(sidebar_area);
        block.render(sidebar_area, buf);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let mut lines: Vec<Line> = Vec::new();

        // Filter bar
        if !self.filter_query.is_empty() || self.input_mode == InputMode::Filter {
            let filter_text = if self.input_mode == InputMode::Filter {
                format!("/{}_", self.input_buffer)
            } else {
                format!("/{}", self.filter_query)
            };
            lines.push(Line::from(Span::styled(
                filter_text,
                Style::default().fg(colors.accent),
            )));
            lines.push(Line::from(""));
        }

        // Input bar for create/rename/delete
        match self.input_mode {
            InputMode::CreateFile => {
                lines.push(Line::from(Span::styled(
                    format!("New file: {}_", self.input_buffer),
                    Style::default().fg(colors.accent),
                )));
                lines.push(Line::from(""));
            }
            InputMode::CreateDir => {
                lines.push(Line::from(Span::styled(
                    format!("New dir: {}_", self.input_buffer),
                    Style::default().fg(colors.accent),
                )));
                lines.push(Line::from(""));
            }
            InputMode::Rename => {
                lines.push(Line::from(Span::styled(
                    format!("Rename: {}_", self.input_buffer),
                    Style::default().fg(colors.accent),
                )));
                lines.push(Line::from(""));
            }
            InputMode::ConfirmDelete => {
                if let Some(node) = self.visible_nodes.get(self.selected) {
                    lines.push(Line::from(Span::styled(
                        format!("Delete {}? (y/N)", node.name),
                        Style::default().fg(Color::Red),
                    )));
                    lines.push(Line::from(""));
                }
            }
            _ => {}
        }

        let header_lines = lines.len();
        let available_height = inner.height as usize - header_lines;

        // Adjust scroll
        let scroll = if self.selected >= self.scroll_offset + available_height {
            self.selected - available_height + 1
        } else if self.selected < self.scroll_offset {
            self.selected
        } else {
            self.scroll_offset
        };

        let max_name_width = inner.width.saturating_sub(2) as usize;

        for (idx, node) in self.visible_nodes.iter().enumerate().skip(scroll).take(available_height) {
            let indent = "  ".repeat(node.depth);
            let is_selected = idx == self.selected;
            let sel_bg = if is_selected { colors.selected_bg } else { colors.bg };

            let (icon, icon_color) = if node.is_dir {
                if self.expanded.contains(&node.path) {
                    ("󰝰 ", Color::Rgb(86, 156, 214))
                } else {
                    ("󰉋 ", Color::Rgb(86, 156, 214))
                }
            } else {
                Self::file_icon(&node.name)
            };

            let git_indicator = match &node.git_status {
                Some(s) if s.contains('?') => (" ?", colors.git_untracked),
                Some(s) if s.contains('M') || s.contains('m') => (" M", colors.git_modified),
                Some(s) if s.contains('A') => (" A", colors.git_staged),
                Some(s) if s.contains('D') => (" D", Color::Red),
                Some(s) if s.contains('R') => (" R", colors.accent),
                Some(_) => (" *", colors.git_modified),
                None => ("", colors.fg),
            };

            let name_fg = if node.is_dir { colors.dir_fg } else { colors.file_fg };

            let mut spans = vec![];

            // Indent
            if !indent.is_empty() {
                spans.push(Span::styled(
                    indent,
                    Style::default().bg(sel_bg),
                ));
            }

            // Chevron for dirs
            if node.is_dir {
                let chevron = if self.expanded.contains(&node.path) { " " } else { " " };
                spans.push(Span::styled(
                    chevron,
                    Style::default().fg(colors.dim).bg(sel_bg),
                ));
            } else {
                spans.push(Span::styled(
                    " ",
                    Style::default().bg(sel_bg),
                ));
            }

            // Icon with its own color
            spans.push(Span::styled(
                icon,
                Style::default().fg(icon_color).bg(sel_bg),
            ));

            // Filename
            let name_style = if is_selected {
                Style::default().fg(name_fg).bg(sel_bg).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(name_fg).bg(sel_bg)
            };

            // Truncate name if needed
            let avail = max_name_width.saturating_sub(node.depth * 2 + 4);
            let display_name = if node.name.len() > avail {
                format!("{}…", &node.name[..avail.saturating_sub(1)])
            } else {
                node.name.clone()
            };

            spans.push(Span::styled(display_name, name_style));

            // Git indicator
            if !git_indicator.0.is_empty() {
                spans.push(Span::styled(
                    git_indicator.0.to_string(),
                    Style::default().fg(git_indicator.1).bg(sel_bg),
                ));
            }

            lines.push(Line::from(spans));
        }

        // Help line at bottom
        if inner.height > 2 {
            while lines.len() < inner.height as usize - 1 {
                lines.push(Line::from(""));
            }
            lines.push(Line::from(vec![
                Span::styled(" a", Style::default().fg(colors.accent)),
                Span::styled("dd ", Style::default().fg(colors.dim)),
                Span::styled("d", Style::default().fg(colors.accent)),
                Span::styled("el ", Style::default().fg(colors.dim)),
                Span::styled("r", Style::default().fg(colors.accent)),
                Span::styled("en ", Style::default().fg(colors.dim)),
                Span::styled("/", Style::default().fg(colors.accent)),
                Span::styled("flt", Style::default().fg(colors.dim)),
            ]));
        }

        let text = Text::from(lines);
        Paragraph::new(text).render(inner, buf);
    }

    fn file_icon(name: &str) -> (&'static str, Color) {
        // Check exact filename first
        let icon = match name {
            "Cargo.toml" | "Cargo.lock" => return ("󰏗 ", Color::Rgb(222, 165, 72)),
            "Makefile" | "CMakeLists.txt" => return (" ", Color::Rgb(130, 130, 130)),
            "Dockerfile" | "docker-compose.yml" | "docker-compose.yaml"
                => return ("󰡨 ", Color::Rgb(56, 151, 240)),
            ".gitignore" | ".gitmodules" | ".gitattributes"
                => return (" ", Color::Rgb(224, 93, 68)),
            ".env" | ".env.local" | ".env.example"
                => return (" ", Color::Rgb(250, 200, 50)),
            "package.json" | "package-lock.json"
                => return (" ", Color::Rgb(232, 197, 72)),
            "tsconfig.json" | "tsconfig.build.json"
                => return (" ", Color::Rgb(49, 120, 198)),
            "README.md" | "readme.md" => return ("󰂺 ", Color::Rgb(66, 165, 245)),
            "LICENSE" | "LICENSE.md" => return ("󰿃 ", Color::Rgb(200, 180, 50)),
            "CLAUDE.md" => return ("󰚩 ", Color::Rgb(204, 136, 63)),
            ".eslintrc" | ".eslintrc.js" | ".eslintrc.json" | "eslint.config.js" | "eslint.config.mjs"
                => return ("󰱺 ", Color::Rgb(75, 50, 175)),
            ".prettierrc" | ".prettierrc.js" | "prettier.config.js"
                => return (" ", Color::Rgb(86, 179, 174)),
            "flake.nix" | "flake.lock" => return (" ", Color::Rgb(126, 186, 228)),
            "shell.nix" | "default.nix" => return (" ", Color::Rgb(126, 186, 228)),
            "go.mod" | "go.sum" => return (" ", Color::Rgb(0, 173, 216)),
            "requirements.txt" | "pyproject.toml" | "setup.py" | "setup.cfg"
                => return (" ", Color::Rgb(55, 118, 171)),
            "Gemfile" | "Gemfile.lock" => return (" ", Color::Rgb(204, 52, 45)),
            "build.zig" | "build.zig.zon" => return (" ", Color::Rgb(236, 169, 56)),
            _ => {}
        };
        let _ = icon;

        let ext = name.rsplit('.').next().unwrap_or("");
        match ext {
            // Rust
            "rs" => ("󱘗 ", Color::Rgb(222, 165, 72)),
            // JavaScript
            "js" | "mjs" | "cjs" => (" ", Color::Rgb(241, 224, 90)),
            "jsx" => (" ", Color::Rgb(97, 218, 251)),
            // TypeScript
            "ts" | "mts" | "cts" => (" ", Color::Rgb(49, 120, 198)),
            "tsx" => (" ", Color::Rgb(97, 218, 251)),
            // Python
            "py" | "pyw" | "pyi" => (" ", Color::Rgb(55, 118, 171)),
            "ipynb" => (" ", Color::Rgb(238, 130, 44)),
            // Go
            "go" => (" ", Color::Rgb(0, 173, 216)),
            // C / C++
            "c" => (" ", Color::Rgb(85, 135, 195)),
            "h" => (" ", Color::Rgb(130, 160, 200)),
            "cpp" | "cc" | "cxx" | "c++" => (" ", Color::Rgb(85, 135, 195)),
            "hpp" | "hh" | "hxx" | "h++" => (" ", Color::Rgb(130, 160, 200)),
            // C#
            "cs" => ("󰌛 ", Color::Rgb(96, 69, 154)),
            // Java / Kotlin
            "java" => (" ", Color::Rgb(204, 62, 68)),
            "kt" | "kts" => (" ", Color::Rgb(129, 105, 194)),
            "gradle" => (" ", Color::Rgb(2, 120, 55)),
            // Lua
            "lua" => (" ", Color::Rgb(81, 160, 207)),
            // Zig
            "zig" => (" ", Color::Rgb(236, 169, 56)),
            // Ruby
            "rb" | "rake" | "gemspec" => (" ", Color::Rgb(204, 52, 45)),
            "erb" => (" ", Color::Rgb(204, 52, 45)),
            // PHP
            "php" => (" ", Color::Rgb(119, 109, 186)),
            // Swift
            "swift" => (" ", Color::Rgb(240, 81, 56)),
            // Dart / Flutter
            "dart" => (" ", Color::Rgb(3, 155, 229)),
            // Elixir / Erlang
            "ex" | "exs" => (" ", Color::Rgb(110, 74, 126)),
            "erl" | "hrl" => (" ", Color::Rgb(163, 0, 36)),
            // Haskell
            "hs" | "lhs" => (" ", Color::Rgb(94, 80, 134)),
            // OCaml
            "ml" | "mli" => (" ", Color::Rgb(227, 122, 29)),
            // Scala
            "scala" | "sc" => (" ", Color::Rgb(204, 62, 68)),
            // Clojure
            "clj" | "cljs" | "cljc" | "edn" => (" ", Color::Rgb(98, 182, 57)),
            // R
            "r" | "R" => ("󰟔 ", Color::Rgb(39, 108, 194)),
            // Shell
            "sh" | "bash" => (" ", Color::Rgb(130, 180, 70)),
            "zsh" => (" ", Color::Rgb(130, 180, 70)),
            "fish" => (" ", Color::Rgb(130, 180, 70)),
            "ps1" | "psm1" => ("󰨊 ", Color::Rgb(55, 130, 200)),
            // Nix
            "nix" => (" ", Color::Rgb(126, 186, 228)),
            // Config / Data
            "toml" => (" ", Color::Rgb(130, 130, 130)),
            "yaml" | "yml" => (" ", Color::Rgb(200, 80, 80)),
            "json" | "jsonc" | "json5" => (" ", Color::Rgb(241, 224, 90)),
            "xml" => ("󰗀 ", Color::Rgb(227, 122, 29)),
            "csv" | "tsv" => (" ", Color::Rgb(105, 170, 70)),
            "sql" => (" ", Color::Rgb(218, 218, 218)),
            "graphql" | "gql" => (" ", Color::Rgb(229, 53, 171)),
            "proto" => (" ", Color::Rgb(130, 130, 130)),
            "env" => (" ", Color::Rgb(250, 200, 50)),
            // Markup / Docs
            "md" | "mdx" => (" ", Color::Rgb(66, 165, 245)),
            "rst" => (" ", Color::Rgb(130, 130, 130)),
            "tex" | "latex" => (" ", Color::Rgb(58, 133, 88)),
            "typ" => (" ", Color::Rgb(35, 155, 175)),
            "org" => (" ", Color::Rgb(119, 170, 153)),
            // Web
            "html" | "htm" => (" ", Color::Rgb(228, 77, 38)),
            "css" => (" ", Color::Rgb(86, 156, 214)),
            "scss" | "sass" => (" ", Color::Rgb(205, 103, 153)),
            "less" => (" ", Color::Rgb(86, 61, 124)),
            "vue" => (" ", Color::Rgb(65, 184, 131)),
            "svelte" => (" ", Color::Rgb(255, 62, 0)),
            "astro" => (" ", Color::Rgb(255, 93, 1)),
            // Images
            "png" | "jpg" | "jpeg" | "gif" | "bmp" | "ico" | "webp" | "avif"
                => (" ", Color::Rgb(168, 130, 209)),
            "svg" => ("󰜡 ", Color::Rgb(255, 180, 50)),
            // Fonts
            "ttf" | "otf" | "woff" | "woff2" => (" ", Color::Rgb(200, 200, 200)),
            // Video / Audio
            "mp4" | "mkv" | "avi" | "mov" | "webm"
                => ("󰿎 ", Color::Rgb(253, 183, 75)),
            "mp3" | "wav" | "flac" | "ogg" | "aac"
                => ("󰎈 ", Color::Rgb(0, 188, 212)),
            // Archives
            "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" | "zst"
                => (" ", Color::Rgb(175, 135, 35)),
            // Binary / Compiled
            "wasm" => (" ", Color::Rgb(101, 79, 240)),
            "o" | "a" | "so" | "dylib" | "dll" => (" ", Color::Rgb(130, 130, 130)),
            // Docker / DevOps
            "tf" | "tfvars" => ("󱁢 ", Color::Rgb(93, 76, 228)),
            // Config files
            "ini" | "cfg" | "conf" => (" ", Color::Rgb(130, 130, 130)),
            "editorconfig" => (" ", Color::Rgb(130, 130, 130)),
            // Lock files
            "lock" => (" ", Color::Rgb(130, 130, 130)),
            // Git
            "diff" | "patch" => (" ", Color::Rgb(224, 93, 68)),
            // Vim
            "vim" => (" ", Color::Rgb(1, 152, 51)),
            // Text
            "txt" | "log" => ("󰈙 ", Color::Rgb(170, 170, 170)),
            // PDF / Documents
            "pdf" => (" ", Color::Rgb(232, 60, 50)),
            "doc" | "docx" => ("󰈬 ", Color::Rgb(45, 91, 183)),
            "xls" | "xlsx" => ("󰈛 ", Color::Rgb(33, 136, 56)),
            "ppt" | "pptx" => ("󰈧 ", Color::Rgb(210, 70, 40)),
            // Default
            _ => (" ", Color::Rgb(170, 170, 170)),
        }
    }
}

impl Plugin for FileTreePlugin {
    fn name(&self) -> &str {
        "file_tree"
    }

    fn handle_key(&mut self, key: KeyEvent, ctx: &mut PluginContext) -> bool {
        if !self.active {
            return false;
        }

        match self.input_mode {
            InputMode::Normal => self.handle_normal_key(key, ctx),
            _ => self.handle_input_key(key, ctx),
        }
    }

    fn render(&self, _area: Rect, _buf: &mut Buffer, _ctx: &PluginContext) {}

    fn render_readonly(&self, area: Rect, buf: &mut Buffer, _ctx: &PluginRenderContext) {
        self.render_tree(area, buf);
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn deactivate(&mut self) {
        self.active = false;
        self.filter_query.clear();
        self.input_mode = InputMode::Normal;
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
