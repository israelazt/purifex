//! The only place that writes to the registry: read a value exactly (type and bytes),
//! put one back, delete one value, delete a key with everything under it. Only the
//! quarantine calls it, always after copying what it is about to remove. It refuses
//! anything but `HKCU\...` and `HKLM\...` at least two levels deep, whatever it is told.

use windows::core::HSTRING;
use windows::Win32::Foundation::{
    ERROR_ACCESS_DENIED, ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
    RegSetValueExW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_SET_VALUE,
    KEY_WOW64_64KEY, REG_OPTION_NON_VOLATILE, REG_VALUE_TYPE,
};

/// More than this in one value is not touched: it is not a cache entry of an app.
const MAX_VALUE_BYTES: u32 = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegError {
    /// The key or the value is not there.
    NotFound,
    /// Windows refused: it needs an administrator (or the key is protected).
    Denied,
    /// Not a place this module may touch, or a value too big.
    Refused,
    /// Any other Windows error code.
    Other(u32),
}

impl RegError {
    pub fn message(self) -> String {
        match self {
            RegError::NotFound => "ya no existe".into(),
            RegError::Denied => "requiere administrador".into(),
            RegError::Refused => "lugar o tamaño no permitido".into(),
            RegError::Other(code) => format!("error del registro {code}"),
        }
    }
}

fn status(code: WIN32_ERROR) -> Result<(), RegError> {
    match code {
        ERROR_SUCCESS => Ok(()),
        ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND => Err(RegError::NotFound),
        ERROR_ACCESS_DENIED => Err(RegError::Denied),
        other => Err(RegError::Other(other.0)),
    }
}

/// A value as Windows stores it: its type code and its bytes, exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawValue {
    pub kind: u32,
    pub data: Vec<u8>,
}

/// `HKCU\Software\X` as the root handle and the path below it.
fn split(path: &str) -> Result<(HKEY, &str), RegError> {
    let (hive, sub) = path.split_once('\\').ok_or(RegError::Refused)?;
    let root = match hive {
        "HKCU" => HKEY_CURRENT_USER,
        "HKLM" => HKEY_LOCAL_MACHINE,
        _ => return Err(RegError::Refused),
    };
    let parts: Vec<&str> = sub.split('\\').collect();
    if parts.len() < 2
        || parts
            .iter()
            .any(|p| p.is_empty() || *p == "." || *p == "..")
    {
        return Err(RegError::Refused);
    }
    Ok((root, sub))
}

struct Open(HKEY);

