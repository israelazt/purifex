//! The application behind the window: it owns what is shown (the `Model`),
//! carries out each `Action` of the interface and reads from the services what
//! is missing. Reading takes a moment, so an action only marks that something is
//! pending and says "busy": the interface draws that, and then `tick` reads.

use std::time::{Duration, Instant};

/// How often the list is read while an uninstaller started here is running.
const WATCH_EVERY: Duration = if cfg!(test) {
    Duration::from_millis(10)
} else {
    Duration::from_millis(1500)
};
/// How long an uninstaller is waited for.
const WATCH_FOR: Duration = Duration::from_secs(30 * 60);

use purifex_domain::driver_offers::Offers;
use purifex_domain::view::{Action, Confirm, Model, Page};

use crate::services::Services;

mod actions;
mod apps;
mod reading;

pub struct Core {
    services: Box<dyn Services>,
    model: Model,
    /// There is something to read that the model does not have yet.
    pending: bool,
    /// The program started again as an administrator: the window closes.
    quit: bool,
    /// The window came back: the list of apps is read again without clearing it.
    refreshing_apps: bool,
    /// The app whose uninstaller was started here, when it was last looked for in the
    /// list, and when it was started: when it disappears the uninstall has finished.
    watching: Option<(purifex_domain::apps::InstalledApp, Instant, Instant)>,
    /// What had been in the quarantine for seven days was already let go this time.
    expired_checked: bool,
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
            refreshing_apps: false,
            watching: None,
            quit: false,
            expired_checked: false,
        }
    }

    pub fn model(&self) -> &Model {
        &self.model
    }

    /// Whether the window should close (the program restarted as an administrator).
    pub fn wants_quit(&self) -> bool {
        self.quit
    }

    /// Whether `tick` has something to do.
    pub fn needs_tick(&self) -> bool {
        self.pending
    }

    pub fn apply(&mut self, action: Action) {
        match action {
            Action::Go(page) => self.go(page),
            Action::Rescan => self.rescan(),
            Action::SearchUpdates => self.search_updates(),
            Action::OpenPage(url) => self.open_page(&url),
            Action::AskUninstall(id) => self.ask(Confirm::Uninstall(id)),
            Action::AskClean(name) => self.ask(Confirm::Clean(name)),
            Action::AskCleanAll(name) => self.ask(Confirm::CleanAll(name)),
            Action::AskFix(id) => self.ask(Confirm::Apply(id)),
            Action::AskUndo(seq) => self.ask(Confirm::Undo(seq)),
            Action::CancelConfirm => self.model.confirm = None,
            Action::Confirm => self.confirmed(),
            Action::ToggleTrace(location) => self.toggle_trace(location),
            Action::SelectTraces(all) => self.select_traces(all),
            Action::OpenTraces => self.open_traces(),
            Action::CloseTraces => self.close_traces(),
            Action::AskEmptyQuarantine => self.ask(Confirm::EmptyQuarantine),
            Action::RestoreApp(app) => self.apps_restored(&app),
            Action::AskRestartAsAdmin(name) => self.ask(Confirm::RestartAsAdmin(name)),
            Action::FindTraces(name) => self.find_traces(name),
            Action::RefreshApps => self.refresh_apps(),
            Action::StopWaiting => self.stop_waiting(),
            Action::RestoreBackup(file) => self.backup_restored(&file),
            Action::SetFilter(filter) => self.model.filter = filter,
            Action::SetQuery(query) => self.model.query = query,
            Action::SelectSession(id) => self.select_session(id),
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
            Confirm::Clean(name) => return self.cleaned(&name, false),
            Confirm::CleanAll(name) => return self.cleaned(&name, true),
            Confirm::EmptyQuarantine => return self.quarantine_emptied(),
            Confirm::RestartAsAdmin(name) => return self.restarted_as_admin(&name),
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

    /// Marks as busy if the current page lacks something.
    fn expect_reading(&mut self) {
        let missing = self.missing();
        // A search of offers still going must be picked up, whatever page is shown.
        self.pending = missing || self.model.offers == Offers::Searching || self.watching.is_some();
        self.model.busy = missing;
    }

    /// Whether the page being shown (and the sidebar) lack data.
    fn missing(&self) -> bool {
        let m = &self.model;
        let page_lacks = match m.page {
            Page::Overview => false,
            Page::Fixes => m.fixes.is_none() || m.history.is_none(),
            Page::Apps => {
                m.apps.is_none()
                    || m.finding.is_some()
                    || m.quarantine.is_none()
                    || m.backups.is_none()
                    || self.refreshing_apps
            }
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
        let arrived = self.poll_offers() | self.check_watch();
        if !self.missing() {
            // Only the search of offers is going: nothing to read, nothing blocks.
            self.pending = self.still_polling();
            self.model.busy = false;
            return arrived;
        }
        let errors = self.read_missing();
        if !errors.is_empty() {
            self.model.error = Some(errors.join(" · "));
        }
        self.pending = self.still_polling();
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

    /// A removal of `days` days ago that is still in the quarantine.
    fn removal(seq: u64, days: i64) -> purifex_domain::cleanup_log::CleanupEntry {
        purifex_domain::cleanup_log::CleanupEntry {
            seq,
            app: "Discord".into(),
            on: purifex_domain::date::Date::from_days(FakeClock::DAY.days() - days),
            kept: format!("q/{seq}"),
            ..Default::default()
        }
    }

    #[test]
    fn the_apps_page_reads_the_quarantine_and_lets_go_of_what_expired_once_telling_the_person() {
        let AppsFixture { mut core, log, .. } =
            apps_page_holding(vec![removal(1, 10), removal(2, 2)]);
        assert_eq!(log.borrow().purged, [1], "seven days old: let go");
        assert_eq!(core.model().quarantine.as_ref().map(|q| q.items), Some(1));
        let said = core.model().message.as_deref().unwrap();
        assert!(said.contains("Se soltaron de la cuarentena"), "{said}");
        // Reading again (a rescan) does not tidy again in the same run.
        log.borrow_mut().entries.push(removal(3, 30));
        core.apply(Action::Rescan);
        core.tick();
        assert_eq!(log.borrow().purged, [1]);
        assert_eq!(core.model().quarantine.as_ref().map(|q| q.items), Some(2));
    }

    #[test]
    fn putting_back_an_app_returns_all_its_traces_and_leaves_the_rest_in_the_quarantine() {
        let AppsFixture { mut core, log, .. } =
            apps_page_holding(vec![removal(1, 1), removal(2, 2)]);
        let latest = core
            .model()
            .quarantine
            .as_ref()
            .unwrap()
            .latest
            .clone()
            .unwrap();
        assert_eq!((latest.app.as_str(), latest.items), ("Discord", 2));
        core.apply(Action::RestoreApp("Discord".into()));
        assert_eq!(log.borrow().restored.len(), 2);
        let said = core.model().message.as_deref().unwrap();
        assert!(
            said.contains("Se devolvieron 2 restos de Discord"),
            "{said}"
        );
        core.tick();
        assert_eq!(
            core.model().quarantine.as_ref().map(|q| q.items),
            Some(0),
            "read again: nothing left to put back"
        );
    }

    #[test]
    fn putting_back_an_app_that_the_quarantine_does_not_hold_is_said_and_changes_nothing() {
        let AppsFixture { mut core, log, .. } = apps_page_holding(vec![removal(1, 1)]);
        core.apply(Action::RestoreApp("Otra".into()));
        assert!(log.borrow().restored.is_empty());
        assert!(core
            .model()
            .error
            .as_deref()
            .unwrap()
            .contains("no guarda restos de Otra"));
    }

    #[test]
    fn emptying_the_quarantine_asks_first_and_then_deletes_for_good() {
        let AppsFixture { mut core, log, .. } =
            apps_page_holding(vec![removal(1, 1), removal(2, 3)]);
        core.apply(Action::AskEmptyQuarantine);
        assert_eq!(core.model().confirm, Some(Confirm::EmptyQuarantine));
        assert!(log.borrow().purged.is_empty(), "asking deletes nothing");
        core.apply(Action::CancelConfirm);
        assert!(log.borrow().purged.is_empty());
        core.apply(Action::AskEmptyQuarantine);
        core.apply(Action::Confirm);
        assert_eq!(log.borrow().purged, [1, 2]);
        let said = core.model().message.as_deref().unwrap();
        assert!(said.contains("Se borraron para siempre 2 restos"), "{said}");
        core.tick();
        assert_eq!(
            core.model().quarantine.as_ref().map(|q| q.items),
            Some(0),
            "read again, now empty"
        );
    }

    /// A core whose services keep the given proposals and open pages through `opener`.
    fn core_with_candidates(
        store: crate::testing::FakeCandidates,
        opener: crate::testing::FakeOpener,
    ) -> Core {
        Core::new(Box::new(
            App::new(
                Scanner::new(FakeInventory::with(vec![gpu()]), FakeClock::ticking(1)),
                Recorder::new(
                    FakeSampler::new(0, 1, true),
                    FakeClock::ticking(0),
                    FakeFile::default(),
                ),
                Archive::new(FakeArchive::with(&[])),
            )
            .with_candidates(crate::Candidates::new(Box::new(store)))
            .with_opener(Box::new(opener)),
        ))
    }

    #[test]
    fn the_drivers_page_reads_the_proposals_and_opening_one_goes_through_the_browser() {
        use crate::testing::{FakeCandidates, FakeOpener};
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
        let mut c = core_with_candidates(store, opener.clone());
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
            uninstall: Some("u.exe".into()),
            size_kb: Some(1024),
            ..Default::default()
        }
    }

    /// The apps page of a computer with Discord installed and one trace of it left on disk.
    struct AppsFixture {
        core: Core,
        installed: crate::testing::FakeApps,
        log: std::rc::Rc<std::cell::RefCell<crate::testing::CleanLog>>,
    }

    fn apps_page() -> AppsFixture {
        apps_page_holding(vec![])
    }

    #[test]
    fn restarting_as_an_administrator_asks_first_and_then_closes_the_window() {
        use crate::testing::FakeElevation;
        let elevation = FakeElevation::default();
        let restarted = elevation.restarted.clone();
        let mut core = apps_page_with(vec![], elevation).core;
        core.apply(Action::AskRestartAsAdmin("Discord".into()));
        assert_eq!(
            core.model().confirm,
            Some(Confirm::RestartAsAdmin("Discord".into()))
        );
        assert!(
            restarted.borrow().is_empty(),
            "nothing until it is confirmed"
        );
        core.apply(Action::CancelConfirm);
        assert!(!core.wants_quit() && restarted.borrow().is_empty());
        core.apply(Action::AskRestartAsAdmin("Discord".into()));
        core.apply(Action::Confirm);
        assert_eq!(*restarted.borrow(), [Some("Discord".to_string())]);
        assert!(core.wants_quit());
    }

    #[test]
    fn if_the_person_refuses_the_administrator_prompt_the_window_stays() {
        use crate::testing::FakeElevation;
        let refusing = FakeElevation {
            refuses: true,
            ..Default::default()
        };
        let mut core = apps_page_with(vec![], refusing).core;
        core.apply(Action::AskRestartAsAdmin("Discord".into()));
        core.apply(Action::Confirm);
        assert!(!core.wants_quit());
        assert!(core.model().error.as_deref().unwrap().contains("permiso"));
    }

    #[test]
    fn the_apps_page_reads_whether_the_program_is_an_administrator() {
        use crate::testing::FakeElevation;
        assert_eq!(apps_page().core.model().elevated, Some(false));
        let admin = apps_page_with(
            vec![],
            FakeElevation {
                elevated: true,
                ..Default::default()
            },
        );
        assert_eq!(admin.core.model().elevated, Some(true));
    }

    /// The apps page of a computer whose quarantine already holds these removals.
    fn apps_page_holding(held: Vec<purifex_domain::cleanup_log::CleanupEntry>) -> AppsFixture {
        apps_page_with(held, crate::testing::FakeElevation::default())
    }

    /// The apps page of a computer that is (or is not) running as an administrator.
    fn apps_page_with(
        held: Vec<purifex_domain::cleanup_log::CleanupEntry>,
        elevation: crate::testing::FakeElevation,
    ) -> AppsFixture {
        use crate::testing::{CleanLog, FakeApps, FakeCleaning, FakeFinder};
        use crate::Cleaning;
        use purifex_domain::app_traces::Candidate;
        use std::cell::RefCell;
        use std::rc::Rc;
        let installed = FakeApps::default();
        installed.0.borrow_mut().push(fake_app("Discord"));
        let log = Rc::new(RefCell::new(CleanLog {
            entries: held,
            ..CleanLog::default()
        }));
        let found = vec![Candidate {
            location: r"C:\Users\u\AppData\Roaming\Discord".into(),
            bytes: 10,
            ..Default::default()
        }];
        let mut core = Core::new(Box::new(
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
            .with_cleaning(
                Cleaning::new(
                    Box::new(FakeFinder(found)),
                    Box::new(FakeCleaning(log.clone())),
                    Box::new(FakeCleaning(log.clone())),
                )
                .with_clock(Box::new(FakeClock::ticking(0))),
            )
            .with_elevation(Box::new(elevation)),
        ));
        core.tick();
        core.apply(Action::Go(Page::Apps));
        assert!(core.model().busy, "the apps are read on the tick");
        core.tick();
        assert_eq!(core.model().visible_apps().len(), 1);
        AppsFixture {
            core,
            installed,
            log,
        }
    }

    #[test]
    fn the_uninstaller_starts_only_when_the_person_confirms() {
        let AppsFixture { mut core, log, .. } = apps_page();
        let id = "HKLM64:Discord".to_string();
        core.apply(Action::AskUninstall(id.clone()));
        assert_eq!(core.model().confirm, Some(Confirm::Uninstall(id.clone())));
        assert!(log.borrow().launched.is_empty(), "asking changes nothing");
        core.apply(Action::CancelConfirm);
        assert!(log.borrow().launched.is_empty());
        core.apply(Action::AskUninstall(id));
        core.apply(Action::Confirm);
        assert_eq!(log.borrow().launched, ["Discord"]);
        assert_eq!(core.model().uninstalled.as_deref(), Some("Discord"));
        let message = core.model().message.as_deref().unwrap();
        assert!(message.contains("busca sus restos"));
    }

    #[test]
    fn while_the_app_is_installed_its_traces_can_be_looked_at_but_not_removed() {
        let AppsFixture { mut core, log, .. } = apps_page();
        core.apply(Action::FindTraces("Discord".into()));
        assert!(core.model().busy);
        core.tick();
        assert_eq!(core.model().traces.as_ref().unwrap().proposed.len(), 1);
        core.apply(Action::AskClean("Discord".into()));
        core.apply(Action::Confirm);
        let error = core.model().error.as_deref().unwrap();
        assert!(error.contains("sigue instalada"));
        assert!(log.borrow().removed.is_empty());
    }

    #[test]
    fn once_the_uninstaller_has_finished_the_traces_go_to_the_quarantine() {
        let AppsFixture {
            mut core,
            installed,
            log,
        } = apps_page();
        core.apply(Action::FindTraces("Discord".into()));
        core.tick();
        installed.0.borrow_mut().clear();
        core.apply(Action::AskClean("Discord".into()));
        core.apply(Action::Confirm);
        assert_eq!(log.borrow().removed.len(), 1);
        let message = core.model().message.as_deref().unwrap();
        assert!(message.contains("Se quitaron 1 restos"));
        assert!(core.model().traces.is_none());
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

    fn apps_core(
        installed: &crate::testing::FakeApps,
    ) -> (
        Core,
        std::rc::Rc<std::cell::RefCell<crate::testing::CleanLog>>,
    ) {
        use crate::testing::{CleanLog, FakeCleaning, FakeFinder};
        use crate::Cleaning;
        use purifex_domain::app_traces::{Candidate, TraceKind};
        let log = std::rc::Rc::new(std::cell::RefCell::new(CleanLog::default()));
        let found = vec![Candidate {
            kind: TraceKind::Folder,
            location: r"C:\Program Files\obs-studio".into(),
            bytes: 10,
            ..Default::default()
        }];
        let core = Core::new(Box::new(
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
        (core, log)
    }

    #[test]
    fn coming_back_to_the_window_reads_the_apps_again_and_an_app_that_vanished_shows_its_traces() {
        use crate::testing::FakeApps;
        let installed = FakeApps::default();
        installed
            .0
            .borrow_mut()
            .extend([fake_app("OBS Studio"), fake_app("Git")]);
        let (mut c, log) = apps_core(&installed);
        c.tick();
        c.apply(Action::Go(Page::Apps));
        c.tick();
        assert_eq!(c.model().visible_apps().len(), 2);
        // Nothing changed: coming back reads again and finds nothing new.
        c.apply(Action::RefreshApps);
        c.tick();
        assert_eq!(c.model().uninstalled, None);
        // The user uninstalled OBS in its own window and comes back.
        installed.0.borrow_mut().retain(|a| a.name != "OBS Studio");
        c.apply(Action::RefreshApps);
        assert_eq!(
            c.model().visible_apps().len(),
            2,
            "the list stays as it is until the new one is read"
        );
        assert!(!c.model().busy, "no loading state shows");
        assert!(c.needs_tick());
        c.tick();
        assert_eq!(c.model().visible_apps().len(), 1, "OBS is no longer listed");
        assert_eq!(c.model().uninstalled.as_deref(), Some("OBS Studio"));
        assert!(c
            .model()
            .message
            .as_deref()
            .unwrap()
            .contains("ya no está instalada"));
        assert_eq!(
            c.model().traces.as_ref().unwrap().proposed.len(),
            1,
            "its traces were looked for at once"
        );
        assert_eq!(log.borrow().remembered.len(), 1);
        assert!(log.borrow().removed.is_empty(), "looking never removes");
    }

    #[test]
    fn a_protected_app_that_changes_id_when_it_updates_is_not_taken_for_an_uninstalled_one() {
        use crate::testing::FakeApps;
        let installed = FakeApps::default();
        let mut runtime = fake_app("Microsoft .NET Runtime 8.0.1");
        runtime.publisher = Some("Microsoft Corporation".into());
        installed.0.borrow_mut().push(runtime);
        let (mut c, _) = apps_core(&installed);
        c.tick();
        c.apply(Action::Go(Page::Apps));
        c.tick();
        let mut updated = fake_app("Microsoft .NET Runtime 8.0.2");
        updated.publisher = Some("Microsoft Corporation".into());
        *installed.0.borrow_mut() = vec![updated];
        c.apply(Action::RefreshApps);
        c.tick();
        assert_eq!(c.model().uninstalled, None);
    }

    #[test]
    fn the_search_belongs_to_the_page_it_was_typed_in_and_looking_for_a_name_shows_the_panel() {
        use crate::testing::FakeApps;
        let installed = FakeApps::default();
        let (mut c, _) = apps_core(&installed);
        c.tick();
        c.apply(Action::Go(Page::Drivers));
        c.apply(Action::SetQuery("nvidia".into()));
        c.apply(Action::Go(Page::Apps));
        assert_eq!(
            c.model().query,
            "",
            "the drivers' search does not hide the apps"
        );
        c.apply(Action::FindTraces("OBS Studio".into()));
        c.tick();
        assert_eq!(c.model().uninstalled.as_deref(), Some("OBS Studio"));
        assert_eq!(c.model().traces.as_ref().unwrap().proposed.len(), 1);
    }

    fn two_traces_core(
        installed: &crate::testing::FakeApps,
    ) -> (
        Core,
        std::rc::Rc<std::cell::RefCell<crate::testing::CleanLog>>,
    ) {
        use crate::testing::{CleanLog, FakeCleaning, FakeFinder};
        use crate::Cleaning;
        use purifex_domain::app_traces::{Candidate, TraceKind};
        let log = std::rc::Rc::new(std::cell::RefCell::new(CleanLog::default()));
        let found = ["obs-studio", "OBS Studio"]
            .map(|n| Candidate {
                kind: TraceKind::Folder,
                location: format!(r"C:\ProgramData\{n}"),
                bytes: 10,
                ..Default::default()
            })
            .to_vec();
        let core = Core::new(Box::new(
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
        (core, log)
    }

    #[test]
    fn when_the_uninstaller_finishes_the_list_of_traces_opens_by_itself() {
        use crate::testing::FakeApps;
        let installed = FakeApps::default();
        installed.0.borrow_mut().push(fake_app("OBS Studio"));
        let (mut c, _) = two_traces_core(&installed);
        c.tick();
        c.apply(Action::Go(Page::Apps));
        c.tick();
        c.apply(Action::AskUninstall("HKLM64:OBS Studio".into()));
        c.apply(Action::Confirm);
        assert!(
            c.needs_tick(),
            "it keeps looking while the uninstaller runs"
        );
        // Still installed: nothing happens, however many times it looks.
        for _ in 0..5 {
            std::thread::sleep(Duration::from_millis(15));
            assert!(!c.tick());
        }
        assert!(!c.model().traces_open);
        // The uninstaller finishes.
        installed.0.borrow_mut().clear();
        let mut changed = false;
        for _ in 0..20 {
            std::thread::sleep(Duration::from_millis(15));
            changed |= c.tick();
            if c.model().traces_open {
                break;
            }
        }
        assert!(
            changed && c.model().traces_open,
            "the window opens by itself"
        );
        assert_eq!(c.model().traces.as_ref().unwrap().proposed.len(), 2);
        assert!(c.model().unchecked.is_empty(), "everything starts checked");
        assert!(c
            .model()
            .message
            .as_deref()
            .unwrap()
            .contains("ya se desinstaló"));
        assert!(!c.needs_tick(), "it stopped watching");
    }

    #[test]
    fn waiting_for_an_uninstaller_can_be_given_up_and_a_very_long_wait_says_so() {
        use crate::testing::FakeApps;
        let installed = FakeApps::default();
        installed.0.borrow_mut().push(fake_app("OBS Studio"));
        let (mut c, _) = two_traces_core(&installed);
        c.tick();
        c.apply(Action::Go(Page::Apps));
        c.tick();
        c.apply(Action::AskUninstall("HKLM64:OBS Studio".into()));
        c.apply(Action::Confirm);
        assert!(c.model().waiting);
        c.apply(Action::StopWaiting);
        assert!(!c.model().waiting && !c.needs_tick());
        // The wait that is never answered ends by itself and says what happened.
        c.apply(Action::AskUninstall("HKLM64:OBS Studio".into()));
        c.apply(Action::Confirm);
        let long_ago = Instant::now() - WATCH_FOR - Duration::from_secs(1);
        let (app, _, _) = c.watching.take().unwrap();
        c.watching = Some((app, long_ago, long_ago));
        assert!(c.tick());
        assert!(!c.model().waiting && !c.needs_tick());
        assert!(c
            .model()
            .message
            .as_deref()
            .unwrap()
            .contains("sigue instalada"));
    }

    #[test]
    fn only_the_checked_traces_go_unless_everything_is_asked_for() {
        use crate::testing::FakeApps;
        let installed = FakeApps::default();
        let (mut c, log) = two_traces_core(&installed);
        c.tick();
        c.apply(Action::Go(Page::Apps));
        c.tick();
        c.apply(Action::FindTraces("OBS Studio".into()));
        c.tick();
        assert!(c.model().traces_open);
        let first = r"C:\ProgramData\obs-studio".to_string();
        // Uncheck one, remove the rest.
        c.apply(Action::ToggleTrace(first.clone()));
        assert_eq!(c.model().unchecked, std::slice::from_ref(&first));
        c.apply(Action::AskClean("OBS Studio".into()));
        c.apply(Action::Confirm);
        assert_eq!(
            log.borrow().removed,
            [r"C:\ProgramData\OBS Studio"],
            "the unchecked one stayed"
        );
        assert!(!c.model().traces_open);
        // "Eliminar todo rastro" ignores the checkboxes.
        c.apply(Action::FindTraces("OBS Studio".into()));
        c.tick();
        c.apply(Action::SelectTraces(false));
        assert_eq!(c.model().unchecked.len(), 2);
        c.apply(Action::AskCleanAll("OBS Studio".into()));
        assert_eq!(
            c.model().confirm,
            Some(Confirm::CleanAll("OBS Studio".into()))
        );
        c.apply(Action::Confirm);
        assert_eq!(
            log.borrow().removed.len(),
            3,
            "the two of this round, with the one left before"
        );
    }

    #[test]
    fn with_nothing_checked_nothing_is_removed_and_the_user_is_told() {
        use crate::testing::FakeApps;
        let installed = FakeApps::default();
        let (mut c, log) = two_traces_core(&installed);
        c.tick();
        c.apply(Action::Go(Page::Apps));
        c.tick();
        c.apply(Action::FindTraces("OBS Studio".into()));
        c.tick();
        c.apply(Action::SelectTraces(false));
        c.apply(Action::AskClean("OBS Studio".into()));
        c.apply(Action::Confirm);
        assert!(log.borrow().removed.is_empty());
        assert_eq!(
            c.model().message.as_deref(),
            Some("No hay ningún resto marcado.")
        );
        c.apply(Action::SelectTraces(true));
        assert!(c.model().unchecked.is_empty());
        c.apply(Action::CloseTraces);
        assert!(!c.model().traces_open);
        c.apply(Action::OpenTraces);
        assert!(
            c.model().traces_open,
            "it can be opened again while the list exists"
        );
    }
}
