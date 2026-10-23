//! Client-side "signal interference" animation for a right-side tab bar
//! entry. Pure presentation state: nothing here touches the wire.

use std::time::{Duration, Instant};

use crate::config::TabBarGlitchConfig;

#[derive(Debug, Clone)]
pub(crate) struct GlitchState {
    config: TabBarGlitchConfig,
    rng: u64,
    /// When the next burst starts while idle.
    next_burst_at: Instant,
    burst: Option<Burst>,
}

#[derive(Debug, Clone)]
struct Burst {
    total: u32,
    remaining: u32,
    frame_seed: u64,
    frame_ends_at: Instant,
}

impl GlitchState {
    pub(crate) fn new(config: TabBarGlitchConfig, now: Instant) -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0x9E37_79B9_7F4A_7C15, |d| d.as_nanos() as u64 ^ 0x9E37_79B9_7F4A_7C15);
        let mut state = Self {
            config,
            rng: seed | 1,
            next_burst_at: now,
            burst: None,
        };
        state.schedule_idle(now);
        state
    }

    pub(crate) fn reconfigure(&mut self, config: TabBarGlitchConfig, now: Instant) {
        if self.config != config {
            self.config = config;
            self.burst = None;
            self.schedule_idle(now);
        }
    }

    pub(crate) fn is_active(&self) -> bool {
        self.config.active()
    }

    fn next_u64(&mut self) -> u64 {
        // xorshift64*: good enough for picking frames, no crate needed.
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn range(&mut self, lo: u64, hi: u64) -> u64 {
        if hi <= lo {
            lo
        } else {
            lo + self.next_u64() % (hi - lo + 1)
        }
    }

    fn schedule_idle(&mut self, now: Instant) {
        let (lo, hi) = self.config.idle_range_ms();
        let wait = self.range(lo, hi);
        self.next_burst_at = now + Duration::from_millis(wait);
    }

    /// Deadline the client loop should wake at, if the animation needs a tick.
    pub(crate) fn deadline(&self) -> Option<Instant> {
        if !self.is_active() {
            return None;
        }
        Some(
            self.burst
                .as_ref()
                .map_or(self.next_burst_at, |burst| burst.frame_ends_at),
        )
    }

    /// Advance the animation. Returns true when the tab bar must be repainted.
    pub(crate) fn tick(&mut self, now: Instant) -> bool {
        if !self.is_active() {
            return false;
        }
        let Some(burst) = self.burst.take() else {
            if now < self.next_burst_at {
                return false;
            }
            let (lo, hi) = self.config.frames_range();
            let total = self.range(u64::from(lo), u64::from(hi)) as u32;
            let frame_seed = self.next_u64();
            self.burst = Some(Burst {
                total,
                remaining: total,
                frame_seed,
                frame_ends_at: now + self.config.frame_duration(),
            });
            return true;
        };
        if now < burst.frame_ends_at {
            self.burst = Some(burst);
            return false;
        }
        let remaining = burst.remaining.saturating_sub(1);
        if remaining == 0 {
            self.schedule_idle(now);
        } else {
            self.burst = Some(Burst {
                total: burst.total,
                remaining,
                frame_seed: burst.frame_seed.rotate_left(17) ^ 0xD1B5_4A32_D192_ED03,
                frame_ends_at: now + self.config.frame_duration(),
            });
        }
        true
    }

    /// The corrupted text for `text` in the current frame, plus whether this
    /// frame is "heavy" (eligible for `noise_fg`). `None` when the entry is
    /// not animated right now.
    pub(crate) fn render(&self, text: &str) -> Option<(String, bool)> {
        let burst = self.burst.as_ref()?;
        if !self.is_active() || !text.contains(&self.config.matches) {
            return None;
        }
        // Intensity 1.0 on the first frame, decaying to ~0 on the last.
        let intensity = f64::from(burst.remaining) / f64::from(burst.total.max(1));
        let noise = self.config.noise_glyphs();
        Some((
            corrupt(text, intensity, burst.frame_seed, &noise),
            intensity > 0.6,
        ))
    }
}

struct FrameRng(u64);

impl FrameRng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn chance(&mut self, p: f64) -> bool {
        (self.next() % 10_000) as f64 / 10_000.0 < p
    }

    fn pick(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

fn corrupt(text: &str, intensity: f64, seed: u64, noise: &[char]) -> String {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    if len < 3 || noise.is_empty() {
        return text.to_string();
    }
    let mut rng = FrameRng(seed | 1);
    let mut out: Vec<char> = Vec::with_capacity(len);
    for &c in &chars {
        let glyph = if c == ' ' {
            if rng.chance(intensity * 0.5) {
                noise[rng.pick(noise.len())]
            } else {
                ' '
            }
        } else if rng.chance(intensity * 0.7) {
            noise[rng.pick(noise.len())]
        } else if rng.chance(intensity * 0.5) {
            leet(c)
        } else if rng.chance(intensity * 0.3) {
            c.to_ascii_lowercase()
        } else {
            c
        };
        out.push(glyph);
    }
    // Horizontal tearing: rotate a slice of the text by a few cells.
    if rng.chance(intensity) {
        let cut = 1 + rng.pick(len - 2);
        out.rotate_left(cut);
    }
    // Smear: repeat one character over its neighbour.
    if rng.chance(intensity * 0.8) {
        let i = rng.pick(len - 1);
        out[i + 1] = out[i];
    }
    out.into_iter().collect()
}

fn leet(c: char) -> char {
    match c {
        'A' => '4',
        'E' => '3',
        'I' => '1',
        'T' => '7',
        'S' => '5',
        'O' => '0',
        'R' => 'Я',
        'F' => 'Ƒ',
        'L' => 'Ł',
        'N' => 'И',
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> TabBarGlitchConfig {
        TabBarGlitchConfig {
            enabled: true,
            matches: "AFTER".into(),
            idle_min_ms: 200,
            idle_max_ms: 200,
            frames_min: 3,
            frames_max: 3,
            frame_ms: 30,
            ..TabBarGlitchConfig::default()
        }
    }

    #[test]
    fn bursts_run_for_the_configured_frames_and_keep_width() {
        let start = Instant::now();
        let mut glitch = GlitchState::new(config(), start);
        assert!(glitch.render("AFTER LIFE").is_none());
        assert!(!glitch.tick(start));
        let t = start + Duration::from_millis(201);
        assert!(glitch.tick(t));
        let mut frames = 0;
        let mut at = t;
        while let Some((text, _)) = glitch.render("▞ AFTER LIFE ▚") {
            assert_eq!(text.chars().count(), "▞ AFTER LIFE ▚".chars().count());
            frames += 1;
            at += Duration::from_millis(31);
            glitch.tick(at);
            assert!(frames < 10, "burst never ended");
        }
        assert_eq!(frames, 3);
        assert!(glitch.render("AFTER LIFE").is_none());
        assert!(glitch.deadline().is_some());
    }

    #[test]
    fn disabled_config_never_animates() {
        let start = Instant::now();
        let mut glitch = GlitchState::new(TabBarGlitchConfig::default(), start);
        assert!(glitch.deadline().is_none());
        assert!(!glitch.tick(start + Duration::from_secs(60)));
        assert!(glitch.render("anything").is_none());
    }

    #[test]
    fn unmatched_text_is_left_alone() {
        let start = Instant::now();
        let mut glitch = GlitchState::new(config(), start);
        glitch.tick(start + Duration::from_millis(201));
        assert!(glitch.render("clock 12:00").is_none());
        assert!(glitch.render("AFTER LIFE").is_some());
    }
}
