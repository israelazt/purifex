//! The orders about apps: list, look for traces, uninstall, clean, restore.

use crate::outcome::Outcome;
use crate::services::Services;

#[derive(Debug, Clone, PartialEq)]
pub enum AppsCommand {
    List {
        json: bool,
    },
    /// What an app leaves (touches nothing).
    Traces {
        name: String,
        json: bool,
    },
    /// Starts the app's own uninstaller.
    Uninstall {
        name: String,
        yes: bool,
    },
    /// Removes the traces of an app that was uninstalled (without `--yes`: only says which).
    Clean {
        name: String,
        yes: bool,
        json: bool,
    },
    Restore {
        seq: u64,
        yes: bool,
    },
    History {
        json: bool,
    },
    /// What starts with Windows (touches nothing).
    Startup {
        json: bool,
    },
    /// The logs of installations kept.
    Logs {
        json: bool,
    },
    /// The junk of Windows that could be cleaned (touches nothing).
    Junk {
        json: bool,
    },
    /// What the quarantine holds.
    Quarantine {
        json: bool,
    },
    /// Empties the quarantine for good (without `--yes`: only says how much).
    Purge {
        yes: bool,
        json: bool,
    },
}

/// The words that name the order: an initial `--json`, then the command if it is one of
/// ours. Returns whether `--json` came first, the command, and what is left.
fn split_command<'a>(
    mut args: impl Iterator<Item = &'a str>,
) -> (bool, Option<&'a str>, Vec<&'a str>) {
    let (json, rest): (bool, Vec<&str>) = match args.next() {
        Some("--json") => (true, args.collect()),
        Some(word) => (false, std::iter::once(word).chain(args).collect()),
        None => (false, Vec::new()),
    };
    match rest.split_first() {
        Some((c, r))
            if [
                "traces",
                "uninstall",
                "clean",
                "restore",
                "history",
                "quarantine",
                "purge",
                "startup",
                "logs",
                "junk",
            ]
            .contains(c) =>
        {
            (json, Some(*c), r.to_vec())
        }
        _ => (json, None, rest),
    }
}

/// Separates the flags (`--json`, `--yes`) from the words; any other `--` is an error.
fn read_flags(words: Vec<&str>) -> Result<(bool, bool, Vec<&str>), String> {
    let (mut json, mut yes, mut rest) = (false, false, Vec::new());
    for arg in words {
        match arg {
            "--json" => json = true,
            "--yes" => yes = true,
            flag if flag.starts_with("--") => return Err(format!("opción desconocida: {flag}")),
            word => rest.push(word),
        }
    }
    Ok((json, yes, rest))
}

