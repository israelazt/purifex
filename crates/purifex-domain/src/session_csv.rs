//! Reads a recording back: the inverse of `Sample::csv_row`. Columns are found by
//! their **name**, not their place, so a file written by an older version (the
//! first recordings have neither the header line nor the `groups` column) reads
//! too, and a column added later by a newer one is ignored.

use std::str::FromStr;

use crate::process_group::{group_of, Group};
use crate::sample::{GameLoad, GpuSample, GroupLoad, ProcLoad, Sample};
use crate::session_header::SessionHeader;

/// A recording read back.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedSession {
    /// `None` for a recording made before there were headers.
    pub header: Option<SessionHeader>,
    pub samples: Vec<Sample>,
}

/// The columns without which a line says nothing.
const REQUIRED: [&str; 3] = ["t_s", "cpu_total", "cpu_max_core"];

/// Reads the text of a CSV file. A last line cut short (the program was closed
/// while writing it) is ignored; a bad line anywhere else is an error that says
/// which one.
pub fn parse(text: &str) -> Result<ParsedSession, String> {
    let mut lines = text
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim_end_matches('\r')))
        .filter(|(_, l)| !l.trim().is_empty())
        .peekable();

    let header = match lines.peek() {
        Some((_, first)) if first.starts_with('#') => {
            let header = SessionHeader::parse(first);
            lines.next();
            header
        }
        _ => None,
    };
    let (_, columns_line) = lines.next().ok_or("el archivo está vacío")?;
    let columns: Vec<&str> = columns_line.split(',').map(str::trim).collect();
    for name in REQUIRED {
        if !columns.contains(&name) {
            return Err(format!(
                "falta la columna «{name}»: no parece una grabación de Purifex"
            ));
        }
    }

    let rows: Vec<(usize, &str)> = lines.collect();
    let mut samples = Vec::with_capacity(rows.len());
    for (index, (number, line)) in rows.iter().enumerate() {
        let fields: Vec<&str> = line.split(',').collect();
        if fields.len() != columns.len() {
            if index + 1 == rows.len() {
                continue;
            }
            return Err(format!(
                "línea {number}: tiene {} campos y la cabecera {}",
                fields.len(),
                columns.len()
            ));
        }
        let row = Row {
            columns: &columns,
            fields: &fields,
            number: *number,
        };
        samples.push(row.sample()?);
    }
    Ok(ParsedSession { header, samples })
}

/// One line with its column names, to ask for fields by name.
struct Row<'a> {
    columns: &'a [&'a str],
    fields: &'a [&'a str],
    number: usize,
}

