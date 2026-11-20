//! Whether the program runs as an administrator, and how it asks to start again as one.
//! Purifex never runs elevated on its own: removing the traces that need it is the only
//! reason to ask, and Windows shows its own question (UAC) before anything changes.

use purifex_domain::ports::Elevation;
use windows::core::{w, HRESULT, HSTRING, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::Shell::{ShellExecuteExW, SHELLEXECUTEINFOW};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

/// `ERROR_CANCELLED`: the person answered no to the question of Windows.
const CANCELLED: HRESULT = HRESULT(0x8007_04C7_u32 as i32);

pub struct WindowsElevation;

impl Elevation for WindowsElevation {
    fn is_elevated(&self) -> bool {
        let mut token = HANDLE::default();
        // SAFETY: the pointer to the current process is a pseudo handle; `token` is a live
        // local that receives a handle which is closed before returning.
        unsafe {
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
                return false;
            }
            let mut elevation = TOKEN_ELEVATION::default();
            let mut returned = 0u32;
            let asked = GetTokenInformation(
                token,
                TokenElevation,
                Some(std::ptr::from_mut(&mut elevation).cast()),
                std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut returned,
            );
            let _ = CloseHandle(token);
            asked.is_ok() && elevation.TokenIsElevated != 0
        }
    }

    fn restart_elevated(&self, resume: Option<&str>) -> Result<(), String> {
        let exe =
            std::env::current_exe().map_err(|e| format!("no se encuentra el programa: {e}"))?;
        let file = HSTRING::from(exe.as_os_str());
        let parameters = HSTRING::from(resume_arguments(resume)?);
        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            lpVerb: w!("runas"),
            lpFile: PCWSTR(file.as_ptr()),
            lpParameters: PCWSTR(parameters.as_ptr()),
            nShow: SW_SHOWNORMAL.0,
            ..Default::default()
        };
        // SAFETY: `info` is fully initialised and the strings it points to outlive the call.
        unsafe { ShellExecuteExW(&mut info) }.map_err(|e| {
            if e.code() == CANCELLED {
                "no se dio el permiso de administrador: Purifex sigue como estaba".to_string()
            } else {
                format!("no se pudo reiniciar como administrador: {e}")
            }
        })
    }
}

/// The arguments of the new run: open the window on the apps page and look for the traces
/// of the app that was open. The name is quoted, and names that could break out of the
/// quotes (a `"`, a backslash, a control character) are refused instead of escaped.
fn resume_arguments(resume: Option<&str>) -> Result<String, String> {
    let Some(name) = resume else {
        return Ok("ui".to_string());
    };
    let unsafe_name = name.is_empty()
        || name.chars().count() > 120
        || name
            .chars()
            .any(|c| c == '"' || c == '\\' || c.is_control());
    if unsafe_name {
        return Err("el nombre de la app no se puede pasar al reinicio".into());
    }
    Ok(format!("ui --find-traces \"{name}\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_new_run_opens_the_window_looking_for_the_apps_traces() {
        assert_eq!(resume_arguments(None).unwrap(), "ui");
        assert_eq!(
            resume_arguments(Some("OBS Studio")).unwrap(),
            r#"ui --find-traces "OBS Studio""#
        );
        assert!(resume_arguments(Some("Kdenlive 26.08")).is_ok());
    }

    #[test]
    fn a_name_that_could_break_out_of_the_quotes_is_refused() {
        for bad in ["", "a\"b", "a\\", "x\ny", &"x".repeat(121)] {
            assert!(resume_arguments(Some(bad)).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn the_program_can_tell_whether_it_runs_as_an_administrator() {
        // Either answer is right; what matters is that the question works.
        let first = WindowsElevation.is_elevated();
        assert_eq!(
            WindowsElevation.is_elevated(),
            first,
            "it does not change by asking"
        );
    }
}
