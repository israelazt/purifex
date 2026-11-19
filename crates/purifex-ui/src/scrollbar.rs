//! The scroll bar of a page: a thumb that can be dragged, a track that can be
//! clicked to go a page at a time. The wheel and the keys are handled by `Ui`; this
//! only knows where the bar is and how a position of the pointer maps to a scroll.

use purifex_domain::draw::Rect;

/// Width of the strip at the right edge that answers to the pointer, in units.
const TRACK_W: f32 = 16.0;
const THUMB_W: f32 = 6.0;
const MIN_THUMB: f32 = 32.0;

/// What pressing on the bar asked for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pressed {
    /// The thumb was taken: moves now follow the pointer.
    Grabbed,
    /// The track beside the thumb: go this many page lengths (negative is up).
    Page(f32),
}

#[derive(Debug, Default)]
pub struct Scrollbar {
    track: Option<Rect>,
    thumb: Option<Rect>,
    /// While the thumb is held: where in it the pointer took it (from its top).
    grab: Option<f32>,
}

impl Scrollbar {
    /// Places the bar for a page of `content` units seen through `main`, scrolled to
    /// `scroll`. A page that fits has no bar.
    pub fn layout(&mut self, main: Rect, right: f32, scroll: f32, content: f32) {
        let max = content - main.h;
        if max <= 0.0 || main.h <= 0.0 {
            (self.track, self.thumb, self.grab) = (None, None, None);
            return;
        }
        let track = Rect::new(right - TRACK_W, main.y, TRACK_W, main.h);
        let height = (main.h * main.h / content).max(MIN_THUMB).min(main.h);
        let top = track.y + (scroll / max).clamp(0.0, 1.0) * (track.h - height);
        self.track = Some(track);
        self.thumb = Some(Rect::new(right - THUMB_W - 4.0, top, THUMB_W, height));
    }

    pub fn thumb(&self) -> Option<Rect> {
        self.thumb
    }

    pub fn track(&self) -> Option<Rect> {
        self.track
    }

    pub fn dragging(&self) -> bool {
        self.grab.is_some()
    }

    /// A press at (`x`, `y`): `None` if it is not on the bar.
    pub fn press(&mut self, x: f32, y: f32) -> Option<Pressed> {
        let (track, thumb) = (self.track?, self.thumb?);
        if !track.contains(x, y) {
            return None;
        }
        // The thumb is hit a little wider than it is drawn: easier to take.
        let grab_area = Rect::new(track.x, thumb.y, track.w, thumb.h);
        if grab_area.contains(x, y) {
            self.grab = Some(y - thumb.y);
            Some(Pressed::Grabbed)
        } else {
            Some(Pressed::Page(if y < thumb.y { -1.0 } else { 1.0 }))
        }
    }

    /// The scroll that puts the thumb where the pointer (at `y`) holds it.
    pub fn scroll_for(&self, y: f32, max_scroll: f32) -> Option<f32> {
        let (track, thumb, grab) = (self.track?, self.thumb?, self.grab?);
        let travel = track.h - thumb.h;
        if travel <= 0.0 {
            return Some(0.0);
        }
        Some(((y - grab - track.y) / travel).clamp(0.0, 1.0) * max_scroll)
    }

    pub fn release(&mut self) -> bool {
        self.grab.take().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(scroll: f32) -> Scrollbar {
        let mut b = Scrollbar::default();
        b.layout(
            Rect::new(264.0, 52.0, 1176.0, 800.0),
            1440.0,
            scroll,
            3200.0,
        );
        b
    }

    #[test]
    fn a_page_that_fits_has_no_bar_and_a_long_one_has_a_thumb_in_proportion() {
        let mut b = Scrollbar::default();
        b.layout(Rect::new(0.0, 0.0, 100.0, 800.0), 100.0, 0.0, 800.0);
        assert!(b.thumb().is_none() && b.track().is_none());
        let b = bar(0.0);
        let thumb = b.thumb().unwrap();
        assert!(
            (thumb.h - 200.0).abs() < 1.0,
            "a quarter of the page is seen: {}",
            thumb.h
        );
        assert_eq!(thumb.y, 52.0);
        assert!(
            bar(2400.0).thumb().unwrap().bottom() <= 852.0 + 0.5,
            "at the end it stays in the track"
        );
    }

    #[test]
    fn the_thumb_can_be_taken_and_dragged_all_the_way_both_ways() {
        let mut b = bar(0.0);
        let t = b.thumb().unwrap();
        assert_eq!(b.press(t.x + 2.0, t.y + 10.0), Some(Pressed::Grabbed));
        assert!(b.dragging());
        let max = 3200.0 - 800.0;
        assert_eq!(b.scroll_for(t.y + 10.0, max), Some(0.0), "not moved");
        assert_eq!(
            b.scroll_for(2000.0, max),
            Some(max),
            "past the end: the end"
        );
        assert_eq!(b.scroll_for(-500.0, max), Some(0.0), "above: the start");
        let middle = b.scroll_for(52.0 + 10.0 + 300.0, max).unwrap();
        assert!(middle > 0.0 && middle < max, "{middle}");
        assert!(b.release());
        assert!(!b.dragging() && !b.release());
    }

    #[test]
    fn the_track_beside_the_thumb_goes_a_page_toward_the_click_and_the_rest_is_not_the_bar() {
        let mut b = bar(800.0);
        let t = b.thumb().unwrap();
        assert_eq!(b.press(t.x, t.bottom() + 40.0), Some(Pressed::Page(1.0)));
        assert_eq!(b.press(t.x, t.y - 20.0), Some(Pressed::Page(-1.0)));
        assert!(!b.dragging(), "a page jump does not hold the thumb");
        assert_eq!(b.press(700.0, 400.0), None, "the page itself");
    }
}
