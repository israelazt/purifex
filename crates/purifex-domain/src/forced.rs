//! A forced uninstall: the person points at what is left of a broken or half-removed app
//! (a name, a folder or a `.exe`) and the program treats it as an app that is gone. Only
//! the reading of that text is here; the rules of what may be touched are the ones every
//! app goes through (seeds, `names_of`, `judge`), so pointing at a folder opens no door.

use crate::app_traces::{last_part, names_of};
use crate::apps::InstalledApp;
use crate::seeds::{seeds_of, Seeds};

/// Parts of a path that belong to Windows or to what it shares: never the leftovers of one app.
fn is_system_part(part: &str) -> bool {
    let p = part.to_lowercase();
    p.starts_with("windows")
        || p.starts_with("microsoft")
        || p == "common files"
        || p == "system32"
        || p == "syswow64"
}

/// Whether the text is an absolute path (`C:\...`) and not a name.
pub fn is_path(text: &str) -> bool {
    let b = text.trim().trim_matches('"').as_bytes();
    b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/')
}

/// The app that stands for what the person wrote, never seen installed: its name, or the
/// folder (or the folder of the `.exe`) it left. Refused when the text is too broad to be
/// safe: a drive, `Program Files`, a folder of Windows, a name too short or too common.
pub fn forced_app(input: &str) -> Result<InstalledApp, String> {
    let text = input.trim().trim_matches('"').trim();
    if text.is_empty() {
        return Err("escribe el nombre, la carpeta o el .exe de la app".into());
    }
    if !is_path(text) {
        let app = InstalledApp {
            id: format!("name:{text}"),
            name: text.to_string(),
            ..Default::default()
        };
        return if names_of(&app).is_empty() {
            Err(format!(
                "«{text}» es demasiado corto o genérico para buscar sus restos sin riesgo"
            ))
        } else {
            Ok(app)
        };
    }
    let path = text.replace('/', "\\");
    let path = path.trim_end_matches('\\');
    let is_exe = path.to_lowercase().ends_with(".exe");
    let folder = if is_exe {
        path.rsplit_once('\\').map_or(path, |(parent, _)| parent)
    } else {
        path
    };
    if folder.split('\\').any(is_system_part) {
        return Err(format!(
            "«{text}» es parte de Windows o de lo que comparten los programas: no se toca"
        ));
    }
    let exes: Vec<String> = is_exe
        .then(|| last_part(path).to_string())
        .into_iter()
        .collect();
    let seeds = Seeds::new(Some(folder), &exes, &[]);
    let mut app = InstalledApp {
        id: format!("path:{folder}"),
        name: last_part_cased(folder).to_string(),
        install_location: Some(folder.to_string()),
        observed: seeds,
        ..Default::default()
    };
    if seeds_of(&app).folder.is_none() {
        return Err(format!(
            "«{text}» es una carpeta demasiado amplia (o no es una ruta completa): indica la carpeta de la app misma"
        ));
    }
    app.name = app.name.trim().to_string();
    Ok(app)
}

/// The installed app whose folder is the one pointed at, or holds it, or is inside it: its
/// files are in use, so it is never treated as an app that is gone.
pub fn installed_there<'a>(
    app: &InstalledApp,
    all: &'a [InstalledApp],
) -> Option<&'a InstalledApp> {
    let mine = seeds_of(app).folder?.to_lowercase();
    let inside = |outer: &str, inner: &str| {
        inner == outer
            || inner
                .strip_prefix(outer)
                .is_some_and(|rest| rest.starts_with('\\'))
    };
    all.iter().find(|other| {
        seeds_of(other).folder.is_some_and(|f| {
            let f = f.to_lowercase();
            inside(&f, &mine) || inside(&mine, &f)
        })
    })
}

/// The last part of a path keeping its capitals (`last_part` lowercases).
fn last_part_cased(path: &str) -> &str {
    path.rsplit('\\').next().unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_becomes_an_app_gone_and_a_generic_one_is_refused() {
        let app = forced_app("  Kdenlive ").unwrap();
        assert_eq!(
            (app.name.as_str(), app.id.as_str()),
            ("Kdenlive", "name:Kdenlive")
        );
        assert!(forced_app("Go").unwrap_err().contains("demasiado corto"));
        assert!(forced_app("   ").is_err());
    }

    #[test]
    fn a_folder_is_the_apps_folder_and_its_last_part_the_name() {
        let app = forced_app(r#""D:\Games\Old Tool\""#).unwrap();
        assert_eq!(app.name, "Old Tool");
        assert_eq!(app.install_location.as_deref(), Some(r"D:\Games\Old Tool"));
        assert_eq!(seeds_of(&app).folder.as_deref(), Some(r"D:\Games\Old Tool"));
    }

    #[test]
    fn an_exe_points_at_its_folder_and_is_kept_as_a_program_of_the_app() {
        let app = forced_app(r"C:\Program Files\Acme Tool\bin\acme.exe").unwrap();
        assert_eq!(
            app.install_location.as_deref(),
            Some(r"C:\Program Files\Acme Tool\bin")
        );
        assert_eq!(seeds_of(&app).exes, ["acme.exe"]);
    }

    #[test]
    fn nothing_broad_or_of_windows_is_accepted() {
        for broad in [
            r"C:\",
            r"C:\Program Files",
            r"C:\Users\me",
            r"C:\Windows\System32\drivers",
            r"C:\Program Files\Windows Defender",
            r"C:\Program Files\Common Files\Acme",
            r"C:\Program Files\Microsoft Office",
            r"C:\Users\me\AppData\Local",
            r"C:\Users\me\..\..\Windows",
        ] {
            assert!(forced_app(broad).is_err(), "{broad} must be refused");
        }
    }

    #[test]
    fn a_folder_of_an_installed_app_is_found_in_either_direction() {
        let kept = InstalledApp {
            name: "Kept".into(),
            install_location: Some(r"C:\Program Files\Kept".into()),
            ..Default::default()
        };
        let all = [kept];
        for path in [
            r"C:\Program Files\Kept",
            r"C:\Program Files\kept\bin",
            r"C:\Program Files\Kept\bin\x.exe",
        ] {
            assert!(
                installed_there(&forced_app(path).unwrap(), &all).is_some(),
                "{path}"
            );
        }
        for path in [r"C:\Program Files\Kept Two", r"D:\Old\Kept"] {
            assert!(installed_there(&forced_app(path).unwrap(), &all).is_none());
        }
    }

    #[test]
    fn a_path_is_told_from_a_name() {
        assert!(is_path(r"C:\x\y") && is_path(r#""d:/x""#));
        assert!(!is_path("Kdenlive") && !is_path(r"\\server\share") && !is_path("C:"));
    }
}
