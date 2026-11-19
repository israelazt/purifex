//! What Windows Update offers, on the drivers page: the panel that asks for it
//! (nothing leaves the computer until the user presses the button) and the tag
//! that a row of the table shows when an offer exists for its device.

use purifex_domain::draw::Rect;
use purifex_domain::driver_candidate::Risk;
use purifex_domain::driver_offers::{DeviceOffer, Offers};
use purifex_domain::view::{Action, Model};

use crate::canvas::Canvas;
use crate::theme::{self, Tone};
use crate::widgets::{button, button_width, panel, tag};

const HEIGHT: f32 = 84.0;

/// The text and the tone of the state of the search.
fn status(offers: &Offers) -> (String, Tone) {
    match offers {
        Offers::NotAsked => (
            "Pregunta a Microsoft qué drivers ofrece para este equipo. Solo se consulta cuando tú lo pides y no se instala nada.".into(),
            Tone::Gray,
        ),
        Offers::Searching => (
            "Buscando en Windows Update… puede tardar hasta un minuto. Puedes seguir usando la app.".into(),
            Tone::Blue,
        ),
        Offers::Found(report) => {
            let mut text = match report.offers.len() {
                0 => "Windows Update no ofrece drivers más nuevos que los instalados.".to_string(),
                1 => "1 dispositivo con un driver más nuevo disponible.".to_string(),
                n => format!("{n} dispositivos con un driver más nuevo disponible."),
            };
            if !report.unresolved.is_empty() {
                text += &format!(
                    " {} sin fuente automática: los resolvería tu IA por MCP.",
                    report.unresolved.len()
                );
            }
            text += " Todavía no se instala nada desde Purifex.";
            (text, if report.offers.is_empty() { Tone::Green } else { Tone::Blue })
        }
        Offers::Failed(error) => (format!("No se pudo buscar: {error}"), Tone::Red),
    }
}

/// Draws the panel at `y`; returns the height it took with the space below.
pub fn draw(c: &mut Canvas, m: &Model, area: Rect, y: f32) -> f32 {
    let rect = Rect::new(area.x, y, area.w, HEIGHT);
    panel(c, rect);
    let searching = m.offers == Offers::Searching;
    let label = if searching {
        "Buscando…"
    } else {
        "Buscar actualizaciones"
    };
    let w = button_width(c, "Buscar actualizaciones");
    let action = (!searching).then_some(Action::SearchUpdates);
    button(
        c,
        Rect::new(rect.right() - 16.0 - w, rect.y + 20.0, w, 44.0),
        label,
        "search",
        false,
        action,
    );
    let (text, tone) = status(&m.offers);
    c.icon("download", rect.x + 18.0, rect.y + 18.0, 18.0, tone.color());
    c.text(
        rect.x + 46.0,
        rect.y + 16.0,
        "Actualizaciones de Windows Update",
        theme::sans(14.0, theme::TEXT).bold(),
    );
    let color = if tone == Tone::Gray {
        theme::TEXT_2
    } else {
        tone.color()
    };
    c.paragraph(
        rect.x + 46.0,
        rect.y + 40.0,
        (rect.w - w - 100.0).max(200.0),
        &text,
        theme::mono(12.0, color),
        Some(2),
    );
    HEIGHT + 16.0
}

/// The tag of an offer in a row of the table, at (`x`, `y`).
pub fn row_tag(c: &mut Canvas, offer: &DeviceOffer, x: f32, y: f32) {
    let version = offer.version.map_or("nueva".to_string(), |v| v.to_string());
    // Sensitive devices (screen, network, disk, system) get the amber warning icon.
    let (tone, icon) = match offer.risk {
        Risk::Sensitive => (Tone::Amber, "triangle-alert"),
        Risk::Normal => (Tone::Blue, "download"),
    };
    tag(c, x, y, &format!("Hay {version}"), tone, Some(icon));
}

#[cfg(test)]
mod tests {
    use purifex_domain::draw::Op;
    use purifex_domain::driver_offers::OfferReport;
    use purifex_domain::version::DriverVersion;

    use super::*;
    use crate::canvas::testing::FakeText;

