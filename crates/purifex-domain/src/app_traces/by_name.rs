//! Deciding by name: a place that carries the app's name (or is the folder it registered) is
//! proposed unless it is Windows', too broad or something another app needs; one that only
//! resembles the name is a guess, proposed only when asked for.

use super::{last_part, simple, Candidate, Evidence, Refusal, Trace, TraceKind, BROAD};
use crate::apps::InstalledApp;

/// Places that belong to Windows, whatever their name.
const SYSTEM_PREFIXES: [&str; 5] = [
    r"c:\windows",
    r"hklm\software\microsoft",
    r"hkcu\software\microsoft",
    r"hklm\system",
    r"hklm\software\classes",
];

/// The shortest name (simplified) that may be taken for a resemblance: with fewer letters
/// almost anything contains it.
const MIN_PARTIAL: usize = 5;

/// Whether a place named `own` (simplified) is worth looking at for an app that goes by
/// `names`: the same name, or one that contains the app's whole name (`discordptb` for
/// `discord`). Never the other way round: `google` is the folder of everything of the
/// publisher of `Google Chrome`. The rules decide what it is.
pub fn names_relate(names: &[String], own: &str) -> bool {
    names.iter().any(|n| {
        n == own || (n.len() >= MIN_PARTIAL && own.len() >= MIN_PARTIAL && own.contains(n.as_str()))
    })
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

/// The classic rule: a place with the app's name (or the folder it registered), that is
/// not Windows', not too broad and not something another installed app needs.
pub(super) fn judge_by_name(
    app: &InstalledApp,
    others: &[InstalledApp],
    candidate: &Candidate,
    location: &str,
    guess: bool,
) -> Result<Trace, Refusal> {
    let lower = location.to_lowercase();
    refuse_system_place(&lower)?;
    let own = simple(last_part(location));
    refuse_too_broad(&lower, &own)?;
    refuse_shared(app, others, &lower, &own)?;
    let names = names_of(app);
    if candidate.registered_install || names.contains(&own) {
        return Ok(trace_by_name(
            candidate,
            location,
            &lower,
            Evidence::NameMatch,
        ));
    }
    // A resemblance is a guess, and it is refused where another app has the same claim.
    if guess && names_relate(&names, &own) {
        let claimed = others
            .iter()
            .filter(|o| o.id != app.id)
            .any(|o| names_relate(&names_of(o), &own));
        if claimed {
            return Err(Refusal::SharedWithOtherApp);
        }
        return Ok(trace_by_name(
            candidate,
            location,
            &lower,
            Evidence::PartialName,
        ));
    }
    Err(Refusal::NotTheApps)
}

/// A place a log of installation names: Windows' places and the too broad are refused as
/// always, and nothing else is asked (the log is the evidence). The log must be of this app:
/// the caller only offers the places of the log of the app being cleaned.
pub(super) fn judge_known(
    candidate: &Candidate,
    location: &str,
    evidence: Evidence,
) -> Result<Trace, Refusal> {
    let lower = location.to_lowercase();
    refuse_system_place(&lower)?;
    refuse_too_broad(&lower, &simple(last_part(location)))?;
    let user_data = candidate.kind == TraceKind::File && lower.contains(r"ppdata\");
    Ok(Trace {
        kind: candidate.kind,
        location: location.to_string(),
        bytes: candidate.bytes,
        why: evidence.label().to_string(),
        user_data,
        value: candidate
            .value
            .clone()
            .filter(|_| candidate.kind == TraceKind::RegistryValue),
        evidence,
    })
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
fn trace_by_name(candidate: &Candidate, location: &str, lower: &str, by_name: Evidence) -> Trace {
    let registered = candidate.registered_install;
    let user_data = !registered
        && ["\\appdata\\", "\\documents\\", "\\saved games\\"]
            .iter()
            .any(|p| lower.contains(p));
    let why = if registered {
        "la carpeta de instalación que registró la app".to_string()
    } else if by_name == Evidence::PartialName {
        format!(
            "posible: el nombre se parece al de la app: {}",
            last_part(location)
        )
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
            by_name
        },
        ..Default::default()
    }
}
