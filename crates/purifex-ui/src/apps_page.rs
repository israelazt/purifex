//! The apps page: the installed apps, each with a button that asks to uninstall it,
//! a search, and a panel for the traces of an app that was uninstalled. The questions
//! and the list of traces are dialogs (`dialog`); what may be removed is decided in
//! the domain (`app_traces`). This only shows and asks.

use purifex_domain::apps::{size_text, InstalledApp};
use purifex_domain::draw::Rect;
use purifex_domain::view::{Action, Model};

use crate::canvas::Canvas;
use crate::drivers_page::search_box;
use crate::page;
use crate::theme::{self, Tone};
use crate::widgets::{button, button_width, icon_box, panel, tag};

const ROW_H: f32 = 60.0;

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

/// The panel of the app uninstalled from here (or seen to disappear): look for what it
/// left, and open the window with the list.
fn cleanup_panel(c: &mut Canvas, m: &Model, name: &str, area: Rect, y: f32) -> f32 {
    let report = m.traces.as_ref().filter(|r| r.app == name);
    let rect = Rect::new(area.x, y, area.w, 84.0);
    panel(c, rect);
    c.icon("trash-2", rect.x + 18.0, rect.y + 18.0, 18.0, theme::AMBER);
    c.text(
        rect.x + 46.0,
        rect.y + 16.0,
        &format!("Restos de {name}"),
        theme::sans(14.0, theme::TEXT).bold(),
    );
    let summary = match report {
        None if m.finding.is_some() => "Buscando lo que dejó…".to_string(),
        None => "Cuando el desinstalador termine, Purifex busca lo que dejó. No se borra nada al buscar.".to_string(),
        Some(r) if r.proposed.is_empty() => "No se encontraron restos.".to_string(),
        Some(r) => {
            let bytes: u64 = r.proposed.iter().map(|t| t.bytes).sum();
            format!("Se encontraron {} restos ({}).", r.proposed.len(), size_text(Some(bytes / 1024)))
        }
    };
    c.paragraph(
        rect.x + 46.0,
        rect.y + 40.0,
        rect.w - 520.0,
        &summary,
        theme::mono(12.0, theme::TEXT_2),
        Some(2),
    );
    let find = if report.is_some() {
        "Buscar otra vez"
    } else {
        "Buscar restos"
    };
    let (find_w, view_w) = (
        button_width(c, "Buscar otra vez"),
        button_width(c, "Ver restos"),
    );
    let busy = m.finding.is_some();
    let x = rect.right() - 16.0 - find_w;
    button(
        c,
        Rect::new(x, rect.y + 20.0, find_w, 44.0),
        find,
        "search",
        false,
        (!busy).then(|| Action::FindTraces(name.to_string())),
    );
    if report.is_some_and(|r| !r.proposed.is_empty()) {
        let view = Rect::new(x - 12.0 - view_w, rect.y + 20.0, view_w, 44.0);
        button(
            c,
            view,
            "Ver restos",
            "list-checks",
            true,
            Some(Action::OpenTraces),
        );
    }
    rect.h + 16.0
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
    app_labels(c, app, bar);
    c.text(
        bar.right() - 330.0,
        bar.y + 21.0,
        &size_text(app.size_kb),
        theme::mono(12.0, theme::TEXT_2),
    );
    if app.protection().is_some() {
        let shield = Some("shield-check");
        tag(
            c,
            bar.right() - 220.0,
            bar.y + 19.0,
            "No tocar",
            Tone::Gray,
            shield,
        );
    } else {
        uninstall_tag(c, app, bar);
    }
}

/// The name of the app and, below it, its publisher and version.
fn app_labels(c: &mut Canvas, app: &InstalledApp, bar: Rect) {
    let room = bar.w - 470.0;
    c.text_cut(
        bar.x + 70.0,
        bar.y + 10.0,
        &app.name,
        theme::sans(13.5, theme::TEXT).bold(),
        room,
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
        room,
    );
}

