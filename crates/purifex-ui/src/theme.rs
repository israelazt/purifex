//! The colors and the type of the design (`docs/DESIGN.md`), measured from the
//! reference. One place, so a change of look is a change here.

use purifex_domain::draw::{Color, Font, TextStyle};

pub const BG: Color = Color::rgb(7, 8, 11);
pub const BG_SIDE: Color = Color::rgb(9, 11, 15);
pub const PANEL: Color = Color::rgb(13, 15, 21);
pub const PANEL_2: Color = Color::rgb(12, 14, 20);
pub const INPUT: Color = Color::rgb(5, 6, 10);
pub const ICON_BOX: Color = Color::rgb(17, 20, 27);
pub const LINE: Color = Color::rgb(30, 32, 38);
pub const LINE_SOFT: Color = Color::rgb(22, 25, 34);
pub const TEXT: Color = Color::rgb(242, 242, 242);
pub const TEXT_2: Color = Color::rgb(154, 163, 178);
pub const TEXT_3: Color = Color::rgb(94, 104, 118);
pub const ACCENT: Color = Color::rgb(0, 242, 170);
pub const ACCENT_2: Color = Color::rgb(0, 218, 153);
pub const ON_ACCENT: Color = Color::rgb(0, 17, 11);
pub const RED: Color = Color::rgb(244, 63, 94);
pub const BLUE: Color = Color::rgb(56, 189, 248);
pub const AMBER: Color = Color::rgb(251, 191, 36);
pub const CHECK_BLUE: Color = Color::rgb(29, 107, 255);

/// The width of the side bar and the height of the top bar, in units.
pub const SIDEBAR_W: f32 = 264.0;
pub const TOPBAR_H: f32 = 52.0;
/// The room around the content of a page.
pub const PAGE_PAD: f32 = 32.0;

pub fn mono(size: f32, color: Color) -> TextStyle {
    TextStyle::new(Font::Mono, size, color)
}

pub fn sans(size: f32, color: Color) -> TextStyle {
    TextStyle::new(Font::Sans, size, color)
}

/// The small labels in capitals (`ESTADO GENERAL`).
pub fn eyebrow(color: Color) -> TextStyle {
    TextStyle::new(Font::Mono, 10.5, color).tracked(1.4)
}

/// A state color and its tints: the fill and the border of a tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Green,
    Red,
    Blue,
    Amber,
    Gray,
}

impl Tone {
    pub fn color(self) -> Color {
        match self {
            Self::Green => ACCENT,
            Self::Red => RED,
            Self::Blue => BLUE,
            Self::Amber => AMBER,
            Self::Gray => TEXT_2,
        }
    }

    pub fn fill(self) -> Color {
        match self {
            Self::Gray => Color::rgb(18, 21, 28),
            other => other.color().with_alpha(0.11),
        }
    }

    pub fn border(self) -> Color {
        match self {
            Self::Gray => LINE,
            other => other.color().with_alpha(0.38),
        }
    }
}
