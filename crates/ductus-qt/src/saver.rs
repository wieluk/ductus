//! Pages are written on their own thread: writing a big page stalled the interface.
//! Writes run in the order they were queued; whatever reads or moves files waits for them.

use std::collections::HashMap;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;

type Write = Box<dyn FnOnce() -> Result<(), ductus_format::Error> + Send>;

#[derive(Default)]
struct State {
    /// Writes queued and not done yet, by page.
    pending: HashMap<String, usize>,
    /// Pages written since last asked, with the error ("" when saved).
    done: Vec<(String, String)>,
}

pub struct Saver {
    jobs: Option<Sender<(String, Write)>>,
    state: Arc<(Mutex<State>, Condvar)>,
    thread: Option<JoinHandle<()>>,
}

impl Saver {
    pub fn new() -> Self {
        let (jobs, inbox) = channel::<(String, Write)>();
        let state = Arc::new((Mutex::new(State::default()), Condvar::new()));
        let shared = Arc::clone(&state);
        let thread = std::thread::Builder::new()
            .name("saving".into())
            .spawn(move || {
                while let Ok((page, write)) = inbox.recv() {
                    // A write that panics must not leave anyone waiting for it.
                    let error = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(write))
                    {
                        Ok(Ok(())) => String::new(),
                        Ok(Err(e)) => e.to_string(),
                        Err(_) => "Saving failed unexpectedly.".into(),
                    };
                    finish(&shared, page, error);
                }
            })
            .expect("a thread can be started");
        Self {
            jobs: Some(jobs),
            state,
            thread: Some(thread),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn queue(
        &self,
        page: &str,
        write: impl FnOnce() -> Result<(), ductus_format::Error> + Send + 'static,
    ) {
        *self.lock().pending.entry(page.to_owned()).or_default() += 1;
        let sent = self
            .jobs
            .as_ref()
            .is_some_and(|jobs| jobs.send((page.to_owned(), Box::new(write))).is_ok());
        if !sent {
            finish(&self.state, page.to_owned(), "Saving has stopped.".into());
        }
    }

    /// Until every queued write is done.
    pub fn wait(&self) {
        let mut state = self.lock();
        while !state.pending.is_empty() {
            state = self.state.1.wait(state).unwrap_or_else(|e| e.into_inner());
        }
    }

    /// Until `page`'s queued writes are done; others may still run.
    pub fn wait_for(&self, page: &str) {
        let mut state = self.lock();
        while state.pending.contains_key(page) {
            state = self.state.1.wait(state).unwrap_or_else(|e| e.into_inner());
        }
    }

    pub fn busy(&self) -> bool {
        !self.lock().pending.is_empty()
    }

    pub fn take_done(&self) -> Vec<(String, String)> {
        std::mem::take(&mut self.lock().done)
    }
}

fn finish(state: &(Mutex<State>, Condvar), page: String, error: String) {
    let mut s = state.0.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(n) = s.pending.get_mut(&page) {
        *n -= 1;
        if *n == 0 {
            s.pending.remove(&page);
        }
    }
    s.done.push((page, error));
    state.1.notify_all();
}

impl Drop for Saver {
    /// Quitting finishes the writes first.
    fn drop(&mut self) {
        drop(self.jobs.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
