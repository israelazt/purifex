//! Fakes of the install monitor: a picture a test can change, an installer that only records
//! what it was asked to run, a dialog that answers what it is told, and logs in memory.

use std::cell::RefCell;
use std::rc::Rc;

use purifex_domain::monitor::{InstallLog, Mark};
use purifex_domain::ports::{FilePicker, InstallLogs, InstallerRunner, SystemPicture};

#[derive(Clone, Default)]
pub struct FakePicture(pub Rc<RefCell<Vec<Mark>>>);

impl SystemPicture for FakePicture {
    fn take(&self) -> Result<Vec<Mark>, String> {
        Ok(self.0.borrow().clone())
    }
}

#[derive(Clone, Default)]
pub struct FakeRunner(pub Rc<RefCell<Vec<String>>>);

impl InstallerRunner for FakeRunner {
    fn run(&self, installer: &str) -> Result<(), String> {
        self.0.borrow_mut().push(installer.to_string());
        Ok(())
    }
}

#[derive(Clone)]
pub struct FakePicker(Option<String>);

impl FakePicker {
    pub fn of(file: &str) -> Self {
        Self(Some(file.to_string()))
    }

    pub fn cancelled() -> Self {
        Self(None)
    }
}

impl FilePicker for FakePicker {
    fn pick(&self, _: &str, _: &[&str]) -> Result<Option<String>, String> {
        Ok(self.0.clone())
    }
}

#[derive(Clone, Default)]
pub struct FakeLogs(pub Rc<RefCell<Vec<InstallLog>>>);

impl InstallLogs for FakeLogs {
    fn all(&self) -> Result<Vec<InstallLog>, String> {
        Ok(self.0.borrow().clone())
    }

    fn save(&self, log: &InstallLog) -> Result<(), String> {
        self.0.borrow_mut().push(log.clone());
        Ok(())
    }

    fn delete(&self, id: &str) -> Result<(), String> {
        self.0.borrow_mut().retain(|l| l.id != id);
        Ok(())
    }

    fn export(&self, id: &str) -> Result<String, String> {
        Ok(format!(r"C:\Docs\purifex-registro-{id}.jsonl"))
    }

    fn import(&self, _: &str) -> Result<InstallLog, String> {
        let log = InstallLog {
            id: "importado-1".into(),
            installer: "x.exe".into(),
            app: Some("Zed".into()),
            on: None,
            own: false,
            marks: vec![],
        };
        self.0.borrow_mut().push(log.clone());
        Ok(log)
    }
}
