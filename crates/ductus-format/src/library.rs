//! A plain folder tree of notes; colors, order, history and trash live in the app's data folder.

use crate::page::{DX_NS, PageFile, read_page, write_page};
use crate::{Error, new_id};
use ductus_core::{Document, PageSettings, Rgba};
use serde::{Deserialize, Serialize};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Every folder's color and order, by path.
const FOLDERS_FILE: &str = "folders.json";
/// Names the library a data folder belongs to, for people looking at it.
const LOCATION_FILE: &str = "library.txt";
pub const PAGE_EXTENSION: &str = "svg";
pub const PDF_EXTENSION: &str = "pdf";
pub const ASSETS_SUFFIX: &str = ".assets";
/// How picture names start, in the canvas and imports alike.
pub const IMAGE_PREFIX: &str = "img-";
const PAGE_PDF: &str = "source.pdf";

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

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
struct FolderMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    color: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    order: Vec<String>,
    /// Set when the folder is a notebook: the name it shows under Notebooks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    notebook: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct HistoryPolicy {
    pub interval: Duration,
    pub keep: usize,
    /// Above this, older versions go (but never the newest few): PDFs written on are large.
    pub max_bytes: u64,
}

impl Default for HistoryPolicy {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(10 * 60),
            keep: 30,
            max_bytes: 256 << 20,
        }
    }
}

/// Versions kept whatever their size.
const MIN_VERSIONS: usize = 5;

