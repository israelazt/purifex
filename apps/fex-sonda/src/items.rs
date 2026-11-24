//! Writing and looking at one row of the manifest, whatever its kind.

use std::fs;
use std::path::{Path, PathBuf};

use crate::manifest::{expand, Content, Item};
use crate::places::{make_parents, path_of};
use crate::system;

/// What a row needs to be written: where Fex Sonda is installed and this very program.
pub struct Context {
    pub install_dir: String,
    pub exe: PathBuf,
}

fn write_file(path: &Path, content: Content, exe: &Path) -> Result<(), String> {
    make_parents(path)?;
    let done = match content {
        Content::Itself => fs::copy(exe, path).map(|_| ()),
        Content::Text(text) => fs::write(path, text),
        Content::Zeros(n) => fs::write(path, vec![0u8; n]),
        Content::Empty => fs::write(path, b""),
    };
    done.map_err(|e| format!("no se pudo escribir {}: {e}", path.display()))
}

/// The files of a folder, written under `base`.
pub fn write_files(base: &Path, files: &[(&str, Content)], exe: &Path) -> Result<(), String> {
    fs::create_dir_all(base).map_err(|e| format!("no se pudo crear {}: {e}", base.display()))?;
    files
        .iter()
        .try_for_each(|(name, content)| write_file(&base.join(name), *content, exe))
}

/// Writes the row. Writing it again leaves it the same.
pub fn write(item: &Item, ctx: &Context) -> Result<(), String> {
    match *item {
        Item::File {
            root,
            path,
            content,
        } => write_file(&path_of(root, path)?, content, &ctx.exe),
        Item::Dir { root, path, files } => write_files(&path_of(root, path)?, files, &ctx.exe),
        Item::RegKey { hive, path, values } => {
            system::add_key(hive, path)?;
            values.iter().try_for_each(|(name, data)| {
                system::set_value(hive, path, name, data, &ctx.install_dir)
            })
        }
        Item::Lnk { root, path, target } => {
            let at = path_of(root, path)?;
            make_parents(&at)?;
            system::make_shortcut(&at.to_string_lossy(), &expand(target, &ctx.install_dir))
        }
        Item::Task { name, command } => {
            system::create_task(name, &expand(command, &ctx.install_dir))
        }
    }
}

/// Whether the row is on this computer now.
pub fn exists(item: &Item) -> Result<bool, String> {
    Ok(match *item {
        Item::File { root, path, .. }
        | Item::Dir { root, path, .. }
        | Item::Lnk { root, path, .. } => path_of(root, path)?.exists(),
        Item::RegKey { hive, path, .. } => system::key_exists(hive, path),
        Item::Task { name, .. } => system::task_exists(name),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fexsonda-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_folder_is_written_with_its_files_and_the_parents_they_need() {
        let base = temp("dir");
        let files = [
            ("a.txt", Content::Text("hola")),
            (r"cache\a.bin", Content::Zeros(64 * 1024)),
            (r"logs\empty.log", Content::Empty),
        ];
        write_files(&base, &files, Path::new("unused")).unwrap();
        assert_eq!(fs::read(base.join("a.txt")).unwrap(), b"hola");
        assert_eq!(
            fs::metadata(base.join(r"cache\a.bin")).unwrap().len(),
            64 * 1024
        );
        assert_eq!(fs::metadata(base.join(r"logs\empty.log")).unwrap().len(), 0);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn writing_a_folder_twice_leaves_it_the_same() {
        let base = temp("twice");
        let files = [("a.txt", Content::Text("hola"))];
        write_files(&base, &files, Path::new("unused")).unwrap();
        write_files(&base, &files, Path::new("unused")).unwrap();
        assert_eq!(fs::read(base.join("a.txt")).unwrap(), b"hola");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn a_file_can_be_a_copy_of_the_program() {
        let base = temp("itself");
        let exe = std::env::current_exe().unwrap();
        write_files(&base, &[("copy.exe", Content::Itself)], &exe).unwrap();
        assert_eq!(
            fs::metadata(base.join("copy.exe")).unwrap().len(),
            fs::metadata(&exe).unwrap().len()
        );
        let _ = fs::remove_dir_all(&base);
    }
}
