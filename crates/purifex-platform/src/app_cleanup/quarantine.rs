//! The quarantine: what is removed after an uninstall is moved, not deleted, to
//! `%LOCALAPPDATA%\Purifex\quarantine\<n>\`, and registry keys are exported to a
//! `.reg` file before they are deleted, so any removal can be put back. The log of
//! what was done is `cleanup.jsonl`, only ever added to.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use purifex_domain::app_traces::{Trace, TraceKind};
use purifex_domain::apps::InstalledApp;
use purifex_domain::cleanup_log::{CleanupEntry, ValueCopy};
use purifex_domain::ports::CleanupStore;

use super::registry_write;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub struct LocalCleanup {
    base: Option<PathBuf>,
}

impl LocalCleanup {
    pub fn new() -> Self {
        Self {
            base: std::env::var_os("LOCALAPPDATA").map(|b| PathBuf::from(b).join("Purifex")),
        }
    }

    pub fn in_dir(base: PathBuf) -> Self {
        Self { base: Some(base) }
    }

    fn base(&self) -> Result<&PathBuf, String> {
        self.base
            .as_ref()
            .ok_or_else(|| "no se encuentra la carpeta de datos del usuario".to_string())
    }

    fn log(&self) -> Result<PathBuf, String> {
        Ok(self.base()?.join("cleanup.jsonl"))
    }
}

impl Default for LocalCleanup {
    fn default() -> Self {
        Self::new()
    }
}

/// Runs `reg.exe` and says whether it succeeded.
fn reg(args: &[&str]) -> Result<(), String> {
    let output = Command::new("reg")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("no se pudo ejecutar reg.exe: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let text = String::from_utf8_lossy(&output.stderr);
        Err(format!("reg.exe: {}", text.trim()))
    }
}

fn last_part(location: &str) -> &str {
    location
        .rsplit(['\\', '/'])
        .find(|p| !p.is_empty())
        .unwrap_or("item")
}

impl CleanupStore for LocalCleanup {
    fn remove(&mut self, trace: &Trace, seq: u64) -> Result<String, String> {
        let folder = self.base()?.join("quarantine").join(seq.to_string());
        fs::create_dir_all(&folder).map_err(|e| format!("no se pudo crear la cuarentena: {e}"))?;
        match trace.kind {
            TraceKind::Folder | TraceKind::File => {
                let from = Path::new(&trace.location);
                if !from.exists() {
                    return Err("ya no existe".into());
                }
                let kept = folder.join(last_part(&trace.location));
                fs::rename(from, &kept).map_err(|e| {
                    format!("no se pudo mover a la cuarentena (¿está en uso o en otro disco?): {e}")
                })?;
                Ok(kept.to_string_lossy().into_owned())
            }
            TraceKind::RegistryKey => {
                let file = folder.join("clave.reg");
                let file_text = file.to_string_lossy().into_owned();
                // Export first: if the delete is refused (no permission) nothing changed.
                reg(&["export", &trace.location, &file_text, "/y"])?;
                registry_write::delete_key_tree(&trace.location).map_err(|e| e.message())?;
                Ok(file_text)
            }
            TraceKind::RegistryValue => {
                let name = trace.value.as_deref().ok_or("falta el nombre del valor")?;
                let raw = registry_write::read_value_raw(&trace.location, name)
                    .map_err(|e| e.message())?;
                let copy = ValueCopy::new(&trace.location, name, raw.kind, &raw.data);
                let file = folder.join("valor.json");
                // The copy is on disk before the value is touched.
                fs::write(&file, copy.to_json())
                    .map_err(|e| format!("no se pudo guardar la copia del valor: {e}"))?;
                registry_write::delete_value(&trace.location, name).map_err(|e| e.message())?;
                Ok(file.to_string_lossy().into_owned())
            }
        }
    }

    fn restore(&mut self, entry: &CleanupEntry) -> Result<(), String> {
        match entry.trace.kind {
            TraceKind::Folder | TraceKind::File => {
                let target = Path::new(&entry.trace.location);
                if target.exists() {
                    return Err("ya hay algo en ese lugar: no se sobrescribe".into());
                }
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                fs::rename(&entry.kept, target).map_err(|e| format!("no se pudo devolver: {e}"))
            }
            TraceKind::RegistryKey => reg(&["import", &entry.kept]),
            TraceKind::RegistryValue => {
                let text = fs::read_to_string(&entry.kept)
                    .map_err(|e| format!("no se pudo leer la copia del valor: {e}"))?;
                let copy = ValueCopy::from_json(&text)?;
                if registry_write::read_value_raw(&copy.key, &copy.name).is_ok() {
                    return Err("ya hay un valor con ese nombre: no se sobrescribe".into());
                }
                let value = registry_write::RawValue {
                    kind: copy.kind,
                    data: copy.data()?,
                };
                registry_write::set_value_raw(&copy.key, &copy.name, &value)
                    .map_err(|e| e.message())
            }
        }
    }

