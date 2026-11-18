//! The four cards of the overview: DPC, drivers, what limits the games and the
//! graphics card. Each shows what the domain decided in `Overview`.

use purifex_domain::draw::Rect;
use purifex_domain::overview::Overview;

use crate::canvas::Canvas;
use crate::theme::{self, Tone};
use crate::widgets::{dot, icon_box, kpi_card, tag};

/// The ages shown as `11 d`, `2.7 años`.
pub(crate) fn age_text(days: i64) -> String {
    match days {
        d if d < 0 => "–".into(),
        d if d >= 365 => format!("{:.1} años", d as f64 / 365.25),
        d if d >= 30 => format!("{} meses", d / 30),
        d => format!("{d} d"),
    }
}

/// The label of a card, in capitals, at its top left; returns where it ends.
pub(crate) fn card_label(c: &mut Canvas, card: Rect, label: &str) {
    let style = theme::eyebrow(theme::TEXT_3);
    c.paragraph(
        card.x + 20.0,
        card.y + 20.0,
        card.w - 40.0 - 48.0,
        &label.to_uppercase(),
        style,
        Some(2),
    );
}

pub(crate) fn note(c: &mut Canvas, card: Rect, text: &str) {
    let style = theme::mono(11.5, theme::TEXT_2);
    let lines = c.wrap(text, &style, card.w - 40.0, Some(3));
    let step = c.line_height(&style) + 2.0;
    let top = card.bottom() - 18.0 - step * lines.len() as f32;
    for (i, line) in lines.iter().enumerate() {
        c.text_cut(
            card.x + 20.0,
            top + step * i as f32,
            line,
            style,
            card.w - 40.0,
        );
    }
}

pub(crate) fn big_number(c: &mut Canvas, card: Rect, value: &str, unit: &str) -> f32 {
    let style = theme::sans(34.0, theme::TEXT).bold();
    let w = c.text(card.x + 20.0, card.y + 64.0, value, style);
    let unit_style = theme::mono(13.0, theme::TEXT_2);
    let h = c.line_height(&style);
    c.text(
        card.x + 20.0 + w + 8.0,
        card.y + 64.0 + h - c.line_height(&unit_style) - 6.0,
        unit,
        unit_style,
    );
    card.x + 20.0 + w + 8.0 + c.width(unit, &unit_style) + 10.0
}

pub(crate) fn card_dpc(c: &mut Canvas, card: Rect, o: &Overview) {
    kpi_card(c, card);
    card_label(c, card, "DPC / interrupciones · p95");
    icon_box(
        c,
        card.right() - 56.0,
        card.y + 16.0,
        36.0,
        "activity",
        Tone::Green,
    );
    match &o.session {
        Some(s) => {
            let x = big_number(c, card, &format!("{:.1}", s.dpc_p95), "%");
            let (label, tone) = if s.dpc_p95 >= 5.0 {
                ("Alto", Tone::Amber)
            } else {
                ("Normal", Tone::Green)
            };
            tag(c, x, card.y + 74.0, label, tone, None);
            note(c, card, &format!("Tiempo del CPU en interrupciones. Alto = un driver acapara el procesador y da tirones. Partida de {}.", purifex_domain::session_list::duration_text(s.seconds)));
        }
        None => {
            big_number(c, card, "–", "%");
            note(c, card, "Graba una partida con purifex watch.");
        }
    }
}