impl Row<'_> {
    /// The text of a column, if it exists and has something.
    fn text(&self, name: &str) -> Option<&str> {
        let at = self.columns.iter().position(|c| *c == name)?;
        let value = self.fields[at].trim();
        (!value.is_empty()).then_some(value)
    }

    fn num<T: FromStr>(&self, name: &str) -> Result<Option<T>, String> {
        self.text(name)
            .map(|value| {
                value.parse().map_err(|_| {
                    format!(
                        "línea {}: «{value}» no es un valor válido en «{name}»",
                        self.number
                    )
                })
            })
            .transpose()
    }

    fn need<T: FromStr>(&self, name: &str) -> Result<T, String> {
        self.num(name)?
            .ok_or_else(|| format!("línea {}: falta el valor de «{name}»", self.number))
    }

    fn or_zero<T: FromStr + Default>(&self, name: &str) -> Result<T, String> {
        Ok(self.num(name)?.unwrap_or_default())
    }

    fn sample(&self) -> Result<Sample, String> {
        let seconds: f64 = self.need("t_s")?;
        let game = match (self.num("game_cpu")?, self.num("game_mb")?) {
            (Some(cpu), Some(memory_mb)) => Some(GameLoad { cpu, memory_mb }),
            _ => None,
        };
        let gpu = match self.num("gpu_util")? {
            Some(util) => Some(GpuSample {
                util,
                memory_util: self.or_zero("gpu_mem_util")?,
                temp_c: self.or_zero("gpu_temp_c")?,
                clock_mhz: self.or_zero("gpu_clock_mhz")?,
                max_clock_mhz: self.or_zero("gpu_max_clock_mhz")?,
                power_w: self.or_zero("gpu_power_w")?,
                throttle: self.throttle()?,
            }),
            None => None,
        };
        let top: Vec<ProcLoad> = self
            .pairs("top")?
            .into_iter()
            .map(|(name, cpu)| ProcLoad { name, cpu })
            .collect();
        Ok(Sample {
            at_ms: (seconds * 1000.0).round().max(0.0) as u64,
            cpu_total: self.need("cpu_total")?,
            cpu_max_core: self.need("cpu_max_core")?,
            dpc_interrupt: self.or_zero("dpc_interrupt")?,
            cpu_mhz: self.or_zero("cpu_mhz")?,
            cpu_max_mhz: self.or_zero("cpu_max_mhz")?,
            ram_used: self.or_zero("ram_used")?,
            game,
            gpu,
            groups: self.groups(&top)?,
            top,
            cost_us: self.or_zero("cost_us")?,
        })
    }

    /// The families of programs. A recording made before the `groups` column
    /// existed has none, so they are rebuilt from the busiest processes it did
    /// list (a lower bound: a process outside its top five is not there).
    fn groups(&self, top: &[ProcLoad]) -> Result<Vec<GroupLoad>, String> {
        if self.columns.contains(&"groups") {
            return Ok(self
                .pairs("groups")?
                .into_iter()
                .filter_map(|(key, cpu)| {
                    Group::from_key(&key).map(|group| GroupLoad { group, cpu })
                })
                .collect());
        }
        let mut sums: Vec<GroupLoad> = Vec::new();
        for process in top {
            let Some(group) = group_of(&process.name) else {
                continue;
            };
            match sums.iter_mut().find(|g| g.group == group) {
                Some(found) => found.cpu += process.cpu,
                None => sums.push(GroupLoad {
                    group,
                    cpu: process.cpu,
                }),
            }
        }
        Ok(sums)
    }

    /// The throttling bits, written as `0x4` (or as a plain number).
    fn throttle(&self) -> Result<u64, String> {
        let Some(text) = self.text("gpu_throttle") else {
            return Ok(0);
        };
        let parsed = match text.strip_prefix("0x") {
            Some(hex) => u64::from_str_radix(hex, 16),
            None => text.parse(),
        };
        parsed.map_err(|_| {
            format!(
                "línea {}: «{text}» no es un valor válido en «gpu_throttle»",
                self.number
            )
        })
    }

    /// `name=1.5;other=2.0` as pairs. The name is what is before the last `=`.
    fn pairs(&self, name: &str) -> Result<Vec<(String, f32)>, String> {
        let Some(text) = self.text(name) else {
            return Ok(Vec::new());
        };
        text.split(';')
            .filter(|item| !item.is_empty())
            .map(|item| {
                item.rsplit_once('=')
                    .and_then(|(key, value)| Some((key.to_string(), value.parse().ok()?)))
                    .ok_or_else(|| {
                        format!(
                            "línea {}: «{item}» no es un valor válido en «{name}»",
                            self.number
                        )
                    })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::Date;
    use crate::sample::{throttle, CSV_HEADER};

    /// Values with the decimals the file keeps, so a round trip is exact.
    fn sample(at_ms: u64, core: f32) -> Sample {
        Sample {
            at_ms,
            cpu_total: 41.3,
            cpu_max_core: core,
            dpc_interrupt: 1.5,
            cpu_mhz: 4070,
            cpu_max_mhz: 3801,
            ram_used: 63.0,
            game: Some(GameLoad {
                cpu: 12.5,
                memory_mb: 1800,
            }),
            gpu: Some(GpuSample {
                util: 88.0,
                memory_util: 40.0,
                temp_c: 71,
                clock_mhz: 1800,
                max_clock_mhz: 2100,
                power_w: 95.2,
                throttle: throttle::POWER_CAP | throttle::SW_THERMAL,
            }),
            groups: vec![GroupLoad {
                group: Group::LeagueClient,
                cpu: 15.9,
            }],
            top: vec![
                ProcLoad {
                    name: "LeagueClientUxRender.exe".into(),
                    cpu: 9.2,
                },
                ProcLoad {
                    name: "msedge.exe".into(),
                    cpu: 3.0,
                },
            ],
            cost_us: 900,
        }
    }

    fn file(samples: &[Sample]) -> String {
        let header = SessionHeader::new(
            "League of Legends.exe",
            12,
            Date::new(2026, 11, 18).unwrap(),
        );
        let mut text = format!("{}\n{CSV_HEADER}\n", header.to_line());
        for s in samples {
            text.push_str(&s.csv_row());
            text.push('\n');
        }
        text
    }

    #[test]
    fn what_csv_row_writes_reads_back_the_same() {
        let written = [sample(1000, 97.0), sample(2500, 55.5)];
        let parsed = parse(&file(&written)).unwrap();
        assert_eq!(parsed.samples, written);
        assert_eq!(parsed.header.unwrap().cores, 12);
    }

    #[test]
    fn a_sample_without_game_or_gpu_reads_back_without_them() {
        let mut bare = sample(1000, 50.0);
        (bare.game, bare.gpu, bare.groups, bare.top) = (None, None, vec![], vec![]);
        assert_eq!(parse(&file(&[bare.clone()])).unwrap().samples, [bare]);
    }

    /// The first two lines of the first real recording (2026-11-18): no header
    /// line and no `groups` column.
    const FIRST_RECORDING: &str = "t_s,cpu_total,cpu_max_core,dpc_interrupt,cpu_mhz,cpu_max_mhz,ram_used,game_cpu,game_mb,gpu_util,gpu_mem_util,gpu_temp_c,gpu_clock_mhz,gpu_max_clock_mhz,gpu_power_w,gpu_throttle,top,cost_us\n\
1.0,43.2,65.2,1.0,3801,3801,45.0,8.6,62,23,17,38,705,2160,15.5,0x1,LeagueClientUxRender.exe=9.2;LeagueClient.exe=5.5;System=4.9,8669\n\
2.0,33.9,56.2,0.8,3801,3801,46.0,5.2,72,12,11,38,555,2160,14.4,0x1,LeagueClientUxRender.exe=6.9;msedge.exe=4.3,9998\n";

    #[test]
    fn the_first_recordings_without_header_or_groups_still_read() {
        let parsed = parse(FIRST_RECORDING).unwrap();
        assert_eq!(parsed.header, None);
        assert_eq!(parsed.samples.len(), 2);
        let first = &parsed.samples[0];
        assert_eq!(first.at_ms, 1000);
        assert_eq!(first.gpu.unwrap().throttle, 1);
        assert_eq!(first.game.unwrap().memory_mb, 62);
        // Rebuilt from the processes it listed: the League client is two of them here.
        let client = first
            .groups
            .iter()
            .find(|g| g.group == Group::LeagueClient)
            .unwrap();
        assert!((client.cpu - 14.7).abs() < 0.01, "{}", client.cpu);
        assert!(first.groups.iter().any(|g| g.group == Group::System));
        assert_eq!(
            first.top[1],
            ProcLoad {
                name: "LeagueClient.exe".into(),
                cpu: 5.5
            }
        );
    }

    #[test]
    fn a_last_line_cut_short_is_ignored_but_a_bad_one_in_the_middle_is_an_error() {
        let good = file(&[sample(1000, 50.0), sample(2000, 60.0)]);
        let cut = format!("{good}3.0,41.3,9");
        assert_eq!(parse(&cut).unwrap().samples.len(), 2);

        // The first sample is line 3 (the header and the columns come before).
        let middle = good.replacen(
            "
1.0,",
            "
1.0,extra,",
            1,
        );
        let error = parse(&middle).unwrap_err();
        assert!(error.contains("línea 3"), "{error}");
    }

    #[test]
    fn a_value_that_is_not_a_number_says_where() {
        let text = file(&[sample(1000, 50.0)]).replace("41.3", "mucho");
        let error = parse(&text).unwrap_err();
        assert!(
            error.contains("mucho") && error.contains("cpu_total") && error.contains("línea 3"),
            "{error}"
        );
    }

    #[test]
    fn a_file_that_is_not_a_recording_is_refused_with_a_reason() {
        assert!(parse("").unwrap_err().contains("vacío"));
        assert!(parse("a,b,c\n1,2,3\n").unwrap_err().contains("t_s"));
        assert!(
            parse("# purifex-session v1 game=\"x\" cores=1 started=2026-11-18\n")
                .unwrap_err()
                .contains("vacío")
        );
    }

    #[test]
    fn columns_of_a_newer_version_are_ignored_and_windows_line_ends_read() {
        let text = "# purifex-session v2 game=\"a.exe\" cores=4 started=2026-11-18\r\nt_s,cpu_total,cpu_max_core,vram\r\n1.0,10.0,20.0,999\r\n";
        let parsed = parse(text).unwrap();
        assert_eq!(parsed.header.unwrap().version, 2);
        assert_eq!(parsed.samples[0].cpu_max_core, 20.0);
        assert_eq!(parsed.samples[0].game, None);
    }

    #[test]
    fn an_unknown_family_is_skipped_not_an_error() {
        let text =
            "t_s,cpu_total,cpu_max_core,groups\n1.0,1.0,1.0,league_client=2.0;future_family=3.0\n";
        let samples = parse(text).unwrap().samples;
        assert_eq!(
            samples[0].groups,
            [GroupLoad {
                group: Group::LeagueClient,
                cpu: 2.0
            }]
        );
    }
}
