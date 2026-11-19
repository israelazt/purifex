//! Motion for the interface: what makes it feel alive without being noisy. All of
//! it is a function of **time** (`now`), so it is tested by asking for the picture
//! at a given moment, and it says whether anything is still moving, so the window
//! only asks for pictures while something does: once everything rests, the window
//! waits for messages again and costs no CPU.

use std::collections::HashMap;

use purifex_domain::draw::Rect;

/// How long a hover takes to fade in or out.
const HOVER_MS: f32 = 140.0;
/// How long a block of the page takes to appear.
pub const REVEAL_MS: f32 = 260.0;
/// How far an appearing block rises, in units.
const REVEAL_RISE: f32 = 10.0;
/// How long a number takes to count up to its value.
pub const COUNT_MS: u64 = 600;

/// Fast at first and gentle at the end: the way things settle in the real world.
pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

#[derive(Debug, Clone, Copy)]
struct Tween {
    from: f32,
    to: f32,
    start_ms: u64,
    duration_ms: u64,
}

impl Tween {
    fn at(&self, now_ms: u64) -> f32 {
        if self.duration_ms == 0 {
            return self.to;
        }
        let t = now_ms.saturating_sub(self.start_ms) as f32 / self.duration_ms as f32;
        self.from + (self.to - self.from) * ease_out(t)
    }

    fn done(&self, now_ms: u64) -> bool {
        now_ms >= self.start_ms + self.duration_ms
    }
}

/// The state of every motion that goes on across pictures.
#[derive(Debug, Default)]
pub struct Anims {
    /// With `false` everything is at its final place at once.
    motion: bool,
    now_ms: u64,
    last_ms: u64,
    /// How much each place is hovered, 0 to 1 (keyed by where it is).
    hover: HashMap<u64, f32>,
    hover_next: HashMap<u64, f32>,
    values: HashMap<&'static str, Tween>,
    /// Something was not at rest in this picture.
    active: bool,
}

/// A place of the window as a key: where it is, to the unit.
pub fn key_of(rect: Rect) -> u64 {
    let q = |v: f32| (v.round() as i64 as u64) & 0xFFFF;
    q(rect.x) | q(rect.y) << 16 | q(rect.w) << 32 | q(rect.h) << 48
}

impl Anims {
    pub fn new() -> Self {
        Self {
            motion: true,
            ..Self::default()
        }
    }

    /// Turns the motion on or off (off when Windows has animations off).
    pub fn set_motion(&mut self, on: bool) {
        self.motion = on;
    }

    pub fn motion(&self) -> bool {
        self.motion
    }

    pub fn now_ms(&self) -> u64 {
        self.now_ms
    }

    /// Starts a picture at `now_ms`.
    pub fn begin(&mut self, now_ms: u64) {
        self.last_ms = if self.last_ms == 0 && self.now_ms == 0 {
            now_ms
        } else {
            self.now_ms
        };
        self.now_ms = now_ms;
        self.active = false;
        self.hover_next.clear();
    }

    /// Ends the picture: places that were not drawn forget their hover.
    pub fn end(&mut self) {
        self.hover = std::mem::take(&mut self.hover_next);
    }

    /// Whether anything is still moving after the last picture.
    pub fn active(&self) -> bool {
        self.active
    }

    /// The hover of a place, moving toward 1 while the pointer is over it and
    /// toward 0 when it is not. A place seen for the first time starts at its
    /// target, so scrolling under a still pointer does not flicker.
    pub fn hover(&mut self, key: u64, inside: bool) -> f32 {
        let target = if inside { 1.0 } else { 0.0 };
        if !self.motion {
            self.hover_next.insert(key, target);
            return target;
        }
        let before = self.hover.get(&key).copied().unwrap_or(target);
        let step = self.now_ms.saturating_sub(self.last_ms) as f32 / HOVER_MS;
        let now = if before < target {
            (before + step).min(target)
        } else {
            (before - step).max(target)
        };
        if (now - target).abs() > 0.001 {
            self.active = true;
        }
        self.hover_next.insert(key, now);
        now
    }

    /// A number that goes to `target`: when the target changes it starts from
    /// what was shown and takes `duration_ms`. A number seen for the first time
    /// counts up from zero.
    pub fn value(&mut self, name: &'static str, target: f32, duration_ms: u64) -> f32 {
        if !self.motion {
            return target;
        }
        let now = self.now_ms;
        let tween = match self.values.get(name).copied() {
            Some(t) if (t.to - target).abs() < f32::EPSILON => t,
            Some(t) => Tween {
                from: t.at(now),
                to: target,
                start_ms: now,
                duration_ms,
            },
            None => Tween {
                from: 0.0,
                to: target,
                start_ms: now,
                duration_ms,
            },
        };
        self.values.insert(name, tween);
        if !tween.done(now) {
            self.active = true;
        }
        tween.at(now)
    }

    /// How far a block that began to appear at `start_ms` (plus `delay_ms`) has
    /// got: its opacity (0 to 1) and how many units below its place it still is.
    pub fn reveal(&mut self, start_ms: u64, delay_ms: u64) -> (f32, f32) {
        if !self.motion {
            return (1.0, 0.0);
        }
        let t = self.now_ms.saturating_sub(start_ms + delay_ms) as f32 / REVEAL_MS;
        if self.now_ms < start_ms + delay_ms + REVEAL_MS as u64 {
            self.active = true;
        }
        let e = ease_out(t);
        (e, (1.0 - e) * REVEAL_RISE)
    }

