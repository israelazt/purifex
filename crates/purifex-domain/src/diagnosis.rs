//! What a session suggests. These are **leads with their evidence**, not
//! verdicts: without the frame times there is no way to say how many frames
//! were lost, only what limited the computer while it played.

use std::fmt::Write;

use crate::process_group::{group_of, Group};
use crate::session::SessionSummary;

/// Thresholds. Each one is a judgment call, so they live in one place.
mod limit {
    /// Graphics card use from which it is the limit.
    pub const GPU_BOUND: f32 = 95.0;
    /// Below this the card has room, so a pinned core is the limit.
    pub const GPU_HAS_ROOM: f32 = 85.0;
    /// Share of the seconds with a pinned core that matters.
    pub const PINNED_SHARE: f32 = 10.0;
    pub const HEAT_SHARE: f32 = 5.0;
    pub const HOT_GPU_C: f32 = 83.0;
    /// Time in interrupts/DPC (percent of the CPU) that points at a driver.
    pub const DPC_P95: f32 = 5.0;
    /// Clock below this share of its maximum, while loaded, is held back.
    pub const LOW_CLOCK: f32 = 70.0;
    pub const RAM_P95: f32 = 90.0;
    /// A background process with this share of the CPU on average.
    pub const BACKGROUND: f32 = 3.0;
    /// The League client, running beside the game, with this share of the CPU.
    pub const LEAGUE_CLIENT: f32 = 5.0;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub severity: Severity,
    pub title: String,
    pub evidence: String,
}

fn finding(severity: Severity, title: &str, evidence: String) -> Finding {
    Finding {
        severity,
        title: title.into(),
        evidence,
    }
}

/// The leads of a session, most serious first.
pub fn diagnose(s: &SessionSummary) -> Vec<Finding> {
    let mut out = Vec::new();
    let gpu_util = s.gpu.as_ref().map(|g| g.util);

    if let Some(g) = &s.gpu {
        if g.util.p95 >= limit::GPU_BOUND {
            out.push(finding(
                Severity::Warning,
                "La tarjeta gráfica va al límite",
                format!("uso de GPU p95 {:.0} %, media {:.0} %: bajar la calidad gráfica o la resolución sube los FPS", g.util.p95, g.util.avg),
            ));
        }
        if g.heat_throttled_pct >= limit::HEAT_SHARE || g.temp_c.max >= limit::HOT_GPU_C {
            out.push(finding(
                Severity::Warning,
                "La GPU se frena por calor",
                format!("frenada por temperatura el {:.0} % del tiempo, máx. {:.0} °C: revisar ventilación y polvo", g.heat_throttled_pct, g.temp_c.max),
            ));
        }
    }

    let gpu_has_room = gpu_util.is_none_or(|u| u.avg < limit::GPU_HAS_ROOM);
    if s.core_pinned_pct >= limit::PINNED_SHARE && gpu_has_room {
        out.push(finding(
            Severity::Warning,
            "Un núcleo del CPU está al tope (el juego espera al procesador)",
            format!("algún núcleo al 95 % o más el {:.0} % del tiempo, núcleo más cargado p95 {:.0} %, y la GPU tiene margen", s.core_pinned_pct, s.cpu_max_core.p95),
        ));
    }
    if s.cpu_clock_pct.avg > 0.0
        && s.cpu_clock_pct.avg < limit::LOW_CLOCK
        && s.cpu_max_core.p95 >= 80.0
    {
        out.push(finding(
            Severity::Warning,
            "El CPU no sube de frecuencia bajo carga",
            format!("trabaja al {:.0} % de su frecuencia máxima de media: plan de energía, ahorro de batería o temperatura", s.cpu_clock_pct.avg),
        ));
    }
    if s.dpc_interrupt.p95 >= limit::DPC_P95 {
        out.push(finding(
            Severity::Warning,
            "Mucho tiempo en interrupciones/DPC (síntoma típico de un driver)",
            format!("p95 {:.1} %, máx. {:.1} %: da tirones aunque los FPS medios sean buenos; candidatos: red, audio, GPU", s.dpc_interrupt.p95, s.dpc_interrupt.max),
        ));
    }
    if s.ram_used.p95 >= limit::RAM_P95 {
        out.push(finding(
            Severity::Warning,
            "Poca memoria libre",
            format!(
                "RAM en uso p95 {:.0} %: el sistema puede estar usando el disco como memoria",
                s.ram_used.p95
            ),
        ));
    }
    let client = s.groups.iter().find(|g| g.group == Group::LeagueClient);
    if let Some(client) = client.filter(|g| g.cpu.avg >= limit::LEAGUE_CLIENT) {
        out.push(finding(
            Severity::Warning,
            "El cliente de League gasta CPU mientras juegas",
            format!(
                "el cliente usa de media un {:.1} % del CPU (p95 {:.1} %) y el juego un {:.1} %: cerrarlo durante la partida lo libera",
                client.cpu.avg, client.cpu.p95, s.game_cpu.avg
            ),
        ));
    }
    let busy: Vec<_> = s
        .background
        .iter()
        // The League client has its own lead above.
        .filter(|b| {
            b.avg_cpu >= limit::BACKGROUND && group_of(&b.name) != Some(Group::LeagueClient)
        })
        .collect();
    if !busy.is_empty() {
        let list: Vec<String> = busy
            .iter()
            .map(|b| format!("{} {:.1} %", b.name, b.avg_cpu))
            .collect();
        out.push(finding(
            Severity::Info,
            "Programas en segundo plano que consumen CPU",
            format!("de media: {}", list.join(", ")),
        ));
    }
    out.sort_by_key(|f| std::cmp::Reverse(f.severity));
    out
}

