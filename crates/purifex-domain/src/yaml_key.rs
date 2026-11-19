//! Reads and changes **one value** of a YAML file without touching anything else:
//! the rest of the file stays byte for byte as it was (line ends, indentation,
//! order, comments). It is not a YAML parser: it follows the nesting by the
//! indentation, which is how the configuration files of the games are written,
//! and refuses (returns an error) whatever it does not understand, so it never
//! invents a structure.

/// Lines with their line ends, so a rewrite keeps `\r\n` or `\n` as it was.
fn lines_of(text: &str) -> Vec<&str> {
    text.split_inclusive('\n').collect()
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

fn is_blank(line: &str) -> bool {
    let t = line.trim();
    t.is_empty() || t.starts_with('#')
}

/// Where a key is: the line, and the first and last line of its block.
struct Found {
    line: usize,
    /// The lines after it that belong to it (more indented).
    block_end: usize,
}

/// The line of `path` (`["a", "b", "c"]` is `c` inside `b` inside `a`).
fn locate(lines: &[&str], path: &[&str]) -> Option<Found> {
    let (mut from, mut to) = (0usize, lines.len());
    let mut parent_indent: Option<usize> = None;
    let mut found = None;
    for segment in path {
        let key = format!("{segment}:");
        let mut child_indent = None;
        let mut hit = None;
        for (i, line) in lines.iter().enumerate().take(to).skip(from) {
            if is_blank(line) {
                continue;
            }
            let indent = indent_of(line);
            if parent_indent.is_some_and(|p| indent <= p) {
                break;
            }
            let expected = *child_indent.get_or_insert(indent);
            if indent == expected && line[indent..].starts_with(&key) {
                hit = Some((i, indent));
                break;
            }
        }
        let (i, indent) = hit?;
        let block_end = (i + 1..to)
            .find(|&j| !is_blank(lines[j]) && indent_of(lines[j]) <= indent)
            .unwrap_or(to);
        found = Some(Found { line: i, block_end });
        (from, to, parent_indent) = (i + 1, block_end, Some(indent));
    }
    found
}

/// The value written after the last key of `path`, without quotes. `None` if the
/// key is not there (or has no value on its line).
pub fn get(text: &str, path: &[&str]) -> Option<String> {
    let lines = lines_of(text);
    let found = locate(&lines, path)?;
    let line = lines[found.line].trim_end_matches(['\r', '\n']);
    let value = line.split_once(':')?.1.split(" #").next()?.trim();
    (!value.is_empty()).then(|| value.trim_matches('"').to_string())
}

/// The text with the value of `path` set to `value`. If the last key is missing
/// but the block that holds it exists, the key is added with the indentation of
/// its siblings; if the block is missing too, it is an error (the structure is
/// not invented).
pub fn set(text: &str, path: &[&str], value: &str) -> Result<String, String> {
    let (key, parents) = path.split_last().ok_or("falta la ruta de la clave")?;
    let lines = lines_of(text);
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut out: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    if let Some(found) = locate(&lines, path) {
        let line = lines[found.line];
        let ending = &line[line.trim_end_matches(['\r', '\n']).len()..];
        let head = &line[..line.find(':').map_or(0, |c| c + 1)];
        out[found.line] = format!("{head} {value}{ending}");
        return Ok(out.concat());
    }
    let parent = if parents.is_empty() {
        None
    } else {
        locate(&lines, parents)
    };
    let Some(parent) = parent else {
        return Err(format!(
            "no existe «{}» en el archivo: no se inventa la estructura",
            parents.join(" › ")
        ));
    };
    // The indentation of the siblings: that of the first line inside the block.
    let child_indent = (parent.line + 1..parent.block_end)
        .find(|&j| !is_blank(lines[j]))
        .map(|j| indent_of(lines[j]))
        .ok_or("el bloque está vacío: no se sabe con qué sangría añadir la clave")?;
    // After the last line with content of the block.
    let after = (parent.line + 1..parent.block_end)
        .rev()
        .find(|&j| !is_blank(lines[j]))
        .unwrap_or(parent.line)
        + 1;
    let mut added = format!("{}{key}: {value}{newline}", " ".repeat(child_indent));
    // The line before may lack its own line end (the last of the file).
    if after > 0 && !out[after - 1].ends_with('\n') {
        out[after - 1].push_str(newline);
        added = added.trim_end_matches(['\r', '\n']).to_string();
    }
    out.insert(after, added);
    Ok(out.concat())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shape of `LCULocalPreferences.yaml`, with its CRLF line ends.
    fn file() -> String {
        [
            "lol-audio:",
            "    data:",
            "        masterVolume: 21",
            "    schemaVersion: 5",
            "lol-clash:",
            "    data:",
            "        simpleStateFlagIds:",
            "        - \"registration_open_tournament_88561\"",
            "    schemaVersion: 1",
            "lol-user-experience:",
            "    data:",
            "        closeLeagueClientDuringGame: false",
            "        hasBeenPromptedForPotatoMode: true",
            "    schemaVersion: 3",
            "",
        ]
        .join("\r\n")
    }

    const PATH: [&str; 3] = ["lol-user-experience", "data", "closeLeagueClientDuringGame"];

    #[test]
    fn it_reads_a_value_by_its_path() {
        assert_eq!(get(&file(), &PATH).as_deref(), Some("false"));
        assert_eq!(
            get(&file(), &["lol-audio", "data", "masterVolume"]).as_deref(),
            Some("21")
        );
        assert_eq!(
            get(&file(), &["lol-user-experience", "schemaVersion"]).as_deref(),
            Some("3")
        );
    }

    #[test]
    fn a_key_in_another_block_with_the_same_name_is_not_confused() {
        let text = "a:\n    data:\n        x: 1\nb:\n    data:\n        x: 2\n";
        assert_eq!(get(text, &["b", "data", "x"]).as_deref(), Some("2"));
        assert_eq!(get(text, &["a", "data", "x"]).as_deref(), Some("1"));
        assert_eq!(get(text, &["a", "data", "y"]), None);
        assert_eq!(get(text, &["c", "data", "x"]), None);
    }

    #[test]
    fn only_the_chosen_line_changes_and_the_line_ends_are_kept() {
        let changed = set(&file(), &PATH, "true").unwrap();
        assert_eq!(get(&changed, &PATH).as_deref(), Some("true"));
        let diff: Vec<usize> = file()
            .split_inclusive('\n')
            .zip(changed.split_inclusive('\n'))
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(diff, [11], "only line 12 differs");
        assert!(
            changed.contains("closeLeagueClientDuringGame: true\r\n"),
            "CRLF kept"
        );
        assert!(
            !changed.replace("\r\n", "").contains('\n'),
            "no bare line feeds appear"
        );
    }

    #[test]
    fn setting_it_back_gives_the_original_file_exactly() {
        let there = set(&file(), &PATH, "true").unwrap();
        assert_eq!(set(&there, &PATH, "false").unwrap(), file());
    }

    #[test]
    fn a_missing_key_is_added_with_the_indentation_of_its_siblings() {
        let text = file().replace("        closeLeagueClientDuringGame: false\r\n", "");
        assert_eq!(get(&text, &PATH), None);
        let added = set(&text, &PATH, "true").unwrap();
        assert_eq!(get(&added, &PATH).as_deref(), Some("true"));
        assert!(added.contains("        hasBeenPromptedForPotatoMode: true\r\n        closeLeagueClientDuringGame: true\r\n    schemaVersion: 3"), "{added:?}");
        // The rest of the file is the same.
        assert_eq!(
            added.replace("        closeLeagueClientDuringGame: true\r\n", ""),
            text
        );
    }

    #[test]
    fn the_structure_is_never_invented() {
        let error = set("a:\n    b: 1\n", &["x", "data", "y"], "1").unwrap_err();
        assert!(error.contains("no se inventa"), "{error}");
        assert!(set("", &PATH, "true").is_err());
        assert!(set("a: 1\n", &[], "1").is_err());
    }

    #[test]
    fn a_file_with_plain_line_feeds_or_no_final_line_end_is_respected() {
        let lf = "p:\n    data:\n        k: false\n";
        assert_eq!(
            set(lf, &["p", "data", "k"], "true").unwrap(),
            "p:\n    data:\n        k: true\n"
        );
        let no_end = "p:\n    data:\n        k: false";
        assert_eq!(
            set(no_end, &["p", "data", "k"], "true").unwrap(),
            "p:\n    data:\n        k: true"
        );
        let add_to_no_end = "p:\n    data:\n        a: 1";
        assert_eq!(
            set(add_to_no_end, &["p", "data", "k"], "true").unwrap(),
            "p:\n    data:\n        a: 1\n        k: true"
        );
    }

    #[test]
    fn comments_and_blank_lines_do_not_break_the_walk() {
        let text = "# header\n\np:\n    # note\n    data:\n\n        k: 1 # trailing\n";
        assert_eq!(get(text, &["p", "data", "k"]).as_deref(), Some("1"));
        assert_eq!(
            set(text, &["p", "data", "k"], "2").unwrap(),
            "# header\n\np:\n    # note\n    data:\n\n        k: 2\n"
        );
    }
}
