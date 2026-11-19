//! The drivers that Windows Update offers for this computer, asked through the
//! Windows Update Agent (the same service Windows itself uses). Searching does
//! not download or install anything: it only lists what is offered. It goes out
//! to the network and can take a minute, so it is called off the interface thread.

use purifex_domain::driver_offers::{version_in_title, Offer};
use purifex_domain::ports::DriverOffers;
use windows::core::{Interface, BSTR};
use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::System::UpdateAgent::{IUpdateSession, IWindowsDriverUpdate, UpdateSession};

use crate::decode::date_from_ole;

/// Drivers not installed and not hidden by the user.
const CRITERIA: &str = "IsInstalled=0 and Type='Driver' and IsHidden=0";

pub struct WindowsUpdateOffers;

impl DriverOffers for WindowsUpdateOffers {
    fn search(&self) -> Result<Vec<Offer>, String> {
        // SAFETY: COM is initialised for this thread before any call and released
        // after the last one; every interface lives inside `list`.
        unsafe {
            let init = CoInitializeEx(None, COINIT_MULTITHREADED);
            // `RPC_E_CHANGED_MODE`: the thread already has COM in another mode,
            // which is fine to use (and not ours to release).
            let ours = init.is_ok();
            if !ours && init != RPC_E_CHANGED_MODE {
                return Err(format!("COM no se pudo iniciar ({init:?})"));
            }
            let result = list();
            if ours {
                CoUninitialize();
            }
            result
        }
    }
}

/// # Safety
/// COM must be initialised on the calling thread.
unsafe fn list() -> Result<Vec<Offer>, String> {
    let fail = |what: &str| {
        let what = what.to_string();
        move |e: windows::core::Error| format!("{what}: {}", e.message().trim())
    };
    // SAFETY: the caller initialised COM; these are plain COM calls on live objects.
    unsafe {
        let session: IUpdateSession = CoCreateInstance(&UpdateSession, None, CLSCTX_INPROC_SERVER)
            .map_err(fail("Windows Update no está disponible"))?;
        let searcher = session
            .CreateUpdateSearcher()
            .map_err(fail("no se pudo crear la búsqueda"))?;
        let found = searcher
            .Search(&BSTR::from(CRITERIA))
            .map_err(fail("la búsqueda en Windows Update falló"))?;
        let updates = found
            .Updates()
            .map_err(fail("no se pudo leer el resultado"))?;
        let count = updates
            .Count()
            .map_err(fail("no se pudo leer el resultado"))?;
        let mut offers = Vec::new();
        for i in 0..count {
            let Ok(update) = updates.get_Item(i) else {
                continue;
            };
            let Ok(driver) = update.cast::<IWindowsDriverUpdate>() else {
                continue;
            };
            let text =
                |r: windows::core::Result<BSTR>| r.map(|b| b.to_string()).unwrap_or_default();
            let hardware_id = text(driver.DriverHardwareID());
            if hardware_id.trim().is_empty() {
                continue;
            }
            let title = text(update.Title());
            offers.push(Offer {
                hardware_id,
                version: version_in_title(&title),
                title,
                provider: text(driver.DriverProvider()),
                date: driver.DriverVerDate().ok().and_then(date_from_ole),
            });
        }
        Ok(offers)
    }
}
