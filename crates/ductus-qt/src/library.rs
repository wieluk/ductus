use crate::engine::CanvasEngine;
use crate::ffi::qobject::{ImportResult, SavedPage, TrashRow, TreeRow};
use crate::import::ImportHandle;
use crate::recognize::RecognitionHandle;
use crate::saver::Saver;
use ductus_core::{Document, PageKind, PageSettings, Rgba};
use ductus_format::library::pdf_stem;
use ductus_format::{Filter, Library, Node, NodeKind, PageFile};
use ductus_pdf::{PdfFile, PdfPainter};
use std::collections::HashSet;
use std::path::Path;
use std::rc::Rc;

enum OpenKind {
    Svg {
        id: String,
        pdf: Option<String>,
        recognized: Vec<ductus_format::RecognizedLine>,
    },
    Pdf,
}

struct OpenPage {
    path: String,
    kind: OpenKind,
}

struct TreeView<'a> {
    filter: Filter,
    expanded: &'a HashSet<&'a str>,
    pinned: &'a HashSet<String>,
    keep: &'a str,
    /// Inside a notebook, folders that are notebooks themselves are listed on their own.
    sections_only: bool,
}

/// Recent pages shown above the notebooks.
const RECENT_SHOWN: usize = 5;

pub struct LibraryHandle {
    /// Through `lib()`, which waits for pending saves, unless nothing written can matter.
    library: Library,
    saver: Saver,
    open: Option<OpenPage>,
    /// The page in the other half of a side-by-side view; `swap_open` makes it the open one.
    parked: Option<OpenPage>,
}

type Result<T> = std::result::Result<T, ductus_format::Error>;

fn pdf_error(e: ductus_pdf::Error) -> ductus_format::Error {
    ductus_format::Error::Invalid(e.to_string())
}

pub fn open_library(root: &str, data_dir: &str) -> Result<Box<LibraryHandle>> {
    Ok(Box::new(LibraryHandle {
        library: Library::open(root, Path::new(data_dir))?,
        saver: Saver::new(),
        open: None,
        parked: None,
    }))
}

pub fn relocate_library(from: &str, to: &str, data_dir: &str) -> Result<()> {
    ductus_format::relocate_library(Path::new(from), Path::new(to), Path::new(data_dir))
}

pub fn merge_pdf_files(files: Vec<String>, target: &str) -> Result<()> {
    let parts = files
        .iter()
        .map(std::fs::read)
        .collect::<std::io::Result<Vec<_>>>()?;
    let bytes = ductus_pdf::merge(&parts).map_err(pdf_error)?;
    ductus_format::library::write_atomic(Path::new(target), &bytes)
}

fn hex(c: Rgba) -> String {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", b(c.r), b(c.g), b(c.b))
}

fn parse_hex(s: &str) -> Option<Rgba> {
    let s = s.strip_prefix('#')?;
    let c = |i: usize| {
        u8::from_str_radix(s.get(i..i + 2)?, 16)
            .ok()
            .map(|v| f32::from(v) / 255.0)
    };
    Some(Rgba::rgb(c(0)?, c(2)?, c(4)?))
}

fn filter(flags: u8) -> Filter {
    Filter {
        pages: flags & 1 != 0,
        pdfs: flags & 2 != 0,
        other_files: flags & 4 != 0,
        hide_empty_folders: flags & 8 != 0,
    }
}

fn row_open(rows: &[TreeRow]) -> bool {
    rows.last().is_some_and(|r| r.expanded)
}

fn parent_of(rel: &str) -> &str {
    rel.rsplit_once('/').map_or("", |(parent, _)| parent)
}

/// Where a notebook is on disk, down to its own folder when that is named differently.
fn notebook_location<'a>(folder: &'a str, name: &str) -> &'a str {
    match folder.rsplit_once('/') {
        Some((parent, own)) if own == name => parent,
        None if folder == name => "",
        _ => folder,
    }
}

fn relative<'a>(path: &'a str, folder: &str) -> &'a str {
    if folder.is_empty() {
        return path;
    }
    match path.strip_prefix(folder) {
        Some("") => "",
        Some(rest) => rest.strip_prefix('/').unwrap_or(path),
        None => path,
    }
}

fn is_pdf_path(rel: &str) -> bool {
    pdf_stem(rel.rsplit('/').next().unwrap_or(rel)).is_some()
}

fn moved(path: &str, from: &str, to: &str) -> Option<String> {
    if path == from {
        Some(to.to_owned())
    } else {
        path.strip_prefix(&format!("{from}/"))
            .map(|rest| format!("{to}/{rest}"))
    }
}

fn open_pdf(bytes: Vec<u8>) -> Result<Rc<PdfFile>> {
    Ok(Rc::new(PdfFile::open(bytes).map_err(pdf_error)?))
}

impl LibraryHandle {
    pub fn root(&self) -> String {
        self.library.root().to_string_lossy().into_owned()
    }

    pub fn data_dir(&self) -> String {
        self.library.data_dir().to_string_lossy().into_owned()
    }

    pub fn is_empty(&self) -> Result<bool> {
        Ok(self.library.list("")?.is_empty())
    }

