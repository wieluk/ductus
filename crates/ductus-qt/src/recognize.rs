use crate::ffi::qobject::RecognitionResult;
use ductus_core::Stroke;
use ductus_hwr::InkLine;
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long after the last scroll, stroke or key press recognition waits before its next line.
const QUIET: Duration = Duration::from_millis(600);

/// A line's own strokes: its image is drawn on the worker, not on the interface thread.
struct Job {
    page: String,
    line: InkLine,
    ink: Vec<Stroke>,
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
    started: Instant,
    /// Milliseconds after `started` of the user's last input.
    last_input: Arc<AtomicU64>,
}

pub fn open_recognizer() -> Box<RecognitionHandle> {
    let (jobs, inbox) = channel::<Job>();
    let shared = Arc::new(Mutex::new(Shared::default()));
    let worker = Arc::clone(&shared);
    let started = Instant::now();
    let last_input = Arc::new(AtomicU64::new(0));
    let input = Arc::clone(&last_input);
    std::thread::Builder::new()
        .name("handwriting".into())
        .spawn(move || {
            crate::priority::lower_this_thread();
            // Kept while the settings stay: a built-in model takes seconds to load.
            type Loaded = ((Config, usize), Option<Box<dyn ductus_hwr::Recognizer>>);
            let mut current: Option<Loaded> = None;
            while let Ok(job) = inbox.recv() {
                // Lines wait while the user scrolls or writes, so the page keeps up.
                loop {
                    let idle = started
                        .elapsed()
                        .saturating_sub(Duration::from_millis(input.load(Ordering::Relaxed)));
                    if idle >= QUIET {
                        break;
                    }
                    std::thread::sleep(QUIET - idle);
                }
                let config = worker.lock().map(|s| s.config.clone()).unwrap_or_default();
                let wanted = (config, ductus_hwr::threads());
                if current.as_ref().is_none_or(|(c, _)| *c != wanted) {
                    let c = &wanted.0;
                    let r = ductus_hwr::from_settings(&c.0, &c.1, &c.2, &c.3);
                    current = Some((wanted, r));
                }
                let png = ductus_hwr::line_png(&job.ink, &job.line, 96);
                let outcome = match (current.as_ref().and_then(|(_, r)| r.as_ref()), png) {
                    // One unreadable line must not end reading for the rest of the session.
                    (Some(r), Some(png)) => crate::guarded(|| r.recognize(&png, &job.lang)),
                    _ => Err(String::new()), // turned off meanwhile, or no ink to see
                };
                let Ok(mut s) = worker.lock() else { break };
                s.queued.remove(&(job.page.clone(), job.line.key.clone()));
                match outcome {
                    Ok(text) => {
                        s.error.clear();
                        let rect = job.line.rect;
                        s.results.push(RecognitionResult {
                            page: job.page,
                            key: job.line.key,
                            text,
                            x: rect.x0,
                            y: rect.y0,
                            w: rect.width(),
                            h: rect.height(),
                            lang: job.lang,
                        });
                    }
                    Err(e) => s.error = e,
                }
            }
        })
        .expect("a thread can be started");
    Box::new(RecognitionHandle {
        jobs,
        shared,
        started,
        last_input,
    })
}

impl RecognitionHandle {
    pub fn configure(&self, kind: &str, url: &str, model: &str, key: &str) {
        if let Ok(mut s) = self.shared.lock() {
            s.config = (kind.into(), url.into(), model.into(), key.into());
        }
    }

    /// Queues a line of `strokes` unless it is queued already; true when it was added.
    pub(crate) fn submit(
        &self,
        page: &str,
        line: &InkLine,
        strokes: &[Stroke],
        lang: &str,
    ) -> bool {
        let Ok(mut s) = self.shared.lock() else {
            return false;
        };
        if !s.queued.insert((page.into(), line.key.clone())) {
            return false;
        }
        drop(s);
        let ink: Vec<Stroke> = line.strokes.iter().map(|&i| strokes[i].clone()).collect();
        let line = InkLine {
            strokes: (0..ink.len()).collect(),
            rect: line.rect,
            key: line.key.clone(),
        };
        self.jobs
            .send(Job {
                page: page.into(),
                line,
                ink,
                lang: lang.into(),
            })
            .is_ok()
    }

    /// The user scrolled, wrote or typed just now.
    pub fn user_active(&self) {
        let now = u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        self.last_input.store(now, Ordering::Relaxed);
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
