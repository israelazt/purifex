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

use purifex_domain::app_list::AppFilter;
use purifex_domain::driver_offers::Offers;
use purifex_domain::view::{Action, Confirm, Model, Page};

use crate::services::Services;

mod actions;
mod apps;
mod batch;
mod hunting;
mod junk;
mod monitoring;
mod reading;
mod startup;

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
        let chosen = services.app_list_settings();
        Self {
            services,
            model: Model {
                busy: true,
                app_filter: chosen.filter,
                app_sort: chosen.sort,
                app_view: chosen.view,
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
            Action::AskDeleteAtReboot(name) => self.ask(Confirm::DeleteAtReboot(name)),
            Action::SetAppFilter(filter) => self.app_list_chosen(|m| m.app_filter = filter),
            Action::ClearAppFilters => {
                self.model.query.clear();
                self.app_list_chosen(|m| m.app_filter = AppFilter::All);
            }
            Action::SetAppSort(sort) => self.app_list_chosen(|m| m.app_sort = sort),
            Action::SetAppView(view) => self.app_list_chosen(|m| m.app_view = view),
            Action::SelectApp(id) => self.select_app(id),
            Action::OpenFolder(path) => self.open_app_folder(&path),
            Action::OpenRegistryKey(id) => self.open_app_registry_key(&id),
            Action::OpenWebsite(url) => self.open_app_website(&url),
            Action::AskDeleteBackup(copy) => self.ask(Confirm::DeleteBackup(copy)),
            Action::RestoreAll(app, copy) => self.everything_restored(&app, &copy),
            Action::AskForceUninstall(id) => self.ask(Confirm::ForceUninstall(id)),
            Action::AskCloseProcesses(name) => self.ask(Confirm::CloseProcesses(name)),
            Action::SetScanMode(mode) => self.scan_mode_set(mode),
            Action::FindTraces(name) => self.find_traces(name),
            Action::RefreshApps => self.refresh_apps(),
            Action::StopWaiting => self.stop_waiting(),
            Action::RestoreBackup(file) => self.backup_restored(&file),
            Action::SetFilter(filter) => self.model.filter = filter,
            Action::SetQuery(query) => self.model.query = query,
            Action::SelectSession(id) => self.select_session(id),
            other => self.apply_apps_tools(other),
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
            Confirm::UninstallMarked => return self.batch_started(),
            Confirm::DeleteLog(id) => return self.log_deleted(&id),
            Confirm::RemoveStartup(id) => return self.startup_removed(&id),
            Confirm::CleanJunk => return self.junk_cleaned(),
            Confirm::Clean(name) => return self.cleaned(&name, false),
            Confirm::CleanAll(name) => return self.cleaned(&name, true),
            Confirm::EmptyQuarantine => return self.quarantine_emptied(),
            Confirm::RestartAsAdmin(name) => return self.restarted_as_admin(&name),
            Confirm::DeleteAtReboot(name) => return self.deleted_at_reboot(&name),
            Confirm::DeleteBackup(copy) => return self.backup_deleted(&copy),
            Confirm::ForceUninstall(id) => return self.force_uninstalled(&id),
            Confirm::CloseProcesses(name) => return self.programs_closed(&name),
            Confirm::CloseHunted(pid) => return self.hunted_closed(pid),
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
                    || m.install_logs.is_none()
                    || self.refreshing_apps
            }
            Page::Startup => m.startup.is_none() || m.quarantine.is_none(),
            Page::Junk => m.junk.is_none(),
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
        two_traces_core_running(installed, crate::testing::FakeProcesses::default())
    }

    fn two_traces_core_running(
        installed: &crate::testing::FakeApps,
        processes: crate::testing::FakeProcesses,
    ) -> (
        Core,
        std::rc::Rc<std::cell::RefCell<crate::testing::CleanLog>>,
    ) {
        use purifex_domain::app_traces::{Candidate, TraceKind};
        let found = ["obs-studio", "OBS Studio"]
            .map(|n| Candidate {
                kind: TraceKind::Folder,
                location: format!(r"C:\ProgramData\{n}"),
                bytes: 10,
                ..Default::default()
            })
            .to_vec();
        core_finding(installed, processes, found)
    }

    /// A core whose finder always finds `found`.
    fn core_finding(
        installed: &crate::testing::FakeApps,
        processes: crate::testing::FakeProcesses,
        found: Vec<purifex_domain::app_traces::Candidate>,
    ) -> (
        Core,
        std::rc::Rc<std::cell::RefCell<crate::testing::CleanLog>>,
    ) {
        use crate::testing::{CleanLog, FakeCleaning, FakeFinder};
        use crate::Cleaning;
        let log = std::rc::Rc::new(std::cell::RefCell::new(CleanLog::default()));
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
            .with_cleaning(
                Cleaning::new(
                    Box::new(FakeFinder(found)),
                    Box::new(FakeCleaning(log.clone())),
                    Box::new(FakeCleaning(log.clone())),
                )
                .with_processes(Box::new(processes)),
            ),
        ));
        (core, log)
    }

    #[test]
    fn the_list_of_apps_is_filtered_and_sorted_from_the_model_and_judges_recent_by_the_clock() {
        use crate::testing::FakeApps;
        use purifex_domain::app_list::{AppFilter, AppSort};
        let installed = FakeApps::default();
        let day = |n: i64| {
            Some(purifex_domain::date::Date::from_days(FakeClock::DAY.days() - n).unwrap())
        };
        let app = |name: &str, kb, on| purifex_domain::apps::InstalledApp {
            id: format!("HKLM64:{name}"),
            name: name.into(),
            uninstall: Some("u.exe".into()),
            size_kb: Some(kb),
            install_date: on,
            ..Default::default()
        };
        *installed.0.borrow_mut() = vec![
            app("Old", 900, day(400)),
            app("Fresh", 100, day(3)),
            app("Mid", 500, day(20)),
        ];
        let (mut c, _log) = two_traces_core(&installed);
        c.tick();
        c.apply(Action::Go(Page::Apps));
        c.tick();
        let names = |c: &Core| -> Vec<String> {
            c.model()
                .visible_apps()
                .iter()
                .map(|a| a.name.clone())
                .collect()
        };
        assert_eq!(names(&c), ["Fresh", "Mid", "Old"]);
        c.apply(Action::SetAppSort(AppSort::Size));
        assert_eq!(names(&c), ["Old", "Mid", "Fresh"]);
        c.apply(Action::SetAppSort(AppSort::Date));
        assert_eq!(names(&c), ["Fresh", "Mid", "Old"]);
        c.apply(Action::SetAppFilter(AppFilter::Recent));
        assert_eq!(names(&c), ["Fresh", "Mid"]);
        c.apply(Action::SetAppFilter(AppFilter::Store));
        assert!(names(&c).is_empty());
    }

    #[test]
    fn how_the_list_of_apps_looks_is_kept_and_is_there_the_next_time() {
        use crate::testing::FakeAppList;
        use purifex_domain::app_list::{AppFilter, AppSort, AppView};
        let store = FakeAppList::default();
        let build = |store: &FakeAppList| {
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
                .with_app_list_store(Box::new(store.clone())),
            ))
        };
        let mut first = build(&store);
        first.apply(Action::SetAppFilter(AppFilter::Programs));
        first.apply(Action::SetAppSort(AppSort::Size));
        first.apply(Action::SetAppView(AppView::List));
        let next = build(&store);
        let m = next.model();
        assert_eq!(
            (m.app_filter, m.app_sort, m.app_view),
            (AppFilter::Programs, AppSort::Size, AppView::List)
        );
        // Without a store the choice still applies for this run.
        let mut plain = Core::new(Box::new(App::new(
            Scanner::new(FakeInventory::with(vec![gpu()]), FakeClock::ticking(1)),
            Recorder::new(
                FakeSampler::new(0, 1, true),
                FakeClock::ticking(0),
                FakeFile::default(),
            ),
            Archive::new(FakeArchive::with(&[])),
        )));
        plain.apply(Action::SetAppView(AppView::List));
        assert_eq!(plain.model().app_view, AppView::List);
    }

    #[test]
    fn a_forced_uninstall_asks_first_then_removes_the_entry_and_shows_what_is_left() {
        use crate::testing::FakeApps;
        let installed = FakeApps::default();
        *installed.0.borrow_mut() = vec![purifex_domain::apps::InstalledApp {
            id: "HKLM64:OBS Studio".into(),
            name: "OBS Studio".into(),
            publisher: Some("OBS".into()),
            uninstall: Some("broken.exe".into()),
            ..Default::default()
        }];
        let (mut c, log) = two_traces_core(&installed);
        c.tick();
        c.apply(Action::Go(Page::Apps));
        c.tick();
        c.apply(Action::AskForceUninstall("HKLM64:OBS Studio".into()));
        assert_eq!(
            c.model().confirm,
            Some(Confirm::ForceUninstall("HKLM64:OBS Studio".into()))
        );
        c.apply(Action::CancelConfirm);
        assert!(log.borrow().removed.is_empty() && log.borrow().launched.is_empty());
        c.apply(Action::AskForceUninstall("HKLM64:OBS Studio".into()));
        c.apply(Action::Confirm);
        let removed = log.borrow().removed.clone();
        assert_eq!(removed.len(), 1);
        assert!(removed[0].ends_with(r"Uninstall\OBS Studio"), "{removed:?}");
        assert!(log.borrow().launched.is_empty(), "no uninstaller ran");
        assert_eq!(c.model().uninstalled.as_deref(), Some("OBS Studio"));
        let said = c.model().message.clone().unwrap();
        assert!(said.contains("sin su desinstalador"), "{said}");
    }

    #[test]
    fn in_advanced_mode_a_name_that_only_resembles_the_app_is_listed_unchecked_and_never_taken_by_everything(
    ) {
        use crate::testing::{FakeApps, FakeProcesses};
        use purifex_domain::app_traces::{Candidate, Evidence, TraceKind};
        use purifex_domain::scan_mode::ScanMode;
        let folder = |n: &str| Candidate {
            kind: TraceKind::Folder,
            location: format!(r"C:\ProgramData\{n}"),
            bytes: 10,
            ..Default::default()
        };
        let found = vec![folder("OBS Studio"), folder("OBS Studio Beta")];
        let (mut c, log) = core_finding(&FakeApps::default(), FakeProcesses::default(), found);
        c.tick();
        c.apply(Action::Go(Page::Apps));
        c.tick();
        let listed = |c: &Core| c.model().traces.as_ref().unwrap().proposed.len();
        c.apply(Action::FindTraces("OBS Studio".into()));
        c.tick();
        assert_eq!(listed(&c), 1, "Moderate does not guess");
        c.apply(Action::SetScanMode(ScanMode::Advanced));
        c.tick();
        let report = c.model().traces.clone().unwrap();
        assert_eq!(listed(&c), 2);
        let guess = report
            .proposed
            .iter()
            .find(|t| t.evidence == Evidence::PartialName)
            .expect("the resemblance is a guess");
        assert!(guess.location.ends_with("OBS Studio Beta"));
        assert_eq!(
            c.model().unchecked,
            [guess.key()],
            "a guess starts unchecked"
        );
        // «Everything» leaves the guess alone; checking it takes it too.
        c.apply(Action::AskCleanAll("OBS Studio".into()));
        c.apply(Action::Confirm);
        assert_eq!(log.borrow().removed, [r"C:\ProgramData\OBS Studio"]);
    }

    /// A core that has the copies of `backups` and nothing else to clean.
    fn core_with_copies(backups: &crate::testing::FakeBackups) -> Core {
        use crate::testing::{CleanLog, FakeApps, FakeCleaning, FakeFinder};
        let log = std::rc::Rc::new(std::cell::RefCell::new(CleanLog::default()));
        let cleaning = crate::Cleaning::new(
            Box::new(FakeFinder(vec![])),
            Box::new(FakeCleaning(log.clone())),
            Box::new(FakeCleaning(log)),
        )
        .with_backups(Box::new(backups.clone()));
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
            .with_apps(Box::new(FakeApps::default()))
            .with_cleaning(cleaning),
        ))
    }

    #[test]
    fn a_copy_is_deleted_only_after_a_warning_and_everything_is_put_back_in_one_step() {
        use crate::testing::FakeBackups;
        use purifex_domain::safeguard::BackupEntry;
        let backups = FakeBackups::default();
        backups.entries.borrow_mut().push(BackupEntry {
            app: "OBS Studio".into(),
            key: "HKCU".into(),
            registry_copy: "obs.reg".into(),
            on: None,
        });
        let mut c = core_with_copies(&backups);
        c.tick();
        c.apply(Action::Go(Page::Apps));
        c.tick();
        // Everything at once: the registration comes back even if there is nothing to put back
        // from the quarantine, and the message says both.
        c.apply(Action::RestoreAll("OBS Studio".into(), "obs.reg".into()));
        assert_eq!(*backups.restored.borrow(), ["OBS Studio"]);
        let said = c.model().message.clone().unwrap();
        assert!(said.contains("registrada otra vez"), "{said}");
        // Deleting asks first, and saying no keeps the copy.
        let ask = || Action::AskDeleteBackup("obs.reg".into());
        c.apply(ask());
        assert_eq!(
            c.model().confirm,
            Some(Confirm::DeleteBackup("obs.reg".into()))
        );
        c.apply(Action::CancelConfirm);
        assert_eq!(backups.entries.borrow().len(), 1);
        c.apply(ask());
        c.apply(Action::Confirm);
        assert!(backups.entries.borrow().is_empty());
        let said = c.model().message.clone().unwrap();
        assert!(said.contains("Se borró la copia"), "{said}");
    }

    #[test]
    fn an_app_opens_its_details_and_only_what_the_list_registered_is_ever_opened() {
        use crate::testing::FakeApps;
        let installed = FakeApps::default();
        *installed.0.borrow_mut() = vec![purifex_domain::apps::InstalledApp {
            id: "HKLM64:Tool".into(),
            name: "Tool".into(),
            uninstall: Some("u.exe".into()),
            install_location: Some(r"C:\Program Files\Tool".into()),
            website: Some("https://tool.example".into()),
            ..Default::default()
        }];
        let (mut c, _log) = two_traces_core(&installed);
        c.tick();
        c.apply(Action::Go(Page::Apps));
        c.tick();
        c.apply(Action::SelectApp("HKLM64:Tool".into()));
        assert_eq!(c.model().app_selected.as_deref(), Some("HKLM64:Tool"));
        c.apply(Action::SelectApp("HKLM64:Tool".into()));
        assert_eq!(c.model().app_selected, None, "a second click closes them");
        // What was not registered by an app of the list is refused before any opener is asked.
        c.apply(Action::OpenFolder(r"C:\Windows".into()));
        assert!(c.model().error.as_deref().unwrap().contains("ninguna app"));
        c.apply(Action::OpenWebsite("https://evil.example".into()));
        assert!(c.model().error.as_deref().unwrap().contains("ninguna app"));
        // The key comes from the app of the list: an id that is not one has none to show.
        c.apply(Action::OpenRegistryKey("HKLM64:Nothing".into()));
        assert!(c
            .model()
            .error
            .as_deref()
            .unwrap()
            .contains("no está registrada"));
        c.apply(Action::OpenRegistryKey("HKLM64:Tool".into()));
        assert!(c
            .model()
            .error
            .as_deref()
            .unwrap()
            .contains("no puede abrir"));
        // What was passes the guard (this core has no opener, and says so).
        c.apply(Action::OpenFolder(r"C:\Program Files\Tool".into()));
        assert!(c
            .model()
            .error
            .as_deref()
            .unwrap()
            .contains("no puede abrir"));
    }

    #[test]
    fn the_programs_a_forced_app_still_runs_are_listed_and_closed_only_after_a_warning() {
        use crate::testing::{FakeApps, FakeProcesses};
        const OLD: &str = r"D:\Games\OBS Studio";
        let processes = FakeProcesses::with(&[
            (10, r"D:\Games\OBS Studio\bin\obs64.exe"),
            (11, r"D:\Games\OBS Studio\helper.exe"),
            (12, r"D:\Games\OBS Studio Two\other.exe"),
            (13, r"C:\Windows\explorer.exe"),
        ]);
        let (mut c, _log) = two_traces_core_running(&FakeApps::default(), processes.clone());
        c.tick();
        c.apply(Action::Go(Page::Apps));
        c.tick();
        c.apply(Action::FindTraces(OLD.into()));
        c.tick();
        let running = c.model().running.clone().expect("it says what still runs");
        assert_eq!(running.names, ["helper.exe", "obs64.exe"]);
        // Asking is not enough, and saying no closes nothing.
        c.apply(Action::AskCloseProcesses(OLD.into()));
        assert_eq!(c.model().confirm, Some(Confirm::CloseProcesses(OLD.into())));
        c.apply(Action::CancelConfirm);
        assert!(processes.closed.borrow().is_empty());
        c.apply(Action::AskCloseProcesses(OLD.into()));
        c.apply(Action::Confirm);
        c.tick();
        let mut closed = processes.closed.borrow().clone();
        closed.sort_unstable();
        assert_eq!(closed, [10, 11], "only what runs from inside the folder");
        assert!(c.model().running.is_none());
        assert!(c
            .model()
            .message
            .as_deref()
            .unwrap()
            .contains("Se cerraron 2"));
    }

    #[test]
    fn a_name_without_a_folder_offers_to_close_nothing() {
        use crate::testing::{FakeApps, FakeProcesses};
        let processes = FakeProcesses::with(&[(10, r"D:\Games\OBS Studio\obs64.exe")]);
        let (mut c, _log) = two_traces_core_running(&FakeApps::default(), processes);
        c.tick();
        c.apply(Action::Go(Page::Apps));
        c.tick();
        c.apply(Action::FindTraces("OBS Studio".into()));
        c.tick();
        assert!(c.model().running.is_none());
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
    fn what_is_open_is_offered_for_the_next_restart_only_after_a_warning_and_is_never_quarantined()
    {
        use crate::testing::FakeApps;
        let installed = FakeApps::default();
        let (mut c, log) = two_traces_core(&installed);
        let open = r"C:\ProgramData\obs-studio".to_string();
        log.borrow_mut().stuck.push(open.clone());
        c.tick();
        c.apply(Action::Go(Page::Apps));
        c.tick();
        c.apply(Action::FindTraces("OBS Studio".into()));
        c.tick();
        c.apply(Action::AskCleanAll("OBS Studio".into()));
        c.apply(Action::Confirm);
        assert_eq!(log.borrow().removed, [r"C:\ProgramData\OBS Studio"]);
        let in_use = c.model().in_use.clone().expect("it says what is open");
        assert_eq!((in_use.app.as_str(), in_use.items), ("OBS Studio", 1));
        assert!(
            log.borrow().scheduled.is_empty(),
            "nothing until it is confirmed"
        );
        // Asking is not enough: the warning comes first, and saying no changes nothing.
        c.apply(Action::AskDeleteAtReboot("OBS Studio".into()));
        assert_eq!(
            c.model().confirm,
            Some(Confirm::DeleteAtReboot("OBS Studio".into()))
        );
        c.apply(Action::CancelConfirm);
        assert!(log.borrow().scheduled.is_empty() && c.model().in_use.is_some());
        c.apply(Action::AskDeleteAtReboot("OBS Studio".into()));
        c.apply(Action::Confirm);
        assert_eq!(log.borrow().scheduled, [open]);
        assert!(c.model().in_use.is_none());
        assert!(c.model().message.as_deref().unwrap().contains("reinicies"));
        let line = log.borrow().entries.last().cloned().unwrap();
        assert_eq!(
            line.state,
            purifex_domain::cleanup_log::CleanupState::Scheduled
        );
    }

    #[test]
    fn a_refused_delete_at_restart_says_why_and_keeps_the_offer() {
        use crate::testing::FakeApps;
        let installed = FakeApps::default();
        let (mut c, log) = two_traces_core(&installed);
        log.borrow_mut()
            .stuck
            .push(r"C:\ProgramData\obs-studio".into());
        log.borrow_mut().refuse_reboot = true;
        c.tick();
        c.apply(Action::Go(Page::Apps));
        c.tick();
        c.apply(Action::FindTraces("OBS Studio".into()));
        c.tick();
        c.apply(Action::AskCleanAll("OBS Studio".into()));
        c.apply(Action::Confirm);
        c.apply(Action::AskDeleteAtReboot("OBS Studio".into()));
        c.apply(Action::Confirm);
        assert!(c
            .model()
            .message
            .as_deref()
            .unwrap()
            .contains("administrador"));
        assert!(
            c.model().in_use.is_some(),
            "it can be tried again as administrator"
        );
    }

    #[test]
    fn changing_the_mode_looks_again_and_the_safe_mode_hides_what_is_known_by_its_name() {
        use crate::testing::FakeApps;
        use purifex_domain::scan_mode::ScanMode;
        let installed = FakeApps::default();
        let (mut c, _) = two_traces_core(&installed);
        c.tick();
        c.apply(Action::Go(Page::Apps));
        c.tick();
        c.apply(Action::FindTraces("OBS Studio".into()));
        c.tick();
        assert_eq!(c.model().traces.as_ref().unwrap().proposed.len(), 2);
        assert_eq!(c.model().scan_mode, ScanMode::Moderate);
        c.apply(Action::SetScanMode(ScanMode::Safe));
        c.tick();
        assert_eq!(c.model().scan_mode, ScanMode::Safe);
        assert!(c.model().traces.as_ref().unwrap().proposed.is_empty());
        c.apply(Action::SetScanMode(ScanMode::Moderate));
        c.tick();
        assert_eq!(c.model().traces.as_ref().unwrap().proposed.len(), 2);
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

    /// A core on the apps page that knows OBS Studio, with these programs running and
    /// a sight that lands on whatever `picker` holds.
    fn hunting_core(
        processes: &crate::testing::FakeProcesses,
        picker: &crate::testing::FakeWindowPicker,
    ) -> Core {
        use crate::testing::{CleanLog, FakeApps, FakeCleaning, FakeFinder};
        use purifex_domain::apps::InstalledApp;
        let obs = InstalledApp {
            id: "HKLM:OBS".into(),
            name: "OBS Studio".into(),
            uninstall: Some(r#""D:\Games\OBS Studio\unins000.exe""#.into()),
            install_location: Some(r"D:\Games\OBS Studio".into()),
            ..Default::default()
        };
        let log = std::rc::Rc::new(std::cell::RefCell::new(CleanLog::default()));
        let installed = FakeApps(std::rc::Rc::new(std::cell::RefCell::new(vec![obs])));
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
            .with_window_picker(Box::new(picker.clone()))
            .with_cleaning(
                crate::Cleaning::new(
                    Box::new(FakeFinder(vec![])),
                    Box::new(FakeCleaning(log.clone())),
                    Box::new(FakeCleaning(log)),
                )
                .with_processes(Box::new(processes.clone())),
            ),
        ));
        c.tick();
        c.apply(Action::Go(Page::Apps));
        c.tick();
        c
    }

    fn drop_sight_on(
        c: &mut Core,
        picker: &crate::testing::FakeWindowPicker,
        pid: u32,
        path: &str,
    ) {
        *picker.0.borrow_mut() = Some(purifex_domain::forced::RunningProcess {
            pid,
            path: path.into(),
        });
        c.apply(Action::StartHunt);
        c.apply(Action::DropHunt);
    }

    #[test]
    fn the_sight_says_which_app_a_window_belongs_to_and_a_drop_with_nothing_held_does_nothing() {
        use crate::testing::{FakeProcesses, FakeWindowPicker};
        let (processes, picker) = (FakeProcesses::default(), FakeWindowPicker::default());
        let mut c = hunting_core(&processes, &picker);
        c.apply(Action::DropHunt);
        assert!(c.model().hunt.found.is_none());
        *picker.0.borrow_mut() = None;
        c.apply(Action::StartHunt);
        assert!(c.model().hunt.aiming);
        c.apply(Action::DropHunt);
        assert!(!c.model().hunt.aiming && c.model().hunt.found.is_none());
        assert!(
            c.model().message.is_some(),
            "it says nothing could be seen there"
        );
        drop_sight_on(&mut c, &picker, 10, r"D:\Games\OBS Studio\bin\obs64.exe");
        let found = c
            .model()
            .hunt
            .found
            .clone()
            .expect("it landed on a program");
        assert_eq!(found.app.map(|a| a.name).as_deref(), Some("OBS Studio"));
        drop_sight_on(&mut c, &picker, 14, r"D:\Tools\notes.exe");
        assert!(c.model().hunt.found.as_ref().unwrap().app.is_none());
        c.apply(Action::CloseHunt);
        assert!(c.model().hunt.found.is_none());
    }

    #[test]
    fn a_program_the_sight_landed_on_is_closed_only_after_a_warning_and_windows_never() {
        use crate::testing::{FakeProcesses, FakeWindowPicker};
        let processes = FakeProcesses::with(&[
            (10, r"D:\Games\OBS Studio\bin\obs64.exe"),
            (13, r"C:\Windows\explorer.exe"),
        ]);
        let picker = FakeWindowPicker::default();
        let mut c = hunting_core(&processes, &picker);
        drop_sight_on(&mut c, &picker, 10, r"D:\Games\OBS Studio\bin\obs64.exe");
        c.apply(Action::AskCloseHunted);
        assert_eq!(c.model().confirm, Some(Confirm::CloseHunted(10)));
        c.apply(Action::CancelConfirm);
        assert!(
            processes.closed.borrow().is_empty(),
            "saying no closes nothing"
        );
        c.apply(Action::AskCloseHunted);
        c.apply(Action::Confirm);
        assert_eq!(*processes.closed.borrow(), [10]);
        assert!(c.model().hunt.found.is_none());
        // Windows' own programs are refused before anything is asked.
        drop_sight_on(&mut c, &picker, 13, r"C:\Windows\explorer.exe");
        c.apply(Action::AskCloseHunted);
        assert!(c.model().confirm.is_none());
        assert!(c.model().error.as_deref().unwrap().contains("explorer.exe"));
        assert_eq!(*processes.closed.borrow(), [10]);
    }

    #[test]
    fn what_could_not_be_removed_comes_back_unchecked_with_why_and_everything_leaves_it_alone() {
        use crate::testing::FakeApps;
        let (mut c, log) = two_traces_core(&FakeApps::default());
        log.borrow_mut().stuck = vec![r"C:\ProgramData\OBS Studio".into()];
        c.tick();
        c.apply(Action::Go(Page::Apps));
        c.tick();
        c.apply(Action::FindTraces("OBS Studio".into()));
        c.tick();
        c.apply(Action::AskCleanAll("OBS Studio".into()));
        c.apply(Action::Confirm);
        assert_eq!(c.model().stuck.len(), 1);
        assert!(c.model().stuck[0].1.contains("en uso"));
        // Looking again lists it unchecked, and asking for everything does not try it again.
        c.apply(Action::FindTraces("OBS Studio".into()));
        c.tick();
        assert_eq!(c.model().unchecked, [c.model().stuck[0].0.clone()]);
        let tried = log.borrow().removed.len();
        c.apply(Action::AskCleanAll("OBS Studio".into()));
        c.apply(Action::Confirm);
        assert_eq!(
            log.borrow().removed.len(),
            tried + 1,
            "only the one that was not stuck"
        );
        assert_eq!(c.model().stuck.len(), 1, "it is still remembered");
        // Another app starts clean.
        c.apply(Action::FindTraces("Other App".into()));
        assert!(c.model().stuck.is_empty());
    }
    /// Three apps installed (one of them the runtime Windows needs) and a core that looks for
    /// nothing, to run batches.
    fn batch_core() -> (
        Core,
        crate::testing::FakeApps,
        std::rc::Rc<std::cell::RefCell<crate::testing::CleanLog>>,
    ) {
        use crate::testing::FakeApps;
        let installed = FakeApps::default();
        for name in ["Alpha", "Beta", "Gamma", "Microsoft .NET Runtime"] {
            let mut app = fake_app(name);
            if name.starts_with("Microsoft") {
                app.publisher = Some("Microsoft Corporation".into());
            }
            installed.0.borrow_mut().push(app);
        }
        let (mut c, log) =
            core_finding(&installed, crate::testing::FakeProcesses::default(), vec![]);
        c.tick();
        c.apply(Action::Go(Page::Apps));
        c.tick();
        (c, installed, log)
    }

    fn remove(installed: &crate::testing::FakeApps, name: &str) {
        installed.0.borrow_mut().retain(|a| a.name != name);
    }

    fn tick_until(c: &mut Core, done: impl Fn(&Core) -> bool) {
        for _ in 0..40 {
            std::thread::sleep(Duration::from_millis(15));
            c.tick();
            if done(c) {
                return;
            }
        }
        panic!("it never got there");
    }

    #[test]
    fn checked_apps_are_uninstalled_one_after_the_other_and_a_summary_ends_it() {
        let (mut c, installed, log) = batch_core();
        for id in [
            "HKLM64:Alpha",
            "HKLM64:Gamma",
            "HKLM64:Microsoft .NET Runtime",
            "HKLM64:Nope",
        ] {
            c.apply(Action::ToggleMark(id.into()));
        }
        assert_eq!(
            c.model().marked,
            ["HKLM64:Alpha", "HKLM64:Gamma"],
            "protected and unknown are not checked"
        );
        c.apply(Action::AskUninstallMarked);
        assert_eq!(c.model().confirm, Some(Confirm::UninstallMarked));
        c.apply(Action::Confirm);
        assert!(c.model().marked.is_empty());
        assert_eq!(log.borrow().launched, ["Alpha"], "one at a time");
        assert_eq!(
            c.model().batch.as_ref().unwrap().current.as_deref(),
            Some("Alpha")
        );
        assert!(c.model().waiting && c.batch_running());
        remove(&installed, "Alpha");
        tick_until(&mut c, |_| log.borrow().launched.len() == 2);
        assert_eq!(log.borrow().launched, ["Alpha", "Gamma"]);
        assert!(
            !c.model().traces_open && c.model().traces.is_none(),
            "the traces wait for the end"
        );
        remove(&installed, "Gamma");
        tick_until(&mut c, |c| !c.batch_running());
        let batch = c.model().batch.as_ref().unwrap();
        assert!(batch.finished() && !batch.cancelled);
        assert_eq!(batch.uninstalled().collect::<Vec<_>>(), ["Alpha", "Gamma"]);
        assert!(c
            .model()
            .message
            .as_deref()
            .unwrap()
            .contains("2 de 2 apps desinstaladas"));
        assert!(!c.model().waiting && c.watching.is_none());
        c.apply(Action::CloseBatch);
        assert!(c.model().batch.is_none());
    }

    #[test]
    fn giving_up_a_batch_leaves_the_rest_unstarted_and_says_so() {
        let (mut c, _installed, log) = batch_core();
        for id in ["HKLM64:Alpha", "HKLM64:Beta", "HKLM64:Gamma"] {
            c.apply(Action::ToggleMark(id.into()));
        }
        c.apply(Action::AskUninstallMarked);
        c.apply(Action::Confirm);
        c.apply(Action::StopWaiting);
        assert_eq!(log.borrow().launched, ["Alpha"], "nothing else was started");
        let batch = c.model().batch.as_ref().unwrap();
        assert!(batch.finished() && batch.cancelled);
        assert!(!c.model().waiting);
        let said = c.model().message.as_deref().unwrap();
        assert!(
            said.contains("2 sin empezar") && said.contains("1 sigue instalada"),
            "{said}"
        );
    }

    #[test]
    fn a_wait_that_runs_out_moves_on_to_the_next_app_and_nothing_checked_asks_nothing() {
        let (mut c, installed, log) = batch_core();
        c.apply(Action::AskUninstallMarked);
        assert!(
            c.model().confirm.is_none(),
            "nothing checked, nothing to confirm"
        );
        c.apply(Action::ToggleMark("HKLM64:Alpha".into()));
        c.apply(Action::ToggleMark("HKLM64:Alpha".into()));
        assert!(c.model().marked.is_empty(), "checking twice unchecks");
        for id in ["HKLM64:Alpha", "HKLM64:Beta"] {
            c.apply(Action::ToggleMark(id.into()));
        }
        c.apply(Action::ClearMarks);
        assert!(c.model().marked.is_empty());
        for id in ["HKLM64:Alpha", "HKLM64:Beta"] {
            c.apply(Action::ToggleMark(id.into()));
        }
        c.apply(Action::AskUninstallMarked);
        c.apply(Action::Confirm);
        let long_ago = Instant::now() - WATCH_FOR - Duration::from_secs(1);
        let (app, _, _) = c.watching.take().unwrap();
        c.watching = Some((app, long_ago, long_ago));
        assert!(c.tick());
        assert_eq!(
            log.borrow().launched,
            ["Alpha", "Beta"],
            "Beta starts although Alpha is still there"
        );
        remove(&installed, "Beta");
        tick_until(&mut c, |c| !c.batch_running());
        let said = c.model().message.as_deref().unwrap();
        assert!(
            said.contains("1 de 2 apps desinstaladas") && said.contains("1 sigue instalada"),
            "{said}"
        );
    }
    /// A core on the apps page with the install monitor over fakes.
    fn monitor_core() -> (
        Core,
        crate::testing::FakeApps,
        crate::testing::FakePicture,
        crate::testing::FakeLogs,
    ) {
        use crate::testing::{FakeApps, FakeLogs, FakePicker, FakePicture, FakeRunner};
        let installed = FakeApps::default();
        installed.0.borrow_mut().push(fake_app("Old"));
        let (picture, logs) = (FakePicture::default(), FakeLogs::default());
        let monitoring = crate::Monitoring::new(
            Box::new(picture.clone()),
            Box::new(FakeRunner::default()),
            Box::new(FakePicker::of(r"C:\Descargas\zed-setup.exe")),
            Box::new(logs.clone()),
        );
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
            .with_monitoring(monitoring),
        ));
        c.tick();
        c.apply(Action::Go(Page::Apps));
        c.tick();
        (c, installed, picture, logs)
    }

    #[test]
    fn an_installer_is_watched_and_what_it_made_is_kept_as_a_log_that_can_be_exported_and_deleted()
    {
        use purifex_domain::app_traces::TraceKind;
        use purifex_domain::monitor::Mark;
        let (mut c, installed, picture, logs) = monitor_core();
        assert_eq!(
            c.model().install_logs.as_deref(),
            Some(&[][..]),
            "read with the page"
        );
        c.apply(Action::MonitorStart);
        assert_eq!(c.model().monitoring.as_deref(), Some("zed-setup.exe"));
        // The installer makes a folder and the app appears.
        picture.0.borrow_mut().push(Mark {
            kind: TraceKind::Folder,
            location: r"C:\ProgramData\Zed".into(),
            value: None,
            stamp: "1".into(),
            bytes: 0,
        });
        installed.0.borrow_mut().push(fake_app("Zed Editor"));
        c.apply(Action::MonitorFinish);
        assert!(c.model().monitoring.is_none());
        let said = c.model().message.clone().unwrap();
        assert!(
            said.contains("1 archivo o carpeta") && said.contains("Zed Editor"),
            "{said}"
        );
        c.tick();
        let kept = c.model().install_logs.clone().unwrap();
        assert_eq!(kept.len(), 1);
        assert_eq!(logs.0.borrow().len(), 1);
        let id = kept[0].id.clone();
        c.apply(Action::OpenLog(id.clone()));
        assert_eq!(c.model().log_open.as_deref(), Some(id.as_str()));
        c.apply(Action::ExportLog(id.clone()));
        assert!(c
            .model()
            .message
            .as_deref()
            .unwrap()
            .contains("Registro exportado"));
        c.apply(Action::AskDeleteLog(id.clone()));
        assert_eq!(c.model().confirm, Some(Confirm::DeleteLog(id)));
        assert_eq!(
            logs.0.borrow().len(),
            1,
            "nothing changes until it is confirmed"
        );
        c.apply(Action::Confirm);
        c.tick();
        assert!(logs.0.borrow().is_empty() && c.model().log_open.is_none());
        assert_eq!(c.model().install_logs.as_deref(), Some(&[][..]));
    }

    #[test]
    fn watching_can_be_given_up_an_import_is_kept_and_finishing_without_start_is_an_error() {
        let (mut c, _, _, logs) = monitor_core();
        c.apply(Action::MonitorFinish);
        assert!(c
            .model()
            .error
            .as_deref()
            .unwrap()
            .contains("no hay ninguna instalación"));
        c.apply(Action::MonitorStart);
        c.apply(Action::MonitorCancel);
        assert!(c.model().monitoring.is_none() && logs.0.borrow().is_empty());
        c.apply(Action::ImportLog);
        c.tick();
        assert_eq!(c.model().install_logs.as_ref().unwrap().len(), 1);
        assert!(!c.model().install_logs.as_ref().unwrap()[0].own);
        assert!(c
            .model()
            .message
            .as_deref()
            .unwrap()
            .contains("Registro importado"));
    }

    fn startup_entry(
        kind: purifex_domain::startup::StartupKind,
        name: &str,
        command: &str,
    ) -> purifex_domain::startup::StartupItem {
        purifex_domain::startup::StartupItem {
            id: format!("{kind:?}|{name}"),
            name: name.into(),
            command: command.into(),
            kind,
            place: match kind {
                purifex_domain::startup::StartupKind::Task => format!(r"\Zed\{name}"),
                _ => r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run".into(),
            },
            enabled: true,
            can_toggle: kind != purifex_domain::startup::StartupKind::Service,
            needs_admin: false,
            app: None,
        }
    }

    /// The core, the fake list, what the quarantine did and the folders opened.
    type StartupRig = (
        Core,
        crate::testing::FakeStartup,
        std::rc::Rc<std::cell::RefCell<crate::testing::CleanLog>>,
        std::rc::Rc<std::cell::RefCell<Vec<String>>>,
    );

    /// A core on the Startup page over a fake list and the fake quarantine.
    fn startup_core(
        entries: Vec<purifex_domain::startup::StartupItem>,
        refuses: Option<&str>,
    ) -> StartupRig {
        use crate::testing::{CleanLog, FakeCleaning, FakeFinder, FakeFolders, FakeStartup};
        use std::{cell::RefCell, rc::Rc};
        let startup = FakeStartup {
            items: Rc::new(RefCell::new(entries)),
            refuses: refuses.map(String::from),
        };
        let log = Rc::new(RefCell::new(CleanLog::default()));
        let opened = Rc::new(RefCell::new(Vec::new()));
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
            .with_startup(Box::new(startup.clone()))
            .with_folder_opener(Box::new(FakeFolders(opened.clone())))
            .with_cleaning(crate::Cleaning::new(
                Box::new(FakeFinder(vec![])),
                Box::new(FakeCleaning(log.clone())),
                Box::new(FakeCleaning(log.clone())),
            )),
        ));
        c.tick();
        c.apply(Action::Go(Page::Startup));
        c.tick();
        (c, startup, log, opened)
    }

    #[test]
    fn an_entry_of_the_startup_page_is_turned_off_and_on_without_being_deleted() {
        use purifex_domain::startup::StartupKind::Registry;
        let item = startup_entry(Registry, "Zed", r#""C:\Apps\Zed\z.exe" -s"#);
        let (mut c, startup, _, _) = startup_core(vec![item.clone()], None);
        assert_eq!(c.model().startup.as_ref().unwrap().len(), 1);
        c.apply(Action::ToggleStartup(item.id.clone()));
        assert!(
            !startup.items.borrow()[0].enabled && !c.model().startup.as_ref().unwrap()[0].enabled
        );
        assert!(c
            .model()
            .message
            .as_deref()
            .unwrap()
            .contains("ya no arranca"));
        c.apply(Action::ToggleStartup(item.id));
        assert!(
            startup.items.borrow()[0].enabled && c.model().startup.as_ref().unwrap()[0].enabled
        );
        // Something that is not in the list is never acted on.
        c.apply(Action::ToggleStartup("inventada".into()));
        assert_eq!(startup.items.borrow().len(), 1);
    }

    #[test]
    fn a_refused_change_says_why_and_leaves_the_entry_as_it_was() {
        use purifex_domain::startup::StartupKind::Registry;
        let item = startup_entry(Registry, "Zed", "z.exe");
        let (mut c, startup, _, _) =
            startup_core(vec![item.clone()], Some("hace falta ser administrador"));
        c.apply(Action::ToggleStartup(item.id));
        assert!(c
            .model()
            .error
            .as_deref()
            .unwrap()
            .contains("administrador"));
        assert!(
            startup.items.borrow()[0].enabled && c.model().startup.as_ref().unwrap()[0].enabled
        );
    }

    #[test]
    fn taking_an_entry_out_asks_first_goes_to_the_quarantine_and_a_service_is_not_removable() {
        use purifex_domain::startup::StartupKind::{Registry, Service};
        let (reg, svc) = (
            startup_entry(Registry, "Zed", "z.exe"),
            startup_entry(Service, "ZedSvc", "s.exe"),
        );
        let (mut c, _, log, _) = startup_core(vec![reg.clone(), svc.clone()], None);
        c.apply(Action::AskRemoveStartup(svc.id));
        assert!(c.model().confirm.is_none(), "a service is only shown");
        c.apply(Action::AskRemoveStartup(reg.id.clone()));
        assert_eq!(c.model().confirm, Some(Confirm::RemoveStartup(reg.id)));
        assert!(
            log.borrow().removed.is_empty(),
            "nothing until it is confirmed"
        );
        c.apply(Action::Confirm);
        assert_eq!(log.borrow().removed.len(), 1);
        assert_eq!(log.borrow().entries[0].app, "Inicio: Zed");
        assert!(c
            .model()
            .message
            .as_deref()
            .unwrap()
            .contains("purifex apps restore"));
        c.tick();
        assert!(c.model().startup.is_some(), "the list is read again");
    }

    #[test]
    fn the_folder_of_an_entry_is_opened_only_for_entries_of_the_list() {
        use purifex_domain::startup::StartupKind::Registry;
        let item = startup_entry(Registry, "Zed", r#""C:\Apps\Zed\z.exe""#);
        let (mut c, _, _, opened) = startup_core(vec![item.clone()], None);
        c.apply(Action::OpenStartupFolder(item.id));
        assert_eq!(opened.borrow().as_slice(), [r"C:\Apps\Zed"]);
        c.apply(Action::OpenStartupFolder("inventada".into()));
        assert!(c.model().error.is_some() && opened.borrow().len() == 1);
    }

    /// A core on the Junk page over fake junk and the fake quarantine.
    fn junk_core(
        items: Vec<purifex_domain::junk::JunkItem>,
        stuck: &[&str],
    ) -> (
        Core,
        std::rc::Rc<std::cell::RefCell<crate::testing::CleanLog>>,
    ) {
        use crate::testing::{CleanLog, FakeCleaning, FakeFinder, FakeJunk};
        use std::{cell::RefCell, rc::Rc};
        let log = Rc::new(RefCell::new(CleanLog::default()));
        log.borrow_mut().stuck = stuck.iter().map(|s| s.to_string()).collect();
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
            .with_cleaning(
                crate::Cleaning::new(
                    Box::new(FakeFinder(vec![])),
                    Box::new(FakeCleaning(log.clone())),
                    Box::new(FakeCleaning(log.clone())),
                )
                .with_junk(Box::new(FakeJunk(Rc::new(RefCell::new(items))))),
            ),
        ));
        c.tick();
        c.apply(Action::Go(Page::Junk));
        c.tick();
        (c, log)
    }

    fn junk_item(
        kind: purifex_domain::junk::JunkKind,
        location: &str,
        bytes: u64,
    ) -> purifex_domain::junk::JunkItem {
        purifex_domain::junk::JunkItem {
            kind,
            location: location.into(),
            bytes,
            folder: false,
        }
    }

    #[test]
    fn only_the_checked_kinds_of_junk_are_moved_after_a_question_and_the_rest_stay() {
        use purifex_domain::junk::JunkKind::{CrashDumps, Temp};
        let items = vec![
            junk_item(Temp, r"C:\Users\u\AppData\Local\Temp\a.tmp", 2048),
            junk_item(Temp, r"C:\Users\u\AppData\Local\Temp\b.tmp", 1024),
            junk_item(CrashDumps, r"C:\Windows\Minidump\x.dmp", 512),
        ];
        let (mut c, log) = junk_core(items, &[]);
        assert_eq!(c.model().junk.as_ref().unwrap().len(), 3);
        c.apply(Action::ToggleJunk(CrashDumps));
        c.apply(Action::AskCleanJunk);
        assert_eq!(c.model().confirm, Some(Confirm::CleanJunk));
        assert!(
            log.borrow().removed.is_empty(),
            "nothing until it is confirmed"
        );
        c.apply(Action::Confirm);
        assert_eq!(
            log.borrow().removed.len(),
            2,
            "the dumps were left unchecked"
        );
        assert!(log.borrow().removed.iter().all(|r| r.contains("Temp")));
        assert_eq!(log.borrow().entries[0].app, "Limpieza: basura");
        let said = c.model().message.clone().unwrap();
        assert!(
            said.contains("2 elementos") && said.contains("cuarentena"),
            "{said}"
        );
        c.tick();
        assert!(c.model().junk.is_some(), "it is read again");
    }

    #[test]
    fn what_cannot_be_moved_is_counted_with_its_reason_and_nothing_checked_asks_nothing() {
        use purifex_domain::junk::JunkKind::Temp;
        let busy = r"C:\Users\u\AppData\Local\Temp\busy.tmp";
        let items = vec![
            junk_item(Temp, busy, 10),
            junk_item(Temp, r"C:\Users\u\AppData\Local\Temp\ok.tmp", 10),
        ];
        let (mut c, _) = junk_core(items, &[busy]);
        c.apply(Action::AskCleanJunk);
        c.apply(Action::Confirm);
        let said = c.model().message.clone().unwrap();
        assert!(
            said.contains("1 elemento (") && said.contains("No se pudieron mover 1"),
            "{said}"
        );
        c.apply(Action::ToggleJunk(Temp));
        c.apply(Action::AskCleanJunk);
        assert!(c.model().confirm.is_none());
        assert!(c
            .model()
            .message
            .as_deref()
            .unwrap()
            .contains("nada marcado"));
    }

    #[test]
    fn clearing_the_filters_shows_every_app_again_and_keeps_the_choice() {
        use purifex_domain::app_list::AppFilter;
        let (mut c, _, _, _) = monitor_core();
        c.apply(Action::SetAppFilter(AppFilter::Store));
        c.apply(Action::SetQuery("zz".into()));
        c.apply(Action::ClearAppFilters);
        assert_eq!(c.model().app_filter, AppFilter::All);
        assert!(c.model().query.is_empty());
    }
}
