//! The programs an app that is gone may still be running from its folder. They hold its files
//! open, so they are shown and, only after the person confirmed, closed before the leftovers
//! are looked for again. Only what runs from inside the app's folder is ever closed.

use purifex_domain::apps::InstalledApp;
use purifex_domain::forced::{running_in, CloseReport, RunningProcess};
use purifex_domain::ports::Processes;
use purifex_domain::seeds::seeds_of;

use super::Cleaning;

impl Cleaning {
    /// Lets the cleaning list and close programs.
    pub fn with_processes(mut self, processes: Box<dyn Processes>) -> Self {
        self.processes = Some(processes);
        self
    }

    /// The programs of the app that run now. Empty when the app is still installed (its
    /// programs are not ours to close) or when it has no known folder.
    pub fn running_of(
        &self,
        apps: &[InstalledApp],
        all: &[InstalledApp],
        query: &str,
    ) -> Result<Vec<RunningProcess>, String> {
        let (app, installed) = self.resolve(apps, all, query)?;
        let (Some(processes), Some(folder), false) =
            (&self.processes, seeds_of(&app).folder, installed)
        else {
            return Ok(Vec::new());
        };
        Ok(running_in(
            &folder,
            &processes.running()?,
            std::process::id(),
        ))
    }

    /// Closes one program, only if the same `.exe` still runs under that number (a number
    /// can be given to another program once the first one ends).
    pub fn close_one(&mut self, expected: &RunningProcess) -> Result<(), String> {
        let processes = self
            .processes
            .as_mut()
            .ok_or("esta versión no sabe cerrar programas")?;
        let same = processes.running()?.iter().any(|p| p == expected);
        if !same {
            return Err(format!("{} ya no está abierto", expected.name()));
        }
        processes.close(expected.pid)
    }

    /// Closes the programs of the app that is gone, each one on its own: one that refuses
    /// does not stop the rest.
    pub fn close_running(
        &mut self,
        apps: &[InstalledApp],
        all: &[InstalledApp],
        query: &str,
    ) -> Result<CloseReport, String> {
        let (app, installed) = self.resolve(apps, all, query)?;
        if installed {
            return Err(format!(
                "{} sigue instalada: sus programas no se cierran desde aquí",
                app.name
            ));
        }
        let running = self.running_of(apps, all, query)?;
        let processes = self
            .processes
            .as_mut()
            .ok_or("esta versión no sabe cerrar programas")?;
        let mut report = CloseReport {
            app: app.name,
            ..Default::default()
        };
        for p in running {
            match processes.close(p.pid) {
                Ok(()) => report.closed.push(p.name().to_string()),
                Err(why) => report.failed.push((p.name().to_string(), why)),
            }
        }
        Ok(report)
    }
}
