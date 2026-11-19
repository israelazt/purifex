//! Dialogs: a window over the page that dims it, takes the pointer and asks for a
//! decision. They fade and slide in and out (`Anims`), and while they fade out they
//! are drawn from the last view that was shown, since the model no longer has it.
//! What each dialog says is built here from the model; what its buttons do is an
//! `Action` like any other.

use purifex_domain::apps::size_text;
use purifex_domain::draw::{Color, Rect};
use purifex_domain::view::{Action, Confirm, Model, TRACES_LISTED};

use crate::canvas::Canvas;
use crate::theme::{self, Tone};
use crate::widgets::{button, button_width, icon_box, panel, tag};

/// How long a dialog takes to appear or go away, in ms.
pub const DIALOG_MS: u64 = 190;
/// How far below its place a dialog starts, in units.
const SLIDE: f32 = 18.0;
const ROW_H: f32 = 40.0;

/// One line of a list with a checkbox.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub label: String,
    pub detail: String,
    pub size: String,
    pub user_data: bool,
    pub checked: bool,
    pub toggle: Action,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Btn {
    pub label: String,
    pub icon: &'static str,
    pub primary: bool,
    pub action: Option<Action>,
}

/// Everything a dialog needs to be drawn.
#[derive(Debug, Clone, PartialEq)]
pub struct DialogView {
    pub width: f32,
    pub icon: &'static str,
    pub tone: Tone,
    pub title: String,
    pub body: String,
    pub rows: Vec<Row>,
    /// Rows that did not fit the list.
    pub more: usize,
    /// A link-like button at the left of the buttons (select all / none).
    pub aside: Option<Btn>,
    pub buttons: Vec<Btn>,
    /// What Escape does.
    pub cancel: Action,
}

const CLEAN_BODY: &str = "Los restos se mueven a una cuarentena (las claves del registro se guardan antes) y quedan en el historial, donde puedes devolverlos. Incluye datos tuyos de la app, como ajustes y partidas guardadas.";

fn confirm_view(title: String, body: &str, yes: &str, icon: &'static str) -> DialogView {
    DialogView {
        width: 520.0,
        icon: "triangle-alert",
        tone: Tone::Amber,
        title,
        body: body.to_string(),
        rows: vec![],
        more: 0,
        aside: None,
        buttons: vec![
            Btn {
                label: "Cancelar".into(),
                icon: "x",
                primary: false,
                action: Some(Action::CancelConfirm),
            },
            Btn {
                label: yes.into(),
                icon,
                primary: true,
                action: Some(Action::Confirm),
            },
        ],
        cancel: Action::CancelConfirm,
    }
}

/// The dialog the model asks for, if any.
pub fn view(m: &Model) -> Option<DialogView> {
    let traces = m.traces.as_ref().filter(|_| m.traces_open);
    match (&m.confirm, traces) {
        (Some(Confirm::Clean(name)), _) => Some(confirm_view(
            format!("¿Quitar los restos marcados de {name}?"),
            CLEAN_BODY,
            "Quitar restos",
            "trash-2",
        )),
        (Some(Confirm::CleanAll(name)), _) => Some(confirm_view(
            format!("¿Eliminar todo rastro de {name}?"),
            CLEAN_BODY,
            "Eliminar todo",
            "trash-2",
        )),
        (Some(Confirm::Uninstall(id)), _) => {
            let name = m
                .apps
                .iter()
                .flatten()
                .find(|a| a.id == *id)
                .map_or(id.as_str(), |a| a.name.as_str());
            Some(confirm_view(
                format!("¿Desinstalar {name}?"),
                "Se abrirá su desinstalador oficial y lo terminas tú en su ventana. Cuando termine, Purifex busca lo que deje y te lo muestra.",
                "Desinstalar",
                "trash-2",
            ))
        }
        (_, Some(report)) => Some(traces_view(m, report)),
        _ => None,
    }
}

