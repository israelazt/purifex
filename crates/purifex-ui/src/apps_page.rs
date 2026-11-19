//! The apps page: the installed apps, each with a button that starts its own
//! uninstaller, and, once one was uninstalled from here, a panel to look for what it
//! left behind and remove it to the quarantine. What may be removed is decided in
//! the domain (`app_traces`); this only shows it and asks for confirmation.

use purifex_domain::app_traces::Trace;
use purifex_domain::apps::{size_text, InstalledApp};
use purifex_domain::draw::Rect;
use purifex_domain::view::{Action, Confirm, Model};

use crate::canvas::Canvas;
use crate::fixes_page::{confirm_height, confirmation};
use crate::page;
use crate::theme::{self, Tone};
use crate::widgets::{button, button_width, icon_box, panel, tag};

const ROW_H: f32 = 60.0;
const MAX_TRACES: usize = 10;

const CLEAN_QUESTION: &str = "Los restos se mueven a una cuarentena (las claves del registro se guardan antes) y quedan en el historial, donde puedes devolverlos. Incluye datos tuyos de la app, como ajustes y partidas guardadas. ¿Quitar?";

fn info(c: &mut Canvas, area: Rect, text: &str) -> f32 {
    let style = theme::mono(12.5, theme::TEXT);
    let lines = c.wrap(text, &style, area.w - 56.0, Some(3));
    let h = 28.0 + lines.len() as f32 * 18.0;
    let bar = Rect::new(area.x, area.y, area.w, h);
    c.fill(bar, 8.0, Tone::Blue.fill());
    c.stroke(bar, 8.0, 1.0, Tone::Blue.border());
    c.icon(
        "circle-check",
        bar.x + 16.0,
        bar.y + 14.0,
        16.0,
        theme::BLUE,
    );
    for (i, line) in lines.iter().enumerate() {
        c.text(bar.x + 42.0, bar.y + 14.0 + i as f32 * 18.0, line, style);
    }
    h + 16.0
}

fn trace_line(c: &mut Canvas, t: &Trace, x: f32, y: f32, width: f32) {
    let style = theme::mono(12.0, theme::TEXT);
    let size = size_text(Some(t.bytes / 1024));
    c.text_cut(x, y, &t.location, style, width - 220.0);
    c.text(
        x + width - 200.0,
        y,
        &size,
        theme::mono(12.0, theme::TEXT_2),
    );
    if t.user_data {
        tag(
            c,
            x + width - 120.0,
            y - 3.0,
            "datos tuyos",
            Tone::Amber,
            None,
        );
    }
    c.text_cut(
        x,
        y + 17.0,
        &t.why,
        theme::mono(11.0, theme::TEXT_3),
        width - 40.0,
    );
}

/// The panel to look for the traces of the app uninstalled from here.
fn cleanup_panel(c: &mut Canvas, m: &Model, name: &str, area: Rect, y: f32) -> f32 {
    let traces = m.traces.as_ref().filter(|r| r.app == name);
    let shown = traces.map_or(0, |r| r.proposed.len().min(MAX_TRACES));
    let body = match traces {
        None => 0.0,
        Some(r) if r.proposed.is_empty() => 30.0,
        Some(r) => {
            12.0 + shown as f32 * 44.0 + if r.proposed.len() > shown { 22.0 } else { 0.0 } + 68.0
        }
    };
    let rect = Rect::new(area.x, y, area.w, 76.0 + body);
    panel(c, rect);
    c.icon("trash-2", rect.x + 18.0, rect.y + 18.0, 18.0, theme::AMBER);
    c.text(
        rect.x + 46.0,
        rect.y + 16.0,
        &format!("Restos de {name}"),
        theme::sans(14.0, theme::TEXT).bold(),
    );
    c.paragraph(
        rect.x + 46.0,
        rect.y + 40.0,
        rect.w - 360.0,
        "Cuando el desinstalador termine, busca lo que dejó. No se borra nada al buscar.",
        theme::mono(12.0, theme::TEXT_2),
        Some(2),
    );
    let find = if traces.is_some() {
        "Buscar otra vez"
    } else {
        "Buscar restos"
    };
    let w = button_width(c, "Buscar otra vez");
    let busy = m.finding.is_some();
    button(
        c,
        Rect::new(rect.right() - 16.0 - w, rect.y + 16.0, w, 44.0),
        find,
        "search",
        false,
        (!busy).then(|| Action::FindTraces(name.to_string())),
    );
    if let Some(r) = traces {
        let top = rect.y + 76.0;
        if r.proposed.is_empty() {
            c.text(
                rect.x + 18.0,
                top + 4.0,
                "No se encontraron restos.",
                theme::mono(12.5, theme::TEXT_2),
            );
        } else {
            c.line(rect.x, top, rect.right(), top, theme::LINE);
            for (i, t) in r.proposed.iter().take(MAX_TRACES).enumerate() {
                trace_line(
                    c,
                    t,
                    rect.x + 18.0,
                    top + 16.0 + i as f32 * 44.0,
                    rect.w - 36.0,
                );
            }
            let mut below = top + 16.0 + shown as f32 * 44.0;
            if r.proposed.len() > shown {
                let more = format!("y {} más", r.proposed.len() - shown);
                c.text(
                    rect.x + 18.0,
                    below - 8.0,
                    &more,
                    theme::mono(12.0, theme::TEXT_3),
                );
                below += 22.0;
            }
            let remove = "Quitar restos";
            let rw = button_width(c, remove);
            let asked = matches!(m.confirm, Some(Confirm::Clean(_)));
            button(
                c,
                Rect::new(rect.right() - 16.0 - rw, below - 4.0, rw, 44.0),
                remove,
                "trash-2",
                false,
                (!asked).then(|| Action::AskClean(name.to_string())),
            );
        }
    }
    rect.h + 16.0
}

