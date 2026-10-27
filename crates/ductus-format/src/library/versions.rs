//! Earlier versions of pages and PDFs, kept in the library's data folder, and putting them back.

use super::{Library, NodeKind, PAGE_EXTENSION, PDF_EXTENSION, stable_key};
use crate::Error;
use crate::page::read_page;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq)]
pub struct Version {
    /// Milliseconds since the epoch: when this version was replaced.
    pub saved: u64,
    pub file: PathBuf,
}

impl Library {
    fn history_key(&self, rel: &str) -> Result<(String, NodeKind), Error> {
        let (_, kind) = self.entry_path(rel)?;
        match kind {
            NodeKind::Page => Ok((self.load_page(rel)?.id, kind)),
            NodeKind::Pdf => Ok((format!("pdf-{}", stable_key(rel)), kind)),
            _ => Err(Error::Invalid(
                "Only pages and PDFs keep earlier versions.".into(),
            )),
        }
    }

    /// Earlier versions of a page or PDF, newest first.
    pub fn versions(&self, rel: &str) -> Result<Vec<Version>, Error> {
        let (key, _) = self.history_key(rel)?;
        let mut versions: Vec<Version> = self
            .history(&key)?
            .into_iter()
            .filter_map(|file| {
                let saved = file.file_stem()?.to_str()?.parse().ok()?;
                Some(Version { saved, file })
            })
            .collect();
        versions.reverse();
        Ok(versions)
    }

    fn version(&self, rel: &str, saved: u64) -> Result<(String, NodeKind, PathBuf), Error> {
        let (key, kind) = self.history_key(rel)?;
        let file = self
            .versions(rel)?
            .into_iter()
            .find(|v| v.saved == saved)
            .ok_or_else(|| Error::Invalid("That version no longer exists.".into()))?
            .file;
        Ok((key, kind, file))
    }

    /// The current state becomes a version first, so a restore can be undone the same way.
    pub fn restore_version(&mut self, rel: &str, saved: u64) -> Result<(), Error> {
        let (key, kind, file) = self.version(rel, saved)?;
        let bytes = fs::read(&file)?;
        let (current, _) = self.entry_path(rel)?;
        let extension = if kind == NodeKind::Pdf {
            PDF_EXTENSION
        } else {
            PAGE_EXTENSION
        };
        self.snapshot_as(&key, &current, extension)?;
        self.snapshots.insert(key.clone(), SystemTime::now());
        super::write_atomic(&current, &bytes)
    }

    /// An earlier version as a new page or PDF next to the original; returns its path.
    pub fn copy_version(&self, rel: &str, saved: u64, name: &str) -> Result<String, Error> {
        let (_, kind, file) = self.version(rel, saved)?;
        let (parent, _) = Self::split(rel);
        let name = self.unique_name(parent, &super::validate_name(name)?);
        let (new_rel, to) = self.target(parent, &name, kind);
        let bytes = fs::read(&file)?;
        if kind == NodeKind::Page {
            // A copy is a page of its own, with a new id and its own PDF.
            let mut page = read_page(&String::from_utf8_lossy(&bytes))?;
            page.id = crate::new_id();
            if let Some(pdf) = &page.pdf {
                let source = self.assets_dir(rel).join(pdf);
                if source.is_file() {
                    let assets = self.assets_dir(&new_rel);
                    fs::create_dir_all(&assets)?;
                    fs::copy(source, assets.join(pdf))?;
                }
            }
            let text = super::write_page(&page);
            super::write_atomic(&to, text.as_bytes())?;
        } else {
            super::write_atomic(&to, &bytes)?;
        }
        self.edit_order(parent, |order| {
            order.retain(|n| *n != name);
            order.push(name.clone());
        })?;
        Ok(new_rel)
    }
}
