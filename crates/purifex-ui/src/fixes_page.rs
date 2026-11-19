//! The fixes page: what Purifex proposes (each with the measurement that justifies
//! it, what it changes, how it is undone and what stops it now), the notice of the
//! last change, and the history with its "undo". Nothing is changed by a click
//! on "Aplicar": it asks to confirm first, and only the confirmation acts.

use purifex_domain::draw::{Rect, TextStyle};
use purifex_domain::fix::Proposal;
use purifex_domain::view::{Action, Confirm, Model};

use crate::canvas::Canvas;
use crate::fix_history::history_panel;
use crate::page;
use crate::theme::{self, Tone};
use crate::widgets::{button, button_width, icon_box, panel, tag};

const PAD: f32 = 20.0;

/// The question and the two buttons that follow a click on "Aplicar" or "Deshacer".
pub(crate) fn confirmation(c: &mut Canvas, question: &str, bar: Rect) {
    c.fill(bar, 8.0, Tone::Amber.fill());
    c.stroke(bar, 8.0, 1.0, Tone::Amber.border());
    c.icon(
        "triangle-alert",
        bar.x + 16.0,
        bar.y + 16.0,
        16.0,
        theme::AMBER,
    );
    let (yes, no) = ("Confirmar", "Cancelar");
    let (yes_w, no_w) = (button_width(c, yes), button_width(c, no));
    let buttons_w = yes_w + no_w + 12.0;
    let text_w = bar.w - 16.0 - 24.0 - buttons_w - 32.0;
    let style = theme::mono(12.0, theme::TEXT);
    let lines = c.wrap(question, &style, text_w, Some(3));
    for (i, line) in lines.iter().enumerate() {
        c.text(bar.x + 42.0, bar.y + 16.0 + i as f32 * 18.0, line, style);
    }
    let y = bar.y + (bar.h - 40.0) / 2.0;
    button(
        c,
        Rect::new(bar.right() - 16.0 - buttons_w, y, yes_w, 40.0),
        yes,
        "circle-check",
        true,
        Some(Action::Confirm),
    );
    button(
        c,
        Rect::new(bar.right() - 16.0 - no_w, y, no_w, 40.0),
        no,
        "x",
        false,
        Some(Action::CancelConfirm),
    );
}

pub(crate) fn confirm_height(c: &Canvas, question: &str, width: f32) -> f32 {
    let (yes_w, no_w) = (button_width(c, "Confirmar"), button_width(c, "Cancelar"));
    let text_w = width - 16.0 - 24.0 - (yes_w + no_w + 12.0) - 32.0;
    let lines = c
        .wrap(question, &theme::mono(12.0, theme::TEXT), text_w, Some(3))
        .len();
    (32.0 + lines as f32 * 18.0).max(64.0)
}

const APPLY_QUESTION: &str = "Se va a cambiar este ajuste. Antes se guarda una copia del archivo y todo queda anotado en el historial, donde puedes deshacerlo. ¿Aplicar?";

/// The texts of a proposal card, wrapped to the width of the card.
struct CardText {
    evidence: Vec<String>,
    undo: Vec<String>,
    blockers: Vec<Vec<String>>,
    style: TextStyle,
    /// Height of one wrapped line.
    line: f32,
    width: f32,
}

impl CardText {
    fn of(c: &Canvas, p: &Proposal, width: f32) -> CardText {
        let style = theme::mono(12.5, theme::TEXT_2);
        CardText {
            evidence: c.wrap(&p.evidence, &style, width, None),
            undo: c.wrap(&p.undo, &style, width, None),
            blockers: p
                .blockers
                .iter()
                .map(|b| c.wrap(b, &theme::mono(12.0, theme::AMBER), width - 26.0, None))
                .collect(),
            line: c.line_height(&style) + 2.0,
            style,
            width,
        }
    }

    /// The height of the whole card, the footer (button or question) included.
    fn card_height(&self, footer: f32) -> f32 {
        let blocker_lines: usize = self.blockers.iter().map(Vec::len).sum();
        PAD + 40.0
            + 20.0
            + (14.0 + self.evidence.len() as f32 * self.line)
            + 16.0
            + 40.0
            + 16.0
            + (14.0 + self.undo.len() as f32 * self.line)
            + 16.0
            + blocker_lines as f32 * 18.0
            + footer
            + PAD
    }
}

