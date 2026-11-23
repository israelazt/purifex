//! Fakes of what the program asks of the system itself: permissions, the apps installed,
//! and the safety nets taken before an uninstall.

use std::cell::RefCell;
use std::rc::Rc;

/// A program that is (or is not) an administrator, and remembers what it was asked to
/// restart for; the person can say no.
#[derive(Clone, Default)]
pub struct FakeElevation {
    pub elevated: bool,
    pub refuses: bool,
    pub restarted: Rc<RefCell<Vec<Option<String>>>>,
}

impl purifex_domain::ports::Elevation for FakeElevation {
    fn is_elevated(&self) -> bool {
        self.elevated
    }
    fn restart_elevated(&self, resume: Option<&str>) -> Result<(), String> {
        if self.refuses {
            return Err("no se dio el permiso de administrador".into());
        }
        self.restarted.borrow_mut().push(resume.map(String::from));
        Ok(())
    }
}

/// Installed apps that a test can change (an uninstaller finishing removes one).
#[derive(Clone, Default)]
pub struct FakeApps(pub Rc<RefCell<Vec<purifex_domain::apps::InstalledApp>>>);

impl purifex_domain::ports::AppInventory for FakeApps {
    fn apps(&self) -> Result<Vec<purifex_domain::apps::InstalledApp>, String> {
        Ok(self.0.borrow().clone())
    }
}

/// A registry backup that records the apps it was asked to save.
#[derive(Clone, Default)]
pub struct FakeBackups {
    pub fails: bool,
    pub saved: Rc<RefCell<Vec<String>>>,
    pub entries: Rc<RefCell<Vec<purifex_domain::safeguard::BackupEntry>>>,
    pub restored: Rc<RefCell<Vec<String>>>,
}

impl purifex_domain::ports::Backups for FakeBackups {
    fn save(&self, app: &purifex_domain::apps::InstalledApp) -> Result<Option<String>, String> {
        if self.fails {
            return Err("no hay permiso".into());
        }
        self.saved.borrow_mut().push(app.name.clone());
        Ok(Some(format!(r"C:\copias\{}.reg", app.name)))
    }
    fn record(&self, entry: &purifex_domain::safeguard::BackupEntry) -> Result<(), String> {
        self.entries.borrow_mut().push(entry.clone());
        Ok(())
    }
    fn entries(&self) -> Result<Vec<purifex_domain::safeguard::BackupEntry>, String> {
        Ok(self.entries.borrow().clone())
    }
    fn restore(&self, entry: &purifex_domain::safeguard::BackupEntry) -> Result<(), String> {
        self.restored.borrow_mut().push(entry.app.clone());
        Ok(())
    }
    fn delete(&self, entry: &purifex_domain::safeguard::BackupEntry) -> Result<(), String> {
        self.entries.borrow_mut().retain(|e| e != entry);
        Ok(())
    }
}

/// Restore points that a test can make fail. It records what it was asked for.
#[derive(Clone, Default)]
pub struct FakeRestorePoints {
    pub fails: Option<String>,
    pub asked: Rc<RefCell<Vec<String>>>,
}

impl purifex_domain::ports::RestorePoints for FakeRestorePoints {
    fn create(&self, description: &str) -> Result<(), String> {
        self.asked.borrow_mut().push(description.to_string());
        self.fails.clone().map_or(Ok(()), Err)
    }
}

/// Programs that are running, that a test can close.
#[derive(Clone, Default)]
pub struct FakeProcesses {
    pub running: Rc<RefCell<Vec<purifex_domain::forced::RunningProcess>>>,
    pub closed: Rc<RefCell<Vec<u32>>>,
}

impl FakeProcesses {
    pub fn with(list: &[(u32, &str)]) -> Self {
        let running = list
            .iter()
            .map(|(pid, path)| purifex_domain::forced::RunningProcess {
                pid: *pid,
                path: (*path).into(),
            })
            .collect();
        Self {
            running: Rc::new(RefCell::new(running)),
            closed: Rc::default(),
        }
    }
}

impl purifex_domain::ports::Processes for FakeProcesses {
    fn running(&self) -> Result<Vec<purifex_domain::forced::RunningProcess>, String> {
        Ok(self.running.borrow().clone())
    }

    fn close(&mut self, pid: u32) -> Result<(), String> {
        self.running.borrow_mut().retain(|p| p.pid != pid);
        self.closed.borrow_mut().push(pid);
        Ok(())
    }
}

/// The choices for the list of apps, kept in memory so a test can look at them.
#[derive(Clone, Default)]
pub struct FakeAppList(pub Rc<RefCell<purifex_domain::app_list::AppListSettings>>);

impl purifex_domain::ports::AppListStore for FakeAppList {
    fn load(&self) -> purifex_domain::app_list::AppListSettings {
        *self.0.borrow()
    }

    fn save(&self, settings: &purifex_domain::app_list::AppListSettings) -> Result<(), String> {
        *self.0.borrow_mut() = *settings;
        Ok(())
    }
}

/// The program under the pointer, chosen by a test.
#[derive(Clone, Default)]
pub struct FakeWindowPicker(pub Rc<RefCell<Option<purifex_domain::forced::RunningProcess>>>);

impl purifex_domain::ports::WindowPicker for FakeWindowPicker {
    fn under_pointer(&self) -> Result<Option<purifex_domain::forced::RunningProcess>, String> {
        Ok(self.0.borrow().clone())
    }
}

/// What starts with Windows, in memory: turning an entry on or off changes it here.
#[derive(Clone, Default)]
pub struct FakeStartup {
    pub items: Rc<RefCell<Vec<purifex_domain::startup::StartupItem>>>,
    /// The reason every change is refused for, when set (no administrator).
    pub refuses: Option<String>,
}

impl purifex_domain::ports::StartupSource for FakeStartup {
    fn read(&self) -> Result<Vec<purifex_domain::startup::StartupItem>, String> {
        Ok(self.items.borrow().clone())
    }

    fn set_enabled(
        &self,
        item: &purifex_domain::startup::StartupItem,
        on: bool,
    ) -> Result<(), String> {
        if let Some(why) = &self.refuses {
            return Err(why.clone());
        }
        for slot in self
            .items
            .borrow_mut()
            .iter_mut()
            .filter(|i| i.id == item.id)
        {
            slot.enabled = on;
        }
        Ok(())
    }
}

/// Folders opened in Explorer, recorded.
#[derive(Clone, Default)]
pub struct FakeFolders(pub Rc<RefCell<Vec<String>>>);

impl purifex_domain::ports::FolderOpener for FakeFolders {
    fn open(&self, folder: &str) -> Result<(), String> {
        self.0.borrow_mut().push(folder.to_string());
        Ok(())
    }
}

/// Junk found, in memory.
#[derive(Clone, Default)]
pub struct FakeJunk(pub Rc<RefCell<Vec<purifex_domain::junk::JunkItem>>>);

impl purifex_domain::ports::JunkFinder for FakeJunk {
    fn scan(&self) -> Result<Vec<purifex_domain::junk::JunkItem>, String> {
        Ok(self.0.borrow().clone())
    }
}
