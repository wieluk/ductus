//! A plain folder tree any sync tool handles: .ductus.json keeps folder colors and order, .ductus/ history and trash.

use crate::page::{DX_NS, PageFile, escape_attr, read_page, write_page, write_page_in};
use crate::{Error, new_id};
use ductus_core::{Document, PageSettings, Rgba};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const META_FILE: &str = ".ductus.json";
pub const APP_DIR: &str = ".ductus";
pub const PAGE_EXTENSION: &str = "svg";
pub const PDF_EXTENSION: &str = "pdf";
pub const ASSETS_SUFFIX: &str = ".assets";
/// Other files in an attachments folder are never moved away.
pub const IMAGE_PREFIX: &str = "img-";
const ASSETS_HISTORY: &str = "assets";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Folder,
    Page,
    Pdf,
    File,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub name: String,
    pub path: String,
    pub kind: NodeKind,
    pub color: Option<Rgba>,
    pub conflict: bool,
    pub children: Vec<Node>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct FolderMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    color: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    order: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct HistoryPolicy {
    pub interval: Duration,
    pub keep: usize,
}

impl Default for HistoryPolicy {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(10 * 60),
            keep: 30,
        }
    }
}

pub struct Library {
    root: PathBuf,
    policy: HistoryPolicy,
    snapshots: HashMap<String, SystemTime>,
    pub(crate) listings: RefCell<HashMap<String, crate::browse::Listing>>,
}

pub fn pdf_stem(name: &str) -> Option<&str> {
    let split = name.len().checked_sub(PDF_EXTENSION.len() + 1)?;
    let (stem, ext) = (name.get(..split)?, name.get(split..)?);
    (ext.starts_with('.') && ext[1..].eq_ignore_ascii_case(PDF_EXTENSION) && !stem.is_empty())
        .then_some(stem)
}

fn stable_key(rel: &str) -> String {
    // FNV-1a: stable across runs and platforms, unlike std's hasher.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in rel.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

pub fn validate_name(name: &str) -> Result<String, Error> {
    let name = name.trim();
    let invalid = |why: &str| Err(Error::Invalid(why.to_owned()));
    if name.is_empty() {
        return invalid("The name can't be empty.");
    }
    if name.starts_with('.') {
        return invalid("The name can't start with a dot.");
    }
    if let Some(c) = name.chars().find(|c| {
        matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control()
    }) {
        return Err(Error::Invalid(format!(
            "The name can't contain \u{201c}{c}\u{201d}."
        )));
    }
    if name.ends_with('.') {
        return invalid("The name can't end with a dot.");
    }
    Ok(name.to_owned())
}

/// Pages declare the Ductus namespace; other SVGs (template logos, figures) are plain files.
fn is_page_file(path: &Path) -> bool {
    let mut head = Vec::new();
    fs::File::open(path)
        .and_then(|f| f.take(4096).read_to_end(&mut head))
        .is_ok()
        && head.windows(DX_NS.len()).any(|w| w == DX_NS.as_bytes())
}

fn is_conflict_copy(name: &str) -> bool {
    name.contains(".sync-conflict-") || name.to_lowercase().contains("conflicted copy")
}

fn color_to_hex(c: Rgba) -> String {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", b(c.r), b(c.g), b(c.b))
}

fn color_from_hex(s: &str) -> Option<Rgba> {
    let s = s.strip_prefix('#')?;
    if s.len() != 6 {
        return None;
    }
    let c = |i: usize| {
        u8::from_str_radix(&s[i..i + 2], 16)
            .ok()
            .map(|v| f32::from(v) / 255.0)
    };
    Some(Rgba::rgb(c(0)?, c(2)?, c(4)?))
}

/// Temp file, flush, rename: readers never see a partial file.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    use std::io::Write;
    let dir = path
        .parent()
        .ok_or_else(|| Error::Invalid("path has no parent".into()))?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{name}.tmp-{}", std::process::id()));
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(e.into());
    }
    #[cfg(unix)]
    if let Ok(d) = fs::File::open(dir) {
        let _ = d.sync_all();
    }
    Ok(())
}

