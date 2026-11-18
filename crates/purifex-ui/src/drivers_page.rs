//! The drivers page: the search, the filters with their counts, and the table of
//! devices with the driver each one runs. The rows and their order come from the
//! domain (`Model::visible_drivers`); this only draws them.

use purifex_domain::device_category::Category;
use purifex_domain::draw::Rect;
use purifex_domain::drivers_view::{DriverRow, RowState};
use purifex_domain::view::{Action, DriverFilter, Model};

use crate::canvas::Canvas;
use crate::page;
use crate::theme::{self, Tone};
use crate::widgets::{dot, icon_box, panel, tag};

const ROW_H: f32 = 64.0;

fn category_icon(category: Category) -> &'static str {
    match category {
        Category::Gpu => "microchip",
        Category::Audio => "audio-lines",
        Category::Network => "router",
        Category::Input => "mouse",
        Category::Bluetooth => "bluetooth",
        Category::Storage => "hard-drive",
        Category::Monitor => "monitor",
        Category::Camera => "video",
        Category::System => "cpu",
        Category::Other => "package",
    }
}

/// The label, color and icon of the tag of a row.
fn state_tag(state: RowState) -> (String, Tone, Option<&'static str>) {
    match state {
        RowState::Healthy => ("Correcto".into(), Tone::Green, Some("circle-check")),
        RowState::Windows => ("Windows".into(), Tone::Gray, Some("circle-check")),
        RowState::NoDriverData => ("Sin datos de driver".into(), Tone::Gray, None),
        RowState::Disabled => ("Desactivado".into(), Tone::Gray, None),
        RowState::MissingDriver => ("Sin driver".into(), Tone::Red, Some("circle-alert")),
        RowState::Fault(code) => (
            format!("Fallo · código {code}"),
            Tone::Red,
            Some("triangle-alert"),
        ),
    }
}

fn search_box(c: &mut Canvas, m: &Model, area: Rect, y: f32) -> f32 {
    let outer = Rect::new(area.x, y, area.w, 76.0);
    panel(c, outer);
    let input = Rect::new(outer.x + 14.0, outer.y + 14.0, outer.w - 28.0, 48.0);
    c.fill(input, 8.0, theme::INPUT);
    c.stroke(
        input,
        8.0,
        1.0,
        if c.search_focus {
            theme::ACCENT
        } else {
            theme::LINE
        },
    );
    c.icon(
        "search",
        input.x + 14.0,
        input.y + 15.0,
        18.0,
        theme::TEXT_3,
    );
    let text_x = input.x + 46.0;
    let style = theme::mono(13.0, theme::TEXT);
    if m.query.is_empty() {
        c.text_in(
            text_x,
            input,
            "Filtrar por nombre, VEN_/DEV_, proveedor…",
            theme::mono(13.0, theme::TEXT_3),
        );
    } else {
        c.text_in(text_x, input, &m.query, style);
    }
    if c.search_focus {
        let x = text_x + c.width(&m.query, &style) + 1.0;
        c.line(x, input.y + 14.0, x, input.bottom() - 14.0, theme::ACCENT);
    }
    // The shortcut hint, at the right.
    let hint = "Ctrl K";
    let hint_style = theme::mono(11.0, theme::TEXT_3);
    let w = c.width(hint, &hint_style) + 18.0;
    let key = Rect::new(input.right() - 14.0 - w, input.y + 12.0, w, 24.0);
    c.stroke(key, 4.0, 1.0, theme::LINE);
    c.text_in(key.x + 9.0, key, hint, hint_style);
    c.set_search_box(input);
    outer.h + 16.0
}

/// A filter chip; returns its width.
/// What a filter chip says and does.
struct Chip<'a> {
    label: &'a str,
    count: usize,
    active: bool,
    /// The chip of what needs attention: red, with a dot.
    red: bool,
    action: Action,
}

