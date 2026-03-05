use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use crate::plugins::{Plugin, PluginContext, PluginRenderContext};

#[derive(Debug, Clone, PartialEq)]
enum GitView {
    Status,
    Log,
    CommitInput,
}

#[derive(Debug, Clone)]
struct FileEntry {
    status: String,
    path: String,
    staged: bool,
}

pub struct GitClientPlugin {
    active: bool,
    view: GitView,
    entries: Vec<FileEntry>,
    selected: usize,
    scroll_offset: usize,
    log_lines: Vec<String>,
    commit_message: String,
    branch_name: String,
    error_message: Option<String>,
    success_message: Option<String>,
    popup_colors: PopupColors,
}

struct PopupColors {
    border: Color,
    bg: Color,
    fg: Color,
    selected_bg: Color,
    accent: Color,
    dim: Color,
}

impl Default for PopupColors {
    fn default() -> Self {
        Self {
            border: Color::Cyan,
            bg: Color::Black,
            fg: Color::White,
            selected_bg: Color::DarkGray,
            accent: Color::Green,
            dim: Color::DarkGray,
        }
    }
}

impl GitClientPlugin {
    pub fn new() -> Self {
        Self {
            active: false,
            view: GitView::Status,
            entries: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            log_lines: Vec::new(),
            commit_message: String::new(),
            branch_name: String::new(),
            error_message: None,
            success_message: None,
            popup_colors: PopupColors::default(),
        }
    }

    pub fn set_colors(&mut self, border: Color, bg: Color, fg: Color, selected_bg: Color, accent: Color, dim: Color) {
        self.popup_colors = PopupColors { border, bg, fg, selected_bg, accent, dim };
    }

    pub fn activate_git(&mut self) {
        self.active = true;
        self.view = GitView::Status;
        self.selected = 0;
        self.scroll_offset = 0;
        self.error_message = None;
        self.success_message = None;
        self.commit_message.clear();
        self.refresh_status();
        self.refresh_branch();
    }

    fn refresh_status(&mut self) {
        self.entries.clear();

        let output = std::process::Command::new("git")
            .args(["status", "--porcelain=v1"])
            .output();

        match output {
            Ok(out) => {
                let text = String::from_utf8_lossy(&out.stdout);
                for line in text.lines() {
                    if line.len() < 4 {
                        continue;
                    }
                    let index_status = line.chars().nth(0).unwrap_or(' ');
                    let worktree_status = line.chars().nth(1).unwrap_or(' ');
                    let path = line[3..].to_string();

                    // Staged changes (index has a status)
                    if index_status != ' ' && index_status != '?' {
                        self.entries.push(FileEntry {
                            status: format!("{}", index_status),
                            path: path.clone(),
                            staged: true,
                        });
                    }

                    // Unstaged changes (worktree has a status)
                    if worktree_status != ' ' {
                        let status = if index_status == '?' { "?" } else { &format!("{}", worktree_status) };
                        self.entries.push(FileEntry {
                            status: status.to_string(),
                            path,
                            staged: false,
                        });
                    }
                }
            }
            Err(_) => {
                self.error_message = Some("Failed to run git status".to_string());
            }
        }
    }

    fn refresh_branch(&mut self) {
        if let Ok(out) = std::process::Command::new("git")
            .args(["branch", "--show-current"])
            .output()
        {
            self.branch_name = String::from_utf8_lossy(&out.stdout).trim().to_string();
        }
    }

    fn refresh_log(&mut self) {
        self.log_lines.clear();
        if let Ok(out) = std::process::Command::new("git")
            .args(["log", "--oneline", "-30"])
            .output()
        {
            let text = String::from_utf8_lossy(&out.stdout);
            self.log_lines = text.lines().map(|l| l.to_string()).collect();
        }
    }

    fn stage_selected(&mut self) {
        if let Some(entry) = self.entries.get(self.selected) {
            if !entry.staged {
                let path = entry.path.clone();
                let _ = std::process::Command::new("git")
                    .args(["add", &path])
                    .output();
                self.refresh_status();
                self.selected = self.selected.min(self.entries.len().saturating_sub(1));
            }
        }
    }

