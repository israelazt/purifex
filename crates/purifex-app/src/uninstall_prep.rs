//! Preparing an uninstall off the thread of the window: Windows can take a minute to make a restore
//! point. One at a time; the result is picked up by polling, without waiting.

use std::sync::mpsc::{channel, Receiver, TryRecvError};

use purifex_domain::prep_step::PrepStep;

use crate::cleaning::{Prepared, UninstallJob};

enum Msg {
    Step(PrepStep),
    Done(Box<Prepared>),
}

struct Running {
    receive: Receiver<Msg>,
    steps: Vec<PrepStep>,
}

/// What a poll found.
pub enum Polled {
    Idle,
    /// Still preparing, and these parts have finished.
    Working(Vec<PrepStep>),
    Done(Result<Box<Prepared>, String>),
}

#[derive(Default)]
pub struct UninstallPrep {
    running: Option<Running>,
}

impl UninstallPrep {
    /// Starts preparing. Another one already going is refused: two uninstalls must not overlap.
    pub fn start(&mut self, job: UninstallJob) -> Result<(), String> {
        if self.running.is_some() {
            return Err("ya se está preparando otra desinstalación".into());
        }
        let (send, receive) = channel();
        std::thread::Builder::new()
            .name("uninstall-prep".into())
            .spawn(move || {
                // The window may be closed while it works: nobody listening is fine.
                let said = send.clone();
                let prepared = job.run(&move |step| {
                    let _ = said.send(Msg::Step(step));
                });
                let _ = send.send(Msg::Done(Box::new(prepared)));
            })
            .map_err(|e| format!("no se pudo preparar la desinstalación: {e}"))?;
        self.running = Some(Running {
            receive,
            steps: Vec::new(),
        });
        Ok(())
    }

    /// What has happened since the last poll. It never waits.
    pub fn poll(&mut self) -> Polled {
        let Some(running) = self.running.as_mut() else {
            return Polled::Idle;
        };
        let result = loop {
            match running.receive.try_recv() {
                Ok(Msg::Step(step)) => running.steps.push(step),
                Ok(Msg::Done(prepared)) => break Some(Ok(prepared)),
                Err(TryRecvError::Empty) => break None,
                Err(TryRecvError::Disconnected) => {
                    break Some(Err("la preparación terminó sin resultado".into()))
                }
            }
        };
        match result {
            None => Polled::Working(running.steps.clone()),
            Some(done) => {
                self.running = None;
                Polled::Done(done)
            }
        }
    }
}