fn chip(c: &mut Canvas, x: f32, y: f32, chip: &Chip) -> f32 {
    let Chip {
        label,
        count,
        active,
        red,
        ref action,
    } = *chip;
    let style = theme::mono(
        12.5,
        if active {
            theme::ON_ACCENT
        } else if red {
            Tone::Red.color().with_alpha(0.85)
        } else {
            theme::TEXT_2
        },
    );
    let style = if active { style.bold() } else { style };
    let count_text = count.to_string();
    let count_style = theme::mono(
        11.0,
        if active {
            theme::ON_ACCENT
        } else {
            theme::TEXT_2
        },
    );
    let lead = if red { 16.0 } else { 0.0 };
    let width =
        28.0 + lead + c.width(label, &style) + 10.0 + c.width(&count_text, &count_style) + 12.0;
    let rect = Rect::new(x, y, width, 38.0);
    let hovered = c.hovered(rect);
    if active {
        c.fill(rect, 6.0, theme::ACCENT_2);
    } else if red {
        c.fill(rect, 6.0, Tone::Red.fill());
        c.stroke(rect, 6.0, 1.0, Tone::Red.border());
    } else {
        c.fill(rect, 6.0, theme::PANEL);
        c.stroke(
            rect,
            6.0,
            1.0,
            if hovered { theme::TEXT_3 } else { theme::LINE },
        );
    }
    let mut cursor = x + 14.0;
    if red {
        dot(c, cursor + 3.5, y + 19.0, Tone::Red);
        cursor += lead;
    }
    cursor += c.text_in(cursor, rect, label, style) + 10.0;
    let box_rect = Rect::new(
        cursor - 6.0,
        y + 10.0,
        c.width(&count_text, &count_style) + 12.0,
        18.0,
    );
    c.fill(
        box_rect,
        4.0,
        if active {
            theme::BG.with_alpha(0.18)
        } else {
            theme::TEXT.with_alpha(0.08)
        },
    );
    c.text_in(box_rect.x + 6.0, box_rect, &count_text, count_style);
    c.hit(rect, action.clone());
    width
}

fn filters(c: &mut Canvas, m: &Model, area: Rect, y: f32) -> f32 {
    let Some(view) = &m.drivers else { return 0.0 };
    let mut entries: Vec<(String, usize, DriverFilter, bool)> = vec![(
        "Todos los dispositivos".into(),
        view.total,
        DriverFilter::All,
        false,
    )];
    if view.attention > 0 {
        entries.push((
            "Con problema".into(),
            view.attention,
            DriverFilter::Attention,
            true,
        ));
    }
    for category in &view.categories {
        entries.push((
            category.label.into(),
            category.count,
            DriverFilter::Category(category.category),
            false,
        ));
    }
    let (mut x, mut row) = (area.x, 0.0);
    for (label, count, filter, red) in entries {
        // Measure the chip first by drawing it off to the side only when it fits.
        let probe = theme::mono(12.5, theme::TEXT_2);
        let width = 28.0
            + if red { 16.0 } else { 0.0 }
            + c.width(&label, &probe)
            + 10.0
            + c.width(&count.to_string(), &theme::mono(11.0, theme::TEXT_2))
            + 12.0;
        if x + width > area.right() && x > area.x {
            x = area.x;
            row += 48.0;
        }
        let chip_spec = Chip {
            label: &label,
            count,
            active: m.filter == filter,
            red,
            action: Action::SetFilter(filter),
        };
        x += chip(c, x, y + row, &chip_spec) + 10.0;
    }
    row + 38.0 + 16.0
}

fn table(c: &mut Canvas, m: &Model, area: Rect, y: f32) -> f32 {
    let rows = m.visible_drivers();
    let height = 48.0 + (rows.len().max(1) as f32) * ROW_H;
    let rect = Rect::new(area.x, y, area.w, height);
    panel(c, rect);
    // The columns: the device takes what the others leave.
    let (class_w, version_w, state_w) = (190.0, 190.0, 190.0);
    let device_w = (rect.w - class_w - version_w - state_w - 32.0).max(220.0);
    let (class_x, version_x, state_x) = (
        rect.x + 16.0 + device_w,
        rect.x + 16.0 + device_w + class_w,
        rect.x + 16.0 + device_w + class_w + version_w,
    );
    let label = theme::eyebrow(theme::TEXT_3);
    let head = Rect::new(rect.x, rect.y, rect.w, 48.0);
    c.text_in(rect.x + 16.0, head, "DISPOSITIVO Y UBICACIÓN", label);
    c.text_in(class_x, head, "CLASE / PROVEEDOR", label);
    c.text_in(version_x, head, "VERSIÓN DEL DRIVER", label);
    c.text_in(state_x, head, "ESTADO", label);
    c.line(
        rect.x,
        rect.y + 48.0,
        rect.right(),
        rect.y + 48.0,
        theme::LINE,
    );
    if rows.is_empty() {
        c.text(
            rect.x + 16.0,
            rect.y + 48.0 + 22.0,
            "Ningún dispositivo coincide.",
            theme::mono(12.5, theme::TEXT_3),
        );
        return height;
    }
    for (i, row) in rows.iter().enumerate() {
        draw_row(
            c,
            row,
            Rect::new(rect.x, rect.y + 48.0 + i as f32 * ROW_H, rect.w, ROW_H),
            [rect.x + 16.0, class_x, version_x, state_x],
            device_w,
            class_w,
        );
    }
    height
}

