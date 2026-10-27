//! ONNX Runtime, loaded at runtime like pdfium: the app ships the library (build-aux/fetch-onnxruntime.sh).

use ort::environment::Environment;
use ort::session::Session;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};

static ENVIRONMENT: OnceLock<Result<Environment, String>> = OnceLock::new();
/// The share of the processor models may use, in percent.
static PROCESSOR_SHARE: AtomicU32 = AtomicU32::new(80);

/// Models opened from now on use about `percent` of the processor's cores, at least one.
pub fn set_processor_share(percent: u32) {
    PROCESSOR_SHARE.store(percent.clamp(1, 100), Ordering::Relaxed);
}

/// The threads a model may run on.
pub fn threads() -> usize {
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    (cores * PROCESSOR_SHARE.load(Ordering::Relaxed) as usize / 100).max(1)
}

fn library_name() -> String {
    format!(
        "{}onnxruntime{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    )
}

fn candidates() -> Vec<PathBuf> {
    let name = library_name();
    let mut out = Vec::new();
    // An explicit file or folder (development, tests, portable installs).
    if let Some(p) = std::env::var_os("DUCTUS_ONNXRUNTIME") {
        let p = PathBuf::from(p);
        out.push(if p.is_dir() { p.join(&name) } else { p });
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            // Next to the executable (Windows) or ../lib (Flatpak: /app/lib).
            out.push(dir.join(&name));
            out.push(dir.join("../lib").join(&name));
        }
    }
    out
}

fn load() -> Result<Environment, String> {
    let mut errors = Vec::new();
    // By name last: the loader's search path (Android's app libraries, system installs).
    let found = candidates().into_iter().filter(|p| p.exists());
    for path in found.chain([PathBuf::from(library_name())]) {
        match ort::init_from(&path) {
            Ok(builder) => {
                return builder
                    .with_name("ductus")
                    .build()
                    .map_err(|e| e.to_string());
            }
            Err(e) => errors.push(e.to_string()),
        }
    }
    Err(format!(
        "the recognizer runtime (ONNX Runtime) is missing: {}",
        errors.join("; ")
    ))
}

fn environment() -> Result<&'static Environment, String> {
    ENVIRONMENT.get_or_init(load).as_ref().map_err(Clone::clone)
}

pub(crate) fn session(path: &Path) -> Result<Session, String> {
    let failed = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
    let mut builder = Session::builder(environment()?)
        .map_err(|e| failed(&e))?
        .with_intra_threads(threads())
        .map_err(|e| failed(&e))?
        .with_inter_threads(1)
        .map_err(|e| failed(&e))?
        // Idle threads sleep: spinning keeps every core busy between lines.
        .with_intra_op_spinning(false)
        .map_err(|e| failed(&e))?
        .with_inter_op_spinning(false)
        .map_err(|e| failed(&e))?;
    builder.commit_from_file(path).map_err(|e| failed(&e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_processor_share_limits_threads_but_keeps_one() {
        let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
        set_processor_share(100);
        assert_eq!(threads(), cores);
        set_processor_share(25);
        assert_eq!(threads(), (cores / 4).max(1));
        set_processor_share(0);
        assert_eq!(threads(), (cores / 100).max(1));
        set_processor_share(80);
    }
}
