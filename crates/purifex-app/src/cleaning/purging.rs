//! Looking after the quarantine: what it holds, emptying it when the person asks, and
//! letting go of what has been there for `QUARANTINE_DAYS` days. What is deleted is only
//! ever a copy inside the quarantine folder (the store refuses anything else), and every
//! deletion is a line of the log, so the history says what can no longer be put back.

use purifex_domain::cleanup_log::{next_seq, CleanupEntry, CleanupState};
use purifex_domain::date::Date;
use purifex_domain::quarantine::{expired, open_removals, PurgeReport, QuarantineInfo};

use super::Cleaning;

/// Which removals to let go.
#[derive(Clone, Copy)]
enum Which {
    /// Everything that is still in the quarantine.
    Everything,
    /// What has been there long enough.
    Expired(Date),
}

impl Cleaning {
    /// What the quarantine holds now.
    pub fn quarantine_info(&mut self) -> Result<QuarantineInfo, String> {
        let today = self.today().ok_or("esta versión no tiene reloj")?;
        let store = self.store()?;
        let bytes = store.quarantine_bytes()?;
        Ok(QuarantineInfo::of(&store.entries()?, bytes, today))
    }

    /// Deletes for good everything in the quarantine. Without `confirmed` it only says how
    /// much it would delete.
    pub fn empty_quarantine(&mut self, confirmed: bool) -> Result<PurgeReport, String> {
        self.purge(Which::Everything, confirmed)
    }

    /// Lets go of what has been in the quarantine for `QUARANTINE_DAYS` days. The person
    /// agreed to this when the removal was made (it says so), so it needs no question.
    pub fn expire_quarantine(&mut self) -> Result<PurgeReport, String> {
        let today = self.today().ok_or("esta versión no tiene reloj")?;
        self.purge(Which::Expired(today), true)
    }