    fn unstage_selected(&mut self) {
        if let Some(entry) = self.entries.get(self.selected) {
            if entry.staged {
                let path = entry.path.clone();
                let _ = std::process::Command::new("git")
                    .args(["restore", "--staged", &path])
                    .output();
                self.refresh_status();
                self.selected = self.selected.min(self.entries.len().saturating_sub(1));
            }
        }
    }

    fn stage_all(&mut self) {
        let _ = std::process::Command::new("git")
            .args(["add", "-A"])
            .output();
        self.refresh_status();
    }

    fn unstage_all(&mut self) {
        let _ = std::process::Command::new("git")
            .args(["reset", "HEAD"])
            .output();
        self.refresh_status();
    }

    fn do_commit(&mut self) {
        if self.commit_message.trim().is_empty() {
            self.error_message = Some("Commit message cannot be empty".to_string());
            self.view = GitView::Status;
            return;
        }

        let msg = self.commit_message.clone();
        let result = std::process::Command::new("git")
            .args(["commit", "-m", &msg])
            .output();

        match result {
            Ok(out) => {
                if out.status.success() {
                    self.commit_message.clear();
                    self.error_message = None;
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let detail = stdout.lines().next().unwrap_or("done");
                    self.success_message = Some(format!("Committed: {}", detail.trim()));
                } else {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    self.error_message = Some(format!("Commit failed: {}", stderr.lines().next().unwrap_or("")));
                }
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to run git commit: {}", e));
            }
        }
        self.view = GitView::Status;
        self.refresh_status();
    }

    fn push(&mut self) {
        self.success_message = None;
        self.error_message = None;
        let result = std::process::Command::new("git")
            .args(["push"])
            .output();

        match result {
            Ok(out) => {
                if !out.status.success() {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    self.error_message = Some(format!("Push failed: {}", stderr.lines().next().unwrap_or("")));
                } else {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    let detail = stderr.lines()
                        .find(|l| l.contains("->"))
                        .unwrap_or("up to date");
                    self.success_message = Some(format!("Pushed: {}", detail.trim()));
                }
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to run git push: {}", e));
            }
        }
    }

    fn pull(&mut self) {
        self.success_message = None;
        self.error_message = None;
        let result = std::process::Command::new("git")
            .args(["pull"])
            .output();

        match result {
            Ok(out) => {
                if !out.status.success() {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    self.error_message = Some(format!("Pull failed: {}", stderr.lines().next().unwrap_or("")));
                } else {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let detail = stdout.lines().next().unwrap_or("done");
                    self.success_message = Some(format!("Pulled: {}", detail.trim()));
                }
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to run git pull: {}", e));
            }
        }
    }

