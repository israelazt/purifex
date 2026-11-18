//! Guards of the architecture (see `ARCHITECTURE.md`). They fail when
//! someone breaks a rule the design depends on, so the rules do not rot.

use std::fs;
use std::path::{Path, PathBuf};

/// The root folder of the workspace.
fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// Where a crate lives: the libraries under `crates/`, the programs under `apps/`.
fn crate_dir(krate: &str) -> PathBuf {
    let app = workspace().join("apps").join(krate);
    if app.is_dir() {
        app
    } else {
        workspace().join("crates").join(krate)
    }
}

/// Every folder with Rust sources of the workspace.
fn source_roots() -> Vec<PathBuf> {
    ["crates", "apps"]
        .iter()
        .map(|d| workspace().join(d))
        .filter(|d| d.is_dir())
        .collect()
}

/// Names of the crates a manifest depends on (all `[*dependencies]` tables).
fn dependencies_of(krate: &str) -> Vec<String> {
    let manifest = crate_dir(krate).join("Cargo.toml");
    let text = fs::read_to_string(&manifest).unwrap();
    let mut in_dependencies = false;
    let mut names = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            let table = line.trim_matches(|c| c == '[' || c == ']');
            in_dependencies = table == "dependencies"
                || table == "build-dependencies"
                || table.ends_with(".dependencies");
        } else if in_dependencies && !line.starts_with('#') {
            let is_entry = line.contains('=') && !line.starts_with('"') && !line.starts_with(' ');
            if let (true, Some(name)) = (is_entry, line.split(['=', '.', ' ']).next()) {
                if !name.is_empty() && !name.starts_with("Win32_") {
                    names.push(name.to_string());
                }
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

#[test]
fn each_crate_depends_only_on_what_the_layers_allow() {
    // crate -> every dependency it may have. Dependencies go only downward:
    // domain <- platform, app <- the binary.
    let allowed: [(&str, &[&str]); 5] = [
        ("purifex-domain", &["serde", "serde_json"]),
        ("purifex-platform", &["purifex-domain", "windows"]),
        ("purifex-app", &["purifex-domain", "serde_json"]),
        // The page: files included in the program, no dependencies at all.
        ("purifex-ui", &[]),
        (
            "purifex",
            &[
                "purifex-domain",
                "purifex-platform",
                "purifex-app",
                "purifex-ui",
            ],
        ),
    ];
    for (krate, may) in allowed {
        assert!(
            may.is_empty() || !dependencies_of(krate).is_empty(),
            "no dependencies read for {krate}: the parser is broken"
        );
        for dep in dependencies_of(krate) {
            assert!(
                may.contains(&dep.as_str()),
                "{krate} must not depend on {dep} (allowed: {may:?})"
            );
        }
    }
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Lines with code in the production part of a file (before its
/// `#[cfg(test)] mod tests`).
fn production_lines(text: &str) -> usize {
    before_the_tests_module(text)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count()
}

#[test]
fn no_source_file_is_too_big() {
    const LIMIT: usize = 400;
    const TESTS_LIMIT: usize = 1000;
    let mut files = Vec::new();
    for root in source_roots() {
        rust_files(&root, &mut files);
    }
    let mut too_big = Vec::new();
    for path in files {
        let relative = path.strip_prefix(workspace()).unwrap().to_path_buf();
        // Examples and integration tests are not product code.
        if relative.components().any(|c| {
            let c = c.as_os_str();
            c == "examples" || c == "tests" && !relative.ends_with("tests.rs")
        }) {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap();
        let (count, limit) = if path.file_name().is_some_and(|n| n == "tests.rs") {
            (
                text.lines().filter(|l| !l.trim().is_empty()).count(),
                TESTS_LIMIT,
            )
        } else {
            (production_lines(&text), LIMIT)
        };
        if count > limit {
            too_big.push(format!(
                "{} has {count} lines (limit {limit})",
                relative.display()
            ));
        }
    }
    assert!(
        too_big.is_empty(),
        "split these files:\n{}",
        too_big.join("\n")
    );
}

/// The part of a file before its `#[cfg(test)] mod tests`. Other items marked
/// `#[cfg(test)]` (a helper module declared in `lib.rs`, say) do not end the
/// production code.
fn before_the_tests_module(text: &str) -> &str {
    let mut from = 0;
    while let Some(found) = text[from..].find("#[cfg(test)]") {
        let at = from + found;
        let after = text[at + "#[cfg(test)]".len()..].trim_start();
        if after.starts_with("mod tests") {
            return &text[..at];
        }
        from = at + 1;
    }
    text
}

/// Where the program may still print straight to the console, and why. Every
/// other message goes through the log (`log_info!`, `log_warn!`, `log_error!`),
/// which reaches the file and, when there is one, the console.
const PRINT_EXCEPTIONS: [(&str, &str); 1] = [(
    "apps/purifex/src/main.rs",
    "the report is the program's output: it goes to the standard output, for people and scripts",
)];

#[test]
fn production_code_logs_instead_of_printing() {
    let mut files = Vec::new();
    for root in source_roots() {
        rust_files(&root, &mut files);
    }
    let mut offenders = Vec::new();
    for path in files {
        let relative = path.strip_prefix(workspace()).unwrap().to_path_buf();
        let shown = relative.to_string_lossy().replace('\\', "/");
        // Examples and tests may print; so may the files listed above.
        if relative
            .components()
            .any(|c| c.as_os_str() == "examples" || c.as_os_str() == "tests")
            || shown.ends_with("/tests.rs")
            || PRINT_EXCEPTIONS
                .iter()
                .any(|(allowed, _)| shown.ends_with(allowed))
        {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap();
        let production = before_the_tests_module(&text);
        for (number, line) in production.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            if ["println!(", "eprintln!(", "print!(", "eprint!("]
                .iter()
                .any(|p| code.contains(p))
            {
                offenders.push(format!("{shown}:{}", number + 1));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "use log_info!/log_warn!/log_error! instead of printing (or add an exception with its reason):\n{}",
        offenders.join("\n")
    );
}

#[test]
fn only_the_tests_module_ends_the_production_code() {
    let file = "fn a() {}
#[cfg(test)]
mod testing;
fn b() {}
#[cfg(test)]
mod tests {
fn c() {}
}
";
    let production = before_the_tests_module(file);
    assert!(production.contains("fn b()"));
    assert!(!production.contains("fn c()"));
    assert_eq!(before_the_tests_module("fn only() {}"), "fn only() {}");
}

#[test]
fn unsafe_lives_only_in_platform() {
    let mut files = Vec::new();
    for root in source_roots() {
        rust_files(&root, &mut files);
    }
    let mut offenders = Vec::new();
    for path in files {
        let relative = path.strip_prefix(workspace()).unwrap().to_path_buf();
        let shown = relative.to_string_lossy().replace('\\', "/");
        // Tests and examples are not product code (and this file names the keywords).
        if shown.starts_with("crates/purifex-platform/")
            || relative
                .components()
                .any(|c| c.as_os_str() == "tests" || c.as_os_str() == "examples")
        {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap();
        for (number, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            if ["unsafe {", "unsafe fn", "unsafe impl", "unsafe extern"]
                .iter()
                .any(|p| code.contains(p))
            {
                offenders.push(format!("{shown}:{}", number + 1));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "unsafe code belongs in purifex-platform, behind a port:
{}",
        offenders.join(
            "
"
        )
    );
}
