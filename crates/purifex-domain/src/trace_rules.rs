//! The rules that recognise a trace by *evidence* other than its name: a reference to
//! the app's folder, a file type the app registered, a name plus a known suffix, a
//! crash dump of its program. Each rule works only in a fixed list of places, so a
//! mistake of the finder cannot reach what is not the app's. Pure functions: no system.

use crate::app_traces::{names_of, simple, Candidate, Evidence, Refusal, Trace, TraceKind};
use crate::apps::InstalledApp;
use crate::reference::path_refers_to;
use crate::seeds::Seeds;

mod shell;
mod system;

/// Keys (below `HKCU\Software` or `HKLM\SOFTWARE`) that only hold caches and usage
/// lists of Windows: one *value* of them may be the app's. Lowercase, with the hive
/// stripped.
const VALUE_KEYS: [&str; 12] = [
    r"\microsoft\windows\shell\muicache",
    r"\microsoft\windows nt\currentversion\appcompatflags\compatibility assistant\store",
    r"\microsoft\windows nt\currentversion\appcompatflags\layers",
    r"\microsoft\windows\currentversion\explorer\featureusage\appswitched",
    r"\microsoft\windows\currentversion\explorer\featureusage\applaunch",
    r"\microsoft\windows\currentversion\explorer\featureusage\showjumpview",
    r"\microsoft\windows\currentversion\search\jumplistdata",
    r"\microsoft\windows\currentversion\applicationassociationtoasts",
    // The files an installer shares and the folders it made: one value is one path.
    r"\microsoft\windows\currentversion\shareddlls",
    r"\microsoft\windows\currentversion\installer\folders",
    // What starts with Windows: one value is one program (the 32-bit view ends the same way).
    r"\microsoft\windows\currentversion\run",
    r"\microsoft\windows\currentversion\runonce",
];

/// Keys of Explorer with one child per file extension.
const EXTENSION_KEYS: [&str; 3] = [
    r"hkcu\software\microsoft\windows\currentversion\explorer\fileexts",
    r"hkcu\software\microsoft\windows\currentversion\explorer\recentdocs",
    r"hkcu\software\microsoft\windows\currentversion\explorer\comdlg32\opensavepidlmru",
];

/// Keys with one child per program, named like the app or its `.exe`.
const PROGRAM_KEYS: [&str; 4] = [
    r"hkcu\software\microsoft\windows\currentversion\notifications\settings",
    r"hkcu\software\microsoft\windows\currentversion\pushnotifications\backup",
    r"hklm\software\microsoft\windows\currentversion\app paths",
    r"hkcu\software\microsoft\windows\currentversion\app paths",
];

/// Folders of the user where an app leaves files named after itself.
const DATA_FOLDERS: [&str; 3] = [
    r"\appdata\local",
    r"\appdata\roaming",
    r"\appdata\local\state",
];

/// Suffixes that make `<app name><suffix>` a settings or data file of the app.
const SUFFIXES: [&str; 10] = [
    "rc", "staterc", ".ini", ".log", ".json", ".cfg", ".conf", ".db", ".cache", ".xml",
];

/// What a rule found: the evidence and the sentence that says why.
type Found = (Evidence, String);

/// What a rule looks at: the app, its seeds, the candidate and where it is.
struct Ctx<'a> {
    app: &'a InstalledApp,
    seeds: &'a Seeds,
    candidate: &'a Candidate,
    /// Everything before the last `\`, and what follows it (lowercase).
    parent: &'a str,
    name: &'a str,
}

/// A rule answers `None` when the candidate is not its case.
type Rule = fn(&Ctx) -> Option<Found>;

const KEY_RULES: [Rule; 8] = [
    classes_key,
    explorer_extension_key,
    program_key,
    tile_key,
    system::service_key,
    shell::com_key,
    shell::typelib_key,
    shell::context_menu_key,
];
const TASK_RULES: [Rule; 1] = [system::task_action];
const FILE_RULES: [Rule; 4] = [
    settings_file,
    crash_dump,
    recent_by_extension,
    shortcut_into_folder,
];

