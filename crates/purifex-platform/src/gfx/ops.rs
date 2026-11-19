//! Drawing one operation of a picture with Direct2D: a fill, an outline, a line, text, an
//! icon, an image or a clip. Each is a small method with the `SAFETY` of its own call.

use purifex_domain::draw::{Color, Op, Rect, TextStyle};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_ALIASED, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, D2D1_DRAW_TEXT_OPTIONS_NONE,
};
use windows_numerics::{Matrix3x2, Vector2};

use super::{rect_f, rounded, Renderer};

impl Renderer {
    /// Draws one operation; `clips` counts the clips pushed and not yet popped.
    pub(super) fn draw_op(&mut self, op: &Op, clips: &mut usize) {
        match op {
            Op::Fill {
                rect,
                radius,
                color,
            } => self.fill(*rect, *radius, *color),
            Op::Stroke {
                rect,
                radius,
                width,
                color,
            } => self.stroke(*rect, *radius, *width, *color),
            Op::Line {
                x1,
                y1,
                x2,
                y2,
                width,
                color,
            } => self.line((*x1, *y1), (*x2, *y2), *width, *color),
            Op::Text {
                x,
                y,
                text,
                style,
                max_width,
            } => self.draw_text(*x, *y, text, style, *max_width),
            Op::Icon {
                name,
                x,
                y,
                size,
                color,
            } => self.icon(name, *x, *y, *size, *color),
            Op::Image {
                name,
                rect,
                opacity,
            } => self.image(name, *rect, *opacity),
            Op::Clip { rect } => {
                // SAFETY: pushes a clip on the live target; it is popped by `Unclip` or at the end.
                unsafe {
                    self.target
                        .PushAxisAlignedClip(&rect_f(*rect), D2D1_ANTIALIAS_MODE_ALIASED);
                }
                *clips += 1;
            }
            Op::Unclip => {
                if *clips > 0 {
                    // SAFETY: pops the last clip pushed.
                    unsafe { self.target.PopAxisAlignedClip() };
                    *clips -= 1;
                }
            }
        }
    }

    fn fill(&self, rect: Rect, radius: f32, color: Color) {
        self.paint(color);
        // SAFETY: a drawing call on the live target and brush.
        unsafe {
            self.target
                .FillRoundedRectangle(&rounded(rect, radius), &self.brush);
        }
    }

    /// An outline drawn inside the rectangle, so its outer edge is the rectangle's.
    fn stroke(&self, rect: Rect, radius: f32, width: f32, color: Color) {
        self.paint(color);
        let inner = rect.inset(width / 2.0);
        // SAFETY: a drawing call on the live target and brush.
        unsafe {
            self.target.DrawRoundedRectangle(
                &rounded(inner, (radius - width / 2.0).max(0.0)),
                &self.brush,
                width,
                None,
            );
        }
    }

    fn line(&self, from: (f32, f32), to: (f32, f32), width: f32, color: Color) {
        self.paint(color);
        // SAFETY: a drawing call on the live target and brush.
        unsafe {
            self.target.DrawLine(
                Vector2 {
                    X: from.0,
                    Y: from.1,
                },
                Vector2 { X: to.0, Y: to.1 },
                &self.brush,
                width,
                None,
            );
        }
    }

    fn draw_text(&mut self, x: f32, y: f32, text: &str, style: &TextStyle, max_width: Option<f32>) {
        let Ok(layout) = self.text.layout(text, style, max_width) else {
            return;
        };
        self.paint(style.color);
        // SAFETY: a drawing call on the live target, brush and layout.
        unsafe {
            self.target.DrawTextLayout(
                Vector2 { X: x, Y: y },
                &layout,
                &self.brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
            );
        }
    }

    /// A Lucide icon (a 24 x 24 outline) scaled to `size` at (`x`, `y`).
    fn icon(&mut self, name: &'static str, x: f32, y: f32, size: f32, color: Color) {
        let Some(geometry) = self.geometry(name) else {
            return;
        };
        self.paint(color);
        let scale = size / 24.0;
        // SAFETY: the transform is set and put back around one drawing call on the live target.
        unsafe {
            self.target.SetTransform(&Matrix3x2 {
                M11: scale,
                M12: 0.0,
                M21: 0.0,
                M22: scale,
                M31: x,
                M32: y,
            });
            self.target
                .DrawGeometry(&geometry, &self.brush, 2.0, &self.stroke);
            self.target.SetTransform(&Matrix3x2::identity());
        }
    }

    fn image(&mut self, name: &'static str, rect: Rect, opacity: f32) {
        let Some(bitmap) = self.picture(name) else {
            return;
        };
        // SAFETY: a drawing call on the live target and bitmap.
        unsafe {
            self.target.DrawBitmap(
                &bitmap,
                Some(&rect_f(rect)),
                opacity.clamp(0.0, 1.0),
                D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                None,
            );
        }
    }
}