fn traces_view(m: &Model, report: &purifex_domain::cleanup_log::CleanReport) -> DialogView {
    let rows: Vec<Row> = report
        .proposed
        .iter()
        .take(TRACES_LISTED)
        .map(|t| Row {
            label: t.location.clone(),
            detail: t.why.clone(),
            size: if t.bytes > 0 {
                size_text(Some(t.bytes / 1024))
            } else {
                String::new()
            },
            user_data: t.user_data,
            checked: !m.unchecked.contains(&t.location),
            toggle: Action::ToggleTrace(t.location.clone()),
        })
        .collect();
    let checked = rows.iter().filter(|r| r.checked).count();
    let name = report.app.clone();
    let all_checked = checked == rows.len();
    DialogView {
        width: 900.0,
        icon: "trash-2",
        tone: Tone::Amber,
        title: format!("Restos de {name}"),
        body: "Esto es lo que quedó después de desinstalar. Marca lo que quieras quitar, o elimina todo de una vez sin elegir.".into(),
        more: report.proposed.len().saturating_sub(TRACES_LISTED),
        rows,
        aside: Some(Btn {
            label: if all_checked { "Quitar marcas".into() } else { "Marcar todo".into() },
            icon: "list-checks",
            primary: false,
            action: Some(Action::SelectTraces(!all_checked)),
        }),
        buttons: vec![
            Btn { label: "Cerrar".into(), icon: "x", primary: false, action: Some(Action::CloseTraces) },
            Btn {
                label: format!("Eliminar marcados ({checked})"),
                icon: "trash-2",
                primary: false,
                action: (checked > 0).then(|| Action::AskClean(name.clone())),
            },
            Btn { label: "Eliminar todo rastro".into(), icon: "trash-2", primary: true, action: Some(Action::AskCleanAll(name)) },
        ],
        cancel: Action::CloseTraces,
    }
}

fn checkbox(c: &mut Canvas, x: f32, y: f32, checked: bool) {
    if checked {
        c.icon("circle-check", x, y, 20.0, theme::ACCENT);
    } else {
        c.icon("square", x, y, 20.0, theme::TEXT_3);
    }
}

fn row(c: &mut Canvas, r: &Row, bar: Rect) {
    let hover = c.hover(bar);
    if hover > 0.01 {
        c.fill(bar, 6.0, theme::TEXT.with_alpha(0.04 * hover));
    }
    checkbox(c, bar.x + 12.0, bar.y + 10.0, r.checked);
    let name = theme::mono(
        12.0,
        if r.checked {
            theme::TEXT
        } else {
            theme::TEXT_3
        },
    );
    c.text_cut(bar.x + 44.0, bar.y + 4.0, &r.label, name, bar.w - 250.0);
    c.text_cut(
        bar.x + 44.0,
        bar.y + 21.0,
        &r.detail,
        theme::mono(10.5, theme::TEXT_3),
        bar.w - 250.0,
    );
    c.text(
        bar.right() - 190.0,
        bar.y + 12.0,
        &r.size,
        theme::mono(11.5, theme::TEXT_2),
    );
    if r.user_data {
        tag(
            c,
            bar.right() - 112.0,
            bar.y + 9.0,
            "datos tuyos",
            Tone::Amber,
            None,
        );
    }
    c.hit(bar, r.toggle.clone());
}

/// The height of the dialog.
fn height_of(c: &Canvas, v: &DialogView) -> f32 {
    let body = c
        .wrap(
            &v.body,
            &theme::mono(12.5, theme::TEXT_2),
            v.width - 56.0,
            Some(4),
        )
        .len() as f32
        * 19.0;
    let list = if v.rows.is_empty() {
        0.0
    } else {
        16.0 + v.rows.len() as f32 * ROW_H + if v.more > 0 { 26.0 } else { 0.0 }
    };
    24.0 + 40.0 + 14.0 + body + list + 24.0 + 44.0 + 24.0
}

