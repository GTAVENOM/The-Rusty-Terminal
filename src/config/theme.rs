//! App-level theme: terminal palette plus chrome colors.
//!
//! Ships a dark theme (default) and a light theme; custom themes load
//! from a JSON file at `%APPDATA%\RustyTerminal\theme.json`.

use std::path::PathBuf;

use egui_term::{ColorPalette, TerminalTheme};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct AppTheme {
    pub name: String,
    pub tab_bar_bg: egui::Color32,
    pub accent: egui::Color32,
    /// Terminal 16-color palette. `None` = default palette.
    palette: Option<ColorPalette>,
    pub light_mode: bool,
}

impl AppTheme {
    pub fn dark() -> Self {
        Self {
            name: "dark".to_string(),
            tab_bar_bg: egui::Color32::from_rgb(0x14, 0x14, 0x18),
            accent: egui::Color32::from_rgb(0xd0, 0x7a, 0x2e), // rusty orange
            palette: None,
            light_mode: false,
        }
    }

    pub fn light() -> Self {
        Self {
            name: "light".to_string(),
            tab_bar_bg: egui::Color32::from_rgb(0xf2, 0xf2, 0xf4),
            accent: egui::Color32::from_rgb(0xb3, 0x5c, 0x14),
            palette: None,
            light_mode: true,
        }
    }

    pub fn terminal_theme(&self) -> TerminalTheme {
        TerminalTheme::new(Box::new(
            self.palette.clone().unwrap_or_default(),
        ))
    }

    pub fn apply_chrome(&self, ctx: &egui::Context) {
        let mut visuals = if self.light_mode {
            egui::Visuals::light()
        } else {
            egui::Visuals::dark()
        };
        visuals.panel_fill = self.tab_bar_bg;
        visuals.selection.bg_fill = self.accent.linear_multiply(0.4);
        ctx.set_visuals(visuals);
    }
}

/// JSON shape of a custom theme file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeFile {
    pub name: String,
    /// CSS-ish hex colors, e.g. "#d07a2e".
    pub accent: String,
    pub tab_bar_bg: String,
    /// `light` or `dark` chrome.
    pub mode: ThemeMode,
    /// Optional ANSI 16-color palette, each "#rrggbb".
    #[serde(default)]
    pub palette: Option<[String; 16]>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    Light,
    Dark,
}

fn parse_hex(s: &str) -> Option<egui::Color32> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some(egui::Color32::from_rgb(r, g, b))
}

/// Load a custom theme from `%APPDATA%\RustyTerminal\theme.json`, falling
/// back to the built-in dark theme on any error.
pub fn load_custom() -> AppTheme {
    let path = theme_path();
    match std::fs::read_to_string(&path) {
        Ok(contents) => match serde_json::from_str::<ThemeFile>(&contents) {
            Ok(file) => build_from_file(&file),
            Err(e) => {
                log::warn!("theme.json parse error, using dark: {e}");
                AppTheme::dark()
            },
        },
        Err(_) => AppTheme::dark(),
    }
}

fn theme_path() -> PathBuf {
    let mut dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    dir.push("RustyTerminal");
    dir.push("theme.json");
    dir
}

fn build_from_file(file: &ThemeFile) -> AppTheme {
    let Some(accent) = parse_hex(&file.accent) else {
        log::warn!("theme.json: invalid accent color, using dark");
        return AppTheme::dark();
    };
    let Some(tab_bar_bg) = parse_hex(&file.tab_bar_bg) else {
        log::warn!("theme.json: invalid tab_bar_bg color, using dark");
        return AppTheme::dark();
    };
    let palette = file.palette.as_ref().and_then(|p| {
        // Validate all 16 first; a single bad color drops the whole
        // custom palette (fall back to defaults) rather than rendering
        // a partially-broken scheme.
        for (i, s) in p.iter().enumerate() {
            if parse_hex(s).is_none() {
                log::warn!(
                    "theme.json: invalid palette color at index {i}, \
                     using default palette"
                );
                return None;
            }
        }
        let hex = |i: usize| p[i].clone();
        Some(ColorPalette {
            foreground: hex(7),
            background: hex(0),
            black: hex(0),
            red: hex(1),
            green: hex(2),
            yellow: hex(3),
            blue: hex(4),
            magenta: hex(5),
            cyan: hex(6),
            white: hex(7),
            bright_black: hex(8),
            bright_red: hex(9),
            bright_green: hex(10),
            bright_yellow: hex(11),
            bright_blue: hex(12),
            bright_magenta: hex(13),
            bright_cyan: hex(14),
            bright_white: hex(15),
            ..ColorPalette::default()
        })
    });
    AppTheme {
        name: file.name.clone(),
        accent,
        tab_bar_bg,
        palette,
        light_mode: file.mode == ThemeMode::Light,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_parsing() {
        assert_eq!(
            parse_hex("#d07a2e"),
            Some(egui::Color32::from_rgb(0xd0, 0x7a, 0x2e))
        );
        assert_eq!(parse_hex("d07a2e"), Some(egui::Color32::from_rgb(0xd0, 0x7a, 0x2e)));
        assert_eq!(parse_hex("nope"), None);
        assert_eq!(parse_hex("#ff00"), None);
    }

    #[test]
    fn invalid_file_falls_back_to_dark() {
        let file = ThemeFile {
            name: "bad".into(),
            accent: "zzz".into(),
            tab_bar_bg: "#000000".into(),
            mode: ThemeMode::Dark,
            palette: None,
        };
        let theme = build_from_file(&file);
        assert_eq!(theme.name, "dark");
    }

    #[test]
    fn light_file_builds() {
        let file = ThemeFile {
            name: "solarized".into(),
            accent: "#268bd2".into(),
            tab_bar_bg: "#002b36".into(),
            mode: ThemeMode::Dark,
            palette: None,
        };
        let theme = build_from_file(&file);
        assert_eq!(theme.name, "solarized");
        assert_eq!(theme.accent, egui::Color32::from_rgb(0x26, 0x8b, 0xd2));
        assert!(!theme.light_mode);
    }
}