fn proposal_card(c: &mut Canvas, m: &Model, p: &Proposal, area: Rect, y: f32) -> f32 {
    let asked = m.confirm == Some(Confirm::Apply(p.id.clone()));
    let text = CardText::of(c, p, area.w - 2.0 * PAD);
    let footer = if asked {
        confirm_height(c, APPLY_QUESTION, text.width) + 16.0
    } else {
        56.0
    };
    let height = text.card_height(footer);
    let rect = Rect::new(area.x, y, area.w, height);
    panel(c, rect);
    card_title(c, p, rect);
    let cursor = rect.y + PAD + 40.0 + 20.0;
    let cursor = text_section(c, rect, cursor, "POR QUÉ", &text.evidence, &text);
    let cursor = change_section(c, p, rect, cursor);
    let cursor = text_section(c, rect, cursor, "SE DESHACE", &text.undo, &text);
    let cursor = blockers(c, rect, cursor, &text.blockers);
    card_footer(c, p, rect, cursor, asked, text.width);
    height + 16.0
}

/// The title of the proposal and whether it can be done now.
fn card_title(c: &mut Canvas, p: &Proposal, rect: Rect) {
    icon_box(
        c,
        rect.x + PAD,
        rect.y + PAD,
        40.0,
        "sliders-horizontal",
        Tone::Green,
    );
    let head = Rect::new(rect.x + PAD + 54.0, rect.y + PAD, rect.w, 40.0);
    c.text_in(
        head.x,
        head,
        &p.title,
        theme::sans(16.0, theme::TEXT).bold(),
    );
    let (label, tone, icon) = if p.blockers.is_empty() {
        ("Listo para aplicar", Tone::Green, "circle-check")
    } else {
        ("Ahora no se puede", Tone::Amber, "triangle-alert")
    };
    let tag_w = c.width(label, &theme::mono(11.0, tone.color()).bold()) + 18.0 + 18.0;
    tag(
        c,
        rect.right() - PAD - tag_w,
        rect.y + PAD + 9.0,
        label,
        tone,
        Some(icon),
    );
}

/// A small heading and the lines of text under it; returns where the next section starts.
fn text_section(
    c: &mut Canvas,
    rect: Rect,
    mut cursor: f32,
    heading: &str,
    lines: &[String],
    text: &CardText,
) -> f32 {
    c.text(rect.x + PAD, cursor, heading, theme::eyebrow(theme::TEXT_3));
    cursor += 14.0;
    for l in lines {
        c.text(rect.x + PAD, cursor, l, text.style);
        cursor += text.line;
    }
    cursor + 16.0
}

/// What the change does: the value now, an arrow, and the value after.
fn change_section(c: &mut Canvas, p: &Proposal, rect: Rect, cursor: f32) -> f32 {
    c.text(
        rect.x + PAD,
        cursor,
        "CAMBIA",
        theme::eyebrow(theme::TEXT_3),
    );
    let chips_y = cursor + 18.0;
    let from = tag(
        c,
        rect.x + PAD,
        chips_y,
        &p.current.to_string(),
        Tone::Gray,
        None,
    );
    c.icon(
        "arrow-right",
        rect.x + PAD + from + 6.0,
        chips_y + 3.0,
        16.0,
        theme::TEXT_3,
    );
    tag(
        c,
        rect.x + PAD + from + 30.0,
        chips_y,
        &p.target.to_string(),
        Tone::Green,
        None,
    );
    cursor + 40.0 + 16.0
}

/// What blocks the change now, one warning per reason.
fn blockers(c: &mut Canvas, rect: Rect, mut cursor: f32, blockers: &[Vec<String>]) -> f32 {
    for lines in blockers {
        c.icon(
            "triangle-alert",
            rect.x + PAD,
            cursor + 1.0,
            14.0,
            theme::AMBER,
        );
        for l in lines {
            c.text(
                rect.x + PAD + 26.0,
                cursor,
                l,
                theme::mono(12.0, theme::AMBER),
            );
            cursor += 18.0;
        }
    }
    cursor
}

/// The footer: the button, or the question that follows it.
fn card_footer(c: &mut Canvas, p: &Proposal, rect: Rect, cursor: f32, asked: bool, text_w: f32) {
    if asked {
        let h = confirm_height(c, APPLY_QUESTION, text_w);
        let place = Rect::new(rect.x + PAD, cursor + 8.0, text_w, h);
        confirmation(c, APPLY_QUESTION, place);
        return;
    }
    let w = button_width(c, "Aplicar");
    let action = p.blockers.is_empty().then(|| Action::AskFix(p.id.clone()));
    let place = Rect::new(rect.x + PAD, cursor + 8.0, w, 40.0);
    button(c, place, "Aplicar", "zap", action.is_some(), action);
}

