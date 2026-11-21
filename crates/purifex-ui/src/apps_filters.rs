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
    view_toggle(c, m, area, y);
    38.0 + 16.0
}

/// «Detalles» and «Lista», at the right end of the row: how each app is drawn.
fn view_toggle(c: &mut Canvas, m: &Model, area: Rect, y: f32) {
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
