//! The install monitor in the window: start watching an installer, say it is done, and keep,
//! open, export, import and delete the logs.

use super::Core;

impl Core {
    /// Asks for an installer and starts it with the picture of the system taken first.
    pub(super) fn monitor_started(&mut self) {
        self.model.error = None;
        match self.services.monitor_start() {
            Ok(Some(installer)) => {
                self.model.message = Some(format!(
                    "Se abrió {installer} y Purifex lo vigila. Instala con calma y, cuando termine (y la app ya esté instalada), pulsa «Terminé la instalación»."
                ));
                self.model.monitoring = Some(installer);
            }
            Ok(None) => {}
            Err(error) => self.model.error = Some(error),
        }
    }

    /// Takes the picture after and keeps what the installer created.
    pub(super) fn monitor_finished(&mut self) {
        match self.services.monitor_finish() {
            Ok(log) => {
                self.model.message = Some(format!(
                    "Registro guardado: {}. {}Cuando quites la app, Purifex lo usa para encontrar sus restos.",
                    log.counts_text(),
                    log.app
                        .as_deref()
                        .map_or_else(|| "No apareció ninguna app nueva en la lista, así que el registro no está enlazado a ninguna. ".to_string(), |a| format!("App enlazada: {a}. "))
                ));
                self.model.monitoring = None;
                (self.model.install_logs, self.model.apps) = (None, None);
                self.expect_reading();
            }
            Err(error) => self.model.error = Some(error),
        }
    }

    pub(super) fn monitor_cancelled(&mut self) {
        self.services.monitor_cancel();
        self.model.monitoring = None;
        self.model.message = Some("Dejé de vigilar el instalador. No se guardó nada.".into());
    }

    /// Opens what a log holds, or closes it if it was open.
    pub(super) fn log_opened(&mut self, id: String) {
        self.model.log_open = (self.model.log_open.as_ref() != Some(&id)).then_some(id);
    }

    pub(super) fn log_exported(&mut self, id: &str) {
        match self.services.export_install_log(id) {
            Ok(path) => self.model.message = Some(format!("Registro exportado a {path}.")),
            Err(error) => self.model.error = Some(error),
        }
    }

    pub(super) fn log_imported(&mut self) {
        match self.services.import_install_log() {
            Ok(Some(log)) => {
                self.model.message = Some(format!(
                    "Registro importado ({}). Como no lo hiciste aquí, solo propone restos en el modo Avanzado y sin marcar.",
                    log.counts_text()
                ));
                self.model.install_logs = None;
                self.expect_reading();
            }
            Ok(None) => {}
            Err(error) => self.model.error = Some(error),
        }
    }

    pub(super) fn log_deleted(&mut self, id: &str) {
        match self.services.delete_install_log(id) {
            Ok(()) => {
                if self.model.log_open.as_deref() == Some(id) {
                    self.model.log_open = None;
                }
                self.model.message = Some("Registro borrado.".into());
                self.model.install_logs = None;
                self.expect_reading();
            }
            Err(error) => self.model.error = Some(error),
        }
    }
}
