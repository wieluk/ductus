/// FNV-1a: stable across runs and platforms, unlike std's hasher.
pub fn fnv1a(bytes: impl IntoIterator<Item = u8>) -> u64 {
    bytes.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// How pictures in a page are named.
pub const IMAGE_PREFIX: &str = "img-";

/// Named by content, so the same picture is stored once.
pub fn picture_name(png: &[u8]) -> String {
    format!("{IMAGE_PREFIX}{:016x}.png", fnv1a(png.iter().copied()))
}