fn draw_row(
    c: &mut Canvas,
    row: &DriverRow,
    bar: Rect,
    columns: [f32; 4],
    device_w: f32,
    class_w: f32,
) {
    let [device_x, class_x, version_x, state_x] = columns;
    if c.hovered(bar) {
        c.fill(bar, 0.0, theme::TEXT.with_alpha(0.02));
    }
    c.line(
        bar.x,
        bar.bottom() - 0.5,
        bar.right(),
        bar.bottom() - 0.5,
        theme::LINE_SOFT,
    );
    let (label, tone, icon) = state_tag(row.state());
    let box_tone = match row.state() {
        RowState::MissingDriver | RowState::Fault(_) => Tone::Red,
        RowState::Healthy => Tone::Green,
        _ => Tone::Gray,
    };
    icon_box(
        c,
        device_x,
        bar.y + 12.0,
        40.0,
        category_icon(row.category),
        box_tone,
    );
    let name_x = device_x + 54.0;
    let name_style = theme::sans(13.5, theme::TEXT).bold();
    let mut name_w = device_w - 70.0;
    if row.count > 1 {
        let extra = format!("×{}", row.count);
        let extra_style = theme::mono(11.0, theme::ACCENT);
        let w = c.width(&extra, &extra_style);
        name_w -= w + 8.0;
        let shown = c.width(&row.name, &name_style).min(name_w);
        c.text(name_x + shown + 8.0, bar.y + 13.0, &extra, extra_style);
    }
    c.text_cut(name_x, bar.y + 12.0, &row.name, name_style, name_w);
    c.text_cut(
        name_x,
        bar.y + 34.0,
        &row.instance_id,
        theme::mono(11.0, theme::TEXT_3),
        device_w - 70.0,
    );
    c.text_cut(
        class_x,
        bar.y + 13.0,
        if row.class.is_empty() {
            "–"
        } else {
            &row.class
        },
        theme::mono(12.5, theme::TEXT),
        class_w - 16.0,
    );
    c.text_cut(
        class_x,
        bar.y + 34.0,
        row.provider.as_deref().unwrap_or("–"),
        theme::mono(12.0, theme::TEXT_2),
        class_w - 16.0,
    );
    let version = row.version.map_or("–".to_string(), |v| v.to_string());
    c.text(
        version_x,
        bar.y + 13.0,
        &version,
        theme::mono(12.5, theme::TEXT).bold(),
    );
    let date = row.date.map_or("–".to_string(), |d| d.to_string());
    let date_text = match row.age_days {
        Some(days) if days >= 0 => format!("{date} · {}", age_text(days)),
        _ => date,
    };
    c.text(
        version_x,
        bar.y + 34.0,
        &date_text,
        theme::mono(12.0, theme::TEXT_2),
    );
    tag(c, state_x, bar.y + 21.0, &label, tone, icon);
}

fn age_text(days: i64) -> String {
    match days {
        d if d >= 365 => format!("{:.1} años", d as f64 / 365.25),
        d if d >= 30 => format!("{} meses", d / 30),
        d => format!("{d} d"),
    }
}

/// Draws the page in `area` and returns the height it used.
pub fn draw(c: &mut Canvas, m: &Model, area: Rect) -> f32 {
    let mut y = area.y;
    y += page::head(
        c,
        m,
        area,
        "Escaneo de hardware",
        "SetupAPI · lo mismo que lee el Administrador de dispositivos",
        "Drivers y dispositivos",
        "Cada dispositivo con el driver que usa hoy. Un driver viejo no es un problema por sí solo: aquí solo se muestran hechos.",
    );
    if let Some(error) = &m.error {
        y += page::error_box(c, Rect::new(area.x, y, area.w, 0.0), error);
    }
    if m.drivers.is_none() {
        if m.error.is_none() {
            y += page::reading(
                c,
                Rect::new(area.x, y, area.w, 60.0),
                "Leyendo dispositivos…",
            );
        }
        return y - area.y;
    }
    y += search_box(c, m, area, y);
    y += filters(c, m, area, y);
    y += table(c, m, area, y);
    y - area.y
}

#[cfg(test)]
mod tests {
    use purifex_domain::date::Date;
    use purifex_domain::device::{Device, InstalledDriver};
    use purifex_domain::draw::Op;
    use purifex_domain::drivers_view::DriversView;
    use purifex_domain::report::{ScanReport, Scope};
    use purifex_domain::version::DriverVersion;

    use super::*;
    use crate::canvas::testing::FakeText;