    fn entries(&self) -> Result<Vec<CleanupEntry>, String> {
        let path = self.log()?;
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(format!("no se pudo leer {}: {e}", path.display())),
        };
        // Like the history of fixes: a damaged line is reported, because undoing relies on it.
        text.lines()
            .enumerate()
            .filter(|(_, l)| !l.trim().is_empty())
            .map(|(i, l)| {
                CleanupEntry::from_line(l).map_err(|e| {
                    format!("la línea {} de {} está dañada: {e}", i + 1, path.display())
                })
            })
            .collect()
    }

    fn remember(&mut self, app: &InstalledApp) -> Result<(), String> {
        let line = serde_line(app)?;
        self.add_line("uninstalled.jsonl", &line)
    }

    fn remembered(&self) -> Result<Vec<InstalledApp>, String> {
        let path = self.base()?.join("uninstalled.jsonl");
        let text = fs::read_to_string(&path).unwrap_or_default();
        Ok(text
            .lines()
            .filter_map(|l| InstalledApp::from_line(l).ok())
            .collect())
    }

    fn append(&mut self, entry: &CleanupEntry) -> Result<(), String> {
        self.add_line("cleanup.jsonl", &entry.to_line())
    }
}

impl LocalCleanup {
    fn add_line(&self, file: &str, line: &str) -> Result<(), String> {
        let path = self.base()?.join(file);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| format!("no se pudo abrir {}: {e}", path.display()))?;
        writeln!(file, "{line}").map_err(|e| e.to_string())
    }
}

fn serde_line(app: &InstalledApp) -> Result<String, String> {
    Ok(app.to_line())
}

#[cfg(test)]
mod tests {
    use purifex_domain::cleanup_log::CleanupState;

    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("purifex-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn trace(kind: TraceKind, location: &str) -> Trace {
        Trace {
            kind,
            location: location.into(),
            bytes: 1,
            why: "test".into(),
            user_data: false,
            value: None,
            evidence: Default::default(),
        }
    }

    fn entry(seq: u64, t: Trace, kept: String) -> CleanupEntry {
        CleanupEntry {
            seq,
            app: "Prueba".into(),
            trace: t,
            kept,
            state: CleanupState::Removed,
            restores: None,
        }
    }

    #[test]
    fn a_folder_goes_to_the_quarantine_with_its_content_and_comes_back_the_same() {
        let work = temp("q-folder");
        let app_data = work.join("AppData").join("PruebaApp");
        fs::create_dir_all(app_data.join("sub")).unwrap();
        fs::write(app_data.join("sub").join("save.dat"), b"partida").unwrap();
        let mut store = LocalCleanup::in_dir(work.join("Purifex"));
        let t = trace(TraceKind::Folder, &app_data.to_string_lossy());
        let kept = store.remove(&t, 1).unwrap();
        assert!(!app_data.exists(), "it is gone from its place");
        assert_eq!(
            fs::read(Path::new(&kept).join("sub").join("save.dat")).unwrap(),
            b"partida",
            "but kept whole"
        );
        store.restore(&entry(1, t, kept)).unwrap();
        assert_eq!(
            fs::read(app_data.join("sub").join("save.dat")).unwrap(),
            b"partida"
        );
        let _ = fs::remove_dir_all(&work);
    }

    #[test]
    fn restoring_never_overwrites_and_removing_what_is_gone_is_an_error() {
        let work = temp("q-guard");
        let place = work.join("Datos");
        fs::create_dir_all(&place).unwrap();
        let mut store = LocalCleanup::in_dir(work.join("Purifex"));
        let t = trace(TraceKind::Folder, &place.to_string_lossy());
        let kept = store.remove(&t, 1).unwrap();
        fs::create_dir_all(&place).unwrap(); // something new took the place
        let err = store.restore(&entry(1, t.clone(), kept)).unwrap_err();
        assert!(err.contains("no se sobrescribe"));
        assert!(store
            .remove(
                &trace(TraceKind::Folder, &work.join("nada").to_string_lossy()),
                2
            )
            .unwrap_err()
            .contains("ya no existe"));
        let _ = fs::remove_dir_all(&work);
    }

    #[test]
    fn a_registry_key_is_exported_then_deleted_and_importing_it_brings_it_back() {
        let work = temp("q-reg");
        let key = format!(r"HKCU\Software\PurifexTest{}", std::process::id());
        reg(&["add", &key, "/v", "Valor", "/d", "hola", "/f"]).unwrap();
        let mut store = LocalCleanup::in_dir(work.join("Purifex"));
        let t = trace(TraceKind::RegistryKey, &key);
        let kept = store.remove(&t, 1).unwrap();
        assert!(reg(&["query", &key]).is_err(), "the key is gone");
        assert!(Path::new(&kept).exists(), "with its copy");
        store.restore(&entry(1, t, kept)).unwrap();
        assert!(reg(&["query", &key, "/v", "Valor"]).is_ok(), "and back");
        reg(&["delete", &key, "/f"]).unwrap();
        let _ = fs::remove_dir_all(&work);
    }

