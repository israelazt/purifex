//! Shows a key in the registry editor. Regedit opens on the last key it showed, which it
//! keeps in the setting `LastKey` of the user; writing that setting and starting Regedit is the
//! way to open it on a chosen key. Windows asks for administrator permission by itself.

use purifex_domain::ports::RegistryViewer;
use windows::core::{w, PCWSTR};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use crate::app_cleanup::registry_write::{set_value_raw, RawValue};

const REGEDIT_SETTINGS: &str =
    r"HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Applets\Regedit";
const REG_SZ: u32 = 1;

pub struct RegeditViewer;

/// Only a key under one of the two roots the apps register in, without anything odd in it.
fn is_shown_key(key: &str) -> bool {
    let rooted = ["HKEY_LOCAL_MACHINE\\", "HKEY_CURRENT_USER\\"]
        .iter()
        .any(|root| key.starts_with(root) && key.len() > root.len());
    rooted && !key.chars().any(char::is_control) && !key.contains("\\\\") && !key.ends_with('\\')
}

impl RegistryViewer for RegeditViewer {
    fn show(&self, key: &str) -> Result<(), String> {
        if !is_shown_key(key) {
            return Err("esa clave no se puede abrir desde aquí".into());
        }
        let text: Vec<u8> = format!("Computer\\{key}")
            .encode_utf16()
            .chain(std::iter::once(0))
            .flat_map(u16::to_le_bytes)
            .collect();
        set_value_raw(
            REGEDIT_SETTINGS,
            "LastKey",
            &RawValue {
                kind: REG_SZ,
                data: text,
            },
        )
        .map_err(|_| "no se pudo preparar el editor del registro".to_string())?;
        // SAFETY: the strings are constants; the other arguments are null or constants.
        let result = unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                w!("regedit.exe"),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        if result.0 as isize > 32 {
            Ok(())
        } else {
            Err(
                "no se abrió el editor del registro (¿dijiste que no al permiso de Windows?)"
                    .into(),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::is_shown_key;

    #[test]
    fn only_a_key_under_the_machine_or_the_user_root_is_shown() {
        assert!(is_shown_key(
            r"HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\X"
        ));
        assert!(is_shown_key(r"HKEY_CURRENT_USER\SOFTWARE\X"));
        for bad in [
            "",
            r"HKEY_LOCAL_MACHINE\",
            r"HKEY_USERS\S-1\X",
            r"HKEY_LOCAL_MACHINE\X\",
            r"HKEY_LOCAL_MACHINE\\X",
            "HKEY_CURRENT_USER\\X\u{1}",
            r"C:\Windows\regedit.exe",
        ] {
            assert!(!is_shown_key(bad), "{bad:?}");
        }
    }
}