    fn device(id: &str, class: &str, name: &str, provider: &str, problem: Option<u32>) -> Device {
        Device {
            instance_id: id.into(),
            class: class.into(),
            name: name.into(),
            problem,
            driver: Some(InstalledDriver {
                provider: Some(provider.into()),
                version: DriverVersion::parse("1.0"),
                date: Date::new(2026, 1, 1),
                inf: None,
            }),
            ..Device::default()
        }
    }

    fn model() -> Model {
        let report = ScanReport::build(
            vec![
                device(
                    r"PCI\A",
                    "Display",
                    "NVIDIA GeForce GTX 1660 Ti",
                    "NVIDIA",
                    None,
                ),
                device(
                    r"USB\B",
                    "Bluetooth",
                    "Realtek Bluetooth",
                    "Realtek",
                    Some(31),
                ),
                device(r"USB\C", "HIDClass", "Entrada USB", "Microsoft", None),
                device(r"USB\D", "HIDClass", "Entrada USB", "Microsoft", None),
            ],
            Scope::Hardware,
            Date::new(2026, 11, 18).unwrap(),
            5,
        );
        Model {
            page: purifex_domain::view::Page::Drivers,
            drivers: Some(DriversView::build(&report)),
            ..Model::default()
        }
    }

    fn run(m: &Model) -> (String, Vec<crate::canvas::Hit>, Option<Rect>) {
        let mut c = Canvas::new(&FakeText, None);
        draw(&mut c, m, Rect::new(296.0, 80.0, 1000.0, 800.0));
        let search = c.take_search_box();
        let (list, hits) = c.finish();
        let texts: Vec<String> = list
            .ops
            .iter()
            .filter_map(|op| {
                if let Op::Text { text, .. } = op {
                    Some(text.clone())
                } else {
                    None
                }
            })
            .collect();
        (texts.join(" | "), hits, search)
    }

    #[test]
    fn the_table_lists_the_visible_devices_with_the_state_of_each() {
        let (text, _, _) = run(&model());
        for expected in [
            "NVIDIA GeForce GTX 1660 Ti",
            "Realtek Bluetooth",
            "×2",
            "Fallo · código 31",
            "Correcto",
            "Windows",
        ] {
            assert!(text.contains(expected), "{expected}: {text}");
        }
    }

    #[test]
    fn every_filter_is_a_chip_with_its_count_and_can_be_clicked() {
        let (text, hits, _) = run(&model());
        assert!(text.contains("Todos los dispositivos") && text.contains("Con problema"));
        let actions: Vec<Action> = hits.into_iter().map(|h| h.action).collect();
        assert!(actions.contains(&Action::SetFilter(DriverFilter::All)));
        assert!(actions.contains(&Action::SetFilter(DriverFilter::Attention)));
        assert!(actions.contains(&Action::SetFilter(DriverFilter::Category(Category::Gpu))));
        assert!(actions.contains(&Action::Rescan));
    }

    #[test]
    fn the_search_box_tells_where_it_is_so_a_click_can_focus_it() {
        let (_, _, search) = run(&model());
        assert!(search.is_some());
    }

    #[test]
    fn a_search_with_no_match_says_so_instead_of_leaving_an_empty_table() {
        let mut m = model();
        m.query = "zzzz".into();
        let (text, _, _) = run(&m);
        assert!(text.contains("Ningún dispositivo coincide."), "{text}");
        assert!(text.contains("zzzz"), "the search is shown");
    }

    #[test]
    fn before_reading_it_says_so_and_a_failure_is_shown() {
        let (text, _, search) = run(&Model {
            page: purifex_domain::view::Page::Drivers,
            busy: true,
            ..Model::default()
        });
        assert!(text.contains("Leyendo dispositivos…"));
        assert!(search.is_none(), "no search box yet");
        let (text, _, _) = run(&Model {
            error: Some("drivers: sin acceso".into()),
            ..Model::default()
        });
        assert!(text.contains("No se pudo leer") && !text.contains("Leyendo dispositivos…"));
    }

    #[test]
    fn the_chips_wrap_to_a_new_row_in_a_narrow_window() {
        let mut c = Canvas::new(&FakeText, None);
        draw(&mut c, &model(), Rect::new(296.0, 80.0, 420.0, 800.0));
        let (_, hits) = c.finish();
        let ys: Vec<f32> = hits
            .iter()
            .filter(|h| matches!(h.action, Action::SetFilter(_)))
            .map(|h| h.rect.y)
            .collect();
        assert!(ys.iter().any(|y| *y > ys[0]), "{ys:?}");
        for hit in hits
            .iter()
            .filter(|h| matches!(h.action, Action::SetFilter(_)))
        {
            assert!(
                hit.rect.right() <= 296.0 + 420.0 + 1.0,
                "{:?} passes the edge",
                hit.rect
            );
        }
    }
}
