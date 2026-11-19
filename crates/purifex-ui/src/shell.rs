//! The frame of the window: the top bar with its chips and the side bar with the
//! engine card, the navigation and the state of the drivers.

use purifex_domain::draw::Rect;
use purifex_domain::view::{Action, Model, Page};

use crate::canvas::Canvas;
use crate::theme::{self, Tone, SIDEBAR_W, TOPBAR_H};
use crate::widgets::{badge, dot, icon_box, progress};

/// A chip of the top bar: an optional dot or icon, then the text (the value in
/// bold). Returns its width.
fn chip(
    c: &mut Canvas,
    x: f32,
    label: &str,
    value: &str,
    icon: Option<&'static str>,
    green_dot: bool,
    ok: bool,
) -> f32 {
    let (text_color, border, fill) = if ok {
        (
            theme::ACCENT,
            theme::ACCENT.with_alpha(0.35),
            theme::ACCENT.with_alpha(0.06),
        )
    } else {
        (theme::TEXT_2, theme::LINE, theme::PANEL)
    };
    let plain = theme::mono(12.0, text_color);
    let bold = theme::mono(12.0, theme::TEXT).bold();
    let lead = if icon.is_some() || green_dot {
        22.0
    } else {
        0.0
    };
    let gap = if value.is_empty() {
        0.0
    } else {
        c.width(" ", &plain)
    };
    let width = 24.0 + lead + c.width(label, &plain) + gap + c.width(value, &bold);
    let rect = Rect::new(x, 12.0, width, 28.0);
    c.fill(rect, 6.0, fill);
    c.stroke(rect, 6.0, 1.0, border);
    let mut cursor = x + 12.0;
    if green_dot {
        dot(c, cursor + 3.5, 26.0, Tone::Green);
    } else if let Some(name) = icon {
        c.icon(name, cursor, 19.0, 14.0, text_color);
    }
    cursor += lead;
    cursor += c.text_in(cursor, rect, label, plain) + gap;
    c.text_in(cursor, rect, value, bold);
    width
}

pub fn topbar(c: &mut Canvas, m: &Model, width: f32) {
    c.fill(Rect::new(0.0, 0.0, width, TOPBAR_H), 0.0, theme::BG);
    c.line(0.0, TOPBAR_H - 0.5, width, TOPBAR_H - 0.5, theme::LINE);
    // The brand.
    let mark = Rect::new(16.0, 14.0, 24.0, 24.0);
    c.stroke(mark, 6.0, 1.0, theme::ACCENT);
    c.icon("zap", 20.0, 18.0, 16.0, theme::ACCENT);
    let brand = theme::mono(13.0, theme::TEXT).bold().tracked(1.0);
    let bar = Rect::new(0.0, 0.0, 10.0, TOPBAR_H);
    let used = c.text_in(50.0, bar, "PURIFEX", brand);
    c.text_in(
        50.0 + used + 8.0,
        bar,
        "// DIAGNÓSTICO",
        theme::mono(12.0, theme::ACCENT).bold(),
    );
    // The chips, aligned with the content of the pages.
    let mut x = SIDEBAR_W;
    x += chip(c, x, "Solo lectura", "", None, true, true) + 12.0;
    x += chip(
        c,
        x,
        "Sin telemetría",
        "",
        Some("shield-check"),
        false,
        false,
    ) + 12.0;
    if let Some(overview) = &m.overview {
        chip(
            c,
            x,
            "Escaneo",
            &format!("{} ms", overview.scan_ms),
            Some("activity"),
            false,
            false,
        );
    }
    // Where the data is kept.
    let folder = r"%LOCALAPPDATA%\Purifex";
    let style = theme::mono(12.0, theme::TEXT_2);
    let w = 24.0 + 22.0 + c.width(folder, &style);
    let rect = Rect::new(width - 16.0 - w, 12.0, w, 28.0);
    c.fill(rect, 6.0, theme::PANEL);
    c.stroke(rect, 6.0, 1.0, theme::LINE);
    c.icon("folder-open", rect.x + 12.0, 19.0, 14.0, theme::TEXT_2);
    c.text_in(rect.x + 34.0, rect, folder, style);
}

/// A row of the card of the side bar: a label on the left and a value on the right.
fn row(
    c: &mut Canvas,
    x: f32,
    right: f32,
    y: f32,
    label: &str,
    value: &str,
    dot_tone: Option<Tone>,
) {
    let bar = Rect::new(x, y, right - x, 18.0);
    let mut label_x = x;
    if let Some(tone) = dot_tone {
        dot(c, x + 3.5, y + 9.0, tone);
        label_x += 14.0;
    }
    c.text_in(label_x, bar, label, theme::mono(11.0, theme::TEXT_3));
    c.text_right(right, bar, value, theme::mono(11.0, theme::TEXT).bold());
}

