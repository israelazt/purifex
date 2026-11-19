//! The icon of the window (title bar, taskbar, Alt+Tab): the logo on a dark square,
//! from a PNG compiled into the program. It is the same on any theme of Windows,
//! because the square keeps the white logo visible on a light taskbar too.

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconFromResourceEx, SendMessageW, ICON_BIG, ICON_SMALL, LR_DEFAULTCOLOR, WM_SETICON,
};

const ICON_PNG: &[u8] = include_bytes!("../../../../assets/purifex-icon.png");
/// The version of the icon resource format that `CreateIconFromResourceEx` expects.
const RESOURCE_VERSION: u32 = 0x0003_0000;

/// Gives the window its icon, in the sizes the system asks for.
pub fn set(hwnd: HWND) {
    for (kind, side) in [(ICON_SMALL, 16), (ICON_BIG, 32)] {
        // SAFETY: the bytes are a PNG that lives for the whole program; the icon
        // handle is handed to the window, which owns it from then on.
        unsafe {
            if let Ok(icon) = CreateIconFromResourceEx(
                ICON_PNG,
                true,
                RESOURCE_VERSION,
                side,
                side,
                LR_DEFAULTCOLOR,
            ) {
                SendMessageW(
                    hwnd,
                    WM_SETICON,
                    Some(WPARAM(kind as usize)),
                    Some(LPARAM(icon.0 as isize)),
                );
            }
        }
    }
}
