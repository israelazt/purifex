//! How the list of apps is narrowed and ordered: the filters of the page («Programas», «Apps de
//! Windows», «Instaladas hace poco») and the column it is sorted by. Plain rules over what the
//! apps registered; the page only asks.

use crate::apps::{AppKind, InstalledApp};
use crate::date::Date;

/// An app is «recent» when it was installed this many days ago or less.
pub const RECENT_DAYS: i64 = 30;

/// Which apps the list shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
