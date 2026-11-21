//! What the sight landed on: the program, the app it belongs to (when it has one) and what can be
//! done to it. Closing it is asked again before anything is closed.

use purifex_domain::hunter::Hunted;
use purifex_domain::view::{Action, Confirm, Model};

use crate::dialog::{Btn, DialogView};
use crate::theme::Tone;

fn button(label: &str, icon: &'static str, primary: bool, action: Action) -> Btn {
    Btn {
        label: label.into(),
        icon,
        primary,
        action: Some(action),
    }
}

/// The dialog of hunter mode, if one is due: the question before closing the program, or what
/// the sight landed on. Any other question the page has comes first.
pub(crate) fn view(m: &Model) -> Option<DialogView> {
    match (&m.confirm, &m.hunt.found) {
        (Some(Confirm::CloseHunted(_)), _) => Some(crate::dialog::confirm_view(
            "¿Cerrar este programa?".to_string(),
            "Se cierra a la fuerza, sin darle tiempo a guardar: lo que tenga sin guardar se pierde.",
            "Cerrar programa",
            "x",
        )),
        (None, Some(found)) => Some(hunted_view(found)),
        _ => None,
    }
}

/// The window that says what the sight landed on.
fn hunted_view(found: &Hunted) -> DialogView {
    let program = found.process.name();
    let (title, whose) = match &found.app {
        Some(app) => (app.name.clone(), format!("Es parte de {}.", app.name)),
        None => (
            program.to_string(),
            "No es parte de ninguna app de la lista: puede ser un programa portátil o de Windows."
                .to_string(),
        ),
    };
    let mut buttons = vec![
        button("Cancelar", "x", false, Action::CloseHunt),
        button(
            "Abrir carpeta",
            "folder-open",
            false,
            Action::OpenHuntedFolder,
        ),
        button("Cerrar programa", "x", false, Action::AskCloseHunted),
    ];
    if let Some(app) = found.app.as_ref().filter(|a| a.protection().is_none()) {
        buttons.push(button(
            "Desinstalar",
            "trash-2",
            true,
            Action::AskUninstall(app.id.clone()),
        ));
    }
    DialogView {
        width: 780.0,
        icon: "crosshair",
        tone: Tone::Blue,
        title,
        body: format!("{whose} El programa es {program}, en {}.", found.folder()),
        rows: vec![],
        aside: None,
        corner: Vec::new(),
        buttons,
        cancel: Action::CloseHunt,
    }
}

#[cfg(test)]
mod tests {
    use purifex_domain::apps::InstalledApp;
    use purifex_domain::forced::RunningProcess;

    use super::*;

    fn found(app: Option<InstalledApp>) -> Hunted {
        Hunted {
            process: RunningProcess {
                pid: 5,
                path: r"D:\Games\OBS Studio\bin\obs64.exe".into(),
            },
            app,
        }
    }

    fn labels(v: &DialogView) -> Vec<&str> {
        v.buttons.iter().map(|b| b.label.as_str()).collect()
    }

    #[test]
    fn a_program_of_an_app_offers_to_uninstall_that_app_and_one_of_none_does_not() {
        let obs = InstalledApp {
            id: "HKLM:OBS".into(),
            name: "OBS Studio".into(),
            ..Default::default()
        };
        let with_app = hunted_view(&found(Some(obs)));
        assert_eq!(with_app.title, "OBS Studio");
        assert!(with_app.body.contains("Es parte de OBS Studio"));
        assert_eq!(
            labels(&with_app),
            [
                "Cancelar",
                "Abrir carpeta",
                "Cerrar programa",
                "Desinstalar"
            ]
        );
        assert_eq!(
            with_app.buttons[3].action,
            Some(Action::AskUninstall("HKLM:OBS".into()))
        );
        let without = hunted_view(&found(None));
        assert_eq!(without.title, "obs64.exe");
        assert_eq!(
            labels(&without),
            ["Cancelar", "Abrir carpeta", "Cerrar programa"]
        );
        assert_eq!(without.cancel, Action::CloseHunt);
    }
}
