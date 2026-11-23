//! The Cleaning page: the junk Windows leaves around, by kind, with what each holds. The checked
//! kinds move to the quarantine (not deleted), where they can be put back.

use purifex_domain::apps::size_text;
use purifex_domain::draw::Rect;
use purifex_domain::junk::{items_text, rows, JunkRow};
use purifex_domain::view::{Action, Confirm, Model};

use crate::apps_page::info;
use crate::canvas::Canvas;
use crate::dialog::{confirm_view, DialogView};
use crate::page;
use crate::theme::{self, Tone};
use crate::widgets::{button, button_width, checkbox, icon_box, panel, tag};

const ROW_H: f32 = 64.0;

fn size_of(bytes: u64) -> String {
    size_text(Some(bytes.div_ceil(1024)))
}

pub fn draw(c: &mut Canvas, m: &Model, area: Rect) -> f32 {
    let mut y = area.y;
    y += page::head(
        c,
        m,
        area,
        "Limpieza",
        "lo que Windows deja por ahí",
        "Limpieza",
        "Archivos que se vuelven a crear solos. Lo que marques no se borra: va a la cuarentena, de donde se puede devolver, y el espacio se libera de verdad cuando la vacíes.",
    );
    if let Some(error) = &m.error {
        y += page::error_box(c, Rect::new(area.x, y, area.w, 0.0), error);
    }
    let Some(items) = &m.junk else {
        if m.error.is_none() {
            y += page::reading(
                c,
                Rect::new(area.x, y, area.w, 60.0),
                "Buscando lo que se puede limpiar…",
            );
        }
        return y - area.y;
    };
    if let Some(text) = &m.message {
        y += info(c, Rect::new(area.x, y, area.w, 0.0), text);
    }
    let table_rows = rows(items);
    y += table(c, m, &table_rows, Rect::new(area.x, y, area.w, 0.0));
    y - area.y
}

fn table(c: &mut Canvas, m: &Model, table_rows: &[JunkRow], area: Rect) -> f32 {
    let height = 48.0 + table_rows.len() as f32 * ROW_H + 76.0;
    let rect = Rect::new(area.x, area.y, area.w, height);
    panel(c, rect);
    let head = Rect::new(rect.x, rect.y, rect.w, 48.0);
    c.text_in(
        rect.x + 16.0,
        head,
        "QUÉ SE PUEDE LIMPIAR",
        theme::eyebrow(theme::TEXT_3),
    );
    c.line(
        rect.x,
        rect.y + 48.0,
        rect.right(),
        rect.y + 48.0,
        theme::LINE,
    );
    for (i, row) in table_rows.iter().enumerate() {
        let bar = Rect::new(rect.x, rect.y + 48.0 + i as f32 * ROW_H, rect.w, ROW_H);
        kind_row(c, m, row, bar);
    }
    footer(
        c,
        m,
        table_rows,
        Rect::new(rect.x, rect.bottom() - 76.0, rect.w, 76.0),
    );
    height + 16.0
}

fn kind_row(c: &mut Canvas, m: &Model, row: &JunkRow, bar: Rect) {
    let mid = bar.y + bar.h / 2.0;
    c.line(
        bar.x,
        bar.bottom() - 0.5,
        bar.right(),
        bar.bottom() - 0.5,
        theme::LINE_SOFT,
    );
    let usable = row.count > 0;
    if usable {
        let checked = !m.junk_unchecked.contains(&row.kind);
        checkbox(c, bar.x + 16.0, mid - 10.0, checked);
        c.hit(bar, Action::ToggleJunk(row.kind));
    }
    let tone = if usable { Tone::Green } else { Tone::Gray };
    icon_box(c, bar.x + 52.0, mid - 20.0, 40.0, "trash-2", tone);
    kind_texts(c, row, bar);
    if row.kind.needs_admin() && usable {
        tag(
            c,
            bar.right() - 124.0,
            mid - 11.0,
            "administrador",
            Tone::Amber,
            None,
        );
    }
}