    fn utf16(text: &str, nul: usize) -> Vec<u8> {
        let mut data: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
        data.extend(std::iter::repeat_n(0, nul * 2));
        data
    }

    fn value_trace(key: &str, name: &str) -> Trace {
        let mut t = trace(TraceKind::RegistryValue, key);
        t.value = Some(name.into());
        t
    }

    #[test]
    fn a_registry_value_of_every_type_comes_back_exactly_and_its_neighbours_stay() {
        use registry_write::{delete_key_tree, read_value_raw, set_value_raw, RawValue};
        let work = temp("q-value");
        let key = format!(r"HKCU\Software\PurifexTest\valores_{}", std::process::id());
        let values = [
            ("sz", 1, utf16(r"C:\Apps\X\x.exe ñ", 1)),
            ("expand", 2, utf16(r"%ProgramFiles%\X", 1)),
            ("binary", 3, vec![0, 1, 2, 254, 255]),
            ("dword", 4, 0xDEADBEEFu32.to_le_bytes().to_vec()),
            ("multi", 7, utf16("uno\0dos", 2)),
            ("empty", 3, vec![]),
            ("nombre con espacios \\ y raro", 1, utf16("x", 1)),
        ];
        for (name, kind, data) in &values {
            let v = RawValue {
                kind: *kind,
                data: data.clone(),
            };
            set_value_raw(&key, name, &v).unwrap();
        }
        let neighbour = RawValue {
            kind: 1,
            data: utf16("vecino", 1),
        };
        set_value_raw(&key, "vecino", &neighbour).unwrap();
        let mut store = LocalCleanup::in_dir(work.join("Purifex"));
        for (seq, (name, kind, data)) in values.iter().enumerate() {
            let t = value_trace(&key, name);
            let kept = store.remove(&t, seq as u64 + 1).unwrap();
            assert_eq!(
                read_value_raw(&key, name),
                Err(registry_write::RegError::NotFound)
            );
            assert!(kept.ends_with("valor.json"));
            store.restore(&entry(seq as u64 + 1, t, kept)).unwrap();
            let back = read_value_raw(&key, name).unwrap();
            assert_eq!((back.kind, &back.data), (*kind, data), "{name}");
        }
        assert_eq!(read_value_raw(&key, "vecino").unwrap(), neighbour);
        delete_key_tree(&key).unwrap();
        let _ = fs::remove_dir_all(&work);
    }

    #[test]
    fn a_value_is_never_overwritten_and_a_missing_one_is_an_error() {
        use registry_write::{delete_key_tree, set_value_raw, RawValue};
        let work = temp("q-value-guard");
        let key = format!(r"HKCU\Software\PurifexTest\guardas_{}", std::process::id());
        let v = RawValue {
            kind: 1,
            data: utf16("a", 1),
        };
        set_value_raw(&key, "v", &v).unwrap();
        let mut store = LocalCleanup::in_dir(work.join("Purifex"));
        let t = value_trace(&key, "v");
        let kept = store.remove(&t, 1).unwrap();
        assert!(store.remove(&t, 2).unwrap_err().contains("ya no existe"));
        set_value_raw(
            &key,
            "v",
            &RawValue {
                kind: 1,
                data: utf16("nuevo", 1),
            },
        )
        .unwrap();
        let err = store.restore(&entry(1, t, kept)).unwrap_err();
        assert!(err.contains("no se sobrescribe"));
        let no_name = trace(TraceKind::RegistryValue, &key);
        assert!(store.remove(&no_name, 3).is_err());
        delete_key_tree(&key).unwrap();
        let _ = fs::remove_dir_all(&work);
    }

    #[test]
    fn the_log_keeps_every_line_in_order_and_a_damaged_one_is_reported() {
        let work = temp("q-log");
        let mut store = LocalCleanup::in_dir(work.clone());
        assert_eq!(store.entries().unwrap(), vec![]);
        let t = trace(TraceKind::Folder, r"C:\x\y");
        store.append(&entry(1, t.clone(), "k".into())).unwrap();
        store.append(&entry(2, t, "k".into())).unwrap();
        assert_eq!(
            store
                .entries()
                .unwrap()
                .iter()
                .map(|e| e.seq)
                .collect::<Vec<_>>(),
            [1, 2]
        );
        let mut f = OpenOptions::new()
            .append(true)
            .open(work.join("cleanup.jsonl"))
            .unwrap();
        writeln!(f, "{{rota").unwrap();
        assert!(store.entries().unwrap_err().contains("dañada"));
        let _ = fs::remove_dir_all(&work);
    }
}