    fn offer(risk: Risk) -> DeviceOffer {
        DeviceOffer {
            instance_id: "PCI\\A".into(),
            name: "GPU".into(),
            class: "Display".into(),
            title: "X - Display - 2.0.0.0".into(),
            provider: "X".into(),
            version: DriverVersion::parse("2.0.0.0"),
            date: None,
            risk,
        }
    }

    fn panel_for(offers: Offers) -> (Vec<String>, Vec<Action>) {
        let mut c = Canvas::new(&FakeText, None);
        let m = Model {
            offers,
            ..Model::default()
        };
        let h = draw(&mut c, &m, Rect::new(0.0, 0.0, 1000.0, 0.0), 0.0);
        assert_eq!(h, HEIGHT + 16.0);
        let (list, hits) = c.finish();
        let texts = list
            .ops
            .iter()
            .filter_map(|op| match op {
                Op::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect();
        (texts, hits.into_iter().map(|h| h.action).collect())
    }

    #[test]
    fn before_asking_it_explains_that_nothing_leaves_until_the_user_asks_and_offers_the_button() {
        let (texts, actions) = panel_for(Offers::NotAsked);
        assert!(texts
            .join(" ")
            .contains("Solo se consulta cuando tú lo pides"));
        assert_eq!(actions, vec![Action::SearchUpdates]);
    }

    #[test]
    fn while_searching_the_button_does_nothing_and_says_so() {
        let (texts, actions) = panel_for(Offers::Searching);
        assert!(actions.is_empty(), "no second search");
        assert!(texts.iter().any(|t| t.contains("Buscando")));
    }

    fn report(offers: Vec<DeviceOffer>, unresolved: usize) -> Offers {
        use purifex_domain::driver_tiers::{Need, Unresolved};
        let unresolved = (0..unresolved)
            .map(|i| Unresolved {
                instance_id: format!("USB_{i}"),
                name: format!("Dispositivo {i}"),
                class: "Net".into(),
                hardware_ids: vec![],
                installed: None,
                need: Need::Fault { code: 31 },
            })
            .collect();
        Offers::Found(OfferReport {
            offers,
            unresolved,
            vendor_covered: 0,
        })
    }

    #[test]
    fn the_result_says_how_many_devices_have_something_newer_or_that_nothing_does() {
        let (none, _) = panel_for(report(vec![], 0));
        assert!(none.join(" ").contains("no ofrece drivers más nuevos"));
        let (one, _) = panel_for(report(vec![offer(Risk::Normal)], 0));
        assert!(one
            .join(" ")
            .contains("1 dispositivo con un driver más nuevo"));
        let (two, _) = panel_for(report(vec![offer(Risk::Normal), offer(Risk::Normal)], 0));
        assert!(two.join(" ").contains("2 dispositivos"));
    }

    #[test]
    fn devices_nobody_covers_are_counted_for_the_ai() {
        let (texts, _) = panel_for(report(vec![], 2));
        assert!(
            texts.join(" ").contains("2 sin fuente automática"),
            "{texts:?}"
        );
    }

    #[test]
    fn a_failure_is_shown_with_its_reason_and_the_button_can_try_again() {
        let (texts, actions) = panel_for(Offers::Failed("sin conexión".into()));
        assert!(texts
            .iter()
            .any(|t| t.contains("No se pudo buscar: sin conexión")));
        assert_eq!(actions, vec![Action::SearchUpdates]);
    }

    #[test]
    fn a_sensitive_device_is_tagged_with_the_warning_icon_and_a_normal_one_with_download() {
        for (risk, icon) in [
            (Risk::Sensitive, "triangle-alert"),
            (Risk::Normal, "download"),
        ] {
            let mut c = Canvas::new(&FakeText, None);
            row_tag(&mut c, &offer(risk), 0.0, 0.0);
            let (list, _) = c.finish();
            assert!(
                list.ops
                    .iter()
                    .any(|op| matches!(op, Op::Icon { name, .. } if *name == icon)),
                "{icon}"
            );
            assert!(list
                .ops
                .iter()
                .any(|op| matches!(op, Op::Text { text, .. } if text == "Hay 2.0.0.0")));
        }
    }
}
