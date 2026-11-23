//! One row of the list of apps: its name, how it is told apart (publisher, version, type),
//! when it was installed, how big it is, and the tag that uninstalls it. In the compact list
//! only the name, the size and the tag are drawn.

use purifex_domain::app_list::AppView;
use purifex_domain::apps::{size_text, InstalledApp};
use purifex_domain::draw::Rect;
use purifex_domain::view::Action;

use crate::canvas::Canvas;
use crate::theme::{self, Tone};
use crate::widgets::{checkbox, icon_box, tag};

/// The room the check at the left of a row takes.
pub(crate) const CHECK_W: f32 = 32.0;

/// The height of a row, by how the list is shown.
pub(crate) fn row_height(view: AppView) -> f32 {
    match view {
        AppView::Details => 60.0,
        AppView::List => 44.0,
    }
}

pub(crate) fn app_row(c: &mut Canvas, app: &InstalledApp, bar: Rect, view: AppView, marked: bool) {
    let hover = c.hover(bar);
    if hover > 0.01 {
        c.fill(bar, 0.0, theme::TEXT.with_alpha(0.02 * hover));
    }
    // The whole row opens its details; the tag of the action, drawn after, wins where it is.
    c.hit(bar, Action::SelectApp(app.id.clone()));
    c.line(
        bar.x,
        bar.bottom() - 0.5,
        bar.right(),
        bar.bottom() - 0.5,
        theme::LINE_SOFT,
    );
    let mid = bar.h / 2.0;
    let compact = view == AppView::List;
    let icon = if compact { 28.0 } else { 40.0 };
    mark_box(c, app, bar, marked);
    icon_box(
        c,
        bar.x + 16.0 + CHECK_W,
        bar.y + mid - icon / 2.0,
        icon,
        "package",
        Tone::Gray,
    );
    app_labels(c, app, bar, compact);
    if !compact {
        let installed = app.install_date.map_or("–".to_string(), |d| d.to_string());
        let style = theme::mono(12.0, theme::TEXT_2);
        c.text(bar.right() - 450.0, bar.y + mid - 9.0, &installed, style);
    }
    let size = size_text(app.size_kb);
    c.text(
        bar.right() - 330.0,
        bar.y + mid - 9.0,
        &size,
        theme::mono(12.0, theme::TEXT_2),
    );
    if app.protection().is_some() {
        let shield = Some("shield-check");
        tag(
            c,
            bar.right() - 220.0,
            bar.y + mid - 11.0,
            "No tocar",
            Tone::Gray,
            shield,
        );
    } else {
        uninstall_tag(c, app, bar);
    }
}

/// The check that marks an app for the batch; an app that may not be uninstalled has none.
fn mark_box(c: &mut Canvas, app: &InstalledApp, bar: Rect, marked: bool) {
    if app.protection().is_some() {
        return;
    }
    let spot = Rect::new(bar.x + 8.0, bar.y + bar.h / 2.0 - 14.0, 28.0, 28.0);
    checkbox(c, spot.x + 4.0, spot.y + 4.0, marked);
    c.hit(spot, Action::ToggleMark(app.id.clone()));
}

/// The name of the app and, below it, its publisher and version.
fn app_labels(c: &mut Canvas, app: &InstalledApp, bar: Rect, compact: bool) {
    let room = bar.w - CHECK_W - if compact { 480.0 } else { 600.0 };
    let top = if compact { bar.h / 2.0 - 9.0 } else { 10.0 };
    c.text_cut(
        bar.x + 70.0 + CHECK_W,
        bar.y + top,
        &app.name,
        theme::sans(13.5, theme::TEXT).bold(),
        room,
    );
    if compact {
        return;
    }
    let detail = [
        app.publisher.as_deref().unwrap_or("editor desconocido"),
        app.version.as_deref().unwrap_or(""),
        app.type_label(),
    ]
    .into_iter()
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join("  ");
    c.text_cut(
        bar.x + 70.0 + CHECK_W,
        bar.y + 32.0,
        detail.trim(),
        theme::mono(11.5, theme::TEXT_3),
        room,
    );
}

/// The red «Desinstalar» tag of an app that may be uninstalled: outlined while the
/// pointer is over it, and a place to click.
fn uninstall_tag(c: &mut Canvas, app: &InstalledApp, bar: Rect) {
    let rect = Rect::new(bar.right() - 220.0, bar.y + bar.h / 2.0 - 11.0, 120.0, 22.0);
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
