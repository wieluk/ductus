use crate::engine::CanvasEngine;
use crate::ffi::qobject::{LineJob, TrashRow, TreeRow};
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
    library: Library,
    open: Option<OpenPage>,
}

type Result<T> = std::result::Result<T, ductus_format::Error>;

fn pdf_error(e: ductus_pdf::Error) -> ductus_format::Error {
    ductus_format::Error::Invalid(e.to_string())
}

pub fn open_library(root: &str, data_dir: &str) -> Result<Box<LibraryHandle>> {
    Ok(Box::new(LibraryHandle {
        library: Library::open(root, Path::new(data_dir))?,
        open: None,
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
        view.keep = "";
        for folder in self.library.notebooks(focus) {
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
            // Where it is on disk, down to its own folder when that is named differently.
            let location = if row.name == node.name {
                parent_of(&folder)
            } else {
                &folder
            };
            row.subtitle = relative(location, focus).replace('/', " › ");
            row.has_children = true;
            row.expanded = expanded.contains(folder.as_str());
            out.push(row);
            if row_open(&out) {
                let before = out.len();
                self.tree_rows(&folder, 1, color, &view, &mut out);
                if out.len() == before {
                    out.push(TreeRow {
                        depth: 1,
                        path: folder.clone(),
                        section: "notebooks".into(),
                        placeholder: true,
                        ..TreeRow::default()
                    });
                }
            }
        }
        Ok(out)
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
                }
            }
            let open = row.expanded;
            out.push(row);
            if open && depth < 64 {
                self.tree_rows(&node.path, depth + 1, color, view, out);
            }
        }
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

    pub fn subfolders(&self, folder: &str) -> String {
        self.library
            .visible(folder, Filter::default())
            .into_iter()
            .filter(|n| n.kind == NodeKind::Folder)
            .map(|n| format!("{}\t{}", n.name, self.notebook_name(&n.path)))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Notebook and section a shortcut leads to, else its folders.
    fn place(&self, path: &str) -> String {
        let parent = parent_of(path);
        match self.library.notebook_of(path) {
            Some(notebook) if notebook != path => {
                let name = self.notebook_name(&notebook);
                match relative(parent, &notebook) {
                    "" => name,
                    section => format!("{name} › {}", section.replace('/', " › ")),
                }
            }
            _ => parent.replace('/', " › "),
        }
    }

    pub fn absolute_path(&self, path: &str) -> String {
        self.library.abs(path).to_string_lossy().into_owned()
    }

    pub fn import_page(&self, parent: &str, file: &str) -> Result<String> {
        self.library.import_page(parent, Path::new(file))
    }

    pub fn unique_name(&self, parent: &str, base: &str) -> String {
        self.library.unique_name(parent, base)
    }

    pub fn create_folder(&self, parent: &str, name: &str) -> Result<String> {
        self.library.create_folder(parent, name)
    }

    pub fn create_page(&mut self, parent: &str, name: &str, like: &CanvasEngine) -> Result<String> {
        let mut settings = *like.canvas().page_settings();
        if settings.kind == PageKind::Pdf {
            settings = PageSettings::default();
        }
        self.library.create_page(parent, name, settings)
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
        self.library.create_page(parent, name, settings)
    }

    pub fn rename(&mut self, path: &str, name: &str) -> Result<String> {
        let new = self.library.rename(path, name)?;
        self.follow_move(path, &new);
        Ok(new)
    }

    pub fn move_to(&mut self, path: &str, parent: &str, index: i32) -> Result<String> {
        let index = usize::try_from(index).ok();
        let new = self.library.move_to(path, parent, index)?;
        self.follow_move(path, &new);
        Ok(new)
    }

    fn follow_move(&mut self, from: &str, to: &str) {
        if let Some(open) = &mut self.open {
            if let Some(updated) = moved(&open.path, from, to) {
                open.path = updated;
            }
        }
    }

    pub fn remove(&mut self, path: &str) -> Result<String> {
        let id = self.library.delete(path)?;
        if self
            .open
            .as_ref()
            .is_some_and(|open| moved(&open.path, path, "").is_some())
        {
            self.open = None;
        }
        Ok(id)
    }

    pub fn pages_in(&self, folder: &str) -> Result<Vec<String>> {
        self.library.pages_in(folder)
    }

    pub fn trash_rows(&self) -> Result<Vec<TrashRow>> {
        Ok(self
            .library
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
        self.library.restore(id)
    }

    pub fn purge(&self, id: &str) -> Result<()> {
        self.library.purge(id)
    }

    pub fn empty_trash(&self) -> Result<()> {
        self.library.empty_trash()
    }

    pub fn versions(&self, path: &str) -> Result<Vec<u64>> {
        Ok(self
            .library
            .versions(path)?
            .into_iter()
            .map(|v| v.saved)
            .collect())
    }

    pub fn restore_version(&mut self, path: &str, saved: u64) -> Result<()> {
        self.library.restore_version(path, saved)
    }

    pub fn copy_version(&self, path: &str, saved: u64, name: &str) -> Result<String> {
        self.library.copy_version(path, saved, name)
    }

    pub fn set_color(&self, path: &str, color: &str) -> Result<()> {
        self.library.set_color(path, parse_hex(color))
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
        let mut p = self.library.abs(path);
        p.set_extension(ductus_format::library::PAGE_EXTENSION);
        p.to_string_lossy().into_owned()
    }

    pub fn load_into(&mut self, path: &str, canvas: &mut CanvasEngine) -> Result<()> {
        if is_pdf_path(path) {
            let bytes = self.library.read_pdf(path)?;
            let ink = ductus_pdf::read_ink(&bytes).map_err(pdf_error)?;
            // pdfium shows the PDF without our ink, which the canvas draws.
            let display = ductus_pdf::strip_ductus_annotations(&bytes).map_err(pdf_error)?;
            let file = open_pdf(display)?;
            let sizes = file.page_sizes().to_vec();
            let strokes = ductus_pdf::document_strokes(&sizes, ink);
            let mut doc = Document::with_contents(
                PageSettings {
                    kind: PageKind::Pdf,
                    ..PageSettings::default()
                },
                strokes,
            );
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
        for image in page.document.images() {
            if let Ok(bytes) = self.library.read_asset(path, &image.source) {
                canvas.canvas_mut().add_picture(&image.source, bytes);
            }
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
                // Text read from ink that has changed since is dropped.
                let current: HashSet<String> = ductus_hwr::find_lines(doc.strokes())
                    .into_iter()
                    .map(|l| l.key)
                    .collect();
                let page = PageFile {
                    id: id.clone(),
                    document: doc.clone(),
                    pdf: pdf.clone(),
                    recognized: recognized
                        .iter()
                        .filter(|l| current.contains(&l.key))
                        .cloned()
                        .collect(),
                };
                for image in doc.images() {
                    if let Some(bytes) = canvas.canvas().picture_bytes(&image.source) {
                        self.library.save_asset(&path, &image.source, bytes)?;
                    }
                }
                self.library.save_page(&path, &page)
            }
            OpenKind::Pdf => {
                let current = self.library.read_pdf(&path)?;
                let updated = ductus_pdf::write_ink(&current, &ductus_pdf::ink_by_page(doc))
                    .map_err(pdf_error)?;
                self.library.save_pdf(&path, &updated)?;
                // Rewriting the PDF parsed all of it.
                drop((current, updated));
                ductus_core::release_freed_memory();
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
                let bytes = self.library.read_pdf(&path)?;
                let updated = ductus_pdf::insert_blank_page(&bytes, after).map_err(pdf_error)?;
                self.library.save_pdf(&path, &updated)
            }
            Some(name) => {
                let bytes = self.library.read_page_pdf(&path, &name)?;
                let updated = ductus_pdf::insert_blank_page(&bytes, after).map_err(pdf_error)?;
                let mut page = self.library.load_page(&path)?;
                if let Some(&(_, y, _, h)) = sheets.get(after) {
                    page.document
                        .shift_below(y + h, sizes[after].1 + ductus_core::SHEET_GAP);
                }
                self.library.save_page_pdf(&path, &name, &updated)?;
                self.library.save_page(&path, &page)
            }
        }
    }

    /// PDFs written on directly can't hold pictures yet.
    pub fn accepts_pictures(&self) -> bool {
        matches!(
            self.open,
            Some(OpenPage {
                kind: OpenKind::Svg { .. },
                ..
            })
        )
    }

    pub fn recognition_jobs(&self, canvas: &CanvasEngine) -> Vec<LineJob> {
        let Some(OpenPage {
            kind: OpenKind::Svg { recognized, .. },
            ..
        }) = &self.open
        else {
            return Vec::new(); // PDFs written on directly: not yet
        };
        let known: HashSet<&str> = recognized.iter().map(|l| l.key.as_str()).collect();
        let strokes = canvas.canvas().document().strokes();
        ductus_hwr::find_lines(strokes)
            .into_iter()
            .filter(|line| !known.contains(line.key.as_str()))
            .filter_map(|line| {
                Some(LineJob {
                    png: ductus_hwr::line_png(strokes, &line, 96)?,
                    key: line.key,
                    x: line.rect.x0,
                    y: line.rect.y0,
                    w: line.rect.width(),
                    h: line.rect.height(),
                })
            })
            .collect()
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
        // Not open (any more): update the file if that ink is still there.
        let mut file = self.library.load_page(page)?;
        let current = ductus_hwr::find_lines(file.document.strokes());
        if current.iter().any(|l| l.key == key) {
            file.recognized.retain(|l| l.key != key);
            file.recognized.push(line);
            self.library.save_page(page, &file)?;
        }
        Ok(false)
    }

    pub fn close_page(&mut self) {
        self.open = None;
    }

    pub fn import_pdf(&mut self, parent: &str, file: &str, annotate: bool) -> Result<String> {
        let source = Path::new(file);
        let bytes = std::fs::read(source)?;
        let stem = source
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Imported PDF".into());
        let name = self.library.unique_name(parent, &stem);
        // Make sure it opens before adding anything to the library.
        let sizes = open_pdf(bytes.clone())?.page_sizes().to_vec();
        if annotate {
            self.library.add_pdf(parent, &name, &bytes)
        } else {
            self.library.create_pdf_page(parent, &name, &bytes, sizes)
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
            kind: OpenKind::Svg { pdf: Some(_), .. },
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
                let current = self.library.read_pdf(&open.path)?;
                ductus_pdf::flatten(&current, &ductus_pdf::ink_by_page(doc), &[])
            }
            OpenKind::Svg {
                pdf: Some(name), ..
            } => {
                let source = self.library.read_page_pdf(&open.path, name)?;
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
        let rows = handle.rows("", 3, true, "Uni/Sem 3", "").unwrap();
        let summary: Vec<_> = rows
            .iter()
            .map(|r| (r.section.as_str(), r.name.as_str(), r.subtitle.as_str()))
            .collect();
        assert_eq!(
            summary,
            [
                ("recent", "Week 1", "Statik"),
                ("notebooks", "Statik", "Statik › VO")
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
}
