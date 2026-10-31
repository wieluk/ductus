//! On Android, Qt loads this crate as a shared library and calls its main.

// Linked for its Qt type registrations, which cxx-qt initializes at startup.
extern crate cxx_qt_lib;

// cxx bridges are inherently unsafe FFI; this is the only place unsafe is allowed.
mod convert;
mod engine;
#[allow(unsafe_code)]
mod ffi;
mod import;
mod library;
mod models;
mod priority;
mod recognize;
mod saver;
mod search;
mod update;

/// Work on a worker thread: a panic (a parser choking on a broken file, say) becomes an error
/// message, so the worker still finishes and the UI never waits for it forever.
pub(crate) fn guarded<T>(work: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(work)).unwrap_or_else(|panic| {
        let why = panic
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| panic.downcast_ref::<String>().cloned())
            .unwrap_or_default();
        Err(format!("an internal error stopped it ({why})"))
    })
}

pub fn run() -> i32 {
    // "Open with Ductus" passes the files as arguments.
    let files = std::env::args_os()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .filter(|arg| !arg.starts_with('-'))
        .collect();
    ffi::qobject::run_app("", files)
}

/// Qt's Android loader calls this with the path of this library as argv[0].
#[cfg(target_os = "android")]
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn main(argc: i32, argv: *const *const std::ffi::c_char) -> i32 {
    let program = if argc > 0 && !argv.is_null() {
        // SAFETY: the loader passes argc valid C strings that outlive this call.
        unsafe { std::ffi::CStr::from_ptr(*argv) }
            .to_string_lossy()
            .into_owned()
    } else {
        String::new()
    };
    // Files to open come in Android intents, which app.cpp reads.
    ffi::qobject::run_app(&program, Vec::new())
}