/// `None`: not a case for these rules, the caller applies its own. `Some`: decided.
pub fn judge_by_evidence(
    app: &InstalledApp,
    others: &[InstalledApp],
    seeds: &Seeds,
    candidate: &Candidate,
    location: &str,
) -> Option<Result<Trace, Refusal>> {
    let lower = location.to_lowercase();
    let (evidence, why) = match candidate.kind {
        TraceKind::RegistryValue => (value_evidence(seeds, &lower, candidate)?, None),
        TraceKind::RegistryKey => decide(&KEY_RULES, app, seeds, candidate, &lower)?,
        TraceKind::File => decide(&FILE_RULES, app, seeds, candidate, &lower)?,
        TraceKind::Task => decide(&TASK_RULES, app, seeds, candidate, &lower)?,
        TraceKind::Folder => return None,
    };
    if seed_shared(others, app, seeds, evidence) {
        return Some(Err(Refusal::SharedWithOtherApp));
    }
    Some(Ok(Trace {
        kind: candidate.kind,
        location: location.to_string(),
        bytes: candidate.bytes,
        why: why.unwrap_or_else(|| why_of(evidence).to_string()),
        user_data: candidate.kind == TraceKind::File && lower.contains(r"\appdata\"),
        value: candidate
            .value
            .clone()
            .filter(|_| candidate.kind == TraceKind::RegistryValue),
        evidence,
    }))
}

/// The first rule of the list that recognises the candidate.
fn decide(
    rules: &[Rule],
    app: &InstalledApp,
    seeds: &Seeds,
    candidate: &Candidate,
    lower: &str,
) -> Option<(Evidence, Option<String>)> {
    let (parent, name) = lower.rsplit_once('\\')?;
    if name.is_empty() {
        return None;
    }
    let ctx = Ctx {
        app,
        seeds,
        candidate,
        parent,
        name,
    };
    let (evidence, why) = rules.iter().find_map(|rule| rule(&ctx))?;
    Some((evidence, Some(why)))
}

fn why_of(evidence: Evidence) -> &'static str {
    match evidence {
        Evidence::PathReference => "apunta a la carpeta o al programa de la app",
        Evidence::Association => "asociación de archivo cuyo comando está en la carpeta de la app",
        Evidence::AssociatedExtension => "depende de una extensión que registró la app",
        Evidence::CrashDump => "volcado de error del programa de la app",
        Evidence::NamePrefix => "archivo de ajustes con el nombre de la app",
        Evidence::NameMatch => "lleva el nombre de la app",
        Evidence::RegisteredInstall => "la carpeta de instalación que registró la app",
        Evidence::ServicePath => "servicio de Windows cuyo programa está en la carpeta de la app",
        Evidence::TaskAction => "tarea programada que ejecuta un programa de la app",
        Evidence::ComServer => "clase COM cuyo servidor está en la carpeta de la app",
        Evidence::ContextMenu => "entrada del menú contextual de un programa de la app",
    }
}

/// Another installed app owns the folder (or lives inside it): references to it are
/// not safe to take. Only seeds that rest on the folder can be shared.
fn seed_shared(others: &[InstalledApp], app: &InstalledApp, seeds: &Seeds, e: Evidence) -> bool {
    let Some(folder) = seeds.folder.as_deref() else {
        return false;
    };
    let rests_on_folder = matches!(
        e,
        Evidence::PathReference
            | Evidence::Association
            | Evidence::ServicePath
            | Evidence::TaskAction
            | Evidence::ComServer
            | Evidence::ContextMenu
    );
    if !rests_on_folder {
        return false;
    }
    others.iter().filter(|o| o.id != app.id).any(|o| {
        o.install_location.as_deref().is_some_and(|l| {
            let l = l.trim_end_matches('\\');
            !l.is_empty() && (path_refers_to(l, folder) || path_refers_to(folder, l))
        })
    })
}

// ---- values -------------------------------------------------------------------

/// A value of one of the fixed keys whose name or data points into the app's folder.
fn value_evidence(seeds: &Seeds, lower_key: &str, candidate: &Candidate) -> Option<Evidence> {
    let value = candidate.value.as_deref().filter(|v| !v.is_empty())?;
    if let Some(found) = system::firewall_value(seeds, lower_key, candidate) {
        return Some(found);
    }
    let hive_len = ["hkcu\\software", "hklm\\software"]
        .iter()
        .find(|h| lower_key.starts_with(*h))?
        .len();
    let key = &lower_key[hive_len..];
    if !VALUE_KEYS.iter().any(|k| key.ends_with(k)) {
        return None;
    }
    if key.ends_with(TOASTS) && is_toast_of_extension(seeds, value) {
        return Some(Evidence::AssociatedExtension);
    }
    let folder = seeds.folder.as_deref()?;
    let data = candidate.text.as_deref().unwrap_or("");
    let refers = path_refers_to(value, folder) || path_refers_to(data, folder);
    refers.then_some(Evidence::PathReference)
}

/// The key where Windows remembers the toasts of file-type associations.
const TOASTS: &str = r"\microsoft\windows\currentversion\applicationassociationtoasts";