impl Open {
    fn new(
        path: &str,
        access: windows::Win32::System::Registry::REG_SAM_FLAGS,
    ) -> Result<Open, RegError> {
        let (root, sub) = split(path)?;
        let mut handle = HKEY::default();
        // SAFETY: `handle` is a live local and `sub` outlives the call.
        let code = unsafe {
            RegOpenKeyExW(
                root,
                &HSTRING::from(sub),
                None,
                access | KEY_WOW64_64KEY,
                &mut handle,
            )
        };
        status(code).map(|()| Open(handle))
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        // SAFETY: closes the handle opened in `new` or in `create`.
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

/// Reads a value with its type and bytes.
pub fn read_value_raw(key: &str, name: &str) -> Result<RawValue, RegError> {
    let open = Open::new(key, KEY_READ)?;
    let name = HSTRING::from(name);
    let mut kind = REG_VALUE_TYPE(0);
    let mut size = 0u32;
    // SAFETY: the first call asks only for the size.
    let code =
        unsafe { RegQueryValueExW(open.0, &name, None, Some(&mut kind), None, Some(&mut size)) };
    status(code)?;
    if size > MAX_VALUE_BYTES {
        return Err(RegError::Refused);
    }
    // An empty value has no bytes to read.
    let mut data = vec![0u8; size as usize];
    if size > 0 {
        // SAFETY: `data` is as long as `size` says.
        let code = unsafe {
            RegQueryValueExW(
                open.0,
                &name,
                None,
                Some(&mut kind),
                Some(data.as_mut_ptr()),
                Some(&mut size),
            )
        };
        status(code)?;
        data.truncate(size as usize);
    }
    Ok(RawValue { kind: kind.0, data })
}

/// Writes a value, creating the key (and its parents) when it is not there, so a value
/// can be put back into a key that was removed with it.
pub fn set_value_raw(key: &str, name: &str, value: &RawValue) -> Result<(), RegError> {
    if value.data.len() as u32 > MAX_VALUE_BYTES {
        return Err(RegError::Refused);
    }
    let (root, sub) = split(key)?;
    let mut handle = HKEY::default();
    // SAFETY: `handle` is a live local and `sub` outlives the call.
    let code = unsafe {
        RegCreateKeyExW(
            root,
            &HSTRING::from(sub),
            None,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE | KEY_WOW64_64KEY,
            None,
            &mut handle,
            None,
        )
    };
    status(code)?;
    let open = Open(handle);
    // SAFETY: `open.0` is a live handle and the data is as long as it says.
    let code = unsafe {
        RegSetValueExW(
            open.0,
            &HSTRING::from(name),
            None,
            REG_VALUE_TYPE(value.kind),
            Some(&value.data),
        )
    };
    status(code)
}

/// Deletes one value of a key. The key and its other values stay.
pub fn delete_value(key: &str, name: &str) -> Result<(), RegError> {
    let open = Open::new(key, KEY_SET_VALUE)?;
    // SAFETY: `open.0` is a live handle.
    let code = unsafe { RegDeleteValueW(open.0, &HSTRING::from(name)) };
    status(code)
}

/// Deletes a key with all its subkeys and values.
pub fn delete_key_tree(key: &str) -> Result<(), RegError> {
    let (root, sub) = split(key)?;
    // SAFETY: `root` is a predefined handle and `sub` outlives the call.
    let code = unsafe { RegDeleteTreeW(root, &HSTRING::from(sub)) };
    status(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A key of the tests, removed when the test ends.
    struct Scratch(String);

    impl Scratch {
        fn new(name: &str) -> Scratch {
            let key = format!(r"HKCU\Software\PurifexTest\{name}_{}", std::process::id());
            let _ = delete_key_tree(&key);
            Scratch(key)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = delete_key_tree(&self.0);
        }
    }

    fn sz(text: &str) -> RawValue {
        let mut data: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
        data.extend([0, 0]);
        RawValue { kind: 1, data }
    }

    #[test]
    fn a_value_is_written_read_back_exactly_and_deleted_alone() {
        let k = Scratch::new("value");
        set_value_raw(&k.0, "a", &sz("uno")).unwrap();
        set_value_raw(&k.0, "b", &sz("dos")).unwrap();
        assert_eq!(read_value_raw(&k.0, "a").unwrap(), sz("uno"));
        delete_value(&k.0, "a").unwrap();
        assert_eq!(read_value_raw(&k.0, "a"), Err(RegError::NotFound));
        assert_eq!(read_value_raw(&k.0, "b").unwrap(), sz("dos"));
        assert_eq!(delete_value(&k.0, "a"), Err(RegError::NotFound));
    }

    #[test]
    fn a_key_goes_with_everything_under_it() {
        let k = Scratch::new("tree");
        set_value_raw(&format!(r"{}\hijo\nieto", k.0), "v", &sz("x")).unwrap();
        delete_key_tree(&k.0).unwrap();
        assert_eq!(
            read_value_raw(&format!(r"{}\hijo\nieto", k.0), "v"),
            Err(RegError::NotFound)
        );
        assert_eq!(delete_key_tree(&k.0), Err(RegError::NotFound));
    }

    #[test]
    fn an_empty_value_and_a_binary_one_keep_their_type() {
        let k = Scratch::new("kinds");
        let empty = RawValue {
            kind: 3,
            data: vec![],
        };
        let binary = RawValue {
            kind: 3,
            data: vec![0, 1, 2, 255],
        };
        set_value_raw(&k.0, "empty", &empty).unwrap();
        set_value_raw(&k.0, "bin", &binary).unwrap();
        assert_eq!(read_value_raw(&k.0, "empty").unwrap(), empty);
        assert_eq!(read_value_raw(&k.0, "bin").unwrap(), binary);
    }

    #[test]
    fn broad_or_foreign_places_are_refused_before_touching_anything() {
        for key in [
            r"HKCU\Software",
            r"HKLM",
            r"HKCU",
            r"HKCR\Software\X",
            r"HKCU\Software\..\X",
            r"HKCU\Software\\X",
            r"C:\x\y",
        ] {
            assert_eq!(delete_key_tree(key), Err(RegError::Refused), "{key}");
            assert_eq!(delete_value(key, "v"), Err(RegError::Refused), "{key}");
            assert_eq!(read_value_raw(key, "v"), Err(RegError::Refused), "{key}");
            assert_eq!(
                set_value_raw(key, "v", &sz("x")),
                Err(RegError::Refused),
                "{key}"
            );
        }
    }

    #[test]
    fn a_missing_key_is_not_found_and_a_protected_one_needs_an_administrator() {
        assert_eq!(
            read_value_raw(r"HKCU\Software\PurifexTest\no_existe_jamas", "v"),
            Err(RegError::NotFound)
        );
        // Without elevation Windows does not let a user write under HKLM\SOFTWARE.
        let denied = set_value_raw(r"HKLM\SOFTWARE\PurifexTestDenied", "v", &sz("x"));
        if denied.is_ok() {
            let _ = delete_key_tree(r"HKLM\SOFTWARE\PurifexTestDenied");
        } else {
            assert_eq!(denied, Err(RegError::Denied));
        }
    }
}
