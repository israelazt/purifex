//! The graphics card through NVML, the library that comes with NVIDIA's driver
//! (`nvml.dll`). It is loaded when needed and its absence (another brand of
//! card) is not an error: there are just no GPU figures.

use std::ffi::c_void;

use purifex_domain::sample::GpuSample;
use windows::core::{s, w, PCWSTR};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

type Device = *mut c_void;

#[repr(C)]
#[derive(Default)]
struct Utilization {
    gpu: u32,
    memory: u32,
}

const CLOCK_GRAPHICS: u32 = 0;
const TEMPERATURE_GPU: u32 = 0;

/// The slow readings (temperature, throttling) are taken every this many
/// samples and held in between: heat and power limits last for seconds, and
/// the driver takes ~20 ms to answer the throttling one.
const SLOW_EVERY: u32 = 5;

pub struct Nvml {
    device: Device,
    max_clock_mhz: u32,
    reads: u32,
    slow: (u32, u64),
    utilization: unsafe extern "C" fn(Device, *mut Utilization) -> i32,
    temperature: unsafe extern "C" fn(Device, u32, *mut u32) -> i32,
    clock: unsafe extern "C" fn(Device, u32, *mut u32) -> i32,
    power: unsafe extern "C" fn(Device, *mut u32) -> i32,
    throttle: unsafe extern "C" fn(Device, *mut u64) -> i32,
}

macro_rules! symbol {
    ($lib:expr, $name:expr) => {{
        // SAFETY: the library is loaded; the type it is cast to is NVML's
        // documented signature for that name.
        let found = unsafe { GetProcAddress($lib, $name) }?;
        unsafe { std::mem::transmute(found) }
    }};
}

impl Nvml {
    /// Loads NVML and opens the first card, or `None` if there is none.
    pub fn open() -> Option<Self> {
        let lib = [
            w!("nvml.dll"),
            w!(r"C:\Program Files\NVIDIA Corporation\NVSMI\nvml.dll"),
        ]
        .into_iter()
        // SAFETY: a valid, NUL-ended wide string.
        .find_map(|path: PCWSTR| unsafe { LoadLibraryW(path) }.ok())?;
        Self::bind(lib)
    }

    fn bind(lib: HMODULE) -> Option<Self> {
        let init: unsafe extern "C" fn() -> i32 = symbol!(lib, s!("nvmlInit_v2"));
        let by_index: unsafe extern "C" fn(u32, *mut Device) -> i32 =
            symbol!(lib, s!("nvmlDeviceGetHandleByIndex_v2"));
        let utilization = symbol!(lib, s!("nvmlDeviceGetUtilizationRates"));
        let temperature = symbol!(lib, s!("nvmlDeviceGetTemperature"));
        let clock = symbol!(lib, s!("nvmlDeviceGetClockInfo"));
        let max_clock: unsafe extern "C" fn(Device, u32, *mut u32) -> i32 =
            symbol!(lib, s!("nvmlDeviceGetMaxClockInfo"));
        let power = symbol!(lib, s!("nvmlDeviceGetPowerUsage"));
        let throttle = symbol!(lib, s!("nvmlDeviceGetCurrentClocksThrottleReasons"));
        let mut device: Device = std::ptr::null_mut();
        // SAFETY: documented calls, with a live output.
        let opened = unsafe { init() == 0 && by_index(0, &mut device) == 0 };
        if !opened {
            return None;
        }
        // It never changes: read once.
        let mut max_clock_mhz = 0u32;
        // SAFETY: a documented call on an open device, with a live output.
        unsafe { max_clock(device, CLOCK_GRAPHICS, &mut max_clock_mhz) };
        Some(Self {
            device,
            max_clock_mhz,
            reads: 0,
            slow: (0, 0),
            utilization,
            temperature,
            clock,
            power,
            throttle,
        })
    }

    /// One reading. The use of the card is required; the rest are zero if the
    /// driver does not answer them.
    pub fn read(&mut self) -> Option<GpuSample> {
        let mut util = Utilization::default();
        let (mut clock, mut power) = (0u32, 0u32);
        // SAFETY: documented calls on an open device, each with a live output.
        unsafe {
            if (self.utilization)(self.device, &mut util) != 0 {
                return None;
            }
            (self.clock)(self.device, CLOCK_GRAPHICS, &mut clock);
            (self.power)(self.device, &mut power);
            if self.reads % SLOW_EVERY == 0 {
                let (mut temp, mut throttle) = (0u32, 0u64);
                (self.temperature)(self.device, TEMPERATURE_GPU, &mut temp);
                (self.throttle)(self.device, &mut throttle);
                self.slow = (temp, throttle);
            }
        }
        self.reads = self.reads.wrapping_add(1);
        Some(GpuSample {
            util: util.gpu as f32,
            memory_util: util.memory as f32,
            temp_c: self.slow.0,
            clock_mhz: clock,
            max_clock_mhz: self.max_clock_mhz,
            power_w: power as f32 / 1000.0,
            throttle: self.slow.1,
        })
    }
}
