//! Reading notes from other apps on a worker thread; the main thread then writes the pages.

use crate::ffi::qobject::ImportSummary;
use ductus_import::Entry;
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

    /// What was read, to show before anything is saved.
    pub fn summary(&self) -> ImportSummary {
        let Ok(s) = self.shared.lock() else {
            return ImportSummary::default();
        };
        match &s.outcome {
            None => ImportSummary::default(),
            Some(Err(e)) => ImportSummary {
                error: e.clone(),
                ..ImportSummary::default()
            },
            Some(Ok(import)) => {
                let (kind, name) = match &import.entry {
                    Entry::Page(page) => ("page", page.name.clone()),
                    Entry::Folder {
                        notebook: true,
                        name,
                        ..
                    } => ("notebook", name.clone()),
                    Entry::Folder { name, .. } => ("section", name.clone()),
                    Entry::File { name, .. } => ("file", name.clone()),
                };
                let (mut pages, mut sections) = (0, 0);
                count(&import.entry, &mut pages, &mut sections);
                ImportSummary {
                    ready: true,
                    kind: kind.into(),
                    name,
                    pages,
                    sections: sections.saturating_sub(u32::from(kind != "page" && kind != "file")),
                    ..ImportSummary::default()
                }
            }
        }
    }

    /// Nothing is saved.
    pub fn discard(&self) {
        drop(self.take());
    }
}

fn count(entry: &Entry, pages: &mut u32, folders: &mut u32) {
    match entry {
        Entry::Page(_) => *pages += 1,
        Entry::Folder { entries, .. } => {
            *folders += 1;
            for e in entries {
                count(e, pages, folders);
            }
        }
        Entry::File { .. } => {}
    }
}
