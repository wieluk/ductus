//! Notes from other apps, turned into ordinary Ductus pages: the originals are only read.

mod folder;
mod onenote;
mod rnote;
mod sheets;
mod xournal;

use ductus_core::{Document, InkPoint, Rgba};
use ductus_format::{Error, Library};
use std::path::Path;

/// File extensions that can be imported, lower case.
pub const EXTENSIONS: &[&str] = &["xopp", "xoj", "rnote", "one", "onetoc2", "onepkg"];

pub struct ImportedPage {
    pub name: String,
    pub document: Document,
    /// The PDF the page is written over.
    pub pdf: Option<Vec<u8>>,
    /// Pictures by file name, as the document's images name them.
    pub assets: Vec<(String, Vec<u8>)>,
}

pub enum Entry {
    Page(ImportedPage),
    Folder {
        name: String,
        notebook: bool,
        color: Option<Rgba>,
        entries: Vec<Entry>,
    },
    /// Kept as it is, such as a PDF attached to a OneNote page.
    File {
        name: String,
        bytes: Vec<u8>,
    },
}

pub struct Import {
    pub entry: Entry,
    /// What couldn't be brought over, for the user.
    pub warnings: Vec<String>,
}

pub struct Saved {
    /// The page or folder created at the top.
    pub root: String,
    pub first_page: Option<String>,
}

pub fn supports(path: &Path) -> bool {
    extension(path).is_some_and(|e| EXTENSIONS.contains(&e.as_str()))
}

/// From the name rather than `Path::extension`: OneNote names notebooks just ".onetoc2".
fn extension(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    Some(name.rsplit_once('.')?.1.to_ascii_lowercase())
}

/// A file, or a folder that becomes a notebook.
pub fn read(path: &Path) -> Result<Import, Error> {
    if path.is_dir() {
        return folder::read(path);
    }
    match extension(path).as_deref() {
        Some("xopp" | "xoj") => xournal::read(path),
        Some("rnote") => rnote::read(path),
        Some("one" | "onetoc2" | "onepkg") => onenote::read(path),
        _ => Err(Error::Invalid(format!(
            "{} isn't a file Ductus can import.",
            path.display()
        ))),
    }
}

pub fn save(library: &mut Library, parent: &str, import: &Import) -> Result<Saved, Error> {
    let mut first_page = None;
    let root = save_entry(library, parent, &import.entry, &mut first_page)?;
    Ok(Saved { root, first_page })
}

fn save_entry(
    library: &mut Library,
    parent: &str,
    entry: &Entry,
    first_page: &mut Option<String>,
) -> Result<String, Error> {
    match entry {
        Entry::Page(page) => {
            let name = library.unique_name(parent, &file_name(&page.name));
            let rel = library.add_page(
                parent,
                &name,
                page.document.clone(),
                page.pdf.as_deref(),
                &page.assets,
            )?;
            first_page.get_or_insert_with(|| rel.clone());
            Ok(rel)
        }
        Entry::Folder {
            name,
            notebook,
            color,
            entries,
        } => {
            let name = library.unique_name(parent, &file_name(name));
            let rel = library.create_folder(parent, &name)?;
            if *notebook {
                library.set_notebook(&rel, Some(&name))?;
            }
            if color.is_some() {
                library.set_color(&rel, *color)?;
            }
            for entry in entries {
                save_entry(library, &rel, entry, first_page)?;
            }
            Ok(rel)
        }
        Entry::File { name, bytes } => {
            let name = file_name(name);
            match ductus_format::library::pdf_stem(&name) {
                Some(stem) => {
                    let stem = library.unique_name(parent, stem);
                    library.add_pdf(parent, &stem, bytes)
                }
                None => library.add_file(parent, &name, bytes),
            }
        }
    }
}

/// A title as a file name: characters file systems refuse become dashes.
fn file_name(title: &str) -> String {
    let name: String = title
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() {
                '-'
            } else {
                c
            }
        })
        .collect();
    let name = name.trim().trim_matches('.').trim();
    if name.is_empty() {
        "Imported".into()
    } else {
        name.chars().take(120).collect()
    }
}

