//! How the list of apps is narrowed and ordered: the filters of the page («Programas», «Apps de
//! Windows», «Instaladas hace poco») and the column it is sorted by. Plain rules over what the
//! apps registered; the page only asks.

use serde::{Deserialize, Serialize};

use crate::apps::{AppKind, InstalledApp};
use crate::date::Date;

/// An app is «recent» when it was installed this many days ago or less.
pub const RECENT_DAYS: i64 = 30;

/// Which apps the list shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppFilter {
    #[default]
    All,
    /// Classic programs (with their own uninstaller).
    Programs,
    /// Apps of the Microsoft Store.
    Store,
    /// Installed in the last `RECENT_DAYS` days (only those that say when).
    Recent,
}

impl AppFilter {
    pub const ALL: [AppFilter; 4] = [Self::All, Self::Programs, Self::Store, Self::Recent];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "Todas",
            Self::Programs => "Programas",
            Self::Store => "Apps de Windows",
            Self::Recent => "Instaladas hace poco",
        }
    }

    /// Whether the app passes the filter; `today` is needed only to judge «recent».
    pub fn allows(self, app: &InstalledApp, today: Option<Date>) -> bool {
        match self {
            Self::All => true,
            Self::Programs => app.kind == AppKind::Desktop,
            Self::Store => app.kind == AppKind::Store,
            Self::Recent => match (app.install_date, today) {
                (Some(on), Some(today)) => (0..=RECENT_DAYS).contains(&on.days_until(today)),
                _ => false,
            },
        }
    }
}

/// The column the list is ordered by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppSort {
    /// By name, A to Z.
    #[default]
    Name,
    /// The biggest first; the ones that do not say their size last.
    Size,
    /// The newest first; the ones that do not say when last.
    Date,
}

impl AppSort {
    /// Orders the apps (stable: equal ones keep their order, which is by name).
    pub fn order(self, apps: &mut [&InstalledApp]) {
        match self {
            Self::Name => apps.sort_by_key(|a| a.name.to_lowercase()),
            Self::Size => apps.sort_by_key(|a| std::cmp::Reverse(a.size_kb.unwrap_or(0))),
            Self::Date => apps.sort_by_key(|a| std::cmp::Reverse(a.install_date)),
        }
    }
}

/// How each app is drawn: with its details (publisher, version, type, install day) or as a
/// compact list of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppView {
    #[default]
    Details,
    List,
}

/// What the person chose for the list of apps, kept between runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AppListSettings {
    #[serde(default)]
    pub filter: AppFilter,
    #[serde(default)]
    pub sort: AppSort,
    #[serde(default)]
    pub view: AppView,
}

impl AppListSettings {
    pub fn to_text(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// What was saved; a file that cannot be read gives the defaults (a bad setting is never
    /// worth stopping for), and a word it does not know keeps only that part at its default.
    pub fn from_text(text: &str) -> Self {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
            return Self::default();
        };
        let part = |name: &str| value.get(name).cloned().unwrap_or_default();
        Self {
            filter: serde_json::from_value(part("filter")).unwrap_or_default(),
            sort: serde_json::from_value(part("sort")).unwrap_or_default(),
            view: serde_json::from_value(part("view")).unwrap_or_default(),
        }
    }
}

/// How many apps each filter lets through (for the count on its chip).
pub fn count(filter: AppFilter, apps: &[InstalledApp], today: Option<Date>) -> usize {
    apps.iter().filter(|a| filter.allows(a, today)).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(name: &str, kind: AppKind, kb: Option<u64>, on: Option<Date>) -> InstalledApp {
        InstalledApp {
            id: name.into(),
            name: name.into(),
            kind,
            size_kb: kb,
            install_date: on,
            ..Default::default()
        }
    }

    fn day(d: u8) -> Option<Date> {
        Date::new(2026, 11, d)
    }

    #[test]
    fn the_filters_tell_programs_from_store_apps_and_recent_from_old() {
        let today = day(20);
        let old = app("Old", AppKind::Desktop, None, Date::new(2026, 5, 1));
        let new = app("New", AppKind::Desktop, None, day(10));
        let store = app("Calc", AppKind::Store, None, None);
        let edge = app("Edge", AppKind::Desktop, None, day(20));
        let unknown = app("Unknown", AppKind::Desktop, None, None);
        let pass = |f: AppFilter, a: &InstalledApp| f.allows(a, today);
        assert!(pass(AppFilter::Programs, &old) && !pass(AppFilter::Programs, &store));
        assert!(pass(AppFilter::Store, &store) && !pass(AppFilter::Store, &new));
        assert!(pass(AppFilter::Recent, &new) && pass(AppFilter::Recent, &edge));
        assert!(!pass(AppFilter::Recent, &old) && !pass(AppFilter::Recent, &unknown));
        assert!(
            !AppFilter::Recent.allows(&new, None),
            "without today nothing is recent"
        );
        assert!(AppFilter::All.allows(&unknown, None));
        // A date in the future (a wrong clock or a wrong registry) is not «recent».
        let future = app("Future", AppKind::Desktop, None, day(25));
        assert!(!pass(AppFilter::Recent, &future));
    }

    #[test]
    fn the_sorts_put_the_unknown_last_and_keep_the_name_order_among_equals() {
        let a = app("Alpha", AppKind::Desktop, Some(10), day(1));
        let b = app("Beta", AppKind::Desktop, None, None);
        let c = app("Gamma", AppKind::Desktop, Some(500), day(9));
        let d = app("delta", AppKind::Desktop, Some(10), day(1));
        let names = |sort: AppSort| {
            let mut list = vec![&a, &b, &c, &d];
            AppSort::Name.order(&mut list);
            sort.order(&mut list);
            list.iter().map(|x| x.name.clone()).collect::<Vec<_>>()
        };
        assert_eq!(names(AppSort::Name), ["Alpha", "Beta", "delta", "Gamma"]);
        assert_eq!(names(AppSort::Size), ["Gamma", "Alpha", "delta", "Beta"]);
        assert_eq!(names(AppSort::Date), ["Gamma", "Alpha", "delta", "Beta"]);
    }

    #[test]
    fn the_settings_survive_the_text_and_a_bad_one_gives_the_defaults_part_by_part() {
        let chosen = AppListSettings {
            filter: AppFilter::Recent,
            sort: AppSort::Size,
            view: AppView::List,
        };
        assert_eq!(AppListSettings::from_text(&chosen.to_text()), chosen);
        assert_eq!(
            AppListSettings::from_text("no es json"),
            AppListSettings::default()
        );
        assert_eq!(AppListSettings::from_text(""), AppListSettings::default());
        // An unknown word (a newer version wrote it) only loses that part.
        let partly =
            AppListSettings::from_text(r#"{"filter":"nueva","sort":"size","view":"list"}"#);
        assert_eq!(
            (partly.filter, partly.sort, partly.view),
            (AppFilter::All, AppSort::Size, AppView::List)
        );
    }

    #[test]
    fn each_filter_counts_what_it_lets_through() {
        let apps = [
            app("A", AppKind::Desktop, None, day(18)),
            app("B", AppKind::Store, None, None),
        ];
        assert_eq!(count(AppFilter::All, &apps, day(20)), 2);
        assert_eq!(count(AppFilter::Store, &apps, day(20)), 1);
        assert_eq!(count(AppFilter::Recent, &apps, day(20)), 1);
    }
}
