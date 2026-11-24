//! Taking a row of the manifest off the computer, and saying where it is. Nothing is removed that
//! the manifest does not name as ours: that is checked again here, right before each removal.

use std::fs;

use crate::manifest::{is_ours, name_of, owned_root, Item};
use crate::places::path_of;
use crate::system;

/// Where the row lives, as a person would read it: a path, a key or a task name.
pub fn where_of(item: &Item) -> Result<String, String> {
    Ok(match *item {
        Item::File { root, path, .. }
        | Item::Dir { root, path, .. }
        | Item::Lnk { root, path, .. } => path_of(root, path)?.to_string_lossy().into_owned(),
        Item::RegKey { hive, path, .. } => system::key_arg(hive, path),
        Item::Task { name, .. } => name.to_string(),
    })
}

fn refuse_what_is_not_ours(item: &Item) -> Result<(), String> {
    let name = name_of(item);
    match is_ours(name) {
        true => Ok(()),
        false => Err(format!("se niega a quitar «{name}»: no es de Fex Sonda")),
    }
}

fn remove_path(path: &std::path::Path, is_dir: bool) -> Result<(), String> {
    let done = match (path.exists(), is_dir) {
        (false, _) => return Ok(()),
        (true, true) => fs::remove_dir_all(path),
        (true, false) => fs::remove_file(path),
    };
    done.map_err(|e| format!("no se pudo quitar {}: {e}", path.display()))
}

/// Removes the row if it is there. Removing what is not there is not an error.
pub fn remove(item: &Item) -> Result<(), String> {
    refuse_what_is_not_ours(item)?;
    match *item {
        Item::File { root, path, .. } | Item::Lnk { root, path, .. } => {
            remove_path(&path_of(root, path)?, false)
        }
        Item::Dir { root, path, .. } => remove_path(&path_of(root, path)?, true),
        Item::RegKey { hive, path, .. } => {
            let key = owned_root(path).ok_or("la clave no tiene nada que sea de Fex Sonda")?;
            match system::key_exists(hive, &key) {
                true => system::delete_key(hive, &key),
                false => Ok(()),
            }
        }
        Item::Task { name, .. } => match system::task_exists(name) {
            true => system::delete_task(name),
            false => Ok(()),
        },
    }
}
