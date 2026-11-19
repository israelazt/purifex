//! The chosen game against the game before it: what got better, what got worse
//! and what stayed the same, with the reasons the comparison may not be fair.
//! The verdicts are the domain's (`Comparison`); this only draws them.

use purifex_domain::compare::{Change, ComparisonReport, MetricChange};
use purifex_domain::draw::Rect;

use crate::canvas::Canvas;
use crate::theme::{self, Tone};
use crate::widgets::{panel, tag};

const HEAD_H: f32 = 56.0;
const SUMMARY_H: f32 = 52.0;
const ROW_H: f32 = 38.0;
const COLUMN_W: f32 = 104.0;

fn warning_lines(c: &Canvas, report: &ComparisonReport, width: f32) -> Vec<Vec<String>> {
    let style = theme::mono(11.5, theme::AMBER.with_alpha(0.9));
    report
        .comparison
        .warnings
        .iter()
        .map(|w| c.wrap(w, &style, width - 64.0, None))
        .collect()
}

/// How tall the panel is for this report at this width.
pub fn height(c: &Canvas, report: &ComparisonReport, width: f32) -> f32 {
    let lines: usize = warning_lines(c, report, width).iter().map(Vec::len).sum();
    HEAD_H
        + SUMMARY_H
        + lines as f32 * 18.0
        + if lines > 0 { 8.0 } else { 0.0 }
        + ROW_H
        + report.comparison.rows.len() as f32 * ROW_H
        + 8.0
}

fn value(row: &MetricChange, v: f32) -> String {
    format!("{v:.*} {}", row.decimals, row.unit)
}

fn verdict(change: Change) -> (&'static str, Tone, &'static str) {
    match change {
        Change::Better => ("Mejor", Tone::Green, "circle-check"),
        Change::Worse => ("Peor", Tone::Red, "triangle-alert"),
        Change::Same => ("Igual", Tone::Gray, "minus"),
    }
}

pub fn draw(c: &mut Canvas, report: &ComparisonReport, rect: Rect) {
    panel(c, rect);
    header(c, report, rect);
    verdict_counts(c, report, rect);
    let cursor = rect.y + HEAD_H + SUMMARY_H;
    let cursor = warnings(c, report, rect, cursor);
    table(c, report, rect, cursor);
}

/// The title, and which games are being compared.
fn header(c: &mut Canvas, report: &ComparisonReport, rect: Rect) {
    c.icon("history", rect.x + 18.0, rect.y + 20.0, 16.0, theme::TEXT);
    c.text(
        rect.x + 42.0,
        rect.y + 20.0,
        "COMPARADA CON LA PARTIDA ANTERIOR",
        theme::mono(12.0, theme::TEXT).tracked(1.0),
    );
    let ids = format!("{} → {}", report.before, report.after).to_uppercase();
    let ids_style = theme::mono(11.5, theme::TEXT_3);
    let ids_x = (rect.right() - 18.0 - c.width(&ids, &ids_style)).max(rect.x + 380.0);
    c.text_cut(
        ids_x,
        rect.y + 21.0,
        &ids,
        ids_style,
        rect.right() - 18.0 - ids_x,
    );
    c.line(
        rect.x,
        rect.y + HEAD_H,
        rect.right(),
        rect.y + HEAD_H,
        theme::LINE_SOFT,
    );
}

/// How many measures got better, worse or stayed the same.
fn verdict_counts(c: &mut Canvas, report: &ComparisonReport, rect: Rect) {
    let k = &report.comparison;
    let mut x = rect.x + 18.0;
    let y = rect.y + HEAD_H + 15.0;
    for (n, label, tone) in [
        (k.better, "mejoran", Tone::Green),
        (k.worse, "empeoran", Tone::Red),
        (k.same, "sin cambio", Tone::Gray),
    ] {
        x += tag(c, x, y, &format!("{n} {label}"), tone, None) + 10.0;
    }
}

/// Why the comparison may not be fair, from `cursor` down; returns where the table starts.
fn warnings(c: &mut Canvas, report: &ComparisonReport, rect: Rect, mut cursor: f32) -> f32 {
    let wrapped = warning_lines(c, report, rect.w);
    for lines in &wrapped {
        c.icon(
            "triangle-alert",
            rect.x + 18.0,
            cursor + 1.0,
            14.0,
            theme::AMBER,
        );
        for line in lines {
            c.text(
                rect.x + 42.0,
                cursor,
                line,
                theme::mono(11.5, theme::AMBER.with_alpha(0.9)),
            );
            cursor += 18.0;
        }
    }
    if !wrapped.is_empty() {
        cursor += 8.0;
    }
    cursor
}

/// The left edge of the «antes», «después» and «cambio» columns.
fn columns(rect: Rect) -> [f32; 3] {
    [
        rect.right() - 18.0 - COLUMN_W * 4.0 - 8.0,
        rect.right() - 18.0 - COLUMN_W * 3.0 - 8.0,
        rect.right() - 18.0 - COLUMN_W * 2.0 - 8.0,
    ]
}