    fn render_status(&self, inner: Rect, buf: &mut Buffer) {
        let colors = &self.popup_colors;
        let mut lines: Vec<Line> = Vec::new();

        // Header
        lines.push(Line::from(vec![
            Span::styled("  Branch: ", Style::default().fg(colors.dim)),
            Span::styled(&self.branch_name, Style::default().fg(colors.accent).add_modifier(Modifier::BOLD)),
        ]));
        lines.push(Line::from(""));

        if let Some(ref msg) = self.success_message {
            lines.push(Line::from(Span::styled(
                format!("  ✓ {}", msg),
                Style::default().fg(Color::Green),
            )));
            lines.push(Line::from(""));
        }

        if let Some(ref err) = self.error_message {
            lines.push(Line::from(Span::styled(
                format!("  ✗ {}", err),
                Style::default().fg(Color::Red),
            )));
            lines.push(Line::from(""));
        }

        if self.entries.is_empty() {
            lines.push(Line::from(Span::styled(
                "  Working tree clean",
                Style::default().fg(colors.dim),
            )));
        } else {
            // Staged section
            let staged: Vec<_> = self.entries.iter().enumerate().filter(|(_, e)| e.staged).collect();
            if !staged.is_empty() {
                lines.push(Line::from(Span::styled(
                    "  Staged changes:",
                    Style::default().fg(colors.accent).add_modifier(Modifier::BOLD),
                )));
                for (idx, entry) in &staged {
                    let marker = if *idx == self.selected { "▸ " } else { "  " };
                    let style = if *idx == self.selected {
                        Style::default().bg(colors.selected_bg).fg(colors.fg)
                    } else {
                        Style::default().fg(colors.fg)
                    };
                    lines.push(Line::from(Span::styled(
                        format!("  {} {} {}", marker, entry.status, entry.path),
                        style,
                    )));
                }
                lines.push(Line::from(""));
            }

            // Unstaged section
            let unstaged: Vec<_> = self.entries.iter().enumerate().filter(|(_, e)| !e.staged).collect();
            if !unstaged.is_empty() {
                lines.push(Line::from(Span::styled(
                    "  Unstaged changes:",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )));
                for (idx, entry) in &unstaged {
                    let marker = if *idx == self.selected { "▸ " } else { "  " };
                    let style = if *idx == self.selected {
                        Style::default().bg(colors.selected_bg).fg(colors.fg)
                    } else {
                        Style::default().fg(colors.fg)
                    };
                    let status_char = &entry.status;
                    lines.push(Line::from(Span::styled(
                        format!("  {} {} {}", marker, status_char, entry.path),
                        style,
                    )));
                }
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("  s", Style::default().fg(colors.accent)),
            Span::styled(" stage  ", Style::default().fg(colors.dim)),
            Span::styled("u", Style::default().fg(colors.accent)),
            Span::styled(" unstage  ", Style::default().fg(colors.dim)),
            Span::styled("S", Style::default().fg(colors.accent)),
            Span::styled(" stage all  ", Style::default().fg(colors.dim)),
            Span::styled("U", Style::default().fg(colors.accent)),
            Span::styled(" unstage all", Style::default().fg(colors.dim)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  c", Style::default().fg(colors.accent)),
            Span::styled(" commit  ", Style::default().fg(colors.dim)),
            Span::styled("p", Style::default().fg(colors.accent)),
            Span::styled(" push  ", Style::default().fg(colors.dim)),
            Span::styled("f", Style::default().fg(colors.accent)),
            Span::styled(" pull  ", Style::default().fg(colors.dim)),
            Span::styled("l", Style::default().fg(colors.accent)),
            Span::styled(" log  ", Style::default().fg(colors.dim)),
            Span::styled("r", Style::default().fg(colors.accent)),
            Span::styled(" refresh  ", Style::default().fg(colors.dim)),
            Span::styled("q/Esc", Style::default().fg(colors.accent)),
            Span::styled(" close", Style::default().fg(colors.dim)),
        ]));

        let text = Text::from(lines);
        Paragraph::new(text).render(inner, buf);
    }

    fn render_log(&self, inner: Rect, buf: &mut Buffer) {
        let colors = &self.popup_colors;
        let mut lines: Vec<Line> = Vec::new();

        lines.push(Line::from(Span::styled(
            "  Git Log (recent commits)",
            Style::default().fg(colors.accent).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));

        let visible_height = inner.height.saturating_sub(5) as usize;
        for (i, log_line) in self.log_lines.iter().enumerate().skip(self.scroll_offset).take(visible_height) {
            let style = if i == self.selected {
                Style::default().bg(colors.selected_bg).fg(colors.fg)
            } else {
                Style::default().fg(colors.fg)
            };
            // Split hash from message
            let parts: Vec<&str> = log_line.splitn(2, ' ').collect();
            if parts.len() == 2 {
                lines.push(Line::from(vec![
                    Span::styled(format!("  {} ", parts[0]), Style::default().fg(colors.accent)),
                    Span::styled(parts[1], style),
                ]));
            } else {
                lines.push(Line::from(Span::styled(format!("  {}", log_line), style)));
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("  s", Style::default().fg(colors.accent)),
            Span::styled(" status  ", Style::default().fg(colors.dim)),
            Span::styled("q/Esc", Style::default().fg(colors.accent)),
            Span::styled(" close", Style::default().fg(colors.dim)),
        ]));

        let text = Text::from(lines);
        Paragraph::new(text).render(inner, buf);
    }

    fn render_commit_input(&self, inner: Rect, buf: &mut Buffer) {
        let colors = &self.popup_colors;
        let mut lines: Vec<Line> = Vec::new();

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Commit message:",
            Style::default().fg(colors.accent).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("  > {}_", self.commit_message),
            Style::default().fg(colors.fg),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("  Enter", Style::default().fg(colors.accent)),
            Span::styled(" confirm  ", Style::default().fg(colors.dim)),
            Span::styled("Esc", Style::default().fg(colors.accent)),
            Span::styled(" cancel", Style::default().fg(colors.dim)),
        ]));

        let text = Text::from(lines);
        Paragraph::new(text).render(inner, buf);
    }
}