/// The red «Desinstalar» tag of an app that may be uninstalled: outlined while the
/// pointer is over it, and a place to click.
fn uninstall_tag(c: &mut Canvas, app: &InstalledApp, bar: Rect) {
    let rect = Rect::new(bar.right() - 220.0, bar.y + 19.0, 120.0, 22.0);
    let hover = c.hover(rect);
    let width = tag(c, rect.x, rect.y, "Desinstalar", Tone::Red, Some("trash-2"));
    let tag_rect = Rect::new(rect.x, rect.y, width, 22.0);
    if hover > 0.01 {
        c.stroke(
            tag_rect,
            4.0,
            1.0,
            Tone::Red.color().with_alpha(0.9 * hover),
        );
    }
    c.hit(tag_rect, Action::AskUninstall(app.id.clone()));
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
        app_row(
            c,
            app,
            Rect::new(rect.x, rect.y + 48.0 + i as f32 * ROW_H, rect.w, ROW_H),
        );
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

/// When what was typed matches no installed app, it may be one that is already gone
/// (uninstalled before, or somewhere else): its traces can still be looked for.
fn already_gone(c: &mut Canvas, m: &Model, area: Rect, y: f32) -> f32 {
    let name = m.query.trim();
    if name.chars().count() < 3
        || !m.visible_apps().is_empty()
        || m.uninstalled.as_deref() == Some(name)
    {
        return 0.0;
    }
    let rect = Rect::new(area.x, y, area.w, 76.0);
    panel(c, rect);
    c.icon("trash-2", rect.x + 18.0, rect.y + 18.0, 18.0, theme::AMBER);
    let title = format!("No hay ninguna app instalada que se llame «{name}»");
    c.text(
        rect.x + 46.0,
        rect.y + 16.0,
        &title,
        theme::sans(14.0, theme::TEXT).bold(),
    );
    c.paragraph(
        rect.x + 46.0,
        rect.y + 40.0,
        rect.w - 360.0,
        "¿Ya la desinstalaste? Puedes buscar lo que dejó (no se borra nada al buscar).",
        theme::mono(12.0, theme::TEXT_2),
        Some(2),
    );
    let label = "Buscar sus restos";
    let w = button_width(c, label);
    button(
        c,
        Rect::new(rect.right() - 16.0 - w, rect.y + 16.0, w, 44.0),
        label,
        "search",
        false,
        Some(Action::FindTraces(name.to_string())),
    );
    rect.h + 16.0
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
        "Desinstala con el desinstalador oficial de cada app y, cuando termine, Purifex te muestra lo que dejó para quitarlo todo de una vez o elegir. Lo quitado va a una cuarentena de donde se puede devolver.",
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
    if let Some(name) = &m.uninstalled {
        y += cleanup_panel(c, m, name, area, y);
    }
    y += search_box(
        c,
        m,
        area,
        y,
        "Filtrar por nombre o editor… (si ya la desinstalaste, escribe su nombre)",
    );
    y += already_gone(c, m, area, y);
    y += table(c, m, area, y);
    y - area.y
}

#[cfg(test)]
mod tests {
    use purifex_domain::app_traces::{Trace, TraceKind};

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
            uninstall: Some("u.exe".into()),
            size_kb: Some(2048),
            ..Default::default()
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

    fn report(n: usize) -> CleanReport {
        CleanReport {
            app: "Discord".into(),
            preview: true,
            removed: vec![],
            proposed: (0..n)
                .map(|i| Trace {
                    kind: TraceKind::Folder,
                    location: format!(r"C:\x\{i}"),
                    bytes: 4096,
                    why: "w".into(),
                    ..Default::default()
                })
                .collect(),
            failed: vec![],
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
    fn the_page_says_it_is_reading_before_the_data_and_does_not_draw_the_questions_itself() {
        let (waiting, _) = render(&Model::default());
        assert!(waiting.iter().any(|t| t.contains("Leyendo las apps")));
        let mut m = model();
        m.confirm = Some(purifex_domain::view::Confirm::Uninstall(
            "HKLM64:Discord".into(),
        ));
        let (texts, actions) = render(&m);
        assert!(
            !texts.join(" ").contains("desinstalador oficial de esta"),
            "the question is a dialog now"
        );
        assert!(
            !actions.contains(&Action::Confirm),
            "its buttons are the dialog's"
        );
    }

    #[test]
    fn the_panel_of_a_gone_app_looks_for_its_traces_and_opens_the_list_when_there_are_some() {
        let mut m = model();
        m.uninstalled = Some("Discord".into());
        let (texts, actions) = render(&m);
        assert!(texts.contains(&"Restos de Discord".to_string()));
        assert!(actions.contains(&Action::FindTraces("Discord".into())));
        assert!(
            !actions.contains(&Action::OpenTraces),
            "nothing to see before looking"
        );
        m.traces = Some(report(3));
        let (texts, actions) = render(&m);
        assert!(texts.join(" ").contains("Se encontraron 3 restos"));
        assert!(actions.contains(&Action::OpenTraces));
        m.traces = Some(report(0));
        let (texts, actions) = render(&m);
        assert!(texts.contains(&"No se encontraron restos.".to_string()));
        assert!(!actions.contains(&Action::OpenTraces));
    }

    #[test]
    fn what_matches_no_installed_app_may_be_one_already_gone_whose_traces_can_be_looked_for() {
        let mut m = model();
        m.query = "obs studio".into();
        let (texts, actions) = render(&m);
        assert!(texts
            .join(" ")
            .contains("No hay ninguna app instalada que se llame «obs studio»"));
        assert!(actions.contains(&Action::FindTraces("obs studio".into())));
        m.query = "discord".into();
        let (texts, actions) = render(&m);
        assert!(!texts.join(" ").contains("No hay ninguna app instalada"));
        assert!(!actions.iter().any(|a| matches!(a, Action::FindTraces(_))));
        m.query = "zz".into();
        let (_, actions) = render(&m);
        assert!(
            !actions.iter().any(|a| matches!(a, Action::FindTraces(_))),
            "too short to search safely"
        );
    }

    #[test]
    fn a_message_of_the_last_action_is_shown() {
        let mut m = model();
        m.message = Some("Se quitaron 2 restos de Discord".into());
        let (texts, _) = render(&m);
        assert!(texts.join(" ").contains("Se quitaron 2 restos"));
    }
}
