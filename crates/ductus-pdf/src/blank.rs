//! Blank pages added to a PDF, for notes between its pages.

use crate::Error;
use crate::annot::PageGeometry;
use lopdf::{Dictionary, Object, ObjectId, Stream, dictionary};

/// A blank page the size of page `after` (0-based), right after it; past the end it goes last.
pub fn insert_blank_page(bytes: &[u8], after: usize) -> Result<Vec<u8>, Error> {
    let mut pdf = lopdf::Document::load_mem(bytes)?;
    let pages: Vec<ObjectId> = pdf.get_pages().into_values().collect();
    let &reference = pages
        .get(after)
        .or(pages.last())
        .ok_or_else(|| Error::Invalid("The PDF has no pages.".into()))?;
    let geometry = PageGeometry::of(&pdf, reference);
    let parent = pdf
        .get_dictionary(reference)?
        .get(b"Parent")
        .and_then(Object::as_reference)?;
    let contents = pdf.add_object(Stream::new(Dictionary::new(), Vec::new()));
    let mut page = dictionary! {
        "Type" => "Page",
        "Parent" => parent,
        "MediaBox" => vec![
            geometry.llx.into(),
            geometry.lly.into(),
            geometry.urx.into(),
            geometry.ury.into(),
        ],
        "Resources" => Dictionary::new(),
        "Contents" => contents,
    };
    // Turned like its neighbour, so it shows the same way round.
    if geometry.rotate != 0 {
        page.set("Rotate", geometry.rotate);
    }
    let blank = pdf.add_object(page);

    let kids = pdf
        .get_dictionary_mut(parent)?
        .get_mut(b"Kids")?
        .as_array_mut()?;
    let at = kids
        .iter()
        .position(|k| k.as_reference().ok() == Some(reference))
        .map_or(kids.len(), |i| i + 1);
    kids.insert(at, Object::Reference(blank));
    // Every page tree node above it counts one more page.
    let mut node = Some(parent);
    while let Some(id) = node {
        let dict = pdf.get_dictionary_mut(id)?;
        let count = dict.get(b"Count").and_then(Object::as_i64).unwrap_or(0);
        dict.set("Count", count + 1);
        node = dict.get(b"Parent").and_then(Object::as_reference).ok();
    }
    let mut out = Vec::with_capacity(bytes.len() + 256);
    pdf.save_to(&mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testpdf::make_pdf;

    #[test]
    fn a_blank_page_goes_after_the_given_one_in_its_size() {
        let pdf = make_pdf(&[(595.0, 842.0, 0), (842.0, 595.0, 0)]);
        let out = insert_blank_page(&pdf, 0).unwrap();
        let doc = lopdf::Document::load_mem(&out).unwrap();
        let pages: Vec<ObjectId> = doc.get_pages().into_values().collect();
        assert_eq!(pages.len(), 3);
        assert_eq!(PageGeometry::of(&doc, pages[1]).size(), (595.0, 842.0));
        assert_eq!(PageGeometry::of(&doc, pages[2]).size(), (842.0, 595.0));
        // Last, after the end.
        let out = insert_blank_page(&pdf, 9).unwrap();
        let doc = lopdf::Document::load_mem(&out).unwrap();
        let pages: Vec<ObjectId> = doc.get_pages().into_values().collect();
        assert_eq!(pages.len(), 3);
        assert_eq!(PageGeometry::of(&doc, pages[2]).size(), (842.0, 595.0));
    }
}
