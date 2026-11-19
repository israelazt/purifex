//! The driver candidates proposed from outside (the user's AI, through MCP). A
//! proposal is judged by the rules of the domain before it is kept; keeping it
//! installs nothing and opens nothing.

use purifex_domain::device::Device;
use purifex_domain::driver_candidate::{judge, Candidate, Checks, Source, Submission, Verdict};
use purifex_domain::ports::CandidateStore;
use purifex_domain::version::DriverVersion;

#[derive(Default)]
pub struct Candidates {
    store: Option<Box<dyn CandidateStore>>,
}

impl Candidates {
    pub fn new(store: Box<dyn CandidateStore>) -> Self {
        Self { store: Some(store) }
    }

    pub fn list(&self) -> Result<Vec<Candidate>, String> {
        self.store.as_ref().map_or(Ok(Vec::new()), |s| s.all())
    }

    /// Judges a proposal for one of `devices` and keeps it unless it is rejected.
    pub fn submit(
        &mut self,
        devices: &[Device],
        device_id: &str,
        url: &str,
        version: &str,
        title: &str,
    ) -> Result<Submission, String> {
        let device = devices
            .iter()
            .find(|d| d.instance_id == device_id)
            .ok_or("no hay ningún dispositivo con ese id: usa los de list_unresolved_devices")?;
        let version = DriverVersion::parse(version)
            .ok_or("la versión debe ser números separados por puntos, como 6.0.9.7000")?;
        let candidate = Candidate {
            device: device.instance_id.clone(),
            source: Source::Suggested,
            url: url.trim().to_string(),
            version,
            title: Candidate::clean_title(title),
        };
        let verdict = judge(device, &candidate, &Checks::default());
        let keep = verdict == Verdict::OfficialPage;
        let mut stored = false;
        if keep {
            let store = self
                .store
                .as_mut()
                .ok_or("esta versión no puede guardar propuestas")?;
            let known = store.all()?.iter().any(|c| {
                c.device == candidate.device && c.url == candidate.url && c.version == version
            });
            if !known {
                store.add(&candidate)?;
            }
            stored = true;
        }
        Ok(Submission {
            candidate,
            verdict,
            stored,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use purifex_domain::device::InstalledDriver;
    use purifex_domain::driver_candidate::Reason;

    use super::*;

    #[derive(Clone, Default)]
    struct Memory(Rc<RefCell<Vec<Candidate>>>);

    impl CandidateStore for Memory {
        fn all(&self) -> Result<Vec<Candidate>, String> {
            Ok(self.0.borrow().clone())
        }
        fn add(&mut self, c: &Candidate) -> Result<(), String> {
            self.0.borrow_mut().push(c.clone());
            Ok(())
        }
    }

    fn bt() -> Device {
        Device {
            instance_id: r"USB\VID_0BDA&PID_A728\1".into(),
            name: "Realtek BT".into(),
            class: "Bluetooth".into(),
            hardware_ids: vec![r"USB\VID_0BDA&PID_A728".into()],
            driver: Some(InstalledDriver {
                version: DriverVersion::parse("1.9.1051.3013"),
                ..InstalledDriver::default()
            }),
            problem: Some(31),
            ..Device::default()
        }
    }

    fn kept() -> (Candidates, Memory) {
        let memory = Memory::default();
        (Candidates::new(Box::new(memory.clone())), memory)
    }

    #[test]
    fn an_official_page_for_a_known_device_is_kept_once() {
        let (mut c, memory) = kept();
        let url = "https://www.realtek.com/Download/List?cate_id=586";
        let first = c
            .submit(
                &[bt()],
                &bt().instance_id,
                url,
                "1.9.1052.0",
                "Realtek BT\ndriver",
            )
            .unwrap();
        assert_eq!(first.verdict, Verdict::OfficialPage);
        assert!(first.stored);
        assert_eq!(first.candidate.title, "Realtek BT driver");
        c.submit(&[bt()], &bt().instance_id, url, "1.9.1052.0", "")
            .unwrap();
        assert_eq!(
            memory.0.borrow().len(),
            1,
            "the same proposal is not kept twice"
        );
        assert_eq!(c.list().unwrap().len(), 1);
    }

    #[test]
    fn a_rejected_proposal_is_answered_with_why_and_is_not_kept() {
        let (mut c, memory) = kept();
        let bad = c
            .submit(
                &[bt()],
                &bt().instance_id,
                "https://realtek.com.evil.io/x.exe",
                "9.0",
                "",
            )
            .unwrap();
        assert_eq!(
            bad.verdict,
            Verdict::Rejected {
                reasons: vec![Reason::UnknownDomain]
            }
        );
        assert!(!bad.stored);
        let old = c
            .submit(
                &[bt()],
                &bt().instance_id,
                "https://www.realtek.com/x",
                "1.0",
                "",
            )
            .unwrap();
        assert!(
            matches!(old.verdict, Verdict::Rejected { .. }),
            "not newer than the installed one"
        );
        assert!(memory.0.borrow().is_empty());
    }

    #[test]
    fn an_unknown_device_or_a_bad_version_is_an_error_not_a_guess() {
        let (mut c, _) = kept();
        let err = c
            .submit(&[bt()], "PCI\\NOPE", "https://www.realtek.com/x", "1.0", "")
            .unwrap_err();
        assert!(err.contains("list_unresolved_devices"));
        let err = c
            .submit(
                &[bt()],
                &bt().instance_id,
                "https://www.realtek.com/x",
                "latest",
                "",
            )
            .unwrap_err();
        assert!(err.contains("versión"));
    }

    #[test]
    fn without_a_store_nothing_can_be_kept_and_listing_is_empty() {
        let mut none = Candidates::default();
        assert_eq!(none.list().unwrap(), vec![]);
        let err = none
            .submit(
                &[bt()],
                &bt().instance_id,
                "https://www.realtek.com/x",
                "2.0",
                "",
            )
            .unwrap_err();
        assert!(err.contains("no puede guardar"));
    }
}
