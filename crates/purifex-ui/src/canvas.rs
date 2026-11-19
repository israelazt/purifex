//! The surface the pages draw on: a list of drawing operations, the places that
//! can be clicked, and the helpers that measure and wrap text. The text is
//! measured through the port, so what is laid out is what will be drawn.

use purifex_domain::draw::{Color, DrawList, Op, Rect, TextStyle};
use purifex_domain::ports::TextMetrics;
use purifex_domain::view::Action;

use crate::anim::{key_of, Anims};
use crate::theme;

/// A place that does something when clicked.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub rect: Rect,
    /// Only counts while the pointer is inside this (a scrolled area's window).
    pub clip: Option<Rect>,
    pub action: Action,
}

pub struct Canvas<'a> {
    list: DrawList,
    hits: Vec<Hit>,
    text: &'a dyn TextMetrics,
    mouse: Option<(f32, f32)>,
    clips: Vec<Rect>,
    search_box: Option<Rect>,
    /// The search box has the keyboard (the page draws it lit, with its caret).
    pub search_focus: bool,
    /// The motion that goes on across pictures (hover, numbers, entrances).
    anims: Anims,
    /// How see-through what is drawn now is, and how far below its place (while a
    /// block of the page is appearing).
    alpha: f32,
    dy: f32,
    /// When the page began to appear.
    entered_ms: u64,
    /// Every place drawn with a hover effect, so the window knows when the pointer
    /// goes from one to another (a row, not only a button) and draws again.
    regions: Vec<(Rect, Option<Rect>)>,
}

impl<'a> Canvas<'a> {
    pub fn new(text: &'a dyn TextMetrics, mouse: Option<(f32, f32)>) -> Self {
        Self {
            list: DrawList::new(theme::BG),
            hits: Vec::new(),
            text,
            mouse,
            clips: Vec::new(),
            search_box: None,
            search_focus: false,
            anims: Anims::default(),
            alpha: 1.0,
            dy: 0.0,
            entered_ms: 0,
            regions: Vec::new(),
        }
    }

    /// With the motion of the interface, as it goes on across pictures.
    pub fn with_anims(mut self, anims: Anims, entered_ms: u64) -> Self {
        self.anims = anims;
        self.entered_ms = entered_ms;
        self
    }

    /// The motion, back to whoever keeps it between pictures.
    pub fn take_anims(&mut self) -> Anims {
        std::mem::take(&mut self.anims)
    }

    /// A color with the opacity of the block being drawn.
    fn tint(&self, color: Color) -> Color {
        if self.alpha >= 1.0 {
            color
        } else {
            color.with_alpha(f32::from(color.a) / 255.0 * self.alpha)
        }
    }

    fn lowered(&self, rect: Rect) -> Rect {
        if self.dy == 0.0 {
            rect
        } else {
            rect.shifted(self.dy)
        }
    }

    /// From here on what is drawn belongs to a block that began to appear `delay_ms`
    /// after the page did: it fades in and rises a little. `0` is the page itself.
    pub fn reveal(&mut self, delay_ms: u64) {
        (self.alpha, self.dy) = self.anims.reveal(self.entered_ms, delay_ms);
    }

    /// What is drawn from here on is not part of an appearing block: opaque and in place.
    pub fn rest(&mut self) {
        (self.alpha, self.dy) = (1.0, 0.0);
    }

    /// How much `rect` is hovered, 0 to 1, moving smoothly (see `Anims::hover`).
    pub fn hover(&mut self, rect: Rect) -> f32 {
        self.regions.push((rect, self.clips.last().copied()));
        let inside = self.hovered(rect);
        self.anims.hover(key_of(rect), inside)
    }

    /// A number on its way to `target` (see `Anims::value`).
    pub fn value(&mut self, name: &'static str, target: f32, duration_ms: u64) -> f32 {
        self.anims.value(name, target, duration_ms)
    }

    /// Marks that something still moves.
    pub fn keep_going(&mut self) {
        self.anims.keep_going();
    }

    /// Whether the motion of this picture is not over.
    pub fn moving(&self) -> bool {
        self.anims.active()
    }

    /// The page tells where its search box is, so a click there gives it the keys.
    pub fn set_search_box(&mut self, rect: Rect) {
        self.search_box = Some(rect);
    }

    pub fn take_search_box(&mut self) -> Option<Rect> {
        self.search_box.take()
    }