/// A value of the toasts key is named `<ProgId>_<extension>` (`Kdenlive_.kdenlive`): the
/// app's if its extension is one the app registered.
pub fn is_toast_of_extension(seeds: &Seeds, name: &str) -> bool {
    let lower = name.to_lowercase();
    seeds
        .extensions
        .iter()
        .any(|ext| lower.ends_with(&format!("_{ext}")))
}

// ---- registry keys ------------------------------------------------------------

/// File-type keys of the classes: the extension the app registered, or a ProgId whose
/// command or icon is inside the app's folder. Never a generic extension.
fn classes_key(c: &Ctx) -> Option<Found> {
    if !matches!(
        c.parent,
        r"hklm\software\classes" | r"hkcu\software\classes"
    ) {
        return None;
    }
    if c.name.starts_with('.') {
        return c.seeds.extensions.iter().any(|e| e == c.name).then(|| {
            (
                Evidence::AssociatedExtension,
                format!("extensión de archivo que registró la app: {}", c.name),
            )
        });
    }
    let folder = c.seeds.folder.as_deref()?;
    let text = c.candidate.text.as_deref()?;
    path_refers_to(text, folder).then(|| {
        (
            Evidence::Association,
            format!(
                "tipo de archivo cuyo comando está en la carpeta de la app: {}",
                c.name
            ),
        )
    })
}

/// The keys of Explorer that hang from an extension the app registered.
fn explorer_extension_key(c: &Ctx) -> Option<Found> {
    if !EXTENSION_KEYS.contains(&c.parent) {
        return None;
    }
    let known = c
        .seeds
        .extensions
        .iter()
        .any(|e| e == c.name || e.trim_start_matches('.') == c.name);
    known.then(|| {
        (
            Evidence::AssociatedExtension,
            format!(
                "clave de Explorador de la extensión {}, que registró la app",
                c.name
            ),
        )
    })
}

/// Keys named after one of the app's programs (its name or its `.exe`).
fn program_key(c: &Ctx) -> Option<Found> {
    if !PROGRAM_KEYS.contains(&c.parent) {
        return None;
    }
    let own = simple(c.name.trim_end_matches(".exe"));
    let is_exe = |e: &String| simple(e.trim_end_matches(".exe")) == own;
    let ours = names_of(c.app).contains(&own) || c.seeds.exes.iter().any(is_exe);
    ours.then(|| {
        (
            Evidence::NameMatch,
            format!("clave del registro con el nombre del programa: {}", c.name),
        )
    })
}

/// A tile of the start menu: a key named like the app inside a `W~{id}` group of the tiles.
fn tile_key(c: &Ctx) -> Option<Found> {
    let (tiles, group) = c.parent.rsplit_once('\\')?;
    if !tiles.ends_with(TILES) || !group.starts_with("w~") {
        return None;
    }
    let ours = names_of(c.app).contains(&simple(c.name));
    ours.then(|| {
        (
            Evidence::NameMatch,
            format!(
                "mosaico del menú Inicio con el nombre de la app: {}",
                c.name
            ),
        )
    })
}

/// Where the start menu keeps its tiles (below HKCU Software).
const TILES: &str = r"hkcu\software\microsoft\windows\currentversion\start\tileproperties";

// ---- files --------------------------------------------------------------------

/// Settings next to the others of the user: `<name>rc`, `<name>.ini`...
fn settings_file(c: &Ctx) -> Option<Found> {
    if !DATA_FOLDERS.iter().any(|d| c.parent.ends_with(d)) || !parent_under_users(c.parent) {
        return None;
    }
    let names = names_of(c.app);
    let named = SUFFIXES.iter().any(|s| {
        c.name
            .strip_suffix(s)
            .is_some_and(|stem| names.contains(&simple(stem)))
    });
    named.then(|| {
        (
            Evidence::NamePrefix,
            format!("archivo de ajustes con el nombre de la app: {}", c.name),
        )
    })
}

/// `<program>.<pid>.dmp` in the crash dumps of the user.
fn crash_dump(c: &Ctx) -> Option<Found> {
    if !c.parent.ends_with(r"\appdata\local\crashdumps") || !parent_under_users(c.parent) {
        return None;
    }
    is_dump_of(c.name, &c.seeds.exes).then(|| {
        (
            Evidence::CrashDump,
            format!("volcado de error del programa de la app: {}", c.name),
        )
    })
}

/// A recent file of a type the app registered.
fn recent_by_extension(c: &Ctx) -> Option<Found> {
    let stem = c.name.strip_suffix(".lnk")?;
    if !c.parent.ends_with(RECENT) || !parent_under_users(c.parent) {
        return None;
    }
    let ext = c
        .seeds
        .extensions
        .iter()
        .find(|e| stem.ends_with(e.as_str()))?;
    Some((
        Evidence::AssociatedExtension,
        format!("acceso reciente a un archivo {ext}, tipo que registró la app"),
    ))
}

