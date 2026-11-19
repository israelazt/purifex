//! Starts the uninstaller that an app registered, the way the Windows list of apps
//! does. It does not wait: the uninstaller shows its own window and the user
//! finishes it there.

use std::os::windows::process::CommandExt;
use std::process::Command;

use purifex_domain::apps::InstalledApp;
use purifex_domain::ports::Uninstaller;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub struct RegisteredUninstaller;

/// The command as the app registered it, if it is safe to hand to the shell: the
/// registry is writable by programs, so anything that chains commands is refused.
pub fn safe_command(app: &InstalledApp) -> Result<&str, String> {
    let command = app
        .uninstall
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .ok_or("esta app no registró cómo desinstalarla")?;
    if command
        .chars()
        .any(|c| matches!(c, '&' | '|' | '<' | '>' | '^' | '%' | '\n' | '\r'))
    {
        return Err(
            "el comando de desinstalación tiene caracteres que no se ejecutan desde aquí".into(),
        );
    }
    Ok(command)
}

impl Uninstaller for RegisteredUninstaller {
    fn launch(&self, app: &InstalledApp) -> Result<(), String> {
        let command = safe_command(app)?;
        Command::new("cmd")
            .raw_arg(format!("/C \"{command}\""))
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("no se pudo iniciar el desinstalador: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use purifex_domain::apps::AppKind;

    use super::*;

    fn app(uninstall: Option<&str>) -> InstalledApp {
        InstalledApp {
            id: "HKLM64:x".into(),
            name: "X".into(),
            publisher: None,
            version: None,
            install_location: None,
            uninstall: uninstall.map(String::from),
            quiet_uninstall: None,
            size_kb: None,
            kind: AppKind::Desktop,
            hidden: false,
            display_icon: None,
            observed: Default::default(),
        }
    }

    #[test]
    fn the_usual_commands_pass_and_chained_or_empty_ones_do_not() {
        for ok in [
            r#""C:\Program Files\X\unins000.exe""#,
            "MsiExec.exe /X{0A1B2C3D-0000-0000-0000-000000000000}",
            r#""C:\X\u.exe" /uninstall"#,
        ] {
            assert!(safe_command(&app(Some(ok))).is_ok(), "{ok}");
        }
        for bad in [
            "calc & del /q C:\\x",
            "a | b",
            "a > b",
            "%COMSPEC% /c x",
            "x ^& y",
            "",
            "   ",
        ] {
            assert!(safe_command(&app(Some(bad))).is_err(), "{bad:?}");
        }
        assert!(safe_command(&app(None)).is_err());
    }
}