/// The name, what it is, how many and how big.
fn kind_texts(c: &mut Canvas, row: &JunkRow, bar: Rect) {
    let mid = bar.y + bar.h / 2.0;
    let ink = if row.count > 0 {
        theme::TEXT
    } else {
        theme::TEXT_3
    };
    let room = bar.w - 480.0;
    c.text_cut(
        bar.x + 106.0,
        bar.y + 11.0,
        row.kind.label(),
        theme::sans(13.5, ink).bold(),
        room,
    );
    c.text_cut(
        bar.x + 106.0,
        bar.y + 34.0,
        row.kind.detail(),
        theme::mono(11.0, theme::TEXT_3),
        room,
    );
    let count = if row.count > 0 {
        items_text(row.count)
    } else {
        "nada".to_string()
    };
    c.text(
        bar.right() - 330.0,
        mid - 9.0,
        &count,
        theme::mono(12.0, theme::TEXT_2),
    );
    c.text(
        bar.right() - 200.0,
        mid - 9.0,
        &size_of(row.bytes),
        theme::mono(12.0, ink),
    );
}

fn footer(c: &mut Canvas, m: &Model, table_rows: &[JunkRow], bar: Rect) {
    c.line(bar.x, bar.y, bar.right(), bar.y, theme::LINE);
    let picked: Vec<&JunkRow> = table_rows
        .iter()
        .filter(|r| r.count > 0 && !m.junk_unchecked.contains(&r.kind))
        .collect();
    let (count, bytes) = (
        picked.iter().map(|r| r.count).sum::<usize>(),
        picked.iter().map(|r| r.bytes).sum::<u64>(),
    );
    let text = format!("Marcado: {} · {}", items_text(count), size_of(bytes));
    c.text(
        bar.x + 16.0,
        bar.y + 28.0,
        &text,
        theme::mono(12.5, theme::TEXT),
    );
    let label = format!("Limpiar {}", size_of(bytes));
    let w = button_width(c, &label);
    let rect = Rect::new(bar.right() - 16.0 - w, bar.y + 16.0, w, 44.0);
    button(
        c,
        rect,
        &label,
        "trash-2",
        true,
        (count > 0).then_some(Action::AskCleanJunk),
    );
}

/// The question before the junk is moved.
pub(crate) fn dialog(m: &Model) -> Option<DialogView> {
    (m.confirm == Some(Confirm::CleanJunk)).then(|| {
        confirm_view(
            "¿Limpiar lo marcado?".to_string(),
            "Se mueve a la cuarentena (no se borra) y se puede devolver con «purifex apps restore». Lo que un programa tiene abierto no se toca. El espacio se libera cuando vacíes la cuarentena.",
            "Limpiar",
            "trash-2",
        )
    })
}

#[cfg(test)]
mod tests {
    use purifex_domain::draw::Op;
    use purifex_domain::junk::{JunkItem, JunkKind};

    use super::*;
    use crate::canvas::testing::FakeText;

    fn item(kind: JunkKind, bytes: u64) -> JunkItem {
        JunkItem {
            kind,
            location: r"C:\T\a".into(),
            bytes,
            folder: false,
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

    #[test]
    fn only_kinds_with_something_can_be_checked_and_the_button_says_how_much_it_cleans() {
        let m = Model {
            junk: Some(vec![
                item(JunkKind::Temp, 3 * 1024 * 1024),
                item(JunkKind::UpdateCache, 1024 * 1024),
            ]),
            ..Model::default()
        };
        let (texts, actions) = render(&m);
        assert!(actions.contains(&Action::ToggleJunk(JunkKind::Temp)));
        assert!(
            !actions.contains(&Action::ToggleJunk(JunkKind::Thumbnails)),
            "nothing to check"
        );
        assert!(actions.contains(&Action::AskCleanJunk));
        assert!(
            texts.contains(&"Marcado: 2 elementos · 4 MB".to_string()),
            "{texts:?}"
        );
        assert!(texts.contains(&"administrador".to_string()));
        let off = Model {
            junk_unchecked: vec![JunkKind::Temp, JunkKind::UpdateCache],
            ..m
        };
        let (_, actions) = render(&off);
        assert!(
            !actions.contains(&Action::AskCleanJunk),
            "nothing checked: no button action"
        );
    }

    #[test]
    fn the_page_says_when_it_is_looking_and_the_question_only_shows_when_asked() {
        let (texts, _) = render(&Model::default());
        assert!(texts
            .iter()
            .any(|t| t.contains("Buscando lo que se puede limpiar")));
        assert!(dialog(&Model::default()).is_none());
        let asking = Model {
            confirm: Some(Confirm::CleanJunk),
            ..Model::default()
        };
        assert!(dialog(&asking).unwrap().body.contains("cuarentena"));
    }
}
