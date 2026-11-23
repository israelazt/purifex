//! The window that walks through getting rid of an app: uninstall, look for what it left, remove
//! it, done. One window for the whole way, with the steps drawn at its top. The list of traces
//! (step three) is the traces window; the other steps are made here. Which step it is on is read
//! from the model, never kept apart.

use purifex_domain::draw::Rect;
use purifex_domain::view::{Action, Model, Wizard};

use crate::canvas::Canvas;
use crate::dialog::{Btn, DialogView};
use crate::theme::{self, Tone};

/// The steps, in order.
const LABELS: [&str; 4] = ["Desinstalar", "Buscar restos", "Quitar restos", "Listo"];
/// The room the steps take at the top of the window.
pub(crate) const STEPS_H: f32 = 64.0;

/// Which step the window is on (0 to 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Steps(pub usize);

fn button(label: &str, icon: &'static str, primary: bool, action: Action) -> Btn {
    Btn {
        label: label.into(),
        icon,
        primary,
        action: Some(action),
    }
}

/// The step a model is on, when the wizard is open and it is not the list of traces (that one is
/// the traces window, which only needs to know it is step three).
pub(crate) fn view(m: &Model) -> Option<DialogView> {
    let wizard = m.wizard.as_ref()?;
    if m.confirm.is_some() {
        return None;
    }
    let app = wizard.app.as_str();
    let listing = m.traces.is_some() && m.traces_open;
    let (step, title, body, buttons) = if let Some(done) = &wizard.done {
        (
            3,
            format!("Listo: {app}"),
            done_body(m, done),
            done_buttons(m, wizard),
        )
    } else if m.waiting {
        waiting_step(app)
    } else if m.finding.is_some() {
        finding_step(app)
    } else if listing {
        return None;
    } else if m.traces.is_some() {
        nothing_left_step(app)
    } else {
        ask_step(app)
    };
    let body = match &m.error {
        Some(error) => format!("{body} Problema: {error}"),
        None => body,
    };
    Some(window(step, title, body, buttons))
}

type Step = (usize, String, String, Vec<Btn>);

fn waiting_step(app: &str) -> Step {
    (
        0,
        format!("Desinstalando {app}"),
        format!("Se abrió el desinstalador de {app}. Termínalo en su ventana: no hace falta que pulses nada aquí. Cuando {app} desaparezca de la lista, Purifex busca lo que dejó y te lo muestra."),
        vec![
            button("Ocultar", "x", false, Action::CloseWizard),
            button("Dejar de esperar", "x", false, Action::StopWaiting),
        ],
    )
}

fn finding_step(app: &str) -> Step {
    (
        1,
        format!("Buscando lo que dejó {app}…"),
        "Mirando carpetas, accesos, tareas programadas y claves del registro. No se borra nada al buscar.".to_string(),
        vec![button("Ocultar", "x", false, Action::CloseWizard)],
    )
}

/// Searched, and there was nothing to remove.
fn nothing_left_step(app: &str) -> Step {
    (
        3,
        format!("{app} no dejó restos"),
        "No se encontró nada que quitar. Si crees que debería haber algo, prueba otro modo de búsqueda (Avanzado) y busca otra vez.".to_string(),
        vec![
            button("Buscar otra vez", "search", false, Action::FindTraces(app.into())),
            button("Cerrar", "x", true, Action::CloseWizard),
        ],
    )
}

/// The uninstaller is not being waited for and nothing was searched: ask whether it is done.
fn ask_step(app: &str) -> Step {
    (
        0,
        format!("¿Ya desinstalaste {app}?"),
        format!("Purifex ya no espera al desinstalador. Si {app} ya no está instalada, busca lo que dejó."),
        vec![
            button("Cerrar", "x", false, Action::CloseWizard),
            button("Buscar restos", "search", true, Action::FindTraces(app.into())),
        ],
    )
}

fn window(step: usize, title: String, body: String, buttons: Vec<Btn>) -> DialogView {
    DialogView {
        width: 760.0,
        icon: if step == 3 { "circle-check" } else { "trash-2" },
        tone: if step == 3 { Tone::Green } else { Tone::Amber },
        title,
        body,
        rows: vec![],
        aside: None,
        corner: Vec::new(),
        buttons,
        cancel: Action::CloseWizard,
        steps: Some(Steps(step)),
    }
}

/// What the removal did, and what is still worth doing about it.
fn done_body(m: &Model, done: &str) -> String {
    let mut text = done.to_string();
    if m.in_use.is_some() {
        text.push_str(" Hay archivos que un programa tiene abiertos: puedes dejar que Windows los borre al reiniciar.");
    }
    if m.running.is_some() {
        text.push_str(" Quedan programas de la app abiertos: ciérralos para soltar sus archivos.");
    }
    text
}

