//! Reading selected ink on a worker thread, to replace it with text or a formula.

use crate::ffi::qobject::{ConvertState, LineJob};
use ductus_hwr::Recognizer;
use std::sync::{Arc, Mutex};

type Config = (String, String, String, String);

#[derive(Default)]
struct Shared {
    busy: bool,
    done: usize,
    total: usize,
    result: String,
    strokes: String,
    error: String,
    // The loaded model stays for the next conversion.
    recognizer: Option<(Config, Box<dyn Recognizer>)>,
}

pub struct ConvertHandle {
    shared: Arc<Mutex<Shared>>,
}

pub fn open_converter() -> Box<ConvertHandle> {
    Box::new(ConvertHandle {
        shared: Arc::default(),
    })
}

impl ConvertHandle {
    /// Reads `jobs` with the recognizer from `ductus_hwr::from_settings`; results joined by line breaks.
    /// Lines read as nothing (a drawing, say) are left out, and so are their strokes.
    pub fn start(
        &self,
        kind: &str,
        url: &str,
        model: &str,
        key: &str,
        lang: &str,
        jobs: Vec<LineJob>,
    ) {
        {
            let Ok(mut s) = self.shared.lock() else {
                return;
            };
            if s.busy {
                return;
            }
            (s.busy, s.done, s.total) = (true, 0, jobs.len());
            s.result.clear();
            s.strokes.clear();
            s.error.clear();
        }
        let config = (
            kind.to_owned(),
            url.to_owned(),
            model.to_owned(),
            key.to_owned(),
        );
        let lang = lang.to_owned();
        let shared = Arc::clone(&self.shared);
        std::thread::Builder::new()
            .name("convert".into())
            .spawn(move || {
                let mut recognizer = shared.lock().ok().and_then(|mut s| s.recognizer.take());
                if recognizer.as_ref().is_none_or(|(c, _)| *c != config) {
                    recognizer =
                        ductus_hwr::from_settings(&config.0, &config.1, &config.2, &config.3)
                            .map(|r| (config.clone(), r));
                }
                let mut lines = Vec::new();
                let mut strokes = Vec::new();
                let mut error = String::new();
                match &recognizer {
                    Some((_, r)) => {
                        for job in &jobs {
                            match crate::guarded(|| r.recognize(&job.png, &lang)) {
                                Ok(text) if text.trim().is_empty() => {}
                                Ok(text) => {
                                    lines.push(text);
                                    strokes.push(job.key.as_str());
                                }
                                Err(e) => {
                                    error = e;
                                    break;
                                }
                            }
                            if let Ok(mut s) = shared.lock() {
                                s.done += 1;
                            }
                        }
                    }
                    None => error = "no recognizer".into(),
                }
                if let Ok(mut s) = shared.lock() {
                    s.result = lines.join("\n");
                    s.strokes = strokes.join(" ");
                    s.error = error;
                    s.recognizer = recognizer;
                    s.busy = false;
                }
            })
            .expect("a thread can be started");
    }

    pub fn state(&self) -> ConvertState {
        let Ok(s) = self.shared.lock() else {
            return ConvertState::default();
        };
        ConvertState {
            busy: s.busy,
            done: s.done,
            total: s.total,
            result: s.result.clone(),
            strokes: s.strokes.clone(),
            error: s.error.clone(),
        }
    }
}
