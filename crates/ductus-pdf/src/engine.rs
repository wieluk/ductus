use crate::Error;
use pdfium_render::prelude::Pdfium;
use std::path::PathBuf;
use std::sync::OnceLock;

static ENGINE: OnceLock<Result<Pdfium, String>> = OnceLock::new();

fn candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    // An explicit file or folder (development, tests, portable installs).
    if let Some(p) = std::env::var_os("DUCTUS_PDFIUM") {
        let p = PathBuf::from(p);
        out.push(if p.is_dir() {
            Pdfium::pdfium_platform_library_name_at_path(&p)
        } else {
            p
        });
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            // Next to the executable (Windows) or ../lib (Flatpak: /app/lib).
            out.push(Pdfium::pdfium_platform_library_name_at_path(dir));
            out.push(Pdfium::pdfium_platform_library_name_at_path(
                &dir.join("../lib"),
            ));
        }
    }
    out
}

fn load() -> Result<Pdfium, String> {
    let mut errors = Vec::new();
    for path in candidates() {
        if !path.exists() {
            continue;
        }
        match Pdfium::bind_to_library(&path) {
            Ok(bindings) => return Ok(Pdfium::new(bindings)),
            Err(e) => errors.push(format!("{}: {e}", path.display())),
        }
    }
    match Pdfium::bind_to_system_library() {
        Ok(bindings) => Ok(Pdfium::new(bindings)),
        Err(e) => {
            errors.push(format!("system library: {e}"));
            Err(format!("pdfium not found ({})", errors.join("; ")))
        }
    }
}

pub fn engine() -> Result<&'static Pdfium, Error> {
    ENGINE
        .get_or_init(load)
        .as_ref()
        .map_err(|e| Error::Unavailable(e.clone()))
}

pub fn pdfium_status() -> Option<String> {
    engine().err().map(|e| e.to_string())
}
