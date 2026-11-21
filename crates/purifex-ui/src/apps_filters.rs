//! The filter chips of the apps page («Todas», «Programas», «Apps de Windows», «Instaladas
//! hace poco»), each with how many apps it lets through. Which apps each one passes is decided
//! in the domain (`app_list`).

use purifex_domain::app_list::{count, AppFilter};
use purifex_domain::draw::Rect;
use purifex_domain::view::{Action, Model};

use crate::canvas::Canvas;
use crate::drivers_filters::{chip, Chip};

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
    38.0 + 16.0
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