fn done_buttons(m: &Model, wizard: &Wizard) -> Vec<Btn> {
    let app = wizard.app.clone();
    let mut buttons = Vec::new();
    if m.running.is_some() {
        buttons.push(button(
            "Cerrar programas",
            "x",
            false,
            Action::AskCloseProcesses(app.clone()),
        ));
    }
    if m.in_use.is_some() {
        buttons.push(button(
            "Borrar al reiniciar",
            "trash-2",
            false,
            Action::AskDeleteAtReboot(app.clone()),
        ));
    }
    buttons.push(button(
        "Devolver todo",
        "history",
        false,
        Action::RestoreApp(app),
    ));
    buttons.push(button("Cerrar", "circle-check", true, Action::CloseWizard));
    buttons
}

/// The four steps across the top of the window: the ones done are lit and ticked, the current one
/// is lit, the rest are dim.
pub(crate) fn draw_steps(c: &mut Canvas, rect: Rect, steps: Steps) {
    let cell = rect.w / LABELS.len() as f32;
    for (i, label) in LABELS.iter().enumerate() {
        let (x, y) = (rect.x + cell * i as f32, rect.y + 16.0);
        let reached = i <= steps.0;
        let color = if reached {
            theme::ACCENT
        } else {
            theme::TEXT_3
        };
        let dot = Rect::new(x + 24.0, y + 2.0, 24.0, 24.0);
        if i < steps.0 || steps.0 == LABELS.len() - 1 && i == steps.0 {
            c.icon("circle-check", dot.x, dot.y, 24.0, color);
        } else {
            c.stroke(dot, 12.0, 1.5, color);
            c.text_in(
                dot.x + 8.0,
                dot,
                &(i + 1).to_string(),
                theme::mono(11.5, color).bold(),
            );
        }
        c.text(
            x + 56.0,
            y + 5.0,
            label,
            theme::mono(12.0, if reached { theme::TEXT } else { theme::TEXT_3 }),
        );
    }
    c.line(
        rect.x,
        rect.y + STEPS_H - 8.0,
        rect.right(),
        rect.y + STEPS_H - 8.0,
        theme::LINE,
    );
}

#[cfg(test)]
mod tests {
    use purifex_domain::view::Confirm;

    use super::*;

    fn model(wizard: Wizard) -> Model {
        Model {
            wizard: Some(wizard),
            ..Model::default()
        }
    }

    fn step_of(m: &Model) -> Option<usize> {
        view(m).and_then(|v| v.steps).map(|s| s.0)
    }

    #[test]
    fn each_state_of_the_removal_is_its_own_step_and_the_list_of_traces_is_left_to_its_window() {
        let mut m = model(Wizard::new("Zed"));
        // Nothing going on and nothing found: the person is asked to look for the traces.
        assert_eq!(step_of(&m), Some(0));
        m.waiting = true;
        let waiting = view(&m).unwrap();
        assert_eq!(waiting.steps, Some(Steps(0)));
        assert!(waiting
            .buttons
            .iter()
            .any(|b| b.action == Some(Action::StopWaiting)));
        m.waiting = false;
        m.finding = Some("Zed".into());
        assert_eq!(step_of(&m), Some(1));
        m.finding = None;
        m.traces = Some(purifex_domain::cleanup_log::CleanReport {
            app: "Zed".into(),
            ..Default::default()
        });
        assert_eq!(
            step_of(&m),
            Some(3),
            "searched and found nothing: it is over"
        );
        m.traces = Some(purifex_domain::cleanup_log::CleanReport {
            app: "Zed".into(),
            proposed: vec![Default::default()],
            ..Default::default()
        });
        m.traces_open = true;
        assert!(view(&m).is_none(), "step 3 is the traces window");
    }

    #[test]
    fn the_last_step_offers_what_is_left_to_do_and_a_question_goes_first() {
        let mut m = model(Wizard {
            app: "Zed".into(),
            done: Some("Se quitaron 3.".into()),
        });
        m.in_use = Some(purifex_domain::view::InUse {
            app: "Zed".into(),
            items: 1,
        });
        let v = view(&m).unwrap();
        assert_eq!(v.steps, Some(Steps(3)));
        let actions: Vec<_> = v.buttons.iter().filter_map(|b| b.action.clone()).collect();
        for wanted in [
            Action::AskDeleteAtReboot("Zed".into()),
            Action::RestoreApp("Zed".into()),
            Action::CloseWizard,
        ] {
            assert!(actions.contains(&wanted), "{wanted:?}");
        }
        assert!(v.body.contains("al reiniciar"));
        m.confirm = Some(Confirm::DeleteAtReboot("Zed".into()));
        assert!(
            view(&m).is_none(),
            "the question is asked over the window, not under it"
        );
        assert!(view(&Model::default()).is_none());
    }
}
