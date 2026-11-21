//! Dialogs: a window over the page that dims it, takes the pointer and asks for a
//! decision. They fade and slide in and out (`Anims`), and while they fade out they
//! are drawn from the last view that was shown, since the model no longer has it.
//! What each dialog says is built here from the model; what its buttons do is an
//! `Action` like any other.

use purifex_domain::apps::size_text;
use purifex_domain::draw::{Color, Rect};
use purifex_domain::view::{Action, Confirm, Model};

use crate::canvas::Canvas;
use crate::theme::{self, Tone};
use crate::traces_dialog::traces_view;
use crate::widgets::{button, button_width, icon_box, panel, tag};

/// How long a dialog takes to appear or go away, in ms.
pub const DIALOG_MS: u64 = 190;
/// How far below its place a dialog starts, in units.
const SLIDE: f32 = 18.0;
const ROW_H: f32 = 40.0;
/// The height of the heading of a group of rows.
const HEADING_H: f32 = 30.0;
/// The most the list of rows shows at once: more rows scroll inside it, so the window fits in
/// the smallest the program can be.
const LIST_MAX_H: f32 = 9.0 * ROW_H + 2.0 * HEADING_H;

/// One line of a list with a checkbox.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// The heading of the group this row opens (drawn above it), if it starts one.
    pub heading: Option<String>,
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
    /// A link-like button at the left of the buttons (select all / none).
    pub aside: Option<Btn>,
    /// Buttons in the top right corner, beside the title, from the right edge in (a way out
    /// of what the dialog warns about, or a setting of it: they do not fit among the buttons).
    pub corner: Vec<Btn>,
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
        aside: None,
        corner: Vec::new(),
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
        (Some(Confirm::EmptyQuarantine), _) => Some(confirm_view(
            "¿Vaciar la cuarentena?".to_string(),
            &empty_quarantine_body(m),
            "Vaciar para siempre",
            "trash-2",
        )),
        (Some(Confirm::RestartAsAdmin(name)), _) => Some(confirm_view(
            "¿Reiniciar Purifex como administrador?".to_string(),
            &format!("Windows te lo preguntará una vez. Purifex se cierra, se abre de nuevo con permisos de administrador y vuelve a buscar los restos de {name} para que puedas quitarlos todos."),
            "Reiniciar",
            "shield-check",
        )),
        (Some(Confirm::DeleteAtReboot(name)), _) => Some(confirm_view(
            format!("¿Borrar al reiniciar lo que {name} dejó abierto?"),
            "Estos archivos los tiene abiertos un programa y no se pueden mover a la cuarentena. Windows los borrará la próxima vez que reinicies el PC, y NO se podrán devolver. Hace falta ser administrador.",
            "Borrar al reiniciar",
            "trash-2",
        )),
        (Some(Confirm::ForceUninstall(id)), _) => Some(force_uninstall_view(m, id)),
        (Some(Confirm::CloseProcesses(name)), _) => Some(confirm_view(
            format!("¿Cerrar los programas de {name}?"),
            "Se cierran a la fuerza, sin darles tiempo a guardar: lo que tengan sin guardar se pierde. Es para soltar los archivos de la app y poder quitar sus restos.",
            "Cerrar programas",
            "x",
        )),
        (Some(Confirm::Uninstall(id)), _) => Some(uninstall_view(m, id)),
        (_, Some(report)) => Some(traces_view(m, report)),
        _ => None,
    }
}

/// The question before an app's own uninstaller starts.
fn uninstall_view(m: &Model, id: &str) -> DialogView {
    let name = m
        .apps
        .iter()
        .flatten()
        .find(|a| a.id == id)
        .map_or(id, |a| a.name.as_str());
    confirm_view(
        format!("¿Desinstalar {name}?"),
        "Se abrirá su desinstalador oficial y lo terminas tú en su ventana. Cuando termine, Purifex busca lo que deje y te lo muestra.",
        "Desinstalar",
        "trash-2",
    )
}

