//! The drivers page (stub, replaced next).

use purifex_domain::draw::Rect;
use purifex_domain::view::Model;

use crate::canvas::Canvas;
use crate::page;

pub fn draw(c: &mut Canvas, m: &Model, area: Rect) -> f32 {
    page::head(
        c,
        m,
        area,
        "Escaneo de hardware",
        "SetupAPI",
        "Drivers y dispositivos",
        "Cada dispositivo con el driver que usa hoy.",
    )
}
