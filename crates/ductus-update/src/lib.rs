//! Nothing reaches the disk unless the release's signed checksums vouch for it.

use base64::Engine;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/wieluk/ductus/releases/latest";

pub const RELEASES_PAGE: &str = "https://github.com/wieluk/ductus/releases/latest";

/// Signs every release's SHA256SUMS.txt (base64 of the raw Ed25519 key).
pub const PUBLIC_KEY: &str = "8jcsW2MBOKkCEHlultqyZnPDGeLHQ+MKQUsDdzQUNsg=";

pub const CHECKSUMS: &str = "SHA256SUMS.txt";
pub const SIGNATURE: &str = "SHA256SUMS.txt.sig";

#[derive(Debug, Clone, PartialEq)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    pub version: String,
    pub notes: String,
    pub page: String,
    pub assets: Vec<Asset>,
}

impl Release {
    pub fn asset(&self, name: &str) -> Option<&Asset> {
        self.assets.iter().find(|a| a.name == name)
    }

    pub fn asset_ending(&self, suffix: &str) -> Option<&Asset> {
        self.assets.iter().find(|a| a.name.ends_with(suffix))
    }
}

/// Non-versions count as not newer, so a stray tag never nags.
pub fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |v: &str| semver::Version::parse(v.trim().trim_start_matches(['v', 'V']));
    matches!((parse(latest), parse(current)), (Ok(l), Ok(c)) if l > c)
}

pub fn parse_release(json: &str) -> Result<Option<Release>, String> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("the release feed isn't JSON: {e}"))?;
    if value["draft"].as_bool() == Some(true) || value["prerelease"].as_bool() == Some(true) {
        return Ok(None);
    }
    let tag = value["tag_name"]
        .as_str()
        .ok_or("the release feed has no tag_name")?;
    let assets = value["assets"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|a| {
                    Some(Asset {
                        name: a["name"].as_str()?.to_owned(),
                        url: a["browser_download_url"].as_str()?.to_owned(),
                        size: a["size"].as_u64().unwrap_or(0),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Some(Release {
        version: tag.trim_start_matches(['v', 'V']).to_owned(),
        notes: value["body"].as_str().unwrap_or("").trim().to_owned(),
        page: value["html_url"]
            .as_str()
            .unwrap_or(RELEASES_PAGE)
            .to_owned(),
        assets,
    }))
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_body(Some(Duration::from_secs(600)))
        .user_agent(concat!("Ductus/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

/// Plain paths and file:// URLs are read from disk, for tests.
fn open(url: &str) -> Result<(Box<dyn Read + Send>, u64), String> {
    if let Some(path) = url
        .strip_prefix("file://")
        .or((!url.contains("://")).then_some(url))
    {
        let file = std::fs::File::open(path).map_err(|e| format!("{path}: {e}"))?;
        let size = file.metadata().map(|m| m.len()).unwrap_or(0);
        return Ok((Box::new(file), size));
    }
    let response = agent()
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| match e {
            ureq::Error::StatusCode(404) => "no release is published yet".to_owned(),
            ureq::Error::StatusCode(code) => format!("GitHub answered HTTP {code}"),
            other => format!("couldn't reach GitHub ({other})"),
        })?;
    let size = response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok()?.parse().ok())
        .unwrap_or(0);
    Ok((Box::new(response.into_body().into_reader()), size))
}

fn read_small(url: &str) -> Result<Vec<u8>, String> {
    let (mut reader, _) = open(url)?;
    let mut out = Vec::new();
    reader
        .by_ref()
        .take(4 << 20)
        .read_to_end(&mut out)
        .map_err(|e| format!("{url}: {e}"))?;
    Ok(out)
}

pub fn latest_release(feed: &str) -> Result<Option<Release>, String> {
    let body = read_small(feed)?;
    parse_release(&String::from_utf8_lossy(&body))
}

pub fn parse_checksums(text: &str) -> HashMap<String, [u8; 32]> {
    text.lines()
        .filter_map(|line| {
            let (hex, name) = line.trim().split_once(char::is_whitespace)?;
            let name = name.trim_start().trim_start_matches('*');
            Some((name.to_owned(), digest_from_hex(hex)?))
        })
        .collect()
}

pub fn digest_from_hex(hex: &str) -> Option<[u8; 32]> {
    let mut digest = [0u8; 32];
    if hex.len() != 64 {
        return None;
    }
    for (i, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(hex.get(2 * i..2 * i + 2)?, 16).ok()?;
    }
    Some(digest)
}

pub fn verify(message: &[u8], signature: &[u8], key: &str) -> bool {
    let Ok(key) = base64::engine::general_purpose::STANDARD.decode(key.trim()) else {
        return false;
    };
    let (Ok(key), Ok(signature)) = (<[u8; 32]>::try_from(key), <[u8; 64]>::try_from(signature))
    else {
        return false;
    };
    let Ok(key) = ed25519_dalek::VerifyingKey::from_bytes(&key) else {
        return false;
    };
    key.verify_strict(message, &ed25519_dalek::Signature::from_bytes(&signature))
        .is_ok()
}

pub fn signed_digest(release: &Release, name: &str, key: &str) -> Result<[u8; 32], String> {
    let (Some(sums), Some(sig)) = (release.asset(CHECKSUMS), release.asset(SIGNATURE)) else {
        return Err("this release has no signed checksums".to_owned());
    };
    let sums = read_small(&sums.url)?;
    let sig = read_small(&sig.url)?;
    if !verify(&sums, &sig, key) {
        return Err("the release's checksums aren't signed with Ductus' key".to_owned());
    }
    parse_checksums(&String::from_utf8_lossy(&sums))
        .remove(name)
        .ok_or_else(|| format!("the release's checksums don't list {name}"))
}

pub fn download(
    asset: &Asset,
    digest: &[u8; 32],
    dest: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64, u64),
) -> Result<(), String> {
    let (mut reader, size) = open(&asset.url)?;
    let total = if asset.size > 0 { asset.size } else { size };
    let partial = dest.with_extension("part");
    let mut file =
        std::fs::File::create(&partial).map_err(|e| format!("{}: {e}", partial.display()))?;
    let result = (|| {
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 64 * 1024];
        let mut done = 0u64;
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err("cancelled".to_owned());
            }
            let n = reader
                .read(&mut buffer)
                .map_err(|e| format!("download: {e}"))?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
            file.write_all(&buffer[..n])
                .map_err(|e| format!("{}: {e}", partial.display()))?;
            done += n as u64;
            progress(done, total);
        }
        file.flush().map_err(|e| e.to_string())?;
        if hasher.finalize().as_slice() != digest {
            return Err("the download doesn't match the release's checksum".to_owned());
        }
        Ok(())
    })();
    drop(file);
    match result {
        Ok(()) => std::fs::rename(&partial, dest).map_err(|e| format!("{}: {e}", dest.display())),
        Err(e) => {
            let _ = std::fs::remove_file(&partial);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests;
