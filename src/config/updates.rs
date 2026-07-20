use super::AppConfig;
use super::metrics::cell_metrics_for_selection;
use super::sanitize::*;
use super::types::{BellMode, CursorShape, RightClickAction, TabBarPosition};

#[derive(Debug, Default, Clone)]
pub struct AppConfigUpdates {
    pub window_width: Option<f32>,
    pub window_height: Option<f32>,
    /// `None` = no change; `Some("auto"|"")` = clear; `Some("<tag>")` = set.
    pub language: Option<String>,
    pub animations_enabled: Option<bool>,
    pub tab_bar_position: Option<TabBarPosition>,
    pub terminal_font_selection: Option<String>,
    pub terminal_font_size: Option<f32>,
    pub terminal_padding_x: Option<f32>,
    pub terminal_padding_y: Option<f32>,
    pub color_scheme: Option<String>,
    pub foreground: Option<[u8; 3]>,
    pub background: Option<[u8; 3]>,
    pub cursor: Option<[u8; 3]>,
    pub ansi_colors: Option<[[u8; 3]; 16]>,
    pub background_opacity: Option<f32>,
    pub blur_enabled: Option<bool>,
    pub macos_blur_radius: Option<i32>,
    pub shortcut_new_tab: Option<String>,
    pub shortcut_close_tab: Option<String>,
    pub shortcut_open_settings: Option<String>,
    pub shortcut_next_tab: Option<String>,
    pub shortcut_prev_tab: Option<String>,
    pub shortcut_quit: Option<String>,
    pub shortcut_font_size_increase: Option<String>,
    pub shortcut_font_size_decrease: Option<String>,
    pub shortcut_font_size_reset: Option<String>,
    pub shortcut_duplicate_tab: Option<String>,
    pub terminal_scrollback: Option<usize>,
    pub terminal_bracketed_paste: Option<bool>,
    pub terminal_multiline_paste_confirm: Option<bool>,
    pub terminal_scroll_multiplier: Option<f32>,
    pub terminal_cursor_shape: Option<CursorShape>,
    pub terminal_cursor_blink: Option<bool>,
    pub terminal_bell_mode: Option<BellMode>,
    pub terminal_right_click_action: Option<RightClickAction>,
}

