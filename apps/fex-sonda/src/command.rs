//! What the tool was asked to do, from its arguments and the name it was started as.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Install,
    Uninstall,
    Status { json: bool },
    Purge,
    Run,
}

/// The command in `args` (without the program name). With none, a program called `uninstall.exe`
/// uninstalls, which is how Windows starts the uninstaller of an app.
pub fn parse(args: &[String], started_as: &str) -> Result<Command, String> {
    let json = args.iter().any(|a| a == "--json");
    let word = args.iter().find(|a| !a.starts_with("--"));
    match word.map(String::as_str) {
        Some("install") => Ok(Command::Install),
        Some("uninstall") => Ok(Command::Uninstall),
        Some("status") => Ok(Command::Status { json }),
        Some("purge") => Ok(Command::Purge),
        Some("run") => Ok(Command::Run),
        Some(other) => Err(format!(
            "orden desconocida «{other}»: install, uninstall, status, purge o run"
        )),
        None if started_as.eq_ignore_ascii_case("uninstall.exe") => Ok(Command::Uninstall),
        None => Err("falta la orden: install, uninstall, status, purge o run".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn each_word_is_its_command_and_status_can_ask_for_json() {
        assert_eq!(
            parse(&args(&["install"]), "fex-sonda.exe"),
            Ok(Command::Install)
        );
        assert_eq!(
            parse(&args(&["status", "--json"]), "fex-sonda.exe"),
            Ok(Command::Status { json: true })
        );
        assert_eq!(
            parse(&args(&["status"]), "fex-sonda.exe"),
            Ok(Command::Status { json: false })
        );
    }

    #[test]
    fn started_as_uninstall_exe_with_no_word_it_uninstalls() {
        assert_eq!(parse(&[], "Uninstall.exe"), Ok(Command::Uninstall));
        assert!(parse(&[], "fex-sonda.exe").is_err());
    }

    #[test]
    fn an_unknown_word_is_refused_with_the_list_of_the_known_ones() {
        let error = parse(&args(&["borrar"]), "fex-sonda.exe").unwrap_err();
        assert!(error.contains("borrar") && error.contains("purge"));
    }
}