    /// The places that were drawn with a hover effect, each with the clip it was under.
    pub fn take_regions(&mut self) -> Vec<(Rect, Option<Rect>)> {
        std::mem::take(&mut self.regions)
    }

    pub fn finish(self) -> (DrawList, Vec<Hit>) {
        (self.list, self.hits)
    }

    pub fn fill(&mut self, rect: Rect, radius: f32, color: Color) {
        self.list.push(Op::Fill {
            rect: self.lowered(rect),
            radius,
            color: self.tint(color),
        });
    }

    pub fn stroke(&mut self, rect: Rect, radius: f32, width: f32, color: Color) {
        self.list.push(Op::Stroke {
            rect: self.lowered(rect),
            radius,
            width,
            color: self.tint(color),
        });
    }

    pub fn line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: Color) {
        self.list.push(Op::Line {
            x1,
            y1: y1 + self.dy,
            x2,
            y2: y2 + self.dy,
            width: 1.0,
            color: self.tint(color),
        });
    }

    pub fn icon(&mut self, name: &'static str, x: f32, y: f32, size: f32, color: Color) {
        self.list.push(Op::Icon {
            name,
            x,
            y: y + self.dy,
            size,
            color: self.tint(color),
        });
    }

    fn tinted(&self, style: TextStyle) -> TextStyle {
        TextStyle {
            color: self.tint(style.color),
            ..style
        }
    }

    /// A picture of the program by name, stretched to `rect`.
    pub fn image(&mut self, name: &'static str, rect: Rect) {
        self.list.push(Op::Image {
            name,
            rect: self.lowered(rect),
            opacity: self.alpha,
        });
    }

    pub fn clip(&mut self, rect: Rect) {
        self.clips.push(rect);
        self.list.push(Op::Clip { rect });
    }

    pub fn unclip(&mut self) {
        if self.clips.pop().is_some() {
            self.list.push(Op::Unclip);
        }
    }

    /// The width of a piece of text on one line.
    pub fn width(&self, text: &str, style: &TextStyle) -> f32 {
        self.text.measure(text, style).w
    }

    /// The height of a line of this style.
    pub fn line_height(&self, style: &TextStyle) -> f32 {
        self.text.measure("Ag", style).h
    }

    /// Draws one line with its top-left corner at (`x`, `y`) and returns its width.
    pub fn text(&mut self, x: f32, y: f32, text: &str, style: TextStyle) -> f32 {
        let width = self.width(text, &style);
        self.list.push(Op::Text {
            x,
            y: y + self.dy,
            text: text.to_string(),
            style: self.tinted(style),
            max_width: None,
        });
        width
    }

    /// One line that ends in `…` where it would pass `max_width`.
    pub fn text_cut(&mut self, x: f32, y: f32, text: &str, style: TextStyle, max_width: f32) {
        self.list.push(Op::Text {
            x,
            y: y + self.dy,
            text: text.to_string(),
            style: self.tinted(style),
            max_width: Some(max_width.max(1.0)),
        });
    }

    /// A line of text centered vertically in `bar` (starting at `x`).
    pub fn text_in(&mut self, x: f32, bar: Rect, text: &str, style: TextStyle) -> f32 {
        let y = bar.y + (bar.h - self.line_height(&style)) / 2.0;
        self.text(x, y, text, style)
    }

    /// A line whose right end is at `right`, centered in `bar`.
    pub fn text_right(&mut self, right: f32, bar: Rect, text: &str, style: TextStyle) {
        let x = right - self.width(text, &style);
        self.text_in(x, bar, text, style);
    }

    /// `text` broken into lines of at most `max_width`, at spaces. With
    /// `max_lines`, what does not fit ends the last line in `…`.
    pub fn wrap(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: f32,
        max_lines: Option<usize>,
    ) -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();
        let mut current = String::new();
        for word in text.split_whitespace() {
            let candidate = if current.is_empty() {
                word.to_string()
            } else {
                format!("{current} {word}")
            };
            if current.is_empty() || self.width(&candidate, style) <= max_width {
                current = candidate;
            } else {
                lines.push(std::mem::take(&mut current));
                current = word.to_string();
            }
        }
        if !current.is_empty() {
            lines.push(current);
        }
        if let Some(max) = max_lines.filter(|max| lines.len() > *max && *max > 0) {
            lines.truncate(max);
            if let Some(last) = lines.last_mut() {
                last.push('…');
            }
        }
        lines
    }

    /// A wrapped paragraph; returns the height it took.
    pub fn paragraph(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        text: &str,
        style: TextStyle,
        max_lines: Option<usize>,
    ) -> f32 {
        let step = self.line_height(&style) + 2.0;
        let lines = self.wrap(text, &style, width, max_lines);
        for (i, line) in lines.iter().enumerate() {
            self.list.push(Op::Text {
                x,
                y: y + step * i as f32 + self.dy,
                text: line.clone(),
                style: self.tinted(style),
                max_width: Some(width + 1.0),
            });
        }
        step * lines.len() as f32
    }

    /// Registers a place that does `action` when clicked.
    pub fn hit(&mut self, rect: Rect, action: Action) {
        self.hits.push(Hit {
            rect: self.lowered(rect),
            clip: self.clips.last().copied(),
            action,
        });
    }

    /// Whether the pointer is over `rect` (and not hidden by a clip).
    pub fn hovered(&self, rect: Rect) -> bool {
        self.mouse.is_some_and(|(x, y)| {
            rect.contains(x, y) && self.clips.last().is_none_or(|c| c.contains(x, y))
        })
    }
}

