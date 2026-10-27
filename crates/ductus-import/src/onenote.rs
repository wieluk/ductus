//! OneNote sections (.one), notebooks (.onetoc2) and notebook packages (.onepkg).
//! Pages become endless Ductus pages. OneNote places things in half inches and
//! draws ink in hundredths of a millimetre.

mod math;

use crate::{Entry, Import, ImportedPage, escape_inline, guard_line_start, picture};
use ductus_core::{
    Document, Image, InkPoint, LinePattern, MathBox, PageBackground, PageKind, PageSettings,
    PatternStyle, Rgba, Stroke, StrokeId, StrokeStyle, TextBox, Tool,
};
use ductus_format::Error;
use onenote_parser::contents::{
    Content, EmbeddedFile, EmbeddedObject, Image as OneImage, Ink, InkStroke, MathInlineObject,
    Outline, OutlineElement, OutlineItem, ParagraphStyling, RichText, Table,
};
use onenote_parser::notebook::Notebook;
use onenote_parser::page::{Page, PageContent};
use onenote_parser::section::{Section, SectionEntry};
use std::io::Read as _;
use std::path::Path;

const HALF_INCH: f64 = 48.0;
/// Ductus units per ink unit (0.01 mm).
const INK_UNIT: f64 = 96.0 / 2540.0;
/// OneNote's default body text, Calibri 11 pt.
const BODY_PT: f64 = 11.0;
const POINT: f64 = 96.0 / 72.0;
/// Room between a text block and what follows it.
const GAP: f64 = 8.0;
/// Text boxes without a width of their own wrap at most this wide.
const MAX_WIDTH: f64 = 640.0;

pub(crate) fn read(path: &Path) -> Result<Import, Error> {
    // The parser finds a notebook's sections relative to an absolute path.
    let path = &std::fs::canonicalize(path)?;
    let parser = onenote_parser::Parser::new();
    let typed = typed(path)?;
    let stem = path
        .file_stem()
        .map_or_else(|| "OneNote".into(), |s| s.to_string_lossy().into_owned());
    let mut warnings = Warnings::default();
    let entry = match crate::extension(path).as_deref() {
        Some("one") => {
            let section = parser.parse_section(typed.to_path()).map_err(unreadable)?;
            section_entry(&section, &mut warnings)
        }
        Some("onepkg") => {
            let notebook = parser.parse_package(typed.to_path()).map_err(unreadable)?;
            notebook_entry(&notebook, stem, &mut warnings)
        }
        _ => {
            // A notebook's sections are separate files, which a sandbox may not let us read.
            let notebook = parser.parse_notebook(typed.to_path()).map_err(|e| {
                Error::Invalid(format!(
                    "The OneNote notebook can't be read ({e}). Ductus needs the whole notebook \
                     folder: if it's outside your Documents folder, move it there, or export \
                     the notebook from OneNote as a single .onepkg file and import that."
                ))
            })?;
            // The table of contents is named "Open Notebook"; its folder names the notebook.
            let name = path
                .parent()
                .and_then(Path::file_name)
                .map_or(stem, |n| n.to_string_lossy().into_owned());
            notebook_entry(&notebook, name, &mut warnings)
        }
    };
    Ok(Import {
        entry,
        warnings: warnings.into_messages(),
    })
}

fn typed(path: &Path) -> Result<typed_path::TypedPathBuf, Error> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt as _;
        Ok(typed_path::TypedPathBuf::from_unix(
            path.as_os_str().as_bytes(),
        ))
    }
    #[cfg(not(unix))]
    {
        let text = path
            .to_str()
            .ok_or_else(|| Error::Invalid("The file's path can't be read.".into()))?;
        Ok(typed_path::TypedPathBuf::from_windows(text))
    }
}

fn unreadable(e: onenote_parser::errors::Error) -> Error {
    Error::Invalid(format!("The OneNote file can't be read: {e}"))
}

#[derive(Default)]
struct Warnings {
    skipped: usize,
    pictures: usize,
}

impl Warnings {
    fn into_messages(self) -> Vec<String> {
        let mut out = Vec::new();
        if self.pictures > 0 {
            out.push(format!(
                "{} picture(s) are in a format Ductus can't show and were left out.",
                self.pictures
            ));
        }
        if self.skipped > 0 {
            out.push(format!(
                "{} element(s) Ductus can't show yet were left out.",
                self.skipped
            ));
        }
        out
    }
}