    pub fn rows(
        &self,
        expanded: &str,
        flags: u8,
        notebooks: bool,
        focus: &str,
        keep: &str,
    ) -> Result<Vec<TreeRow>> {
        let expanded: HashSet<&str> = expanded.lines().collect();
        let filter = filter(flags);
        let pinned: HashSet<String> = self.library.favorites().into_iter().collect();
        let mut out = Vec::new();
        let mut view = TreeView {
            filter,
            expanded: &expanded,
            pinned: &pinned,
            keep,
            sections_only: notebooks,
        };
        if !notebooks {
            let color = self.inherited_color(focus);
            self.tree_rows(focus, 0, color, &view, &mut out);
            return Ok(out);
        }
        // Narrowed to one notebook: its sections and pages, then notebooks kept inside it.
        let single = !focus.is_empty();
        if single {
            let color = self.inherited_color(focus);
            self.notebook_rows(focus, 0, color, &view, &mut out);
        } else {
            let recent = self.library.recent().into_iter().take(RECENT_SHOWN);
            for (section, path) in self
                .library
                .favorites()
                .into_iter()
                .map(|p| ("favorites", p))
                .chain(recent.map(|p| ("recent", p)))
            {
                if let Some(node) = self.node(&path) {
                    let mut row = self.row(&node, 0, self.inherited_color(&path), &pinned);
                    row.quick = true;
                    row.section = section.into();
                    row.subtitle = self.place(&path);
                    out.push(row);
                }
            }
        }
        view.keep = "";
        for folder in self.library.notebooks(focus) {
            if folder == focus {
                continue;
            }
            let Some(node) = self.node(&folder) else {
                continue;
            };
            let color = self.inherited_color(&folder);
            let mut row = self.row(&node, 0, color, &pinned);
            row.section = "notebooks".into();
            row.is_notebook = true;
            if let Some(name) = self.library.notebook_name(&folder) {
                row.name = name;
            }
            row.subtitle =
                relative(notebook_location(&folder, &row.name), focus).replace('/', " › ");
            row.has_children = true;
            row.expanded = expanded.contains(folder.as_str());
            out.push(row);
            if row_open(&out) {
                self.notebook_rows(&folder, 1, color, &view, &mut out);
            }
        }
        // One notebook needs no headings.
        if single {
            for row in &mut out {
                row.section.clear();
            }
        }
        Ok(out)
    }

    /// A notebook's sections and pages, then rows that add to it.
    fn notebook_rows(
        &self,
        notebook: &str,
        depth: u32,
        color: Option<Rgba>,
        view: &TreeView,
        out: &mut Vec<TreeRow>,
    ) {
        let before = out.len();
        self.tree_rows(notebook, depth, color, view, out);
        let adders: &[&str] = if out.len() == before {
            &["page", "section"]
        } else {
            &["section"]
        };
        for adder in adders {
            out.push(TreeRow {
                depth,
                path: notebook.to_owned(),
                section: "notebooks".into(),
                placeholder: (*adder).into(),
                ..TreeRow::default()
            });
        }
    }

    fn tree_rows(
        &self,
        folder: &str,
        depth: u32,
        inherited: Option<Rgba>,
        view: &TreeView,
        out: &mut Vec<TreeRow>,
    ) {
        let Ok(nodes) = self.library.list(folder) else {
            return;
        };
        for node in nodes.iter() {
            if !self.library.is_shown(node, view.filter)
                && moved(view.keep, &node.path, "").is_none()
            {
                continue;
            }
            if view.sections_only
                && node.kind == NodeKind::Folder
                && self.library.is_notebook(&node.path)
            {
                continue;
            }
            let color = match node.kind {
                NodeKind::Folder => self.library.folder_color(&node.path).or(inherited),
                _ => inherited,
            };
            let mut row = self.row(node, depth, color, view.pinned);
            if view.sections_only {
                row.section = "notebooks".into();
            }
            if node.kind == NodeKind::Folder {
                row.has_children = self.library.has_visible(&node.path, view.filter);
                row.expanded = view.expanded.contains(node.path.as_str());
                if let Some(name) = self.library.notebook_name(&node.path) {
                    row.is_notebook = true;
                    row.notebook_name = name;
                } else if view.sections_only {
                    row.kind = if self.holds_sections(&node.path, view.filter) {
                        "group"
                    } else {
                        "section"
                    }
                    .into();
                }
            }
            let open = row.expanded;
            out.push(row);
            if open && depth < 64 {
                self.tree_rows(&node.path, depth + 1, color, view, out);
            }
        }
    }

    fn holds_sections(&self, folder: &str, filter: Filter) -> bool {
        self.library.list(folder).is_ok_and(|nodes| {
            nodes.iter().any(|n| {
                n.kind == NodeKind::Folder
                    && self.library.is_shown(n, filter)
                    && !self.library.is_notebook(&n.path)
            })
        })
    }

    fn row(
        &self,
        node: &Node,
        depth: u32,
        color: Option<Rgba>,
        pinned: &HashSet<String>,
    ) -> TreeRow {
        TreeRow {
            depth,
            is_folder: node.kind == NodeKind::Folder,
            is_pdf: node.kind == NodeKind::Pdf,
            is_file: node.kind == NodeKind::File,
            name: node.name.clone(),
            path: node.path.clone(),
            color: color.map(hex).unwrap_or_default(),
            conflict: node.conflict,
            pinned: pinned.contains(&node.path),
            ..TreeRow::default()
        }
    }