/// The row of the app being asked about: the question and its two buttons in place,
/// so they are where the user is looking (the list is long and scrolled).
fn asking_row(c: &mut Canvas, app: &InstalledApp, bar: Rect) {
    c.fill(bar, 0.0, Tone::Amber.fill());
    c.line(
        bar.x,
        bar.bottom() - 0.5,
        bar.right(),
        bar.bottom() - 0.5,
        theme::LINE_SOFT,
    );
    icon_box(
        c,
        bar.x + 16.0,
        bar.y + 10.0,
        40.0,
        "triangle-alert",
        Tone::Amber,
    );
    let name = theme::sans(13.5, theme::TEXT).bold();
    c.text_cut(bar.x + 70.0, bar.y + 10.0, &app.name, name, bar.w - 520.0);
    c.text_cut(
        bar.x + 70.0,
        bar.y + 32.0,
        "Se abrirá su desinstalador oficial; lo terminas tú en su ventana.",
        theme::mono(11.5, theme::AMBER),
        bar.w - 520.0,
    );
    let (yes, no) = ("Desinstalar", "Cancelar");
    let (yes_w, no_w) = (button_width(c, yes), button_width(c, no));
    let y = bar.y + 10.0;
    let yes_x = bar.right() - 16.0 - no_w - 12.0 - yes_w;
    button(
        c,
        Rect::new(yes_x, y, yes_w, 40.0),
        yes,
        "trash-2",
        true,
        Some(Action::Confirm),
    );
    let no_x = bar.right() - 16.0 - no_w;
    button(
        c,
        Rect::new(no_x, y, no_w, 40.0),
        no,
        "x",
        false,
        Some(Action::CancelConfirm),
    );
}

fn app_row(c: &mut Canvas, app: &InstalledApp, bar: Rect) {
    let hover = c.hover(bar);
    if hover > 0.01 {
        c.fill(bar, 0.0, theme::TEXT.with_alpha(0.02 * hover));
    }
    c.line(
        bar.x,
        bar.bottom() - 0.5,
        bar.right(),
        bar.bottom() - 0.5,
        theme::LINE_SOFT,
    );
    icon_box(c, bar.x + 16.0, bar.y + 10.0, 40.0, "package", Tone::Gray);
    c.text_cut(
        bar.x + 70.0,
        bar.y + 10.0,
        &app.name,
        theme::sans(13.5, theme::TEXT).bold(),
        bar.w - 470.0,
    );
    let detail = format!(
        "{}  {}",
        app.publisher.as_deref().unwrap_or("editor desconocido"),
        app.version.as_deref().unwrap_or("")
    );
    c.text_cut(
        bar.x + 70.0,
        bar.y + 32.0,
        detail.trim(),
        theme::mono(11.5, theme::TEXT_3),
        bar.w - 470.0,
    );
    c.text(
        bar.right() - 330.0,
        bar.y + 21.0,
        &size_text(app.size_kb),
        theme::mono(12.0, theme::TEXT_2),
    );
    match app.protection() {
        Some(_) => {
            tag(
                c,
                bar.right() - 220.0,
                bar.y + 19.0,
                "No tocar",
                Tone::Gray,
                Some("shield-check"),
            );
        }
        None => {
            let rect = Rect::new(bar.right() - 220.0, bar.y + 19.0, 120.0, 22.0);
            let h = c.hover(rect);
            let width = tag(c, rect.x, rect.y, "Desinstalar", Tone::Red, Some("trash-2"));
            if h > 0.01 {
                c.stroke(
                    Rect::new(rect.x, rect.y, width, 22.0),
                    4.0,
                    1.0,
                    Tone::Red.color().with_alpha(0.9 * h),
                );
            }
            c.hit(
                Rect::new(rect.x, rect.y, width, 22.0),
                Action::AskUninstall(app.id.clone()),
            );
        }
    }
}

