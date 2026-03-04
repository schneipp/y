use crossterm::event::KeyEvent;
use ratatui::{buffer::Buffer, layout::Rect, style::Color};
use tree_sitter::{Parser, Query, QueryCursor, Tree};
use tree_sitter_rust;

use crate::plugins::{Plugin, PluginContext};

pub struct SyntaxHighlighter {
    parser: Parser,
    tree: Option<Tree>,
    highlight_query: Query,
    // Cache of line highlights: Vec<(start_col, end_col, Color)>
    line_highlights: Vec<Vec<(usize, usize, Color)>>,
}

impl SyntaxHighlighter {
    pub fn new() -> Self {
        let mut parser = Parser::new();
        parser
            .set_language(tree_sitter_rust::language())
            .expect("Error loading Rust grammar");

        // Define highlight query for Rust syntax
        let highlight_query_source = r#"
            (function_item name: (identifier) @function)
            (call_expression function: (identifier) @function.call)
            (type_identifier) @type
            (primitive_type) @type.builtin
            (string_literal) @string
            (integer_literal) @number
            (float_literal) @number
            (boolean_literal) @constant.builtin
            (line_comment) @comment
            (block_comment) @comment
            "fn" @keyword
            "let" @keyword
            "const" @keyword
            "static" @keyword
            "if" @keyword
            "else" @keyword
            "match" @keyword
            "for" @keyword
            "while" @keyword
            "loop" @keyword
            "return" @keyword
            "break" @keyword
            "continue" @keyword
            "pub" @keyword
            "use" @keyword
            "mod" @keyword
            "struct" @keyword
            "enum" @keyword
            "impl" @keyword
            "trait" @keyword
            "where" @keyword
            "as" @keyword
            "in" @keyword
            (mutable_specifier) @keyword
            (self) @variable.builtin
            (macro_invocation macro: (identifier) @function.macro)
        "#;

        let highlight_query = Query::new(tree_sitter_rust::language(), highlight_query_source)
            .expect("Error creating highlight query");

        Self {
            parser,
            tree: None,
            highlight_query,
            line_highlights: Vec::new(),
        }
    }

    /// Parse the buffer and update syntax tree
    pub fn parse_buffer(&mut self, buffer: &crate::YBuffer) {
        let source_code: String = buffer
            .lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        self.tree = self.parser.parse(&source_code, None);

        // Compute highlights for all lines
        self.compute_highlights(&source_code);
    }

    fn compute_highlights(&mut self, source_code: &str) {
        self.line_highlights.clear();

        // Count lines
        let line_count = source_code.lines().count().max(1);
        self.line_highlights.resize(line_count, Vec::new());

        if let Some(tree) = &self.tree {
            let mut query_cursor = QueryCursor::new();
            let matches = query_cursor.matches(
                &self.highlight_query,
                tree.root_node(),
                source_code.as_bytes(),
            );

            for match_ in matches {
                for capture in match_.captures {
                    let capture_name = &self.highlight_query.capture_names()[capture.index as usize];
                    let color = self.color_for_capture(capture_name);

                    let start_pos = capture.node.start_position();
                    let end_pos = capture.node.end_position();

                    // Add highlight for each line this capture spans
                    for line_idx in start_pos.row..=end_pos.row {
                        if line_idx < self.line_highlights.len() {
                            let start_col = if line_idx == start_pos.row {
                                start_pos.column
                            } else {
                                0
                            };
                            let end_col = if line_idx == end_pos.row {
                                end_pos.column
                            } else {
                                usize::MAX // Rest of line
                            };

                            self.line_highlights[line_idx].push((start_col, end_col, color));
                        }
                    }
                }
            }

            // Sort highlights by start column for each line
            for line_highlights in &mut self.line_highlights {
                line_highlights.sort_by_key(|(start, _, _)| *start);
            }
        }
    }

    fn color_for_capture(&self, capture_name: &str) -> Color {
        match capture_name {
            "function" | "function.call" => Color::Yellow,
            "function.macro" => Color::Cyan,
            "type" | "type.builtin" => Color::Blue,
            "string" => Color::Green,
            "number" => Color::Magenta,
            "comment" => Color::DarkGray,
            "keyword" | "keyword.operator" => Color::Red,
            "constant.builtin" => Color::Magenta,
            "variable.builtin" => Color::Cyan,
            _ => Color::Reset,
        }
    }

    /// Get highlights for a specific line
    pub fn get_line_highlights(&self, line_idx: usize) -> &[(usize, usize, Color)] {
        if line_idx < self.line_highlights.len() {
            &self.line_highlights[line_idx]
        } else {
            &[]
        }
    }
}

impl Plugin for SyntaxHighlighter {
    fn name(&self) -> &str {
        "syntax_highlighter"
    }

    fn handle_key(&mut self, _key: KeyEvent, ctx: &mut PluginContext) -> bool {
        // Reparse buffer when it changes
        // Note: In a real implementation, we'd want to track if the buffer actually changed
        // For now, we'll reparse on every key press (could be optimized)
        self.parse_buffer(ctx.buffer);
        false // Don't consume the event
    }

    fn render(&self, _area: Rect, _buf: &mut Buffer, _ctx: &PluginContext) {
        // Syntax highlighting is passive - it doesn't render UI
        // The rendering logic will query this plugin for highlights
    }

    fn is_active(&self) -> bool {
        true // Always active
    }

    fn deactivate(&mut self) {
        // Syntax highlighter stays active
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
