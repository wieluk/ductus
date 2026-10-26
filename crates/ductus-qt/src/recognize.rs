use crate::ffi::qobject::RecognitionResult;
use std::collections::HashSet;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};

struct Job {
    page: String,
    key: String,
    rect: (f64, f64, f64, f64),
    png: Vec<u8>,
    lang: String,
}

type Config = (String, String, String, String);

#[derive(Default)]
struct Shared {
    config: Config,
    queued: HashSet<(String, String)>,
    results: Vec<RecognitionResult>,
    error: String,
}

pub struct RecognitionHandle {
    jobs: Sender<Job>,
    shared: Arc<Mutex<Shared>>,
}

pub fn open_recognizer() -> Box<RecognitionHandle> {
    let (jobs, inbox) = channel::<Job>();
    let shared = Arc::new(Mutex::new(Shared::default()));
    let worker = Arc::clone(&shared);
    std::thread::Builder::new()
        .name("handwriting".into())
        .spawn(move || {
            // Kept while the settings stay: a built-in model takes seconds to load.
            let mut current: Option<(Config, Option<Box<dyn ductus_hwr::Recognizer>>)> = None;
            while let Ok(job) = inbox.recv() {
                let config = worker.lock().map(|s| s.config.clone()).unwrap_or_default();
                if current.as_ref().is_none_or(|(c, _)| *c != config) {
                    let r = ductus_hwr::from_settings(&config.0, &config.1, &config.2, &config.3);
                    current = Some((config, r));
                }
                let outcome = match current.as_ref().and_then(|(_, r)| r.as_ref()) {
                    Some(r) => r.recognize(&job.png, &job.lang),
                    None => Err(String::new()), // turned off meanwhile
                };
                let Ok(mut s) = worker.lock() else { break };
                s.queued.remove(&(job.page.clone(), job.key.clone()));
                match outcome {
                    Ok(text) => {
                        s.error.clear();
                        s.results.push(RecognitionResult {
                            page: job.page,
                            key: job.key,
                            text,
                            x: job.rect.0,
                            y: job.rect.1,
                            w: job.rect.2,
                            h: job.rect.3,
                            lang: job.lang,
                        });
                    }
                    Err(e) => s.error = e,
                }
            }
        })
        .expect("a thread can be started");
    Box::new(RecognitionHandle { jobs, shared })
}

impl RecognitionHandle {
    pub fn configure(&self, kind: &str, url: &str, model: &str, key: &str) {
        if let Ok(mut s) = self.shared.lock() {
            s.config = (kind.into(), url.into(), model.into(), key.into());
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn submit(
        &self,
        page: &str,
        key: &str,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        png: &[u8],
        lang: &str,
    ) {
        let Ok(mut s) = self.shared.lock() else {
            return;
        };
        if !s.queued.insert((page.into(), key.into())) {
            return;
        }
        drop(s);
        let _ = self.jobs.send(Job {
            page: page.into(),
            key: key.into(),
            rect: (x, y, w, h),
            png: png.to_vec(),
            lang: lang.into(),
        });
    }

    pub fn pending(&self) -> usize {
        self.shared.lock().map_or(0, |s| s.queued.len())
    }

    pub fn take_results(&self) -> Vec<RecognitionResult> {
        self.shared
            .lock()
            .map(|mut s| std::mem::take(&mut s.results))
            .unwrap_or_default()
    }

    pub fn last_error(&self) -> String {
        self.shared
            .lock()
            .map(|s| s.error.clone())
            .unwrap_or_default()
    }
}