fn notice(c: &mut Canvas, m: &Model, area: Rect, y: f32) -> f32 {
    let Some(report) = &m.notice else { return 0.0 };
    let rect = Rect::new(area.x, y, area.w, 64.0);
    c.fill(rect, 8.0, Tone::Green.fill());
    c.stroke(rect, 8.0, 1.0, Tone::Green.border());
    c.icon(
        "circle-check",
        rect.x + 18.0,
        rect.y + 20.0,
        20.0,
        theme::ACCENT,
    );
    let verb = if report.was_undo {
        "Deshecho"
    } else {
        "Aplicado"
    };
    c.text(
        rect.x + 52.0,
        rect.y + 12.0,
        &format!("{verb}: {}", report.title),
        theme::sans(14.0, theme::TEXT).bold(),
    );
    let seq = report.seq.map_or(String::new(), |s| {
        format!(" · anotado en el historial como #{s}")
    });
    c.text(
        rect.x + 52.0,
        rect.y + 36.0,
        &format!("Ajuste: {} → {}{seq}", report.before, report.after),
        theme::mono(12.0, theme::TEXT_2),
    );
    80.0
}

/// Draws the page in `area` and returns the height it used.
pub fn draw(c: &mut Canvas, m: &Model, area: Rect) -> f32 {
    let mut y = area.y;
    y += page::head(
        c,
        m,
        area,
        "Arreglos",
        "cada cambio se explica, se anota y se puede deshacer",
        "Arreglos",
        "Purifex solo propone un arreglo cuando una partida medida lo justifica. Nada se cambia sin que lo confirmes, y todo se puede deshacer.",
    );
    if let Some(error) = &m.error {
        y += page::error_box(c, Rect::new(area.x, y, area.w, 0.0), error);
    }
    let (Some(fixes), Some(history)) = (&m.fixes, &m.history) else {
        if m.error.is_none() {
            y += page::reading(
                c,
                Rect::new(area.x, y, area.w, 60.0),
                "Leyendo los arreglos…",
            );
        }
        return y - area.y;
    };
    y += notice(c, m, area, y);
    if fixes.is_empty() {
        let rect = Rect::new(area.x, y, area.w, 92.0);
        panel(c, rect);
        c.icon(
            "circle-check",
            rect.x + 20.0,
            rect.y + 22.0,
            20.0,
            theme::TEXT_3,
        );
        c.text(
            rect.x + 56.0,
            rect.y + 20.0,
            "No hay arreglos que proponer",
            theme::sans(15.0, theme::TEXT).bold(),
        );
        c.paragraph(rect.x + 56.0, rect.y + 46.0, rect.w - 76.0, "Lo que se ha medido no los justifica. Graba una partida de al menos 2 minutos con purifex watch y vuelve aquí.", theme::mono(12.0, theme::TEXT_3), None);
        y += rect.h + 16.0;
    }
    for proposal in fixes {
        y += proposal_card(c, m, proposal, area, y);
    }
    y += history_panel(c, m, history, area, y);
    y - area.y
}

#[cfg(test)]
mod tests {
    use purifex_domain::draw::Op;
    use purifex_domain::fix::{
        FixReport, HistoryEntry, HistoryState, SettingValue, LOL_CLOSE_CLIENT,
    };

    use super::*;
    use crate::canvas::testing::FakeText;

    fn proposal(blockers: &[&str]) -> Proposal {
        Proposal {
            id: LOL_CLOSE_CLIENT.into(),
            title: "Cerrar el cliente de League durante la partida".into(),
            evidence:
                "El cliente de League usó de media un 15.9 % del CPU durante tu última partida."
                    .into(),
            current: SettingValue::Bool(false),
            target: SettingValue::Bool(true),
            undo: "Vuelve a poner el ajuste como estaba.".into(),
            needs_admin: false,
            blockers: blockers.iter().map(|b| b.to_string()).collect(),
        }
    }

    fn entry(seq: u64, state: HistoryState, undoes: Option<u64>) -> HistoryEntry {
        HistoryEntry {
            seq,
            fix: LOL_CLOSE_CLIENT.into(),
            before: SettingValue::Bool(false),
            after: SettingValue::Bool(true),
            state,
            undoes,
            session: None,
        }
    }

    fn model(fixes: Vec<Proposal>, history: Vec<HistoryEntry>) -> Model {
        Model {
            page: purifex_domain::view::Page::Fixes,
            fixes: Some(fixes),
            history: Some(history),
            ..Model::default()
        }
    }

    fn run(m: &Model) -> (String, Vec<Action>, f32) {
        let mut c = Canvas::new(&FakeText, None);
        let h = draw(&mut c, m, Rect::new(296.0, 80.0, 1000.0, 800.0));
        let (list, hits) = c.finish();
        let texts: Vec<String> = list
            .ops
            .iter()
            .filter_map(|op| {
                if let Op::Text { text, .. } = op {
                    Some(text.clone())
                } else {
                    None
                }
            })
            .collect();
        (
            texts.join(" | "),
            hits.into_iter().map(|h| h.action).collect(),
            h,
        )
    }