/// The question before a forced uninstall: what it does and when it is for.
fn force_uninstall_view(m: &Model, id: &str) -> DialogView {
    let name = m
        .apps
        .iter()
        .flatten()
        .find(|a| a.id == id)
        .map_or(id, |a| a.name.as_str());
    confirm_view(
        format!("¿Desinstalación forzada de {name}?"),
        "No se usa su desinstalador: se quita su entrada del registro (con copia, se puede devolver) y Purifex busca lo que dejó para que lo quites. Sus archivos siguen donde están hasta que los quites tú. Úsala solo si el desinstalador de la app falla o ya no existe.",
        "Forzar",
        "zap",
    )
}

/// What emptying the quarantine means: what goes, and that it cannot be put back.
fn empty_quarantine_body(m: &Model) -> String {
    let held = match &m.quarantine {
        Some(q) if q.items > 0 => format!(
            "{} restos ({})",
            q.items,
            size_text(Some(q.bytes.div_ceil(1024)))
        ),
        _ => "lo que guarda".to_string(),
    };
    format!(
        "Se borran para siempre {held} que quitaste de tu PC. Después ya no podrás devolverlos con «purifex apps restore»."
    )
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

/// How the rows sit in their window: all of them are `content` tall, and `view` of that shows.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ListMetrics {
    pub content: f32,
    pub view: f32,
}

impl ListMetrics {
    /// How far the rows can be scrolled.
    pub fn max_scroll(self) -> f32 {
        (self.content - self.view).max(0.0)
    }
}

fn list_metrics(v: &DialogView) -> ListMetrics {
    let headings = v.rows.iter().filter(|r| r.heading.is_some()).count();
    let content = v.rows.len() as f32 * ROW_H + headings as f32 * HEADING_H;
    ListMetrics {
        content,
        view: content.min(LIST_MAX_H),
    }
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
        16.0 + list_metrics(v).view
    };
    24.0 + 40.0 + 14.0 + body + list + 24.0 + 44.0 + 24.0
}

/// Draws the dialog at `t` (0 gone, 1 settled) over a window of `width` x `height`, with its
/// rows scrolled by `scroll`. Says how the rows sit, so the wheel knows how far it may go.
pub fn draw(
    c: &mut Canvas,
    v: &DialogView,
    t: f32,
    (width, height): (f32, f32),
    scroll: f32,
) -> ListMetrics {
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
    let y = heading_and_body(c, v, rect);
    if !v.rows.is_empty() {
        list(c, v, rect, y, scroll);
    }
    buttons(c, v, rect);
    c.rest();
    list_metrics(v)
}

/// The icon, the title and the paragraph of the dialog; returns where the paragraph ends.
fn heading_and_body(c: &mut Canvas, v: &DialogView, rect: Rect) -> f32 {
    icon_box(c, rect.x + 24.0, rect.y + 24.0, 40.0, v.icon, v.tone);
    let mut right = rect.right() - 24.0;
    for b in v.corner.iter().rev() {
        let w = button_width(c, &b.label);
        let place = Rect::new(right - w, rect.y + 22.0, w, 44.0);
        button(c, place, &b.label, b.icon, false, b.action.clone());
        right -= w + 12.0;
    }
    c.text(
        rect.x + 78.0,
        rect.y + 32.0,
        &v.title,
        theme::sans(18.0, theme::TEXT).bold(),
    );
    let y = rect.y + 24.0 + 40.0 + 14.0;
    y + c.paragraph(
        rect.x + 28.0,
        y,
        v.width - 56.0,
        &v.body,
        theme::mono(12.5, theme::TEXT_2),
        Some(4),
    )
}

/// The checkable rows from `y` down, inside a window that clips them and that `scroll` moves
/// over them. Rows out of the window are not drawn.
fn list(c: &mut Canvas, v: &DialogView, rect: Rect, y: f32, scroll: f32) {
    let metrics = list_metrics(v);
    let window = Rect::new(rect.x, y + 16.0, rect.w, metrics.view);
    c.line(
        rect.x,
        window.y - 8.0,
        rect.right(),
        window.y - 8.0,
        theme::LINE,
    );
    c.clip(window);
    let mut at = window.y - scroll.clamp(0.0, metrics.max_scroll());
    for r in &v.rows {
        let height = ROW_H + if r.heading.is_some() { HEADING_H } else { 0.0 };
        if at + height >= window.y && at <= window.bottom() {
            if let Some(heading) = &r.heading {
                let place = rect.x + 28.0;
                c.text(place, at + 8.0, heading, theme::eyebrow(theme::TEXT_3));
            }
            let row_at = at + height - ROW_H;
            row(c, r, Rect::new(rect.x + 12.0, row_at, rect.w - 24.0, ROW_H));
        }
        at += height;
    }
    c.unclip();
    scrollbar(c, window, metrics, scroll);
}