fn notebook_entry(notebook: &Notebook, name: String, warnings: &mut Warnings) -> Entry {
    Entry::Folder {
        name,
        notebook: true,
        color: notebook.color().map(color),
        entries: section_entries(notebook.entries(), warnings),
    }
}

fn section_entries(entries: &[SectionEntry], warnings: &mut Warnings) -> Vec<Entry> {
    entries
        .iter()
        .filter_map(|entry| match entry {
            SectionEntry::Section(section) => Some(section_entry(section, warnings)),
            SectionEntry::SectionGroup(group) if group.display_name() == "OneNote_RecycleBin" => {
                None
            }
            SectionEntry::SectionGroup(group) => Some(Entry::Folder {
                name: group.display_name().to_owned(),
                notebook: false,
                color: None,
                entries: section_entries(group.entries(), warnings),
            }),
        })
        .collect()
}

fn section_entry(section: &Section, warnings: &mut Warnings) -> Entry {
    let name = section.display_name();
    let name = name.strip_suffix(".one").unwrap_or(name);
    let mut entries = Vec::new();
    for page in section.page_series().iter().flat_map(|s| s.pages()) {
        let mut builder = Builder::default();
        builder.page(page);
        warnings.skipped += builder.skipped;
        warnings.pictures += builder.bad_pictures;
        let files = std::mem::take(&mut builder.files);
        entries.push(Entry::Page(builder.finish(page)));
        entries.extend(
            files
                .into_iter()
                .map(|(name, bytes)| Entry::File { name, bytes }),
        );
    }
    Entry::Folder {
        name: name.to_owned(),
        notebook: false,
        color: section.color().map(color),
        entries,
    }
}

fn color(c: onenote_parser::property::common::Color) -> Rgba {
    Rgba::rgb(
        f32::from(c.r()) / 255.0,
        f32::from(c.g()) / 255.0,
        f32::from(c.b()) / 255.0,
    )
}

#[derive(Default)]
struct Builder {
    strokes: Vec<Stroke>,
    images: Vec<Image>,
    texts: Vec<TextBox>,
    maths: Vec<MathBox>,
    assets: Vec<(String, Vec<u8>)>,
    files: Vec<(String, Vec<u8>)>,
    skipped: usize,
    bad_pictures: usize,
}

/// Text collected for one box, and where the next box goes.
struct Flow {
    x: f64,
    y: f64,
    width: Option<f64>,
    /// Markdown blocks with their font size in points.
    blocks: Vec<(String, f64)>,
}

/// Roughly how wide a line of text is.
fn text_width(chars: usize, size: f64) -> f64 {
    chars as f64 * size * POINT * 0.5
}

impl Flow {
    fn new(x: f64, y: f64, width: Option<f64>) -> Self {
        Self {
            x,
            y,
            width,
            blocks: Vec::new(),
        }
    }

    fn push(&mut self, markdown: String, size: f64) {
        if !markdown.trim().is_empty() {
            self.blocks.push((markdown, size));
        }
    }