/// A shortcut (desktop, start menu, recent) whose target is inside the app's folder.
fn shortcut_into_folder(c: &Ctx) -> Option<Found> {
    c.name.strip_suffix(".lnk")?;
    let in_place = [r"\desktop", r"\microsoft\windows\start menu\programs"]
        .iter()
        .any(|d| c.parent.contains(d));
    if !in_place && !c.parent.ends_with(RECENT) {
        return None;
    }
    let folder = c.seeds.folder.as_deref()?;
    let target = c.candidate.text.as_deref()?;
    path_refers_to(target, folder).then(|| {
        (
            Evidence::PathReference,
            format!("acceso directo a la carpeta de la app: {}", c.name),
        )
    })
}

const RECENT: &str = r"\microsoft\windows\recent";

/// The user's own tree (`c:\users\x\appdata\...`): not a folder somebody else made.
fn parent_under_users(parent: &str) -> bool {
    parent.starts_with(r"c:\users\") || parent.get(1..).is_some_and(|r| r.starts_with(r":\users\"))
}

/// `<exe>.<pid>.dmp`, also `<exe>(<n>).<pid>.dmp`, for one of the app's programs.
fn is_dump_of(name: &str, exes: &[String]) -> bool {
    let Some(rest) = name.strip_suffix(".dmp") else {
        return false;
    };
    let Some((exe, pid)) = rest.rsplit_once('.') else {
        return false;
    };
    if pid.is_empty() || !pid.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let exe = match exe.strip_suffix(')').and_then(|e| e.rsplit_once('(')) {
        Some((base, n)) if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) => base,
        _ => exe,
    };
    exes.iter().any(|e| e == exe)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_traces::{judge_with, propose_with};

    const DIR: &str = r"C:\Program Files\Kdenlive";
    const USER: &str = r"C:\Users\ana\AppData";

    fn app(name: &str, location: Option<&str>) -> InstalledApp {
        InstalledApp {
            id: format!("id:{name}"),
            name: name.into(),
            install_location: location.map(String::from),
            ..Default::default()
        }
    }

    fn seeds() -> Seeds {
        Seeds::new(
            Some(DIR),
            &["kdenlive.exe".into()],
            &[".kdenlive".into(), ".txt".into()],
        )
    }

    fn cand(kind: TraceKind, location: &str, value: Option<&str>, text: Option<&str>) -> Candidate {
        Candidate {
            kind,
            location: location.into(),
            bytes: 10,
            value: value.map(String::from),
            text: text.map(String::from),
            ..Default::default()
        }
    }

    fn judge(c: &Candidate, others: &[InstalledApp]) -> Result<Trace, Refusal> {
        judge_with(&app("Kdenlive", None), others, &seeds(), c)
    }

    fn evidence(c: &Candidate) -> Evidence {
        judge(c, &[]).unwrap().evidence
    }

    const SERVICES: &str = r"HKLM\SYSTEM\CurrentControlSet\Services";

    #[test]
    fn a_task_that_runs_a_program_of_the_apps_folder_is_proposed_and_a_name_alone_is_not() {
        let task = |path: &str, text: &str| cand(TraceKind::Task, path, None, Some(text));
        let own = task(
            r"\Kdenlive\Updater",
            "\"C:\\Program Files\\Kdenlive\\update.exe\"\n/silent\n",
        );
        let t = judge(&own, &[]).unwrap();
        assert_eq!(
            (t.evidence, t.kind),
            (Evidence::TaskAction, TraceKind::Task)
        );
        assert!(t.needs_admin());
        // Named like the app but running something else.
        let by_name = task(r"\Kdenlive", r"C:\Windows\System32\cmd.exe /c echo");
        assert!(judge(&by_name, &[]).is_err());
        // No text read: nothing to go by.
        let blind = cand(TraceKind::Task, r"\Kdenlive\Updater", None, None);
        assert!(judge(&blind, &[]).is_err());
        // A path that climbs out is not a task path.
        let escaping = task(r"\..\Kdenlive", r"C:\Program Files\Kdenlive\x.exe");
        assert_eq!(judge(&escaping, &[]).unwrap_err(), Refusal::NotAbsolute);
        // Another installed app lives in the folder: not taken.
        let shared = InstalledApp {
            install_location: Some(DIR.into()),
            ..app("Other app", None)
        };
        assert!(judge(&own, &[shared]).is_err());
    }

    #[test]
    fn a_service_whose_program_is_in_the_apps_folder_is_proposed_and_only_through_the_folder() {
        let service = |name: &str, image: &str, parent: &str| {
            cand(
                TraceKind::RegistryKey,
                &format!(r"{parent}\{name}"),
                None,
                Some(image),
            )
        };
        let own = service(
            "KdenliveHelper",
            r#""C:\Program Files\Kdenlive\bin\helper.exe" -service"#,
            SERVICES,
        );
        let t = judge(&own, &[]).unwrap();
        assert_eq!(t.evidence, Evidence::ServicePath);
        assert!(t.needs_admin());
        // The name is the app's but the program is somewhere else: not proved.
        let by_name = service(
            "Kdenlive",
            r"C:\Windows\System32\svchost.exe -k x",
            SERVICES,
        );
        assert!(judge(&by_name, &[]).is_err());
        // A folder of another app that only starts like this one.
        let other = service("Other", r"C:\Program Files\Kdenlive2\x.exe", SERVICES);
        assert!(judge(&other, &[]).is_err());
        // The same text under any other key is not a service.
        let elsewhere = service(
            "KdenliveHelper",
            r"C:\Program Files\Kdenlive\x.exe",
            r"HKLM\SOFTWARE\Foo",
        );
        assert!(judge(&elsewhere, &[]).is_err());
        // Another installed app lives in that folder: it is not taken.
        let shared = InstalledApp {
            install_location: Some(DIR.into()),
            ..app("Other app", None)
        };
        assert!(judge(&own, &[shared]).is_err());
    }

    const MUI: &str =
        r"HKCU\Software\Classes\Local Settings\Software\Microsoft\Windows\Shell\MuiCache";

    #[test]
    fn a_value_that_points_into_the_apps_folder_is_proposed_as_a_value() {
        let c = cand(
            TraceKind::RegistryValue,
            MUI,
            Some(r"C:\Program Files\Kdenlive\bin\kdenlive.exe.FriendlyAppName"),
            Some("Kdenlive"),
        );
        let t = judge(&c, &[]).unwrap();
        assert_eq!(t.kind, TraceKind::RegistryValue);
        assert_eq!(t.location, MUI);
        assert_eq!(t.evidence, Evidence::PathReference);
        assert!(t.value.is_some());
    }

    #[test]
    fn a_type_library_and_the_shared_files_and_folders_of_an_installer_are_taken_through_the_folder(
    ) {
        let key =
            |location: &str, text: &str| cand(TraceKind::RegistryKey, location, None, Some(text));
        let lib = key(
            r"HKLM\SOFTWARE\Classes\TypeLib\{ABC}",
            r"C:\Program Files\Kdenlive\lib.tlb",
        );
        assert_eq!(evidence(&lib), Evidence::ComServer);
        let elsewhere = key(
            r"HKLM\SOFTWARE\Classes\TypeLib\{ABC}",
            r"C:\Program Files\Kdenlive2\lib.tlb",
        );
        assert!(judge(&elsewhere, &[]).is_err());
        let other_place = key(
            r"HKLM\SOFTWARE\Classes\Interface\{ABC}",
            r"C:\Program Files\Kdenlive\lib.tlb",
        );
        assert!(judge(&other_place, &[]).is_err());
        for (place, name) in [
            (
                r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\SharedDLLs",
                r"C:\Program Files\Kdenlive\bin\x.dll",
            ),
            (
                r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Installer\Folders",
                r"C:\Program Files\Kdenlive\",
            ),
        ] {
            let value = cand(TraceKind::RegistryValue, place, Some(name), None);
            assert_eq!(evidence(&value), Evidence::PathReference, "{place}");
            let neighbour = cand(
                TraceKind::RegistryValue,
                place,
                Some(r"C:\Program Files\Kdenlive2\x.dll"),
                None,
            );
            assert!(judge(&neighbour, &[]).is_err(), "{place}");
        }
    }

    #[test]
    fn a_com_class_and_a_context_menu_entry_are_taken_only_through_the_apps_folder() {
        let key =
            |location: &str, text: &str| cand(TraceKind::RegistryKey, location, None, Some(text));
        let inside = r"C:\Program Files\Kdenlive\shell.dll";
        let class = key(r"HKLM\SOFTWARE\Classes\CLSID\{ABC}", inside);
        assert_eq!(evidence(&class), Evidence::ComServer);
        let wow = key(r"HKLM\SOFTWARE\WOW6432Node\Classes\CLSID\{ABC}", inside);
        assert_eq!(evidence(&wow), Evidence::ComServer);
        for place in [
            r"HKLM\SOFTWARE\Classes\*\shell\Abrir con Kdenlive",
            r"HKCU\Software\Classes\Directory\Background\shell\kdenlive",
            r"HKLM\SOFTWARE\Classes\*\shellex\ContextMenuHandlers\KdenliveMenu",
        ] {
            assert_eq!(
                evidence(&key(place, inside)),
                Evidence::ContextMenu,
                "{place}"
            );
        }
        // The folder of another app, a server elsewhere or an unknown place: nothing.
        for (place, text) in [
            (
                r"HKLM\SOFTWARE\Classes\CLSID\{ABC}",
                r"C:\Program Files\Kdenlive2\a.dll",
            ),
            (
                r"HKLM\SOFTWARE\Classes\CLSID\{ABC}",
                r"C:\Windows\System32\shell32.dll",
            ),
            (r"HKLM\SOFTWARE\Classes\Foo\shell\x", inside),
            (r"HKLM\SOFTWARE\Classes\CLSID\{ABC}\InprocServer32", inside),
        ] {
            assert!(judge(&key(place, text), &[]).is_err(), "{place}");
        }
        let shared = InstalledApp {
            install_location: Some(DIR.into()),
            ..app("Other app", None)
        };
        assert!(judge(&class, &[shared]).is_err());
    }

    #[test]
    fn a_firewall_rule_is_taken_when_its_program_is_in_the_apps_folder_and_not_by_its_name() {
        const RULES: &str = r"HKLM\SYSTEM\CurrentControlSet\Services\SharedAccess\Parameters\FirewallPolicy\FirewallRules";
        let rule = |name: &str, app: &str, key: &str| {
            let text = format!("v2.33|Action=Allow|Dir=In|App={app}|Name={name}|");
            cand(TraceKind::RegistryValue, key, Some(name), Some(&text))
        };
        let own = rule("{3A1F}", r"C:\Program Files\Kdenlive\bin\melt.exe", RULES);
        let t = judge(&own, &[]).unwrap();
        assert_eq!(t.evidence, Evidence::PathReference);
        assert_eq!(t.value.as_deref(), Some("{3A1F}"));
        assert!(t.needs_admin());
        let named = rule("Kdenlive", r"C:\Windows\System32\svchost.exe", RULES);
        assert_eq!(judge(&named, &[]), Err(Refusal::NotTheApps));
        let other = rule("x", r"C:\Program Files\Kdenlive2\a.exe", RULES);
        assert_eq!(judge(&other, &[]), Err(Refusal::NotTheApps));
        // The same text under another key is not a rule of the firewall.
        let elsewhere = rule(
            "x",
            r"C:\Program Files\Kdenlive\a.exe",
            r"HKLM\SOFTWARE\Foo",
        );
        assert_eq!(judge(&elsewhere, &[]), Err(Refusal::NotTheApps));
    }

    #[test]
    fn what_starts_with_windows_is_taken_when_it_runs_a_program_of_the_apps_folder() {
        let command = r#""C:\Program Files\Kdenlive\update.exe" /silent"#;
        for key in [
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
            r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce",
            r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run",
        ] {
            let c = cand(
                TraceKind::RegistryValue,
                key,
                Some("KdenliveUpdater"),
                Some(command),
            );
            assert_eq!(evidence(&c), Evidence::PathReference, "{key}");
        }
        let other = cand(
            TraceKind::RegistryValue,
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
            Some("Otra"),
            Some(r"C:\Program Files\Kdenlive2\a.exe"),
        );
        assert_eq!(judge(&other, &[]), Err(Refusal::NotTheApps));
    }

    #[test]
    fn a_value_in_a_key_off_the_list_or_of_another_app_is_left_alone() {
        let name = r"C:\Program Files\Kdenlive\bin\k.exe";
        for key in [
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\Run",
            r"HKCU\Software\Whatever",
            r"HKLM\SYSTEM\CurrentControlSet\Services\x",
        ] {
            let c = cand(TraceKind::RegistryValue, key, Some(name), None);
            assert_eq!(judge(&c, &[]), Err(Refusal::NotTheApps), "{key}");
        }
        let kdenlive2 = cand(
            TraceKind::RegistryValue,
            MUI,
            Some(r"C:\Program Files\Kdenlive2\bin\k.exe.FriendlyAppName"),
            None,
        );
        assert_eq!(judge(&kdenlive2, &[]), Err(Refusal::NotTheApps));
        let nameless = cand(
            TraceKind::RegistryValue,
            MUI,
            Some("kdenlive"),
            Some("Kdenlive"),
        );
        assert_eq!(judge(&nameless, &[]), Err(Refusal::NotTheApps));
        let no_value = cand(TraceKind::RegistryValue, MUI, None, Some(name));
        assert_eq!(judge(&no_value, &[]), Err(Refusal::NotTheApps));
    }

    #[test]
    fn a_folder_that_another_installed_app_owns_gives_no_references() {
        let inner = app(
            "Kdenlive Plugin",
            Some(r"C:\Program Files\Kdenlive\plugins"),
        );
        let c = cand(
            TraceKind::RegistryValue,
            MUI,
            Some(r"C:\Program Files\Kdenlive\bin\k.exe.FriendlyAppName"),
            None,
        );
        assert_eq!(judge(&c, &[inner]), Err(Refusal::SharedWithOtherApp));
    }

    #[test]
    fn the_registered_extension_and_its_progid_are_taken_and_txt_is_not() {
        let ext = cand(
            TraceKind::RegistryKey,
            r"HKLM\SOFTWARE\Classes\.kdenlive",
            None,
            None,
        );
        assert_eq!(evidence(&ext), Evidence::AssociatedExtension);
        let progid = cand(
            TraceKind::RegistryKey,
            r"HKLM\SOFTWARE\Classes\Kdenlive",
            None,
            Some(r#""C:\Program Files\Kdenlive\bin\kdenlive.exe" "%1""#),
        );
        assert_eq!(evidence(&progid), Evidence::Association);
        let txt = cand(
            TraceKind::RegistryKey,
            r"HKLM\SOFTWARE\Classes\.txt",
            None,
            Some(r"C:\Program Files\Kdenlive\bin\kdenlive.exe"),
        );
        assert!(judge(&txt, &[]).is_err());
        let other = cand(
            TraceKind::RegistryKey,
            r"HKLM\SOFTWARE\Classes\txtfile",
            None,
            Some(r"C:\Windows\system32\notepad.exe"),
        );
        assert!(judge(&other, &[]).is_err());
        let deep = cand(
            TraceKind::RegistryKey,
            r"HKLM\SOFTWARE\Classes\Kdenlive\shell",
            None,
            Some(DIR),
        );
        assert!(judge(&deep, &[]).is_err());
    }

    #[test]
    fn keys_of_explorer_hang_from_the_apps_extension() {
        let base = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer";
        for key in [
            format!(r"{base}\FileExts\.kdenlive"),
            format!(r"{base}\RecentDocs\.kdenlive"),
            format!(r"{base}\ComDlg32\OpenSavePidlMRU\kdenlive"),
        ] {
            let c = cand(TraceKind::RegistryKey, &key, None, None);
            assert_eq!(evidence(&c), Evidence::AssociatedExtension, "{key}");
        }
        for key in [
            format!(r"{base}\FileExts\.txt"),
            format!(r"{base}\FileExts"),
            format!(r"{base}\FileExts\.kdenlive\OpenWithList"),
        ] {
            let c = cand(TraceKind::RegistryKey, &key, None, None);
            assert!(judge(&c, &[]).is_err(), "{key}");
        }
    }

    #[test]
    fn program_keys_need_the_apps_name_and_other_microsoft_keys_stay_closed() {
        let cv = r"HKCU\Software\Microsoft\Windows\CurrentVersion";
        let ok = cand(
            TraceKind::RegistryKey,
            &format!(r"{cv}\Notifications\Settings\kdenlive"),
            None,
            None,
        );
        assert_eq!(evidence(&ok), Evidence::NameMatch);
        let other = cand(
            TraceKind::RegistryKey,
            &format!(r"{cv}\Notifications\Settings\Microsoft.Windows.Photos"),
            None,
            None,
        );
        assert!(judge(&other, &[]).is_err());
        let named = cand(
            TraceKind::RegistryKey,
            r"HKCU\Software\Microsoft\Kdenlive",
            None,
            None,
        );
        assert_eq!(judge(&named, &[]), Err(Refusal::SystemPlace));
    }

    #[test]
    fn a_toast_of_the_apps_extension_and_a_start_tile_with_its_name_are_taken() {
        let cv = r"HKCU\Software\Microsoft\Windows\CurrentVersion";
        let toast = |name: &str| {
            cand(
                TraceKind::RegistryValue,
                &format!(r"{cv}\ApplicationAssociationToasts"),
                Some(name),
                None,
            )
        };
        assert_eq!(
            evidence(&toast("Kdenlive_.kdenlive")),
            Evidence::AssociatedExtension
        );
        assert_eq!(
            evidence(&toast("kdenlive_.KDENLIVE")),
            Evidence::AssociatedExtension
        );
        for other in [
            "Kdenlive_.txt",
            "txtfile_.txt",
            "Kdenlive",
            "Kdenlive_.kdenlive2",
        ] {
            assert!(judge(&toast(other), &[]).is_err(), "{other}");
        }
        let tile = |name: &str| {
            cand(
                TraceKind::RegistryKey,
                &format!(r"{cv}\Start\TileProperties\W~{{6D80}}\{name}"),
                None,
                None,
            )
        };
        assert_eq!(evidence(&tile("Kdenlive")), Evidence::NameMatch);
        assert!(
            judge(&tile("Photos"), &[]).is_err(),
            "a tile of another app"
        );
        let direct = cand(
            TraceKind::RegistryKey,
            &format!(r"{cv}\Start\TileProperties\Kdenlive"),
            None,
            None,
        );
        assert!(judge(&direct, &[]).is_err(), "only inside a W~ group");
    }

    #[test]
    fn settings_files_with_the_apps_name_and_a_known_suffix_are_taken() {
        for name in [
            "kdenliverc",
            "kdenlivestaterc",
            "kdenlive.ini",
            "Kdenlive.json",
        ] {
            let c = cand(
                TraceKind::File,
                &format!(r"{USER}\Local\{name}"),
                None,
                None,
            );
            assert_eq!(evidence(&c), Evidence::NamePrefix, "{name}");
        }
        let state = cand(
            TraceKind::File,
            &format!(r"{USER}\Local\State\kdenlivestaterc"),
            None,
            None,
        );
        assert_eq!(evidence(&state), Evidence::NamePrefix);
        for bad in [
            format!(r"{USER}\Local\kdenlive2rc"),
            format!(r"{USER}\Local\kdenlive.exe"),
            format!(r"{USER}\Local\kdenlive-backup.ini"),
            format!(r"{USER}\Local\Other\kdenliverc"),
            r"C:\Temp\kdenliverc".to_string(),
        ] {
            let c = cand(TraceKind::File, &bad, None, None);
            assert!(judge(&c, &[]).is_err(), "{bad}");
        }
    }

    #[test]
    fn crash_dumps_of_the_apps_programs_are_taken() {
        let dumps = format!(r"{USER}\Local\CrashDumps");
        for name in ["kdenlive.exe.33876.dmp", "kdenlive.exe(1).33876.dmp"] {
            let c = cand(TraceKind::File, &format!(r"{dumps}\{name}"), None, None);
            assert_eq!(evidence(&c), Evidence::CrashDump, "{name}");
        }
        for name in [
            "other.exe.12.dmp",
            "kdenlive.exe.dmp",
            "kdenlive.exe.12.txt",
            "xkdenlive.exe.12.dmp",
        ] {
            let c = cand(TraceKind::File, &format!(r"{dumps}\{name}"), None, None);
            assert!(judge(&c, &[]).is_err(), "{name}");
        }
    }

    #[test]
    fn recent_and_desktop_shortcuts_need_the_extension_or_a_target_in_the_folder() {
        let recent = format!(r"{USER}\Roaming\Microsoft\Windows\Recent");
        let video = cand(
            TraceKind::File,
            &format!(r"{recent}\Video.kdenlive.lnk"),
            None,
            None,
        );
        assert_eq!(evidence(&video), Evidence::AssociatedExtension);
        let txt = cand(
            TraceKind::File,
            &format!(r"{recent}\Notes.txt.lnk"),
            None,
            None,
        );
        assert!(judge(&txt, &[]).is_err());
        let desk = cand(
            TraceKind::File,
            r"C:\Users\ana\Desktop\Editor.lnk",
            None,
            Some(r"C:\Program Files\Kdenlive\bin\kdenlive.exe"),
        );
        assert_eq!(evidence(&desk), Evidence::PathReference);
        let elsewhere = cand(
            TraceKind::File,
            r"C:\Users\ana\Desktop\Editor.lnk",
            None,
            Some(r"C:\Program Files\Kdenlive2\bin\k.exe"),
        );
        assert!(judge(&elsewhere, &[]).is_err());
    }

    #[test]
    fn without_a_usable_seed_nothing_is_taken_by_reference() {
        let none = Seeds::default();
        let c = cand(
            TraceKind::RegistryValue,
            MUI,
            Some(r"C:\Program Files\Kdenlive\bin\k.exe"),
            None,
        );
        assert!(judge_with(&app("Kdenlive", None), &[], &none, &c).is_err());
    }

    #[test]
    fn two_values_of_one_key_are_both_proposed_and_one_value_is_not_hidden_by_its_key() {
        let a = cand(
            TraceKind::RegistryValue,
            MUI,
            Some(r"C:\Program Files\Kdenlive\a.exe.x"),
            None,
        );
        let b = cand(
            TraceKind::RegistryValue,
            MUI,
            Some(r"C:\Program Files\Kdenlive\b.exe.x"),
            None,
        );
        let got = propose_with(&app("Kdenlive", None), &[], &seeds(), &[a.clone(), b, a]);
        assert_eq!(got.len(), 2);
    }
}