/// An entry of the navigation.
struct NavItem<'a> {
    icon: &'static str,
    label: &'a str,
    /// `None` for one that is not available yet: it is shown dimmed and cannot be clicked.
    page: Option<Page>,
    badge: Option<(String, Tone)>,
}

fn nav_item(c: &mut Canvas, rect: Rect, item: &NavItem, active: bool) {
    let soon = item.page.is_none();
    let hover = if item.page.is_some() {
        c.hover(rect)
    } else {
        0.0
    };
    if active {
        c.fill(rect, 8.0, theme::ACCENT.with_alpha(0.08));
        c.stroke(rect, 8.0, 1.0, theme::ACCENT.with_alpha(0.45));
    } else if hover > 0.01 {
        c.fill(rect, 8.0, theme::TEXT.with_alpha(0.03 * hover));
    }
    let dim = if soon { 0.5 } else { 1.0 };
    let icon_color = if active {
        theme::ACCENT
    } else {
        theme::TEXT_2.with_alpha(dim)
    };
    c.icon(item.icon, rect.x + 12.0, rect.y + 11.0, 18.0, icon_color);
    let style = if active {
        theme::mono(13.0, theme::TEXT).bold()
    } else {
        theme::mono(13.0, theme::TEXT_2.mix(theme::TEXT, hover).with_alpha(dim))
    };
    c.text_in(rect.x + 42.0, rect, item.label, style);
    if soon {
        badge(c, rect.right() - 12.0, rect, "pronto", Tone::Gray);
    } else if let Some((text, tone)) = &item.badge {
        badge(c, rect.right() - 12.0, rect, text, *tone);
    }
    if let Some(page) = item.page {
        c.hit(rect, Action::Go(page));
    }
}

pub fn sidebar(c: &mut Canvas, m: &Model, height: f32) {
    let area = Rect::new(0.0, TOPBAR_H, SIDEBAR_W, height - TOPBAR_H);
    c.fill(area, 0.0, theme::BG_SIDE);
    c.line(
        SIDEBAR_W - 0.5,
        TOPBAR_H,
        SIDEBAR_W - 0.5,
        height,
        theme::LINE,
    );

    // The engine card.
    let card = Rect::new(12.0, TOPBAR_H + 16.0, 240.0, 112.0);
    c.fill(card, 8.0, theme::PANEL);
    c.stroke(card, 8.0, 1.0, theme::LINE);
    icon_box(
        c,
        card.x + 14.0,
        card.y + 14.0,
        36.0,
        "layout-dashboard",
        Tone::Green,
    );
    let head = Rect::new(card.x + 62.0, card.y + 14.0, 10.0, 36.0);
    c.text_in(
        head.x,
        head,
        "Purifex",
        theme::sans(14.0, theme::TEXT).bold(),
    );
    badge(c, card.right() - 14.0, head, "activo", Tone::Green);
    let devices = m
        .overview
        .as_ref()
        .map_or("–".to_string(), |o| o.devices.hardware.to_string());
    let sessions = m
        .sessions
        .as_ref()
        .map_or("–".to_string(), |s| s.len().to_string());
    row(
        c,
        card.x + 14.0,
        card.right() - 14.0,
        card.y + 62.0,
        "Dispositivos",
        &devices,
        None,
    );
    row(
        c,
        card.x + 14.0,
        card.right() - 14.0,
        card.y + 82.0,
        "Grabaciones",
        &sessions,
        None,
    );

    // Navigation.
    let mut y = card.bottom() + 24.0;
    c.text(20.0, y, "NAVEGACIÓN", theme::eyebrow(theme::TEXT_3));
    y += 24.0;
    let attention = m
        .overview
        .as_ref()
        .map_or(0, |o| o.devices.faults + o.devices.missing_driver);
    let drivers_badge = (attention > 0).then(|| (attention.to_string(), Tone::Red));
    let sessions_badge = m
        .sessions
        .as_ref()
        .map(|s| (s.len().to_string(), Tone::Gray));
    // The number of fixes on offer, once they have been read.
    let fixes_badge = m
        .fixes
        .as_ref()
        .filter(|f| !f.is_empty())
        .map(|f| (f.len().to_string(), Tone::Amber));
    let items = [
        NavItem {
            icon: "gauge",
            label: "Estado general",
            page: Some(Page::Overview),
            badge: None,
        },
        NavItem {
            icon: "layers",
            label: "Drivers",
            page: Some(Page::Drivers),
            badge: drivers_badge,
        },
        NavItem {
            icon: "gamepad-2",
            label: "Partidas",
            page: Some(Page::Sessions),
            badge: sessions_badge,
        },
        NavItem {
            icon: "sliders-horizontal",
            label: "Arreglos",
            page: Some(Page::Fixes),
            badge: fixes_badge,
        },
    ];
    for item in &items {
        let rect = Rect::new(12.0, y, 240.0, 40.0);
        nav_item(c, rect, item, item.page == Some(m.page));
        y += 44.0;
    }

    // The state of the drivers, at the bottom.
    let foot = Rect::new(12.0, height - 12.0 - 88.0, 240.0, 88.0);
    if foot.y > y {
        c.fill(foot, 8.0, theme::PANEL);
        c.stroke(foot, 8.0, 1.0, theme::LINE);
        row(
            c,
            foot.x + 14.0,
            foot.right() - 14.0,
            foot.y + 14.0,
            "Modo",
            "Solo lectura",
            Some(Tone::Green),
        );
        let health = m
            .overview
            .as_ref()
            .filter(|o| o.devices.hardware > 0)
            .map(|o| o.devices.healthy as f32 * 100.0 / o.devices.hardware as f32);
        let text = health.map_or("–".to_string(), |h| format!("{h:.1} %"));
        row(
            c,
            foot.x + 14.0,
            foot.right() - 14.0,
            foot.y + 38.0,
            "Salud de drivers",
            &text,
            None,
        );
        // The bar fills the first time the figure is known.
        let shown = c.value("health", health.unwrap_or(0.0), 800);
        progress(
            c,
            Rect::new(foot.x + 14.0, foot.y + 66.0, foot.w - 28.0, 4.0),
            shown,
        );
    }
}