impl AppConfig {
    pub fn apply_updates(&mut self, updates: AppConfigUpdates) {
        if let Some(width) = updates.window_width {
            self.ui.window_width = sanitize_positive(width, self.ui.window_width);
        }
        if let Some(height) = updates.window_height {
            self.ui.window_height = sanitize_positive(height, self.ui.window_height);
        }
        if let Some(lang) = updates.language.as_deref() {
            self.ui.language = sanitize_language(lang);
        }
        if let Some(enabled) = updates.animations_enabled {
            self.ui.animations_enabled = enabled;
        }
        if let Some(position) = updates.tab_bar_position {
            self.ui.tab_bar_position = position;
        }
        let old_font = self.terminal.font_selection.clone();
        if let Some(selection) = updates.terminal_font_selection {
            self.terminal.font_selection = sanitize_terminal_font_selection(&selection);
        }
        if let Some(size) = updates.terminal_font_size {
            self.terminal.font_size = sanitize_terminal_font_size(size, self.terminal.font_size);
        }
        // Recalculate cell metrics if font or size changed
        if self.terminal.font_selection != old_font || updates.terminal_font_size.is_some() {
            let (cw, ch) = cell_metrics_for_selection(
                self.terminal.font_selection.as_deref(),
                self.terminal.font_size,
            );
            self.terminal.cell_width = cw;
            self.terminal.cell_height = ch;
        }
        if let Some(px) = updates.terminal_padding_x {
            self.terminal.padding_x = sanitize_padding(px);
        }
        if let Some(py) = updates.terminal_padding_y {
            self.terminal.padding_y = sanitize_padding(py);
        }
        if let Some(lines) = updates.terminal_scrollback {
            self.terminal.scrollback_lines =
                sanitize_scrollback(lines, self.terminal.scrollback_lines);
        }
        if let Some(enabled) = updates.terminal_bracketed_paste {
            self.terminal.bracketed_paste = enabled;
        }
        if let Some(enabled) = updates.terminal_multiline_paste_confirm {
            self.terminal.multiline_paste_confirm = enabled;
        }
        if let Some(mult) = updates.terminal_scroll_multiplier {
            self.terminal.scroll_multiplier =
                sanitize_scroll_multiplier(mult, self.terminal.scroll_multiplier);
        }
        if let Some(shape) = updates.terminal_cursor_shape {
            self.terminal.cursor_shape = shape;
        }
        if let Some(enabled) = updates.terminal_cursor_blink {
            self.terminal.cursor_blink = enabled;
        }
        if let Some(mode) = updates.terminal_bell_mode {
            self.terminal.bell_mode = mode;
        }
        if let Some(action) = updates.terminal_right_click_action {
            self.terminal.right_click_action = action;
        }
        if let Some(scheme) = updates.color_scheme {
            self.theme.color_scheme = scheme;
        }
        if let Some(foreground) = updates.foreground {
            self.theme.foreground = foreground;
        }
        if let Some(background) = updates.background {
            self.theme.background = background;
        }
        if let Some(cursor) = updates.cursor {
            self.theme.cursor = cursor;
        }
        if let Some(ansi) = updates.ansi_colors {
            self.theme.ansi_colors = Some(ansi);
        }
        if let Some(opacity) = updates.background_opacity {
            self.theme.background_opacity =
                sanitize_opacity(opacity, self.theme.background_opacity);
        }
        if let Some(enabled) = updates.blur_enabled {
            self.theme.blur_enabled = enabled;
        }
        if let Some(radius) = updates.macos_blur_radius {
            self.theme.macos_blur_radius = radius.clamp(0, 100);
        }

        if let Some(value) = updates.shortcut_new_tab {
            self.shortcuts.new_tab = sanitize_shortcut(&value, &self.shortcuts.new_tab);
        }
        if let Some(value) = updates.shortcut_close_tab {
            self.shortcuts.close_tab = sanitize_shortcut(&value, &self.shortcuts.close_tab);
        }
        if let Some(value) = updates.shortcut_open_settings {
            self.shortcuts.open_settings = sanitize_shortcut(&value, &self.shortcuts.open_settings);
        }
        if let Some(value) = updates.shortcut_next_tab {
            self.shortcuts.next_tab = sanitize_shortcut(&value, &self.shortcuts.next_tab);
        }
        if let Some(value) = updates.shortcut_prev_tab {
            self.shortcuts.prev_tab = sanitize_shortcut(&value, &self.shortcuts.prev_tab);
        }
        if let Some(value) = updates.shortcut_quit {
            self.shortcuts.quit = sanitize_shortcut(&value, &self.shortcuts.quit);
        }
        if let Some(value) = updates.shortcut_font_size_increase {
            self.shortcuts.font_size_increase =
                sanitize_shortcut(&value, &self.shortcuts.font_size_increase);
        }
        if let Some(value) = updates.shortcut_font_size_decrease {
            self.shortcuts.font_size_decrease =
                sanitize_shortcut(&value, &self.shortcuts.font_size_decrease);
        }
        if let Some(value) = updates.shortcut_font_size_reset {
            self.shortcuts.font_size_reset =
                sanitize_shortcut(&value, &self.shortcuts.font_size_reset);
        }
        if let Some(value) = updates.shortcut_duplicate_tab {
            self.shortcuts.duplicate_tab = sanitize_shortcut(&value, &self.shortcuts.duplicate_tab);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DEFAULT_TERMINAL_SCROLLBACK;

    #[test]
    fn apply_updates_sanitizes_invalid_values() {
        let mut config = AppConfig::default();
        let original = config.clone();

        config.apply_updates(AppConfigUpdates {
            window_width: Some(-1.0),
            window_height: Some(f32::NAN),
            background_opacity: Some(1.5),
            macos_blur_radius: Some(200),
            shortcut_new_tab: Some("Ctrl+".to_string()),
            shortcut_close_tab: Some("Ctrl+W".to_string()),
            ..Default::default()
        });

        assert_eq!(config.ui.window_width, original.ui.window_width);
        assert_eq!(config.ui.window_height, original.ui.window_height);
        assert_eq!(
            config.theme.background_opacity,
            original.theme.background_opacity
        );
        assert_eq!(config.theme.macos_blur_radius, 100); // clamped from 200
        assert_eq!(config.shortcuts.new_tab, original.shortcuts.new_tab);
        assert_eq!(config.shortcuts.close_tab, "Ctrl+W");
    }

    #[test]
    fn apply_updates_terminal_font_selection_and_size_are_sanitized() {
        let mut config = AppConfig::default();
        let default_size = config.terminal.font_size;

        config.apply_updates(AppConfigUpdates {
            terminal_font_selection: Some(" Fira Code ".to_string()),
            terminal_font_size: Some(18.5),
            ..Default::default()
        });
        assert_eq!(
            config.terminal.font_selection,
            Some("Fira Code".to_string())
        );
        assert_eq!(config.terminal.font_size, 18.5);

        config.apply_updates(AppConfigUpdates {
            terminal_font_selection: Some("   ".to_string()),
            terminal_font_size: Some(4.0),
            ..Default::default()
        });
        assert_eq!(config.terminal.font_selection, None);
        assert_eq!(config.terminal.font_size, 18.5);

        config.apply_updates(AppConfigUpdates {
            terminal_font_size: Some(f32::NAN),
            ..Default::default()
        });
        assert_eq!(config.terminal.font_size, 18.5);

        config.apply_updates(AppConfigUpdates {
            terminal_font_size: Some(default_size),
            ..Default::default()
        });
        assert_eq!(config.terminal.font_size, default_size);
    }

    #[test]
    fn scrollback_sanitize_clamps_to_valid_range() {
        let mut config = AppConfig::default();
        assert_eq!(
            config.terminal.scrollback_lines,
            DEFAULT_TERMINAL_SCROLLBACK
        );

        // Value within range is accepted
        config.apply_updates(AppConfigUpdates {
            terminal_scrollback: Some(500),
            ..Default::default()
        });
        assert_eq!(config.terminal.scrollback_lines, 500);

        // Value below minimum keeps previous
        config.apply_updates(AppConfigUpdates {
            terminal_scrollback: Some(99),
            ..Default::default()
        });
        assert_eq!(config.terminal.scrollback_lines, 500);

        // Value above maximum keeps previous
        config.apply_updates(AppConfigUpdates {
            terminal_scrollback: Some(1_000_001),
            ..Default::default()
        });
        assert_eq!(config.terminal.scrollback_lines, 500);

        // Boundary values are accepted
        config.apply_updates(AppConfigUpdates {
            terminal_scrollback: Some(100),
            ..Default::default()
        });
        assert_eq!(config.terminal.scrollback_lines, 100);

        config.apply_updates(AppConfigUpdates {
            terminal_scrollback: Some(1_000_000),
            ..Default::default()
        });
        assert_eq!(config.terminal.scrollback_lines, 1_000_000);
    }
}
