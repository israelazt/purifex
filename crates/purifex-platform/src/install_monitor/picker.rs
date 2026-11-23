//! The system's «Open file» dialog.

use purifex_domain::ports::FilePicker;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_PATHMUSTEXIST, OPENFILENAMEW,
};

pub struct WindowsFilePicker;

impl FilePicker for WindowsFilePicker {
    fn pick(&self, title: &str, extensions: &[&str]) -> Result<Option<String>, String> {
        // «name\0*.exe;*.msi\0\0»: the list of filters of the dialog.
        let patterns = extensions
            .iter()
            .map(|e| format!("*.{e}"))
            .collect::<Vec<_>>()
            .join(";");
        let mut filter: Vec<u16> = format!("{patterns}\0{patterns}\0\0")
            .encode_utf16()
            .collect();
        let title: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        let mut buffer = vec![0u16; 1024];
        let mut dialog = OPENFILENAMEW {
            lStructSize: size_of::<OPENFILENAMEW>() as u32,
            lpstrFilter: PCWSTR(filter.as_mut_ptr()),
            lpstrFile: PWSTR(buffer.as_mut_ptr()),
            nMaxFile: buffer.len() as u32,
            lpstrTitle: PCWSTR(title.as_ptr()),
            Flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST,
            ..Default::default()
        };
        // SAFETY: every pointer in `dialog` points into a local that outlives the call.
        let chosen = unsafe { GetOpenFileNameW(&mut dialog) }.as_bool();
        if !chosen {
            return Ok(None);
        }
        let end = buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len());
        Ok(Some(String::from_utf16_lossy(&buffer[..end])))
    }
}