    fn node(&self, rel: &str) -> Option<Node> {
        let nodes = self.library.list(parent_of(rel)).ok()?;
        nodes.iter().find(|n| n.path == rel).cloned()
    }

    fn inherited_color(&self, rel: &str) -> Option<Rgba> {
        let mut folder = rel;
        while !folder.is_empty() {
            if let Some(c) = self.library.folder_color(folder) {
                return Some(c);
            }
            folder = parent_of(folder);
        }
        None
    }

    pub fn set_pinned(&self, path: &str, pinned: bool) -> Result<()> {
        self.library.set_pinned(path, pinned)
    }

    pub fn move_notebook(&self, path: &str, delta: i32, focus: &str) -> Result<()> {
        self.library.move_notebook(path, delta, focus)
    }

    pub fn set_notebook(&self, folder: &str, name: &str) -> Result<()> {
        let name = name.trim();
        self.library
            .set_notebook(folder, (!name.is_empty()).then_some(name))
    }

    pub fn notebook_name(&self, folder: &str) -> String {
        self.library.notebook_name(folder).unwrap_or_default()
    }

    pub fn notebook_of(&self, path: &str) -> String {
        self.library.notebook_of(path).unwrap_or_default()
    }

    pub fn add_recent(&self, page: &str) -> Result<()> {
        self.library.add_recent(page)
    }

    pub fn clear_recent(&self) -> Result<()> {
        self.library.clear_recent()
    }

    pub fn page_targets(&self) -> String {
        let mut lines = Vec::new();
        for notebook in self.library.notebooks("") {
            let name = self.notebook_name(&notebook);
            lines.push(format!("{notebook}\t{name}"));
            self.section_targets(&notebook, &name, &mut lines);
        }
        lines.join("\n")
    }

    fn section_targets(&self, folder: &str, label: &str, lines: &mut Vec<String>) {
        for node in self.library.visible(folder, Filter::default()) {
            if node.kind == NodeKind::Folder && !self.library.is_notebook(&node.path) {
                let label = format!("{label} › {}", node.name);
                lines.push(format!("{}\t{label}", node.path));
                self.section_targets(&node.path, &label, lines);
            }
        }
    }

