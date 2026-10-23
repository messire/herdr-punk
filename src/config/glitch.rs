use serde::Deserialize;

use super::sidebar::SidebarTokenColor;

/// Client-side "signal interference" animation for one right-side tab bar
/// entry. The matching entry's text is corrupted for a short burst of frames,
/// decaying back to the clean text; bursts repeat after a random idle gap.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct TabBarGlitchConfig {
    /// Animate when true. Default: false.
    pub enabled: bool,
    /// Entries whose text contains this string are animated. Empty animates
    /// nothing.
    pub matches: String,
    /// Shortest / longest pause between bursts, in milliseconds. Defaults: 3000 / 6000.
    pub idle_min_ms: u64,
    pub idle_max_ms: u64,
    /// Fewest / most frames in one burst. Defaults: 6 / 10.
    pub frames_min: u32,
    pub frames_max: u32,
    /// Time each frame stays on screen, in milliseconds. Default: 90.
    pub frame_ms: u64,
    /// Colour for heavily corrupted frames. Falls back to the entry's own style.
    pub noise_fg: Option<SidebarTokenColor>,
    /// Glyphs used as noise. Each must be a single-cell character.
    pub noise: String,
}

impl Default for TabBarGlitchConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            matches: String::new(),
            idle_min_ms: 3000,
            idle_max_ms: 6000,
            frames_min: 6,
            frames_max: 10,
            frame_ms: 90,
            noise_fg: None,
            noise: "▓▒░█▚▞╳▌▐".into(),
        }
    }
}

impl TabBarGlitchConfig {
    pub(crate) fn active(&self) -> bool {
        self.enabled && !self.matches.is_empty()
    }

    pub(crate) fn idle_range_ms(&self) -> (u64, u64) {
        let lo = self.idle_min_ms.clamp(200, 600_000);
        let hi = self.idle_max_ms.clamp(lo, 600_000);
        (lo, hi)
    }

    pub(crate) fn frames_range(&self) -> (u32, u32) {
        let lo = self.frames_min.clamp(1, 60);
        let hi = self.frames_max.clamp(lo, 60);
        (lo, hi)
    }

    pub(crate) fn frame_duration(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.frame_ms.clamp(30, 2000))
    }

    pub(crate) fn noise_glyphs(&self) -> Vec<char> {
        let glyphs: Vec<char> = self
            .noise
            .chars()
            .filter(|c| unicode_width::UnicodeWidthChar::width(*c) == Some(1))
            .collect();
        if glyphs.is_empty() {
            Self::default().noise.chars().collect()
        } else {
            glyphs
        }
    }
}

pub(crate) fn glitch_diagnostics(config: &TabBarGlitchConfig) -> Vec<String> {
    let mut diagnostics = Vec::new();
    if !config.enabled {
        return diagnostics;
    }
    if config.matches.is_empty() {
        diagnostics.push("ui.tab_bar_glitch.matches is empty; nothing to animate".into());
    }
    if config.idle_min_ms > config.idle_max_ms {
        diagnostics
            .push("ui.tab_bar_glitch.idle_min_ms exceeds idle_max_ms; using idle_min_ms".into());
    }
    if config.frames_min > config.frames_max {
        diagnostics
            .push("ui.tab_bar_glitch.frames_min exceeds frames_max; using frames_min".into());
    }
    diagnostics
}
