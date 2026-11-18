//! Drawing the interface with Direct2D: a list of rectangles, lines, text and
//! icons (`purifex_domain::draw`) turned into pixels, on a window or on a bitmap
//! (for the screenshots that check the design). Event-driven: it draws when it
//! is asked to, so a window nobody touches costs nothing.

mod capture;
mod text;

use std::collections::HashMap;

use purifex_domain::draw::{Color, DrawList, Op, Rect};
use purifex_domain::icons::{icon, Seg};
use windows::core::Interface;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_BEZIER_SEGMENT, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_HOLLOW,
    D2D1_FIGURE_END_CLOSED, D2D1_FIGURE_END_OPEN, D2D1_PIXEL_FORMAT, D2D_RECT_F, D2D_SIZE_U,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1Factory, ID2D1HwndRenderTarget, ID2D1PathGeometry, ID2D1RenderTarget,
    ID2D1SolidColorBrush, ID2D1StrokeStyle, D2D1_ANTIALIAS_MODE_ALIASED, D2D1_CAP_STYLE_ROUND,
    D2D1_DASH_STYLE_SOLID, D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_FACTORY_TYPE_SINGLE_THREADED,
    D2D1_FEATURE_LEVEL_DEFAULT, D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_LINE_JOIN_ROUND,
    D2D1_PRESENT_OPTIONS_NONE, D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT,
    D2D1_RENDER_TARGET_USAGE_NONE, D2D1_ROUNDED_RECT, D2D1_STROKE_STYLE_PROPERTIES,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows_numerics::{Matrix3x2, Vector2};

pub use capture::render_png;
pub use text::TextEngine;

/// `D2DERR_RECREATE_TARGET`: the graphics device was lost; draw on a new target.
const RECREATE_TARGET: i32 = 0x8899000C_u32 as i32;

/// What happened when drawing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drawn {
    Done,
    /// The device was lost: build the renderer again.
    Recreate,
}

pub struct Renderer {
    target: ID2D1RenderTarget,
    window: Option<ID2D1HwndRenderTarget>,
    factory: ID2D1Factory,
    brush: ID2D1SolidColorBrush,
    stroke: ID2D1StrokeStyle,
    text: TextEngine,
    icons: HashMap<&'static str, ID2D1PathGeometry>,
}

fn rect_f(r: Rect) -> D2D_RECT_F {
    D2D_RECT_F {
        left: r.x,
        top: r.y,
        right: r.right(),
        bottom: r.bottom(),
    }
}

fn color_f(c: Color) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: f32::from(c.r) / 255.0,
        g: f32::from(c.g) / 255.0,
        b: f32::from(c.b) / 255.0,
        a: f32::from(c.a) / 255.0,
    }
}

fn rounded(r: Rect, radius: f32) -> D2D1_ROUNDED_RECT {
    let radius = radius.min(r.w / 2.0).min(r.h / 2.0).max(0.0);
    D2D1_ROUNDED_RECT {
        rect: rect_f(r),
        radiusX: radius,
        radiusY: radius,
    }
}

fn properties(dpi: f32) -> D2D1_RENDER_TARGET_PROPERTIES {
    D2D1_RENDER_TARGET_PROPERTIES {
        r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: dpi,
        dpiY: dpi,
        usage: D2D1_RENDER_TARGET_USAGE_NONE,
        minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
    }
}

fn factory() -> Result<ID2D1Factory, String> {
    // SAFETY: creates the factory of the system.
    unsafe { D2D1CreateFactory::<ID2D1Factory>(D2D1_FACTORY_TYPE_SINGLE_THREADED, None) }
        .map_err(|e| format!("no se pudo iniciar Direct2D: {e}"))
}