    /// The collected text as a box; the flow continues below it.
    fn flush(&mut self, texts: &mut Vec<TextBox>) {
        if self.blocks.is_empty() {
            return;
        }
        let lines = || {
            self.blocks
                .iter()
                .flat_map(|(md, size)| md.lines().map(move |l| (l.chars().count(), *size)))
        };
        let natural = lines()
            .map(|(chars, size)| text_width(chars, size))
            .fold(0.0, f64::max);
        let width = self.width.unwrap_or(natural.min(MAX_WIDTH)) + GAP;
        // Long lines wrap; each takes a line height per row.
        let height: f64 = lines()
            .map(|(chars, size)| {
                let rows = (text_width(chars, size) / width).ceil().max(1.0);
                rows * size * POINT * 1.45
            })
            .sum::<f64>()
            + (self.blocks.len() - 1) as f64 * BODY_PT * POINT * 0.6
            + GAP;
        let markdown = self
            .blocks
            .iter()
            .map(|(md, _)| md.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");
        texts.push(TextBox {
            id: StrokeId(0),
            markdown,
            x: self.x,
            y: self.y,
            width,
            height,
            scale: BODY_PT * POINT / TextBox::FONT_SIZE,
        });
        self.y += height + GAP;
        self.blocks.clear();
    }
}

/// A paragraph's text, or an equation in it.
enum Piece {
    Text(String),
    Math(String),
}

impl Builder {
    fn page(&mut self, page: &Page) {
        if let (Some(title), Some(text)) = (page.title(), page.title_text()) {
            if !text.trim().is_empty() {
                let x = f64::from(title.offset_horizontal()) * HALF_INCH + HALF_INCH;
                let y = f64::from(title.offset_vertical()) * HALF_INCH + HALF_INCH / 2.0;
                let mut flow = Flow::new(x, y, None);
                flow.push(format!("# {}", escape_inline(text.trim())), BODY_PT * 2.0);
                flow.flush(&mut self.texts);
            }
        }
        for content in page.contents() {
            match content {
                PageContent::Outline(outline) => self.outline(outline),
                PageContent::Image(image) => {
                    let at = offset(image.offset_horizontal(), image.offset_vertical());
                    self.image(image, at);
                }
                PageContent::EmbeddedFile(file) => self.file(file),
                PageContent::Ink(ink) => {
                    self.ink(ink, None);
                }
                _ => self.skipped += 1,
            }
        }
    }

    fn finish(self, page: &Page) -> ImportedPage {
        let Builder {
            mut strokes,
            mut images,
            mut texts,
            mut maths,
            assets,
            ..
        } = self;
        // Nothing may lie above or left of the page's corner.
        let corners = strokes
            .iter()
            .flat_map(|s| s.points.iter().map(|p| (p.x, p.y)))
            .chain(images.iter().map(|i| (i.x, i.y)))
            .chain(texts.iter().map(|t| (t.x, t.y)))
            .chain(maths.iter().map(|m| (m.x, m.y)));
        let (left, top) = corners.fold((0.0, 0.0), |(l, t): (f64, f64), (x, y)| {
            (l.min(x), t.min(y))
        });
        if left < 0.0 || top < 0.0 {
            for p in strokes.iter_mut().flat_map(|s| s.points.iter_mut()) {
                (p.x, p.y) = (p.x - left, p.y - top);
            }
            for i in &mut images {
                (i.x, i.y) = (i.x - left, i.y - top);
            }
            for t in &mut texts {
                (t.x, t.y) = (t.x - left, t.y - top);
            }
            for m in &mut maths {
                (m.x, m.y) = (m.x - left, m.y - top);
            }
        }
        let settings = PageSettings {
            kind: PageKind::Endless,
            background: PageBackground {
                pattern: PatternStyle::Blank,
                paper: Rgba::rgb(1.0, 1.0, 1.0),
                ..PageBackground::default()
            },
        };
        let name = page
            .title_text()
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| "Untitled page".into());
        ImportedPage {
            name,
            document: Document::with_contents(settings, strokes)
                .with_images(images)
                .with_texts(texts)
                .with_maths(maths),
            pdf: None,
            assets,
        }
    }

    fn outline(&mut self, outline: &Outline) {
        let (x, y) = offset(outline.offset_horizontal(), outline.offset_vertical());
        let width = outline.layout_max_width().map(|w| f64::from(w) * HALF_INCH);
        let mut flow = Flow::new(x, y, width);
        self.items(outline.items(), 0, &mut flow);
        flow.flush(&mut self.texts);
    }

    fn items(&mut self, items: &[OutlineItem], lists: usize, flow: &mut Flow) {
        for item in items {
            match item {
                OutlineItem::Group(group) => self.items(group.outlines(), lists, flow),
                OutlineItem::Element(element) => self.element(element, lists, flow),
            }
        }
    }

