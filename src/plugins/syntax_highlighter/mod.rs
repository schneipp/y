use std::collections::HashMap;

use crossterm::event::KeyEvent;
use ratatui::{buffer::Buffer, layout::Rect, style::Color};
use tree_sitter::{Parser, Query, QueryCursor, Tree};
use tree_sitter_rust;

use crate::plugins::{Plugin, PluginContext};
use crate::buffer::{BufferId, YBuffer};
use crate::theme::SyntaxColors;

/// Per-buffer cached parse state
struct BufferCache {
    tree: Option<Tree>,
    line_highlights: Vec<Vec<(usize, usize, Color)>>,
}

pub struct SyntaxHighlighter {
    parser: Parser,
    highlight_query: Query,
    /// Per-buffer parse trees and highlight caches
    caches: HashMap<BufferId, BufferCache>,
    /// Current syntax colors from theme
    syntax_colors: SyntaxColors,
}

impl SyntaxHighlighter {
    pub fn new() -> Self {
        let mut parser = Parser::new();
        parser
            .set_language(tree_sitter_rust::language())
            .expect("Error loading Rust grammar");

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

        // Default colors — will be overwritten by theme on init
        let syntax_colors = SyntaxColors {
            keyword: Color::Red,
            function: Color::Yellow,
            function_macro: Color::Cyan,
            type_: Color::Blue,
            type_builtin: Color::Blue,
            string: Color::Green,
            number: Color::Magenta,
            comment: Color::DarkGray,
            constant_builtin: Color::Magenta,
            variable_builtin: Color::Cyan,
            operator: Color::Red,
            default: Color::Reset,
        };

        Self {
            parser,
            highlight_query,
            caches: HashMap::new(),
            syntax_colors,
        }
    }

    /// Update syntax colors from theme; invalidates all caches
    pub fn set_syntax_colors(&mut self, colors: SyntaxColors) {
        self.syntax_colors = colors;
        self.caches.clear();
    }

    /// Parse the buffer and update syntax tree
    pub fn parse_buffer(&mut self, buffer: &YBuffer, buffer_id: BufferId) {
        let source_code: String = buffer
            .lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        let cache = self.caches.entry(buffer_id).or_insert_with(|| BufferCache {
            tree: None,
            line_highlights: Vec::new(),
        });

        // Full reparse every time — incremental parsing requires Tree::edit()
        cache.tree = self.parser.parse(&source_code, None);

        Self::compute_highlights(
            &self.highlight_query,
            &self.syntax_colors,
            &mut cache.line_highlights,
            &source_code,
            &cache.tree,
        );
    }

    fn compute_highlights(
        query: &Query,
        colors: &SyntaxColors,
        line_highlights: &mut Vec<Vec<(usize, usize, Color)>>,
        source_code: &str,
        tree: &Option<Tree>,
    ) {
        line_highlights.clear();

        let line_count = source_code.lines().count().max(1);
        line_highlights.resize(line_count, Vec::new());

        if let Some(tree) = tree {
            let mut query_cursor = QueryCursor::new();
            let matches = query_cursor.matches(
                query,
                tree.root_node(),
                source_code.as_bytes(),
            );

            for match_ in matches {
                for capture in match_.captures {
                    let capture_name = &query.capture_names()[capture.index as usize];
                    let color = Self::color_for_capture(colors, capture_name);

                    let start_pos = capture.node.start_position();
                    let end_pos = capture.node.end_position();

                    for line_idx in start_pos.row..=end_pos.row {
                        if line_idx < line_highlights.len() {
                            let start_col = if line_idx == start_pos.row {
                                start_pos.column
                            } else {
                                0
                            };
                            let end_col = if line_idx == end_pos.row {
                                end_pos.column
                            } else {
                                usize::MAX
                            };

                            line_highlights[line_idx].push((start_col, end_col, color));
                        }
                    }
                }
            }

            for lh in line_highlights.iter_mut() {
                lh.sort_by_key(|(start, _, _)| *start);
            }
        }
    }

    fn color_for_capture(colors: &SyntaxColors, capture_name: &str) -> Color {
        match capture_name {
            "function" | "function.call" => colors.function,
            "function.macro" => colors.function_macro,
            "type" => colors.type_,
            "type.builtin" => colors.type_builtin,
            "string" => colors.string,
            "number" => colors.number,
            "comment" => colors.comment,
            "keyword" | "keyword.operator" => colors.keyword,
            "constant.builtin" => colors.constant_builtin,
            "variable.builtin" => colors.variable_builtin,
            _ => colors.default,
        }
    }

    /// Get highlights for a specific line of a specific buffer
    pub fn get_line_highlights(&self, buffer_id: BufferId, line_idx: usize) -> &[(usize, usize, Color)] {
        if let Some(cache) = self.caches.get(&buffer_id) {
            if line_idx < cache.line_highlights.len() {
                return &cache.line_highlights[line_idx];
            }
        }
        &[]
    }
}

impl Plugin for SyntaxHighlighter {
    fn name(&self) -> &str {
        "syntax_highlighter"
    }

    fn handle_key(&mut self, _key: KeyEvent, ctx: &mut PluginContext) -> bool {
        self.parse_buffer(ctx.buffer, ctx.buffer_id);
        false
    }

    fn render(&self, _area: Rect, _buf: &mut Buffer, _ctx: &PluginContext) {}

    fn is_active(&self) -> bool {
        true
    }

    fn deactivate(&mut self) {}

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
