//! The filter chips of the apps page («Todas», «Programas», «Apps de Windows», «Instaladas
//! hace poco»), each with how many apps it lets through. Which apps each one passes is decided
//! in the domain (`app_list`).

use purifex_domain::app_list::{count, AppFilter, AppView};
use purifex_domain::draw::Rect;
use purifex_domain::view::{Action, Model};

use crate::canvas::Canvas;
use crate::drivers_filters::{chip, Chip};
use crate::theme;

const GAP: f32 = 10.0;

pub(crate) fn app_filters(c: &mut Canvas, m: &Model, area: Rect, y: f32) -> f32 {
    let Some(apps) = m.apps.as_deref() else {
        return 0.0;
    };
    let mut x = area.x;
    for filter in AppFilter::ALL {
        let spec = Chip {
            label: filter.label(),
            count: count(filter, apps, m.today),
            active: m.app_filter == filter,
            red: false,
            action: Action::SetAppFilter(filter),
        };
        x += chip(c, x, y, &spec) + GAP;
    }
    let left = view_toggle(c, m, area, y);
    sight(c, m, left, y);
    38.0 + 16.0
}

/// When the list shows fewer apps than are installed, says how many and why, with a way back:
/// a filter kept from another day must not look like apps that are missing.
pub(crate) fn hidden_note(c: &mut Canvas, m: &Model, area: Rect, y: f32) -> f32 {
    let Some(all) = m.apps.as_deref() else {
        return 0.0;
    };
    let shown = m.visible_apps().len();
    if shown == all.len() {
        return 0.0;
    }
    let mut why = Vec::new();
    if m.app_filter != AppFilter::All {
        why.push(format!("filtro: {}", m.app_filter.label()));
    }
    if !m.query.trim().is_empty() {
        why.push(format!("búsqueda: «{}»", m.query.trim()));
    }
    let text = format!(
        "Mostrando {shown} de {} apps ({})",
        all.len(),
        why.join(" · ")
    );
    c.icon("triangle-alert", area.x + 4.0, y + 8.0, 16.0, theme::AMBER);
    c.text(
        area.x + 30.0,
        y + 8.0,
        &text,
        theme::mono(12.5, theme::TEXT),
    );
    let label = "Quitar filtros";
    let w = crate::widgets::button_width(c, label);
    let rect = Rect::new(
        area.x + 40.0 + c.width(&text, &theme::mono(12.5, theme::TEXT)),
        y,
        w,
        34.0,
    );
    crate::widgets::button(c, rect, label, "x", false, Some(Action::ClearAppFilters));
    34.0 + 12.0
}

/// «Detalles» and «Lista», at the right end of the row: how each app is drawn.
fn view_toggle(c: &mut Canvas, m: &Model, area: Rect, y: f32) -> f32 {
    let mut right = area.right();
    for view in [AppView::List, AppView::Details] {
        let label = if view == AppView::List {
            "Lista"
        } else {
            "Detalles"
        };
        let style = theme::mono(12.5, theme::TEXT_2);
        let rect = Rect::new(0.0, y, c.width(label, &style) + 28.0, 38.0);
        let rect = Rect::new(right - rect.w, y, rect.w, 38.0);
        let active = m.app_view == view;
        let hover = c.hover(rect);
        if active {
            c.fill(rect, 6.0, theme::ACCENT_2);
        } else {
            c.fill(rect, 6.0, theme::PANEL);
            c.stroke(rect, 6.0, 1.0, theme::LINE.mix(theme::TEXT_3, hover));
        }
        let color = if active {
            theme::ON_ACCENT
        } else {
            theme::TEXT_2
        };
        c.text_in(rect.x + 14.0, rect, label, theme::mono(12.5, color));
        c.hit(rect, Action::SetAppView(view));
        right = rect.x - GAP;
    }
    right
}

/// The sight of hunter mode: held down and dragged onto a window of any program, it says which
/// app that window belongs to. It acts when it is let go, not when it is pressed.
fn sight(c: &mut Canvas, m: &Model, right: f32, y: f32) {
    let label = if m.hunt.aiming {
        "Suelta sobre una ventana…"
    } else {
        "Cazador: arrastra"
    };
    let style = theme::mono(12.5, theme::TEXT_2);
    let width = c.width(label, &style) + 48.0;
    let rect = Rect::new(right - width, y, width, 38.0);
    let hover = c.hover(rect);
    if m.hunt.aiming {
        c.fill(rect, 6.0, theme::ACCENT_2);
    } else {
        c.fill(rect, 6.0, theme::PANEL);
        c.stroke(rect, 6.0, 1.0, theme::LINE.mix(theme::TEXT_3, hover));
    }
    let color = if m.hunt.aiming {
        theme::ON_ACCENT
    } else {
        theme::TEXT_2
    };
    c.icon("crosshair", rect.x + 12.0, rect.y + 10.0, 18.0, color);
    c.text_in(rect.x + 36.0, rect, label, theme::mono(12.5, color));
    c.hit(rect, Action::StartHunt);
}

#[cfg(test)]
mod tests {
    use purifex_domain::apps::InstalledApp;
    use purifex_domain::draw::Op;

    use super::*;
    use crate::canvas::testing::FakeText;

    #[test]
    fn every_filter_is_a_chip_with_its_count_and_a_click() {
        let m = Model {
            apps: Some(vec![InstalledApp {
                name: "A".into(),
                ..Default::default()
            }]),
            ..Model::default()
        };
        let mut c = Canvas::new(&FakeText, None);
        let h = app_filters(&mut c, &m, Rect::new(0.0, 0.0, 1100.0, 0.0), 0.0);
        assert!(h > 0.0);
        let (list, hits) = c.finish();
        let texts: Vec<String> = list
            .ops
            .iter()
            .filter_map(|op| match op {
                Op::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect();
        for label in [
            "Todas",
            "Programas",
            "Apps de Windows",
            "Instaladas hace poco",
        ] {
            assert!(
                texts.iter().any(|t| t.contains(label)),
                "{label}: {texts:?}"
            );
        }
        let actions: Vec<Action> = hits.into_iter().map(|h| h.action).collect();
        for filter in AppFilter::ALL {
            assert!(actions.contains(&Action::SetAppFilter(filter)));
        }
        assert!(actions.contains(&Action::SetAppView(AppView::List)));
        assert!(actions.contains(&Action::SetAppView(AppView::Details)));
        assert_eq!(
            app_filters(
                &mut Canvas::new(&FakeText, None),
                &Model::default(),
                Rect::new(0.0, 0.0, 9.0, 0.0),
                0.0
            ),
            0.0,
            "nothing before the apps are read"
        );
    }
}
