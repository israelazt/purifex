//! The restore points of Windows (System Restore): the way back for the whole system.
//! Creating one needs an administrator; without it the call says so and nothing changes.

use purifex_domain::ports::RestorePoints;
use windows::Win32::System::Restore::{
    SRSetRestorePointW, APPLICATION_UNINSTALL, BEGIN_SYSTEM_CHANGE, END_SYSTEM_CHANGE, MAX_DESC_W,
    RESTOREPOINTINFOW, STATEMGRSTATUS,
};

pub struct WindowsRestorePoints;

impl RestorePoints for WindowsRestorePoints {
    fn create(&self, description: &str) -> Result<(), String> {
        let mut info = RESTOREPOINTINFOW {
            dwEventType: BEGIN_SYSTEM_CHANGE,
            dwRestorePtType: APPLICATION_UNINSTALL,
            ..Default::default()
        };
        // The struct is packed: the text is built apart and moved in whole. It is cut to leave
        // room for the end mark.
        let mut text = [0u16; MAX_DESC_W as usize];
        for (slot, unit) in text
            .iter_mut()
            .zip(description.encode_utf16().take(MAX_DESC_W as usize - 1))
        {
            *slot = unit;
        }
        info.szDescription = text;
        let mut status = STATEMGRSTATUS::default();
        // SAFETY: both pointers are live locals that the call only reads and fills.
        if !unsafe { SRSetRestorePointW(&info, &mut status) }.as_bool() {
            return Err(reason(status.nStatus.0));
        }
        // The change that the restore point covers starts now: the point is closed at once
        // because the uninstaller is another program, whose work is not tracked here.
        info.dwEventType = END_SYSTEM_CHANGE;
        info.llSequenceNumber = status.llSequenceNumber;
        // SAFETY: as above.
        let _ = unsafe { SRSetRestorePointW(&info, &mut status) };
        Ok(())
    }
}

/// What the code of System Restore means, in words.
fn reason(code: u32) -> String {
    match code {
        5 => "hace falta ejecutar Purifex como administrador".into(),
        1058 => "la Protección del sistema está desactivada en Windows".into(),
        112 => "no hay espacio en el disco para el punto".into(),
        other => format!("Windows no lo creó (código {other})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_codes_that_a_person_can_fix_are_explained() {
        assert!(reason(5).contains("administrador"));
        assert!(reason(1058).contains("Protección del sistema"));
        assert!(reason(9999).contains("9999"));
    }
}
