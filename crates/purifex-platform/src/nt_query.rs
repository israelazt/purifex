//! The tables of the system that `NtQuerySystemInformation` gives (all the
//! processes, the times of every core), read into a buffer that grows as the
//! system asks, with a ceiling. Used by the sampler (every second) and by the
//! fixes (to know whether a program is running).

use windows::Wdk::System::SystemInformation::{NtQuerySystemInformation, SYSTEM_INFORMATION_CLASS};

use crate::nt_layout::parse_processes;

pub(crate) const PROCESSES: SYSTEM_INFORMATION_CLASS = SYSTEM_INFORMATION_CLASS(5);
pub(crate) const CORES: SYSTEM_INFORMATION_CLASS = SYSTEM_INFORMATION_CLASS(8);
/// Size of one core's entry in the performance table.
pub(crate) const CORE_ENTRY: usize = 48;
const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xC000_0004_u32 as i32;
/// The most memory a system table may take.
const MAX_TABLE: usize = 64 * 1024 * 1024;

/// Fills `buffer` with a system table, growing it as the system asks, and says
/// how many bytes are the table. `unit` is the size the table must be a multiple
/// of (its entry size, for the tables that insist on it).
pub(crate) fn query(
    buffer: &mut Vec<u8>,
    class: SYSTEM_INFORMATION_CLASS,
    unit: usize,
) -> Result<usize, String> {
    let round = |n: usize| n.div_ceil(unit) * unit;
    if buffer.len() < 64 * 1024 {
        buffer.resize(round(256 * 1024), 0);
    }
    buffer.truncate(round(buffer.len()).min(buffer.len()));
    buffer.resize(round(buffer.len()), 0);
    for _ in 0..8 {
        let mut needed = 0u32;
        // SAFETY: the buffer is as long as the length passed.
        let status = unsafe {
            NtQuerySystemInformation(
                class,
                buffer.as_mut_ptr().cast(),
                buffer.len() as u32,
                &mut needed,
            )
        };
        if status.0 >= 0 {
            return Ok(if needed == 0 {
                buffer.len()
            } else {
                needed as usize
            });
        }
        if status.0 != STATUS_INFO_LENGTH_MISMATCH {
            return Err(format!(
                "NtQuerySystemInformation falló: 0x{:08x}",
                status.0 as u32
            ));
        }
        let size = round((needed as usize + 64 * 1024).max(buffer.len() * 2));
        if size > MAX_TABLE {
            return Err("la tabla del sistema es demasiado grande".into());
        }
        buffer.resize(size, 0);
    }
    Err("la tabla del sistema no deja de crecer".into())
}

/// The names of the programs running now (`LeagueClient.exe`).
pub fn running_programs() -> Result<Vec<String>, String> {
    let mut buffer = Vec::new();
    let used = query(&mut buffer, PROCESSES, 1)?;
    Ok(parse_processes(&buffer[..used.min(buffer.len())])
        .into_iter()
        .map(|p| p.name)
        .collect())
}