pub struct Library {
    root: PathBuf,
    /// History, trash and the sidebar's lists: per device, outside the synced folder.
    data: PathBuf,
    folders: RefCell<BTreeMap<String, FolderMeta>>,
    /// When `folders` was last read or written, to skip rereading an unchanged file.
    folders_stamp: Cell<Option<SystemTime>>,
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

/// A sync client's placeholder (OneDrive, Nextcloud): the first read downloads it, and waits for that.
#[cfg(windows)]
pub fn online_only(meta: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    // FILE_ATTRIBUTE_OFFLINE, FILE_ATTRIBUTE_RECALL_ON_OPEN, FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS
    meta.file_attributes() & (0x1000 | 0x4_0000 | 0x40_0000) != 0
}

#[cfg(not(windows))]
pub fn online_only(_meta: &fs::Metadata) -> bool {
    false
}

/// Pages declare the Ductus namespace; other SVGs (template logos, figures) are plain files.
/// A placeholder counts as a page unread: listing a library must not download all of it.
fn is_page_file(entry: &fs::DirEntry) -> bool {
    entry.metadata().is_ok_and(|m| online_only(&m)) || is_page(&entry.path())
}

/// Whether the SVG at `file` is a Ductus page; reads only its start.
pub fn is_page(file: &Path) -> bool {
    let mut head = Vec::new();
    fs::File::open(file)
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
    // Unique per write, so two saves of one file never share a temporary file.
    static WRITES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = WRITES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = dir.join(format!(".{name}.tmp-{}-{n}", std::process::id()));
    let written = fs::File::create(&tmp).and_then(|mut file| {
        file.write_all(bytes)?;
        file.sync_all()
    });
    // A full disk must not leave half-written files in a folder that syncs.
    if let Err(e) = written.and_then(|()| fs::rename(&tmp, path)) {
        let _ = fs::remove_file(&tmp);
        return Err(e.into());
    }
    #[cfg(unix)]
    if let Ok(d) = fs::File::open(dir) {
        let _ = d.sync_all();
    }
    Ok(())
}

/// One of Ductus's own data files, or the default when there is none. One that can't be read is
/// kept beside it as `<name>.broken` first, since the next save replaces it.
pub(crate) fn read_data<T: serde::de::DeserializeOwned + Default>(file: &Path) -> T {
    let Ok(bytes) = fs::read(file) else {
        return T::default();
    };
    serde_json::from_slice(&bytes).unwrap_or_else(|_| {
        let mut broken = file.as_os_str().to_owned();
        broken.push(".broken");
        let _ = fs::copy(file, PathBuf::from(broken));
        T::default()
    })
}

/// Stable per library path; the search index uses it too.
pub fn library_key(root: &Path) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in root.to_string_lossy().as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn plain_file(name: &str) -> Result<&str, Error> {
    match crate::page::is_plain_file_name(name) {
        true => Ok(name),
        false => Err(Error::Invalid(format!(
            "\u{201c}{name}\u{201d} isn't a file name."
        ))),
    }
}

/// Rename where possible; across file systems, copy and then delete.
fn move_path(from: &Path, to: &Path) -> Result<(), Error> {
    if fs::rename(from, to).is_ok() {
        return Ok(());
    }
    copy_tree(from, to)?;
    if from.is_dir() {
        fs::remove_dir_all(from)?;
    } else {
        fs::remove_file(from)?;
    }
    Ok(())
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), Error> {
    // A linked folder could lead back into itself: links are copied as links.
    if fs::symlink_metadata(from)?.file_type().is_symlink() {
        #[cfg(unix)]
        return Ok(std::os::unix::fs::symlink(fs::read_link(from)?, to)?);
        #[cfg(not(unix))]
        if from.is_file() {
            fs::copy(from, to)?;
            return Ok(());
        }
        #[cfg(not(unix))]
        return Ok(());
    }
    if from.is_dir() {
        fs::create_dir_all(to)?;
        for entry in fs::read_dir(from)? {
            let entry = entry?;
            copy_tree(&entry.path(), &to.join(entry.file_name()))?;
        }
    } else {
        fs::copy(from, to)?;
    }
    Ok(())
}

/// Moves `old` into `new`; entries already in `new` win.
fn merge_into(old: &Path, new: &Path) -> Result<(), Error> {
    fs::create_dir_all(new)?;
    for entry in fs::read_dir(old)? {
        let entry = entry?;
        let target = new.join(entry.file_name());
        if !target.exists() {
            move_path(&entry.path(), &target)?;
        } else if entry.file_type()?.is_dir() && target.is_dir() {
            merge_into(&entry.path(), &target)?;
        }
    }
    // Only goes when everything moved.
    let _ = fs::remove_dir(old);
    Ok(())
}

/// Moves a library's notes into another folder, with its history, trash and notebook settings.
pub fn relocate_library(from: &Path, to: &Path, app_data: &Path) -> Result<(), Error> {
    if from.is_dir() {
        merge_into(from, to)?;
    } else {
        fs::create_dir_all(to)?;
    }
    let libraries = app_data.join("libraries");
    let old = libraries.join(library_key(from));
    if old.is_dir() {
        merge_into(&old, &libraries.join(library_key(to)))?;
    }
    Ok(())
}

impl Library {
    /// Each library gets its own folder in `app_data`.
    pub fn open(root: impl Into<PathBuf>, app_data: &Path) -> Result<Self, Error> {
        Self::with_policy(root, app_data, HistoryPolicy::default())
    }

    pub fn with_policy(
        root: impl Into<PathBuf>,
        app_data: &Path,
        policy: HistoryPolicy,
    ) -> Result<Self, Error> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        let data = app_data.join("libraries").join(library_key(&root));
        fs::create_dir_all(&data)?;
        let location = data.join(LOCATION_FILE);
        let named = root.to_string_lossy();
        if fs::read_to_string(&location).ok().as_deref() != Some(named.as_ref()) {
            write_atomic(&location, named.as_bytes())?;
        }
        let library = Self {
            root,
            data,
            folders: RefCell::default(),
            folders_stamp: Cell::new(None),
            policy,
            snapshots: HashMap::new(),
            listings: RefCell::default(),
        };
        library.reload_folders();
        library.mark_existing_notebooks()?;
        Ok(library)
    }

