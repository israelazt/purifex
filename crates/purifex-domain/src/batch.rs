//! Uninstalling several apps in a row: the queue, and what became of each one. The
//! uninstallers open windows, so they run one at a time; the person is told at the end
//! what was done and what was not.

/// What became of one app of the batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchResult {
    /// It is no longer in the list: its uninstaller finished.
    Uninstalled,
    /// Its uninstaller could not be started (or it is not allowed), and why.
    Failed(String),
    /// Its uninstaller was waited for and the app is still there.
    StillInstalled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchEntry {
    pub name: String,
    pub result: BatchResult,
}

/// A batch being run, or one that has just finished and is shown as a summary.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Batch {
    /// Ids still to uninstall, in order.
    pub queue: Vec<String>,
    /// The name of the app whose uninstaller is running now.
    pub current: Option<String>,
    pub done: Vec<BatchEntry>,
    /// How many apps the batch had when it started.
    pub total: usize,
    /// What was kept for the whole batch (the restore point), said once.
    pub safeguard: Option<String>,
    /// The person gave up: what was left in the queue was not started.
    pub cancelled: bool,
}

impl Batch {
    pub fn new(queue: Vec<String>, safeguard: Option<String>) -> Self {
        Self {
            total: queue.len(),
            queue,
            safeguard,
            ..Self::default()
        }
    }

    pub fn finished(&self) -> bool {
        (self.queue.is_empty() || self.cancelled) && self.current.is_none()
    }

    pub fn count(&self, wanted: &BatchResult) -> usize {
        self.done
            .iter()
            .filter(|e| std::mem::discriminant(&e.result) == std::mem::discriminant(wanted))
            .count()
    }

    /// The names that were uninstalled: their traces can be looked for.
    pub fn uninstalled(&self) -> impl Iterator<Item = &str> {
        self.done
            .iter()
            .filter(|e| e.result == BatchResult::Uninstalled)
            .map(|e| e.name.as_str())
    }

    /// One line about how it went, for the message.
    pub fn summary(&self) -> String {
        let (ok, failed, left) = (
            self.count(&BatchResult::Uninstalled),
            self.count(&BatchResult::Failed(String::new())),
            self.count(&BatchResult::StillInstalled),
        );
        let apps = |n: usize| if n == 1 { "app" } else { "apps" };
        let mut text = format!("{ok} de {} {} desinstaladas", self.total, apps(self.total));
        if failed > 0 {
            let verb = if failed == 1 {
                "no se pudo empezar"
            } else {
                "no se pudieron empezar"
            };
            text += &format!(", {failed} {verb}");
        }
        if left > 0 {
            let verb = if left == 1 {
                "sigue instalada"
            } else {
                "siguen instaladas"
            };
            text += &format!(", {left} {verb}");
        }
        if self.cancelled {
            text += &format!(", {} sin empezar", self.queue.len());
        }
        text + "."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, result: BatchResult) -> BatchEntry {
        BatchEntry {
            name: name.into(),
            result,
        }
    }

    #[test]
    fn the_summary_counts_each_kind_and_says_what_was_not_started() {
        let mut b = Batch::new(vec!["c".into(), "d".into()], None);
        assert_eq!(b.total, 2);
        assert!(!b.finished());
        b.queue.clear();
        b.done = vec![
            entry("A", BatchResult::Uninstalled),
            entry("B", BatchResult::Failed("no".into())),
            entry("C", BatchResult::StillInstalled),
        ];
        assert!(b.finished());
        assert_eq!(
            b.summary(),
            "1 de 2 apps desinstaladas, 1 no se pudo empezar, 1 sigue instalada."
        );
        assert_eq!(b.uninstalled().collect::<Vec<_>>(), ["A"]);
        b.queue = vec!["x".into()];
        b.cancelled = true;
        assert!(b.summary().ends_with(", 1 sin empezar."));
    }
}
