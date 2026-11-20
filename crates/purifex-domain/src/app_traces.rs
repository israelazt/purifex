//! The traces an app leaves after it is uninstalled (folders, registry keys,
//! shortcuts) and the rules that say which of them may be removed. The finder only
//! proposes places that *might* belong to the app; whether each one does is decided
//! here, by rules, so a mistake of the finder cannot delete what is not the app's.

use serde::{Deserialize, Serialize};

use crate::apps::InstalledApp;
use crate::seeds::{seeds_of, Seeds};
use crate::trace_rules::judge_by_evidence;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceKind {
    #[default]
    Folder,
    File,
    /// A key of the registry, written `HKCU\Software\App`.
    RegistryKey,
    /// One value inside a registry key that is not the app's (`MuiCache`): only that
    /// value is the app's. `Trace.location` is the key and `Trace.value` the name.
    RegistryValue,
}

/// What proves a trace is the app's. Without evidence nothing is proposed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Evidence {
    /// The install folder the app registered itself.
    RegisteredInstall,
    /// A name identical to the app's, in a fixed place.
    #[default]
    NameMatch,
    /// The app's name plus a known suffix (`kdenliverc`).
    NamePrefix,
    /// A value or file that contains the app's install path or one of its `.exe`.
    PathReference,
    /// A file-type key whose command or icon points inside the app's folder.
    Association,
    /// A key or file that hangs from an extension the app registered.
    AssociatedExtension,
    /// `<app exe>.<pid>.dmp` in `CrashDumps`.
    CrashDump,
}

impl Evidence {
    /// What proves it, in words for the person.
    pub fn label(self) -> &'static str {
        match self {
            Evidence::RegisteredInstall => "carpeta de instalación registrada",
            Evidence::NameMatch => "nombre de la app",
            Evidence::NamePrefix => "nombre de la app + sufijo",
            Evidence::PathReference => "referencia a la carpeta de la app",
            Evidence::Association => "asociación de archivo",
            Evidence::AssociatedExtension => "extensión de archivo de la app",
            Evidence::CrashDump => "volcado de error",
        }
    }
}

/// A place that might belong to the app, as the finder found it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Candidate {
    pub kind: TraceKind,
    pub location: String,
    pub bytes: u64,
    /// The finder found it as the install folder the app itself registered.
    pub registered_install: bool,
    /// For a registry value: its name (`location` is the key).
    pub value: Option<String>,
    /// What the trace contains, read by the finder: the data of a value, the command
    /// or icon of a file-type key, the target of a shortcut.
    pub text: Option<String>,
}

/// A trace that may be removed, and why.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trace {
    pub kind: TraceKind,
    pub location: String,
    pub bytes: u64,
    pub why: String,
    /// Settings or saved data of the user (not the program itself).
    pub user_data: bool,
    /// For `RegistryValue`: the name of the value inside `location`.
    #[serde(default)]
    pub value: Option<String>,
    /// What proves it is the app's. History written before this field reads as `NameMatch`.
    #[serde(default)]
    pub evidence: Evidence,
}

impl Trace {
    /// What tells this trace from every other: its place, and for a registry value also
    /// its name (several values share one key).
    pub fn key(&self) -> String {
        match &self.value {
            Some(value) => format!("{}\u{1f}{value}", self.location),
            None => self.location.clone(),
        }
    }

    /// A key or a value of the registry (not a file or a folder).
    pub fn is_registry(&self) -> bool {
        matches!(self.kind, TraceKind::RegistryKey | TraceKind::RegistryValue)
    }

    /// Whether Windows lets only an administrator remove it: anything in the registry of the
    /// machine (`HKLM`), and what is inside Program Files. (The rest the user may remove.)
    pub fn needs_admin(&self) -> bool {
        let lower = self.location.to_lowercase();
        lower.starts_with(r"hklm\")
            || lower
                .get(1..)
                .is_some_and(|rest| rest.starts_with(r":\program files"))
    }
}

/// Why a candidate was left alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    NotAbsolute,
    SystemPlace,
    TooBroad,
    SharedWithOtherApp,
    NotTheApps,
}

/// Names that mean "everything of a kind": never a trace of one app.
pub(crate) const BROAD: [&str; 19] = [
    "microsoft",
    "windows",
    "common files",
    "windowsapps",
    "packages",
    "programs",
    "program files",
    "program files (x86)",
    "programdata",
    "appdata",
    "local",
    "roaming",
    "locallow",
    "software",
    "users",
    "system32",
    "wow6432node",
    "classes",
    "temp",
];