    /// `lists` counts the list items this element is nested in, for Markdown indentation.
    fn element(&mut self, element: &OutlineElement, lists: usize, flow: &mut Flow) {
        let bullet = element.list_contents().first().map(|list| {
            let numbered = list
                .list_format()
                .iter()
                .any(|c| (*c as u32) < 0x20 || c.is_ascii_digit());
            if numbered { "1. " } else { "- " }
        });
        let indent = "   ".repeat(lists);
        let mut first = true;
        for content in element.contents() {
            match content {
                Content::RichText(text) => {
                    let (pieces, size) = rich_text(text);
                    let marker = bullet.filter(|_| first).unwrap_or("");
                    let mut line = format!("{indent}{marker}");
                    for piece in pieces {
                        match piece {
                            Piece::Text(t) => line.push_str(&t),
                            Piece::Math(latex) => {
                                flow.push(std::mem::replace(&mut line, indent.clone()), size);
                                flow.flush(&mut self.texts);
                                match math_box(&latex, (flow.x, flow.y), size) {
                                    Some(math) => {
                                        flow.y += math.height + GAP;
                                        self.maths.push(math);
                                    }
                                    None => line.push_str(&format!("`{latex}`")),
                                }
                            }
                        }
                    }
                    flow.push(line, size);
                    for object in text.embedded_objects() {
                        if let EmbeddedObject::Ink(container) = object {
                            flow.flush(&mut self.texts);
                            let height = self.ink(container.ink(), Some((flow.x, flow.y)));
                            flow.y += height + GAP;
                        }
                    }
                }
                Content::Table(table) => flow.push(table_markdown(table), BODY_PT),
                Content::Image(image) => {
                    flow.flush(&mut self.texts);
                    if let Some(height) = self.image(image, (flow.x, flow.y)) {
                        flow.y += height + GAP;
                    }
                }
                Content::EmbeddedFile(file) => {
                    self.file(file);
                    flow.push(
                        format!("{indent}\u{1f4ce} {}", escape_inline(file.filename())),
                        BODY_PT,
                    );
                }
                Content::Ink(ink) => {
                    flow.flush(&mut self.texts);
                    let height = self.ink(ink, Some((flow.x, flow.y)));
                    flow.y += height + GAP;
                }
                _ => self.skipped += 1,
            }
            first = false;
        }
        let nested = lists + usize::from(bullet.is_some());
        self.items(element.children(), nested, flow);
    }

    /// Returns the picture's height.
    fn image(&mut self, image: &OneImage, (x, y): (f64, f64)) -> Option<f64> {
        let read = |reader: Option<Box<dyn std::io::Read>>| {
            let mut bytes = Vec::new();
            reader?.read_to_end(&mut bytes).ok()?;
            Some(bytes)
        };
        // Some formats (such as EMF) come with a picture browsers can show.
        let Some((name, png)) = read(image.read())
            .and_then(|b| picture(&b))
            .or_else(|| read(image.web_picture().map(|p| p.read())).and_then(|b| picture(&b)))
        else {
            self.bad_pictures += 1;
            return None;
        };
        let (pw, ph) = pixel_size(&png).unwrap_or((100.0, 100.0));
        let half_inches = |v: Option<f32>| v.map(|v| f64::from(v) * HALF_INCH).filter(|v| *v > 0.0);
        let (width, height) = match (
            half_inches(image.picture_width()).or(half_inches(image.layout_max_width())),
            half_inches(image.picture_height()).or(half_inches(image.layout_max_height())),
        ) {
            (Some(w), Some(h)) => (w, h),
            (Some(w), None) => (w, w * ph / pw),
            (None, Some(h)) => (h * pw / ph, h),
            (None, None) => (pw, ph),
        };
        if !self.assets.iter().any(|(n, _)| *n == name) {
            self.assets.push((name.clone(), png));
        }
        self.images.push(Image {
            id: StrokeId(0),
            source: name,
            x,
            y,
            width,
            height,
        });
        Some(height)
    }

    fn file(&mut self, file: &EmbeddedFile) {
        let mut bytes = Vec::new();
        let read = file.read().read_to_end(&mut bytes).is_ok();
        if read && !bytes.is_empty() && !file.filename().is_empty() {
            self.files.push((file.filename().to_owned(), bytes));
        } else {
            self.skipped += 1;
        }
    }

