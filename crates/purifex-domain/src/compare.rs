//! Two recordings side by side: did a change (closing the League client, a
//! driver, a setting) make the games better or not? Each measure has a direction
//! (less CPU is better) and a noise: a difference smaller than what two games of
//! the same PC differ by anyway is "same", not an improvement.

use std::fmt::Write;

use serde::Serialize;

use crate::analysis::{Analysis, MIN_GAME_SECONDS};
use crate::process_group::Group;

/// The seconds under which a comparison is not to be trusted.
const SHORT_SECONDS: u64 = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Change {
    Better,
    Worse,
    Same,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MetricChange {
    pub key: &'static str,
    pub label: &'static str,
    pub unit: &'static str,
    pub decimals: usize,
    pub before: f32,
    pub after: f32,
    pub delta: f32,
    pub change: Change,
}

/// A comparison with the names of the two recordings.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ComparisonReport {
    pub before: String,
    pub after: String,
    #[serde(flatten)]
    pub comparison: Comparison,
}

impl ComparisonReport {
    pub fn of(before: &Analysis, after: &Analysis) -> Self {
        Self {
            before: before.id.clone(),
            after: after.id.clone(),
            comparison: compare(before, after),
        }
    }

    pub fn to_text(&self) -> String {
        self.comparison.to_text(&self.before, &self.after)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Comparison {
    pub rows: Vec<MetricChange>,
    pub better: usize,
    pub worse: usize,
    pub same: usize,
    /// Why the comparison may not be fair (too short, not games, different PCs).
    pub warnings: Vec<String>,
}

/// What is measured and how to judge it.
struct Spec {
    key: &'static str,
    label: &'static str,
    unit: &'static str,
    decimals: usize,
    /// Whether a lower value is the better one.
    lower_is_better: bool,
    /// A difference under this is the noise between two games.
    noise: f32,
}

const CORE_P95: Spec = Spec {
    key: "core_p95",
    label: "Núcleo más cargado (p95)",
    unit: "%",
    decimals: 1,
    lower_is_better: true,
    noise: 3.0,
};
const CORE_PINNED: Spec = Spec {
    key: "core_pinned",
    label: "Segundos con un núcleo >= 95 %",
    unit: "%",
    decimals: 0,
    lower_is_better: true,
    noise: 3.0,
};
const CPU_TOTAL: Spec = Spec {
    key: "cpu_total",
    label: "CPU total (media)",
    unit: "%",
    decimals: 1,
    lower_is_better: true,
    noise: 3.0,
};
const DPC: Spec = Spec {
    key: "dpc_p95",
    label: "Interrupciones / DPC (p95)",
    unit: "%",
    decimals: 1,
    lower_is_better: true,
    noise: 0.5,
};
const RAM: Spec = Spec {
    key: "ram_p95",
    label: "RAM en uso (p95)",
    unit: "%",
    decimals: 0,
    lower_is_better: true,
    noise: 3.0,
};
const LEAGUE_CLIENT: Spec = Spec {
    key: "league_client",
    label: "Cliente de League (media)",
    unit: "%",
    decimals: 1,
    lower_is_better: true,
    noise: 1.5,
};
const GPU_TEMP: Spec = Spec {
    key: "gpu_temp",
    label: "GPU · temperatura máx.",
    unit: "°C",
    decimals: 0,
    lower_is_better: true,
    noise: 2.0,
};
const GPU_CLOCK: Spec = Spec {
    key: "gpu_clock",
    label: "GPU · reloj (de la máx.)",
    unit: "%",
    decimals: 0,
    lower_is_better: false,
    noise: 5.0,
};

fn change(spec: &Spec, before: f32, after: f32) -> MetricChange {
    let delta = after - before;
    let judged = if delta.abs() < spec.noise {
        Change::Same
    } else if (delta < 0.0) == spec.lower_is_better {
        Change::Better
    } else {
        Change::Worse
    };
    MetricChange {
        key: spec.key,
        label: spec.label,
        unit: spec.unit,
        decimals: spec.decimals,
        before,
        after,
        delta,
        change: judged,
    }
}

fn client_cpu(a: &Analysis) -> f32 {
    a.summary
        .groups
        .iter()
        .find(|g| g.group == Group::LeagueClient)
        .map_or(0.0, |g| g.cpu.avg)
}

/// Compares `after` with `before`.
pub fn compare(before: &Analysis, after: &Analysis) -> Comparison {
    let (b, a) = (&before.summary, &after.summary);
    let mut rows = vec![
        change(&CORE_P95, b.cpu_max_core.p95, a.cpu_max_core.p95),
        change(&CORE_PINNED, b.core_pinned_pct, a.core_pinned_pct),
        change(&CPU_TOTAL, b.cpu_total.avg, a.cpu_total.avg),
        change(&DPC, b.dpc_interrupt.p95, a.dpc_interrupt.p95),
        change(&RAM, b.ram_used.p95, a.ram_used.p95),
    ];
    // The League client only when it showed up in either recording.
    if client_cpu(before) > 0.0 || client_cpu(after) > 0.0 {
        rows.push(change(
            &LEAGUE_CLIENT,
            client_cpu(before),
            client_cpu(after),
        ));
    }
    if let (Some(gb), Some(ga)) = (&b.gpu, &a.gpu) {
        rows.push(change(&GPU_TEMP, gb.temp_c.max, ga.temp_c.max));
        // A card that is the limit always runs fast: its clock says nothing then.
        if gb.util.p95 < 95.0 && ga.util.p95 < 95.0 {
            rows.push(change(&GPU_CLOCK, gb.clock_pct.avg, ga.clock_pct.avg));
        }
    }
    let count = |wanted: Change| rows.iter().filter(|r| r.change == wanted).count();
    Comparison {
        better: count(Change::Better),
        worse: count(Change::Worse),
        same: count(Change::Same),
        warnings: warnings(before, after),
        rows,
    }
}

fn warnings(before: &Analysis, after: &Analysis) -> Vec<String> {
    let mut out = Vec::new();
    for (name, analysis) in [("la primera", before), ("la segunda", after)] {
        if !analysis.is_game() {
            out.push(format!("{name} grabación no es una partida (menos de {} min o sin el juego abierto): los números no valen para comparar", MIN_GAME_SECONDS / 60));
        } else if analysis.summary.seconds < SHORT_SECONDS {
            out.push(format!(
                "{name} partida dura solo {} s: poco fiable",
                analysis.summary.seconds
            ));
        }
    }
    let (short, long) = (
        before.summary.seconds.min(after.summary.seconds),
        before.summary.seconds.max(after.summary.seconds),
    );
    if long > 0 && short * 2 < long {
        out.push("las dos duran muy distinto: una partida corta no se parece a una larga".into());
    }
    if let (Some(hb), Some(ha)) = (&before.header, &after.header) {
        if hb.cores != ha.cores {
            out.push(format!(
                "se grabaron en equipos con distinto número de núcleos ({} y {})",
                hb.cores, ha.cores
            ));
        }
    }
    out
}

fn number(row: &MetricChange, value: f32) -> String {
    format!("{value:.*} {}", row.decimals, row.unit)
}

impl Comparison {
    pub fn to_text(&self, before_id: &str, after_id: &str) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "Comparación: {before_id} → {after_id}\n");
        let _ = writeln!(
            out,
            "{:<34} {:>10} {:>10}   Cambio",
            "Medida", "Antes", "Después"
        );
        for row in &self.rows {
            let verdict = match row.change {
                Change::Better => "✓ mejor",
                Change::Worse => "✗ peor",
                Change::Same => "= igual",
            };
            let _ = writeln!(
                out,
                "{:<34} {:>10} {:>10}   {:+.*} {}  {verdict}",
                row.label,
                number(row, row.before),
                number(row, row.after),
                row.decimals,
                row.delta,
                row.unit
            );
        }
        let _ = writeln!(
            out,
            "\nResumen: {} mejoran · {} empeoran · {} sin cambio",
            self.better, self.worse, self.same
        );
        for warning in &self.warnings {
            let _ = writeln!(out, "Aviso: {warning}");
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_csv::parse;

    /// A recording of `seconds` with the given busiest-core and League-client load.
    fn analysis(id: &str, seconds: u32, core: f32, client: f32, gpu_util: Option<f32>) -> Analysis {
        let gpu = gpu_util
            .map(|u| format!(",{u},45,1500,2000,30.0"))
            .unwrap_or_default();
        let gpu_cols = if gpu_util.is_some() {
            ",gpu_util,gpu_temp_c,gpu_clock_mhz,gpu_max_clock_mhz,gpu_power_w"
        } else {
            ""
        };
        let mut csv = format!("# purifex-session v1 game=\"a.exe\" cores=12 started=2026-11-18\nt_s,cpu_total,cpu_max_core,dpc_interrupt,ram_used,game_cpu,game_mb,groups{gpu_cols}\n");
        for t in 1..=seconds {
            csv.push_str(&format!(
                "{t}.0,40.0,{core},1.0,50.0,13.0,900,league_client={client}{gpu}\n"
            ));
        }
        Analysis::of(id, parse(&csv).unwrap()).unwrap()
    }

    fn game(core: f32, client: f32) -> Analysis {
        analysis("a", 600, core, client, None)
    }

    fn change_of(c: &Comparison, key: &str) -> Change {
        c.rows.iter().find(|r| r.key == key).unwrap().change
    }

    #[test]
    fn a_clear_improvement_is_better_a_clear_loss_is_worse() {
        let c = compare(&game(88.0, 16.0), &game(60.0, 0.5));
        assert_eq!(change_of(&c, "core_p95"), Change::Better);
        assert_eq!(change_of(&c, "league_client"), Change::Better);
        let c = compare(&game(60.0, 0.5), &game(88.0, 16.0));
        assert_eq!(change_of(&c, "core_p95"), Change::Worse);
        assert_eq!(change_of(&c, "league_client"), Change::Worse);
    }

    #[test]
    fn a_difference_inside_the_noise_is_the_same_not_an_improvement() {
        let c = compare(&game(88.0, 16.0), &game(86.5, 15.5));
        assert_eq!(change_of(&c, "core_p95"), Change::Same);
        assert_eq!(change_of(&c, "league_client"), Change::Same);
        assert_eq!((c.better, c.worse), (0, 0));
        assert_eq!(c.same, c.rows.len());
    }

    #[test]
    fn the_counts_add_up_and_the_delta_is_after_minus_before() {
        let c = compare(&game(88.0, 16.0), &game(60.0, 16.0));
        assert_eq!(c.better + c.worse + c.same, c.rows.len());
        let row = c.rows.iter().find(|r| r.key == "core_p95").unwrap();
        assert_eq!((row.before, row.after, row.delta), (88.0, 60.0, -28.0));
    }

    #[test]
    fn the_league_client_row_only_appears_if_it_was_there() {
        let c = compare(&game(80.0, 0.0), &game(80.0, 0.0));
        assert!(c.rows.iter().all(|r| r.key != "league_client"));
        let c = compare(&game(80.0, 0.0), &game(80.0, 9.0));
        assert!(c.rows.iter().any(|r| r.key == "league_client"));
    }

    #[test]
    fn the_card_is_compared_only_when_both_have_it_and_its_clock_only_when_it_is_not_the_limit() {
        let none = compare(&analysis("a", 600, 80.0, 0.0, Some(40.0)), &game(80.0, 0.0));
        assert!(none.rows.iter().all(|r| !r.key.starts_with("gpu")));
        let both = compare(
            &analysis("a", 600, 80.0, 0.0, Some(40.0)),
            &analysis("b", 600, 80.0, 0.0, Some(45.0)),
        );
        assert!(
            both.rows.iter().any(|r| r.key == "gpu_temp")
                && both.rows.iter().any(|r| r.key == "gpu_clock")
        );
        let limited = compare(
            &analysis("a", 600, 80.0, 0.0, Some(99.0)),
            &analysis("b", 600, 80.0, 0.0, Some(45.0)),
        );
        assert!(limited.rows.iter().any(|r| r.key == "gpu_temp"));
        assert!(
            limited.rows.iter().all(|r| r.key != "gpu_clock"),
            "a card at the limit says nothing with its clock"
        );
    }

    #[test]
    fn a_comparison_that_may_not_be_fair_says_why() {
        let fair = compare(&game(80.0, 0.0), &game(80.0, 0.0));
        assert!(fair.warnings.is_empty(), "{:?}", fair.warnings);
        let short = compare(&analysis("a", 200, 80.0, 0.0, None), &game(80.0, 0.0));
        assert!(
            short.warnings.iter().any(|w| w.contains("poco fiable")),
            "{:?}",
            short.warnings
        );
        let uneven = compare(
            &analysis("a", 301, 80.0, 0.0, None),
            &analysis("b", 900, 80.0, 0.0, None),
        );
        assert!(
            uneven.warnings.iter().any(|w| w.contains("distinto")),
            "{:?}",
            uneven.warnings
        );
        let test = compare(&analysis("a", 5, 80.0, 0.0, None), &game(80.0, 0.0));
        assert!(
            test.warnings
                .iter()
                .any(|w| w.contains("no es una partida")),
            "{:?}",
            test.warnings
        );
    }

    #[test]
    fn the_text_is_a_table_with_the_verdict_of_each_row_and_a_summary() {
        let text = compare(&game(88.0, 16.0), &game(60.0, 0.5)).to_text("lol-1", "lol-2");
        for expected in [
            "lol-1 → lol-2",
            "Núcleo más cargado (p95)",
            "88.0 %",
            "60.0 %",
            "-28.0 %",
            "✓ mejor",
            "= igual",
            "Resumen:",
        ] {
            assert!(text.contains(expected), "{expected}:\n{text}");
        }
    }

    #[test]
    fn the_json_has_stable_names() {
        let value = serde_json::to_value(compare(&game(88.0, 16.0), &game(60.0, 0.5))).unwrap();
        assert_eq!(value["rows"][0]["key"], "core_p95");
        assert_eq!(value["rows"][0]["change"], "better");
        assert!(value["warnings"].is_array() && value["better"].is_number());
    }
}
