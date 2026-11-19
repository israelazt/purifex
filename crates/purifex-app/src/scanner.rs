//! The analyzer: reads the inventory once and turns it into a report.

use purifex_domain::ports::{Clock, SystemInventory};
use purifex_domain::report::{ScanReport, Scope};

pub struct Scanner<I, C> {
    inventory: I,
    clock: C,
}

impl<I: SystemInventory, C: Clock> Scanner<I, C> {
    pub fn new(inventory: I, clock: C) -> Self {
        Self { inventory, clock }
    }

    /// The devices present, as the system lists them.
    pub fn devices(&self) -> Result<Vec<purifex_domain::device::Device>, String> {
        self.inventory.devices()
    }

    /// Reads the devices and builds the report, timing the reading.
    pub fn scan(&self, scope: Scope) -> Result<ScanReport, String> {
        let started = self.clock.monotonic();
        let devices = self.inventory.devices()?;
        let elapsed = self.clock.monotonic().saturating_sub(started);
        Ok(ScanReport::build(
            devices,
            scope,
            self.clock.today(),
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
        ))
    }
}

#[cfg(test)]
mod tests {
    use purifex_domain::device::Device;

    use super::*;
    use crate::testing::{FakeClock, FakeInventory};

    #[test]
    fn it_scans_and_times_the_reading() {
        let inventory = FakeInventory::with(vec![Device {
            instance_id: "PCI\\A".into(),
            name: "GPU".into(),
            ..Device::default()
        }]);
        let scanner = Scanner::new(inventory, FakeClock::ticking(7));
        let report = scanner.scan(Scope::Hardware).unwrap();
        assert_eq!(report.summary.shown, 1);
        assert_eq!(report.elapsed_ms, 7);
        assert_eq!(report.scanned_on, FakeClock::DAY);
    }

    #[test]
    fn a_failing_inventory_is_an_error_not_an_empty_report() {
        let scanner = Scanner::new(FakeInventory::failing("no access"), FakeClock::ticking(1));
        assert_eq!(scanner.scan(Scope::All).unwrap_err(), "no access");
    }
}
