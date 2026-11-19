//! The application behind the window: it owns what is shown (the `Model`),
//! carries out each `Action` of the interface and reads from the services what
//! is missing. Reading takes a moment, so an action only marks that something is
//! pending and says "busy": the interface draws that, and then `tick` reads.

use std::time::Duration;

use purifex_domain::driver_offers::Offers;
use purifex_domain::view::{Action, Confirm, Model, Page};

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
                // What was waiting for a confirmation, or told of a change, stays on its page.
                (self.model.confirm, self.model.notice) = (None, None);
                self.expect_reading();
            }
            Action::Rescan => {
                let m = &mut self.model;
                (m.overview, m.drivers, m.sessions, m.analysis, m.comparison) =
                    (None, None, None, None, None);
                (m.candidates, m.apps, m.traces) = (None, None, None);
                (m.fixes, m.history, m.confirm, m.notice) = (None, None, None, None);
                m.error = None;
                self.expect_reading();
            }
            Action::SearchUpdates => match self.services.start_offers() {
                Ok(()) => {
                    self.model.offers = Offers::Searching;
                    self.pending = true;
                }
                Err(error) => self.model.offers = Offers::Failed(error),
            },
            Action::OpenPage(url) => {
                self.model.error = self.services.open_page(&url).err();
            }
            Action::AskUninstall(id) => self.model.confirm = Some(Confirm::Uninstall(id)),
            Action::AskClean(name) => self.model.confirm = Some(Confirm::Clean(name)),
            Action::FindTraces(name) => {
                (self.model.traces, self.model.message) = (None, None);
                self.model.finding = Some(name);
                self.expect_reading();
            }
            Action::SetFilter(filter) => self.model.filter = filter,
            Action::SetQuery(query) => self.model.query = query,
            Action::AskFix(id) => self.model.confirm = Some(Confirm::Apply(id)),
            Action::AskUndo(seq) => self.model.confirm = Some(Confirm::Undo(seq)),
            Action::CancelConfirm => self.model.confirm = None,
            Action::Confirm => self.confirmed(),
            Action::SelectSession(id) => {
                self.model.selected = Some(id);
                self.model.analysis = None;
                self.model.comparison = None;
                self.model.error = None;
                self.expect_reading();
            }
        }
    }

    /// Carries out what was waiting for the user's confirmation. The change is
    /// made by the services (which check it is still proposed, that nothing blocks
    /// it and that it holds); here only its result is kept to be shown.
    fn confirmed(&mut self) {
        let Some(confirm) = self.model.confirm.take() else {
            return;
        };
        self.model.error = None;
        let result = match confirm {
            Confirm::Apply(id) => self.services.apply_fix(&id, true, false),
            Confirm::Undo(seq) => self.services.undo_fix(seq, true),
            Confirm::Uninstall(id) => return self.uninstalled(&id),
            Confirm::Clean(name) => return self.cleaned(&name),
        };
        match result {
            Ok(report) => {
                self.model.notice = Some(report);
                // What is offered and what was changed are different now.
                (self.model.fixes, self.model.history) = (None, None);
                self.expect_reading();
            }
            Err(error) => {
                self.model.notice = None;
                self.model.error = Some(error);
            }
        }
    }

    /// Starts the uninstaller of an app; its traces are looked for afterwards.
    fn uninstalled(&mut self, id: &str) {
        match self.services.uninstall_app(id) {
            Ok(app) => {
                self.model.message = Some(format!(
                    "Se abrió el desinstalador de {}. Cuando termine, pulsa «Buscar restos».",
                    app.name
                ));
                (self.model.uninstalled, self.model.traces) = (Some(app.name), None);
            }
            Err(error) => self.model.error = Some(error),
        }
    }

    /// Removes the traces of an app to the quarantine.
    fn cleaned(&mut self, name: &str) {
        match self.services.clean_app(name, true) {
            Ok(report) => {
                let (done, failed) = (report.removed.len(), report.failed.len());
                self.model.message = Some(format!(
                    "Se quitaron {done} restos de {name} a la cuarentena{}. Para devolverlos: purifex apps restore N.",
                    if failed > 0 { format!(" ({failed} no se pudieron quitar)") } else { String::new() }
                ));
                (self.model.traces, self.model.apps) = (None, None);
                self.expect_reading();
            }
            Err(error) => self.model.error = Some(error),
        }
    }

    /// Marks as busy if the current page lacks something.
    fn expect_reading(&mut self) {
        let missing = self.missing();
        // A search of offers still going must be picked up, whatever page is shown.
        self.pending = missing || self.model.offers == Offers::Searching;
        self.model.busy = missing;
    }

    /// Whether the page being shown (and the sidebar) lack data.
    fn missing(&self) -> bool {
        let m = &self.model;
        let page_lacks = match m.page {
            Page::Overview => false,
            Page::Fixes => m.fixes.is_none() || m.history.is_none(),
            Page::Apps => m.apps.is_none() || m.finding.is_some(),
            Page::Drivers => m.drivers.is_none() || m.candidates.is_none(),
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
    /// Returns whether the model changed (the window only redraws if it did).
    pub fn tick(&mut self) -> bool {
        if !self.pending {
            return false;
        }
        let arrived = self.poll_offers();
        if !self.missing() {
            // Only the search of offers is going: nothing to read, nothing blocks.
            self.pending = self.model.offers == Offers::Searching;
            self.model.busy = false;
            return arrived;
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
        if self.model.page == Page::Apps {
            if self.model.apps.is_none() {
                match self.services.apps() {
                    Ok(apps) => self.model.apps = Some(apps),
                    Err(error) => note("apps", error),
                }
            }
            if let Some(name) = self.model.finding.take() {
                match self.services.clean_app(&name, false) {
                    Ok(report) => self.model.traces = Some(report),
                    Err(error) => note("restos", error),
                }
            }
        }
        if self.model.page == Page::Drivers && self.model.candidates.is_none() {
            match self.services.candidates() {
                Ok(list) => self.model.candidates = Some(list),
                Err(error) => {
                    // Shown once; not asked again on every visit.
                    self.model.candidates = Some(Vec::new());
                    note("propuestas", error);
                }
            }
        }
        if self.model.page == Page::Fixes {
            if self.model.fixes.is_none() {
                match self.services.fixes() {
                    Ok(fixes) => self.model.fixes = Some(fixes),
                    Err(error) => note("arreglos", error),
                }
            }
            if self.model.history.is_none() {
                match self.services.history() {
                    Ok(history) => self.model.history = Some(history),
                    Err(error) => note("historial", error),
                }
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
        if !errors.is_empty() {
            self.model.error = Some(errors.join(" · "));
        }
        self.pending = self.model.offers == Offers::Searching;
        self.model.busy = false;
        true
    }

    /// Picks up the search of offers if it has finished. It never waits: the window
    /// must keep answering the mouse, and asks again a moment later. Returns
    /// whether the result arrived.
    fn poll_offers(&mut self) -> bool {
        if self.model.offers != Offers::Searching {
            return false;
        }
        let Some(found) = self.services.poll_offers(Duration::ZERO) else {
            return false;
        };
        self.model.offers = match found {
            Ok(offers) => Offers::Found(offers),
            Err(error) => Offers::Failed(error),
        };
        true
    }
}

#[cfg(test)]
mod tests {
    use purifex_domain::device::Device;
    use purifex_domain::fix::SettingValue;
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

    const CLIENT_HEAVY: &str = "t_s,cpu_total,cpu_max_core,game_cpu,game_mb,groups\n1.0,40.0,80.0,13.0,900,league_client=16.0\n300.0,40.0,80.0,13.0,900,league_client=16.0\n";

    fn fixes_core(
        value: Option<SettingValue>,
    ) -> (
        Core,
        std::rc::Rc<std::cell::RefCell<crate::testing::FixState>>,
    ) {
        let (fake, state) = crate::testing::FakeFixes::with(value);
        let app = App::new(
            Scanner::new(FakeInventory::with(vec![gpu()]), FakeClock::ticking(1)),
            Recorder::new(
                FakeSampler::new(0, 1, true),
                FakeClock::ticking(0),
                FakeFile::default(),
            ),
            Archive::new(FakeArchive::with(&[("lol-20261118-145915", CLIENT_HEAVY)])),
        )
        .with_fixer(fake.fixer());
        let mut core = Core::new(Box::new(app));
        core.tick();
        core.apply(Action::Go(Page::Fixes));
        core.tick();
        (core, state)
    }

    #[test]
    fn the_fixes_page_reads_what_is_proposed_and_the_history() {
        let (c, _) = fixes_core(Some(SettingValue::Bool(false)));
        assert_eq!(c.model().fixes.as_ref().unwrap().len(), 1);
        assert!(c.model().history.as_ref().unwrap().is_empty());
        assert!(!c.model().busy && c.model().error.is_none());
    }

    #[test]
    fn asking_to_apply_changes_nothing_until_it_is_confirmed_and_can_be_cancelled() {
        let (mut c, state) = fixes_core(Some(SettingValue::Bool(false)));
        let id = c.model().fixes.as_ref().unwrap()[0].id.clone();
        c.apply(Action::AskFix(id.clone()));
        assert_eq!(c.model().confirm, Some(Confirm::Apply(id)));
        assert!(state.borrow().writes.is_empty(), "asking is not doing");
        c.apply(Action::CancelConfirm);
        assert!(c.model().confirm.is_none());
        c.apply(Action::Confirm);
        assert!(
            state.borrow().writes.is_empty(),
            "a confirmation with nothing to confirm does nothing"
        );
    }

    #[test]
    fn confirming_applies_it_notes_it_and_what_is_offered_is_read_again() {
        let (mut c, state) = fixes_core(Some(SettingValue::Bool(false)));
        let id = c.model().fixes.as_ref().unwrap()[0].id.clone();
        c.apply(Action::AskFix(id));
        c.apply(Action::Confirm);
        assert_eq!(state.borrow().value, Some(SettingValue::Bool(true)));
        assert!(c
            .model()
            .notice
            .as_ref()
            .is_some_and(|n| !n.preview && n.seq == Some(1)));
        assert!(
            c.model().confirm.is_none() && c.model().busy,
            "it reads again what is offered"
        );
        c.tick();
        assert!(
            c.model().fixes.as_ref().unwrap().is_empty(),
            "it is already done: nothing to offer"
        );
        assert_eq!(c.model().history.as_ref().unwrap().len(), 1);
        assert!(
            c.model().notice.is_some(),
            "the notice stays after the reading"
        );
    }

    #[test]
    fn a_change_that_cannot_be_made_is_shown_and_nothing_is_changed() {
        let (mut c, state) = fixes_core(Some(SettingValue::Bool(false)));
        state.borrow_mut().blockers = vec!["Cierra el cliente de League".into()];
        let id = c.model().fixes.as_ref().unwrap()[0].id.clone();
        c.apply(Action::AskFix(id));
        c.apply(Action::Confirm);
        assert!(c
            .model()
            .error
            .as_deref()
            .unwrap()
            .contains("Cierra el cliente de League"));
        assert!(c.model().notice.is_none());
        assert!(state.borrow().writes.is_empty());
    }

    #[test]
    fn a_change_can_be_undone_from_the_history_with_its_own_confirmation() {
        let (mut c, state) = fixes_core(Some(SettingValue::Bool(false)));
        let id = c.model().fixes.as_ref().unwrap()[0].id.clone();
        c.apply(Action::AskFix(id));
        c.apply(Action::Confirm);
        c.tick();
        c.apply(Action::AskUndo(1));
        assert_eq!(c.model().confirm, Some(Confirm::Undo(1)));
        assert_eq!(
            state.borrow().value,
            Some(SettingValue::Bool(true)),
            "still applied until confirmed"
        );
        c.apply(Action::Confirm);
        assert_eq!(state.borrow().value, Some(SettingValue::Bool(false)));
        c.tick();
        assert_eq!(
            c.model().fixes.as_ref().unwrap().len(),
            1,
            "offered again once undone"
        );
    }

    #[test]
    fn leaving_the_page_forgets_a_pending_confirmation_and_the_notice() {
        let (mut c, _) = fixes_core(Some(SettingValue::Bool(false)));
        c.apply(Action::AskFix("x".into()));
        c.apply(Action::Go(Page::Overview));
        assert!(c.model().confirm.is_none() && c.model().notice.is_none());
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

    use crate::testing::FakeOffers;
    use purifex_domain::driver_offers::Offer;
    use purifex_domain::version::DriverVersion;
    use std::sync::Arc;

    fn core_with_offers(source: FakeOffers) -> Core {
        let mut gpu = gpu();
        gpu.hardware_ids = vec![r"PCI\VEN_10DE&DEV_2182".into()];
        gpu.driver = Some(purifex_domain::device::InstalledDriver {
            version: DriverVersion::parse("1.0.0.0"),
            ..Default::default()
        });
        gpu.instance_id = r"PCI\VEN_10DE&DEV_2182".into();
        Core::new(Box::new(
            App::new(
                Scanner::new(FakeInventory::with(vec![gpu]), FakeClock::ticking(1)),
                Recorder::new(
                    FakeSampler::new(0, 1, true),
                    FakeClock::ticking(0),
                    FakeFile::default(),
                ),
                Archive::new(FakeArchive::with(&[])),
            )
            .with_offers(crate::Offering::new(Arc::new(source))),
        ))
    }

    fn newer_offer() -> Offer {
        Offer {
            hardware_id: r"PCI\VEN_10DE&DEV_2182".into(),
            title: "NVIDIA - Display - 2.0.0.0".into(),
            provider: "NVIDIA".into(),
            version: DriverVersion::parse("2.0.0.0"),
            date: None,
        }
    }

    fn tick_until_done(c: &mut Core) {
        for _ in 0..400 {
            c.tick();
            // The window asks again after a moment, not in a tight loop.
            std::thread::sleep(Duration::from_millis(5));
            if !c.needs_tick() {
                return;
            }
        }
        panic!("the search never ended");
    }

    #[test]
    fn nothing_is_asked_of_windows_update_until_the_user_asks() {
        let mut c = core_with_offers(FakeOffers(0, Ok(vec![newer_offer()])));
        c.tick();
        c.apply(Action::Go(Page::Drivers));
        c.tick();
        assert_eq!(c.model().offers, Offers::NotAsked);
        assert!(!c.needs_tick());
    }

    #[test]
    fn asking_searches_in_the_background_and_the_offers_arrive_matched_to_their_device() {
        let mut c = core_with_offers(FakeOffers(80, Ok(vec![newer_offer()])));
        c.tick();
        c.apply(Action::Go(Page::Drivers));
        c.tick();
        c.apply(Action::SearchUpdates);
        assert_eq!(c.model().offers, Offers::Searching);
        assert!(
            c.needs_tick() && !c.model().busy,
            "the page is usable meanwhile"
        );
        // Going to another page does not lose the search.
        c.apply(Action::Go(Page::Overview));
        assert!(c.needs_tick());
        tick_until_done(&mut c);
        let Offers::Found(report) = &c.model().offers else {
            panic!("{:?}", c.model().offers)
        };
        assert_eq!(report.offers.len(), 1);
        assert_eq!(report.offers[0].version, DriverVersion::parse("2.0.0.0"));
    }

    #[test]
    fn a_failed_search_says_why_and_a_missing_source_does_too() {
        let mut c = core_with_offers(FakeOffers(0, Err("sin conexión".into())));
        c.tick();
        c.apply(Action::SearchUpdates);
        tick_until_done(&mut c);
        assert_eq!(c.model().offers, Offers::Failed("sin conexión".into()));
        let mut bare = core();
        bare.tick();
        bare.apply(Action::SearchUpdates);
        assert!(matches!(&bare.model().offers, Offers::Failed(e) if e.contains("no sabe buscar")));
        assert!(!bare.needs_tick());
    }

    #[test]
    fn the_drivers_page_reads_the_proposals_and_opening_one_goes_through_the_browser() {
        use crate::testing::{FakeCandidates, FakeOpener};
        use crate::Candidates;
        use purifex_domain::driver_candidate::{Candidate, Source};
        use purifex_domain::ports::CandidateStore;
        let mut store = FakeCandidates::default();
        store
            .add(&Candidate {
                device: r"PCI\A".into(),
                source: Source::Suggested,
                url: "https://www.realtek.com/Download".into(),
                version: DriverVersion::parse("2.0").unwrap(),
                title: "t".into(),
            })
            .unwrap();
        let opener = FakeOpener::default();
        let mut c = Core::new(Box::new(
            App::new(
                Scanner::new(FakeInventory::with(vec![gpu()]), FakeClock::ticking(1)),
                Recorder::new(
                    FakeSampler::new(0, 1, true),
                    FakeClock::ticking(0),
                    FakeFile::default(),
                ),
                Archive::new(FakeArchive::with(&[])),
            )
            .with_candidates(Candidates::new(Box::new(store)))
            .with_opener(Box::new(opener.clone())),
        ));
        c.tick();
        assert!(
            c.model().candidates.is_none(),
            "only the drivers page reads them"
        );
        c.apply(Action::Go(Page::Drivers));
        c.tick();
        assert_eq!(c.model().candidates.as_ref().unwrap().len(), 1);
        c.apply(Action::OpenPage("https://www.realtek.com/Download".into()));
        assert_eq!(
            opener.0.borrow().as_slice(),
            ["https://www.realtek.com/Download"]
        );
        assert_eq!(c.model().error, None);
        // Whatever asks, a page that is not an official one is never opened.
        c.apply(Action::OpenPage("https://realtek.com.evil.io/x".into()));
        c.apply(Action::OpenPage(
            "file:///C:/Windows/System32/cmd.exe".into(),
        ));
        assert_eq!(opener.0.borrow().len(), 1);
        assert!(c
            .model()
            .error
            .as_deref()
            .unwrap()
            .contains("solo se abren"));
    }

    #[test]
    fn while_the_search_goes_on_a_tick_never_waits_and_reports_no_change_until_it_ends() {
        let mut c = core_with_offers(FakeOffers(300, Ok(vec![newer_offer()])));
        c.tick();
        c.apply(Action::SearchUpdates);
        let started = std::time::Instant::now();
        let mut changed = false;
        for _ in 0..20 {
            changed |= c.tick();
        }
        assert!(
            started.elapsed() < Duration::from_millis(100),
            "20 ticks took {:?}: the window would not answer the mouse",
            started.elapsed()
        );
        assert!(!changed, "nothing to repaint while it searches");
        assert!(c.needs_tick());
        let mut arrived = false;
        for _ in 0..400 {
            arrived |= c.tick();
            if !c.needs_tick() {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(
            arrived,
            "the tick that brings the result says the model changed"
        );
        assert!(matches!(c.model().offers, Offers::Found(_)));
    }

    fn fake_app(name: &str) -> purifex_domain::apps::InstalledApp {
        purifex_domain::apps::InstalledApp {
            id: format!("HKLM64:{name}"),
            name: name.into(),
            publisher: Some("Discord Inc.".into()),
            version: Some("1.0".into()),
            install_location: None,
            uninstall: Some("u.exe".into()),
            quiet_uninstall: None,
            size_kb: Some(1024),
            kind: purifex_domain::apps::AppKind::Desktop,
            hidden: false,
        }
    }

    #[test]
    fn the_apps_page_uninstalls_with_a_confirmation_then_finds_and_removes_the_traces() {
        use crate::testing::{CleanLog, FakeApps, FakeCleaning, FakeFinder};
        use crate::Cleaning;
        use purifex_domain::app_traces::{Candidate, TraceKind};
        use std::cell::RefCell;
        use std::rc::Rc;
        let installed = FakeApps::default();
        installed.0.borrow_mut().push(fake_app("Discord"));
        let log = Rc::new(RefCell::new(CleanLog::default()));
        let found = vec![Candidate {
            kind: TraceKind::Folder,
            location: r"C:\Users\u\AppData\Roaming\Discord".into(),
            bytes: 10,
            registered_install: false,
        }];
        let mut c = Core::new(Box::new(
            App::new(
                Scanner::new(FakeInventory::with(vec![gpu()]), FakeClock::ticking(1)),
                Recorder::new(
                    FakeSampler::new(0, 1, true),
                    FakeClock::ticking(0),
                    FakeFile::default(),
                ),
                Archive::new(FakeArchive::with(&[])),
            )
            .with_apps(Box::new(installed.clone()))
            .with_cleaning(Cleaning::new(
                Box::new(FakeFinder(found)),
                Box::new(FakeCleaning(log.clone())),
                Box::new(FakeCleaning(log.clone())),
            )),
        ));
        c.tick();
        c.apply(Action::Go(Page::Apps));
        assert!(c.model().busy, "the apps are read on the tick");
        c.tick();
        assert_eq!(c.model().visible_apps().len(), 1);
        // Asking changes nothing; the uninstaller starts only on confirming.
        c.apply(Action::AskUninstall("HKLM64:Discord".into()));
        assert_eq!(
            c.model().confirm,
            Some(Confirm::Uninstall("HKLM64:Discord".into()))
        );
        assert!(log.borrow().launched.is_empty());
        c.apply(Action::CancelConfirm);
        assert!(log.borrow().launched.is_empty());
        c.apply(Action::AskUninstall("HKLM64:Discord".into()));
        c.apply(Action::Confirm);
        assert_eq!(log.borrow().launched, ["Discord"]);
        assert_eq!(c.model().uninstalled.as_deref(), Some("Discord"));
        assert!(c
            .model()
            .message
            .as_deref()
            .unwrap()
            .contains("Buscar restos"));
        // While it is still installed the traces can be looked at but not removed.
        c.apply(Action::FindTraces("Discord".into()));
        assert!(c.model().busy);
        c.tick();
        assert_eq!(c.model().traces.as_ref().unwrap().proposed.len(), 1);
        c.apply(Action::AskClean("Discord".into()));
        c.apply(Action::Confirm);
        assert!(c
            .model()
            .error
            .as_deref()
            .unwrap()
            .contains("sigue instalada"));
        assert!(log.borrow().removed.is_empty());
        // The uninstaller finished: now they go to the quarantine.
        installed.0.borrow_mut().clear();
        c.apply(Action::AskClean("Discord".into()));
        c.apply(Action::Confirm);
        assert_eq!(log.borrow().removed.len(), 1);
        assert!(c
            .model()
            .message
            .as_deref()
            .unwrap()
            .contains("Se quitaron 1 restos"));
        assert!(c.model().traces.is_none());
    }

    #[test]
    fn a_protected_app_cannot_be_uninstalled_from_the_page() {
        use crate::testing::{CleanLog, FakeApps, FakeCleaning, FakeFinder};
        use crate::Cleaning;
        use std::cell::RefCell;
        use std::rc::Rc;
        let installed = FakeApps::default();
        let mut runtime = fake_app("Microsoft .NET Runtime");
        runtime.publisher = Some("Microsoft Corporation".into());
        installed.0.borrow_mut().push(runtime);
        let log = Rc::new(RefCell::new(CleanLog::default()));
        let mut c = Core::new(Box::new(
            App::new(
                Scanner::new(FakeInventory::with(vec![gpu()]), FakeClock::ticking(1)),
                Recorder::new(
                    FakeSampler::new(0, 1, true),
                    FakeClock::ticking(0),
                    FakeFile::default(),
                ),
                Archive::new(FakeArchive::with(&[])),
            )
            .with_apps(Box::new(installed))
            .with_cleaning(Cleaning::new(
                Box::new(FakeFinder(vec![])),
                Box::new(FakeCleaning(log.clone())),
                Box::new(FakeCleaning(log.clone())),
            )),
        ));
        c.tick();
        c.apply(Action::Go(Page::Apps));
        c.tick();
        c.apply(Action::AskUninstall("HKLM64:Microsoft .NET Runtime".into()));
        c.apply(Action::Confirm);
        assert!(c
            .model()
            .error
            .as_deref()
            .unwrap()
            .contains("no se desinstala desde aquí"));
        assert!(log.borrow().launched.is_empty());
    }
}