    /// Page ink sits at its offset; ink inside text starts at `at`. Returns the ink's height.
    fn ink(&mut self, ink: &Ink, at: Option<(f64, f64)>) -> f64 {
        let mut height: f64 = 0.0;
        for child in ink.child_groups() {
            height = height.max(self.ink(child, at));
        }
        let strokes = ink.ink_strokes();
        if strokes.is_empty() {
            return height;
        }
        let paths: Vec<Vec<(f64, f64)>> = strokes
            .iter()
            .map(|s| {
                s.absolute_path()
                    .iter()
                    .map(|p| (f64::from(p.x()) * INK_UNIT, f64::from(p.y()) * INK_UNIT))
                    .collect()
            })
            .collect();
        let points = || paths.iter().flatten();
        let (min_y, max_y) = points().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(p.1), hi.max(p.1))
        });
        let (dx, dy) = match at {
            None => offset(ink.offset_horizontal(), ink.offset_vertical()),
            Some((x, y)) => {
                let min_x = points().map(|p| p.0).fold(f64::INFINITY, f64::min);
                (x - min_x, y - min_y)
            }
        };
        for (stroke, path) in strokes.iter().zip(&paths) {
            if let Some(s) = ink_stroke(stroke, path, (dx, dy)) {
                self.strokes.push(s);
            }
        }
        height.max(max_y - min_y)
    }
}

fn offset(x: Option<f32>, y: Option<f32>) -> (f64, f64) {
    (
        f64::from(x.unwrap_or(0.0)) * HALF_INCH,
        f64::from(y.unwrap_or(0.0)) * HALF_INCH,
    )
}

fn ink_stroke(stroke: &InkStroke, path: &[(f64, f64)], (dx, dy): (f64, f64)) -> Option<Stroke> {
    if path.is_empty() {
        return None;
    }
    // See-through ink is OneNote's highlighter.
    let highlighter = stroke.transparency().unwrap_or(0) > 0;
    let color = stroke.color().map_or(Rgba::BLACK, |v| {
        let channel = |shift: u32| ((v >> shift) & 0xff) as f32 / 255.0;
        Rgba::rgb(channel(0), channel(8), channel(16))
    });
    let tip = if highlighter {
        stroke.width().max(stroke.height())
    } else {
        stroke.width()
    };
    let width = (f64::from(tip) * INK_UNIT).max(0.5);
    Some(Stroke {
        id: StrokeId(0),
        style: StrokeStyle {
            tool: if highlighter {
                Tool::Highlighter
            } else {
                Tool::Pen
            },
            color,
            width,
            pattern: LinePattern::Solid,
        },
        points: crate::densify(
            path.iter()
                .map(|&(x, y)| InkPoint {
                    x: x + dx,
                    y: y + dy,
                    pressure: 1.0,
                    tilt_x: 0.0,
                    tilt_y: 0.0,
                    t: 0,
                })
                .collect(),
        ),
    })
}

/// An equation as a Ductus formula, sized like the text around it.
fn math_box(latex: &str, (x, y): (f64, f64), size: f64) -> Option<MathBox> {
    let rendered =
        ductus_math::render(latex, ductus_math::Syntax::Latex, ductus_math::BODY_SIZE).ok()?;
    let scale = size / ductus_math::BODY_SIZE;
    Some(MathBox {
        id: StrokeId(0),
        source: latex.to_owned(),
        syntax: ductus_math::Syntax::Latex.name().into(),
        x,
        y,
        width: rendered.width * POINT * scale,
        height: rendered.height * POINT * scale,
        scale,
        color: Rgba::BLACK,
    })
}

