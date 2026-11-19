//! Opens a web page in the user's default browser. Purifex downloads nothing
//! itself: the user gets the driver from the manufacturer's page.

use purifex_domain::ports::UrlOpener;
use windows::core::{w, PCWSTR};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

pub struct BrowserOpener;

impl UrlOpener for BrowserOpener {
    fn open(&self, url: &str) -> Result<(), String> {
        // The last guard: only a web address is ever handed to the shell, never a
        // file path or a program.
        if !url.starts_with("https://") {
            return Err("solo se abren direcciones https".into());
        }
        let wide: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: `wide` is NUL terminated and outlives the call; the other
        // arguments are null or constants.
        let result = unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                PCWSTR(wide.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        // ShellExecute reports success with a value above 32.
        if result.0 as isize > 32 {
            Ok(())
        } else {
            Err("no se pudo abrir el navegador".into())
        }
    }
}