impl Plugin for GitClientPlugin {
    fn name(&self) -> &str {
        "git_client"
    }

    fn handle_key(&mut self, key: KeyEvent, ctx: &mut PluginContext) -> bool {
        if !self.active {
            return false;
        }

        match self.view {
            GitView::CommitInput => {
                match key.code {
                    KeyCode::Esc => {
                        self.commit_message.clear();
                        self.view = GitView::Status;
                    }
                    KeyCode::Enter => {
                        self.do_commit();
                    }
                    KeyCode::Backspace => {
                        self.commit_message.pop();
                    }
                    KeyCode::Char(c) => {
                        self.commit_message.push(c);
                    }
                    _ => {}
                }
                return true;
            }
            GitView::Log => {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => {
                        self.active = false;
                        *ctx.mode = ctx.default_mode.clone();
                    }
                    KeyCode::Char('s') => {
                        self.view = GitView::Status;
                        self.selected = 0;
                        self.refresh_status();
                    }
                    KeyCode::Char('j') | KeyCode::Down => {
                        if self.selected + 1 < self.log_lines.len() {
                            self.selected += 1;
                            if self.selected >= self.scroll_offset + (20) {
                                self.scroll_offset += 1;
                            }
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        if self.selected > 0 {
                            self.selected -= 1;
                            if self.selected < self.scroll_offset {
                                self.scroll_offset = self.selected;
                            }
                        }
                    }
                    _ => {}
                }
                return true;
            }
            GitView::Status => {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => {
                        self.active = false;
                        *ctx.mode = ctx.default_mode.clone();
                    }
                    KeyCode::Char('j') | KeyCode::Down => {
                        if !self.entries.is_empty() && self.selected + 1 < self.entries.len() {
                            self.selected += 1;
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        if self.selected > 0 {
                            self.selected -= 1;
                        }
                    }
                    KeyCode::Char('s') => self.stage_selected(),
                    KeyCode::Char('u') => self.unstage_selected(),
                    KeyCode::Char('S') => self.stage_all(),
                    KeyCode::Char('U') => self.unstage_all(),
                    KeyCode::Char('c') => {
                        // Check if there are staged changes
                        if self.entries.iter().any(|e| e.staged) {
                            self.view = GitView::CommitInput;
                            self.commit_message.clear();
                        } else {
                            self.error_message = Some("Nothing staged to commit".to_string());
                        }
                    }
                    KeyCode::Char('p') => self.push(),
                    KeyCode::Char('f') => self.pull(),
                    KeyCode::Char('l') => {
                        self.refresh_log();
                        self.view = GitView::Log;
                        self.selected = 0;
                        self.scroll_offset = 0;
                    }
                    KeyCode::Char('r') => {
                        self.refresh_status();
                        self.refresh_branch();
                        self.error_message = None;
                        self.success_message = None;
                    }
                    _ => {}
                }
                return true;
            }
        }
    }

    fn render(&self, _area: Rect, _buf: &mut Buffer, _ctx: &PluginContext) {
        // Uses render_readonly instead
    }

    fn render_readonly(&self, area: Rect, buf: &mut Buffer, _ctx: &PluginRenderContext) {
        if !self.active {
            return;
        }

        let colors = &self.popup_colors;

        let popup_width = (area.width as f32 * 0.8) as u16;
        let popup_height = (area.height as f32 * 0.75) as u16;
        let popup_x = (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = (area.height.saturating_sub(popup_height)) / 2;

        let popup_area = Rect {
            x: popup_x,
            y: popup_y,
            width: popup_width.min(area.width),
            height: popup_height.min(area.height),
        };

        Clear.render(popup_area, buf);

        let title = match self.view {
            GitView::Status => " Git Status ",
            GitView::Log => " Git Log ",
            GitView::CommitInput => " Git Commit ",
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(colors.border))
            .style(Style::default().bg(colors.bg).fg(colors.fg));

        let inner = block.inner(popup_area);
        block.render(popup_area, buf);

        match self.view {
            GitView::Status => self.render_status(inner, buf),
            GitView::Log => self.render_log(inner, buf),
            GitView::CommitInput => self.render_commit_input(inner, buf),
        }
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn deactivate(&mut self) {
        self.active = false;
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
