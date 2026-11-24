//! The signs that something is going on, drawn with lines and rectangles as functions of the time.
//! With the animations of Windows off (`Canvas::clock` is `None`) each one is a still picture.

use std::f32::consts::TAU;

use purifex_domain::draw::{Color, Rect, TextStyle};

use crate::anim::ease_out;
use crate::canvas::Canvas;
use crate::theme;

const SPOKES: u64 = 12;
/// How long each spoke of the spinner leads: a whole turn is 960 ms.
const SPOKE_MS: u64 = 80;
const RING_MS: u64 = 1400;
const SHINE_MS: u64 = 1200;
const SHINE_WIDTH: f32 = 60.0;
const DOTS_MS: u64 = 400;
/// How long the bar takes to reach a new fraction.
const BAR_MS: u64 = 300;

fn faded(color: Color, alpha: f32) -> Color {
    color.with_alpha(f32::from(color.a) / 255.0 * alpha)
}

/// Twelve spokes around `(cx, cy)`, brighter just behind the one that leads.
pub fn spinner(c: &mut Canvas, cx: f32, cy: f32, r: f32, color: Color) {
    let head = c.clock().map(|ms| (ms / SPOKE_MS) % SPOKES);
    for i in 0..SPOKES {
        let alpha = head.map_or(0.6, |head| {
            0.15 + 0.85 * ((i + SPOKES - head) % SPOKES) as f32 / (SPOKES - 1) as f32
        });
        let angle = TAU * i as f32 / SPOKES as f32;
        let (sin, cos) = angle.sin_cos();
        c.line_w(
            cx + cos * r * 0.55,
            cy + sin * r * 0.55,
            cx + cos * r,
            cy + sin * r,
            2.0,
            faded(color, alpha),
        );
    }
}

/// A bar filled up to `fraction` (0 to 1), which slides to a new value, with a shine that runs
/// along the filled part.
pub fn progress_bar(c: &mut Canvas, rect: Rect, fraction: f32, color: Color) {
    c.fill(rect, 3.0, theme::LINE);
    let shown = c.follow("progress", fraction.clamp(0.0, 1.0), BAR_MS);
    let filled = Rect::new(rect.x, rect.y, rect.w * shown.clamp(0.0, 1.0), rect.h);
    if filled.w <= 0.0 {
        return;
    }
    c.fill(filled, 3.0, color);
    let Some(ms) = c.clock() else { return };
    let along = (ms % SHINE_MS) as f32 / SHINE_MS as f32;
    let x = filled.x - SHINE_WIDTH + (filled.w + SHINE_WIDTH) * along;
    c.clip(filled);
    c.fill(
        Rect::new(x, filled.y, SHINE_WIDTH, filled.h),
        3.0,
        Color::rgb(255, 255, 255).with_alpha(0.25),
    );
    c.unclip();
}

/// A ring that grows out of `rect` and fades, again and again. Nothing when still.
pub fn pulse_ring(c: &mut Canvas, rect: Rect, radius: f32, color: Color) {
    let Some(ms) = c.clock() else { return };
    let t = (ms % RING_MS) as f32 / RING_MS as f32;
    let grown = t * 6.0;
    c.stroke(
        rect.inset(-grown),
        radius + grown,
        2.0,
        faded(color, 0.5 * (1.0 - t)),
    );
}

/// Where `t` (0 to 1) falls between `from` and `to`, eased.
fn part(t: f32, from: f32, to: f32) -> f32 {
    ease_out(((t - from) / (to - from)).clamp(0.0, 1.0))
}

/// A tick of `size` inside a ring, drawn stroke by stroke as `t` goes from 0 to 1.
pub fn check_mark(c: &mut Canvas, center: (f32, f32), size: f32, t: f32, color: Color) {
    let t = t.clamp(0.0, 1.0);
    let ring = Rect::new(center.0 - size / 2.0, center.1 - size / 2.0, size, size);
    c.stroke(ring, size / 2.0, 2.0, faded(color, (t * 2.0).min(1.0)));
    let at = |x: f32, y: f32| (ring.x + x * size / 24.0, ring.y + y * size / 24.0);
    let (start, corner, end) = (at(4.0, 12.0), at(9.0, 17.0), at(20.0, 6.0));
    for ((a, b), done) in [
        ((start, corner), part(t, 0.35, 0.6)),
        ((corner, end), part(t, 0.6, 1.0)),
    ] {
        if done > 0.0 {
            c.line_w(
                a.0,
                a.1,
                a.0 + (b.0 - a.0) * done,
                a.1 + (b.1 - a.1) * done,
                3.0,
                color,
            );
        }
    }
}

