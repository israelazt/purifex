//! The install monitor: choose an installer, take a picture of the system, run it, and when
//! the person says it is done take another and keep the difference as a log. The pictures and
//! the installer go through ports; what is new is decided in `purifex_domain::monitor`.

use purifex_domain::apps::InstalledApp;
use purifex_domain::date::Date;
use purifex_domain::monitor::{added, next_id, InstallLog, Mark};
use purifex_domain::ports::{FilePicker, InstallLogs, InstallerRunner, SystemPicture};

/// An installation that was started and is not over.
struct Pending {
    /// The file name of the installer.
    installer: String,
    before: Vec<Mark>,
    /// The apps that were listed before it ran (their ids).
    apps_before: Vec<String>,
}

#[derive(Default)]
pub struct Monitoring {
    picture: Option<Box<dyn SystemPicture>>,
    runner: Option<Box<dyn InstallerRunner>>,
    picker: Option<Box<dyn FilePicker>>,
    logs: Option<Box<dyn InstallLogs>>,
    pending: Option<Pending>,
}

impl Monitoring {
    pub fn new(
        picture: Box<dyn SystemPicture>,
        runner: Box<dyn InstallerRunner>,
        picker: Box<dyn FilePicker>,
        logs: Box<dyn InstallLogs>,
    ) -> Self {
        Self {
            picture: Some(picture),
            runner: Some(runner),
            picker: Some(picker),
            logs: Some(logs),
            pending: None,
        }
    }

    fn parts(&self) -> Result<(&dyn SystemPicture, &dyn InstallLogs), String> {
        match (&self.picture, &self.logs) {
            (Some(p), Some(l)) => Ok((p.as_ref(), l.as_ref())),
            _ => Err("esta versión no tiene monitor de instalaciones".into()),
        }
    }

    /// The name of the installer being watched, if one is.
    pub fn watching(&self) -> Option<&str> {
        self.pending.as_ref().map(|p| p.installer.as_str())
    }

    /// Asks for an installer, takes the picture before and starts it. `None`: the person
    /// cancelled the dialog and nothing was started.
    pub fn start(&mut self, apps: &[InstalledApp]) -> Result<Option<String>, String> {
        if self.pending.is_some() {
            return Err("ya hay una instalación vigilada: termínala o cancélala".into());
        }
        let (picture, _) = self.parts()?;
        let Some(picker) = &self.picker else {
            return Err("esta versión no puede elegir archivos".into());
        };
        let Some(file) = picker.pick("Elige el instalador a vigilar", &["exe", "msi"])? else {
            return Ok(None);
        };
        let before = picture.take()?;
        if let Some(runner) = &self.runner {
            runner.run(&file)?;
        }
        let installer = file.rsplit('\\').next().unwrap_or(&file).to_string();
        self.pending = Some(Pending {
            installer: installer.clone(),
            before,
            apps_before: apps.iter().map(|a| a.id.clone()).collect(),
        });
        Ok(Some(installer))
    }

    /// Takes the picture after, keeps what is new as a log and ends the watching. The app that
    /// appeared in the list (if one did) names the log.
    pub fn finish(
        &mut self,
        apps: &[InstalledApp],
        today: Option<Date>,
    ) -> Result<InstallLog, String> {
        let (picture, logs) = self.parts()?;
        let pending = self
            .pending
            .as_ref()
            .ok_or("no hay ninguna instalación vigilada")?;
        let after = picture.take()?;
        let mut marks = added(&pending.before, &after);
        picture.measure(&mut marks);
        let app = apps
            .iter()
            .find(|a| !pending.apps_before.contains(&a.id))
            .map(|a| a.name.clone());
        let log = InstallLog {
            id: next_id(&logs.all()?, today),
            installer: pending.installer.clone(),
            app,
            on: today,
            own: true,
            marks,
        };
        logs.save(&log)?;
        self.pending = None;
        Ok(log)
    }

    /// Gives up the watching; nothing is kept.
    pub fn cancel(&mut self) {
        self.pending = None;
    }

    pub fn logs(&self) -> Result<Vec<InstallLog>, String> {
        self.parts()?.1.all()
    }

    pub fn delete(&self, id: &str) -> Result<(), String> {
        self.parts()?.1.delete(id)
    }

    pub fn export(&self, id: &str) -> Result<String, String> {
        self.parts()?.1.export(id)
    }

