//! The application behind the window: it owns what is shown (the `Model`),
//! carries out each `Action` of the interface and reads from the services what
//! is missing. Reading takes a moment, so an action only marks that something is
//! pending and says "busy": the interface draws that, and then `tick` reads.

use purifex_domain::view::{Action, Model, Page};

use crate::services::Services;

pub struct Core {
    services: Box<dyn Services>,
    model: Model,
    /// There is something to read that the model does not have yet.
    pending: bool,
}

impl Core {
    /// Starts on the overview, with its reading pending.
    pub fn new(services: Box<dyn Services>) -> Self {
        Self {
            services,
            model: Model {
                busy: true,
                ..Model::default()
            },
            pending: true,
        }
    }

    pub fn model(&self) -> &Model {
        &self.model
    }

    /// Whether `tick` has something to do.
    pub fn needs_tick(&self) -> bool {
        self.pending
    }

    pub fn apply(&mut self, action: Action) {
        match action {
            Action::Go(page) => {
                self.model.page = page;
                self.model.error = None;
                self.expect_reading();
            }
            Action::Rescan => {
                let m = &mut self.model;
                (m.overview, m.drivers, m.sessions, m.analysis, m.comparison) =
                    (None, None, None, None, None);
                m.error = None;
                self.expect_reading();
            }
            Action::SetFilter(filter) => self.model.filter = filter,
            Action::SetQuery(query) => self.model.query = query,
            Action::SelectSession(id) => {
                self.model.selected = Some(id);
                self.model.analysis = None;
                self.model.comparison = None;
                self.model.error = None;
                self.expect_reading();
            }
        }
    }

    /// Marks as busy if the current page lacks something.
    fn expect_reading(&mut self) {
        self.pending = self.missing();
        self.model.busy = self.pending;
    }

    /// Whether the page being shown (and the sidebar) lack data.
    fn missing(&self) -> bool {
        let m = &self.model;
        let page_lacks = match m.page {
            Page::Overview => false,
            Page::Drivers => m.drivers.is_none(),
            Page::Sessions => {
                m.analysis.is_none()
                    && m.sessions
                        .as_ref()
                        .is_none_or(|rows| rows.iter().any(|r| r.problem.is_none()))
            }
        };
        m.overview.is_none() || m.sessions.is_none() || page_lacks
    }

    /// The chosen recording against the game before it: the next older readable
    /// game of the list. `None` if the chosen one is not a game or has none before.
    fn compare_with_previous(&self, id: &str) -> Option<purifex_domain::compare::ComparisonReport> {
        let rows = self.model.sessions.as_ref()?;
        let at = rows.iter().position(|r| r.id == id)?;
        if !rows[at].is_game {
            return None;
        }
        let previous = rows[at + 1..]
            .iter()
            .find(|r| r.is_game && r.problem.is_none())?;
        self.services.compare(Some(&previous.id), Some(id)).ok()
    }

    /// Reads what is missing. Each failure is shown, and the rest is still read.
    pub fn tick(&mut self) {
        if !self.pending {
            return;
        }
        let mut errors = Vec::new();
        let mut note = |what: &str, error: String| errors.push(format!("{what}: {error}"));
        if self.model.overview.is_none() {
            match self.services.overview() {
                Ok(overview) => self.model.overview = Some(overview),
                Err(error) => note("estado general", error),
            }
        }
        if self.model.sessions.is_none() {
            match self.services.sessions() {
                Ok(rows) => self.model.sessions = Some(rows),
                Err(error) => note("grabaciones", error),
            }
        }
        if self.model.page == Page::Drivers && self.model.drivers.is_none() {
            match self.services.drivers() {
                Ok(view) => self.model.drivers = Some(view),
                Err(error) => note("drivers", error),
            }
        }
        if self.model.page == Page::Sessions && self.model.analysis.is_none() {
            // With nothing chosen, the newest recording that can be read.
            let chosen = self.model.selected.clone().or_else(|| {
                self.model
                    .sessions
                    .as_ref()
                    .and_then(|rows| rows.iter().find(|r| r.problem.is_none()))
                    .map(|r| r.id.clone())
            });
            if let Some(id) = chosen {
                match self.services.analyze(Some(&id)) {
                    Ok(analysis) => {
                        self.model.selected = Some(id.clone());
                        self.model.analysis = Some(analysis);
                        self.model.comparison = self.compare_with_previous(&id);
                    }
                    Err(error) => note("partida", error),
                }
            }
        }
        self.model.error = (!errors.is_empty()).then(|| errors.join(" · "));
        self.pending = false;
        self.model.busy = false;
    }
}

#[cfg(test)]
mod tests {
    use purifex_domain::device::Device;
    use purifex_domain::view::DriverFilter;

    use super::*;
    use crate::archive::Archive;
    use crate::recorder::Recorder;
    use crate::scanner::Scanner;
    use crate::services::App;
    use crate::testing::{FakeArchive, FakeClock, FakeFile, FakeInventory, FakeSampler};

    const GAME: &str =
        "t_s,cpu_total,cpu_max_core,game_cpu,game_mb\n1.0,1.0,1.0,5.0,900\n300.0,1.0,1.0,5.0,900\n";

    fn gpu() -> Device {
        Device {
            instance_id: r"PCI\A".into(),
            class: "Display".into(),
            name: "GPU".into(),
            ..Device::default()
        }
    }

