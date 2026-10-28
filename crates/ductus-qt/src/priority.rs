//! Background work yields the processor to the interface.

/// Lowers the calling thread's priority; on Linux and Android threads it starts later inherit it.
pub fn lower_this_thread() {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[allow(unsafe_code)]
    // SAFETY: setpriority only changes the scheduling of the calling thread.
    unsafe {
        libc::setpriority(libc::PRIO_PROCESS, libc::gettid() as libc::id_t, 10);
    }
}