    #[test]
    fn a_proposal_shows_why_what_changes_how_it_is_undone_and_can_be_asked_to_apply() {
        let (text, actions, _) = run(&model(vec![proposal(&[])], vec![]));
        for expected in [
            "Cerrar el cliente de League durante la partida",
            "POR QUÉ",
            "15.9 %",
            "CAMBIA",
            "desactivado",
            "activado",
            "SE DESHACE",
            "Listo para aplicar",
            "APLICAR",
        ] {
            assert!(text.contains(expected), "{expected}: {text}");
        }
        assert!(
            actions.contains(&Action::AskFix(LOL_CLOSE_CLIENT.into())),
            "{actions:?}"
        );
        assert!(
            !actions.contains(&Action::Confirm),
            "a click on Aplicar only asks"
        );
    }

    #[test]
    fn what_blocks_a_fix_is_shown_and_its_button_does_nothing() {
        let (text, actions, _) = run(&model(
            vec![proposal(&["Cierra el cliente de League"])],
            vec![],
        ));
        assert!(
            text.contains("Ahora no se puede") && text.contains("Cierra el cliente de League"),
            "{text}"
        );
        assert!(
            !actions.iter().any(|a| matches!(a, Action::AskFix(_))),
            "no way to apply it now"
        );
    }

    #[test]
    fn after_asking_the_question_and_the_two_buttons_replace_the_apply_button() {
        let mut m = model(vec![proposal(&[])], vec![]);
        m.confirm = Some(Confirm::Apply(LOL_CLOSE_CLIENT.into()));
        let (text, actions, _) = run(&m);
        assert!(
            text.contains("¿Aplicar?") || text.contains("Aplicar?"),
            "{text}"
        );
        assert!(actions.contains(&Action::Confirm) && actions.contains(&Action::CancelConfirm));
        assert!(!actions.iter().any(|a| matches!(a, Action::AskFix(_))));
    }

    #[test]
    fn with_nothing_to_propose_it_says_why_and_how_to_get_one() {
        let (text, _, _) = run(&model(vec![], vec![]));
        assert!(
            text.contains("No hay arreglos que proponer") && text.contains("purifex watch"),
            "{text}"
        );
        assert!(text.contains("Todavía no se ha cambiado nada."));
    }

    #[test]
    fn the_history_lists_the_newest_first_and_only_a_live_change_can_be_undone() {
        let history = vec![
            entry(1, HistoryState::Applied, None),
            entry(2, HistoryState::Undone, Some(1)),
            entry(3, HistoryState::Applied, None),
        ];
        let (text, actions, _) = run(&model(vec![], history));
        assert!(
            text.find("#3").unwrap() < text.find("#1").unwrap(),
            "newest first: {text}"
        );
        assert!(text.contains("Deshecho después") && text.contains("Deshace #1"));
        let undo: Vec<&Action> = actions
            .iter()
            .filter(|a| matches!(a, Action::AskUndo(_)))
            .collect();
        assert_eq!(
            undo,
            [&Action::AskUndo(3)],
            "#1 is already undone and #2 is an undo"
        );
    }

    #[test]
    fn asking_to_undo_shows_its_question_and_hides_the_other_undo_buttons() {
        let mut m = model(
            vec![],
            vec![
                entry(1, HistoryState::Applied, None),
                entry(2, HistoryState::Applied, None),
            ],
        );
        m.confirm = Some(Confirm::Undo(2));
        let (text, actions, _) = run(&m);
        assert!(
            text.contains("Cambio #2") && text.contains("¿Deshacer?"),
            "{text}"
        );
        assert!(actions.contains(&Action::Confirm));
        assert!(!actions.iter().any(|a| matches!(a, Action::AskUndo(_))));
    }

    #[test]
    fn the_notice_of_a_change_says_what_was_done() {
        let mut m = model(vec![], vec![]);
        m.notice = Some(FixReport {
            fix: LOL_CLOSE_CLIENT.into(),
            title: "Cerrar el cliente".into(),
            before: SettingValue::Bool(false),
            after: SettingValue::Bool(true),
            preview: false,
            seq: Some(1),
            undo: String::new(),
            needs_confirmation: false,
            was_undo: false,
        });
        let (text, _, _) = run(&m);
        assert!(
            text.contains("Aplicado: Cerrar el cliente")
                && text.contains("desactivado → activado")
                && text.contains("#1"),
            "{text}"
        );
    }

    #[test]
    fn before_reading_it_says_so_and_a_failure_is_shown() {
        let (text, _, _) = run(&Model {
            page: purifex_domain::view::Page::Fixes,
            busy: true,
            ..Model::default()
        });
        assert!(text.contains("Leyendo los arreglos…"));
        let (text, _, _) = run(&Model {
            error: Some("ahora no se puede".into()),
            ..Model::default()
        });
        assert!(text.contains("No se pudo leer") && !text.contains("Leyendo los arreglos…"));
    }
}
