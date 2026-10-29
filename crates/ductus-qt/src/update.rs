use crate::ffi::qobject::UpdateState;
use ductus_update::Release;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Shared {
    busy: bool,
    checked: bool,
    latest: Option<Release>,
    /// The notes of every release since the installed one, when there are several.
    notes: String,
    error: String,
    received: u64,
    total: u64,
    file: String,
}

pub struct UpdateHandle {
    shared: Arc<Mutex<Shared>>,
    cancel: Arc<AtomicBool>,
}

pub fn open_updater() -> Box<UpdateHandle> {
    Box::new(UpdateHandle {
        shared: Arc::default(),
        cancel: Arc::default(),
    })
}

/// Tests use their own key.
fn signing_key() -> String {
    std::env::var("DUCTUS_UPDATE_KEY").unwrap_or_else(|_| ductus_update::PUBLIC_KEY.to_owned())
}

pub fn update_is_newer(latest: &str, current: &str) -> bool {
    ductus_update::is_newer(latest, current)
}

impl UpdateHandle {
    fn run(&self, job: impl FnOnce(&Arc<Mutex<Shared>>, &AtomicBool) + Send + 'static) {
        {
            let Ok(mut s) = self.shared.lock() else {
                return;
            };
            if s.busy {
                return;
            }
            s.busy = true;
            s.error.clear();
        }
        self.cancel.store(false, Ordering::Relaxed);
        let shared = Arc::clone(&self.shared);
        let cancel = Arc::clone(&self.cancel);
        std::thread::Builder::new()
            .name("update".into())
            .spawn(move || {
                job(&shared, &cancel);
                if let Ok(mut s) = shared.lock() {
                    s.busy = false;
                }
            })
            .expect("a thread can be started");
    }

    /// Test feeds are single releases; GitHub's list then adds the notes of skipped versions.
    pub fn check(&self, feed: &str, current: &str) {
        let (feed, list) = match feed {
            "" => (ductus_update::LATEST_RELEASE_URL.to_owned(), true),
            feed => (feed.to_owned(), false),
        };
        let current = current.to_owned();
        self.run(move |shared, _| {
            let outcome = ductus_update::latest_release(&feed);
            let notes = match (&outcome, list) {
                (Ok(Some(_)), true) => ductus_update::releases(ductus_update::RELEASES_URL)
                    .map(|all| ductus_update::notes_since(&all, &current))
                    .unwrap_or_default(),
                _ => String::new(),
            };
            let Ok(mut s) = shared.lock() else { return };
            s.checked = true;
            s.notes = notes;
            match outcome {
                Ok(latest) => s.latest = latest,
                Err(e) => s.error = e,
            }
        });
    }

    pub fn download(&self, suffix: &str, dir: &str) {
        let Some(release) = self.shared.lock().ok().and_then(|s| s.latest.clone()) else {
            return;
        };
        let (suffix, dir) = (suffix.to_owned(), PathBuf::from(dir));
        self.run(move |shared, cancel| {
            let outcome = (|| {
                let asset = release.asset_ending(&suffix).ok_or_else(|| {
                    format!("version {} has no {suffix} package", release.version)
                })?;
                let digest = ductus_update::signed_digest(&release, &asset.name, &signing_key())?;
                std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
                let dest = dir.join(&asset.name);
                ductus_update::download(asset, &digest, &dest, cancel, |done, total| {
                    if let Ok(mut s) = shared.lock() {
                        (s.received, s.total) = (done, total);
                    }
                })?;
                Ok::<_, String>(dest.to_string_lossy().into_owned())
            })();
            let Ok(mut s) = shared.lock() else { return };
            match outcome {
                Ok(file) => s.file = file,
                Err(e) => s.error = e,
            }
        });
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn asset_url(&self, suffix: &str) -> String {
        self.shared
            .lock()
            .ok()
            .and_then(|s| Some(s.latest.as_ref()?.asset_ending(suffix)?.url.clone()))
            .unwrap_or_default()
    }

    pub fn state(&self) -> UpdateState {
        let Ok(s) = self.shared.lock() else {
            return UpdateState::default();
        };
        UpdateState {
            busy: s.busy,
            checked: s.checked,
            latest: s
                .latest
                .as_ref()
                .map(|r| r.version.clone())
                .unwrap_or_default(),
            notes: match s.notes.is_empty() {
                false => s.notes.clone(),
                true => s
                    .latest
                    .as_ref()
                    .map(|r| r.notes.clone())
                    .unwrap_or_default(),
            },
            page: s
                .latest
                .as_ref()
                .map(|r| r.page.clone())
                .unwrap_or_else(|| ductus_update::RELEASES_PAGE.to_owned()),
            error: s.error.clone(),
            received: s.received,
            total: s.total,
            file: s.file.clone(),
        }
    }
}