#[cfg(test)]
mod tests {
    use purifex_domain::draw::Op;

    use super::*;
    use crate::canvas::testing::FakeText;

    fn texts(c: Canvas) -> (Vec<String>, Vec<crate::canvas::Hit>) {
        let (list, hits) = c.finish();
        let texts = list
            .ops
            .iter()
            .filter_map(|op| match op {
                Op::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect();
        (texts, hits)
    }

    #[test]
    fn the_sidebar_has_a_click_place_for_each_page() {
        let mut c = Canvas::new(&FakeText, None);
        sidebar(&mut c, &Model::default(), 900.0);
        let (texts, hits) = texts(c);
        let actions: Vec<_> = hits.iter().map(|h| h.action.clone()).collect();
        assert_eq!(
            actions,
            [
                Action::Go(Page::Overview),
                Action::Go(Page::Drivers),
                Action::Go(Page::Sessions),
                Action::Go(Page::Fixes)
            ]
        );
        for expected in [
            "Estado general",
            "Drivers",
            "Partidas",
            "Arreglos",
            "NAVEGACIÓN",
        ] {
            assert!(texts.iter().any(|t| t == expected), "{expected}");
        }
        // Nothing is shown as "coming soon": the history is part of the fixes page.
        assert!(!texts.iter().any(|t| t == "PRONTO" || t == "Historial"));
    }

    #[test]
    fn what_has_not_been_read_shows_a_dash_and_not_a_made_up_number() {
        let mut c = Canvas::new(&FakeText, None);
        sidebar(&mut c, &Model::default(), 900.0);
        let (texts, _) = texts(c);
        assert!(texts.iter().filter(|t| *t == "–").count() >= 3, "{texts:?}");
    }

    #[test]
    fn the_top_bar_says_it_is_read_only_and_has_no_telemetry() {
        let mut c = Canvas::new(&FakeText, None);
        topbar(&mut c, &Model::default(), 1440.0);
        let (texts, hits) = texts(c);
        for expected in ["PURIFEX", "Solo lectura", "Sin telemetría"] {
            assert!(texts.iter().any(|t| t == expected), "{expected}");
        }
        assert!(hits.is_empty());
    }

    #[test]
    fn the_footer_of_the_sidebar_gives_way_on_a_short_window() {
        let mut c = Canvas::new(&FakeText, None);
        sidebar(&mut c, &Model::default(), 420.0);
        let (texts, _) = texts(c);
        assert!(!texts.iter().any(|t| t == "Salud de drivers"));
    }
}