    fn purge(&mut self, which: Which, confirmed: bool) -> Result<PurgeReport, String> {
        let today = self.today();
        let store = self.store()?;
        let mut log = store.entries()?;
        let chosen: Vec<CleanupEntry> = match which {
            Which::Everything => open_removals(&log).into_iter().cloned().collect(),
            Which::Expired(today) => expired(&log, today).into_iter().cloned().collect(),
        };
        if !confirmed {
            return Ok(PurgeReport {
                preview: true,
                purged: chosen.len(),
                bytes: if chosen.is_empty() {
                    0
                } else {
                    store.quarantine_bytes()?
                },
                failed: vec![],
            });
        }
        let mut report = PurgeReport::default();
        for entry in &chosen {
            match store.purge(entry) {
                Ok(freed) => {
                    let line = CleanupEntry {
                        seq: next_seq(&log),
                        state: CleanupState::Purged,
                        settles: Some(entry.seq),
                        on: today,
                        ..entry.clone()
                    };
                    store.append(&line)?;
                    log.push(line);
                    report.purged += 1;
                    report.bytes += freed;
                }
                Err(why) => report.failed.push((entry.seq, why)),
            }
        }
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use purifex_domain::app_traces::Trace;
    use purifex_domain::quarantine::QUARANTINE_DAYS;

    use super::*;
    use crate::testing::{CleanLog, FakeCleaning, FakeClock, FakeFinder, PURGE_BYTES};

    fn days_ago(n: i64) -> Option<Date> {
        Date::from_days(FakeClock::DAY.days() - n)
    }

    /// A cleaning with a clock and a quarantine that already holds these removals
    /// (number, days ago; `None` = a line written before the day was kept).
    fn quarantine_with(held: &[(u64, Option<i64>)]) -> (Cleaning, Rc<RefCell<CleanLog>>) {
        let log = Rc::new(RefCell::new(CleanLog::default()));
        log.borrow_mut().entries = held
            .iter()
            .map(|(seq, ago)| CleanupEntry {
                seq: *seq,
                app: "Discord".into(),
                trace: Trace {
                    location: format!(r"C:\x\{seq}"),
                    ..Default::default()
                },
                kept: format!("q/{seq}"),
                on: ago.and_then(days_ago),
                ..Default::default()
            })
            .collect();
        let cleaning = Cleaning::new(
            Box::new(FakeFinder(vec![])),
            Box::new(FakeCleaning(log.clone())),
            Box::new(FakeCleaning(log.clone())),
        )
        .with_clock(Box::new(FakeClock::ticking(0)));
        (cleaning, log)
    }

    #[test]
    fn what_is_removed_is_dated_so_it_can_expire() {
        let (mut c, log) = quarantine_with(&[]);
        c.remove_all("Discord", &[Trace::default()]).unwrap();
        assert_eq!(log.borrow().entries[0].on, Some(FakeClock::DAY));
    }

    #[test]
    fn emptying_says_first_what_it_would_delete_and_deletes_nothing_without_confirmation() {
        let (mut c, log) = quarantine_with(&[(1, Some(1)), (2, Some(2))]);
        let preview = c.empty_quarantine(false).unwrap();
        assert!(preview.preview && preview.purged == 2);
        assert!(log.borrow().purged.is_empty(), "nothing was deleted");
        assert_eq!(log.borrow().entries.len(), 2, "nothing was logged");
    }

    #[test]
    fn emptying_deletes_every_copy_for_good_and_each_deletion_is_a_line_of_the_log() {
        let (mut c, log) = quarantine_with(&[(1, Some(1)), (2, None), (3, Some(30))]);
        let done = c.empty_quarantine(true).unwrap();
        assert_eq!((done.purged, done.bytes), (3, 3 * PURGE_BYTES));
        assert_eq!(log.borrow().purged, [1, 2, 3]);
        let lines = log.borrow().entries.clone();
        let purged: Vec<Option<u64>> = lines
            .iter()
            .filter(|e| e.state == CleanupState::Purged)
            .map(|e| e.settles)
            .collect();
        assert_eq!(purged, [Some(1), Some(2), Some(3)]);
        assert_eq!(
            c.empty_quarantine(true).unwrap().purged,
            0,
            "nothing left to empty"
        );
        assert!(c.quarantine_info().unwrap().items == 0);
    }

    #[test]
    fn a_copy_that_cannot_be_deleted_is_reported_and_the_rest_go() {
        let (mut c, log) = quarantine_with(&[(1, Some(1)), (2, Some(1)), (3, Some(1))]);
        log.borrow_mut().stuck_purge = vec![2];
        let done = c.empty_quarantine(true).unwrap();
        assert_eq!(done.purged, 2);
        assert_eq!(done.failed, [(2, "en uso".to_string())]);
        assert_eq!(
            c.quarantine_info().unwrap().items,
            1,
            "the stuck one is still held"
        );
    }

    #[test]
    fn a_purged_removal_can_no_longer_be_put_back() {
        let (mut c, _) = quarantine_with(&[(1, Some(1))]);
        c.empty_quarantine(true).unwrap();
        let why = c.restore(1).unwrap_err();
        assert!(why.contains("ya se borró de la cuarentena"), "{why}");
    }

    #[test]
    fn only_what_is_seven_days_old_expires_and_a_line_without_a_day_never_does() {
        let (mut c, log) = quarantine_with(&[
            (1, Some(QUARANTINE_DAYS)),
            (2, Some(QUARANTINE_DAYS - 1)),
            (3, Some(60)),
            (4, None),
        ]);
        assert_eq!(c.quarantine_info().unwrap().expired, 2);
        let done = c.expire_quarantine().unwrap();
        assert_eq!(done.purged, 2);
        assert_eq!(log.borrow().purged, [1, 3]);
        assert_eq!(
            c.quarantine_info().unwrap().items,
            2,
            "the fresh one and the undated one stay"
        );
    }

    #[test]
    fn without_a_clock_the_quarantine_says_so_instead_of_guessing_ages() {
        let log = Rc::new(RefCell::new(CleanLog::default()));
        let mut c = Cleaning::new(
            Box::new(FakeFinder(vec![])),
            Box::new(FakeCleaning(log.clone())),
            Box::new(FakeCleaning(log)),
        );
        assert!(c.quarantine_info().unwrap_err().contains("reloj"));
        assert!(c.expire_quarantine().is_err());
        assert!(
            c.empty_quarantine(false).is_ok(),
            "emptying everything needs no ages"
        );
    }
}