/// A thin bar at the right edge of the rows, only when they do not all fit.
fn scrollbar(c: &mut Canvas, window: Rect, metrics: ListMetrics, scroll: f32) {
    let max = metrics.max_scroll();
    if max <= 0.0 {
        return;
    }
    let thumb = (window.h * window.h / metrics.content).max(28.0);
    let top = window.y + (scroll.clamp(0.0, max) / max) * (window.h - thumb);
    let bar = Rect::new(window.right() - 7.0, top, 3.0, thumb);
    c.fill(bar, 1.5, theme::TEXT_3.with_alpha(0.55));
}

/// The buttons at the bottom right, and the aside one at the left.
fn buttons(c: &mut Canvas, v: &DialogView, rect: Rect) {
    let by = rect.bottom() - 24.0 - 44.0;
    let mut x = rect.right() - 24.0;
    for b in v.buttons.iter().rev() {
        let w = button_width(c, &b.label);
        x -= w;
        let place = Rect::new(x, by, w, 44.0);
        button(c, place, &b.label, b.icon, b.primary, b.action.clone());
        x -= 12.0;
    }
    if let Some(b) = &v.aside {
        let w = button_width(c, &b.label);
        let place = Rect::new(rect.x + 24.0, by, w, 44.0);
        button(c, place, &b.label, b.icon, false, b.action.clone());
    }
}

