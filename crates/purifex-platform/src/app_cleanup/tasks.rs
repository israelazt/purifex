//! The scheduled tasks of Windows: the ones that run a program inside the app's folder (an
//! updater or a starter an uninstaller left), and how to take one out and put it back. All
//! goes through `schtasks.exe`: its XML is the same in every language, and it is the tool
//! Windows gives for this (the Task Scheduler keeps a file and several registry keys per
//! task, which are not to be edited by hand).

use std::fs;
use std::os::windows::process::CommandExt;
use std::process::Command;

use purifex_domain::app_traces::{Candidate, TraceKind};
use purifex_domain::reference::path_refers_to;
use purifex_domain::seeds::Seeds;

use super::find_services::expand;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// The tasks whose action runs something inside `seeds.folder`.
pub fn find_tasks(seeds: &Seeds) -> Vec<Candidate> {
    let Some(folder) = seeds.folder.as_deref() else {
        return Vec::new();
    };
    let Ok(output) = schtasks(&["/query", "/xml"]) else {
        return Vec::new();
    };
    parse_tasks(&String::from_utf8_lossy(&output))
        .into_iter()
        .filter(|(_, actions)| path_refers_to(&expand(actions), folder))
        .map(|(path, actions)| Candidate {
            kind: TraceKind::Task,
            location: path,
            text: Some(expand(&actions)),
            ..Default::default()
        })
        .collect()
}

/// `schtasks /query /xml` writes every task as `<!-- \Path -->` followed by its XML. Gives
/// each task's path and the text of what it runs (command, arguments, working folder).
fn parse_tasks(output: &str) -> Vec<(String, String)> {
    output
        .split("<!-- ")
        .skip(1)
        .filter_map(|chunk| {
            let (path, xml) = chunk.split_once(" -->")?;
            let mut actions = String::new();
            for tag in ["Command", "Arguments", "WorkingDirectory"] {
                for text in texts_of(xml, tag) {
                    actions.push_str(&text);
                    actions.push('\n');
                }
            }
            (!actions.is_empty()).then(|| (path.trim().to_string(), actions))
        })
        .collect()
}

/// The text inside each `<tag>…</tag>` of `xml`, with the entities of XML undone.
fn texts_of(xml: &str, tag: &str) -> Vec<String> {
    let (open, close) = (format!("<{tag}>"), format!("</{tag}>"));
    xml.split(&open)
        .skip(1)
        .filter_map(|part| part.split_once(&close).map(|(text, _)| unescape(text)))
        .collect()
}

fn unescape(text: &str) -> String {
    text.replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn schtasks(args: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("schtasks")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("no se pudo ejecutar schtasks.exe: {e}"))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        let text = String::from_utf8_lossy(&output.stderr);
        Err(format!("schtasks.exe: {}", text.trim()))
    }
}

/// The tasks that run when someone logs on or when Windows starts, other than those of
/// Windows itself (`\Microsoft\`): path, what they run, and whether they are enabled.
pub fn startup_tasks() -> Vec<(String, String, bool)> {
    let Ok(output) = schtasks(&["/query", "/xml"]) else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output)
        .split("<!-- ")
        .skip(1)
        .filter_map(|chunk| {
            let (path, xml) = chunk.split_once(" -->")?;
            let path = path.trim();
            let starts = xml.contains("<LogonTrigger") || xml.contains("<BootTrigger");
            if !starts || path.to_lowercase().starts_with(r"\microsoft\") {
                return None;
            }
            let command = texts_of(xml, "Command").into_iter().next()?;
            let arguments = texts_of(xml, "Arguments").into_iter().next();
            let line = match arguments {
                Some(a) => format!("{command} {a}"),
                None => command,
            };
            let settings = xml.split("<Settings>").nth(1).unwrap_or_default();
            let enabled = !settings.contains("<Enabled>false</Enabled>");
            Some((path.to_string(), expand(&line), enabled))
        })
        .collect()
}

/// Turns a task on or off (it stays defined).
pub fn set_enabled(task: &str, on: bool) -> Result<(), String> {
    let flag = if on { "/enable" } else { "/disable" };
    schtasks(&["/change", "/tn", task, flag]).map(|_| ())
}

pub fn exists(task: &str) -> bool {
    schtasks(&["/query", "/tn", task]).is_ok()
}

/// Saves the definition of the task into `file` (as UTF-16, what its own XML declares) and
/// removes it. The file is written before anything is deleted.
pub fn take_out(task: &str, file: &str) -> Result<(), String> {
    let xml = schtasks(&["/query", "/tn", task, "/xml"])?;
    let text = String::from_utf8_lossy(&xml);
    let mut bytes = vec![0xFF, 0xFE];
    bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    fs::write(file, bytes).map_err(|e| format!("no se pudo guardar la tarea: {e}"))?;
    schtasks(&["/delete", "/tn", task, "/f"]).map(|_| ())
}

/// Creates the task again from the file `take_out` wrote. It never replaces one that exists.
pub fn put_back(task: &str, file: &str) -> Result<(), String> {
    if exists(task) {
        return Err("ya hay una tarea con ese nombre: no se sobrescribe".into());
    }
    schtasks(&["/create", "/tn", task, "/xml", file]).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    const OUTPUT: &str = r#"<Tasks>

<!-- \Kdenlive\Updater -->
<?xml version="1.0" encoding="UTF-16"?>
<Task><Actions Context="Author"><Exec>
<Command>"C:\Program Files\Kdenlive\update.exe"</Command>
<Arguments>/silent &amp; /x</Arguments>
</Exec></Actions></Task>

<!-- \Sin accion -->
<Task><Triggers/></Task>
</Tasks>"#;

    #[test]
    fn each_task_gives_its_path_and_what_it_runs_and_a_task_without_actions_is_skipped() {
        let tasks = parse_tasks(OUTPUT);
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].0, r"\Kdenlive\Updater");
        assert!(tasks[0]
            .1
            .contains(r#""C:\Program Files\Kdenlive\update.exe""#));
        assert!(tasks[0].1.contains("/silent & /x"), "entities are undone");
    }

    #[test]
    fn a_real_task_can_be_taken_out_and_put_back_the_same() {
        // A task of the user's own, made for this test.
        let task = format!(r"\PurifexTest{}", std::process::id());
        let made = schtasks(&[
            "/create",
            "/tn",
            &task,
            "/tr",
            r"C:\Windows\System32\cmd.exe /c exit",
            "/sc",
            "once",
            "/st",
            "23:59",
            "/f",
        ]);
        if made.is_err() {
            return; // This session may not create tasks: nothing to check.
        }
        let file = std::env::temp_dir().join(format!("purifex-task-{}.xml", std::process::id()));
        let file_text = file.to_string_lossy().into_owned();
        let out = take_out(&task, &file_text);
        let gone = !exists(&task);
        let again_while_gone = put_back(&task, &file_text);
        let back = exists(&task);
        let refuses = put_back(&task, &file_text).unwrap_err();
        let _ = schtasks(&["/delete", "/tn", &task, "/f"]);
        let _ = fs::remove_file(&file);
        out.unwrap();
        assert!(gone, "it was removed");
        again_while_gone.unwrap();
        assert!(back, "it came back");
        assert!(refuses.contains("no se sobrescribe"));
    }
}