    /// Marks that something still moves (the scroll of the page, say).
    pub fn keep_going(&mut self) {
        self.active = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(a: &mut Anims, ms: u64) {
        a.end();
        a.begin(ms);
    }

    #[test]
    fn easing_goes_from_zero_to_one_fast_at_first_and_never_leaves_that_range() {
        assert_eq!(ease_out(0.0), 0.0);
        assert_eq!(ease_out(1.0), 1.0);
        assert!(ease_out(0.5) > 0.8, "more than half done at half the time");
        assert_eq!((ease_out(-3.0), ease_out(9.0)), (0.0, 1.0));
        assert!((0..=100)
            .map(|i| ease_out(i as f32 / 100.0))
            .collect::<Vec<_>>()
            .windows(2)
            .all(|w| w[0] <= w[1]));
    }

    #[test]
    fn a_hover_fades_in_and_out_in_steps_and_reports_that_it_is_moving() {
        let mut a = Anims::new();
        a.begin(1000);
        assert_eq!(a.hover(1, false), 0.0);
        assert!(!a.active(), "at rest");
        at(&mut a, 1070);
        let half = a.hover(1, true);
        assert!(half > 0.4 && half < 0.6, "{half}");
        assert!(a.active());
        at(&mut a, 1300);
        assert_eq!(a.hover(1, true), 1.0);
        assert!(
            !a.active(),
            "settled: the window can stop asking for pictures"
        );
        at(&mut a, 1370);
        assert!(a.hover(1, false) < 1.0 && a.active(), "fading out");
    }

    #[test]
    fn a_place_seen_for_the_first_time_starts_at_its_target() {
        let mut a = Anims::new();
        a.begin(500);
        assert_eq!(
            a.hover(7, true),
            1.0,
            "scrolling under a still pointer does not flicker"
        );
        assert!(!a.active());
    }

    #[test]
    fn a_place_that_is_not_drawn_any_more_forgets_its_hover() {
        let mut a = Anims::new();
        a.begin(0);
        a.hover(1, true);
        at(&mut a, 16);
        a.hover(2, false);
        at(&mut a, 32);
        assert_eq!(
            a.hover(1, false),
            0.0,
            "it came back from nothing, not from where it was"
        );
    }

    #[test]
    fn a_number_counts_up_from_zero_then_stays_and_a_new_target_starts_from_what_was_shown() {
        let mut a = Anims::new();
        a.begin(0);
        assert_eq!(a.value("n", 100.0, COUNT_MS), 0.0);
        assert!(a.active());
        at(&mut a, COUNT_MS / 2);
        let mid = a.value("n", 100.0, COUNT_MS);
        assert!(mid > 50.0 && mid < 100.0, "{mid}");
        at(&mut a, COUNT_MS + 10);
        assert_eq!(a.value("n", 100.0, COUNT_MS), 100.0);
        assert!(!a.active());
        at(&mut a, COUNT_MS + 20);
        a.value("n", 200.0, COUNT_MS);
        at(&mut a, COUNT_MS + 20 + COUNT_MS / 2);
        let next = a.value("n", 200.0, COUNT_MS);
        assert!(
            next > 100.0 && next < 200.0,
            "from 100 toward 200, not from 0: {next}"
        );
    }

    #[test]
    fn a_block_appears_with_a_fade_and_a_small_rise_and_a_delay_holds_it_back() {
        let mut a = Anims::new();
        a.begin(1000);
        assert_eq!(a.reveal(1000, 0), (0.0, REVEAL_RISE));
        assert_eq!(a.reveal(1000, 100).0, 0.0);
        at(&mut a, 1130);
        let (alpha, rise) = a.reveal(1000, 0);
        assert!(
            alpha > 0.5 && alpha < 1.0 && rise < REVEAL_RISE && rise > 0.0,
            "{alpha} {rise}"
        );
        assert!(a.reveal(1000, 100).0 < alpha, "the delayed one is behind");
        at(&mut a, 1000 + REVEAL_MS as u64 + 100);
        assert_eq!(a.reveal(1000, 0), (1.0, 0.0));
        assert!(!a.active());
    }

    #[test]
    fn with_motion_off_everything_is_at_its_final_place_and_nothing_is_ever_active() {
        let mut a = Anims::new();
        a.set_motion(false);
        a.begin(0);
        assert_eq!(a.hover(1, true), 1.0);
        assert_eq!(a.value("n", 42.0, COUNT_MS), 42.0);
        assert_eq!(a.reveal(0, 500), (1.0, 0.0));
        assert!(!a.active());
    }

    #[test]
    fn two_places_of_the_window_have_different_keys_and_the_same_place_the_same_one() {
        let r = Rect::new(10.0, 20.0, 100.0, 40.0);
        assert_eq!(key_of(r), key_of(Rect::new(10.2, 19.8, 100.0, 40.0)));
        assert_ne!(key_of(r), key_of(Rect::new(10.0, 70.0, 100.0, 40.0)));
        assert_ne!(key_of(r), key_of(Rect::new(10.0, 20.0, 101.0, 40.0)));
    }
}