    pub fn data_dir(&self) -> &Path {
        &self.data
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// A library path on disk. It never leads out of the library: "." and ".." parts are dropped.
    pub fn abs(&self, rel: &str) -> PathBuf {
        rel.split('/')
            .filter(|p| !p.is_empty() && *p != "." && *p != "..")
            .fold(self.root.clone(), |acc, p| acc.join(p))
    }

    /// Appended, not replaced: a page may be called "18.05.2026".
    pub fn page_file(&self, rel: &str) -> PathBuf {
        let mut p = self.abs(rel).into_os_string();
        p.push(format!(".{PAGE_EXTENSION}"));
        p.into()
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
        // Pages are named without extension: a file with exactly this name is some other file.
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

    /// Another Library on the same data (the search worker's) may have written since.
    fn reload_folders(&self) {
        let file = self.data.join(FOLDERS_FILE);
        let stamp = fs::metadata(&file).and_then(|m| m.modified()).ok();
        if stamp.is_some() && stamp == self.folders_stamp.get() {
            return;
        }
        *self.folders.borrow_mut() = read_data(&file);
        self.folders_stamp.set(stamp);
    }

    fn save_folders(&self) -> Result<(), Error> {
        let file = self.data.join(FOLDERS_FILE);
        let json = serde_json::to_string_pretty(&*self.folders.borrow())?;
        write_atomic(&file, json.as_bytes())?;
        self.folders_stamp
            .set(fs::metadata(&file).and_then(|m| m.modified()).ok());
        Ok(())
    }

    fn read_meta(&self, folder: &str) -> FolderMeta {
        if let Some(meta) = self.folders.borrow().get(folder) {
            return meta.clone();
        }
        self.reload_folders();
        self.folders
            .borrow()
            .get(folder)
            .cloned()
            .unwrap_or_default()
    }

    fn write_meta(&self, folder: &str, meta: &FolderMeta) -> Result<(), Error> {
        self.reload_folders();
        if *meta == FolderMeta::default() {
            self.folders.borrow_mut().remove(folder);
        } else {
            self.folders
                .borrow_mut()
                .insert(folder.to_owned(), meta.clone());
        }
        self.save_folders()
    }

    /// The name a folder shows as a notebook, if it is one.
    pub fn notebook_name(&self, folder: &str) -> Option<String> {
        self.read_meta(folder).notebook
    }

    /// Makes a folder a notebook under `name` (its own name when blank), or `None` a plain folder again.
    pub fn set_notebook(&self, folder: &str, name: Option<&str>) -> Result<(), Error> {
        let mut meta = self.read_meta(folder);
        meta.notebook = name.map(|n| {
            let n = n.trim();
            if n.is_empty() {
                folder.rsplit('/').next().unwrap_or_default().to_owned()
            } else {
                n.to_owned()
            }
        });
        self.write_meta(folder, &meta)
    }

    /// Every folder marked as a notebook, in path order.
    pub(crate) fn marked_notebooks(&self) -> Vec<String> {
        self.reload_folders();
        self.folders
            .borrow()
            .iter()
            .filter(|(path, meta)| meta.notebook.is_some() && !path.is_empty())
            .map(|(path, _)| path.clone())
            .collect()
    }

    /// Colors and orders follow a renamed or moved folder; `to` None drops them with it.
    pub(crate) fn folders_moved(&self, from: &str, to: Option<&str>) -> Result<(), Error> {
        self.reload_folders();
        let mut folders = self.folders.borrow_mut();
        let keys: Vec<String> = folders
            .keys()
            .filter(|k| *k == from || k.starts_with(&format!("{from}/")))
            .cloned()
            .collect();
        if keys.is_empty() {
            return Ok(());
        }
        for key in keys {
            let meta = folders.remove(&key).unwrap_or_default();
            if let Some(to) = to {
                folders.insert(format!("{to}{}", &key[from.len()..]), meta);
            }
        }
        drop(folders);
        self.save_folders()
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

    /// Pages and PDFs in a folder and its subfolders, in tree order.
    pub fn pages_in(&self, folder: &str) -> Result<Vec<String>, Error> {
        fn walk(nodes: &[Node], out: &mut Vec<String>) {
            for node in nodes {
                match node.kind {
                    NodeKind::Page | NodeKind::Pdf => out.push(node.path.clone()),
                    NodeKind::Folder => walk(&node.children, out),
                    NodeKind::File => {}
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.scan(folder, true)?, &mut out);
        Ok(out)
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
                .filter(|_| is_page_file(&entry))
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
        self.add_page(
            parent,
            name,
            Document::with_contents(settings, []),
            None,
            &[],
        )
    }

    /// A new page holding `document`, with its pictures and the PDF it is written over.
    pub fn add_page(
        &mut self,
        parent: &str,
        name: &str,
        document: Document,
        pdf: Option<&[u8]>,
        pictures: &[(String, Vec<u8>)],
    ) -> Result<String, Error> {
        let name = validate_name(name)?;
        self.ensure_free(parent, &name)?;
        let rel = Self::join(parent, &name);
        if let Some(bytes) = pdf {
            let dir = self.assets_dir(&rel);
            fs::create_dir_all(&dir)?;
            write_atomic(&dir.join(PAGE_PDF), bytes)?;
        }
        let page = PageFile {
            id: new_id(),
            document,
            pdf: pdf.map(|_| PAGE_PDF.to_owned()),
            recognized: Vec::new(),
            pictures: pictures.iter().cloned().collect(),
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
            }
        }
        Ok(())
    }

    /// A notebook takes the new name too, so it is called like its folder.
    pub fn rename(&self, rel: &str, new_name: &str) -> Result<String, Error> {
        let new_name = validate_name(new_name)?;
        let (from, kind, parent, old_name) = self.entry(rel)?;
        let new_rel = if old_name == new_name {
            rel.to_owned()
        } else {
            self.rename_entry(rel, &from, kind, &parent, &old_name, &new_name)?
        };
        if kind == NodeKind::Folder && self.is_notebook(&new_rel) {
            self.set_notebook(&new_rel, Some(&new_name))?;
        }
        Ok(new_rel)
    }

    fn rename_entry(
        &self,
        rel: &str,
        from: &Path,
        kind: NodeKind,
        parent: &str,
        old_name: &str,
        new_name: &str,
    ) -> Result<String, Error> {
        // Allow case-only renames on case-insensitive file systems.
        if !old_name.eq_ignore_ascii_case(new_name) {
            self.ensure_free(parent, new_name)?;
        }
        let (new_rel, to) = self.target(parent, new_name, kind);
        self.edit_order(parent, |order| {
            for n in order.iter_mut() {
                if n == old_name {
                    *n = new_name.to_owned();
                }
            }
        })?;
        self.relocate(rel, from, kind, &new_rel, &to)?;
        self.path_lists_moved(rel, &new_rel)?;
        Ok(new_rel)
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

    /// Any other file, such as an attachment brought along by an import; a free name is picked.
    pub fn add_file(&self, parent: &str, file_name: &str, bytes: &[u8]) -> Result<String, Error> {
        let file_name = validate_name(file_name)?;
        let (stem, ext) = match file_name.rsplit_once('.') {
            Some((stem, ext)) if !stem.is_empty() => (stem, format!(".{ext}")),
            _ => (file_name.as_str(), String::new()),
        };
        let name = (1..)
            .map(|n| match n {
                1 => format!("{stem}{ext}"),
                n => format!("{stem} {n}{ext}"),
            })
            .find(|n| self.ensure_free(parent, n).is_ok())
            .expect("some number is free");
        let rel = Self::join(parent, &name);
        write_atomic(&self.abs(&rel), bytes)?;
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
        let mut document = Document::with_contents(
            PageSettings {
                kind: ductus_core::PageKind::Pdf,
                ..PageSettings::default()
            },
            [],
        );
        document.set_pdf_sheets(page_sizes);
        self.add_page(parent, name, document, Some(pdf), &[])
    }

    /// Replaces the PDF a notes page is written over.
    pub fn save_page_pdf(&self, rel: &str, file: &str, bytes: &[u8]) -> Result<(), Error> {
        let file = plain_file(file)?;
        let dir = self.assets_dir(rel);
        fs::create_dir_all(&dir)?;
        write_atomic(&dir.join(file), bytes)
    }

    pub fn read_page_pdf(&self, rel: &str, file: &str) -> Result<Vec<u8>, Error> {
        Ok(fs::read(self.assets_dir(rel).join(plain_file(file)?))?)
    }

    pub fn read_pdf(&self, rel: &str) -> Result<Vec<u8>, Error> {
        Ok(fs::read(self.abs(rel))?)
    }

    pub fn save_pdf(&mut self, rel: &str, bytes: &[u8]) -> Result<(), Error> {
        self.prepare_pdf_save(rel).write(bytes)
    }

    /// A save of a PDF written on, to be written later (maybe on another thread).
    pub fn prepare_pdf_save(&mut self, rel: &str) -> PendingSave {
        let path = self.abs(rel);
        let key = format!("pdf-{}", stable_key(rel));
        self.prepare_save(&key, path, PDF_EXTENSION)
    }

    /// History gets the first save of a session, then at most one per policy interval.
    fn prepare_save(&mut self, key: &str, path: PathBuf, extension: &'static str) -> PendingSave {
        let mut history = None;
        if path.is_file() {
            let now = SystemTime::now();
            let due = self.snapshots.get(key).is_none_or(|last| {
                now.duration_since(*last).unwrap_or_default() >= self.policy.interval
            });
            if due {
                history = Some((self.history_dir(key), extension));
                self.snapshots.insert(key.to_owned(), now);
            }
        }
        PendingSave {
            path,
            history,
            policy: self.policy,
        }
    }

    pub fn set_color(&self, folder: &str, color: Option<Rgba>) -> Result<(), Error> {
        let mut meta = self.read_meta(folder);
        meta.color = color.map(color_to_hex);
        self.write_meta(folder, &meta)
    }

    pub fn load_page(&self, rel: &str) -> Result<PageFile, Error> {
        read_page(&fs::read_to_string(self.page_file(rel))?)
    }

    pub fn save_page(&mut self, rel: &str, page: &PageFile) -> Result<(), Error> {
        self.prepare_page_save(rel, &page.id)
            .write(write_page(page).as_bytes())
    }

    /// A save of a page, to be written later (maybe on another thread).
    pub fn prepare_page_save(&mut self, rel: &str, page_id: &str) -> PendingSave {
        let path = self.page_file(rel);
        self.prepare_save(page_id, path, PAGE_EXTENSION)
    }

    /// A save that keeps no history: for what can be made again, like read handwriting.
    pub fn prepare_page_save_without_history(&self, rel: &str) -> PendingSave {
        PendingSave {
            path: self.page_file(rel),
            history: None,
            policy: self.policy,
        }
    }

    fn history_dir(&self, page_id: &str) -> PathBuf {
        self.data.join("history").join(page_id)
    }

    fn snapshot_as(&self, key: &str, current: &Path, extension: &str) -> Result<(), Error> {
        snapshot_into(&self.history_dir(key), current, extension, &self.policy)
    }

    pub fn history(&self, page_id: &str) -> Result<Vec<PathBuf>, Error> {
        history_files(&self.history_dir(page_id))
    }
}

/// A file save the library has prepared; writing it needs no library, so it can run elsewhere.
pub struct PendingSave {
    path: PathBuf,
    /// Where the file as it is goes first, and as what.
    history: Option<(PathBuf, &'static str)>,
    policy: HistoryPolicy,
}

impl PendingSave {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn write(&self, bytes: &[u8]) -> Result<(), Error> {
        if let Some((dir, extension)) = &self.history {
            snapshot_into(dir, &self.path, extension, &self.policy)?;
        }
        write_atomic(&self.path, bytes)
    }
}

fn snapshot_into(
    dir: &Path,
    current: &Path,
    extension: &str,
    policy: &HistoryPolicy,
) -> Result<(), Error> {
    fs::create_dir_all(dir)?;
    let mut stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    // Two snapshots in one millisecond would overwrite each other.
    while dir.join(format!("{stamp}.{extension}")).exists() {
        stamp += 1;
    }
    fs::copy(current, dir.join(format!("{stamp}.{extension}")))?;
    let size = |p: &PathBuf| fs::metadata(p).map_or(0, |m| m.len());
    let mut versions = history_files(dir)?;
    let mut total: u64 = versions.iter().map(size).sum();
    while versions.len() > policy.keep
        || (total > policy.max_bytes && versions.len() > MIN_VERSIONS)
    {
        let oldest = versions.remove(0);
        total -= size(&oldest);
        fs::remove_file(oldest)?;
    }
    Ok(())
}

fn history_files(dir: &Path) -> Result<Vec<PathBuf>, Error> {
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

mod trash;
mod versions;
pub use trash::TrashEntry;
pub use versions::Version;

#[cfg(test)]
mod tests;
