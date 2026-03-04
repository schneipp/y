pub mod fuzzy_finder;
pub mod deno_runtime;
pub mod syntax_highlighter;
pub mod js_fuzzy_finder;

use crossterm::event::KeyEvent;
use ratatui::{buffer::Buffer, layout::Rect};

use crate::{Cursor, Mode, YBuffer};

/// Plugin trait for extending editor functionality
pub trait Plugin {
    /// Plugin name for identification
    fn name(&self) -> &str;

    /// Handle key events, return true if event was consumed
    fn handle_key(&mut self, key: KeyEvent, ctx: &mut PluginContext) -> bool;

    /// Render plugin UI (if active)
    fn render(&self, area: Rect, buf: &mut Buffer, ctx: &PluginContext);

    /// Plugin is currently active (has focus)
    fn is_active(&self) -> bool;

    /// Deactivate the plugin
    fn deactivate(&mut self);

    /// Downcast to concrete type (for accessing plugin-specific methods)
    fn as_any(&self) -> &dyn std::any::Any;

    /// Downcast to concrete mutable type (for accessing plugin-specific methods)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

/// Context passed to plugins for accessing editor state
pub struct PluginContext<'a> {
    pub buffer: &'a mut YBuffer,
    pub cursor: &'a mut Cursor,
    pub mode: &'a mut Mode,
    pub filename: &'a Option<String>,
    pub modified: &'a mut bool,
}

/// Manages all plugins
pub struct PluginManager {
    plugins: Vec<Box<dyn Plugin>>,
}

impl std::fmt::Debug for PluginManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginManager")
            .field("plugins_count", &self.plugins.len())
            .finish()
    }
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// Register a plugin
    pub fn register(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.push(plugin);
    }

    /// Distribute key event to active plugins, return true if consumed
    pub fn handle_key(&mut self, key: KeyEvent, ctx: &mut PluginContext) -> bool {
        for plugin in &mut self.plugins {
            if plugin.is_active() {
                if plugin.handle_key(key, ctx) {
                    // Plugin consumed the event
                    return true;
                }
                // Plugin didn't consume, continue to next active plugin
            }
        }
        false
    }

    /// Render all active plugins
    pub fn render(&self, area: Rect, buf: &mut Buffer, ctx: &PluginContext) {
        for plugin in &self.plugins {
            if plugin.is_active() {
                plugin.render(area, buf, ctx);
            }
        }
    }

    /// Activate a plugin by name
    pub fn activate(&mut self, name: &str) -> Result<(), String> {
        // Deactivate all plugins first
        for plugin in &mut self.plugins {
            plugin.deactivate();
        }

        // Find and activate the requested plugin
        for plugin in &mut self.plugins {
            if plugin.name() == name {
                // Plugin will activate itself when it starts handling keys
                return Ok(());
            }
        }

        Err(format!("Plugin '{}' not found", name))
    }

    /// Get reference to plugin by name
    pub fn get(&self, name: &str) -> Option<&Box<dyn Plugin>> {
        self.plugins.iter().find(|p| p.name() == name)
    }

    /// Get mutable reference to plugin by name
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Box<dyn Plugin>> {
        self.plugins.iter_mut().find(|p| p.name() == name)
    }

    /// Deactivate all plugins
    pub fn deactivate_all(&mut self) {
        for plugin in &mut self.plugins {
            plugin.deactivate();
        }
    }

    /// Check if any plugin is active
    pub fn has_active_plugin(&self) -> bool {
        self.plugins.iter().any(|p| p.is_active())
    }
}