#[cfg(test)]
mod tests {
    use purifex_domain::app_traces::{Evidence, Trace, TraceKind};
    use purifex_domain::apps::InstalledApp;
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
            ..Default::default()
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
                ..Default::default()
            }),
            ..Model::default()
        }
    }

    /// A report with `files` folders and `registry` values of one key (their size in `bytes`).
    fn mixed_report(files: usize, registry: usize) -> CleanReport {
        let file = |i: usize| Trace {
            location: format!(r"C:\Users\u\AppData\Local\app{i}"),
            bytes: 10_000 - i as u64,
            evidence: Evidence::NameMatch,
            ..Default::default()
        };
        let value = |i: usize| Trace {
            kind: TraceKind::RegistryValue,
            location: r"HKCU\Software\Cache".into(),
            value: Some(format!("app{i}.exe.FriendlyAppName")),
            evidence: Evidence::PathReference,
            ..Default::default()
        };
        CleanReport {
            app: "Kdenlive".into(),
            preview: true,
            proposed: (0..files)
                .map(file)
                .chain((0..registry).map(value))
                .collect(),
            ..Default::default()
        }
    }

    fn model_with(report: CleanReport) -> Model {
        Model {
            traces_open: true,
            traces: Some(report),
            ..Model::default()
        }
    }

    #[test]
    fn the_list_is_grouped_with_how_many_there_are_of_each_and_says_what_proves_each_trace() {
        let v = view(&model_with(mixed_report(7, 10))).unwrap();
        let headings: Vec<&str> = v.rows.iter().filter_map(|r| r.heading.as_deref()).collect();
        assert_eq!(
            headings,
            ["Archivos y carpetas (7) · 69 KB", "Registro (10)"].map(|h| h)
        );
        let files = v.rows.iter().filter(|r| r.label.starts_with("C:")).count();
        let registry = v.rows.len() - files;
        assert_eq!((files, registry), (7, 10), "all of them are listed");
        let value_row = v.rows.iter().find(|r| r.label.starts_with("HKCU")).unwrap();
        assert_eq!(
            value_row.detail,
            "referencia a la carpeta de la app · valor: app0.exe.FriendlyAppName"
        );
        assert_eq!(v.rows[0].detail, "nombre de la app");
    }

    #[test]
    fn two_values_of_one_key_are_checked_one_by_one() {
        let mut m = model_with(mixed_report(0, 2));
        let v = view(&m).unwrap();
        assert_eq!(v.rows.len(), 2);
        let (first, second) = (v.rows[0].toggle.clone(), v.rows[1].toggle.clone());
        assert_ne!(first, second, "same key, different values");
        let Action::ToggleTrace(key) = first else {
            panic!("a row toggles a trace");
        };
        m.unchecked = vec![key];
        let v = view(&m).unwrap();
        assert_eq!(
            v.rows.iter().map(|r| r.checked).collect::<Vec<_>>(),
            [false, true]
        );
    }

    #[test]
    fn a_search_cut_short_says_so_in_the_window() {
        let mut report = mixed_report(1, 1);
        assert!(!view(&model_with(report.clone()))
            .unwrap()
            .body
            .contains("se cortó"));
        report.incomplete = true;
        assert!(view(&model_with(report))
            .unwrap()
            .body
            .contains("se cortó por tiempo"));
    }

    #[test]
    fn the_window_is_as_tall_as_its_headings_and_rows_need() {
        let one_kind = view(&model_with(mixed_report(3, 0))).unwrap();
        let two_kinds = view(&model_with(mixed_report(2, 1))).unwrap();
        let c = Canvas::new(&FakeText, None);
        assert_eq!(
            height_of(&c, &two_kinds) - height_of(&c, &one_kind),
            HEADING_H,
            "same three rows, one more heading"
        );
    }

    #[test]
    fn what_only_an_administrator_can_remove_is_said_while_the_program_is_not_one() {
        let report = CleanReport {
            app: "Kdenlive".into(),
            proposed: vec![
                Trace {
                    kind: TraceKind::RegistryKey,
                    location: r"HKLM\SOFTWARE\Classes\.kdenlive".into(),
                    ..Default::default()
                },
                trace(r"C:\Users\u\AppData\Local\kdenlive", false),
            ],
            ..Default::default()
        };
        let mut m = model_with(report);
        m.elevated = Some(false);
        let v = view(&m).unwrap();
        let with_note = v
            .rows
            .iter()
            .filter(|r| r.detail.contains("requiere administrador"))
            .count();
        assert_eq!(with_note, 1);
        assert!(v
            .body
            .contains("1 de estos restos solo los quita un administrador"));
        let corner = v.corner.last().unwrap();
        assert_eq!(corner.label, "Reiniciar como administrador");
        assert_eq!(
            corner.action,
            Some(Action::AskRestartAsAdmin("Kdenlive".into()))
        );
        m.elevated = Some(true);
        let v = view(&m).unwrap();
        assert_eq!(v.corner.len(), 1, "only the mode is left");
        assert!(v.rows.iter().all(|r| !r.detail.contains("administrador")));
        assert!(!v.body.contains("administrador"));
        m.elevated = None;
        assert!(!view(&m).unwrap().body.contains("administrador"));
    }

    #[test]
    fn emptying_the_quarantine_says_what_goes_and_that_it_cannot_be_put_back() {
        let mut m = Model {
            confirm: Some(Confirm::EmptyQuarantine),
            quarantine: Some(purifex_domain::quarantine::QuarantineInfo {
                items: 4,
                bytes: 2 * 1024 * 1024,
                oldest: None,
                expired: 0,
                ..Default::default()
            }),
            ..Model::default()
        };
        let v = view(&m).unwrap();
        assert!(v.title.contains("Vaciar la cuarentena"));
        assert!(
            v.body.contains("4 restos (2 MB)") && v.body.contains("para siempre"),
            "{}",
            v.body
        );
        assert_eq!(labels(&v), ["Cancelar", "Vaciar para siempre"]);
        assert_eq!(v.cancel, Action::CancelConfirm);
        m.quarantine = None;
        assert!(
            view(&m).unwrap().body.contains("lo que guarda"),
            "without the numbers it still says it"
        );
    }

    fn labels(v: &DialogView) -> Vec<&str> {
        v.buttons.iter().map(|b| b.label.as_str()).collect()
    }

    fn render(v: &DialogView, t: f32) -> (Vec<Op>, Vec<Action>) {
        let mut c = Canvas::new(&FakeText, None);
        draw(&mut c, v, t, (1440.0, 900.0), 0.0);
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
    fn more_traces_than_fit_scroll_inside_a_window_of_fixed_height() {
        let few = view(&model_with_traces(3)).unwrap();
        let many = view(&model_with_traces(40)).unwrap();
        assert_eq!(many.rows.len(), 40, "all of them are listed");
        let (m_few, m_many) = (list_metrics(&few), list_metrics(&many));
        assert_eq!(m_few.max_scroll(), 0.0, "they fit: nothing to scroll");
        assert_eq!(
            m_many.view, LIST_MAX_H,
            "the window does not grow with the rows"
        );
        assert_eq!(m_many.max_scroll(), m_many.content - LIST_MAX_H);
        let c = Canvas::new(&FakeText, None);
        assert!(height_of(&c, &many) - height_of(&c, &few) <= LIST_MAX_H);
        // Scrolled to the end, the last row is drawn and the first is not.
        let drew = |scroll: f32, label: &str| {
            let mut c = Canvas::new(&FakeText, None);
            draw(&mut c, &many, 1.0, (1440.0, 900.0), scroll);
            c.finish()
                .0
                .ops
                .iter()
                .any(|o| matches!(o, Op::Text { text, .. } if text == label))
        };
        let (first, last) = (r"C:\ProgramData\obs0", r"C:\ProgramData\obs39");
        assert!(drew(0.0, first) && !drew(0.0, last));
        assert!(!drew(m_many.max_scroll(), first) && drew(m_many.max_scroll(), last));
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
            uninstall: Some("u".into()),
            ..Default::default()
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

    #[test]
    fn the_tasks_have_their_own_group_between_the_files_and_the_registry() {
        let make = |kind: TraceKind, location: &str| Trace {
            kind,
            location: location.into(),
            ..Default::default()
        };
        let m = Model {
            traces_open: true,
            traces: Some(CleanReport {
                app: "OBS Studio".into(),
                preview: true,
                proposed: vec![
                    make(TraceKind::RegistryKey, r"HKCU\Software\obs"),
                    make(TraceKind::Task, r"\OBS\Updater"),
                    make(TraceKind::Folder, r"C:\ProgramData\obs"),
                ],
                ..Default::default()
            }),
            ..Model::default()
        };
        let rows = view(&m).unwrap().rows;
        let headings: Vec<&str> = rows.iter().filter_map(|r| r.heading.as_deref()).collect();
        assert_eq!(headings.len(), 3);
        assert!(headings[0].starts_with("Archivos y carpetas (1)"));
        assert_eq!(headings[1], "Tareas programadas (1)");
        assert_eq!(headings[2], "Registro (1)");
        assert_eq!(rows[1].label, r"\OBS\Updater");
    }

    #[test]
    fn the_window_offers_the_other_mode_and_says_when_it_is_the_safe_one() {
        use purifex_domain::scan_mode::ScanMode;
        let mut m = model_with_traces(2);
        let (_, actions) = render(&view(&m).unwrap(), 1.0);
        assert!(actions.contains(&Action::SetScanMode(ScanMode::Safe)));
        assert_eq!(view(&m).unwrap().corner[0].label, "Modo: Moderado");
        m.scan_mode = ScanMode::Safe;
        let v = view(&m).unwrap();
        assert!(v.body.contains("Modo Seguro"));
        let (_, actions) = render(&v, 1.0);
        assert!(actions.contains(&Action::SetScanMode(ScanMode::Moderate)));
    }

    #[test]
    fn an_app_still_installed_offers_to_uninstall_it_and_not_to_clean_it() {
        let mut m = model_with_traces(2);
        m.traces.as_mut().unwrap().still_installed = true;
        m.apps = Some(vec![InstalledApp {
            id: "HKLM64:OBS Studio".into(),
            name: "OBS Studio".into(),
            ..Default::default()
        }]);
        let v = view(&m).unwrap();
        let (ops, actions) = render(&v, 1.0);
        assert!(!actions
            .iter()
            .any(|a| matches!(a, Action::AskClean(_) | Action::AskCleanAll(_))));
        assert!(actions.contains(&Action::AskUninstall("HKLM64:OBS Studio".into())));
        assert!(actions.contains(&Action::CloseTraces));
        let said = ops
            .iter()
            .any(|o| matches!(o, Op::Text { text, .. } if text.contains("sigue")));
        assert!(said, "it explains why the traces are not removed yet");
    }
}