    /// Asks for a file and keeps the log in it (as one not made here).
    pub fn import(&self) -> Result<Option<InstallLog>, String> {
        let (_, logs) = self.parts()?;
        let Some(picker) = &self.picker else {
            return Err("esta versión no puede elegir archivos".into());
        };
        let Some(file) = picker.pick("Elige el registro a importar", &["jsonl"])? else {
            return Ok(None);
        };
        logs.import(&file).map(Some)
    }
}

#[cfg(test)]
mod tests {
    use purifex_domain::app_traces::TraceKind;

    use super::*;
    use crate::testing::{FakeLogs, FakePicker, FakePicture, FakeRunner};

    fn mark(location: &str) -> Mark {
        Mark {
            kind: TraceKind::Folder,
            location: location.into(),
            value: None,
            stamp: "1".into(),
            bytes: 0,
        }
    }

    fn app(name: &str) -> InstalledApp {
        InstalledApp {
            id: format!("HKLM64:{name}"),
            name: name.into(),
            ..Default::default()
        }
    }

    #[test]
    fn what_the_installer_created_is_kept_as_a_log_named_after_the_app_that_appeared() {
        let (picture, runner, picker, logs) = (
            FakePicture::default(),
            FakeRunner::default(),
            FakePicker::of(r"C:\Descargas\zed-setup.exe"),
            FakeLogs::default(),
        );
        *picture.0.borrow_mut() = vec![mark(r"C:\ProgramData\Shared")];
        let mut m = Monitoring::new(
            Box::new(picture.clone()),
            Box::new(runner.clone()),
            Box::new(picker),
            Box::new(logs.clone()),
        );
        let started = m.start(&[app("Old")]).unwrap();
        assert_eq!(started.as_deref(), Some("zed-setup.exe"));
        assert_eq!(
            runner.0.borrow().as_slice(),
            [r"C:\Descargas\zed-setup.exe"]
        );
        assert_eq!(m.watching(), Some("zed-setup.exe"));
        assert!(m.start(&[]).is_err(), "one at a time");
        // The installer makes a folder and the app shows up.
        picture.0.borrow_mut().push(mark(r"C:\ProgramData\Zed"));
        let log = m.finish(&[app("Old"), app("Zed Editor")], None).unwrap();
        assert_eq!(log.app.as_deref(), Some("Zed Editor"));
        assert!(log.own);
        let places: Vec<_> = log.marks.iter().map(|x| x.location.as_str()).collect();
        assert_eq!(places, [r"C:\ProgramData\Zed"], "only what is new");
        assert_eq!(logs.0.borrow().len(), 1);
        assert!(m.watching().is_none() && m.finish(&[], None).is_err());
    }

    #[test]
    fn cancelling_the_dialog_or_the_watching_keeps_and_starts_nothing() {
        let logs = FakeLogs::default();
        let runner = FakeRunner::default();
        let mut m = Monitoring::new(
            Box::new(FakePicture::default()),
            Box::new(runner.clone()),
            Box::new(FakePicker::cancelled()),
            Box::new(logs.clone()),
        );
        assert_eq!(m.start(&[]).unwrap(), None);
        assert!(runner.0.borrow().is_empty() && m.watching().is_none());
        assert_eq!(m.import().unwrap(), None);
        let mut m = Monitoring::new(
            Box::new(FakePicture::default()),
            Box::new(runner),
            Box::new(FakePicker::of("a.exe")),
            Box::new(logs.clone()),
        );
        m.start(&[]).unwrap();
        m.cancel();
        assert!(m.watching().is_none() && logs.0.borrow().is_empty());
    }

    #[test]
    fn a_log_is_exported_deleted_and_an_imported_one_is_never_own() {
        let logs = FakeLogs::default();
        let m = Monitoring::new(
            Box::new(FakePicture::default()),
            Box::new(FakeRunner::default()),
            Box::new(FakePicker::of("x.jsonl")),
            Box::new(logs),
        );
        let imported = m.import().unwrap().unwrap();
        assert!(!imported.own);
        assert_eq!(m.logs().unwrap().len(), 1);
        assert!(m.export(&imported.id).unwrap().contains(&imported.id));
        m.delete(&imported.id).unwrap();
        assert!(m.logs().unwrap().is_empty());
        assert!(
            Monitoring::default().logs().is_err(),
            "without the parts it says so"
        );
    }
}