/// Places that belong to Windows, whatever their name.
const SYSTEM_PREFIXES: [&str; 5] = [
    r"c:\windows",
    r"hklm\software\microsoft",
    r"hkcu\software\microsoft",
    r"hklm\system",
    r"hklm\software\classes",
];

/// Lowercase with only letters and digits, to compare names (`Riot Games` = `riot-games`).
pub fn simple(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

pub(crate) fn last_part(location: &str) -> &str {
    location
        .rsplit(['\\', '/'])
        .find(|p| !p.is_empty())
        .unwrap_or("")
}

fn is_absolute(location: &str) -> bool {
    let l = location.trim();
    let drive = l.len() > 3
        && l.as_bytes()[1] == b':'
        && l.as_bytes()[2] == b'\\'
        && l.as_bytes()[0].is_ascii_alphabetic();
    let registry = l.starts_with("HKCU\\") || l.starts_with("HKLM\\");
    (drive || registry) && !l.split(['\\', '/']).any(|p| p == ".." || p == ".")
}

/// The name under which an app puts its things: its own full name without the
/// words that are versions (`Git 2.55` is `git`). Never a word of it alone (`Google`
/// of `Google Chrome` is the folder of the whole publisher) and never the name of
/// its install folder (`Application`).
pub fn names_of(app: &InstalledApp) -> Vec<String> {
    let without_version: Vec<&str> = app
        .name
        .split_whitespace()
        .filter(|w| !w.chars().next().is_some_and(|c| c.is_ascii_digit()))
        .collect();
    let name = simple(&without_version.join(" "));
    if name.len() >= 3 && !BROAD.contains(&name.as_str()) {
        vec![name]
    } else {
        Vec::new()
    }
}

fn same_publisher(a: &InstalledApp, b: &InstalledApp) -> bool {
    match (&a.publisher, &b.publisher) {
        (Some(x), Some(y)) => simple(x) == simple(y) && !simple(x).is_empty(),
        _ => false,
    }
}

/// Decides whether `candidate` is a trace of `app` that may be removed. `others` are
/// the apps still installed. The seeds are the ones the registry entry alone gives.
pub fn judge(
    app: &InstalledApp,
    others: &[InstalledApp],
    candidate: &Candidate,
) -> Result<Trace, Refusal> {
    judge_with(app, others, &seeds_of(app), candidate)
}

/// `judge` with the seeds observed for the app (its folder, programs and extensions).
pub fn judge_with(
    app: &InstalledApp,
    others: &[InstalledApp],
    seeds: &Seeds,
    candidate: &Candidate,
) -> Result<Trace, Refusal> {
    let location = candidate.location.trim().trim_end_matches('\\');
    if !is_absolute(location) {
        return Err(Refusal::NotAbsolute);
    }
    if let Some(decided) = judge_by_evidence(app, others, seeds, candidate, location) {
        return decided;
    }
    // A value is the app's only by evidence: it has no name of its own to go by.
    if candidate.kind == TraceKind::RegistryValue {
        return Err(Refusal::NotTheApps);
    }
    judge_by_name(app, others, candidate, location)
}

/// The classic rule: a place with the app's name (or the folder it registered), that is
/// not Windows', not too broad and not something another installed app needs.
fn judge_by_name(
    app: &InstalledApp,
    others: &[InstalledApp],
    candidate: &Candidate,
    location: &str,
) -> Result<Trace, Refusal> {
    let lower = location.to_lowercase();
    refuse_system_place(&lower)?;
    let own = simple(last_part(location));
    refuse_too_broad(&lower, &own)?;
    refuse_shared(app, others, &lower, &own)?;
    if !candidate.registered_install && !names_of(app).contains(&own) {
        return Err(Refusal::NotTheApps);
    }
    Ok(trace_by_name(candidate, location, &lower))
}

fn refuse_system_place(lower: &str) -> Result<(), Refusal> {
    let system = SYSTEM_PREFIXES
        .iter()
        .any(|p| lower == *p || lower.starts_with(&format!("{p}\\")));
    if system {
        Err(Refusal::SystemPlace)
    } else {
        Ok(())
    }
}

fn refuse_too_broad(lower: &str, own: &str) -> Result<(), Refusal> {
    if BROAD.contains(&last_part(lower)) || own.is_empty() || lower.matches('\\').count() < 2 {
        Err(Refusal::TooBroad)
    } else {
        Ok(())
    }
}

/// Something another installed app needs: its folder is this one or inside it, or this
/// folder is the publisher's shared one and the publisher has other apps.
fn refuse_shared(
    app: &InstalledApp,
    others: &[InstalledApp],
    lower: &str,
    own: &str,
) -> Result<(), Refusal> {
    let publisher_folder =
        app.publisher.as_deref().map(simple).is_some_and(|p| {
            p.len() >= 3 && (p == own || p.starts_with(own) || own.starts_with(&p))
        });
    let needed = others.iter().filter(|o| o.id != app.id).any(|other| {
        let inside = other.install_location.as_deref().is_some_and(|l| {
            let l = l.trim_end_matches('\\').to_lowercase();
            !l.is_empty() && (l == lower || l.starts_with(&format!("{lower}\\")))
        });
        inside || (publisher_folder && same_publisher(app, other))
    });
    if needed {
        Err(Refusal::SharedWithOtherApp)
    } else {
        Ok(())
    }
}

/// The trace of a place proposed by name or as the registered install folder. The
/// folder the app installed itself in is the program, not the user's data, even when
/// it lives under AppData (`AppData\Local\Programs`).
fn trace_by_name(candidate: &Candidate, location: &str, lower: &str) -> Trace {
    let registered = candidate.registered_install;
    let user_data = !registered
        && ["\\appdata\\", "\\documents\\", "\\saved games\\"]
            .iter()
            .any(|p| lower.contains(p));
    let why = if registered {
        "la carpeta de instalación que registró la app".to_string()
    } else if candidate.kind == TraceKind::RegistryKey {
        format!(
            "clave del registro con el nombre de la app: {}",
            last_part(location)
        )
    } else {
        format!("lleva el nombre de la app: {}", last_part(location))
    };
    Trace {
        kind: candidate.kind,
        location: location.to_string(),
        bytes: candidate.bytes,
        why,
        user_data,
        evidence: if registered {
            Evidence::RegisteredInstall
        } else {
            Evidence::NameMatch
        },
        ..Default::default()
    }
}

/// The traces to propose: every candidate that passes `judge`, the biggest first,
/// and a folder inside another proposed folder is not listed apart.
pub fn propose(
    app: &InstalledApp,
    others: &[InstalledApp],
    candidates: &[Candidate],
) -> Vec<Trace> {
    propose_with(app, others, &seeds_of(app), candidates)
}

pub fn propose_with(
    app: &InstalledApp,
    others: &[InstalledApp],
    seeds: &Seeds,
    candidates: &[Candidate],
) -> Vec<Trace> {
    let mut traces: Vec<Trace> = candidates
        .iter()
        .filter_map(|c| judge_with(app, others, seeds, c).ok())
        .collect();
    traces.sort_by(|a, b| {
        b.bytes
            .cmp(&a.bytes)
            .then(a.location.cmp(&b.location))
            .then(a.value.cmp(&b.value))
    });
    traces.dedup_by(|b, a| a.location.eq_ignore_ascii_case(&b.location) && a.value == b.value);
    // Whole places (not single values) that are proposed: what is inside is not listed apart.
    let parents: Vec<String> = traces
        .iter()
        .filter(|t| t.kind != TraceKind::RegistryValue)
        .map(|t| t.location.to_lowercase())
        .collect();
    traces.retain(|t| {
        let l = t.location.to_lowercase();
        let is_value = t.kind == TraceKind::RegistryValue;
        !parents
            .iter()
            .any(|p| l.starts_with(&format!("{p}\\")) || (is_value && *p == l))
    });
    traces
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace_text::traces_to_text;

    #[test]
    fn history_written_before_values_and_evidence_still_reads() {
        let old = r#"{"kind":"folder","location":"C:\\x\\y","bytes":3,"why":"w","user_data":true}"#;
        let t: Trace = serde_json::from_str(old).unwrap();
        assert_eq!(t.value, None);
        assert_eq!(t.evidence, Evidence::NameMatch);
    }

    #[test]
    fn the_registry_of_the_machine_and_program_files_need_an_administrator_and_the_rest_does_not() {
        let at = |kind: TraceKind, location: &str| Trace {
            kind,
            location: location.into(),
            ..Default::default()
        };
        for admin in [
            at(TraceKind::RegistryKey, r"HKLM\SOFTWARE\Classes\.kdenlive"),
            at(TraceKind::RegistryValue, r"hklm\software\x"),
            at(TraceKind::Folder, r"C:\Program Files\Kdenlive"),
            at(TraceKind::File, r"D:\Program Files (x86)\Old\o.dll"),
        ] {
            assert!(admin.needs_admin(), "{}", admin.location);
        }
        for user in [
            at(TraceKind::RegistryKey, r"HKCU\Software\Kdenlive"),
            at(TraceKind::Folder, r"C:\Users\u\AppData\Local\kdenlive"),
            at(TraceKind::Folder, r"C:\ProgramData\Kdenlive"),
            at(TraceKind::File, r"C:\Games\Program Files\x"),
            at(TraceKind::File, "x"),
        ] {
            assert!(!user.needs_admin(), "{}", user.location);
        }
    }

    #[test]
    fn a_registry_value_round_trips() {
        let t = Trace {
            kind: TraceKind::RegistryValue,
            location: r"HKCU\Software\X\MuiCache".into(),
            why: "w".into(),
            value: Some(r"C:\Apps\X\x.exe".into()),
            evidence: Evidence::PathReference,
            ..Default::default()
        };
        let back: Trace = serde_json::from_str(&serde_json::to_string(&t).unwrap()).unwrap();
        assert_eq!(back, t);
    }

    fn app(name: &str, publisher: &str, location: Option<&str>) -> InstalledApp {
        InstalledApp {
            id: format!("HKLM64:{name}"),
            name: name.into(),
            publisher: Some(publisher.into()),
            install_location: location.map(String::from),
            uninstall: Some("u.exe".into()),
            ..Default::default()
        }
    }

    fn cand(kind: TraceKind, location: &str) -> Candidate {
        Candidate {
            kind,
            location: location.into(),
            bytes: 1000,
            ..Default::default()
        }
    }

    fn folder(location: &str) -> Candidate {
        cand(TraceKind::Folder, location)
    }

    #[test]
    fn the_folder_the_app_registered_and_the_ones_with_its_name_are_traces_and_appdata_is_the_users(
    ) {
        let discord = app(
            "Discord",
            "Discord Inc.",
            Some(r"C:\Users\u\AppData\Local\Discord"),
        );
        let mut registered = folder(r"C:\Users\u\AppData\Local\Discord");
        registered.registered_install = true;
        let t = judge(&discord, &[], &registered).unwrap();
        assert!(t.why.contains("registró"));
        let roaming = judge(
            &discord,
            &[],
            &folder(r"C:\Users\u\AppData\Roaming\discord"),
        )
        .unwrap();
        assert!(roaming.user_data, "settings of the user are marked");
        let program = judge(&discord, &[], &folder(r"C:\Program Files\Discord")).unwrap();
        assert!(!program.user_data);
        // Its own install folder under AppData is the program, not the user's data.
        assert!(!t.user_data, "registered install folder");
        let key = judge(
            &discord,
            &[],
            &cand(TraceKind::RegistryKey, r"HKCU\Software\Discord"),
        )
        .unwrap();
        assert!(key.why.contains("registro"));
    }

    #[test]
    fn what_is_not_named_like_the_app_is_not_proposed() {
        let discord = app("Discord", "Discord Inc.", None);
        assert_eq!(
            judge(&discord, &[], &folder(r"C:\Users\u\AppData\Roaming\Slack")),
            Err(Refusal::NotTheApps)
        );
        assert_eq!(
            judge(&discord, &[], &folder(r"C:\Program Files\DiscordPTB")),
            Err(Refusal::NotTheApps)
        );
    }

    #[test]
    fn broad_places_and_the_system_are_never_proposed_whatever_the_name() {
        let app_named = |n: &str| app(n, "X", Some(r"C:\Program Files"));
        for (name, place, why) in [
            (
                "Microsoft",
                r"C:\Program Files\Microsoft",
                Refusal::TooBroad,
            ),
            ("Windows", r"C:\Windows", Refusal::SystemPlace),
            ("Discord", r"C:\Windows\Temp\Discord", Refusal::SystemPlace),
            (
                "Discord",
                r"C:\Windows\System32\Discord",
                Refusal::SystemPlace,
            ),
            ("Programs", r"C:\Program Files", Refusal::TooBroad),
            ("Roaming", r"C:\Users\u\AppData\Roaming", Refusal::TooBroad),
            ("Discord", r"C:\Discord", Refusal::TooBroad),
            ("Discord", "relative\\Discord", Refusal::NotAbsolute),
            (
                "Discord",
                r"C:\Users\u\AppData\Roaming\..\..\Discord",
                Refusal::NotAbsolute,
            ),
            ("Discord", "", Refusal::NotAbsolute),
        ] {
            let mut c = folder(place);
            c.registered_install = true;
            assert_eq!(judge(&app_named(name), &[], &c), Err(why), "{place}");
        }
        let windows_key = cand(TraceKind::RegistryKey, r"HKLM\SOFTWARE\Microsoft\Discord");
        assert_eq!(
            judge(&app("Discord", "X", None), &[], &windows_key),
            Err(Refusal::SystemPlace)
        );
        let classes = cand(TraceKind::RegistryKey, r"HKLM\SOFTWARE\Classes\Discord");
        assert_eq!(
            judge(&app("Discord", "X", None), &[], &classes),
            Err(Refusal::SystemPlace)
        );
    }

    #[test]
    fn a_folder_of_the_publisher_stays_while_the_publisher_has_another_app_installed() {
        let chrome = app(
            "Google Chrome",
            "Google LLC",
            Some(r"C:\Program Files\Google\Chrome\Application"),
        );
        let earth = app(
            "Google Earth Pro",
            "Google LLC",
            Some(r"C:\Program Files\Google\Google Earth Pro"),
        );
        let mut shared = folder(r"C:\Program Files\Google");
        shared.registered_install = true;
        assert_eq!(
            judge(&chrome, std::slice::from_ref(&earth), &shared),
            Err(Refusal::SharedWithOtherApp)
        );
        // The folder of the app's own install is fine even so.
        let mut own = folder(r"C:\Program Files\Google\Chrome\Application");
        own.registered_install = true;
        assert!(judge(&chrome, &[earth], &own).is_ok());
    }

    #[test]
    fn a_folder_that_holds_the_install_of_another_app_is_not_proposed() {
        let a = app("Alpha Tool", "Acme", None);
        let b = app("Beta", "Other", Some(r"C:\Program Files\Alpha Tool\Beta"));
        assert_eq!(
            judge(&a, &[b], &folder(r"C:\Program Files\Alpha Tool")),
            Err(Refusal::SharedWithOtherApp)
        );
    }

    #[test]
    fn a_word_of_the_name_alone_or_a_version_is_not_the_name() {
        assert_eq!(names_of(&app("Git 2.55.0.3", "G", None)), ["git"]);
        assert_eq!(
            names_of(&app("Google Chrome", "Google LLC", None)),
            ["googlechrome"]
        );
        assert!(names_of(&app("Microsoft", "M", None)).is_empty());
        assert!(
            names_of(&app("Go", "G", None)).is_empty(),
            "too short to be a safe name"
        );
        let chrome = app("Google Chrome", "Google LLC", None);
        assert_eq!(
            judge(&chrome, &[], &folder(r"C:\Program Files\Google")),
            Err(Refusal::NotTheApps)
        );
        assert_eq!(
            judge(&chrome, &[], &folder(r"C:\Program Files\Application")),
            Err(Refusal::NotTheApps)
        );
    }

    #[test]
    fn the_proposal_keeps_what_passes_lists_the_biggest_first_and_hides_what_is_inside_another() {
        let a = app("Discord", "Discord Inc.", None);
        let mut big = folder(r"C:\Users\u\AppData\Local\Discord");
        big.bytes = 500;
        let mut inner = folder(r"C:\Users\u\AppData\Local\Discord\Discord");
        inner.bytes = 9;
        let small = Candidate {
            bytes: 10,
            ..folder(r"C:\Users\u\AppData\Roaming\Discord")
        };
        let other = folder(r"C:\Users\u\AppData\Roaming\Slack");
        let list = propose(&a, &[], &[small, other, inner, big]);
        let places: Vec<&str> = list.iter().map(|t| t.location.as_str()).collect();
        assert_eq!(
            places,
            [
                r"C:\Users\u\AppData\Local\Discord",
                r"C:\Users\u\AppData\Roaming\Discord"
            ]
        );
    }

    #[test]
    fn the_text_marks_the_users_data_and_says_nothing_was_deleted() {
        let a = app("Discord", "Discord Inc.", None);
        let t = judge(&a, &[], &folder(r"C:\Users\u\AppData\Roaming\Discord")).unwrap();
        let text = traces_to_text(&a, &[t]);
        assert!(text.contains("nada se ha borrado") && text.contains("[datos tuyos]"));
        assert!(traces_to_text(&a, &[]).contains("No se encontraron restos"));
    }
}
