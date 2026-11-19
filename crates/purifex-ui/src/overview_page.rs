//! The general state: four readings (DPC, drivers, what limits the games, the
//! graphics card), the leads of the last game, the console of events and the
//! principles. Everything shown was decided in the domain (`Overview`).

use purifex_domain::draw::Rect;
use purifex_domain::overview::Overview;
use purifex_domain::view::Model;

use crate::canvas::Canvas;
use crate::overview_cards::{card_devices, card_dpc, card_gpu, card_limiter};
use crate::overview_panels::{console, console_height, leads, principles, principles_height};
use crate::page;

/// Draws the page in `area` and returns the height it used.
pub fn draw(c: &mut Canvas, m: &Model, area: Rect) -> f32 {
    let mut y = area.y;
    y += page::head(
        c,
        m,
        area,
        "Estado general",
        "Escaneo de dispositivos · grabación de partidas",
        "Estado de tu PC",
        "Lo que hay instalado y lo que limitó tu última partida. Cada cifra sale de una medición real hecha en este equipo.",
    );
    if let Some(error) = &m.error {
        y += page::error_box(
            c,
            Rect::new(area.x, y, area.w, 0.0).with_width(area.w),
            error,
        );
    }
    let Some(o) = &m.overview else {
        y += if m.error.is_none() {
            page::reading(c, Rect::new(area.x, y, area.w, 60.0), "Leyendo el equipo…")
        } else {
            0.0
        };
        return y - area.y;
    };
    y += cards(c, o, area, y);
    y += leads(c, Rect::new(area.x, y, area.w, 0.0).with_width(area.w), o);
    y += console_and_principles(c, o, area, y);
    y - area.y
}

/// The four cards: in a row, or two by two when the window is narrow. Returns their height.
fn cards(c: &mut Canvas, o: &Overview, area: Rect, y: f32) -> f32 {
    let columns = if area.w < 820.0 { 2 } else { 4 };
    let gap = 16.0;
    let card_w = (area.w - gap * (columns as f32 - 1.0)) / columns as f32;
    let card_h = 186.0;
    let draws: [fn(&mut Canvas, Rect, &Overview); 4] =
        [card_dpc, card_devices, card_limiter, card_gpu];
    for (i, draw_card) in draws.iter().enumerate() {
        let (col, row) = (i % columns, i / columns);
        let card = Rect::new(
            area.x + col as f32 * (card_w + gap),
            y + row as f32 * (card_h + gap),
            card_w,
            card_h,
        );
        // Each card appears a moment after the one before.
        c.reveal(60 * (i as u64 + 1));
        draw_card(c, card, o);
    }
    c.reveal(0);
    4usize.div_ceil(columns) as f32 * (card_h + gap) + 4.0
}

/// The console of events and the principles, side by side. Returns their height.
fn console_and_principles(c: &mut Canvas, o: &Overview, area: Rect, y: f32) -> f32 {
    let gap = 16.0;
    let side = (area.w * 0.32).clamp(280.0, 420.0);
    let console_w = area.w - side - gap;
    let h = console_height(c, console_w, &o.console)
        .max(principles_height(c, side))
        .max(230.0);
    console(
        c,
        Rect::new(area.x, y, console_w, h),
        &o.console,
        &o.scanned_on.to_string(),
    );
    principles(c, Rect::new(area.x + console_w + gap, y, side, h));
    h + 24.0
}

trait WithWidth {
    fn with_width(self, w: f32) -> Self;
}

impl WithWidth for Rect {
    fn with_width(self, w: f32) -> Self {
        Self { w, ..self }
    }
}

#[cfg(test)]
mod tests {
    use purifex_domain::analysis::Analysis;
    use purifex_domain::date::Date;
    use purifex_domain::device::{Device, InstalledDriver};
    use purifex_domain::draw::Op;
    use purifex_domain::report::{ScanReport, Scope};
    use purifex_domain::session_csv::parse;
    use purifex_domain::version::DriverVersion;

    use super::*;
    use crate::canvas::testing::FakeText;

