//! A small read-only view of the Windows registry: open a key, list its subkeys and
//! read text and number values. Nothing here writes or deletes.

use windows::core::{HSTRING, PWSTR};
use windows::Win32::Foundation::{ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS, ERROR_SUCCESS};
use windows::Win32::System::Registry::{
    RegCloseKey, RegEnumKeyExW, RegEnumValueW, RegOpenKeyExW, RegQueryValueExW, HKEY, KEY_READ,
    KEY_WOW64_32KEY, KEY_WOW64_64KEY, REG_DWORD, REG_EXPAND_SZ, REG_SZ, REG_VALUE_TYPE,
};

/// Which view of the registry: the 64-bit one or the 32-bit one (`WOW6432Node`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Bits64,
    Bits32,
}

pub struct Key(HKEY);

impl Key {
    pub fn open(root: HKEY, path: &str, view: View) -> Option<Self> {
        let wow = match view {
            View::Bits64 => KEY_WOW64_64KEY,
            View::Bits32 => KEY_WOW64_32KEY,
        };
        let mut handle = HKEY::default();
        // SAFETY: `handle` is a live local and `path` outlives the call.
        let status = unsafe {
            RegOpenKeyExW(
                root,
                &HSTRING::from(path),
                None,
                KEY_READ | wow,
                &mut handle,
            )
        };
        (status == ERROR_SUCCESS).then_some(Self(handle))
    }

    /// The names of the subkeys.
    pub fn subkeys(&self) -> Vec<String> {
        let mut names = Vec::new();
        let mut buffer = vec![0u16; 512];
        for index in 0.. {
            let mut length = buffer.len() as u32;
            // SAFETY: the buffer is as long as `length` says.
            let status = unsafe {
                RegEnumKeyExW(
                    self.0,
                    index,
                    Some(PWSTR(buffer.as_mut_ptr())),
                    &mut length,
                    None,
                    None,
                    None,
                    None,
                )
            };
            if status != ERROR_SUCCESS {
                break;
            }
            names.push(String::from_utf16_lossy(&buffer[..length as usize]));
        }
        names
    }

    /// The values of the key: each name with its text when it is a text value of up to
    /// 8 KiB (`None` for numbers, binaries and longer ones). At most 20 000.
    pub fn values(&self) -> Vec<(String, Option<String>)> {
        let mut found = Vec::new();
        let mut name = vec![0u16; 16_384];
        let mut data = vec![0u8; 8 * 1024];
        for index in 0..20_000u32 {
            let mut name_len = name.len() as u32;
            let mut size = data.len() as u32;
            let mut kind = 0u32;
            // SAFETY: the buffers are as long as the lengths say.
            let status = unsafe {
                RegEnumValueW(
                    self.0,
                    index,
                    Some(PWSTR(name.as_mut_ptr())),
                    &mut name_len,
                    None,
                    Some(&mut kind),
                    Some(data.as_mut_ptr()),
                    Some(&mut size),
                )
            };
            if status == ERROR_NO_MORE_ITEMS {
                break;
            }
            let label = String::from_utf16_lossy(&name[..name_len as usize]);
            let text = (status == ERROR_SUCCESS && (kind == REG_SZ.0 || kind == REG_EXPAND_SZ.0))
                .then(|| crate::decode::utf16_string(&data[..size as usize]))
                .flatten();
            if status == ERROR_SUCCESS || status == ERROR_MORE_DATA {
                found.push((label, text));
            }
        }
        found
    }

    fn raw(&self, name: &str) -> Option<(REG_VALUE_TYPE, Vec<u8>)> {
        let name = HSTRING::from(name);
        let mut kind = REG_VALUE_TYPE(0);
        let mut size = 0u32;
        // SAFETY: first call asks only for the size.
        let status = unsafe {
            RegQueryValueExW(self.0, &name, None, Some(&mut kind), None, Some(&mut size))
        };
        if status != ERROR_SUCCESS || size == 0 || size > 64 * 1024 {
            return None;
        }
        let mut data = vec![0u8; size as usize];
        // SAFETY: `data` is as long as `size` says.
        let status = unsafe {
            RegQueryValueExW(
                self.0,
                &name,
                None,
                Some(&mut kind),
                Some(data.as_mut_ptr()),
                Some(&mut size),
            )
        };
        (status == ERROR_SUCCESS).then(|| {
            data.truncate(size as usize);
            (kind, data)
        })
    }

    pub fn text(&self, name: &str) -> Option<String> {
        let (kind, data) = self.raw(name)?;
        if kind != REG_SZ && kind != REG_EXPAND_SZ {
            return None;
        }
        crate::decode::utf16_string(&data)
    }

    pub fn number(&self, name: &str) -> Option<u32> {
        let (kind, data) = self.raw(name)?;
        (kind == REG_DWORD)
            .then(|| {
                data.get(..4)
                    .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            })
            .flatten()
    }
}

impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: closes the handle opened in `open`.
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}
