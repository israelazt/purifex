//! Reads the tables that `NtQuerySystemInformation` fills (x64 layout):
//! every process with its times and memory, and the times of every core. They
//! are plain bytes, so this is tested without calling the system. One call
//! returns all the processes, protected ones (anti-cheats) included, which
//! opening each process one by one could not do.

/// A process, as the system's table lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawProcess {
    pub pid: u64,
    pub name: String,
    /// User plus kernel time used since it started, in 100 ns ticks.
    pub cpu_ticks: u64,
    pub working_set: u64,
}

/// The times of one logical core, in 100 ns ticks since boot.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CoreTimes {
    pub idle: u64,
    /// Kernel plus user time (the idle time is inside the kernel's).
    pub total: u64,
    /// Time in deferred calls and interrupts (drivers).
    pub dpc_interrupt: u64,
}

const PROCESS_FIXED: usize = 152;
const CORE_ENTRY: usize = 48;

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn u64_at(bytes: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(bytes.get(at..at + 8)?.try_into().ok()?))
}

/// `SystemProcessInformation`: a chain of entries, each saying how far the
/// next one is. The name is a pointer into the same buffer.
pub fn parse_processes(buffer: &[u8]) -> Vec<RawProcess> {
    let mut out = Vec::with_capacity(256);
    let mut at = 0usize;
    loop {
        let Some(entry) = buffer.get(at..).filter(|e| e.len() >= PROCESS_FIXED) else {
            break;
        };
        let next = u32_at(entry, 0).unwrap_or(0) as usize;
        let user = u64_at(entry, 40).unwrap_or(0);
        let kernel = u64_at(entry, 48).unwrap_or(0);
        let name_len = u32_at(entry, 56).unwrap_or(0) as usize & 0xFFFF;
        let name_ptr = u64_at(entry, 64).unwrap_or(0) as usize;
        let name = name_from(buffer, name_ptr, name_len);
        out.push(RawProcess {
            pid: u64_at(entry, 80).unwrap_or(0),
            name,
            cpu_ticks: user.saturating_add(kernel),
            working_set: u64_at(entry, 144).unwrap_or(0),
        });
        if next == 0 {
            break;
        }
        at += next;
    }
    out
}

/// The name lives in the same buffer; its pointer is checked against it.
fn name_from(buffer: &[u8], pointer: usize, length: usize) -> String {
    let base = buffer.as_ptr() as usize;
    pointer
        .checked_sub(base)
        .and_then(|from| buffer.get(from..from.checked_add(length)?))
        .map(|bytes| {
            let units: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            String::from_utf16_lossy(&units)
        })
        .unwrap_or_default()
}

/// `SystemProcessorPerformanceInformation`: one 48-byte entry per core.
pub fn parse_cores(buffer: &[u8]) -> Vec<CoreTimes> {
    buffer
        .chunks_exact(CORE_ENTRY)
        .map(|e| {
            let (idle, kernel, user) = (
                u64_at(e, 0).unwrap_or(0),
                u64_at(e, 8).unwrap_or(0),
                u64_at(e, 16).unwrap_or(0),
            );
            CoreTimes {
                idle,
                total: kernel.saturating_add(user),
                dpc_interrupt: u64_at(e, 24)
                    .unwrap_or(0)
                    .saturating_add(u64_at(e, 32).unwrap_or(0)),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a table of entries the way the system does, with the names after
    /// the entries inside the same buffer.
    fn table(entries: &[(&str, u64, u64, u64, u64)]) -> Vec<u8> {
        let stride = PROCESS_FIXED + 64;
        let mut buffer = vec![0u8; stride * entries.len()];
        let base = buffer.as_ptr() as usize;
        for (i, (name, pid, user, kernel, ws)) in entries.iter().enumerate() {
            let at = i * stride;
            let wide: Vec<u8> = name.encode_utf16().flat_map(u16::to_le_bytes).collect();
            let name_at = at + PROCESS_FIXED;
            buffer[name_at..name_at + wide.len()].copy_from_slice(&wide);
            let next = if i + 1 < entries.len() {
                stride as u32
            } else {
                0
            };
            buffer[at..at + 4].copy_from_slice(&next.to_le_bytes());
            buffer[at + 40..at + 48].copy_from_slice(&user.to_le_bytes());
            buffer[at + 48..at + 56].copy_from_slice(&kernel.to_le_bytes());
            buffer[at + 56..at + 58].copy_from_slice(&(wide.len() as u16).to_le_bytes());
            buffer[at + 64..at + 72].copy_from_slice(&((base + name_at) as u64).to_le_bytes());
            buffer[at + 80..at + 88].copy_from_slice(&pid.to_le_bytes());
            buffer[at + 144..at + 152].copy_from_slice(&ws.to_le_bytes());
        }
        buffer
    }

    #[test]
    fn it_walks_the_chain_of_processes() {
        let buffer = table(&[
            ("League of Legends.exe", 4242, 700, 300, 2_000_000),
            ("vgc.exe", 8, 5, 5, 10),
        ]);
        let processes = parse_processes(&buffer);
        assert_eq!(processes.len(), 2);
        assert_eq!(
            processes[0],
            RawProcess {
                pid: 4242,
                name: "League of Legends.exe".into(),
                cpu_ticks: 1000,
                working_set: 2_000_000
            }
        );
        assert_eq!(processes[1].name, "vgc.exe");
    }

    #[test]
    fn a_pointer_outside_the_buffer_gives_no_name_instead_of_a_crash() {
        let mut buffer = table(&[("x.exe", 1, 0, 0, 0)]);
        buffer[64..72].copy_from_slice(&1u64.to_le_bytes());
        assert_eq!(parse_processes(&buffer)[0].name, "");
        assert!(parse_processes(&[]).is_empty());
        assert!(parse_processes(&[0; 10]).is_empty());
    }

    #[test]
    fn it_reads_the_times_of_each_core() {
        let mut buffer = vec![0u8; CORE_ENTRY * 2];
        for (i, (idle, kernel, user, dpc, int)) in
            [(10u64, 60u64, 40u64, 3u64, 2u64), (0, 1, 1, 0, 0)]
                .iter()
                .enumerate()
        {
            for (j, v) in [idle, kernel, user, dpc, int].iter().enumerate() {
                let at = i * CORE_ENTRY + j * 8;
                buffer[at..at + 8].copy_from_slice(&v.to_le_bytes());
            }
        }
        let cores = parse_cores(&buffer);
        assert_eq!(
            cores[0],
            CoreTimes {
                idle: 10,
                total: 100,
                dpc_interrupt: 5
            }
        );
        assert_eq!(cores.len(), 2);
    }
}