pub(crate) fn card_devices(c: &mut Canvas, card: Rect, o: &Overview) {
    kpi_card(c, card);
    card_label(c, card, "Integridad de drivers");
    icon_box(
        c,
        card.right() - 56.0,
        card.y + 16.0,
        36.0,
        "microchip",
        Tone::Blue,
    );
    big_number(c, card, &o.devices.hardware.to_string(), "dispositivos");
    // The three counts, flowing onto a second line if they do not fit.
    let d = &o.devices;
    let stats = [
        (Tone::Green, d.healthy, "sin problemas"),
        (Tone::Blue, d.missing_driver, "sin driver"),
        (Tone::Red, d.faults, "con fallo"),
    ];
    c.line(
        card.x + 20.0,
        card.bottom() - 62.0,
        card.right() - 20.0,
        card.bottom() - 62.0,
        theme::LINE_SOFT,
    );
    let (mut x, mut y) = (card.x + 20.0, card.bottom() - 52.0);
    for (tone, count, label) in stats {
        let bold = theme::mono(11.5, theme::TEXT).bold();
        let plain = theme::mono(11.5, theme::TEXT_2);
        let count_text = count.to_string();
        let w = 14.0 + c.width(&count_text, &bold) + 6.0 + c.width(label, &plain) + 14.0;
        if x + w > card.right() - 12.0 && x > card.x + 20.0 {
            x = card.x + 20.0;
            y += 20.0;
        }
        dot(c, x + 3.5, y + 8.0, tone);
        let used = c.text(x + 14.0, y, &count_text, bold);
        c.text(x + 14.0 + used + 6.0, y, label, plain);
        x += w;
    }
}

pub(crate) fn card_limiter(c: &mut Canvas, card: Rect, o: &Overview) {
    kpi_card(c, card);
    card_label(c, card, "Lo que más te limita");
    icon_box(
        c,
        card.right() - 56.0,
        card.y + 16.0,
        36.0,
        "scan-search",
        Tone::Amber,
    );
    let good = o.limiter.title == "Nada destacable";
    c.icon(
        if good {
            "circle-check"
        } else {
            "triangle-alert"
        },
        card.x + 20.0,
        card.y + 68.0,
        18.0,
        if good { theme::ACCENT } else { theme::AMBER },
    );
    // Two lines at most, so the title never reaches the note under it.
    let style = theme::sans(17.0, theme::TEXT).bold();
    c.paragraph(
        card.x + 46.0,
        card.y + 64.0,
        card.w - 66.0,
        &o.limiter.title,
        style,
        Some(2),
    );
    note(c, card, &o.limiter.detail);
}

pub(crate) fn card_gpu(c: &mut Canvas, card: Rect, o: &Overview) {
    kpi_card(c, card);
    card_label(c, card, "Tarjeta gráfica");
    icon_box(
        c,
        card.right() - 56.0,
        card.y + 16.0,
        36.0,
        "monitor",
        Tone::Green,
    );
    let Some(gpu) = &o.gpu else {
        note(c, card, "No se encontró ninguna.");
        return;
    };
    c.paragraph(
        card.x + 20.0,
        card.y + 64.0,
        card.w - 40.0,
        &gpu.name,
        theme::sans(17.0, theme::TEXT).bold(),
        Some(2),
    );
    c.line(
        card.x + 20.0,
        card.bottom() - 62.0,
        card.right() - 20.0,
        card.bottom() - 62.0,
        theme::LINE_SOFT,
    );
    let plain = theme::mono(11.5, theme::TEXT_2);
    let bold = theme::mono(11.5, theme::TEXT).bold();
    let version = gpu.version.map_or("–".to_string(), |v| v.to_string());
    let used = c.text(card.x + 20.0, card.bottom() - 50.0, "Driver ", plain);
    c.text(card.x + 20.0 + used, card.bottom() - 50.0, &version, bold);
    let date = gpu.date.map_or("–".to_string(), |d| d.to_string());
    let used = c.text(card.x + 20.0, card.bottom() - 30.0, &date, plain);
    if let Some(days) = gpu.age_days {
        tag(
            c,
            card.x + 20.0 + used + 12.0,
            card.bottom() - 34.0,
            &age_text(days),
            Tone::Blue,
            None,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ages_read_in_the_unit_that_fits() {
        assert_eq!(age_text(11), "11 d");
        assert_eq!(age_text(95), "3 meses");
        assert_eq!(age_text(730), "2.0 años");
        assert_eq!(age_text(-1), "–");
    }
}