/// Draws the dialog at `t` (0 gone, 1 settled) over a window of `width` x `height`.
pub fn draw(c: &mut Canvas, v: &DialogView, t: f32, width: f32, height: f32) {
    // The page goes dim.
    c.rest();
    c.fill(
        Rect::new(0.0, 0.0, width, height),
        0.0,
        Color::rgb(0, 0, 0).with_alpha(0.62 * t),
    );
    // The window fades in and rises a little.
    c.fade(t, (1.0 - t) * SLIDE);
    let h = height_of(c, v).min(height - 40.0);
    let rect = Rect::new((width - v.width) / 2.0, (height - h) / 2.0, v.width, h);
    c.fill(rect.inset(-6.0), 16.0, Color::rgb(0, 0, 0).with_alpha(0.35));
    panel(c, rect);
    icon_box(c, rect.x + 24.0, rect.y + 24.0, 40.0, v.icon, v.tone);
    c.text(
        rect.x + 78.0,
        rect.y + 32.0,
        &v.title,
        theme::sans(18.0, theme::TEXT).bold(),
    );
    let mut y = rect.y + 24.0 + 40.0 + 14.0;
    y += c.paragraph(
        rect.x + 28.0,
        y,
        v.width - 56.0,
        &v.body,
        theme::mono(12.5, theme::TEXT_2),
        Some(4),
    );
    if !v.rows.is_empty() {
        y += 16.0;
        c.line(rect.x, y - 8.0, rect.right(), y - 8.0, theme::LINE);
        for r in &v.rows {
            row(c, r, Rect::new(rect.x + 12.0, y, rect.w - 24.0, ROW_H));
            y += ROW_H;
        }
        if v.more > 0 {
            let note = format!(
                "y {} más: solo se quitan con «Eliminar todo rastro»",
                v.more
            );
            c.text(
                rect.x + 28.0,
                y + 4.0,
                &note,
                theme::mono(11.5, theme::TEXT_3),
            );
        }
    }
    // The buttons, at the bottom right; the aside one at the left.
    let by = rect.bottom() - 24.0 - 44.0;
    let mut x = rect.right() - 24.0;
    for b in v.buttons.iter().rev() {
        let w = button_width(c, &b.label);
        x -= w;
        button(
            c,
            Rect::new(x, by, w, 44.0),
            &b.label,
            b.icon,
            b.primary,
            b.action.clone(),
        );
        x -= 12.0;
    }
    if let Some(b) = &v.aside {
        let w = button_width(c, &b.label);
        button(
            c,
            Rect::new(rect.x + 24.0, by, w, 44.0),
            &b.label,
            b.icon,
            false,
            b.action.clone(),
        );
    }
    c.rest();
}

#[cfg(test)]
mod tests {
    use purifex_domain::app_traces::{Trace, TraceKind};
    use purifex_domain::apps::{AppKind, InstalledApp};
    use purifex_domain::cleanup_log::CleanReport;
    use purifex_domain::draw::Op;

    use super::*;
    use crate::canvas::testing::FakeText;

    fn trace(location: &str, user_data: bool) -> Trace {
        Trace {
            kind: TraceKind::Folder,
            location: location.into(),
            bytes: 2048,
            why: "lleva el nombre de la app".into(),
            user_data,
            value: None,
            evidence: Default::default(),
        }
    }

    fn model_with_traces(n: usize) -> Model {
        Model {
            traces_open: true,
            traces: Some(CleanReport {
                app: "OBS Studio".into(),
                preview: true,
                removed: vec![],
                proposed: (0..n)
                    .map(|i| trace(&format!(r"C:\ProgramData\obs{i}"), i == 0))
                    .collect(),
                failed: vec![],
            }),
            ..Model::default()
        }
    }

    fn labels(v: &DialogView) -> Vec<&str> {
        v.buttons.iter().map(|b| b.label.as_str()).collect()
    }

    fn render(v: &DialogView, t: f32) -> (Vec<Op>, Vec<Action>) {
        let mut c = Canvas::new(&FakeText, None);
        draw(&mut c, v, t, 1440.0, 900.0);
        let (list, hits) = c.finish();
        (list.ops, hits.into_iter().map(|h| h.action).collect())
    }

    #[test]
    fn without_a_question_or_a_list_there_is_no_dialog() {
        assert_eq!(view(&Model::default()), None);
        let mut m = model_with_traces(2);
        m.traces_open = false;
        assert_eq!(view(&m), None, "closed");
    }