fn table_markdown(table: &Table) -> String {
    let rows: Vec<Vec<String>> = table
        .contents()
        .iter()
        .map(|row| {
            row.contents()
                .iter()
                .map(|cell| {
                    cell.contents()
                        .iter()
                        .flat_map(OutlineElement::contents)
                        .filter_map(Content::rich_text)
                        .flat_map(|t| rich_text(t).0)
                        .map(|piece| match piece {
                            Piece::Text(t) => t.replace("\\\n", " "),
                            Piece::Math(latex) => format!("`{latex}`"),
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .collect()
        })
        .collect();
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0).max(1);
    let line = |cells: &[String]| {
        let mut cells = cells.to_vec();
        cells.resize(columns, String::new());
        format!("| {} |", cells.join(" | "))
    };
    let mut out = Vec::with_capacity(rows.len() + 1);
    out.push(line(rows.first().map_or(&[][..], Vec::as_slice)));
    out.push(line(&vec!["---".to_owned(); columns]));
    out.extend(rows.iter().skip(1).map(|r| line(r)));
    out.join("\n")
}

/// A paragraph as Markdown and equations, with its font size in points.
fn rich_text(text: &RichText) -> (Vec<Piece>, f64) {
    let chars: Vec<char> = text.text().chars().collect();
    // Runs and links count UTF-16 units; math letters take two.
    let mut char_at: Vec<usize> = Vec::with_capacity(chars.len() + 1);
    for (i, c) in chars.iter().enumerate() {
        char_at.extend(std::iter::repeat_n(i, c.len_utf16()));
    }
    let char_at = |unit: u32| char_at.get(unit as usize).copied().unwrap_or(chars.len());
    let base = text.paragraph_style();
    let size = base.font_size().map_or(BODY_PT, |s| f64::from(s) / 2.0);
    let links = text.hyperlinks();
    let styles = text.text_run_formatting();
    let ends = text.text_run_indices();
    // One object per math run: the one its opening character opens.
    let mut objects = text.math_inline_objects().iter().copied();
    let mut pieces: Vec<Piece> = Vec::new();
    let mut math: Vec<(char, MathInlineObject)> = Vec::new();
    let flush_math = |math: &mut Vec<(char, MathInlineObject)>, pieces: &mut Vec<Piece>| {
        let latex = math::to_latex(math);
        // OneNote's placeholder in a new, empty equation.
        if !latex.is_empty() && !latex.contains("Type equation here") {
            pieces.push(Piece::Math(latex));
        }
        math.clear();
    };
    let mut start = 0;
    let runs = styles.len().max(1);
    for i in 0..runs {
        let end = if i + 1 == runs {
            chars.len()
        } else {
            ends.get(i)
                .map_or(chars.len(), |&e| char_at(e))
                .clamp(start, chars.len())
        };
        let style = styles.get(i).unwrap_or(base);
        let run = &chars[start..end];
        if style.math_formatting() {
            let object = objects.next().unwrap_or_default();
            math.extend(run.iter().map(|&c| (c, object)));
        } else {
            if !math.is_empty() {
                flush_math(&mut math, &mut pieces);
            }
            let run: String = run.iter().collect();
            if !style.hidden() && !run.is_empty() {
                let mut piece = styled(&run, style);
                if let Some(link) = links
                    .iter()
                    .find(|l| char_at(l.start()) <= start && end <= char_at(l.end()))
                {
                    piece = format!("[{piece}](<{}>)", link.target().replace('>', "%3E"));
                }
                match pieces.last_mut() {
                    Some(Piece::Text(t)) => t.push_str(&piece),
                    _ => pieces.push(Piece::Text(piece)),
                }
            }
        }
        start = end;
    }
    if !math.is_empty() {
        flush_math(&mut math, &mut pieces);
    }
    if let Some(Piece::Text(first)) = pieces.first_mut() {
        guard_line_start(first);
        let heading = match base.style_id() {
            Some("h1") => "# ",
            Some("h2") => "## ",
            Some("h3") => "### ",
            Some("h4" | "h5" | "h6") => "#### ",
            _ => "",
        };
        first.insert_str(0, heading);
    }
    (pieces, size)
}

fn styled(run: &str, style: &ParagraphStyling) -> String {
    let body = run.trim();
    if body.is_empty() {
        return run.replace(['\u{b}', '\n'], " ");
    }
    let lead = &run[..run.len() - run.trim_start().len()];
    let trail = &run[run.trim_end().len()..];
    let mut text = escape_inline(body).replace(['\u{b}', '\n', '\r'], "\\\n");
    if style.strikethrough() {
        text = format!("~~{text}~~");
    }
    if style.italic() {
        text = format!("*{text}*");
    }
    if style.bold() {
        text = format!("**{text}**");
    }
    format!("{lead}{text}{trail}")
}

fn pixel_size(png: &[u8]) -> Option<(f64, f64)> {
    let (w, h) = image::ImageReader::new(std::io::Cursor::new(png))
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()?;
    Some((f64::from(w.max(1)), f64::from(h.max(1))))
}
