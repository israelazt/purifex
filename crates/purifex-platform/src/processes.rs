//! The programs running now, with the place of their `.exe` (a snapshot of the process list,
//! then each one asked for its image path), and ending one. Ending is `TerminateProcess`: the
//! program has no chance to save, which is why the person confirms before.

use purifex_domain::forced::RunningProcess;
use purifex_domain::ports::Processes;
use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, HANDLE};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, TerminateProcess, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
};

pub struct WindowsProcesses;

/// Closes the handle when it goes out of scope.
struct Handle(HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the handle was opened by this program and is closed once.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

fn pids() -> Result<Vec<u32>, String> {
    // SAFETY: a snapshot handle, closed by `Handle`; the entry is a live local whose size is set.
    unsafe {
        let snapshot = Handle(
            CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
                .map_err(|e| format!("no se pudo leer la lista de programas: {e}"))?,
        );
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut pids = Vec::new();
        let mut more = Process32FirstW(snapshot.0, &mut entry).is_ok();
        while more {
            pids.push(entry.th32ProcessID);
            more = Process32NextW(snapshot.0, &mut entry).is_ok();
        }
        Ok(pids)
    }
}

/// The path of the `.exe` of a program, if it can be read (a system one may refuse).
pub(crate) fn image_path(pid: u32) -> Option<String> {
    // SAFETY: the handle is closed by `Handle`; the buffer is as long as `size` says.
    unsafe {
        let process = Handle(OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?);
        let mut buffer = vec![0u16; 32_768];
        let mut size = buffer.len() as u32;
        QueryFullProcessImageNameW(
            process.0,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        )
        .ok()?;
        Some(String::from_utf16_lossy(&buffer[..size as usize]))
    }
}

impl Processes for WindowsProcesses {
    fn running(&self) -> Result<Vec<RunningProcess>, String> {
        Ok(pids()?
            .into_iter()
            .filter_map(|pid| {
                Some(RunningProcess {
                    pid,
                    path: image_path(pid)?,
                })
            })
            .collect())
    }

    fn close(&mut self, pid: u32) -> Result<(), String> {
        // SAFETY: the handle is closed by `Handle`.
        unsafe {
            let process = Handle(OpenProcess(PROCESS_TERMINATE, false, pid).map_err(|e| {
                if e.code() == ERROR_ACCESS_DENIED.to_hresult() {
                    "hace falta ejecutar Purifex como administrador".to_string()
                } else {
                    format!("ya no está abierto o no se puede abrir: {e}")
                }
            })?);
            TerminateProcess(process.0, 1).map_err(|e| format!("Windows no lo cerró: {e}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::process::{Command, Stdio};

    use purifex_domain::forced::running_in;

    use super::*;

    #[test]
    fn a_program_running_from_a_folder_is_found_by_it_and_closed() {
        let folder = std::env::temp_dir().join(format!("purifex-proc-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let exe = folder.join("pfxtest.exe");
        let ping = std::env::var("SystemRoot").unwrap() + r"\System32\PING.EXE";
        std::fs::copy(ping, &exe).unwrap();
        let mut child = Command::new(&exe)
            .args(["-t", "127.0.0.1"])
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let mut real = WindowsProcesses;
        let mine = running_in(
            &folder.to_string_lossy(),
            &real.running().unwrap(),
            std::process::id(),
        );
        let pids: Vec<u32> = mine.iter().map(|p| p.pid).collect();
        assert_eq!(pids, [child.id()], "only the one started from the folder");
        assert_eq!(mine[0].name(), "pfxtest.exe");
        real.close(child.id()).unwrap();
        let status = child.wait().unwrap();
        assert!(!status.success());
        let _ = std::fs::remove_dir_all(folder);
    }
}