impl Renderer {
    fn with_target(
        factory: ID2D1Factory,
        target: ID2D1RenderTarget,
        window: Option<ID2D1HwndRenderTarget>,
    ) -> Result<Self, String> {
        let black = color_f(Color::rgb(0, 0, 0));
        // SAFETY: plain creations on the live target and factory.
        let (brush, stroke) = unsafe {
            let brush = target
                .CreateSolidColorBrush(&black, None)
                .map_err(|e| e.to_string())?;
            let style = D2D1_STROKE_STYLE_PROPERTIES {
                startCap: D2D1_CAP_STYLE_ROUND,
                endCap: D2D1_CAP_STYLE_ROUND,
                dashCap: D2D1_CAP_STYLE_ROUND,
                lineJoin: D2D1_LINE_JOIN_ROUND,
                miterLimit: 10.0,
                dashStyle: D2D1_DASH_STYLE_SOLID,
                dashOffset: 0.0,
            };
            let stroke = factory
                .CreateStrokeStyle(&style, None)
                .map_err(|e| e.to_string())?;
            (brush, stroke)
        };
        Ok(Self {
            target,
            window,
            factory,
            brush,
            stroke,
            text: TextEngine::new()?,
            icons: HashMap::new(),
        })
    }

    /// A renderer for a window, `width` x `height` in pixels at `dpi`.
    pub fn for_window(hwnd: HWND, width: u32, height: u32, dpi: f32) -> Result<Self, String> {
        let factory = factory()?;
        let window_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
            hwnd,
            pixelSize: D2D_SIZE_U {
                width: width.max(1),
                height: height.max(1),
            },
            presentOptions: D2D1_PRESENT_OPTIONS_NONE,
        };
        // SAFETY: the properties are live locals and the window handle is valid.
        let window = unsafe { factory.CreateHwndRenderTarget(&properties(dpi), &window_props) }
            .map_err(|e| format!("no se pudo crear la superficie de dibujo: {e}"))?;
        let target: ID2D1RenderTarget = window.cast().map_err(|e| e.to_string())?;
        Self::with_target(factory, target, Some(window))
    }

    /// The same drawing on a WIC bitmap (see `render_png`).
    fn for_bitmap(
        bitmap: &windows::Win32::Graphics::Imaging::IWICBitmap,
        dpi: f32,
    ) -> Result<Self, String> {
        let factory = factory()?;
        // SAFETY: the bitmap is live and the properties a local.
        let target = unsafe { factory.CreateWicBitmapRenderTarget(bitmap, &properties(dpi)) }
            .map_err(|e| format!("no se pudo dibujar en la imagen: {e}"))?;
        Self::with_target(factory, target, None)
    }

    pub fn text(&self) -> &TextEngine {
        &self.text
    }

    pub fn resize(&self, width: u32, height: u32) {
        if let Some(window) = &self.window {
            let size = D2D_SIZE_U {
                width: width.max(1),
                height: height.max(1),
            };
            // SAFETY: the size is a live local.
            let _ = unsafe { window.Resize(&size) };
        }
    }

    pub fn set_dpi(&self, dpi: f32) {
        // SAFETY: a plain setter.
        unsafe { self.target.SetDpi(dpi, dpi) };
    }

    /// The outline of an icon as a Direct2D geometry, built once.
    fn geometry(&mut self, name: &'static str) -> Option<ID2D1PathGeometry> {
        if let Some(found) = self.icons.get(name) {
            return Some(found.clone());
        }
        let segments = icon(name)?;
        // SAFETY: the sink is opened, filled with points that are live locals,
        // and closed before the geometry is used.
        let geometry = unsafe {
            let geometry = self.factory.CreatePathGeometry().ok()?;
            let sink = geometry.Open().ok()?;
            let mut open = false;
            for seg in segments {
                match *seg {
                    Seg::Move(x, y) => {
                        if open {
                            sink.EndFigure(D2D1_FIGURE_END_OPEN);
                        }
                        sink.BeginFigure(Vector2 { X: x, Y: y }, D2D1_FIGURE_BEGIN_HOLLOW);
                        open = true;
                    }
                    Seg::Line(x, y) => sink.AddLine(Vector2 { X: x, Y: y }),
                    Seg::Cubic(a, b, c, d, x, y) => sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                        point1: Vector2 { X: a, Y: b },
                        point2: Vector2 { X: c, Y: d },
                        point3: Vector2 { X: x, Y: y },
                    }),
                    Seg::Close => {
                        if open {
                            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                            open = false;
                        }
                    }
                }
            }
            if open {
                sink.EndFigure(D2D1_FIGURE_END_OPEN);
            }
            sink.Close().ok()?;
            geometry
        };
        self.icons.insert(name, geometry.clone());
        Some(geometry)
    }

    fn paint(&self, color: Color) {
        // SAFETY: a plain setter on the live brush.
        unsafe { self.brush.SetColor(&color_f(color)) };
    }

    /// Draws a whole picture.
    pub fn draw(&mut self, list: &DrawList) -> Result<Drawn, String> {
        // SAFETY: every call is on the live target, brush and styles, with
        // arguments that are live locals; clips are pushed and popped in pairs.
        unsafe {
            self.target.BeginDraw();
            self.target.Clear(Some(&color_f(list.background)));
            let mut clips = 0usize;
            for op in &list.ops {
                match op {
                    Op::Fill {
                        rect,
                        radius,
                        color,
                    } => {
                        self.paint(*color);
                        self.target
                            .FillRoundedRectangle(&rounded(*rect, *radius), &self.brush);
                    }
                    Op::Stroke {
                        rect,
                        radius,
                        width,
                        color,
                    } => {
                        self.paint(*color);
                        let inner = rect.inset(width / 2.0);
                        self.target.DrawRoundedRectangle(
                            &rounded(inner, (radius - width / 2.0).max(0.0)),
                            &self.brush,
                            *width,
                            None,
                        );
                    }
                    Op::Line {
                        x1,
                        y1,
                        x2,
                        y2,
                        width,
                        color,
                    } => {
                        self.paint(*color);
                        self.target.DrawLine(
                            Vector2 { X: *x1, Y: *y1 },
                            Vector2 { X: *x2, Y: *y2 },
                            &self.brush,
                            *width,
                            None,
                        );
                    }
                    Op::Text {
                        x,
                        y,
                        text,
                        style,
                        max_width,
                    } => {
                        if let Ok(layout) = self.text.layout(text, style, *max_width) {
                            self.paint(style.color);
                            self.target.DrawTextLayout(
                                Vector2 { X: *x, Y: *y },
                                &layout,
                                &self.brush,
                                D2D1_DRAW_TEXT_OPTIONS_NONE,
                            );
                        }
                    }
                    Op::Icon {
                        name,
                        x,
                        y,
                        size,
                        color,
                    } => {
                        if let Some(geometry) = self.geometry(name) {
                            self.paint(*color);
                            let scale = size / 24.0;
                            self.target.SetTransform(&Matrix3x2 {
                                M11: scale,
                                M12: 0.0,
                                M21: 0.0,
                                M22: scale,
                                M31: *x,
                                M32: *y,
                            });
                            self.target
                                .DrawGeometry(&geometry, &self.brush, 2.0, &self.stroke);
                            self.target.SetTransform(&Matrix3x2::identity());
                        }
                    }
                    Op::Clip { rect } => {
                        self.target
                            .PushAxisAlignedClip(&rect_f(*rect), D2D1_ANTIALIAS_MODE_ALIASED);
                        clips += 1;
                    }
                    Op::Unclip => {
                        if clips > 0 {
                            self.target.PopAxisAlignedClip();
                            clips -= 1;
                        }
                    }
                }
            }
            for _ in 0..clips {
                self.target.PopAxisAlignedClip();
            }
            match self.target.EndDraw(None, None) {
                Ok(()) => Ok(Drawn::Done),
                Err(error) if error.code().0 == RECREATE_TARGET => Ok(Drawn::Recreate),
                Err(error) => Err(format!("fallo al dibujar: {error}")),
            }
        }
    }
}
