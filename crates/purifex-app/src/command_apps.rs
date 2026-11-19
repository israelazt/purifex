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
}

impl AppsCommand {
    pub fn parse<'a>(mut args: impl Iterator<Item = &'a str>) -> Result<Self, String> {
        let first = args.next();
        let (mut json, mut yes, mut words) = (false, false, Vec::new());
        let rest: Vec<&str> = match first {
            Some("--json") => {
                json = true;
                args.collect()
            }
            Some(word) => std::iter::once(word).chain(args).collect(),
            None => Vec::new(),
        };
        let (command, rest) = match rest.split_first() {
            Some((c, r)) if ["traces", "uninstall", "clean", "restore", "history"].contains(c) => {
                (Some(*c), r.to_vec())
            }
            _ => (None, rest),
        };
        for arg in rest {
            match arg {
                "--json" => json = true,
                "--yes" => yes = true,
                flag if flag.starts_with("--") => {
                    return Err(format!("opción desconocida: {flag}"))
                }
                word => words.push(word),
            }
        }
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
                let app = services.uninstall_app(&name)?;
                Ok(Outcome::Message(format!(
                    "Se abrió el desinstalador de {}. Cuando termine, busca los restos con: purifex apps clean \"{}\"",
                    app.name, app.name
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
}