pub(crate) fn pdf_error(e: ductus_pdf::Error) -> Error {
    Error::Invalid(e.to_string())
}

/// Ductus stores pictures as PNG, named by content like pictures pasted in.
pub(crate) fn picture(bytes: &[u8]) -> Option<(String, Vec<u8>)> {
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n";
    let png = if bytes.starts_with(PNG) {
        bytes.to_vec()
    } else {
        let decoded = image::load_from_memory(bytes).ok()?;
        let mut out = std::io::Cursor::new(Vec::new());
        decoded.write_to(&mut out, image::ImageFormat::Png).ok()?;
        out.into_inner()
    };
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in &png {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    Some((
        format!("{}{h:016x}.png", ductus_format::library::IMAGE_PREFIX),
        png,
    ))
}

/// Ductus smooths ink through its points, so sparse ones (shapes, ruler lines) get points
/// every two units, like its own shapes, to keep their corners.
pub(crate) fn densify(points: Vec<InkPoint>) -> Vec<InkPoint> {
    const STEP: f64 = 2.0;
    let mut out = Vec::with_capacity(points.len());
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        out.push(a);
        let n = ((b.x - a.x).hypot(b.y - a.y) / STEP).ceil() as usize;
        out.extend((1..n).map(|i| {
            let t = i as f64 / n as f64;
            InkPoint {
                x: a.x + (b.x - a.x) * t,
                y: a.y + (b.y - a.y) * t,
                pressure: a.pressure + (b.pressure - a.pressure) * t as f32,
                ..a
            }
        }));
    }
    out.extend(points.last());
    out
}

/// Characters Markdown would read as formatting, escaped.
pub(crate) fn escape_inline(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    for c in text.chars() {
        if matches!(
            c,
            '\\' | '`' | '*' | '_' | '[' | ']' | '<' | '>' | '#' | '|' | '~'
        ) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Escapes a line start that would begin a list, a numbered list or a heading underline.
pub(crate) fn guard_line_start(line: &mut String) {
    let start = line.trim_start();
    let digits = start.chars().take_while(char::is_ascii_digit).count();
    let numbered = digits > 0 && matches!(start[digits..].chars().next(), Some('.' | ')'));
    if start.starts_with(['-', '+', '=']) || numbered {
        let at = line.len() - start.len() + if numbered { digits } else { 0 };
        line.insert(at, '\\');
    }
}

/// Plain text as Markdown that shows it unchanged.
pub(crate) fn plain_markdown(text: &str) -> String {
    let escape_line = |line: &str| {
        let mut out = escape_inline(line.trim_end());
        guard_line_start(&mut out);
        out
    };
    text.replace("\r\n", "\n")
        .split("\n\n")
        .map(|paragraph| {
            paragraph
                .lines()
                .map(escape_line)
                .collect::<Vec<_>>()
                .join("\\\n")
        })
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_stays_plain() {
        assert_eq!(plain_markdown("a*b\n- c\n2. d"), "a\\*b\\\n\\- c\\\n2\\. d");
        assert_eq!(plain_markdown("one\n\ntwo"), "one\n\ntwo");
    }

    #[test]
    fn long_gaps_get_points() {
        let point = |x: f64| InkPoint {
            x,
            y: 0.0,
            pressure: 1.0,
            tilt_x: 0.0,
            tilt_y: 0.0,
            t: 0,
        };
        let dense = densify(vec![point(0.0), point(1.0), point(9.0)]);
        let xs: Vec<f64> = dense.iter().map(|p| p.x).collect();
        assert_eq!(xs, [0.0, 1.0, 3.0, 5.0, 7.0, 9.0]);
    }

    #[test]
    fn titles_become_file_names() {
        assert_eq!(file_name("Lecture 3: Limits?"), "Lecture 3- Limits-");
        assert_eq!(file_name(" .. "), "Imported");
    }
}
