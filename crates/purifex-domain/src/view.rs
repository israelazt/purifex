//! What the interface shows (`Model`) and what it can ask for (`Action`): the
//! whole contract between the interface and the application. The interface only
//! reads the model and returns actions; the application decides what each one
//! does. Neither knows the other.

use crate::analysis::Analysis;
use crate::compare::ComparisonReport;
use crate::device_category::Category;
use crate::drivers_view::{DriverRow, DriversView};
use crate::overview::Overview;
use crate::session_list::SessionRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    Overview,
    Drivers,
    Sessions,
}

/// Which devices the drivers table lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DriverFilter {
    #[default]
    All,
    /// Only what needs attention (no driver, a fault).
    Attention,
    Category(Category),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Model {
    pub page: Page,
    pub overview: Option<Overview>,
    pub drivers: Option<DriversView>,
    pub sessions: Option<Vec<SessionRow>>,
    pub analysis: Option<Analysis>,
    /// The chosen game against the game before it, when there is one.
    pub comparison: Option<ComparisonReport>,
    /// The recording shown in the Sessions page.
    pub selected: Option<String>,
    pub filter: DriverFilter,
    pub query: String,
    /// Something is being read: the picture says so before the wait.
    pub busy: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Go(Page),
    /// Read everything again.
    Rescan,
    SetFilter(DriverFilter),
    SetQuery(String),
    SelectSession(String),
}

impl Model {
    /// The rows of the drivers table that the filter and the search let through.
    pub fn visible_drivers(&self) -> Vec<&DriverRow> {
        let Some(view) = &self.drivers else {
            return Vec::new();
        };
        let query = self.query.trim().to_lowercase();
        view.rows
            .iter()
            .filter(|row| match self.filter {
                DriverFilter::All => true,
                DriverFilter::Attention => row.needs_attention(),
                DriverFilter::Category(category) => row.category == category,
            })
            .filter(|row| query.is_empty() || row.matches(&query))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::Date;
    use crate::device::{Device, InstalledDriver};
    use crate::report::{ScanReport, Scope};
    use crate::version::DriverVersion;

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
                    r"PCI\VEN_10DE&DEV_2182",
                    "Display",
                    "NVIDIA GeForce GTX 1660 Ti",
                    "NVIDIA",
                    None,
                ),
                device(
                    r"USB\VID_0BDA",
                    "Bluetooth",
                    "Realtek Bluetooth",
                    "Realtek",
                    Some(31),
                ),
                device(
                    r"HDAUDIO\FUNC_01",
                    "MEDIA",
                    "Realtek Audio",
                    "Realtek",
                    None,
                ),
            ],
            Scope::Hardware,
            Date::new(2026, 11, 18).unwrap(),
            5,
        );
        Model {
            drivers: Some(DriversView::build(&report)),
            ..Model::default()
        }
    }

    fn names(m: &Model) -> Vec<&str> {
        m.visible_drivers()
            .iter()
            .map(|r| r.name.as_str())
            .collect()
    }

    #[test]
    fn with_no_filter_every_row_shows_and_what_needs_attention_is_first() {
        assert_eq!(names(&model())[0], "Realtek Bluetooth");
        assert_eq!(names(&model()).len(), 3);
        assert!(
            Model::default().visible_drivers().is_empty(),
            "nothing read yet"
        );
    }

    #[test]
    fn a_filter_keeps_only_its_devices() {
        let mut m = model();
        m.filter = DriverFilter::Attention;
        assert_eq!(names(&m), ["Realtek Bluetooth"]);
        m.filter = DriverFilter::Category(Category::Gpu);
        assert_eq!(names(&m), ["NVIDIA GeForce GTX 1660 Ti"]);
        m.filter = DriverFilter::Category(Category::Storage);
        assert!(names(&m).is_empty());
    }

    #[test]
    fn the_search_looks_in_the_name_the_id_the_provider_and_the_class_whatever_the_case() {
        let mut m = model();
        for (query, expected) in [
            ("nvidia", 1),
            ("VEN_10DE", 1),
            ("realtek", 2),
            ("media", 1),
            ("  audio ", 1),
            ("zzz", 0),
        ] {
            m.query = query.into();
            assert_eq!(names(&m).len(), expected, "{query:?}");
        }
    }

    #[test]
    fn the_filter_and_the_search_work_together() {
        let mut m = model();
        m.filter = DriverFilter::Category(Category::Audio);
        m.query = "realtek".into();
        assert_eq!(names(&m), ["Realtek Audio"]);
    }
}