fn table(c: &mut Canvas, m: &Model, area: Rect, y: f32) -> f32 {
    let apps = m.visible_apps();
    let height = 48.0 + apps.len().max(1) as f32 * ROW_H;
    let rect = Rect::new(area.x, y, area.w, height);
    panel(c, rect);
    let head = Rect::new(rect.x, rect.y, rect.w, 48.0);
    let label = theme::eyebrow(theme::TEXT_3);
    c.text_in(rect.x + 16.0, head, "APP Y EDITOR", label);
    c.text_in(rect.right() - 330.0, head, "TAMAÑO", label);
    c.text_in(rect.right() - 220.0, head, "ACCIÓN", label);
    c.line(
        rect.x,
        rect.y + 48.0,
        rect.right(),
        rect.y + 48.0,
        theme::LINE,
    );
    for (i, app) in apps.iter().enumerate() {
        c.reveal(if i < 12 { 18 * i as u64 } else { 0 });
        let bar = Rect::new(rect.x, rect.y + 48.0 + i as f32 * ROW_H, rect.w, ROW_H);
        if m.confirm == Some(Confirm::Uninstall(app.id.clone())) {
            asking_row(c, app, bar);
        } else {
            app_row(c, app, bar);
        }
    }
    c.reveal(0);
    if apps.is_empty() {
        c.text(
            rect.x + 16.0,
            rect.y + 70.0,
            "Ninguna app coincide.",
            theme::mono(12.5, theme::TEXT_3),
        );
    }
    height
}

/// The question at the top of the page: the one about removing traces (its button is
/// up there too). The one about uninstalling is in the row of the app itself.
fn question(m: &Model) -> Option<&'static str> {
    matches!(m.confirm, Some(Confirm::Clean(_))).then_some(CLEAN_QUESTION)
}

/// Draws the page in `area` and returns the height it used.
pub fn draw(c: &mut Canvas, m: &Model, area: Rect) -> f32 {
    let mut y = area.y;
    y += page::head(
        c,
        m,
        area,
        "Desinstalador",
        "lo que cada app registró en Windows",
        "Apps",
        "Desinstala con el desinstalador oficial de cada app y después busca y quita lo que deja. Nada se borra sin que lo confirmes, y lo quitado va a una cuarentena de donde se puede devolver.",
    );
    if let Some(error) = &m.error {
        y += page::error_box(c, Rect::new(area.x, y, area.w, 0.0), error);
    }
    if m.apps.is_none() {
        if m.error.is_none() {
            y += page::reading(
                c,
                Rect::new(area.x, y, area.w, 60.0),
                "Leyendo las apps instaladas…",
            );
        }
        return y - area.y;
    }
    if let Some(text) = &m.message {
        y += info(c, Rect::new(area.x, y, area.w, 0.0), text);
    }
    if let Some(q) = question(m) {
        let h = confirm_height(c, q, area.w);
        confirmation(c, q, Rect::new(area.x, y, area.w, h));
        y += h + 16.0;
    }
    if let Some(name) = &m.uninstalled {
        y += cleanup_panel(c, m, name, area, y);
    }
    y += table(c, m, area, y);
    y - area.y
}

#[cfg(test)]
mod tests {
    use purifex_domain::app_traces::TraceKind;
    use purifex_domain::apps::AppKind;
    use purifex_domain::cleanup_log::CleanReport;
    use purifex_domain::draw::Op;

    use super::*;
    use crate::canvas::testing::FakeText;

