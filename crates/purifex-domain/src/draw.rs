//! What the interface draws, as plain data: rectangles, lines, text and icons in
//! device-independent units (1 unit = 1/96 inch at 100 % scale). The interface
//! (`purifex-ui`) builds a list of these from the model; the platform draws it
//! with the system's graphics. Neither knows the other.

/// An RGBA color, 8 bits per channel (not premultiplied).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// The color between `self` (`t` = 0) and `other` (`t` = 1), channel by channel.
    pub fn mix(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let lerp = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u8;
        Self {
            r: lerp(self.r, other.r),
            g: lerp(self.g, other.g),
            b: lerp(self.b, other.b),
            a: lerp(self.a, other.a),
        }
    }

    /// The same color with another opacity, 0.0 (clear) to 1.0 (solid).
    pub fn with_alpha(self, opacity: f32) -> Self {
        Self {
            a: (opacity.clamp(0.0, 1.0) * 255.0).round() as u8,
            ..self
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    /// The rectangle shrunk by `by` on every side (grown if negative).
    pub fn inset(&self, by: f32) -> Self {
        Self::new(
            self.x + by,
            self.y + by,
            (self.w - 2.0 * by).max(0.0),
            (self.h - 2.0 * by).max(0.0),
        )
    }

    /// The same rectangle moved down by `dy`.
    pub fn shifted(&self, dy: f32) -> Self {
        Self {
            y: self.y + dy,
            ..*self
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Size {
    pub w: f32,
    pub h: f32,
}

/// The two typefaces of the design: monospace for data, sans for headings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Font {
    Mono,
    Sans,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    pub font: Font,
    /// In units (the height of the em).
    pub size: f32,
    pub bold: bool,
    pub color: Color,
    /// Extra space after each character, in units (the labels in capitals).
    pub tracking: f32,
}

impl TextStyle {
    pub const fn new(font: Font, size: f32, color: Color) -> Self {
        Self {
            font,
            size,
            bold: false,
            color,
            tracking: 0.0,
        }
    }

    pub const fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    pub const fn tracked(mut self, tracking: f32) -> Self {
        self.tracking = tracking;
        self
    }
}

/// One thing to draw. They are drawn in order: later ones are on top.
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    Fill {
        rect: Rect,
        radius: f32,
        color: Color,
    },
    /// The outline is drawn inside the rectangle.
    Stroke {
        rect: Rect,
        radius: f32,
        width: f32,
        color: Color,
    },
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        width: f32,
        color: Color,
    },
    /// One line of text with its top-left corner at (`x`, `y`). With
    /// `max_width`, what does not fit ends in `…`.
    Text {
        x: f32,
        y: f32,
        text: String,
        style: TextStyle,
        max_width: Option<f32>,
    },
    /// A Lucide icon by name, in a square of side `size` (the icons are drawn on a
    /// 24-unit grid with a stroke of 2 units, scaled with the icon).
    Icon {
        name: &'static str,
        x: f32,
        y: f32,
        size: f32,
        color: Color,
    },
    /// A picture of the program by name (`logo-dark-mode`), stretched to `rect`,
    /// `opacity` from 0 to 1.
    Image {
        name: &'static str,
        rect: Rect,
        opacity: f32,
    },
    /// What is drawn until the matching [`Op::Unclip`] is cut to `rect`.
    Clip {
        rect: Rect,
    },
    Unclip,
}

/// A whole picture of the window.
#[derive(Debug, Clone, PartialEq)]
pub struct DrawList {
    pub background: Color,
    pub ops: Vec<Op>,
}

impl DrawList {
    pub fn new(background: Color) -> Self {
        Self {
            background,
            ops: Vec::new(),
        }
    }

    pub fn push(&mut self, op: Op) {
        self.ops.push(op);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rectangle_knows_what_it_contains() {
        let r = Rect::new(10.0, 20.0, 30.0, 40.0);
        assert!(r.contains(10.0, 20.0));
        assert!(r.contains(39.9, 59.9));
        assert!(!r.contains(40.0, 30.0), "the right edge is outside");
        assert!(!r.contains(9.9, 30.0));
        assert_eq!((r.right(), r.bottom()), (40.0, 60.0));
    }

    #[test]
    fn insetting_never_makes_a_negative_size() {
        assert_eq!(
            Rect::new(0.0, 0.0, 10.0, 10.0).inset(2.0),
            Rect::new(2.0, 2.0, 6.0, 6.0)
        );
        assert_eq!(Rect::new(0.0, 0.0, 10.0, 10.0).inset(20.0).w, 0.0);
        assert_eq!(Rect::new(0.0, 5.0, 1.0, 1.0).shifted(-3.0).y, 2.0);
    }

    #[test]
    fn a_color_changes_its_opacity_and_nothing_else() {
        let c = Color::rgb(0, 242, 170).with_alpha(0.5);
        assert_eq!((c.r, c.g, c.b, c.a), (0, 242, 170, 128));
        assert_eq!(Color::rgb(1, 2, 3).with_alpha(9.0).a, 255);
        assert_eq!(Color::rgb(1, 2, 3).with_alpha(-1.0).a, 0);
    }

    #[test]
    fn mixing_goes_from_one_color_to_the_other_and_never_past_them() {
        let (a, b) = (Color::rgb(0, 0, 0), Color::rgba(200, 100, 50, 100));
        assert_eq!(a.mix(b, 0.0), a);
        assert_eq!(a.mix(b, 1.0), b);
        assert_eq!(a.mix(b, 0.5), Color::rgba(100, 50, 25, 178));
        assert_eq!(a.mix(b, 9.0), b);
        assert_eq!(a.mix(b, -9.0), a);
    }

    #[test]
    fn a_style_is_built_by_steps() {
        let s = TextStyle::new(Font::Mono, 12.0, Color::rgb(255, 255, 255))
            .bold()
            .tracked(1.5);
        assert!(s.bold);
        assert_eq!(s.tracking, 1.5);
        assert_eq!(s.font, Font::Mono);
    }
}
