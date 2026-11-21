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
pub struct FakeRegistryBackup {
    pub fails: bool,
    pub saved: Rc<RefCell<Vec<String>>>,
}

impl purifex_domain::ports::RegistryBackup for FakeRegistryBackup {
    fn save(&self, app: &purifex_domain::apps::InstalledApp) -> Result<Option<String>, String> {
        if self.fails {
            return Err("no hay permiso".into());
        }
        self.saved.borrow_mut().push(app.name.clone());
        Ok(Some(format!(r"C:\copias\{}.reg", app.name)))
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
