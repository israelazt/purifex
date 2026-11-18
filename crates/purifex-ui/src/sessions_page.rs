//! The recordings page: the list of games and, for the one chosen, what limited
//! the computer and the table of measures. The lists and the rows of the table
//! come from the domain (`SessionRow`, `Analysis::stat_rows`).

use purifex_domain::analysis::{Analysis, StatRow};
use purifex_domain::draw::Rect;
use purifex_domain::session_list::{duration_text, SessionRow};
use purifex_domain::view::{Action, Model};

use crate::canvas::Canvas;
use crate::page;
use crate::theme::{self, Tone};
use crate::widgets::{icon_box, panel};

const ROW_H: f32 = 60.0;
const LIST_W: f32 = 340.0;

fn session_row(c: &mut Canvas, row: &SessionRow, bar: Rect, selected: bool) {
    let hovered = c.hovered(bar);
    if selected {
        c.fill(bar, 0.0, theme::ACCENT.with_alpha(0.06));
        c.fill(Rect::new(bar.x, bar.y, 2.0, bar.h), 0.0, theme::ACCENT);
    } else if hovered {
        c.fill(bar, 0.0, theme::TEXT.with_alpha(0.02));
    }
    c.line(
        bar.x,
        bar.bottom() - 0.5,
        bar.right(),
        bar.bottom() - 0.5,
        theme::LINE_SOFT,
    );
    let broken = row.problem.is_some();
    icon_box(
        c,
        bar.x + 16.0,
        bar.y + 12.0,
        36.0,
        if broken {
            "triangle-alert"
        } else {
            "gamepad-2"
        },
        if broken { Tone::Amber } else { Tone::Green },
    );
    let title = row.started.map_or(row.id.clone(), |s| s.to_string());
    c.text_cut(
        bar.x + 64.0,
        bar.y + 11.0,
        &title,
        theme::mono(12.5, theme::TEXT).bold(),
        bar.w - 64.0 - 100.0,
    );
    let sub = if broken {
        "no se puede leer".to_string()
    } else {
        format!("{} muestras", row.samples)
    };
    c.text(
        bar.x + 64.0,
        bar.y + 31.0,
        &sub,
        theme::mono(11.0, theme::TEXT_3),
    );
    if !broken {
        let text = duration_text(row.seconds);
        let style = theme::mono(12.0, theme::TEXT_2);
        c.text(
            bar.right() - 16.0 - c.width(&text, &style),
            bar.y + 11.0,
            &text,
            style,
        );
    }
    c.hit(bar, Action::SelectSession(row.id.clone()));
}

fn list(c: &mut Canvas, m: &Model, rect: Rect) {
    panel(c, rect);
    let Some(rows) = &m.sessions else { return };
    if rows.is_empty() {
        c.paragraph(
            rect.x + 16.0,
            rect.y + 18.0,
            rect.w - 32.0,
            "Todavía no hay grabaciones. Graba una partida con purifex watch.",
            theme::mono(12.0, theme::TEXT_3),
            None,
        );
        return;
    }
    for (i, row) in rows.iter().enumerate() {
        let selected = m.selected.as_deref() == Some(row.id.as_str());
        session_row(
            c,
            row,
            Rect::new(rect.x, rect.y + i as f32 * ROW_H, rect.w, ROW_H),
            selected,
        );
    }
}

fn list_height(m: &Model) -> f32 {
    m.sessions.as_ref().map_or(80.0, |rows| {
        if rows.is_empty() {
            80.0
        } else {
            rows.len() as f32 * ROW_H
        }
    })
}

fn number(row: &StatRow, value: f32) -> String {
    let n = if row.decimals == 0 {
        format!("{:.0}", value)
    } else {
        format!("{value:.1}")
    };
    format!("{n} {}", row.unit)
}