    fn app(name: &str, publisher: &str) -> InstalledApp {
        InstalledApp {
            id: format!("HKLM64:{name}"),
            name: name.into(),
            publisher: Some(publisher.into()),
            version: Some("1.0".into()),
            install_location: None,
            uninstall: Some("u.exe".into()),
            quiet_uninstall: None,
            size_kb: Some(2048),
            kind: AppKind::Desktop,
            hidden: false,
        }
    }

    fn model() -> Model {
        Model {
            apps: Some(vec![
                app("Discord", "Discord Inc."),
                app("Microsoft .NET Runtime 8", "Microsoft Corporation"),
            ]),
            ..Model::default()
        }
    }

    fn render(m: &Model) -> (Vec<String>, Vec<Action>) {
        let mut c = Canvas::new(&FakeText, None);
        draw(&mut c, m, Rect::new(0.0, 0.0, 1100.0, 0.0));
        let (list, hits) = c.finish();
        let texts = list.ops.iter().filter_map(|op| match op {
            Op::Text { text, .. } => Some(text.clone()),
            _ => None,
        });
        (
            texts.collect(),
            hits.into_iter().map(|h| h.action).collect(),
        )
    }

    fn trace(location: &str, user_data: bool) -> Trace {
        Trace {
            kind: TraceKind::Folder,
            location: location.into(),
            bytes: 4096,
            why: "lleva el nombre de la app".into(),
            user_data,
        }
    }

    #[test]
    fn each_app_can_be_uninstalled_except_the_protected_ones_which_say_so() {
        let (texts, actions) = render(&model());
        assert!(
            texts.contains(&"Desinstalar".to_string()) && texts.contains(&"No tocar".to_string())
        );
        assert!(actions.contains(&Action::AskUninstall("HKLM64:Discord".into())));
        assert!(!actions
            .iter()
            .any(|a| matches!(a, Action::AskUninstall(id) if id.contains(".NET"))));
    }

    #[test]
    fn asking_shows_the_question_with_confirm_and_cancel_and_the_page_says_it_is_reading_before_the_data(
    ) {
        let mut m = model();
        m.confirm = Some(Confirm::Uninstall("HKLM64:Discord".into()));
        let (texts, actions) = render(&m);
        assert!(texts.join(" ").contains("desinstalador oficial"));
        assert!(actions.contains(&Action::Confirm) && actions.contains(&Action::CancelConfirm));
        let (waiting, _) = render(&Model::default());
        assert!(waiting.iter().any(|t| t.contains("Leyendo las apps")));
    }

    #[test]
    fn after_an_uninstall_the_traces_can_be_looked_for_and_then_removed_with_a_confirmation() {
        let mut m = model();
        m.uninstalled = Some("Discord".into());
        let (texts, actions) = render(&m);
        assert!(texts.contains(&"Restos de Discord".to_string()));
        assert!(actions.contains(&Action::FindTraces("Discord".into())));
        assert!(
            !actions.contains(&Action::AskClean("Discord".into())),
            "nothing to remove before looking"
        );
        m.traces = Some(CleanReport {
            app: "Discord".into(),
            preview: true,
            removed: vec![],
            proposed: vec![
                trace(r"C:\Users\u\AppData\Roaming\Discord", true),
                trace(r"HKCU\Software\Discord", false),
            ],
            failed: vec![],
        });
        let (texts, actions) = render(&m);
        assert!(texts.contains(&r"C:\Users\u\AppData\Roaming\Discord".to_string()));
        assert!(
            texts.contains(&"datos tuyos".to_string()),
            "the user's data is marked"
        );
        assert!(actions.contains(&Action::AskClean("Discord".into())));
        m.confirm = Some(Confirm::Clean("Discord".into()));
        let (texts, actions) = render(&m);
        assert!(texts.join(" ").contains("cuarentena"));
        assert!(
            !actions.contains(&Action::AskClean("Discord".into())),
            "already asking"
        );
    }

    #[test]
    fn no_traces_says_so_and_a_message_of_the_last_action_is_shown() {
        let mut m = model();
        m.uninstalled = Some("Discord".into());
        m.message = Some("Se quitaron 2 restos de Discord".into());
        m.traces = Some(CleanReport {
            app: "Discord".into(),
            preview: true,
            removed: vec![],
            proposed: vec![],
            failed: vec![],
        });
        let (texts, _) = render(&m);
        assert!(texts.contains(&"No se encontraron restos.".to_string()));
        assert!(texts.join(" ").contains("Se quitaron 2 restos"));
    }
}
