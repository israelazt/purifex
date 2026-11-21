//! The whole way of a leftover on this computer: an app that is gone leaves its folder, a
//! scheduled task and an entry that starts with Windows; the finder sees them, the rules
//! of the domain accept them, the quarantine takes them out and puts them back. Everything
//! is made for the test, in the user's own part of the system, and removed at the end.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use purifex_domain::app_traces::{propose_with, Evidence, Trace, TraceKind};
use purifex_domain::apps::InstalledApp;
use purifex_domain::cleanup_log::{CleanupEntry, CleanupState};
use purifex_domain::ports::{CleanupStore, TraceFinder};

use super::finder::WindowsTraceFinder;
use super::quarantine::{reg, LocalCleanup};
use super::tasks;

const RUN: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

/// What the test makes. Dropping it removes everything, even when an assertion fails halfway.
struct Scene {
    name: String,
    task: String,
    root: PathBuf,
    folder: PathBuf,
}

impl Scene {
    /// The leftovers of an app called after this process; `None` if this session cannot make
    /// scheduled tasks (there is then nothing to check).
    fn make() -> Option<Scene> {
        let id = std::process::id();
        let name = format!("PurifexE2E{id}");
        let root = std::env::temp_dir().join(format!("purifex-e2e-{id}"));
        let folder = root.join(&name);
        let scene = Scene {
            task: format!(r"\{name}"),
            name,
            root,
            folder,
        };
        fs::create_dir_all(&scene.folder).unwrap();
        fs::write(scene.program(), b"x").unwrap();
        let made = Command::new("schtasks")
            .args(["/create", "/tn", &scene.task, "/tr", &scene.program()])
            .args(["/sc", "once", "/st", "23:59", "/f"])
            .output();
        if !made.is_ok_and(|o| o.status.success()) {
            return None;
        }
        let command = format!("\"{}\" /silent", scene.program());
        reg(&["add", RUN, "/v", &scene.name, "/d", &command, "/f"]).unwrap();
        scene.wait_until_listed();
        Some(scene)
    }

    /// The scheduler can take a moment to list a task it has just created (more when the
    /// computer is busy): the finder reads that list, so the test waits for the task to be in it.
    fn wait_until_listed(&self) {
        let marker = format!("<!-- {} -->", self.task);
        for _ in 0..100 {
            let listed = Command::new("schtasks")
                .args(["/query", "/xml"])
                .output()
                .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains(&marker));
            if listed {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    fn program(&self) -> String {
        self.folder.join("run.exe").to_string_lossy().into_owned()
    }

    /// The app, as the system knows it once it is gone: only what it registered.
    fn app(&self) -> InstalledApp {
        InstalledApp {
            id: format!("name:{}", self.name),
            name: self.name.clone(),
            install_location: Some(self.folder.to_string_lossy().into_owned()),
            ..Default::default()
        }
    }

    fn startup_entry_exists(&self) -> bool {
        reg(&["query", RUN, "/v", &self.name]).is_ok()
    }
}

impl Drop for Scene {
    fn drop(&mut self) {
        let _ = reg(&["delete", RUN, "/v", &self.name, "/f"]);
        let _ = Command::new("schtasks")
            .args(["/delete", "/tn", &self.task, "/f"])
            .output();
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// What the finder and the rules propose for the app of the scene.
fn proposed(scene: &Scene) -> Vec<Trace> {
    let app = scene.app();
    let found = WindowsTraceFinder.search(&app).unwrap();
    propose_with(&app, &[], &found.seeds, &found.candidates)
}

/// Takes each trace out to the quarantine, as `Cleaning` does, and keeps what it logged.
fn take_out(store: &mut LocalCleanup, app: &str, traces: &[&Trace]) -> Vec<CleanupEntry> {
    traces
        .iter()
        .zip(1u64..)
        .map(|(trace, seq)| CleanupEntry {
            seq,
            app: app.into(),
            trace: (*trace).clone(),
            kept: store.remove(trace, seq).unwrap(),
            state: CleanupState::Removed,
            ..Default::default()
        })
        .collect()
}

#[test]
fn an_uninstalled_app_leaves_a_folder_a_task_and_a_startup_entry_that_go_out_and_come_back() {
    let Some(scene) = Scene::make() else { return };
    let traces = proposed(&scene);
    let of = |kind: TraceKind| traces.iter().find(|t| t.kind == kind);
    let folder = of(TraceKind::Folder).expect("the folder");
    let task = of(TraceKind::Task).expect("the task");
    let startup = of(TraceKind::RegistryValue).expect("the startup entry");
    assert_eq!(task.evidence, Evidence::TaskAction);
    assert_eq!(startup.evidence, Evidence::PathReference);
    assert_eq!(startup.value.as_deref(), Some(scene.name.as_str()));

    let mut store = LocalCleanup::in_dir(scene.root.join("quarantine"));
    let entries = take_out(&mut store, &scene.name, &[folder, task, startup]);
    assert!(!scene.folder.exists() && !tasks::exists(&scene.task));
    assert!(!scene.startup_entry_exists(), "the entry is gone");

    for entry in &entries {
        store.restore(entry).unwrap();
    }
    assert!(scene.folder.join("run.exe").exists() && tasks::exists(&scene.task));
    assert!(scene.startup_entry_exists(), "the entry is back");
}
