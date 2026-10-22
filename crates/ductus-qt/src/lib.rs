//! On Android, Qt loads this crate as a shared library and calls its main.

// Linked for its Qt type registrations, which cxx-qt initializes at startup.
extern crate cxx_qt_lib;

// cxx bridges are inherently unsafe FFI; this is the only place unsafe is allowed.
mod convert;
mod engine;
#[allow(unsafe_code)]
mod ffi;
mod library;
mod models;
mod recognize;
mod search;
mod update;

pub fn run() -> i32 {
    ffi::qobject::run_app()
}

/// Qt's Android loader calls this.
#[cfg(target_os = "android")]
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn main(_argc: i32, _argv: *const *const std::ffi::c_char) -> i32 {
    run()
}
