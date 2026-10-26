/// Hands freed memory back to the system; glibc keeps much of it, even after gigabytes are freed.
pub fn release_freed_memory() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    #[allow(unsafe_code)]
    {
        unsafe extern "C" {
            fn malloc_trim(pad: usize) -> i32;
        }
        // SAFETY: malloc_trim only returns unused heap pages; it is safe to call from any thread.
        unsafe {
            malloc_trim(0);
        }
    }
}