    #[test]
    fn the_list_of_traces_offers_all_selected_or_everything_and_counts_what_is_checked() {
        let mut m = model_with_traces(3);
        let v = view(&m).unwrap();
        assert_eq!(v.rows.len(), 3);
        assert!(
            v.rows.iter().all(|r| r.checked),
            "everything starts checked"
        );
        assert_eq!(
            labels(&v),
            ["Cerrar", "Eliminar marcados (3)", "Eliminar todo rastro"]
        );
        assert!(v.buttons[2].primary, "removing it all is the main way");
        assert_eq!(
            v.buttons[2].action,
            Some(Action::AskCleanAll("OBS Studio".into()))
        );
        assert_eq!(v.cancel, Action::CloseTraces);
        m.unchecked = vec![r"C:\ProgramData\obs1".into()];
        let v = view(&m).unwrap();
        assert_eq!(labels(&v)[1], "Eliminar marcados (2)");
        assert!(!v.rows[1].checked);
        assert_eq!(v.aside.as_ref().unwrap().label, "Marcar todo");
        m.unchecked = (0..3).map(|i| format!(r"C:\ProgramData\obs{i}")).collect();
        let v = view(&m).unwrap();
        assert_eq!(
            v.buttons[1].action, None,
            "nothing checked: nothing to remove"
        );
    }

    #[test]
    fn more_traces_than_fit_say_so_and_only_the_all_button_takes_them() {
        let v = view(&model_with_traces(TRACES_LISTED + 3)).unwrap();
        assert_eq!(v.rows.len(), TRACES_LISTED);
        assert_eq!(v.more, 3);
        let (ops, _) = render(&v, 1.0);
        assert!(ops
            .iter()
            .any(|o| matches!(o, Op::Text { text, .. } if text.contains("y 3 más"))));
    }

    #[test]
    fn a_confirmation_replaces_the_list_and_cancel_goes_back_to_it() {
        let mut m = model_with_traces(2);
        m.confirm = Some(Confirm::CleanAll("OBS Studio".into()));
        let v = view(&m).unwrap();
        assert!(v.title.contains("Eliminar todo rastro de OBS Studio"));
        assert_eq!(labels(&v), ["Cancelar", "Eliminar todo"]);
        assert_eq!(v.cancel, Action::CancelConfirm);
        let app = InstalledApp {
            id: "HKLM64:Discord".into(),
            name: "Discord".into(),
            publisher: None,
            version: None,
            install_location: None,
            uninstall: Some("u".into()),
            quiet_uninstall: None,
            size_kb: None,
            kind: AppKind::Desktop,
            hidden: false,
            display_icon: None,
            observed: Default::default(),
        };
        let m = Model {
            apps: Some(vec![app]),
            confirm: Some(Confirm::Uninstall("HKLM64:Discord".into())),
            ..Model::default()
        };
        assert!(view(&m).unwrap().title.contains("¿Desinstalar Discord?"));
    }

    #[test]
    fn a_dialog_fades_and_rises_while_it_appears_and_is_in_place_when_settled() {
        let v = view(&model_with_traces(2)).unwrap();
        let scrim_alpha = |ops: &[Op]| match &ops[0] {
            Op::Fill { color, .. } => color.a,
            other => panic!("{other:?}"),
        };
        let title_at = |ops: &[Op]| {
            ops.iter().find_map(|o| match o {
                Op::Text { text, y, style, .. } if text == "Restos de OBS Studio" => {
                    Some((*y, style.color.a))
                }
                _ => None,
            })
        };
        let (start, _) = render(&v, 0.2);
        let (end, _) = render(&v, 1.0);
        assert!(
            scrim_alpha(&start) < scrim_alpha(&end),
            "the page dims little by little"
        );
        let ((y0, a0), (y1, a1)) = (title_at(&start).unwrap(), title_at(&end).unwrap());
        assert!(
            y0 > y1 && a0 < a1,
            "it rises and fades in: {y0} {a0} -> {y1} {a1}"
        );
        assert_eq!(a1, 255);
    }

    #[test]
    fn the_rows_toggle_and_the_buttons_act_and_nothing_else_is_clickable() {
        let v = view(&model_with_traces(2)).unwrap();
        let (_, actions) = render(&v, 1.0);
        assert!(actions.contains(&Action::ToggleTrace(r"C:\ProgramData\obs0".into())));
        assert!(actions.contains(&Action::AskClean("OBS Studio".into())));
        assert!(actions.contains(&Action::SelectTraces(false)));
        assert!(actions.contains(&Action::CloseTraces));
    }
}