#[cfg(test)]
pub mod testing {
    use purifex_domain::draw::{Font, Size, TextStyle};
    use purifex_domain::ports::TextMetrics;

    /// Text of a fixed width per character, so layouts are predictable.
    pub struct FakeText;

    impl TextMetrics for FakeText {
        fn measure(&self, text: &str, style: &TextStyle) -> Size {
            let per_char = style.size * if style.font == Font::Mono { 0.6 } else { 0.55 };
            Size {
                w: text.chars().count() as f32 * (per_char + style.tracking),
                h: (style.size * 1.35).round(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::FakeText;
    use super::*;

    fn style() -> TextStyle {
        theme::mono(10.0, theme::TEXT) // 6 units per character
    }

    #[test]
    fn text_is_broken_at_spaces_and_never_in_the_middle_of_a_word() {
        let c = Canvas::new(&FakeText, None);
        let lines = c.wrap("uno dos tres cuatro", &style(), 60.0, None);
        assert_eq!(lines, ["uno dos", "tres", "cuatro"]);
        assert_eq!(c.wrap("", &style(), 60.0, None), Vec::<String>::new());
        assert_eq!(
            c.wrap("extraordinariamente", &style(), 30.0, None),
            ["extraordinariamente"],
            "a long word stays whole"
        );
    }

    #[test]
    fn a_paragraph_cut_to_some_lines_ends_in_an_ellipsis() {
        let c = Canvas::new(&FakeText, None);
        let lines = c.wrap("uno dos tres cuatro cinco seis", &style(), 60.0, Some(2));
        assert_eq!(lines.len(), 2);
        assert!(lines[1].ends_with('…'));
    }

    #[test]
    fn a_paragraph_reports_the_height_it_used() {
        let mut c = Canvas::new(&FakeText, None);
        let line = c.line_height(&style()) + 2.0;
        let h = c.paragraph(0.0, 0.0, 60.0, "uno dos tres cuatro", style(), None);
        assert_eq!(h, line * 3.0);
    }

    #[test]
    fn a_click_place_only_counts_inside_the_clip_it_was_made_in() {
        let mut c = Canvas::new(&FakeText, Some((50.0, 50.0)));
        c.clip(Rect::new(0.0, 0.0, 100.0, 40.0));
        c.hit(Rect::new(0.0, 0.0, 100.0, 100.0), Action::Rescan);
        assert!(
            !c.hovered(Rect::new(0.0, 0.0, 100.0, 100.0)),
            "the pointer is outside the clip"
        );
        c.unclip();
        assert!(c.hovered(Rect::new(0.0, 0.0, 100.0, 100.0)));
        let (_, hits) = c.finish();
        assert_eq!(hits[0].clip, Some(Rect::new(0.0, 0.0, 100.0, 40.0)));
    }

    #[test]
    fn unclipping_without_a_clip_does_nothing_and_clips_are_paired() {
        let mut c = Canvas::new(&FakeText, None);
        c.unclip();
        c.clip(Rect::new(0.0, 0.0, 1.0, 1.0));
        c.unclip();
        let (list, _) = c.finish();
        assert_eq!(
            list.ops,
            [
                Op::Clip {
                    rect: Rect::new(0.0, 0.0, 1.0, 1.0)
                },
                Op::Unclip
            ]
        );
    }
}
