//! Reading notes from other apps on a worker thread; the main thread then writes the pages.

use std::path::Path;
use std::sync::{Arc, Mutex};

type Outcome = Result<ductus_import::Import, String>;

#[derive(Default)]
struct Shared {
    busy: bool,
    outcome: Option<Outcome>,
}

pub struct ImportHandle {
    shared: Arc<Mutex<Shared>>,
}

pub fn open_importer() -> Box<ImportHandle> {
    Box::new(ImportHandle {
        shared: Arc::default(),
    })
}

/// Xournal++, Rnote and OneNote files.
pub fn can_import(file: &str) -> bool {
    ductus_import::supports(Path::new(file))
}

impl ImportHandle {
    pub fn start(&self, file: &str) {
        {
            let Ok(mut s) = self.shared.lock() else {
                return;
            };
            if s.busy {
                return;
            }
            s.busy = true;
            s.outcome = None;
        }
        let file = file.to_owned();
        let shared = Arc::clone(&self.shared);
        std::thread::Builder::new()
            .name("import".into())
            .spawn(move || {
                let outcome = ductus_import::read(Path::new(&file)).map_err(|e| e.to_string());
                if let Ok(mut s) = shared.lock() {
                    s.outcome = Some(outcome);
                    s.busy = false;
                }
            })
            .expect("a thread can be started");
    }

    pub fn busy(&self) -> bool {
        self.shared.lock().is_ok_and(|s| s.busy)
    }

    pub(crate) fn take(&self) -> Option<Outcome> {
        self.shared.lock().ok()?.outcome.take()
    }
}