/// The detail of the chosen recording; returns its height.
fn detail(c: &mut Canvas, a: &Analysis, rect: Rect) -> f32 {
    // The leads, measured first so the panel is as tall as what it holds.
    let title_style = theme::sans(14.0, theme::TEXT).bold();
    let text_style = theme::mono(12.0, theme::TEXT_2);
    let wrap_w = rect.w - 36.0 - 30.0;
    let leads: Vec<(String, Vec<String>)> = a
        .findings
        .iter()
        .map(|f| {
            (
                f.title.clone(),
                c.wrap(&f.evidence, &text_style, wrap_w, None),
            )
        })
        .collect();
    let mut head_h = 56.0 + 16.0;
    if leads.is_empty() {
        head_h += 34.0;
    }
    for (_, lines) in &leads {
        head_h += 26.0 + lines.len() as f32 * (c.line_height(&text_style) + 2.0) + 22.0;
    }
    let top = Rect::new(rect.x, rect.y, rect.w, head_h);
    panel(c, top);
    c.icon(
        "chart-no-axes-combined",
        top.x + 18.0,
        top.y + 20.0,
        16.0,
        theme::TEXT,
    );
    c.text(
        top.x + 42.0,
        top.y + 20.0,
        &a.id.to_uppercase(),
        theme::mono(12.0, theme::TEXT).tracked(1.0),
    );
    let who = match &a.header {
        Some(h) => format!(
            "{} · {} núcleos · {}",
            h.game,
            h.cores,
            duration_text(a.summary.seconds)
        ),
        None => format!(
            "sin cabecera (grabación antigua) · {}",
            duration_text(a.summary.seconds)
        ),
    };
    let who_style = theme::mono(11.5, theme::TEXT_3);
    c.text(
        top.right() - 18.0 - c.width(&who, &who_style),
        top.y + 21.0,
        &who,
        who_style,
    );
    c.line(
        top.x,
        top.y + 56.0,
        top.right(),
        top.y + 56.0,
        theme::LINE_SOFT,
    );
    let mut y = top.y + 72.0;
    if leads.is_empty() {
        c.icon("circle-check", top.x + 18.0, y, 16.0, theme::TEXT_3);
        c.text(
            top.x + 42.0,
            y,
            "Nada destacable en esta partida.",
            theme::mono(12.5, theme::TEXT_3),
        );
    }
    for (title, lines) in leads {
        c.icon("triangle-alert", top.x + 18.0, y + 1.0, 18.0, theme::AMBER);
        c.text(top.x + 48.0, y, &title, title_style);
        y += 26.0;
        for line in lines {
            c.text(top.x + 48.0, y, &line, text_style);
            y += c.line_height(&text_style) + 2.0;
        }
        y += 22.0;
    }

    // The table of measures.
    let rows = a.stat_rows();
    let table = Rect::new(
        rect.x,
        top.bottom() + 16.0,
        rect.w,
        44.0 + rows.len() as f32 * 40.0 + 44.0,
    );
    panel(c, table);
    let columns = [
        table.right() - 16.0 - 100.0 * 3.0,
        table.right() - 16.0 - 100.0 * 2.0,
        table.right() - 16.0 - 100.0,
    ];
    let label = theme::eyebrow(theme::TEXT_3);
    let head = Rect::new(table.x, table.y, table.w, 44.0);
    c.text_in(table.x + 16.0, head, "MEDIDA", label);
    for (x, text) in columns.iter().zip(["MEDIA", "P95", "MÁX."]) {
        c.text_right(x + 100.0, head, text, label);
    }
    c.line(
        table.x,
        table.y + 44.0,
        table.right(),
        table.y + 44.0,
        theme::LINE,
    );
    for (i, row) in rows.iter().enumerate() {
        let bar = Rect::new(table.x, table.y + 44.0 + i as f32 * 40.0, table.w, 40.0);
        c.line(
            bar.x,
            bar.bottom() - 0.5,
            bar.right(),
            bar.bottom() - 0.5,
            theme::LINE_SOFT,
        );
        c.text_in(
            bar.x + 16.0,
            bar,
            &row.label,
            theme::mono(12.5, theme::TEXT),
        );
        for (x, value) in columns
            .iter()
            .zip([row.stat.avg, row.stat.p95, row.stat.max])
        {
            c.text_right(
                x + 100.0,
                bar,
                &number(row, value),
                theme::mono(12.5, theme::TEXT).bold(),
            );
        }
    }
    let foot = table.bottom() - 44.0;
    c.text_in(
        table.x + 16.0,
        Rect::new(table.x, foot, table.w, 44.0),
        "Medir costó",
        theme::mono(11.0, theme::TEXT_3),
    );
    let cost = format!(
        "{:.1} ms por muestra",
        a.summary.sample_cost_us.avg / 1000.0
    );
    let cost_style = theme::mono(11.0, theme::ACCENT).bold();
    c.text_right(
        table.right() - 16.0,
        Rect::new(table.x, foot, table.w, 44.0),
        &cost,
        cost_style,
    );
    table.bottom() - rect.y
}

/// Draws the page in `area` and returns the height it used.
pub fn draw(c: &mut Canvas, m: &Model, area: Rect) -> f32 {
    let mut y = area.y;
    y += page::head(
        c,
        m,
        area,
        "Partidas grabadas",
        "purifex watch · una muestra por segundo",
        "Partidas",
        "Elige una grabación para ver qué limitó al equipo mientras jugabas.",
    );
    if let Some(error) = &m.error {
        y += page::error_box(c, Rect::new(area.x, y, area.w, 0.0), error);
    }
    if m.sessions.is_none() {
        if m.error.is_none() {
            y += page::reading(
                c,
                Rect::new(area.x, y, area.w, 60.0),
                "Leyendo las grabaciones…",
            );
        }
        return y - area.y;
    }
    let list_rect = Rect::new(area.x, y, LIST_W.min(area.w * 0.4), list_height(m));
    list(c, m, list_rect);
    let detail_rect = Rect::new(
        list_rect.right() + 16.0,
        y,
        area.w - list_rect.w - 16.0,
        0.0,
    );
    let used = match &m.analysis {
        Some(a) => detail(c, a, detail_rect),
        None => {
            let rect = Rect::new(detail_rect.x, y, detail_rect.w, 70.0);
            panel(c, rect);
            let text = if m.busy {
                "Leyendo la grabación…"
            } else {
                "Elige una grabación."
            };
            c.text(
                rect.x + 18.0,
                rect.y + 24.0,
                text,
                theme::mono(12.5, theme::TEXT_3),
            );
            70.0
        }
    };
    y + used.max(list_rect.h) - area.y
}