/// One, two or three dots, in turn.
pub fn breathing_dots(c: &mut Canvas, x: f32, y: f32, style: TextStyle) {
    let count = c.clock().map_or(3, |ms| 1 + (ms / DOTS_MS) % 3);
    c.text(x, y, &".".repeat(count as usize), style);
}

#[cfg(test)]
mod tests {
    use purifex_domain::draw::Op;

    use super::*;
    use crate::anim::Anims;
    use crate::canvas::testing::FakeText;

    fn canvas_at(ms: u64, motion: bool) -> Canvas<'static> {
        let mut anims = Anims::new();
        anims.set_motion(motion);
        anims.begin(ms);
        Canvas::new(&FakeText, None).with_anims(anims, 0)
    }

    fn lines(c: Canvas) -> Vec<(f32, f32, f32, f32, u8)> {
        c.finish()
            .0
            .ops
            .iter()
            .filter_map(|op| match op {
                Op::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    color,
                    ..
                } => Some((*x1, *y1, *x2, *y2, color.a)),
                _ => None,
            })
            .collect()
    }

    fn spokes_at(ms: u64, motion: bool) -> (Vec<u8>, bool) {
        let mut c = canvas_at(ms, motion);
        spinner(&mut c, 10.0, 10.0, 8.0, theme::AMBER);
        let moving = c.moving();
        (lines(c).into_iter().map(|l| l.4).collect(), moving)
    }

    #[test]
    fn the_spinner_turns_and_keeps_the_window_asking_for_pictures() {
        let (first, moving) = spokes_at(1000, true);
        let (later, _) = spokes_at(1080, true);
        assert_eq!(first.len(), 12);
        assert_ne!(first, later);
        assert!(moving);
    }

    #[test]
    fn without_animations_the_spinner_is_still_and_asks_for_nothing() {
        let (alphas, moving) = spokes_at(1000, false);
        assert!(alphas.windows(2).all(|w| w[0] == w[1]));
        assert!(!moving);
    }

    #[test]
    fn the_tick_is_not_there_at_the_start_and_is_whole_at_the_end() {
        let mut c = canvas_at(0, true);
        check_mark(&mut c, (28.0, 28.0), 56.0, 0.0, theme::ACCENT);
        assert!(lines(c).is_empty());
        let mut c = canvas_at(0, true);
        check_mark(&mut c, (28.0, 28.0), 56.0, 1.0, theme::ACCENT);
        let drawn = lines(c);
        assert_eq!(drawn.len(), 2);
        let end = drawn[1];
        assert!((end.2 - 20.0 * 56.0 / 24.0).abs() < 0.01, "{end:?}");
    }

    #[test]
    fn the_bar_starts_where_it_is_told_without_counting_from_zero() {
        let mut c = canvas_at(0, true);
        progress_bar(&mut c, Rect::new(0.0, 0.0, 200.0, 6.0), 0.5, theme::ACCENT);
        let filled = c
            .finish()
            .0
            .ops
            .iter()
            .any(|op| matches!(op, Op::Fill { rect, .. } if (rect.w - 100.0).abs() < 0.01));
        assert!(filled);
    }

    #[test]
    fn the_dots_breathe_in_turns_of_one_two_and_three() {
        let said = |ms| {
            let mut c = canvas_at(ms, true);
            breathing_dots(&mut c, 0.0, 0.0, theme::mono(10.0, theme::TEXT));
            c.finish()
                .0
                .ops
                .into_iter()
                .find_map(|op| match op {
                    Op::Text { text, .. } => Some(text),
                    _ => None,
                })
                .unwrap_or_default()
        };
        assert_eq!([said(0), said(400), said(800)], [".", "..", "..."]);
    }
}