    pub fn notebook_list(&self) -> String {
        self.library
            .notebooks("")
            .into_iter()
            .map(|n| format!("{n}\t{}", self.notebook_name(&n)))
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn subfolders(&self, folder: &str) -> String {
        self.library
            .visible(folder, Filter::default())
            .into_iter()
            .filter(|n| n.kind == NodeKind::Folder)
            .map(|n| format!("{}\t{}", n.name, self.notebook_name(&n.path)))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Where a shortcut leads: the notebook's folders, then the notebook and section, which eliding keeps.
    fn place(&self, path: &str) -> String {
        let parent = parent_of(path);
        match self.library.notebook_of(path) {
            Some(notebook) if notebook != path => {
                let name = self.notebook_name(&notebook);
                let inside = match relative(parent, &notebook) {
                    "" => name.clone(),
                    section => format!("{name} › {}", section.replace('/', " › ")),
                };
                match notebook_location(&notebook, &name) {
                    "" => inside,
                    location => format!("{} · {inside}", location.replace('/', " › ")),
                }
            }
            _ => parent.replace('/', " › "),
        }
    }

    pub fn absolute_path(&self, path: &str) -> String {
        self.library.abs(path).to_string_lossy().into_owned()
    }

    pub fn import_page(&self, parent: &str, file: &str) -> Result<String> {
        self.lib().import_page(parent, Path::new(file))
    }

    pub fn unique_name(&self, parent: &str, base: &str) -> String {
        self.lib().unique_name(parent, base)
    }

    pub fn create_folder(&self, parent: &str, name: &str) -> Result<String> {
        self.lib().create_folder(parent, name)
    }

    pub fn create_page(&mut self, parent: &str, name: &str, like: &CanvasEngine) -> Result<String> {
        let mut settings = *like.canvas().page_settings();
        if settings.kind == PageKind::Pdf {
            settings = PageSettings::default();
        }
        self.lib_mut().create_page(parent, name, settings)
    }

    pub fn create_page_styled(
        &mut self,
        parent: &str,
        name: &str,
        kind: &str,
        landscape: bool,
        pattern: &str,
    ) -> Result<String> {
        let settings = crate::engine::styled(PageSettings::default(), kind, landscape, pattern);
        self.lib_mut().create_page(parent, name, settings)
    }

    pub fn rename(&mut self, path: &str, name: &str) -> Result<String> {
        let new = self.lib().rename(path, name)?;
        self.follow_move(path, &new);
        Ok(new)
    }

    pub fn move_to(&mut self, path: &str, parent: &str, index: i32) -> Result<String> {
        let index = usize::try_from(index).ok();
        let new = self.lib().move_to(path, parent, index)?;
        self.follow_move(path, &new);
        Ok(new)
    }

    fn follow_move(&mut self, from: &str, to: &str) {
        for page in [&mut self.open, &mut self.parked].into_iter().flatten() {
            if let Some(updated) = moved(&page.path, from, to) {
                page.path = updated;
            }
        }
    }

    pub fn remove(&mut self, path: &str) -> Result<String> {
        let id = self.lib().delete(path)?;
        for page in [&mut self.open, &mut self.parked] {
            if page
                .as_ref()
                .is_some_and(|p| moved(&p.path, path, "").is_some())
            {
                *page = None;
            }
        }
        Ok(id)
    }

    pub fn swap_open(&mut self) {
        std::mem::swap(&mut self.open, &mut self.parked);
    }

    pub fn parked_page(&self) -> String {
        self.parked
            .as_ref()
            .map(|o| o.path.clone())
            .unwrap_or_default()
    }

    pub fn pages_in(&self, folder: &str) -> Result<Vec<String>> {
        self.lib().pages_in(folder)
    }

    pub fn trash_rows(&self) -> Result<Vec<TrashRow>> {
        Ok(self
            .lib()
            .trash()?
            .into_iter()
            .map(|entry| {
                let (parent, name) = entry
                    .original
                    .rsplit_once('/')
                    .unwrap_or(("", entry.original.as_str()));
                TrashRow {
                    id: entry.id,
                    name: name.to_owned(),
                    location: parent.split('/').collect::<Vec<_>>().join(" \u{203a} "),
                    kind: match entry.kind {
                        NodeKind::Page => "page",
                        NodeKind::Pdf => "pdf",
                        NodeKind::Folder => "folder",
                        NodeKind::File => "file",
                    }
                    .into(),
                    deleted: entry.deleted,
                }
            })
            .collect())
    }

    pub fn restore(&mut self, id: &str) -> Result<String> {
        self.lib().restore(id)
    }

    pub fn purge(&self, id: &str) -> Result<()> {
        self.lib().purge(id)
    }

    pub fn empty_trash(&self) -> Result<()> {
        self.lib().empty_trash()
    }

    pub fn versions(&self, path: &str) -> Result<Vec<u64>> {
        Ok(self
            .lib()
            .versions(path)?
            .into_iter()
            .map(|v| v.saved)
            .collect())
    }

    pub fn restore_version(&mut self, path: &str, saved: u64) -> Result<()> {
        self.lib_mut().restore_version(path, saved)
    }

    pub fn copy_version(&self, path: &str, saved: u64, name: &str) -> Result<String> {
        self.lib().copy_version(path, saved, name)
    }

    pub fn set_color(&self, path: &str, color: &str) -> Result<()> {
        self.lib().set_color(path, parse_hex(color))
    }

    pub fn open_page(&self) -> String {
        self.open
            .as_ref()
            .map(|o| o.path.clone())
            .unwrap_or_default()
    }

    pub fn page_file(&self, path: &str) -> String {
        if is_pdf_path(path) {
            return self.library.abs(path).to_string_lossy().into_owned();
        }
        self.library.page_file(path).to_string_lossy().into_owned()
    }

    fn lib(&self) -> &Library {
        self.saver.wait();
        &self.library
    }

    fn lib_mut(&mut self) -> &mut Library {
        self.saver.wait();
        &mut self.library
    }

    /// Saves finished since last asked.
    pub fn take_saved(&self) -> Vec<SavedPage> {
        self.saver
            .take_done()
            .into_iter()
            .map(|(path, error)| SavedPage { path, error })
            .collect()
    }

    pub fn saving(&self) -> bool {
        self.saver.busy()
    }

    pub fn wait_saved(&self) {
        self.saver.wait();
    }

    pub fn load_into(&mut self, path: &str, canvas: &mut CanvasEngine) -> Result<()> {
        // Opening another page needn't wait for the one just left to be written.
        self.saver.wait_for(path);
        if is_pdf_path(path) {
            let bytes = self.library.read_pdf(path)?;
            let ink = ductus_pdf::read_ink(&bytes).map_err(pdf_error)?;
            let placed = ductus_pdf::read_objects(&bytes).map_err(pdf_error)?;
            // pdfium shows the PDF without our ink and objects, which the canvas draws.
            let display = ductus_pdf::strip_ductus_annotations(&bytes).map_err(pdf_error)?;
            let file = open_pdf(display)?;
            let sizes = file.page_sizes().to_vec();
            let strokes = ductus_pdf::document_strokes(&sizes, ink);
            let (mut texts, mut maths, mut images) = (Vec::new(), Vec::new(), Vec::new());
            for object in ductus_pdf::document_objects(&sizes, placed) {
                match object {
                    ductus_pdf::PageObject::Text(t) => texts.push(t),
                    ductus_pdf::PageObject::Math(m) => maths.push(m),
                    ductus_pdf::PageObject::Picture(image, png) => {
                        canvas.canvas_mut().add_picture(&image.source, png);
                        images.push(image);
                    }
                }
            }
            let mut doc = Document::with_contents(
                PageSettings {
                    kind: PageKind::Pdf,
                    ..PageSettings::default()
                },
                strokes,
            )
            .with_texts(texts)
            .with_maths(maths)
            .with_images(images);
            doc.set_pdf_sheets(sizes);
            let c = canvas.canvas_mut();
            c.set_sheet_painter(Some(Box::new(PdfPainter(file))));
            c.load_document(doc);
            self.open = Some(OpenPage {
                path: path.to_owned(),
                kind: OpenKind::Pdf,
            });
            // Parsing the PDF and dropping the previous page freed a lot.
            ductus_core::release_freed_memory();
            return Ok(());
        }

        let mut page = self.library.load_page(path)?;
        // Pictures must be known before the page is shown; missing ones stay blank.
        for (name, bytes) in std::mem::take(&mut page.pictures) {
            canvas.canvas_mut().add_picture(&name, bytes);
        }
        let painter: Option<Box<dyn ductus_render::SheetPainter>> = match &page.pdf {
            Some(name) => {
                let file = open_pdf(self.library.read_page_pdf(path, name)?)?;
                let sizes = file.page_sizes().to_vec();
                // The PDF is authoritative for its page sizes.
                page.document.set_pdf_sheets(sizes);
                Some(Box::new(PdfPainter(file)))
            }
            None => None,
        };
        let c = canvas.canvas_mut();
        c.set_sheet_painter(painter);
        c.load_document(page.document);
        self.open = Some(OpenPage {
            path: path.to_owned(),
            kind: OpenKind::Svg {
                id: page.id,
                pdf: page.pdf,
                recognized: page.recognized,
            },
        });
        ductus_core::release_freed_memory();
        Ok(())
    }

    /// Takes what the page holds now and writes it on the saving thread (`take_saved` reports it).
    pub fn save_from(&mut self, canvas: &CanvasEngine) -> Result<()> {
        let Some(open) = &self.open else {
            return Ok(());
        };
        let doc = canvas.canvas().document();
        let path = open.path.clone();
        match &open.kind {
            OpenKind::Svg {
                id,
                pdf,
                recognized,
            } => {
                let mut page = PageFile {
                    id: id.clone(),
                    document: doc.clone(),
                    pdf: pdf.clone(),
                    recognized: recognized.clone(),
                    pictures: doc
                        .images()
                        .iter()
                        .filter_map(|i| {
                            let bytes = canvas.canvas().picture_bytes(&i.source)?;
                            Some((i.source.clone(), bytes.to_vec()))
                        })
                        .collect(),
                };
                let save = self.library.prepare_page_save(&path, id);
                self.saver.queue(&path, move || {
                    // Text read from ink that has changed since is dropped.
                    if !page.recognized.is_empty() {
                        let current: HashSet<String> =
                            ductus_hwr::find_lines(page.document.strokes())
                                .into_iter()
                                .map(|l| l.key)
                                .collect();
                        page.recognized.retain(|l| current.contains(&l.key));
                    }
                    save.write(ductus_format::write_page(&page).as_bytes())
                });
                Ok(())
            }
            OpenKind::Pdf => {
                let picture = |name: &str| Some(canvas.canvas().picture_bytes(name)?.to_vec());
                let objects = ductus_pdf::objects_by_page(doc, &picture);
                let ink = ductus_pdf::ink_by_page(doc);
                let save = self.library.prepare_pdf_save(&path);
                self.saver.queue(&path, move || {
                    let current = std::fs::read(save.path())?;
                    let updated = ductus_pdf::write_annotations(&current, &ink, &objects)
                        .map_err(pdf_error)?;
                    save.write(&updated)?;
                    // Rewriting the PDF parsed all of it.
                    drop((current, updated));
                    ductus_core::release_freed_memory();
                    Ok(())
                });
                Ok(())
            }
        }
    }

    /// A blank page after the PDF page at `view_y`; notes below move down with their pages.
    /// Reload the page afterwards.
    pub fn insert_blank_sheet(&mut self, canvas: &CanvasEngine, view_y: f64) -> Result<()> {
        let Some(open) = &self.open else {
            return Ok(());
        };
        let path = open.path.clone();
        let kind = match &open.kind {
            OpenKind::Pdf => None,
            OpenKind::Svg {
                pdf: Some(name), ..
            } => Some(name.clone()),
            OpenKind::Svg { pdf: None, .. } => {
                return Err(ductus_format::Error::Invalid(
                    "Only pages with a PDF get blank PDF pages.".into(),
                ));
            }
        };
        self.save_from(canvas)?;
        let doc = canvas.canvas().document();
        let sizes = doc.pdf_sheets().to_vec();
        let sheets = ductus_core::stack_sheets(&sizes);
        let after = sheets.iter().rposition(|s| s.1 <= view_y).unwrap_or(0);
        match kind {
            None => {
                let bytes = self.lib().read_pdf(&path)?;
                let updated = ductus_pdf::insert_blank_page(&bytes, after).map_err(pdf_error)?;
                self.lib_mut().save_pdf(&path, &updated)
            }
            Some(name) => {
                let bytes = self.lib().read_page_pdf(&path, &name)?;
                let updated = ductus_pdf::insert_blank_page(&bytes, after).map_err(pdf_error)?;
                let mut page = self.lib().load_page(&path)?;
                if let Some(&(_, y, _, h)) = sheets.get(after) {
                    page.document
                        .shift_below(y + h, sizes[after].1 + ductus_core::SHEET_GAP);
                }
                self.lib().save_page_pdf(&path, &name, &updated)?;
                self.lib_mut().save_page(&path, &page)
            }
        }
    }

    /// Queues the open page's lines that haven't been read yet; returns how many were added.
    pub fn queue_recognition(
        &self,
        canvas: &CanvasEngine,
        recognizer: &RecognitionHandle,
        lang: &str,
    ) -> usize {
        let Some(OpenPage {
            path,
            kind: OpenKind::Svg { recognized, .. },
        }) = &self.open
        else {
            return 0; // PDFs written on directly: not yet
        };
        let known: HashSet<&str> = recognized.iter().map(|l| l.key.as_str()).collect();
        let strokes = canvas.canvas().document().strokes();
        ductus_hwr::find_lines(strokes)
            .iter()
            .filter(|line| !known.contains(line.key.as_str()))
            .filter(|line| recognizer.submit(path, line, strokes, lang))
            .count()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn apply_recognition(
        &mut self,
        page: &str,
        key: &str,
        text: &str,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        lang: &str,
    ) -> Result<bool> {
        let line = ductus_format::RecognizedLine {
            key: key.into(),
            text: text.into(),
            rect: (x, y, w, h),
            lang: (!lang.is_empty() && lang != "auto").then(|| lang.to_owned()),
        };
        if let Some(OpenPage {
            path,
            kind: OpenKind::Svg { recognized, .. },
        }) = &mut self.open
        {
            if path == page {
                recognized.retain(|l| l.key != key);
                recognized.push(line);
                return Ok(true);
            }
        }
        // The other half was saved when it was left, so its file below is current too.
        if let Some(OpenPage {
            path,
            kind: OpenKind::Svg { recognized, .. },
        }) = &mut self.parked
        {
            if path == page {
                recognized.retain(|l| l.key != key);
                recognized.push(line.clone());
            }
        }
        // Not open (any more): update the file if that ink is still there. A cache, so no history.
        let save = self.library.prepare_page_save_without_history(page);
        let key = key.to_owned();
        self.saver.queue(page, move || {
            let mut file = ductus_format::read_page(&std::fs::read_to_string(save.path())?)?;
            let current = ductus_hwr::find_lines(file.document.strokes());
            if current.iter().any(|l| l.key == key) {
                file.recognized.retain(|l| l.key != key);
                file.recognized.push(line);
                save.write(ductus_format::write_page(&file).as_bytes())?;
            }
            Ok(())
        });
        Ok(false)
    }

    pub fn close_page(&mut self) {
        self.open = None;
    }

    pub fn finish_import(
        &mut self,
        importer: &ImportHandle,
        parent: &str,
        name: &str,
    ) -> Result<ImportResult> {
        let mut import = importer
            .take()
            .ok_or_else(|| ductus_format::Error::Invalid("Nothing was imported.".into()))?
            .map_err(ductus_format::Error::Invalid)?;
        let name = name.trim();
        if !name.is_empty() {
            match &mut import.entry {
                ductus_import::Entry::Page(page) => page.name = name.to_owned(),
                ductus_import::Entry::Folder { name: n, .. } => *n = name.to_owned(),
                ductus_import::Entry::File { .. } => {}
            }
        }
        let saved = ductus_import::save(self.lib_mut(), parent, &import)?;
        Ok(ImportResult {
            root: saved.root,
            page: saved.first_page.unwrap_or_default(),
            warnings: import.warnings.join("\n"),
        })
    }

    pub fn import_pdf(&mut self, parent: &str, file: &str, annotate: bool) -> Result<String> {
        let source = Path::new(file);
        let bytes = std::fs::read(source)?;
        let stem = source
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Imported PDF".into());
        let name = self.lib().unique_name(parent, &stem);
        // Make sure it opens before adding anything to the library.
        let sizes = open_pdf(bytes.clone())?.page_sizes().to_vec();
        if annotate {
            self.lib().add_pdf(parent, &name, &bytes)
        } else {
            self.lib_mut().create_pdf_page(parent, &name, &bytes, sizes)
        }
    }

    pub fn export_pdf(&self, target: &str, canvas: &CanvasEngine, overlay: &[u8]) -> Result<()> {
        let bytes = self.export_bytes(canvas, overlay)?;
        ductus_format::library::write_atomic(Path::new(target), &bytes)
    }

    pub fn export_areas(&self, canvas: &CanvasEngine) -> Vec<f64> {
        let doc = canvas.canvas().document();
        let areas = match &self.open {
            Some(OpenPage {
                kind: OpenKind::Svg { pdf: None, .. },
                ..
            }) => ductus_pdf::page_areas(doc),
            Some(_) => ductus_core::stack_sheets(doc.pdf_sheets())
                .into_iter()
                .map(|(x, y, w, h)| kurbo::Rect::new(x, y, x + w, y + h))
                .collect(),
            None => Vec::new(),
        };
        areas
            .iter()
            .flat_map(|r| [r.x0, r.y0, r.width(), r.height()])
            .collect()
    }

    pub fn print_job(&self, canvas: &CanvasEngine, overlay: &[u8]) -> Result<Box<PrintJob>> {
        let bytes = self.export_bytes(canvas, overlay)?;
        Ok(Box::new(PrintJob(PdfFile::open(bytes).map_err(pdf_error)?)))
    }

    fn export_bytes(&self, canvas: &CanvasEngine, overlay: &[u8]) -> Result<Vec<u8>> {
        let mut bytes = self.flattened(canvas)?;
        // Notes pages draw formulas themselves; pages over a PDF get them on top.
        if let Some(OpenPage {
            kind: OpenKind::Svg { pdf: Some(_), .. } | OpenKind::Pdf,
            ..
        }) = &self.open
        {
            let doc = canvas.canvas().document();
            let areas: Vec<kurbo::Rect> = ductus_core::stack_sheets(doc.pdf_sheets())
                .into_iter()
                .map(|(x, y, w, h)| kurbo::Rect::new(x, y, x + w, y + h))
                .collect();
            if let Some(maths) = ductus_pdf::math_overlay(doc, &areas).map_err(pdf_error)? {
                bytes = ductus_pdf::overlay_pages(&bytes, &maths).map_err(pdf_error)?;
            }
        }
        if overlay.is_empty() {
            return Ok(bytes);
        }
        ductus_pdf::overlay_pages(&bytes, overlay).map_err(pdf_error)
    }

    fn flattened(&self, canvas: &CanvasEngine) -> Result<Vec<u8>> {
        let Some(open) = &self.open else {
            return Err(ductus_format::Error::Invalid("No page is open.".into()));
        };
        let doc = canvas.canvas().document();
        let picture = |name: &str| {
            canvas
                .canvas()
                .picture_bytes(name)
                .and_then(ductus_pdf::PdfImage::from_png)
        };
        match &open.kind {
            OpenKind::Pdf => {
                let current = self.lib().read_pdf(&open.path)?;
                let pictures = ductus_pdf::pictures_by_page(doc, &picture);
                ductus_pdf::flatten(&current, &ductus_pdf::ink_by_page(doc), &pictures)
            }
            OpenKind::Svg {
                pdf: Some(name), ..
            } => {
                let source = self.lib().read_page_pdf(&open.path, name)?;
                let pictures = ductus_pdf::pictures_by_page(doc, &picture);
                ductus_pdf::flatten(&source, &ductus_pdf::ink_by_page(doc), &pictures)
            }
            OpenKind::Svg { pdf: None, .. } => ductus_pdf::export_notes(doc, &picture),
        }
        .map_err(pdf_error)
    }
}

pub struct PrintJob(PdfFile);

impl PrintJob {
    pub fn print_page_count(&self) -> usize {
        self.0.page_count()
    }

    pub fn print_page_aspect(&self, index: usize) -> f64 {
        let (w, h) = self.0.page_sizes()[index];
        w / h
    }

    pub fn print_page_pixels(&self, index: usize, width: u32, height: u32) -> Result<Vec<u8>> {
        let mut pixmap = tiny_skia::Pixmap::new(width.max(1), height.max(1)).ok_or_else(|| {
            ductus_format::Error::Invalid("The page is too large to print.".into())
        })?;
        let (w, _) = self.0.page_sizes()[index];
        self.0
            .render_into(index, &mut pixmap, (0.0, 0.0), f64::from(width) / w)
            .map_err(pdf_error)?;
        Ok(pixmap.take())
    }
}

pub fn pdf_text(path: &str) -> String {
    let Ok(file) = std::fs::read(path)
        .map_err(|_| ())
        .and_then(|b| PdfFile::open(b).map_err(|_| ()))
    else {
        return String::new();
    };
    (0..file.page_count())
        .filter_map(|i| file.page_text(i).ok())
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn pdf_support_problem() -> String {
    ductus_pdf::pdfium_status().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moved_paths_follow_renames() {
        assert_eq!(moved("A/B", "A/B", "A/C").as_deref(), Some("A/C"));
        assert_eq!(moved("A/B/p", "A/B", "X/B").as_deref(), Some("X/B/p"));
        assert_eq!(moved("A/Bee/p", "A/B", "X"), None);
    }

    #[test]
    fn rows_start_at_the_focus_folder() {
        let dir = tempfile::tempdir().unwrap();
        let mut handle = open_library(
            dir.path().join("Notes").to_str().unwrap(),
            dir.path().join("app").to_str().unwrap(),
        )
        .unwrap();
        let canvas = crate::engine::new_canvas_engine();
        handle.create_folder("", "Uni").unwrap();
        handle.set_color("Uni", "#1f4fd1").unwrap();
        handle.create_folder("Uni", "Sem 3").unwrap();
        handle.create_folder("Uni/Sem 3", "Statik").unwrap();
        handle.create_folder("Uni/Sem 3/Statik", "VO").unwrap();
        handle
            .create_page("Uni/Sem 3/Statik/VO", "Week 1", &canvas)
            .unwrap();
        handle.create_folder("", "Other").unwrap();
        handle.create_page("Other", "Note", &canvas).unwrap();

        let rows = handle
            .rows("Uni/Sem 3/Statik", 3, false, "Uni/Sem 3", "")
            .unwrap();
        let names: Vec<_> = rows.iter().map(|r| (r.depth, r.name.as_str())).collect();
        assert_eq!(names, [(0, "Statik"), (1, "VO")]);
        assert!(rows.iter().all(|r| r.color == "#1f4fd1"));

        // Only marked folders are notebooks.
        handle
            .set_notebook("Uni/Sem 3/Statik/VO", "Statik")
            .unwrap();
        handle.add_recent("Uni/Sem 3/Statik/VO/Week 1").unwrap();
        let rows = handle.rows("", 3, true, "", "").unwrap();
        let summary: Vec<_> = rows
            .iter()
            .map(|r| (r.section.as_str(), r.name.as_str(), r.subtitle.as_str()))
            .collect();
        assert_eq!(
            summary,
            [
                ("recent", "Week 1", "Uni › Sem 3 › Statik › VO · Statik"),
                ("notebooks", "Statik", "Uni › Sem 3 › Statik › VO")
            ]
        );
        // Narrowed to that notebook: its pages, without headings or shortcuts.
        let rows = handle.rows("", 3, true, "Uni/Sem 3/Statik/VO", "").unwrap();
        let summary: Vec<_> = rows
            .iter()
            .map(|r| (r.depth, r.section.as_str(), r.name.as_str()))
            .collect();
        assert_eq!(summary, [(0, "", "Week 1"), (0, "", "")]);
        assert_eq!(rows[1].placeholder, "section");
        // Folders inside a notebook are sections, or groups when they hold sections.
        handle
            .create_folder("Uni/Sem 3/Statik/VO", "Exams")
            .unwrap();
        handle
            .create_folder("Uni/Sem 3/Statik/VO/Exams", "2025")
            .unwrap();
        handle.create_folder("Uni/Sem 3/Statik/VO", "Labs").unwrap();
        let rows = handle
            .rows(
                "Uni/Sem 3/Statik/VO/Exams",
                3,
                true,
                "Uni/Sem 3/Statik/VO",
                "",
            )
            .unwrap();
        let kinds: Vec<_> = rows
            .iter()
            .map(|r| (r.name.as_str(), r.kind.as_str(), r.placeholder.as_str()))
            .collect();
        assert_eq!(
            kinds,
            [
                ("Week 1", "", ""),
                ("Exams", "group", ""),
                ("2025", "section", ""),
                ("Labs", "section", ""),
                ("", "", "section")
            ]
        );
        assert_eq!(relative("Uni/Sem 3", "Uni/Sem 3"), "");
        assert_eq!(relative("Uni/Sem 30/x", "Uni/Sem 3"), "Uni/Sem 30/x");
    }

    #[test]
    fn open_page_survives_rename_and_saves() {
        let dir = tempfile::tempdir().unwrap();
        let mut handle = open_library(
            dir.path().join("Notes").to_str().unwrap(),
            dir.path().join("app").to_str().unwrap(),
        )
        .unwrap();
        let mut canvas = crate::engine::new_canvas_engine();
        handle.create_folder("", "Math").unwrap();
        let page = handle.create_page("Math", "Limits", &canvas).unwrap();
        handle.load_into(&page, &mut canvas).unwrap();
        handle.rename("Math", "Mathematics").unwrap();
        assert_eq!(handle.open_page(), "Mathematics/Limits");
        handle.save_from(&canvas).unwrap();
        assert!(dir.path().join("Notes/Mathematics/Limits.svg").is_file());
        handle.remove("Mathematics").unwrap();
        assert_eq!(handle.open_page(), "");
    }

    #[test]
    fn moving_files_waits_for_saves_being_written() {
        let dir = tempfile::tempdir().unwrap();
        let mut handle = open_library(
            dir.path().join("Notes").to_str().unwrap(),
            dir.path().join("app").to_str().unwrap(),
        )
        .unwrap();
        let mut canvas = crate::engine::new_canvas_engine();
        handle.create_folder("", "Math").unwrap();
        let page = handle.create_page("Math", "Limits", &canvas).unwrap();
        handle.load_into(&page, &mut canvas).unwrap();
        handle.save_from(&canvas).unwrap();
        handle.rename("Math", "Calculus").unwrap();
        // The write landed before the move, not at the old place after it.
        assert!(!dir.path().join("Notes/Math").exists());
        assert!(dir.path().join("Notes/Calculus/Limits.svg").is_file());
        let saved = handle.take_saved();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].path, "Math/Limits");
        assert_eq!(saved[0].error, "");
        assert!(!handle.saving());
    }

    #[test]
    fn the_other_half_of_a_split_follows_renames_and_deletes() {
        let dir = tempfile::tempdir().unwrap();
        let mut handle = open_library(
            dir.path().join("Notes").to_str().unwrap(),
            dir.path().join("app").to_str().unwrap(),
        )
        .unwrap();
        let mut canvas = crate::engine::new_canvas_engine();
        handle.create_folder("", "Math").unwrap();
        let a = handle.create_page("Math", "A", &canvas).unwrap();
        let b = handle.create_page("Math", "B", &canvas).unwrap();
        handle.load_into(&a, &mut canvas).unwrap();
        handle.swap_open();
        handle.load_into(&b, &mut canvas).unwrap();
        handle.rename("Math", "Maths").unwrap();
        assert_eq!(handle.parked_page(), "Maths/A");
        handle.swap_open();
        assert_eq!(handle.open_page(), "Maths/A");
        handle.remove("Maths/B").unwrap();
        assert_eq!(handle.parked_page(), "");
    }
}