#[cfg(test)]
mod tests {
    use purifex_domain::analysis::Analysis;
    use purifex_domain::draw::Op;
    use purifex_domain::session_csv::parse;
    use purifex_domain::session_id::stamp_of;

    use super::*;
    use crate::canvas::testing::FakeText;

    const CSV: &str = "# purifex-session v1 game=\"League of Legends.exe\" cores=12 started=2026-11-18\nt_s,cpu_total,cpu_max_core,game_cpu,game_mb,groups\n1.0,40.0,99.0,13.0,900,league_client=16.0\n200.0,40.0,98.0,13.0,900,league_client=17.0\n";

    fn row(id: &str, problem: Option<&str>) -> SessionRow {
        SessionRow {
            id: id.into(),
            started: stamp_of(id),
            seconds: 1566,
            samples: 1567,
            has_summary: false,
            game: None,
            problem: problem.map(String::from),
        }
    }

    fn model() -> Model {
        Model {
            sessions: Some(vec![
                row("lol-20261118-165329", None),
                row("lol-20261118-120000", Some("falta la columna")),
            ]),
            selected: Some("lol-20261118-165329".into()),
            analysis: Some(Analysis::of("lol-20261118-165329", parse(CSV).unwrap()).unwrap()),
            ..Model::default()
        }
    }

    fn run(m: &Model) -> (String, Vec<crate::canvas::Hit>) {
        let mut c = Canvas::new(&FakeText, None);
        draw(&mut c, m, Rect::new(296.0, 80.0, 1080.0, 800.0));
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
        (texts.join(" | "), hits)
    }

    #[test]
    fn the_list_shows_each_recording_and_flags_the_one_that_cannot_be_read() {
        let (text, hits) = run(&model());
        assert!(
            text.contains("2026-11-18 16:53:29") && text.contains("26 min 06 s"),
            "{text}"
        );
        assert!(text.contains("no se puede leer"));
        let chosen: Vec<&Action> = hits
            .iter()
            .map(|h| &h.action)
            .filter(|a| matches!(a, Action::SelectSession(_)))
            .collect();
        assert_eq!(chosen.len(), 2, "both can be clicked");
    }

    #[test]
    fn the_detail_names_the_game_the_leads_and_the_measures() {
        let (text, _) = run(&model());
        for expected in [
            "LOL-20261118-165329",
            "League of Legends.exe · 12 núcleos",
            "CPU total",
            "El juego · CPU",
            "Cliente de League · CPU",
            "MEDIA",
            "Medir costó",
        ] {
            assert!(text.contains(expected), "{expected}: {text}");
        }
        assert!(text.contains("13.0 %"), "the game's CPU with its unit");
    }

    #[test]
    fn a_recording_without_a_header_says_it_is_an_old_one() {
        let mut m = model();
        let old = "t_s,cpu_total,cpu_max_core\n1.0,1.0,1.0\n2.0,1.0,1.0\n";
        m.analysis = Some(Analysis::of("old", parse(old).unwrap()).unwrap());
        let (text, _) = run(&m);
        assert!(text.contains("sin cabecera (grabación antigua)"));
        assert!(text.contains("Nada destacable en esta partida."));
    }

    #[test]
    fn with_no_recordings_it_says_how_to_make_one() {
        let (text, _) = run(&Model {
            sessions: Some(vec![]),
            ..Model::default()
        });
        assert!(text.contains("purifex watch"));
        assert!(text.contains("Elige una grabación."));
    }

    #[test]
    fn before_reading_it_says_so_and_a_failure_is_shown() {
        let (text, _) = run(&Model {
            busy: true,
            ..Model::default()
        });
        assert!(text.contains("Leyendo las grabaciones…"));
        let (text, _) = run(&Model {
            error: Some("grabaciones: sin acceso".into()),
            ..Model::default()
        });
        assert!(text.contains("No se pudo leer"));
    }

    #[test]
    fn a_measure_is_written_with_its_unit_and_decimals() {
        let r = StatRow {
            label: "x".into(),
            stat: Default::default(),
            unit: "MB",
            decimals: 0,
        };
        assert_eq!(number(&r, 1019.6), "1020 MB");
        let r = StatRow {
            decimals: 1,
            unit: "%",
            ..r
        };
        assert_eq!(number(&r, 13.54), "13.5 %");
    }
}
