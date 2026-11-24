//! What this tool asks of Windows: the registry through `reg.exe`, tasks through `schtasks.exe` and
//! shortcuts through PowerShell. Arguments always go as a list, never inside a shell text.

// The deletions are used from `uninstall` (T.3) on: remove this then.
#![allow(dead_code)]

use std::process::{Command, Output};

use crate::manifest::{Data, Hive};

fn exec(program: &str, args: &[&str]) -> Result<Output, String> {
    Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("no se pudo ejecutar {program}: {e}"))
}

/// Runs a program and fails with its own words when it ends with an error.
fn must(program: &str, args: &[&str]) -> Result<(), String> {
    let out = exec(program, args)?;
    if out.status.success() {
        return Ok(());
    }
    let said = String::from_utf8_lossy(&out.stderr);
    Err(format!("{program} {}: {}", args.join(" "), said.trim()))
}

fn succeeds(program: &str, args: &[&str]) -> bool {
    exec(program, args).is_ok_and(|out| out.status.success())
}

/// `HKLM\path`, as `reg.exe` wants it.
pub fn key_arg(hive: Hive, path: &str) -> String {
    let name = match hive {
        Hive::Hkcu => "HKCU",
        Hive::Hklm => "HKLM",
    };
    format!(r"{name}\{path}")
}

/// Only an administrator can read the profile of a system account.
pub fn is_admin() -> bool {
    succeeds("reg.exe", &["query", r"HKU\S-1-5-19"])
}

pub fn key_exists(hive: Hive, path: &str) -> bool {
    succeeds("reg.exe", &["query", &key_arg(hive, path)])
}

pub fn add_key(hive: Hive, path: &str) -> Result<(), String> {
    must("reg.exe", &["add", &key_arg(hive, path), "/f"])
}

/// Sets a value; the empty name is the default value of the key.
pub fn set_value(
    hive: Hive,
    path: &str,
    name: &str,
    data: &Data,
    folder: &str,
) -> Result<(), String> {
    match data {
        Data::Text(text) => set_raw(
            hive,
            path,
            name,
            "REG_SZ",
            &crate::manifest::expand(text, folder),
        ),
        Data::Dword(n) => set_raw(hive, path, name, "REG_DWORD", &n.to_string()),
    }
}

/// A text that is only known when it runs (`Data::Text` holds only what is written in the code).
pub fn set_text(hive: Hive, path: &str, name: &str, text: &str) -> Result<(), String> {
    set_raw(hive, path, name, "REG_SZ", text)
}

fn set_raw(hive: Hive, path: &str, name: &str, kind: &str, text: &str) -> Result<(), String> {
    let key = key_arg(hive, path);
    let mut args = vec!["add", key.as_str()];
    match name {
        "" => args.push("/ve"),
        _ => args.extend(["/v", name]),
    }
    args.extend(["/t", kind, "/d", text, "/f"]);
    must("reg.exe", &args)
}

pub fn delete_key(hive: Hive, path: &str) -> Result<(), String> {
    must("reg.exe", &["delete", &key_arg(hive, path), "/f"])
}

pub fn delete_value(hive: Hive, path: &str, name: &str) -> Result<(), String> {
    must(
        "reg.exe",
        &["delete", &key_arg(hive, path), "/v", name, "/f"],
    )
}

/// The text of a value (the default one when `name` is empty), or `None` when there is none.
pub fn read_value(hive: Hive, path: &str, name: &str) -> Option<String> {
    let key = key_arg(hive, path);
    let mut args = vec!["query", key.as_str()];
    match name {
        "" => args.push("/ve"),
        _ => args.extend(["/v", name]),
    }
    let out = exec("reg.exe", &args).ok().filter(|o| o.status.success())?;
    parse_value(&String::from_utf8_lossy(&out.stdout))
}

/// The data in the output of `reg query`: `    Name    REG_SZ    data`.
pub fn parse_value(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let (_, after) = line.split_once("REG_SZ")?;
        let data = after.trim();
        (!data.is_empty()).then(|| data.to_string())
    })
}

pub fn task_exists(name: &str) -> bool {
    succeeds("schtasks.exe", &["/Query", "/TN", name])
}

/// A task that runs weekly: it must exist to be found, not to do anything.
pub fn create_task(name: &str, command: &str) -> Result<(), String> {
    must(
        "schtasks.exe",
        &[
            "/Create", "/TN", name, "/TR", command, "/SC", "WEEKLY", "/D", "SUN", "/ST", "03:00",
            "/RL", "HIGHEST", "/F",
        ],
    )
}

pub fn delete_task(name: &str) -> Result<(), String> {
    must("schtasks.exe", &["/Delete", "/TN", name, "/F"])
}

const MAKE_SHORTCUT: &str =
    "$s = (New-Object -ComObject WScript.Shell).CreateShortcut($env:FEX_LNK); \
$s.TargetPath = $env:FEX_TARGET; $s.Save()";

/// A `.lnk` at `path` that opens `target`. The two travel in the environment of the child, so
/// no text of theirs is ever read as part of a script.
pub fn make_shortcut(path: &str, target: &str) -> Result<(), String> {
    let out = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", MAKE_SHORTCUT])
        .env("FEX_LNK", path)
        .env("FEX_TARGET", target)
        .output()
        .map_err(|e| format!("no se pudo ejecutar powershell.exe: {e}"))?;
    if out.status.success() {
        return Ok(());
    }
    Err(format!(
        "no se pudo crear el acceso directo: {}",
        String::from_utf8_lossy(&out.stderr).trim()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_named_with_its_hive() {
        assert_eq!(
            key_arg(Hive::Hklm, r"SOFTWARE\FexSonda"),
            r"HKLM\SOFTWARE\FexSonda"
        );
        assert_eq!(
            key_arg(Hive::Hkcu, r"Software\FexSonda"),
            r"HKCU\Software\FexSonda"
        );
    }

    #[test]
    fn the_data_is_read_from_what_reg_query_prints() {
        let said = "\r\nHKEY_LOCAL_MACHINE\\SOFTWARE\\Classes\\.fexcomun\r\n    (Default)    REG_SZ    FexOtra.Doc\r\n\r\n";
        assert_eq!(parse_value(said).as_deref(), Some("FexOtra.Doc"));
        assert_eq!(parse_value("    (Default)    REG_SZ    \r\n"), None);
        assert_eq!(parse_value("ERROR: value not found"), None);
    }
}
