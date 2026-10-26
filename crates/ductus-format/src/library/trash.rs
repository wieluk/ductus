//! Deleted entries, kept in the library's data folder with where they came from.
//! Entries from before the index restore to the top level.

use super::{ASSETS_SUFFIX, Library, NodeKind, PAGE_EXTENSION, move_path};
use crate::Error;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const TRASH_DIR: &str = "trash";
const TRASH_INDEX: &str = "trash.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Deleted {
    /// Where it was, as a library path without extension (like the tree shows it).
    original: String,
    kind: String,
    /// Milliseconds since the epoch.
    deleted: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    assets: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrashEntry {
    /// The entry's name inside the trash folder.
    pub id: String,
    pub original: String,
    pub kind: NodeKind,
    /// Milliseconds since the epoch.
    pub deleted: u64,
}

fn kind_name(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Folder => "folder",
        NodeKind::Page => "page",
        NodeKind::Pdf => "pdf",
        NodeKind::File => "file",
    }
}

fn kind_from(name: &str) -> NodeKind {
    match name {
        "folder" => NodeKind::Folder,
        "page" => NodeKind::Page,
        "pdf" => NodeKind::Pdf,
        _ => NodeKind::File,
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

impl Library {
    fn trash_dir(&self) -> PathBuf {
        self.data.join(TRASH_DIR)
    }

    fn trash_index(&self) -> BTreeMap<String, Deleted> {
        fs::read_to_string(self.data.join(TRASH_INDEX))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save_trash_index(&self, index: &BTreeMap<String, Deleted>) -> Result<(), Error> {
        fs::create_dir_all(&self.data)?;
        let json = serde_json::to_string_pretty(index)?;
        super::write_atomic(&self.data.join(TRASH_INDEX), json.as_bytes())
    }

    /// Moves an entry (and a page's pictures) into the trash; returns its trash id.
    pub fn delete(&self, rel: &str) -> Result<String, Error> {
        let (from, kind, parent, name) = self.entry(rel)?;
        let trash = self.trash_dir();
        fs::create_dir_all(&trash)?;
        let file_name = from
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut stamp = now_ms();
        while trash.join(format!("{stamp} {file_name}")).exists() {
            stamp += 1;
        }
        let id = format!("{stamp} {file_name}");
        move_path(&from, &trash.join(&id))?;
        let mut assets = None;
        let assets_dir = self.assets_dir(rel);
        if kind == NodeKind::Page && assets_dir.is_dir() {
            let assets_id = format!("{stamp} {name}{ASSETS_SUFFIX}");
            move_path(&assets_dir, &trash.join(&assets_id))?;
            assets = Some(assets_id);
        }
        let mut index = self.trash_index();
        index.insert(
            id.clone(),
            Deleted {
                original: Self::join(&parent, &name),
                kind: kind_name(kind).into(),
                deleted: stamp,
                assets,
            },
        );
        self.save_trash_index(&index)?;
        self.edit_order(&parent, |order| order.retain(|n| *n != name))?;
        self.path_lists_removed(rel)?;
        Ok(id)
    }

    /// Everything in the trash, most recently deleted first.
    pub fn trash(&self) -> Result<Vec<TrashEntry>, Error> {
        let dir = self.trash_dir();
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        let index = self.trash_index();
        let pictures: Vec<&String> = index.values().filter_map(|d| d.assets.as_ref()).collect();
        let mut entries = Vec::new();
        for entry in fs::read_dir(&dir)?.flatten() {
            let id = entry.file_name().to_string_lossy().into_owned();
            if pictures.contains(&&id) {
                continue;
            }
            if let Some(deleted) = index.get(&id) {
                entries.push(TrashEntry {
                    id,
                    original: deleted.original.clone(),
                    kind: kind_from(&deleted.kind),
                    deleted: deleted.deleted,
                });
                continue;
            }
            // From before the index: "<seconds> <file name>".
            let Some((stamp, file)) = id.split_once(' ') else {
                continue;
            };
            let is_dir = entry.file_type().is_ok_and(|t| t.is_dir());
            if is_dir && file.ends_with(ASSETS_SUFFIX) {
                continue;
            }
            let (original, kind) = if is_dir {
                (file.to_owned(), NodeKind::Folder)
            } else if let Some(page) = file.strip_suffix(&format!(".{PAGE_EXTENSION}")) {
                (page.to_owned(), NodeKind::Page)
            } else if let Some(pdf) = super::pdf_stem(file) {
                (pdf.to_owned(), NodeKind::Pdf)
            } else {
                (file.to_owned(), NodeKind::File)
            };
            entries.push(TrashEntry {
                id: id.clone(),
                original,
                kind,
                deleted: stamp.parse::<u64>().unwrap_or(0).saturating_mul(1000),
            });
        }
        entries.sort_by(|a, b| b.deleted.cmp(&a.deleted).then_with(|| a.id.cmp(&b.id)));
        Ok(entries)
    }

    /// Puts an entry back where it was, numbered if the name is taken; returns its path.
    pub fn restore(&self, id: &str) -> Result<String, Error> {
        let entry = self
            .trash()?
            .into_iter()
            .find(|e| e.id == id)
            .ok_or_else(|| Error::Invalid("That item is no longer in the trash.".into()))?;
        let (parent, name) = Self::split(&entry.original);
        let parent = parent.to_owned();
        if !parent.is_empty() && !self.abs(&parent).is_dir() {
            fs::create_dir_all(self.abs(&parent))?;
        }
        let name = self.unique_name(&parent, name);
        let (rel, to) = self.target(&parent, &name, entry.kind);
        move_path(&self.trash_dir().join(id), &to)?;
        let mut index = self.trash_index();
        let assets = index.get(id).and_then(|d| d.assets.clone()).or_else(|| {
            let legacy = format!(
                "{} {}{ASSETS_SUFFIX}",
                id.split_once(' ').map_or("", |(s, _)| s),
                Self::split(&entry.original).1
            );
            (entry.kind == NodeKind::Page && self.trash_dir().join(&legacy).is_dir())
                .then_some(legacy)
        });
        if let Some(assets) = assets {
            let from = self.trash_dir().join(&assets);
            if from.is_dir() {
                move_path(&from, &self.assets_dir(&Self::join(&parent, &name)))?;
            }
        }
        index.remove(id);
        self.save_trash_index(&index)?;
        self.edit_order(&parent, |order| {
            order.retain(|n| *n != name);
            order.push(name.clone());
        })?;
        Ok(rel)
    }

    /// Deletes an entry from the trash for good.
    pub fn purge(&self, id: &str) -> Result<(), Error> {
        let dir = self.trash_dir();
        let mut index = self.trash_index();
        let mut doomed = vec![dir.join(id)];
        if let Some(assets) = index.get(id).and_then(|d| d.assets.clone()) {
            doomed.push(dir.join(assets));
        }
        for path in doomed {
            if path.is_dir() {
                fs::remove_dir_all(&path)?;
            } else if path.exists() {
                fs::remove_file(&path)?;
            }
        }
        if index.remove(id).is_some() {
            self.save_trash_index(&index)?;
        }
        Ok(())
    }

    /// Empties the trash, including entries from before the index.
    pub fn empty_trash(&self) -> Result<(), Error> {
        let dir = self.trash_dir();
        if dir.is_dir() {
            fs::remove_dir_all(&dir)?;
        }
        self.save_trash_index(&BTreeMap::new())
    }
}
