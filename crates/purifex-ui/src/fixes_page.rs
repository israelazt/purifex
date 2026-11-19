//! The fixes page: what Purifex proposes (each with the measurement that justifies
//! it, what it changes, how it is undone and what stops it now), the notice of the
//! last change, and the history with its "undo". Nothing is changed by a click
//! on "Aplicar": it asks to confirm first, and only the confirmation acts.

use purifex_domain::draw::Rect;
use purifex_domain::fix::{is_undone, spec, HistoryEntry, HistoryState, Proposal};
use purifex_domain::view::{Action, Confirm, Model};

use crate::canvas::Canvas;
use crate::page;
use crate::theme::{self, Tone};
use crate::widgets::{button, button_width, icon_box, panel, tag};

const PAD: f32 = 20.0;

/// The question and the two buttons that follow a click on "Aplicar" or "Deshacer".
fn confirmation(c: &mut Canvas, question: &str, bar: Rect) {
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

fn confirm_height(c: &Canvas, question: &str, width: f32) -> f32 {
    let (yes_w, no_w) = (button_width(c, "Confirmar"), button_width(c, "Cancelar"));
    let text_w = width - 16.0 - 24.0 - (yes_w + no_w + 12.0) - 32.0;
    let lines = c
        .wrap(question, &theme::mono(12.0, theme::TEXT), text_w, Some(3))
        .len();
    (32.0 + lines as f32 * 18.0).max(64.0)
}

const APPLY_QUESTION: &str = "Se va a cambiar este ajuste. Antes se guarda una copia del archivo y todo queda anotado en el historial, donde puedes deshacerlo. ¿Aplicar?";
const UNDO_QUESTION: &str = "El ajuste volverá al valor que tenía antes de este cambio. ¿Deshacer?";

fn proposal_card(c: &mut Canvas, m: &Model, p: &Proposal, area: Rect, y: f32) -> f32 {
    let asked = m.confirm == Some(Confirm::Apply(p.id.clone()));
    let text_w = area.w - 2.0 * PAD;
    let evidence_style = theme::mono(12.5, theme::TEXT_2);
    let evidence = c.wrap(&p.evidence, &evidence_style, text_w, None);
    let undo = c.wrap(&p.undo, &evidence_style, text_w, None);
    let line = c.line_height(&evidence_style) + 2.0;
    let blockers: Vec<Vec<String>> = p
        .blockers
        .iter()
        .map(|b| c.wrap(b, &theme::mono(12.0, theme::AMBER), text_w - 26.0, None))
        .collect();
    let blocker_lines: usize = blockers.iter().map(Vec::len).sum();
    let footer = if asked {
        confirm_height(c, APPLY_QUESTION, text_w) + 16.0
    } else {
        56.0
    };
    let height = PAD
        + 40.0
        + 20.0
        + (14.0 + evidence.len() as f32 * line)
        + 16.0
        + 40.0
        + 16.0
        + (14.0 + undo.len() as f32 * line)
        + 16.0
        + blocker_lines as f32 * 18.0
        + footer
        + PAD;
    let rect = Rect::new(area.x, y, area.w, height);
    panel(c, rect);

    // The title and whether it can be done now.
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

    let label_style = theme::eyebrow(theme::TEXT_3);
    let mut cursor = rect.y + PAD + 40.0 + 20.0;
    c.text(rect.x + PAD, cursor, "POR QUÉ", label_style);
    cursor += 14.0;
    for l in &evidence {
        c.text(rect.x + PAD, cursor, l, evidence_style);
        cursor += line;
    }
    cursor += 16.0;
    c.text(rect.x + PAD, cursor, "CAMBIA", label_style);
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
    cursor += 40.0 + 16.0;
    c.text(rect.x + PAD, cursor, "SE DESHACE", label_style);
    cursor += 14.0;
    for l in &undo {
        c.text(rect.x + PAD, cursor, l, evidence_style);
        cursor += line;
    }
    cursor += 16.0;
    for lines in &blockers {
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
    // The footer: the button, or the question that follows it.
    if asked {
        let h = confirm_height(c, APPLY_QUESTION, text_w);
        confirmation(
            c,
            APPLY_QUESTION,
            Rect::new(rect.x + PAD, cursor + 8.0, text_w, h),
        );
    } else {
        let w = button_width(c, "Aplicar");
        let action = p.blockers.is_empty().then(|| Action::AskFix(p.id.clone()));
        button(
            c,
            Rect::new(rect.x + PAD, cursor + 8.0, w, 40.0),
            "Aplicar",
            "zap",
            action.is_some(),
            action,
        );
    }
    height + 16.0
}

fn history_height(c: &Canvas, m: &Model, rows: usize, width: f32) -> f32 {
    let asked = matches!(m.confirm, Some(Confirm::Undo(_)));
    56.0 + if rows == 0 { 56.0 } else { rows as f32 * 56.0 }
        + if asked {
            confirm_height(c, UNDO_QUESTION, width - 32.0) + 24.0
        } else {
            0.0
        }
        + 8.0
}

fn history_panel(c: &mut Canvas, m: &Model, entries: &[HistoryEntry], area: Rect, y: f32) -> f32 {
    let height = history_height(c, m, entries.len(), area.w);
    let rect = Rect::new(area.x, y, area.w, height);
    panel(c, rect);
    c.icon("history", rect.x + 18.0, rect.y + 20.0, 16.0, theme::TEXT);
    c.text(
        rect.x + 42.0,
        rect.y + 20.0,
        "HISTORIAL DE CAMBIOS",
        theme::mono(12.0, theme::TEXT).tracked(1.0),
    );
    c.line(
        rect.x,
        rect.y + 56.0,
        rect.right(),
        rect.y + 56.0,
        theme::LINE_SOFT,
    );
    if entries.is_empty() {
        c.text(
            rect.x + 18.0,
            rect.y + 56.0 + 20.0,
            "Todavía no se ha cambiado nada.",
            theme::mono(12.5, theme::TEXT_3),
        );
        return height + 16.0;
    }
    for (i, entry) in entries.iter().rev().enumerate() {
        let bar = Rect::new(rect.x, rect.y + 56.0 + i as f32 * 56.0, rect.w, 56.0);
        c.line(
            bar.x,
            bar.bottom() - 0.5,
            bar.right(),
            bar.bottom() - 0.5,
            theme::LINE_SOFT,
        );
        c.text_in(
            bar.x + 18.0,
            bar,
            &format!("#{}", entry.seq),
            theme::mono(12.5, theme::TEXT_3),
        );
        let title = spec(&entry.fix).map_or(entry.fix.as_str(), |s| s.title);
        c.text_cut(
            bar.x + 64.0,
            bar.y + 10.0,
            title,
            theme::sans(13.5, theme::TEXT).bold(),
            bar.w - 64.0 - 330.0,
        );
        c.text_cut(
            bar.x + 64.0,
            bar.y + 31.0,
            &format!("{} → {}", entry.before, entry.after),
            theme::mono(11.5, theme::TEXT_2),
            bar.w - 64.0 - 330.0,
        );
        let undone = is_undone(entries, entry.seq);
        let (label, tone) = match entry.state {
            HistoryState::Applied if undone => ("Deshecho después".to_string(), Tone::Gray),
            HistoryState::Applied => ("Aplicado".to_string(), Tone::Green),
            HistoryState::Undone => (
                format!("Deshace #{}", entry.undoes.unwrap_or(0)),
                Tone::Gray,
            ),
        };
        tag(
            c,
            bar.right() - 18.0 - 140.0 - 110.0,
            bar.y + 17.0,
            &label,
            tone,
            None,
        );
        if entry.state == HistoryState::Applied && !undone && m.confirm.is_none() {
            let w = button_width(c, "Deshacer");
            button(
                c,
                Rect::new(bar.right() - 18.0 - w, bar.y + 8.0, w, 40.0),
                "Deshacer",
                "undo-2",
                false,
                Some(Action::AskUndo(entry.seq)),
            );
        }
    }
    if let Some(Confirm::Undo(seq)) = &m.confirm {
        let h = confirm_height(c, UNDO_QUESTION, rect.w - 32.0);
        let text = format!("Cambio #{seq}. {UNDO_QUESTION}");
        confirmation(
            c,
            &text,
            Rect::new(rect.x + 16.0, rect.bottom() - h - 16.0, rect.w - 32.0, h),
        );
    }
    height + 16.0
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
    use purifex_domain::fix::{FixReport, SettingValue, LOL_CLOSE_CLIENT};

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
