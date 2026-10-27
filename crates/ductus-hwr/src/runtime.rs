//! ONNX Runtime, loaded at runtime like pdfium: the app ships the library (build-aux/fetch-onnxruntime.sh).

use ort::environment::Environment;
use ort::session::Session;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static ENVIRONMENT: OnceLock<Result<Environment, String>> = OnceLock::new();

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
    Session::builder(environment()?)
        .and_then(|mut b| b.commit_from_file(path))
        .map_err(|e| format!("{}: {e}", path.display()))
}