    fn core_with(inventory: FakeInventory, archive: FakeArchive) -> Core {
        Core::new(Box::new(App::new(
            Scanner::new(inventory, FakeClock::ticking(1)),
            Recorder::new(
                FakeSampler::new(0, 1, true),
                FakeClock::ticking(0),
                FakeFile::default(),
            ),
            Archive::new(archive),
        )))
    }

    fn core() -> Core {
        core_with(
            FakeInventory::with(vec![gpu()]),
            FakeArchive::with(&[("lol-20261118-145915", GAME)]),
        )
    }

    #[test]
    fn it_starts_busy_on_the_overview_and_reads_on_the_tick() {
        let mut c = core();
        assert!(
            c.model().busy && c.needs_tick(),
            "the first picture says it is reading"
        );
        assert!(c.model().overview.is_none());
        c.tick();
        assert!(!c.model().busy && !c.needs_tick());
        assert_eq!(c.model().overview.as_ref().unwrap().devices.hardware, 1);
        assert_eq!(
            c.model().sessions.as_ref().unwrap().len(),
            1,
            "the sidebar counts them"
        );
        assert_eq!(c.model().error, None);
    }

    #[test]
    fn a_page_reads_its_data_the_first_time_and_never_again_while_going_back_and_forth() {
        let mut c = core();
        c.tick();
        c.apply(Action::Go(Page::Drivers));
        assert!(c.model().busy, "the drivers are not read yet");
        c.tick();
        assert_eq!(c.model().drivers.as_ref().unwrap().total, 1);
        c.apply(Action::Go(Page::Overview));
        assert!(!c.needs_tick(), "the overview is already read");
        c.apply(Action::Go(Page::Drivers));
        assert!(!c.needs_tick() && !c.model().busy);
    }

    #[test]
    fn the_sessions_page_opens_the_newest_readable_recording() {
        let mut c = core();
        c.tick();
        c.apply(Action::Go(Page::Sessions));
        c.tick();
        assert_eq!(c.model().selected.as_deref(), Some("lol-20261118-145915"));
        assert_eq!(c.model().analysis.as_ref().unwrap().summary.samples, 2);
    }

    #[test]
    fn choosing_a_recording_reads_its_analysis_and_a_missing_one_says_so() {
        let mut c = core_with(
            FakeInventory::with(vec![]),
            FakeArchive::with(&[("lol-20261118-145915", GAME), ("lol-20261118-165329", GAME)]),
        );
        c.tick();
        c.apply(Action::Go(Page::Sessions));
        c.tick();
        c.apply(Action::SelectSession("lol-20261118-145915".into()));
        assert!(c.model().busy && c.model().analysis.is_none());
        c.tick();
        assert_eq!(
            c.model().analysis.as_ref().unwrap().id,
            "lol-20261118-145915"
        );
        c.apply(Action::SelectSession("zzz".into()));
        c.tick();
        assert!(
            c.model().error.as_deref().unwrap().contains("no existe"),
            "{:?}",
            c.model().error
        );
    }

    #[test]
    fn a_game_is_shown_against_the_game_before_it_and_the_first_one_has_none() {
        let mut c = core_with(
            FakeInventory::with(vec![]),
            FakeArchive::with(&[
                ("lol-20261118-100000", GAME),
                (
                    "lol-20261118-120000",
                    "t_s,cpu_total,cpu_max_core
1.0,1.0,1.0
",
                ),
                ("lol-20261118-140000", GAME),
            ]),
        );
        c.tick();
        c.apply(Action::Go(Page::Sessions));
        c.tick();
        let newest = c
            .model()
            .comparison
            .as_ref()
            .expect("the newest game has one before it");
        assert_eq!(
            (newest.before.as_str(), newest.after.as_str()),
            ("lol-20261118-100000", "lol-20261118-140000"),
            "the short test in between is skipped"
        );
        c.apply(Action::SelectSession("lol-20261118-100000".into()));
        c.tick();
        assert!(
            c.model().comparison.is_none(),
            "nothing before the first game"
        );
        c.apply(Action::SelectSession("lol-20261118-120000".into()));
        c.tick();
        assert!(c.model().comparison.is_none(), "a test is not compared");
    }

    #[test]
    fn rescanning_reads_everything_again() {
        let mut c = core();
        c.tick();
        c.apply(Action::Go(Page::Drivers));
        c.tick();
        c.apply(Action::Rescan);
        let m = c.model();
        assert!(m.busy && m.overview.is_none() && m.drivers.is_none() && m.sessions.is_none());
        c.tick();
        assert!(c.model().overview.is_some() && c.model().drivers.is_some());
    }

    #[test]
    fn the_filter_and_the_search_need_no_reading() {
        let mut c = core();
        c.tick();
        c.apply(Action::SetFilter(DriverFilter::Attention));
        c.apply(Action::SetQuery("nvidia".into()));
        assert!(!c.needs_tick() && !c.model().busy);
        assert_eq!(
            (c.model().filter, c.model().query.as_str()),
            (DriverFilter::Attention, "nvidia")
        );
    }

    #[test]
    fn a_failure_is_shown_and_does_not_stop_the_rest_from_being_read() {
        let mut c = core_with(
            FakeInventory::failing("sin acceso"),
            FakeArchive::with(&[("lol-20261118-145915", GAME)]),
        );
        c.tick();
        assert!(c.model().error.as_deref().unwrap().contains("sin acceso"));
        assert!(
            c.model().sessions.is_some(),
            "the recordings were read all the same"
        );
        assert!(!c.model().busy, "it does not stay busy forever");
        c.apply(Action::Go(Page::Overview));
        assert!(
            c.needs_tick(),
            "the overview is still missing and is tried again"
        );
    }
}
