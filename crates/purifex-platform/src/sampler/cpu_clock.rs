//! The clock speed the CPU really runs at, from the performance counter of
//! Windows (`% Processor Performance`, as a share of the base clock). The
//! clock that `CallNtPowerInformation` returns is the base one even when the
//! CPU is boosting or saving power, so it cannot be used for this.

use windows::core::{w, PCWSTR};
use windows::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterValue,
    PdhOpenQueryW, PDH_FMT_COUNTERVALUE, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY,
};

const PERFORMANCE: PCWSTR = w!(r"\Processor Information(_Total)\% Processor Performance");
/// `ERROR_SUCCESS` and the counter status of a good value.
const OK: u32 = 0;

pub struct CpuClock {
    query: PDH_HQUERY,
    counter: PDH_HCOUNTER,
}

impl CpuClock {
    /// Opens the counter, or `None` if the system does not have it.
    pub fn open() -> Option<Self> {
        let mut query = PDH_HQUERY::default();
        let mut counter = PDH_HCOUNTER::default();
        // SAFETY: the outputs are live locals; the query is closed in `Drop`
        // (or right here when the counter cannot be added).
        unsafe {
            if PdhOpenQueryW(PCWSTR::null(), 0, &mut query) != OK {
                return None;
            }
            if PdhAddEnglishCounterW(query, PERFORMANCE, 0, &mut counter) != OK {
                PdhCloseQuery(query);
                return None;
            }
            // The first reading only sets the baseline of the difference.
            PdhCollectQueryData(query);
        }
        Some(Self { query, counter })
    }

    /// The performance of the CPU as a percent of its base clock, since the
    /// last reading. `None` if there is no valid value yet.
    pub fn read(&mut self) -> Option<f64> {
        let mut value = PDH_FMT_COUNTERVALUE::default();
        // SAFETY: the handles are open (see `open`) and the output is live.
        unsafe {
            if PdhCollectQueryData(self.query) != OK {
                return None;
            }
            if PdhGetFormattedCounterValue(self.counter, PDH_FMT_DOUBLE, None, &mut value) != OK {
                return None;
            }
            (value.CStatus == OK).then_some(value.Anonymous.doubleValue)
        }
    }
}

impl Drop for CpuClock {
    fn drop(&mut self) {
        // SAFETY: the query was opened in `open` and is closed once.
        unsafe {
            PdhCloseQuery(self.query);
        }
    }
}