impl AppsCommand {
    pub fn parse<'a>(args: impl Iterator<Item = &'a str>) -> Result<Self, String> {
        let (json_first, command, rest) = split_command(args);
        let (json_later, yes, words) = read_flags(rest)?;
        Self::build(command, json_first || json_later, yes, &words)
    }

    /// The order from its command (`traces`, `clean`...), the flags and the name of the app.
    fn build(command: Option<&str>, json: bool, yes: bool, words: &[&str]) -> Result<Self, String> {
        let name = words.join(" ");
        let need_name = || {
            if name.trim().is_empty() {
                Err("falta el nombre de la app (mira `purifex apps`)".to_string())
            } else {
                Ok(name.clone())
            }
        };
        match command {
            None if words.is_empty() => Ok(Self::List { json }),
            None => Err(format!("orden desconocida: apps {}", words[0])),
            Some("traces") => Ok(Self::Traces {
                name: need_name()?,
                json,
            }),
            Some("uninstall") => Ok(Self::Uninstall {
                name: need_name()?,
                yes,
            }),
            Some("clean") => Ok(Self::Clean {
                name: need_name()?,
                yes,
                json,
            }),
            Some("history") => Ok(Self::History { json }),
            Some("startup") => Ok(Self::Startup { json }),
            Some("logs") => Ok(Self::Logs { json }),
            Some("junk") => Ok(Self::Junk { json }),
            Some("quarantine") => Ok(Self::Quarantine { json }),
            Some("purge") => Ok(Self::Purge { yes, json }),
            Some(_) => {
                let seq = name.trim().trim_start_matches('#').parse().map_err(|_| {
                    "falta el número de la eliminación (mira `purifex apps history`)".to_string()
                })?;
                Ok(Self::Restore { seq, yes })
            }
        }
    }

    pub fn wants_json(&self) -> bool {
        matches!(
            self,
            Self::List { json: true }
                | Self::Traces { json: true, .. }
                | Self::Clean { json: true, .. }
                | Self::History { json: true }
                | Self::Startup { json: true }
                | Self::Logs { json: true }
                | Self::Junk { json: true }
                | Self::Quarantine { json: true }
                | Self::Purge { json: true, .. }
        )
    }

    pub fn execute(self, services: &mut dyn Services) -> Result<Outcome, String> {
        match self {
            Self::List { .. } => services.apps().map(Outcome::Apps),
            Self::Traces { name, .. } => services
                .clean_app(&name, false, None)
                .map(|r| Outcome::Clean(Box::new(r))),
            Self::Clean { name, yes, .. } => services
                .clean_app(&name, yes, None)
                .map(|r| Outcome::Clean(Box::new(r))),
            Self::Uninstall { name, yes } => {
                if !yes {
                    return Ok(Outcome::Message(format!(
                        "Se abriría el desinstalador oficial de «{name}». Para hacerlo: purifex apps uninstall \"{name}\" --yes"
                    )));
                }
                let started = services.uninstall_app(&name)?;
                let safeguard = format!(" {}", started.safeguards_text());
                Ok(Outcome::Message(format!(
                    "Se abrió el desinstalador de {}. Cuando termine, busca los restos con: purifex apps clean \"{}\".{safeguard}",
                    started.app.name, started.app.name
                )))
            }
            Self::Restore { seq, yes } => {
                if !yes {
                    return Ok(Outcome::Message(format!("Se devolvería la eliminación #{seq}. Para hacerlo: purifex apps restore {seq} --yes")));
                }
                let line = services.restore_cleanup(seq)?;
                Ok(Outcome::Message(format!(
                    "Devuelto: {}",
                    line.trace.location
                )))
            }
            Self::History { .. } => services.cleanup_history().map(Outcome::CleanupLog),
            Self::Startup { .. } => services.startup_items().map(Outcome::Startup),
            Self::Logs { .. } => services.install_logs().map(Outcome::Logs),
            Self::Junk { .. } => services.junk_scan().map(Outcome::Junk),
            Self::Quarantine { .. } => {
                services.expire_quarantine()?;
                services.quarantine().map(Outcome::Quarantine)
            }
            Self::Purge { yes, .. } => services.empty_quarantine(yes).map(Outcome::Purge),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<AppsCommand, String> {
        AppsCommand::parse(args.iter().copied())
    }

    #[test]
    fn the_orders_are_read_with_names_of_several_words_and_flags_anywhere() {
        assert_eq!(parse(&[]).unwrap(), AppsCommand::List { json: false });
        assert_eq!(
            parse(&["--json"]).unwrap(),
            AppsCommand::List { json: true }
        );
        assert_eq!(
            parse(&["traces", "Google", "Chrome"]).unwrap(),
            AppsCommand::Traces {
                name: "Google Chrome".into(),
                json: false
            }
        );
        assert_eq!(
            parse(&["uninstall", "Discord", "--yes"]).unwrap(),
            AppsCommand::Uninstall {
                name: "Discord".into(),
                yes: true
            }
        );
        assert_eq!(
            parse(&["clean", "--yes", "Discord"]).unwrap(),
            AppsCommand::Clean {
                name: "Discord".into(),
                yes: true,
                json: false
            }
        );
        assert_eq!(
            parse(&["restore", "#3", "--yes"]).unwrap(),
            AppsCommand::Restore { seq: 3, yes: true }
        );
        assert_eq!(
            parse(&["history"]).unwrap(),
            AppsCommand::History { json: false }
        );
    }

    #[test]
    fn the_quarantine_can_be_looked_at_and_emptied_only_with_yes() {
        assert_eq!(
            parse(&["quarantine", "--json"]).unwrap(),
            AppsCommand::Quarantine { json: true }
        );
        assert_eq!(
            parse(&["purge"]).unwrap(),
            AppsCommand::Purge {
                yes: false,
                json: false
            }
        );
        assert_eq!(
            parse(&["--json", "purge", "--yes"]).unwrap(),
            AppsCommand::Purge {
                yes: true,
                json: true
            }
        );
        assert!(parse(&["purge", "--force"]).is_err());
        assert!(AppsCommand::Purge {
            yes: false,
            json: true
        }
        .wants_json());
    }

    #[test]
    fn a_missing_name_a_bad_number_or_an_unknown_word_is_an_error() {
        assert!(parse(&["uninstall"])
            .unwrap_err()
            .contains("falta el nombre"));
        assert!(parse(&["restore", "x"]).unwrap_err().contains("número"));
        assert!(parse(&["clean", "X", "--force"])
            .unwrap_err()
            .contains("opción desconocida"));
        assert!(parse(&["borrar"])
            .unwrap_err()
            .contains("orden desconocida"));
    }

    #[test]
    fn the_read_only_lists_are_parsed_with_json_and_say_so() {
        assert_eq!(
            parse(&["startup"]).unwrap(),
            AppsCommand::Startup { json: false }
        );
        assert_eq!(
            parse(&["--json", "logs"]).unwrap(),
            AppsCommand::Logs { json: true }
        );
        assert_eq!(
            parse(&["junk", "--json"]).unwrap(),
            AppsCommand::Junk { json: true }
        );
        assert!(AppsCommand::Startup { json: true }.wants_json());
        assert!(!AppsCommand::Junk { json: false }.wants_json());
    }
}
