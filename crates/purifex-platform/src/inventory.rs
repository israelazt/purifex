//! The devices of the computer through SetupAPI / CfgMgr32: the same source the
//! Device Manager reads, in memory and without starting other programs (the
//! slow route, WMI, takes seconds; this one, milliseconds).

use purifex_domain::date::Date;
use purifex_domain::device::{Device, InstalledDriver};
use purifex_domain::log_warn;
use purifex_domain::ports::SystemInventory;
use purifex_domain::version::DriverVersion;
use windows::core::PCWSTR;
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    CM_Get_DevNode_Status, SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInfo,
    SetupDiGetClassDevsW, SetupDiGetDevicePropertyW, CM_DEVNODE_STATUS_FLAGS, CM_PROB, CR_SUCCESS,
    DIGCF_ALLCLASSES, DIGCF_PRESENT, HDEVINFO, SP_DEVINFO_DATA,
};
use windows::Win32::Devices::Properties::{
    DEVPKEY_Device_Class, DEVPKEY_Device_DeviceDesc, DEVPKEY_Device_DriverDate,
    DEVPKEY_Device_DriverInfPath, DEVPKEY_Device_DriverProvider, DEVPKEY_Device_DriverVersion,
    DEVPKEY_Device_FriendlyName, DEVPKEY_Device_HardwareIds, DEVPKEY_Device_InstanceId,
    DEVPKEY_Device_Manufacturer, DEVPROPTYPE,
};
use windows::Win32::Foundation::{DEVPROPKEY, HWND};

use crate::decode::{u64_le, utf16_list, utf16_string};

pub struct SetupApiInventory;

/// Frees the list of devices when it goes out of scope.
struct DeviceSet(HDEVINFO);

impl Drop for DeviceSet {
    fn drop(&mut self) {
        // SAFETY: the handle came from `SetupDiGetClassDevsW` and is freed once.
        unsafe {
            let _ = SetupDiDestroyDeviceInfoList(self.0);
        }
    }
}

/// Reads properties of one device, reusing one buffer for all of them.
struct Properties<'a> {
    set: &'a DeviceSet,
    data: SP_DEVINFO_DATA,
    buffer: Vec<u8>,
}

impl Properties<'_> {
    /// The raw bytes of a property, or `None` if the device does not have it.
    fn bytes(&mut self, key: &DEVPROPKEY) -> Option<&[u8]> {
        let mut kind = DEVPROPTYPE::default();
        let mut needed = 0u32;
        self.buffer.resize(512, 0);
        // SAFETY: every pointer is to a live local; the buffer is as long as
        // the slice says.
        let first = unsafe {
            SetupDiGetDevicePropertyW(
                self.set.0,
                &self.data,
                key,
                &mut kind,
                Some(&mut self.buffer),
                Some(&mut needed),
                0,
            )
        };
        if first.is_err() {
            if needed as usize <= self.buffer.len() {
                return None;
            }
            self.buffer.resize(needed as usize, 0);
            // SAFETY: as above, with a buffer now big enough.
            unsafe {
                SetupDiGetDevicePropertyW(
                    self.set.0,
                    &self.data,
                    key,
                    &mut kind,
                    Some(&mut self.buffer),
                    Some(&mut needed),
                    0,
                )
            }
            .ok()?;
        }
        self.buffer.get(..needed as usize)
    }

    fn string(&mut self, key: &DEVPROPKEY) -> Option<String> {
        self.bytes(key).and_then(utf16_string)
    }
}

impl SetupApiInventory {
    fn read(set: &DeviceSet, data: SP_DEVINFO_DATA) -> Device {
        let problem = problem_of(data.DevInst);
        let mut p = Properties {
            set,
            data,
            buffer: Vec::new(),
        };
        let name = p
            .string(&DEVPKEY_Device_FriendlyName)
            .or_else(|| p.string(&DEVPKEY_Device_DeviceDesc))
            .unwrap_or_default();
        let hardware_ids = p
            .bytes(&DEVPKEY_Device_HardwareIds)
            .map(utf16_list)
            .unwrap_or_default();
        let driver = InstalledDriver {
            provider: p.string(&DEVPKEY_Device_DriverProvider),
            version: p
                .string(&DEVPKEY_Device_DriverVersion)
                .and_then(|v| DriverVersion::parse(&v)),
            date: p
                .bytes(&DEVPKEY_Device_DriverDate)
                .and_then(u64_le)
                .and_then(Date::from_filetime),
            inf: p.string(&DEVPKEY_Device_DriverInfPath),
        };
        Device {
            instance_id: p.string(&DEVPKEY_Device_InstanceId).unwrap_or_default(),
            name,
            class: p.string(&DEVPKEY_Device_Class).unwrap_or_default(),
            manufacturer: p.string(&DEVPKEY_Device_Manufacturer),
            hardware_ids,
            driver: (driver != InstalledDriver::default()).then_some(driver),
            problem,
        }
    }
}

/// The problem code of the device manager for a device node, if it has one.
fn problem_of(devinst: u32) -> Option<u32> {
    let mut status = CM_DEVNODE_STATUS_FLAGS(0);
    let mut problem = CM_PROB(0);
    // SAFETY: both outputs are live locals.
    let result = unsafe { CM_Get_DevNode_Status(&mut status, &mut problem, devinst, 0) };
    (result == CR_SUCCESS && problem.0 != 0).then_some(problem.0)
}

impl SystemInventory for SetupApiInventory {
    fn devices(&self) -> Result<Vec<Device>, String> {
        // SAFETY: no class, no enumerator and no parent window means "all
        // present devices".
        let set = unsafe {
            SetupDiGetClassDevsW(
                None,
                PCWSTR::null(),
                Some(HWND::default()),
                DIGCF_PRESENT | DIGCF_ALLCLASSES,
            )
        }
        .map_err(|e| format!("could not list the devices: {e}"))?;
        let set = DeviceSet(set);
        let mut devices = Vec::with_capacity(512);
        for index in 0.. {
            let mut data = SP_DEVINFO_DATA {
                cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32,
                ..SP_DEVINFO_DATA::default()
            };
            // SAFETY: `data` has its size set, as the function requires.
            if unsafe { SetupDiEnumDeviceInfo(set.0, index, &mut data) }.is_err() {
                break;
            }
            let device = Self::read(&set, data);
            if device.instance_id.is_empty() {
                log_warn!("device {index} has no instance id; skipped");
                continue;
            }
            devices.push(device);
        }
        Ok(devices)
    }
}
