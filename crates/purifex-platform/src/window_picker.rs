//! Hunter mode, the part that touches Windows: which program owns the window under the pointer.
//! The pointer is read, the window at that point is asked for its process, and that process for
//! the place of its `.exe`. Nothing is changed.

use purifex_domain::forced::RunningProcess;
use purifex_domain::ports::WindowPicker;
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM, POINT};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, GetCursorPos, GetWindowThreadProcessId, WindowFromPoint,
};

use crate::processes::image_path;

pub struct WindowsPicker;

/// The program that owns a window.
fn owner(hwnd: HWND) -> Option<RunningProcess> {
    let mut pid = 0u32;
    // SAFETY: `pid` is a live local; a window that is gone just gives 0.
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    (pid != 0).then_some(())?;
    Some(RunningProcess {
        pid,
        path: image_path(pid)?,
    })
}

/// A window of a Store app is held by `ApplicationFrameHost.exe`; the app itself is the
/// process of a window inside it.
fn inside_frame_host(frame: HWND, host_pid: u32) -> Option<HWND> {
    struct Search {
        host_pid: u32,
        found: Option<HWND>,
    }
    unsafe extern "system" fn visit(child: HWND, data: LPARAM) -> BOOL {
        // SAFETY: `data` is the `Search` passed below, alive for the whole enumeration.
        let search = unsafe { &mut *(data.0 as *mut Search) };
        let mut pid = 0u32;
        // SAFETY: `pid` is a live local.
        unsafe { GetWindowThreadProcessId(child, Some(&mut pid)) };
        if pid != 0 && pid != search.host_pid {
            search.found = Some(child);
            return BOOL(0);
        }
        BOOL(1)
    }
    let mut search = Search {
        host_pid,
        found: None,
    };
    // SAFETY: the callback only uses `search`, which outlives the call.
    unsafe {
        let _ = EnumChildWindows(
            Some(frame),
            Some(visit),
            LPARAM(&mut search as *mut Search as isize),
        );
    }
    search.found
}

/// The program of the window `hwnd`, looking inside the frame of a Store app.
pub(crate) fn program_of(hwnd: HWND) -> Option<RunningProcess> {
    let process = owner(hwnd)?;
    if !process
        .name()
        .eq_ignore_ascii_case("ApplicationFrameHost.exe")
    {
        return Some(process);
    }
    inside_frame_host(hwnd, process.pid)
        .and_then(owner)
        .or(Some(process))
}

impl WindowPicker for WindowsPicker {
    fn under_pointer(&self) -> Result<Option<RunningProcess>, String> {
        let mut at = POINT::default();
        // SAFETY: `at` is a live local.
        unsafe { GetCursorPos(&mut at) }
            .map_err(|e| format!("no se pudo leer dónde está el puntero: {e}"))?;
        // SAFETY: any point is accepted; a point over nothing gives a null window.
        let hwnd = unsafe { WindowFromPoint(at) };
        Ok((!hwnd.is_invalid()).then(|| program_of(hwnd)).flatten())
    }
}

#[cfg(test)]
mod tests {
    use windows::core::w;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, WINDOW_EX_STYLE, WS_POPUP,
    };

    use super::*;

    #[test]
    fn a_window_of_this_program_is_attributed_to_this_programs_exe() {
        // SAFETY: a tiny window of the system class `STATIC`, destroyed before returning.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!("sight test"),
                WS_POPUP,
                0,
                0,
                1,
                1,
                None,
                None,
                None,
                None,
            )
        }
        .unwrap();
        let program = program_of(hwnd);
        // SAFETY: the window made above.
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
        let program = program.expect("its own window has a program");
        assert_eq!(program.pid, std::process::id());
        let exe = std::env::current_exe().unwrap();
        assert!(program.path.eq_ignore_ascii_case(&exe.to_string_lossy()));
    }

    #[test]
    fn a_window_that_does_not_exist_has_no_program() {
        assert_eq!(program_of(HWND(std::ptr::null_mut())), None);
    }

    #[test]
    fn the_pointer_can_always_be_asked_what_is_under_it() {
        // Whatever is there (or nothing), asking never fails.
        assert!(WindowsPicker.under_pointer().is_ok());
    }
}
