//! Downloading and removing the built-in recognition models, on a worker thread.

use crate::ffi::qobject::{LanguageRow, ModelRow, ModelsState};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Shared {
    busy: String,
    received: u64,
    total: u64,
    error: String,
}

pub struct ModelsHandle {
    shared: Arc<Mutex<Shared>>,
    cancel: Arc<AtomicBool>,
}

pub fn open_models() -> Box<ModelsHandle> {
    Box::new(ModelsHandle {
        shared: Arc::default(),
        cancel: Arc::default(),
    })
}

pub fn model_rows(dir: &str) -> Vec<ModelRow> {
    ductus_hwr::builtin::MODELS
        .iter()
        .map(|m| ModelRow {
            id: m.id.into(),
            title: m.title.into(),
            size: m.size(),
            installed: ductus_hwr::builtin::installed(dir.as_ref(), m.id),
        })
        .collect()
}

pub fn languages() -> Vec<LanguageRow> {
    ductus_hwr::builtin::LANGUAGES
        .iter()
        .map(|l| LanguageRow {
            code: l.code.into(),
            name: match l.native {
                "" => l.english.into(),
                native => format!("{} ({native})", l.english),
            },
            model: l.model.into(),
        })
        .collect()
}

pub fn text_model(lang: &str) -> String {
    ductus_hwr::builtin::text_model(lang).into()
}

pub fn remove_model(dir: &str, id: &str) -> String {
    ductus_hwr::builtin::remove(dir.as_ref(), id)
        .err()
        .unwrap_or_default()
}

impl ModelsHandle {
    pub fn install(&self, dir: &str, id: &str) {
        {
            let Ok(mut s) = self.shared.lock() else {
                return;
            };
            if !s.busy.is_empty() {
                return;
            }
            *s = Shared {
                busy: id.into(),
                ..Default::default()
            };
        }
        self.cancel.store(false, Ordering::Relaxed);
        let (dir, id) = (PathBuf::from(dir), id.to_owned());
        let (shared, cancel) = (Arc::clone(&self.shared), Arc::clone(&self.cancel));
        std::thread::Builder::new()
            .name("models".into())
            .spawn(move || {
                let outcome = ductus_hwr::builtin::install(&dir, &id, &cancel, |done, total| {
                    if let Ok(mut s) = shared.lock() {
                        (s.received, s.total) = (done, total);
                    }
                });
                if let Ok(mut s) = shared.lock() {
                    // A cancel is no error to report.
                    s.error = match cancel.load(Ordering::Relaxed) {
                        true => String::new(),
                        false => outcome.err().unwrap_or_default(),
                    };
                    s.busy.clear();
                }
            })
            .expect("a thread can be started");
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn state(&self) -> ModelsState {
        let Ok(s) = self.shared.lock() else {
            return ModelsState::default();
        };
        ModelsState {
            busy: s.busy.clone(),
            received: s.received,
            total: s.total,
            error: s.error.clone(),
        }
    }
}