impl Library {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, Error> {
        Self::with_policy(root, HistoryPolicy::default())
    }

    pub fn with_policy(root: impl Into<PathBuf>, policy: HistoryPolicy) -> Result<Self, Error> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        Ok(Self {
            root,
            policy,
            snapshots: HashMap::new(),
            listings: RefCell::default(),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn abs(&self, rel: &str) -> PathBuf {
        rel.split('/')
            .filter(|p| !p.is_empty())
            .fold(self.root.clone(), |acc, p| acc.join(p))
    }

    fn page_file(&self, rel: &str) -> PathBuf {
        let mut p = self.abs(rel);
        p.set_extension(PAGE_EXTENSION);
        p
    }

    fn entry_path(&self, rel: &str) -> Result<(PathBuf, NodeKind), Error> {
        let folder = self.abs(rel);
        let (_, file) = Self::split(rel);
        if pdf_stem(file).is_some() && folder.is_file() {
            return Ok((folder, NodeKind::Pdf));
        }
        if !rel.is_empty() && folder.is_dir() {
            return Ok((folder, NodeKind::Folder));
        }
        // Pages are named without their extension, so a file with exactly
        // this name is some other file (such as a plain `.svg` drawing).
        if !rel.is_empty() && folder.is_file() {
            return Ok((folder, NodeKind::File));
        }
        let page = self.page_file(rel);
        if page.is_file() {
            return Ok((page, NodeKind::Page));
        }
        Err(Error::Invalid(format!(
            "\u{201c}{rel}\u{201d} doesn't exist."
        )))
    }

    pub fn exists(&self, rel: &str) -> bool {
        self.entry_path(rel).is_ok()
    }

    pub fn assets_dir(&self, rel: &str) -> PathBuf {
        let mut p = self.abs(rel).into_os_string();
        p.push(ASSETS_SUFFIX);
        PathBuf::from(p)
    }

    fn split(rel: &str) -> (&str, &str) {
        match rel.rsplit_once('/') {
            Some((parent, name)) => (parent, name),
            None => ("", rel),
        }
    }

    fn join(parent: &str, name: &str) -> String {
        if parent.is_empty() {
            name.to_owned()
        } else {
            format!("{parent}/{name}")
        }
    }

    fn read_meta(&self, folder: &str) -> FolderMeta {
        fs::read_to_string(self.abs(folder).join(META_FILE))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn write_meta(&self, folder: &str, meta: &FolderMeta) -> Result<(), Error> {
        let json = serde_json::to_string_pretty(meta)?;
        write_atomic(&self.abs(folder).join(META_FILE), json.as_bytes())
    }

    fn edit_order(&self, folder: &str, f: impl FnOnce(&mut Vec<String>)) -> Result<(), Error> {
        let mut meta = self.read_meta(folder);
        // Start from the displayed order so unordered entries keep their place.
        meta.order = self.child_names(folder)?;
        f(&mut meta.order);
        self.write_meta(folder, &meta)
    }

    fn child_names(&self, folder: &str) -> Result<Vec<String>, Error> {
        Ok(self
            .scan(folder, false)?
            .into_iter()
            .map(|n| n.name)
            .collect())
    }

    pub fn tree(&self) -> Result<Node, Error> {
        let name = self
            .root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(Node {
            name,
            path: String::new(),
            kind: NodeKind::Folder,
            color: None,
            conflict: false,
            children: self.scan("", true)?,
        })
    }

    fn scan(&self, folder: &str, recursive: bool) -> Result<Vec<Node>, Error> {
        let mut nodes: Vec<Node> = self
            .list(folder)?
            .iter()
            .filter(|n| n.kind != NodeKind::File)
            .cloned()
            .collect();
        for node in nodes.iter_mut().filter(|n| n.kind == NodeKind::Folder) {
            node.color = self.folder_color(&node.path);
            if recursive && node.path.matches('/').count() < 48 {
                node.children = self.scan(&node.path, true).unwrap_or_default();
            }
        }
        Ok(nodes)
    }

    pub fn folder_color(&self, folder: &str) -> Option<Rgba> {
        self.read_meta(folder)
            .color
            .as_deref()
            .and_then(color_from_hex)
    }

    pub(crate) fn read_folder(&self, folder: &str) -> Result<Vec<Node>, Error> {
        let mut nodes = Vec::new();
        for entry in fs::read_dir(self.abs(folder))?.flatten() {
            let file_name = entry.file_name().to_string_lossy().into_owned();
            if file_name.starts_with('.') {
                continue;
            }
            let Ok(mut file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                // Linked folders and files count as what they point to.
                match fs::metadata(entry.path()) {
                    Ok(meta) => file_type = meta.file_type(),
                    Err(_) => continue,
                }
            }
            let path = Self::join(folder, &file_name);
            let node = |name: &str, path: String, kind: NodeKind| Node {
                name: name.to_owned(),
                path,
                kind,
                color: None,
                conflict: kind != NodeKind::Folder && is_conflict_copy(name),
                children: Vec::new(),
            };
            if file_type.is_dir() {
                if !file_name.ends_with(ASSETS_SUFFIX) {
                    nodes.push(node(&file_name, path, NodeKind::Folder));
                }
            } else if let Some(stem) = pdf_stem(&file_name) {
                nodes.push(node(stem, path, NodeKind::Pdf));
            } else if let Some(stem) = file_name
                .strip_suffix(&format!(".{PAGE_EXTENSION}"))
                .filter(|_| is_page_file(&entry.path()))
            {
                nodes.push(node(stem, Self::join(folder, stem), NodeKind::Page));
            } else if file_type.is_file() {
                nodes.push(node(&file_name, path, NodeKind::File));
            }
        }
        // Ordered entries first, in their stored order; the rest alphabetically.
        let order = self.read_meta(folder).order;
        let rank = |name: &str| order.iter().position(|o| o == name).unwrap_or(usize::MAX);
        nodes.sort_by(|a, b| {
            rank(&a.name)
                .cmp(&rank(&b.name))
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(nodes)
    }

    pub fn unique_name(&self, folder: &str, base: &str) -> String {
        let taken = |name: &str| {
            let rel = Self::join(folder, name);
            self.abs(&rel).exists()
                || self.page_file(&rel).exists()
                || self.abs(&format!("{rel}.{PDF_EXTENSION}")).exists()
        };
        if !taken(base) {
            return base.to_owned();
        }
        (2..)
            .map(|n| format!("{base} {n}"))
            .find(|n| !taken(n))
            .expect("some number is free")
    }

    fn ensure_free(&self, folder: &str, name: &str) -> Result<(), Error> {
        let rel = Self::join(folder, name);
        if self.abs(&rel).exists()
            || self.page_file(&rel).exists()
            || self.abs(&format!("{rel}.{PDF_EXTENSION}")).exists()
        {
            return Err(Error::Invalid(format!(
                "There is already an entry named \u{201c}{name}\u{201d} here."
            )));
        }
        Ok(())
    }

    pub fn create_folder(&self, parent: &str, name: &str) -> Result<String, Error> {
        let name = validate_name(name)?;
        self.ensure_free(parent, &name)?;
        let rel = Self::join(parent, &name);
        fs::create_dir(self.abs(&rel))?;
        self.edit_order(parent, |order| {
            order.retain(|n| n != &name);
            order.push(name.clone());
        })?;
        Ok(rel)
    }

    pub fn create_page(
        &mut self,
        parent: &str,
        name: &str,
        settings: PageSettings,
    ) -> Result<String, Error> {
        let name = validate_name(name)?;
        self.ensure_free(parent, &name)?;
        let rel = Self::join(parent, &name);
        let page = PageFile {
            id: new_id(),
            document: Document::with_contents(settings, []),
            pdf: None,
            recognized: Vec::new(),
        };
        write_atomic(&self.page_file(&rel), write_page(&page).as_bytes())?;
        self.edit_order(parent, |order| {
            order.retain(|n| n != &name);
            order.push(name.clone());
        })?;
        Ok(rel)
    }

    fn entry(&self, rel: &str) -> Result<(PathBuf, NodeKind, String, String), Error> {
        let (path, kind) = self.entry_path(rel)?;
        let (parent, file) = Self::split(rel);
        let name = match kind {
            NodeKind::Pdf => pdf_stem(file).unwrap_or(file),
            _ => file,
        };
        Ok((path, kind, parent.to_owned(), name.to_owned()))
    }

    fn target(&self, parent: &str, name: &str, kind: NodeKind) -> (String, PathBuf) {
        let rel = Self::join(parent, name);
        match kind {
            NodeKind::Folder | NodeKind::File => (rel.clone(), self.abs(&rel)),
            NodeKind::Page => (rel.clone(), self.page_file(&rel)),
            NodeKind::Pdf => {
                let rel = format!("{rel}.{PDF_EXTENSION}");
                (rel.clone(), self.abs(&rel))
            }
        }
    }

    fn relocate(
        &self,
        from_rel: &str,
        from: &Path,
        kind: NodeKind,
        to_rel: &str,
        to: &Path,
    ) -> Result<(), Error> {
        fs::rename(from, to)?;
        if kind == NodeKind::Page {
            let assets = self.assets_dir(from_rel);
            if assets.is_dir() {
                fs::rename(assets, self.assets_dir(to_rel))?;
                // Picture links in the page name its attachments folder.
                let (old, new) = (
                    Self::assets_dir_name(from_rel),
                    Self::assets_dir_name(to_rel),
                );
                if old != new {
                    let svg = fs::read_to_string(to)?;
                    let link = |dir: &str| format!(r#"href="{}/"#, escape_attr(dir));
                    if svg.contains(&link(&old)) {
                        write_atomic(to, svg.replace(&link(&old), &link(&new)).as_bytes())?;
                    }
                }
            }
        }
        Ok(())
    }

    fn assets_dir_name(rel: &str) -> String {
        format!("{}{ASSETS_SUFFIX}", Self::split(rel).1)
    }

    pub fn save_asset(&self, rel: &str, name: &str, bytes: &[u8]) -> Result<(), Error> {
        let dir = self.assets_dir(rel);
        let path = dir.join(name);
        if !path.is_file() {
            fs::create_dir_all(&dir)?;
            write_atomic(&path, bytes)?;
        }
        Ok(())
    }

    pub fn read_asset(&self, rel: &str, name: &str) -> Result<Vec<u8>, Error> {
        Ok(fs::read(self.assets_dir(rel).join(name))?)
    }

    /// Older versions keep their pictures while the attachments folder stays tidy.
    fn retire_unused_images(&self, rel: &str, page: &PageFile) -> Result<(), Error> {
        let Ok(entries) = fs::read_dir(self.assets_dir(rel)) else {
            return Ok(());
        };
        let used: std::collections::HashSet<&str> = page
            .document
            .images()
            .iter()
            .map(|i| i.source.as_str())
            .collect();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with(IMAGE_PREFIX) && !used.contains(name.as_str()) {
                let keep = self.history_dir(&page.id).join(ASSETS_HISTORY);
                fs::create_dir_all(&keep)?;
                fs::rename(entry.path(), keep.join(&name))?;
            }
        }
        Ok(())
    }

    pub fn rename(&self, rel: &str, new_name: &str) -> Result<String, Error> {
        let new_name = validate_name(new_name)?;
        let (from, kind, parent, old_name) = self.entry(rel)?;
        if old_name == new_name {
            return Ok(rel.to_owned());
        }
        // Allow case-only renames on case-insensitive file systems.
        if !old_name.eq_ignore_ascii_case(&new_name) {
            self.ensure_free(&parent, &new_name)?;
        }
        let (new_rel, to) = self.target(&parent, &new_name, kind);
        self.edit_order(&parent, |order| {
            for n in order.iter_mut() {
                if *n == old_name {
                    n.clone_from(&new_name);
                }
            }
        })?;
        self.relocate(rel, &from, kind, &new_rel, &to)?;
        self.path_lists_moved(rel, &new_rel)?;
        Ok(new_rel)
    }

    pub fn delete(&self, rel: &str) -> Result<(), Error> {
        let (from, kind, parent, name) = self.entry(rel)?;
        let trash = self.root.join(APP_DIR).join("trash");
        fs::create_dir_all(&trash)?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let file_name = from
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        fs::rename(&from, trash.join(format!("{stamp} {file_name}")))?;
        let assets = self.assets_dir(rel);
        if kind == NodeKind::Page && assets.is_dir() {
            fs::rename(assets, trash.join(format!("{stamp} {name}{ASSETS_SUFFIX}")))?;
        }
        self.edit_order(&parent, |order| order.retain(|n| *n != name))?;
        self.path_lists_removed(rel)
    }

    pub fn move_to(
        &self,
        rel: &str,
        new_parent: &str,
        index: Option<usize>,
    ) -> Result<String, Error> {
        let (from, kind, old_parent, name) = self.entry(rel)?;
        if kind == NodeKind::Folder
            && (new_parent == rel || new_parent.starts_with(&format!("{rel}/")))
        {
            return Err(Error::Invalid(
                "A folder can't be moved into itself.".into(),
            ));
        }
        let (new_rel, to) = self.target(new_parent, &name, kind);
        if old_parent != new_parent {
            self.ensure_free(new_parent, &name)?;
            self.relocate(rel, &from, kind, &new_rel, &to)?;
            self.edit_order(&old_parent, |order| order.retain(|n| *n != name))?;
        }
        self.edit_order(new_parent, |order| {
            order.retain(|n| *n != name);
            let at = index.unwrap_or(order.len()).min(order.len());
            order.insert(at, name.clone());
        })?;
        self.path_lists_moved(rel, &new_rel)?;
        Ok(new_rel)
    }

    pub fn import_page(&self, parent: &str, file: &Path) -> Result<String, Error> {
        let text = fs::read_to_string(file)?;
        read_page(&text)?;
        let stem = file
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Page".into());
        let name = self.unique_name(parent, &validate_name(&stem)?);
        let rel = Self::join(parent, &name);
        let assets = PathBuf::from(format!(
            "{}{ASSETS_SUFFIX}",
            file.with_extension("").display()
        ));
        if assets.is_dir() {
            let target = self.assets_dir(&rel);
            fs::create_dir_all(&target)?;
            for entry in fs::read_dir(&assets)?.flatten() {
                if entry.file_type().is_ok_and(|t| t.is_file()) {
                    fs::copy(entry.path(), target.join(entry.file_name()))?;
                }
            }
        }
        write_atomic(&self.page_file(&rel), text.as_bytes())?;
        self.edit_order(parent, |order| {
            order.retain(|n| *n != name);
            order.push(name.clone());
        })?;
        Ok(rel)
    }

    pub fn add_pdf(&self, parent: &str, name: &str, bytes: &[u8]) -> Result<String, Error> {
        let name = validate_name(name)?;
        self.ensure_free(parent, &name)?;
        let (rel, path) = self.target(parent, &name, NodeKind::Pdf);
        write_atomic(&path, bytes)?;
        self.edit_order(parent, |order| {
            order.retain(|n| *n != name);
            order.push(name.clone());
        })?;
        Ok(rel)
    }

    pub fn create_pdf_page(
        &mut self,
        parent: &str,
        name: &str,
        pdf: &[u8],
        page_sizes: Vec<(f64, f64)>,
    ) -> Result<String, Error> {
        let name = validate_name(name)?;
        self.ensure_free(parent, &name)?;
        let rel = Self::join(parent, &name);
        let assets = self.assets_dir(&rel);
        fs::create_dir_all(&assets)?;
        write_atomic(&assets.join("source.pdf"), pdf)?;
        let mut document = Document::with_contents(
            PageSettings {
                kind: ductus_core::PageKind::Pdf,
                ..PageSettings::default()
            },
            [],
        );
        document.set_pdf_sheets(page_sizes);
        let page = PageFile {
            id: new_id(),
            document,
            pdf: Some("source.pdf".into()),
            recognized: Vec::new(),
        };
        write_atomic(&self.page_file(&rel), write_page(&page).as_bytes())?;
        self.edit_order(parent, |order| {
            order.retain(|n| *n != name);
            order.push(name.clone());
        })?;
        Ok(rel)
    }

    pub fn read_page_pdf(&self, rel: &str, file: &str) -> Result<Vec<u8>, Error> {
        Ok(fs::read(self.assets_dir(rel).join(file))?)
    }

    pub fn read_pdf(&self, rel: &str) -> Result<Vec<u8>, Error> {
        Ok(fs::read(self.abs(rel))?)
    }

    pub fn save_pdf(&mut self, rel: &str, bytes: &[u8]) -> Result<(), Error> {
        let path = self.abs(rel);
        let key = format!("pdf-{}", stable_key(rel));
        if path.is_file() {
            let now = SystemTime::now();
            let due = self.snapshots.get(&key).is_none_or(|last| {
                now.duration_since(*last).unwrap_or_default() >= self.policy.interval
            });
            if due {
                self.snapshot_as(&key, &path, PDF_EXTENSION)?;
                self.snapshots.insert(key, now);
            }
        }
        write_atomic(&path, bytes)
    }

    pub fn set_color(&self, folder: &str, color: Option<Rgba>) -> Result<(), Error> {
        let mut meta = self.read_meta(folder);
        meta.color = color.map(color_to_hex);
        self.write_meta(folder, &meta)
    }

    pub fn load_page(&self, rel: &str) -> Result<PageFile, Error> {
        read_page(&fs::read_to_string(self.page_file(rel))?)
    }

    /// History gets the first save of a session, then at most one per policy interval.
    pub fn save_page(&mut self, rel: &str, page: &PageFile) -> Result<(), Error> {
        let path = self.page_file(rel);
        if path.is_file() {
            let now = SystemTime::now();
            let due = self.snapshots.get(&page.id).is_none_or(|last| {
                now.duration_since(*last).unwrap_or_default() >= self.policy.interval
            });
            if due {
                self.snapshot(&page.id, &path)?;
                self.snapshots.insert(page.id.clone(), now);
            }
        }
        write_atomic(
            &path,
            write_page_in(page, &Self::assets_dir_name(rel)).as_bytes(),
        )?;
        self.retire_unused_images(rel, page)
    }

    fn history_dir(&self, page_id: &str) -> PathBuf {
        self.root.join(APP_DIR).join("history").join(page_id)
    }

    fn snapshot(&self, page_id: &str, current: &Path) -> Result<(), Error> {
        self.snapshot_as(page_id, current, PAGE_EXTENSION)
    }

    fn snapshot_as(&self, key: &str, current: &Path, extension: &str) -> Result<(), Error> {
        let dir = self.history_dir(key);
        fs::create_dir_all(&dir)?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis());
        fs::copy(current, dir.join(format!("{stamp}.{extension}")))?;
        let mut versions = self.history(key)?;
        while versions.len() > self.policy.keep {
            fs::remove_file(versions.remove(0))?;
        }
        Ok(())
    }

    pub fn history(&self, page_id: &str) -> Result<Vec<PathBuf>, Error> {
        let dir = self.history_dir(page_id);
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut files: Vec<(u128, PathBuf)> = fs::read_dir(dir)?
            .filter_map(|e| {
                let path = e.ok()?.path();
                let stamp = path.file_stem()?.to_str()?.parse().ok()?;
                Some((stamp, path))
            })
            .collect();
        files.sort();
        Ok(files.into_iter().map(|(_, p)| p).collect())
    }
}

#[cfg(test)]
mod tests;
