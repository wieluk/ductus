//! Several PDFs as one, such as the pages of a notebook exported together.

use crate::Error;
use crate::annot::inherited;
use lopdf::{Dictionary, Object, ObjectId, Stream, dictionary};
use std::collections::HashSet;

/// Where a page of a composed PDF comes from; blank sizes are in PDF points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SheetSource {
    Page { file: usize, index: usize },
    Blank { width: f64, height: f64 },
}

/// The pages of `files`, one after another.
pub fn merge(files: &[Vec<u8>]) -> Result<Vec<u8>, Error> {
    assemble(files, None)
}

/// Pages picked from `files` and blank pages, in the given order; a page may be picked twice.
pub fn compose(files: &[Vec<u8>], sheets: &[SheetSource]) -> Result<Vec<u8>, Error> {
    assemble(files, Some(sheets))
}

fn assemble(files: &[Vec<u8>], sheets: Option<&[SheetSource]>) -> Result<Vec<u8>, Error> {
    let mut out = lopdf::Document::with_version("1.7");
    let pages_id = out.new_object_id();
    let mut file_pages = Vec::with_capacity(files.len());
    for bytes in files {
        let mut doc = lopdf::Document::load_mem(bytes)?;
        doc.renumber_objects_with(out.max_id + 1);
        out.max_id = out.max_id.max(doc.max_id);
        let pages: Vec<ObjectId> = doc.get_pages().into_values().collect();
        for &page in &pages {
            // What a page inherits from its old page tree goes onto the page itself.
            let own: Vec<(&[u8], Object)> = [&b"MediaBox"[..], b"CropBox", b"Resources", b"Rotate"]
                .into_iter()
                .filter_map(|key| inherited(&doc, page, key).map(|v| (key, v.clone())))
                .collect();
            let dict = doc.get_dictionary_mut(page)?;
            for (key, value) in own {
                if !dict.has(key) {
                    dict.set(key, value);
                }
            }
            dict.set("Parent", pages_id);
        }
        file_pages.push(pages);
        out.objects.extend(doc.objects);
    }
    let every: Vec<SheetSource>;
    let sheets = match sheets {
        Some(sheets) => sheets,
        None => {
            every = file_pages
                .iter()
                .enumerate()
                .flat_map(|(file, pages)| {
                    (0..pages.len()).map(move |index| SheetSource::Page { file, index })
                })
                .collect();
            &every
        }
    };
    let mut used = HashSet::new();
    let mut kids = Vec::with_capacity(sheets.len());
    for sheet in sheets {
        let id = match *sheet {
            SheetSource::Page { file, index } => {
                let page = *file_pages
                    .get(file)
                    .and_then(|pages| pages.get(index))
                    .ok_or_else(|| Error::Invalid(format!("The PDF has no page {}.", index + 1)))?;
                if used.insert(page) {
                    page
                } else {
                    // Picked again: a page object can only appear once in the tree.
                    let mut copy = out.get_dictionary(page)?.clone();
                    copy.remove(b"Annots");
                    out.add_object(copy)
                }
            }
            SheetSource::Blank { width, height } => {
                let contents = out.add_object(Stream::new(Dictionary::new(), Vec::new()));
                out.add_object(dictionary! {
                    "Type" => "Page",
                    "Parent" => pages_id,
                    "MediaBox" => vec![0.into(), 0.into(), (width as f32).into(), (height as f32).into()],
                    "Resources" => Dictionary::new(),
                    "Contents" => contents,
                })
            }
        };
        kids.push(Object::Reference(id));
    }
    if kids.is_empty() {
        return Err(Error::Invalid("There are no pages to export.".into()));
    }
    let count = kids.len() as i64;
    out.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count }),
    );
    let catalog = out.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    out.trailer.set("Root", catalog);
    // The old catalogs and page trees are left over.
    out.prune_objects();
    let mut bytes = Vec::new();
    out.save_to(&mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testpdf::make_pdf;

    #[test]
    fn merged_pages_keep_their_order_and_size() {
        let a = make_pdf(&[(595.0, 842.0, 0)]);
        let b = make_pdf(&[(842.0, 595.0, 90), (300.0, 300.0, 0)]);
        let merged = merge(&[a, b]).unwrap();
        let doc = lopdf::Document::load_mem(&merged).unwrap();
        let pages: Vec<ObjectId> = doc.get_pages().into_values().collect();
        assert_eq!(pages.len(), 3);
        let size = |i: usize| crate::annot::PageGeometry::of(&doc, pages[i]).size();
        assert_eq!(size(0), (595.0, 842.0));
        assert_eq!(size(1), (842.0, 595.0));
        assert_eq!(crate::annot::PageGeometry::of(&doc, pages[1]).rotate, 90);
        assert_eq!(size(2), (300.0, 300.0));
        assert!(merge(&[]).is_err());
    }

    #[test]
    fn composed_pages_follow_the_sheets() {
        let a = make_pdf(&[(595.0, 842.0, 0), (300.0, 300.0, 0)]);
        let sheets = [
            SheetSource::Page { file: 0, index: 1 },
            SheetSource::Blank {
                width: 200.0,
                height: 100.0,
            },
            SheetSource::Page { file: 0, index: 1 },
        ];
        let composed = compose(std::slice::from_ref(&a), &sheets).unwrap();
        let doc = lopdf::Document::load_mem(&composed).unwrap();
        let pages: Vec<ObjectId> = doc.get_pages().into_values().collect();
        let size = |i: usize| crate::annot::PageGeometry::of(&doc, pages[i]).size();
        assert_eq!(
            (size(0), size(1), size(2)),
            ((300.0, 300.0), (200.0, 100.0), (300.0, 300.0))
        );
        assert!(compose(&[a], &[SheetSource::Page { file: 0, index: 2 }]).is_err());
    }
}
