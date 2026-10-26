//! SQLite's WAL lets the worker write while searches read.

use crate::ffi::qobject::SearchHit;
use ductus_format::Library;
use ductus_search::{Index, Progress};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};

enum Job {
    Scan,
    File(String),
}

#[derive(Default)]
struct Status {
    busy: bool,
    progress: Progress,
    error: String,
}

pub struct SearchHandle {
    reader: Index,
    jobs: Sender<Job>,
    status: Arc<Mutex<Status>>,
    cancel: Arc<AtomicBool>,
}

type Result<T> = std::result::Result<T, ductus_search::Error>;

pub fn open_search(cache_dir: &str, root: &str, data_dir: &str) -> Result<Box<SearchHandle>> {
    let db = ductus_search::index_path(Path::new(cache_dir), Path::new(root));
    // Opening here first creates the tables before the worker starts.
    let reader = Index::open(&db)?;
    let (jobs, inbox) = channel::<Job>();
    let status = Arc::new(Mutex::new(Status::default()));
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_status = Arc::clone(&status);
    let worker_cancel = Arc::clone(&cancel);
    let root = PathBuf::from(root);
    let data_dir = PathBuf::from(data_dir);
    std::thread::Builder::new()
        .name("search-index".into())
        .spawn(move || {
            let set = |f: &dyn Fn(&mut Status)| {
                if let Ok(mut s) = worker_status.lock() {
                    f(&mut s);
                }
            };
            let (mut index, library) = match (Index::open(&db), Library::open(&root, &data_dir)) {
                (Ok(index), Ok(library)) => (index, library),
                (Err(e), _) => return set(&|s| s.error = e.to_string()),
                (_, Err(e)) => return set(&|s| s.error = e.to_string()),
            };
            while let Ok(job) = inbox.recv() {
                if worker_cancel.load(Ordering::Relaxed) {
                    break;
                }
                set(&|s| s.busy = true);
                let result = match job {
                    Job::Scan => index.update(&library, &worker_cancel, |p| {
                        set(&|s| s.progress = p);
                    }),
                    Job::File(path) => index.index_file(&library, &path, true),
                };
                set(&|s| {
                    s.busy = false;
                    s.error = result
                        .as_ref()
                        .err()
                        .map(ToString::to_string)
                        .unwrap_or_default();
                });
            }
        })
        .map_err(ductus_search::Error::Io)?;
    Ok(Box::new(SearchHandle {
        reader,
        jobs,
        status,
        cancel,
    }))
}

impl Drop for SearchHandle {
    fn drop(&mut self) {
        // The worker stops after its current file; the channel closing ends it.
        self.cancel.store(true, Ordering::Relaxed);
    }
}

impl SearchHandle {
    pub fn refresh(&self) {
        let _ = self.jobs.send(Job::Scan);
    }

    pub fn reindex(&self, path: &str) {
        let _ = self.jobs.send(Job::File(path.to_owned()));
    }

    pub fn indexing(&self) -> bool {
        self.status.lock().is_ok_and(|s| s.busy)
    }

    pub fn index_progress(&self) -> Vec<u64> {
        self.status
            .lock()
            .map(|s| vec![s.progress.done as u64, s.progress.total as u64])
            .unwrap_or_default()
    }

    pub fn index_error(&self) -> String {
        self.status
            .lock()
            .map(|s| s.error.clone())
            .unwrap_or_default()
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchHit> {
        let hits = self.reader.search(query, limit).unwrap_or_default();
        hits.into_iter()
            .map(|h| {
                let name = h.path.rsplit('/').next().unwrap_or(&h.path);
                let title = ductus_format::library::pdf_stem(name).unwrap_or(name);
                let (x, y, w, height) = h.rect.unwrap_or((0.0, 0.0, -1.0, -1.0));
                SearchHit {
                    title: title.to_owned(),
                    location: h
                        .path
                        .rsplit_once('/')
                        .map_or(String::new(), |(f, _)| f.replace('/', " › ")),
                    path: h.path,
                    kind: h.kind.name().to_owned(),
                    page: h.page.map_or(-1, |p| p as i64),
                    x,
                    y,
                    w,
                    h: height,
                    snippet: h.snippet,
                }
            })
            .collect()
    }
}