    fn overview() -> Overview {
        let gpu = Device {
            instance_id: r"PCI\A".into(),
            class: "Display".into(),
            name: "NVIDIA GeForce GTX 1660 Ti".into(),
            driver: Some(InstalledDriver {
                provider: Some("NVIDIA".into()),
                version: DriverVersion::parse("32.0.16.2124"),
                date: Date::new(2026, 11, 7),
                inf: None,
            }),
            ..Device::default()
        };
        let scan = ScanReport::build(
            vec![gpu],
            Scope::Hardware,
            Date::new(2026, 11, 18).unwrap(),
            211,
        );
        let csv = "t_s,cpu_total,cpu_max_core,dpc_interrupt,groups,game_cpu,game_mb\n1.0,40.0,99.0,2.0,league_client=16.0,13.0,900\n200.0,40.0,98.0,2.4,league_client=17.0,13.0,900\n";
        let analysis = Analysis::of("lol-20261118-145915", parse(csv).unwrap()).unwrap();
        Overview::build(&scan, Some(&analysis))
    }

    fn texts(m: &Model, width: f32) -> (Vec<String>, f32) {
        let mut c = Canvas::new(&FakeText, None);
        let h = draw(&mut c, m, Rect::new(296.0, 80.0, width, 700.0));
        let (list, _) = c.finish();
        (
            list.ops
                .iter()
                .filter_map(|op| {
                    if let Op::Text { text, .. } = op {
                        Some(text.clone())
                    } else {
                        None
                    }
                })
                .collect(),
            h,
        )
    }

    #[test]
    fn with_data_it_shows_the_four_readings_the_leads_the_console_and_the_principles() {
        let m = Model {
            overview: Some(overview()),
            ..Model::default()
        };
        let (texts, height) = texts(&m, 1080.0);
        // Text is wrapped into lines, so a phrase is looked for in all of it.
        let all = texts.join(" ");
        for expected in [
            "Estado de tu PC",
            "NVIDIA GeForce GTX 1660 Ti",
            "dispositivos",
            "PRINCIPIOS",
            "PISTAS DE LA ÚLTIMA PARTIDA",
            "Solo lectura",
        ] {
            assert!(all.contains(expected), "{expected}: {all}");
        }
        assert!(texts.iter().any(|t| t == "2.4"), "the DPC of the game");
        assert!(texts.iter().any(|t| t == "[PISTA]"));
        assert!(height > 600.0);
    }

    #[test]
    fn before_the_first_reading_it_says_it_is_reading_and_makes_up_nothing() {
        let (texts, _) = texts(
            &Model {
                busy: true,
                ..Model::default()
            },
            1080.0,
        );
        assert!(texts.iter().any(|t| t == "Leyendo el equipo…"));
        assert!(!texts.iter().any(|t| t == "dispositivos"));
    }

    #[test]
    fn a_failure_is_shown_even_when_nothing_could_be_read() {
        let m = Model {
            error: Some("estado general: sin acceso".into()),
            ..Model::default()
        };
        let (texts, _) = texts(&m, 1080.0);
        assert!(texts.iter().any(|t| t == "No se pudo leer"));
        assert!(
            !texts.iter().any(|t| t == "Leyendo el equipo…"),
            "a failure is not a wait"
        );
    }

    #[test]
    fn with_no_recording_the_readings_say_so() {
        let scan = ScanReport::build(vec![], Scope::Hardware, Date::new(2026, 11, 18).unwrap(), 5);
        let m = Model {
            overview: Some(Overview::build(&scan, None)),
            ..Model::default()
        };
        let (texts, _) = texts(&m, 1080.0);
        let all = texts.join(" ");
        assert!(all.contains("Sin datos de partida"), "{all}");
        assert!(all.contains("Sin partidas grabadas"), "{all}");
    }

    #[test]
    fn a_narrow_window_stacks_the_cards_two_by_two_and_everything_stays_inside() {
        let m = Model {
            overview: Some(overview()),
            ..Model::default()
        };
        let mut c = Canvas::new(&FakeText, None);
        let area = Rect::new(296.0, 80.0, 700.0, 700.0);
        draw(&mut c, &m, area);
        let (list, _) = c.finish();
        for op in &list.ops {
            if let Op::Fill { rect, .. } | Op::Stroke { rect, .. } = op {
                assert!(
                    rect.right() <= area.right() + 4.0,
                    "{rect:?} passes the right edge"
                );
            }
        }
    }
}
