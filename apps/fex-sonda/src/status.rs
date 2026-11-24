//! `status`: what of the manifest is on this computer now, one line per row, or as JSON to compare
//! before and after. It only reads.

use crate::manifest::{Hive, Role, ITEMS};
use crate::{items, removal, say, system};

const FEXCOMUN: &str = r"SOFTWARE\Classes\.fexcomun";

/// What `status` knows about one row.
pub struct Line {
    pub id: &'static str,
    pub present: bool,
    pub role: Role,
    pub place: String,
}

pub fn read() -> Result<Vec<Line>, String> {
    ITEMS
        .iter()
        .map(|row| {
            Ok(Line {
                id: row.id,
                present: items::exists(&row.item)?,
                role: row.role,
                place: removal::where_of(&row.item)?,
            })
        })
        .collect()
}

fn role_word(role: Role) -> &'static str {
    match role {
        Role::RemovedByUninstaller => "desinstalador",
        Role::Leftover => "resto",
        Role::Control => "control",
    }
}

fn role_key(role: Role) -> &'static str {
    match role {
        Role::RemovedByUninstaller => "removed_by_uninstaller",
        Role::Leftover => "leftover",
        Role::Control => "control",
    }
}

/// Who `.fexcomun` opens with now.
fn fexcomun_points_to() -> Option<String> {
    system::read_value(Hive::Hklm, FEXCOMUN, "")
}

fn present_word(present: bool) -> &'static str {
    if present {
        "presente"
    } else {
        "ausente"
    }
}

/// A text as a JSON string.
pub fn json_text(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn as_json(lines: &[Line], points_to: Option<&str>) -> String {
    let rows: Vec<String> = lines
        .iter()
        .map(|l| {
            format!(
                "{{\"id\":{},\"present\":{},\"role\":{},\"where\":{}}}",
                json_text(l.id),
                l.present,
                json_text(role_key(l.role)),
                json_text(&l.place)
            )
        })
        .collect();
    let owner = points_to.map_or("null".to_string(), json_text);
    format!(
        "{{\"fexcomun_points_to\":{owner},\"items\":[{}]}}",
        rows.join(",")
    )
}

fn totals(lines: &[Line]) -> Vec<String> {
    [Role::RemovedByUninstaller, Role::Leftover, Role::Control]
        .into_iter()
        .map(|role| {
            let of_role: Vec<_> = lines.iter().filter(|l| l.role == role).collect();
            let present = of_role.iter().filter(|l| l.present).count();
            format!(
                "{}: {present} de {} presentes",
                role_word(role),
                of_role.len()
            )
        })
        .collect()
}

pub fn status(json: bool) -> Result<(), String> {
    let lines = read()?;
    let owner = fexcomun_points_to();
    if json {
        say::line(&as_json(&lines, owner.as_deref()));
        return Ok(());
    }
    for l in &lines {
        say::line(&format!(
            "{:<4} {:<8} {:<14} {}",
            l.id,
            present_word(l.present),
            role_word(l.role),
            l.place
        ));
    }
    say::line(&format!(
        ".fexcomun abre con: {}",
        owner.as_deref().unwrap_or("(nada)")
    ));
    totals(&lines).iter().for_each(|t| say::line(t));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_text_is_written_as_json_with_its_quotes_and_backslashes_escaped() {
        assert_eq!(json_text(r#"C:\a "b""#), r#""C:\\a \"b\"""#);
    }

    #[test]
    fn the_json_lists_each_row_and_says_who_has_the_extension() {
        let lines = [Line {
            id: "C1",
            present: true,
            role: Role::Control,
            place: r"HKLM\x".into(),
        }];
        let json = as_json(&lines, Some("FexOtra.Doc"));
        assert_eq!(
            json,
            r#"{"fexcomun_points_to":"FexOtra.Doc","items":[{"id":"C1","present":true,"role":"control","where":"HKLM\\x"}]}"#
        );
        assert!(as_json(&[], None).contains("\"fexcomun_points_to\":null"));
    }

    #[test]
    fn the_totals_count_what_is_present_of_each_role() {
        let line = |id, present, role| Line {
            id,
            present,
            role,
            place: String::new(),
        };
        let lines = [
            line("2", true, Role::Leftover),
            line("4", false, Role::Leftover),
            line("C1", true, Role::Control),
        ];
        let said = totals(&lines);
        assert!(said.contains(&"resto: 1 de 2 presentes".to_string()));
        assert!(said.contains(&"control: 1 de 1 presentes".to_string()));
        assert!(said.contains(&"desinstalador: 0 de 0 presentes".to_string()));
    }
}