/// The head of the table and one row per measure, from `cursor` down.
fn table(c: &mut Canvas, report: &ComparisonReport, rect: Rect, cursor: f32) {
    let columns = columns(rect);
    let label = theme::eyebrow(theme::TEXT_3);
    let head = Rect::new(rect.x, cursor, rect.w, ROW_H);
    c.text_in(rect.x + 18.0, head, "MEDIDA", label);
    for (x, text) in columns.iter().zip(["ANTES", "DESPUÉS", "CAMBIO"]) {
        c.text_right(x + COLUMN_W, head, text, label);
    }
    c.text_in(
        rect.right() - 18.0 - COLUMN_W + 14.0,
        head,
        "VEREDICTO",
        label,
    );
    c.line(
        rect.x,
        head.bottom(),
        rect.right(),
        head.bottom(),
        theme::LINE,
    );
    for (i, row) in report.comparison.rows.iter().enumerate() {
        let bar = Rect::new(rect.x, head.bottom() + i as f32 * ROW_H, rect.w, ROW_H);
        metric_row(c, row, bar, rect, &columns);
    }
}

/// One measure: its name, the value before and after, the change and the verdict.
fn metric_row(c: &mut Canvas, row: &MetricChange, bar: Rect, rect: Rect, columns: &[f32; 3]) {
    c.line(
        bar.x,
        bar.bottom() - 0.5,
        bar.right(),
        bar.bottom() - 0.5,
        theme::LINE_SOFT,
    );
    c.text_cut(
        rect.x + 18.0,
        bar.y + (ROW_H - 16.0) / 2.0,
        row.label,
        theme::mono(12.0, theme::TEXT),
        columns[0] - rect.x - 30.0,
    );
    let plain = theme::mono(12.0, theme::TEXT_2);
    c.text_right(columns[0] + COLUMN_W, bar, &value(row, row.before), plain);
    c.text_right(
        columns[1] + COLUMN_W,
        bar,
        &value(row, row.after),
        theme::mono(12.0, theme::TEXT).bold(),
    );
    let (label, tone, icon) = verdict(row.change);
    let delta = format!("{:+.*} {}", row.decimals, row.delta, row.unit);
    c.text_right(
        columns[2] + COLUMN_W,
        bar,
        &delta,
        theme::mono(12.0, tone.color()),
    );
    tag(
        c,
        rect.right() - 18.0 - COLUMN_W + 14.0,
        bar.y + 8.0,
        label,
        tone,
        Some(icon),
    );
}

#[cfg(test)]
mod tests {
    use purifex_domain::analysis::Analysis;
    use purifex_domain::draw::Op;
    use purifex_domain::session_csv::parse;

    use super::*;
    use crate::canvas::testing::FakeText;

    fn analysis(id: &str, seconds: u32, core: f32, client: f32) -> Analysis {
        let mut csv = String::from("t_s,cpu_total,cpu_max_core,game_cpu,game_mb,groups\n");
        for t in 1..=seconds {
            csv.push_str(&format!(
                "{t}.0,40.0,{core},13.0,900,league_client={client}\n"
            ));
        }
        Analysis::of(id, parse(&csv).unwrap()).unwrap()
    }

    fn texts(report: &ComparisonReport, width: f32) -> (String, f32, f32) {
        let mut c = Canvas::new(&FakeText, None);
        let h = height(&c, report, width);
        draw(&mut c, report, Rect::new(0.0, 0.0, width, h));
        let (list, _) = c.finish();
        let t: Vec<String> = list
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
        let bottom = list
            .ops
            .iter()
            .filter_map(|op| {
                if let Op::Line { y2, .. } = op {
                    Some(*y2)
                } else {
                    None
                }
            })
            .fold(0.0f32, f32::max);
        (t.join(" | "), h, bottom)
    }

    #[test]
    fn it_shows_each_measure_before_and_after_with_its_verdict() {
        let report = ComparisonReport::of(
            &analysis("lol-1", 600, 88.0, 16.0),
            &analysis("lol-2", 600, 60.0, 0.5),
        );
        let (text, _, _) = texts(&report, 1000.0);
        for expected in [
            "COMPARADA CON LA PARTIDA ANTERIOR",
            "LOL-1 → LOL-2",
            "Núcleo más cargado (p95)",
            "88.0 %",
            "60.0 %",
            "-28.0 %",
            "Mejor",
            "Igual",
            "mejoran",
        ] {
            assert!(text.contains(expected), "{expected}: {text}");
        }
    }

    #[test]
    fn a_comparison_that_may_not_be_fair_shows_its_warnings_and_the_panel_grows_for_them() {
        let fair = ComparisonReport::of(
            &analysis("a", 600, 80.0, 0.0),
            &analysis("b", 600, 80.0, 0.0),
        );
        let unfair = ComparisonReport::of(
            &analysis("a", 200, 80.0, 0.0),
            &analysis("b", 600, 80.0, 0.0),
        );
        let (text, h_unfair, _) = texts(&unfair, 1000.0);
        let (_, h_fair, _) = texts(&fair, 1000.0);
        assert!(text.contains("poco fiable"), "{text}");
        assert!(h_unfair > h_fair);
    }

    #[test]
    fn everything_drawn_stays_inside_the_height_it_asked_for() {
        let report = ComparisonReport::of(
            &analysis("a", 200, 88.0, 16.0),
            &analysis("b", 600, 60.0, 0.5),
        );
        let (_, h, lowest_line) = texts(&report, 900.0);
        assert!(
            lowest_line <= h,
            "a row passes the bottom of the panel: {lowest_line} > {h}"
        );
    }
}