impl SessionSummary {
    /// The summary and its leads, as text for a terminal.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        let line = |out: &mut String, name: &str, s: &crate::session::Stat, unit: &str| {
            let _ = writeln!(
                out,
                "  {name:<26} media {:>6.1}{unit}  p95 {:>6.1}{unit}  máx {:>6.1}{unit}",
                s.avg, s.p95, s.max
            );
        };
        let _ = writeln!(
            out,
            "Sesión de {} s ({} muestras)\n",
            self.seconds, self.samples
        );
        line(&mut out, "CPU total", &self.cpu_total, " %");
        line(&mut out, "Núcleo más cargado", &self.cpu_max_core, " %");
        let _ = writeln!(
            out,
            "  Segundos con un núcleo >= 95 %: {:.0} %",
            self.core_pinned_pct
        );
        for g in &self.groups {
            let name = format!("CPU {}", g.group.label());
            line(&mut out, &name, &g.cpu, " %");
        }
        line(&mut out, "Interrupciones/DPC", &self.dpc_interrupt, " %");
        line(
            &mut out,
            "Frecuencia (de la base)",
            &self.cpu_clock_pct,
            " %",
        );
        line(&mut out, "RAM en uso", &self.ram_used, " %");
        if self.game_cpu.max > 0.0 {
            line(&mut out, "El juego: CPU", &self.game_cpu, " %");
            line(&mut out, "El juego: memoria", &self.game_memory_mb, " MB");
        }
        match &self.gpu {
            Some(g) => {
                line(&mut out, "GPU: uso", &g.util, " %");
                line(&mut out, "GPU: temperatura", &g.temp_c, " °C");
                line(&mut out, "GPU: consumo", &g.power_w, " W");
                line(&mut out, "GPU: reloj (de la máx.)", &g.clock_pct, " %");
                let _ = writeln!(
                    out,
                    "  GPU frenada por calor {:.0} % · por límite de consumo {:.0} %",
                    g.heat_throttled_pct, g.power_capped_pct
                );
            }
            None => {
                let _ = writeln!(
                    out,
                    "  GPU: sin datos (solo se leen tarjetas NVIDIA por ahora)"
                );
            }
        }
        let _ = writeln!(
            out,
            "  Coste de medir: {:.0} µs por muestra (máx. {:.0})\n",
            self.sample_cost_us.avg, self.sample_cost_us.max
        );
        let findings = diagnose(self);
        if findings.is_empty() {
            let _ = writeln!(out, "Nada destacable en esta sesión: ni CPU, ni GPU, ni drivers, ni calor la limitaron.");
        } else {
            let _ = writeln!(out, "Pistas:");
            for f in findings {
                let _ = writeln!(out, " - {}\n     {}", f.title, f.evidence);
            }
        }
        out.push_str("\nNota: sin los tiempos de fotograma no se sabe cuántos FPS se perdieron, solo qué limitaba al equipo.\n");
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample::throttle;
    use crate::session::{Background, GpuSummary, Stat};

    fn flat(v: f32) -> Stat {
        Stat {
            avg: v,
            p95: v,
            max: v,
        }
    }

    fn calm() -> SessionSummary {
        SessionSummary {
            seconds: 100,
            samples: 100,
            cpu_total: flat(30.0),
            cpu_max_core: flat(50.0),
            dpc_interrupt: flat(1.0),
            cpu_clock_pct: flat(95.0),
            ram_used: flat(60.0),
            gpu: Some(GpuSummary {
                util: flat(60.0),
                temp_c: flat(65.0),
                clock_pct: flat(95.0),
                ..GpuSummary::default()
            }),
            ..SessionSummary::default()
        }
    }

    fn titles(s: &SessionSummary) -> Vec<String> {
        diagnose(s).into_iter().map(|f| f.title).collect()
    }

    #[test]
    fn a_calm_session_has_no_leads() {
        assert!(diagnose(&calm()).is_empty());
        assert!(calm().to_text().contains("Nada destacable"));
    }

    #[test]
    fn a_pinned_core_with_a_free_gpu_points_at_the_cpu() {
        let mut s = calm();
        s.core_pinned_pct = 40.0;
        s.cpu_max_core = flat(98.0);
        assert!(titles(&s)[0].contains("núcleo"));
        // If the GPU is the busy one, it is not the CPU.
        s.gpu.as_mut().unwrap().util = flat(99.0);
        let t = titles(&s);
        assert!(t.iter().any(|x| x.contains("límite")));
        assert!(!t.iter().any(|x| x.contains("núcleo")));
    }

    #[test]
    fn heat_and_power_are_told_apart() {
        let mut s = calm();
        s.gpu.as_mut().unwrap().power_capped_pct = 90.0;
        assert!(
            diagnose(&s).is_empty(),
            "a power limit alone is normal under load"
        );
        s.gpu.as_mut().unwrap().heat_throttled_pct = 20.0;
        assert!(titles(&s)[0].contains("calor"));
        let _ = throttle::HEAT;
    }

    #[test]
    fn drivers_ram_clock_and_background_each_have_their_lead() {
        let mut s = calm();
        s.dpc_interrupt = flat(9.0);
        s.ram_used = flat(95.0);
        s.cpu_clock_pct = flat(50.0);
        s.cpu_max_core = flat(90.0);
        s.background = vec![Background {
            name: "chrome.exe".into(),
            avg_cpu: 6.0,
        }];
        let t = titles(&s).join("|");
        for word in ["DPC", "memoria", "frecuencia", "segundo plano"] {
            assert!(t.contains(word), "{word} in {t}");
        }
        assert_eq!(
            diagnose(&s).last().unwrap().severity,
            Severity::Info,
            "info goes last"
        );
    }

    #[test]
    fn the_league_client_has_its_own_lead_and_is_not_repeated_as_background() {
        use crate::session::{Background, GroupSummary};
        let mut s = calm();
        s.game_cpu = flat(13.5);
        s.groups = vec![GroupSummary {
            group: Group::LeagueClient,
            cpu: flat(15.9),
        }];
        s.background = vec![
            Background {
                name: "LeagueClientUxRender.exe".into(),
                avg_cpu: 8.3,
            },
            Background {
                name: "msedge.exe".into(),
                avg_cpu: 4.0,
            },
        ];
        let found = diagnose(&s);
        assert!(found[0].title.contains("cliente de League"));
        assert!(found[0].evidence.contains("15.9") && found[0].evidence.contains("13.5"));
        let background = found
            .iter()
            .find(|f| f.title.contains("segundo plano"))
            .unwrap();
        assert!(background.evidence.contains("msedge.exe"));
        assert!(!background.evidence.contains("LeagueClient"));
        assert!(s.to_text().contains("CPU Cliente de League"));
        // A client that barely uses CPU is not a lead.
        s.groups[0].cpu = flat(1.0);
        assert!(!titles(&s).iter().any(|t| t.contains("cliente de League")));
    }

    #[test]
    fn without_a_gpu_reading_it_says_so() {
        let mut s = calm();
        s.gpu = None;
        assert!(s.to_text().contains("sin datos"));
    }
}
