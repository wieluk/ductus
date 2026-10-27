pub mod browse;
pub mod library;
pub mod page;

pub use browse::Filter;
pub use library::{Library, Node, NodeKind, library_key, relocate_library};
pub use page::{PageFile, RecognizedLine, read_page, write_page};

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("XML error: {0}")]
    Xml(#[from] quick_xml::Error),
    #[error("invalid attribute: {0}")]
    Attr(#[from] quick_xml::events::attributes::AttrError),
    #[error("invalid escape sequence: {0}")]
    Escape(#[from] quick_xml::escape::EscapeError),
    #[error("metadata error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Invalid(String),
}

pub fn new_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u128(nanos);
    hasher.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
    hasher.write_u32(std::process::id());
    format!(
        "{:016x}{:08x}",
        hasher.finish(),
        (nanos & 0xffff_ffff) as u32
    )
}
