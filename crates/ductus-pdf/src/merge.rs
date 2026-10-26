//! Several PDFs as one, such as the pages of a notebook exported together.

use crate::Error;
use crate::annot::inherited;
use lopdf::{Object, ObjectId, dictionary};

/// The pages of `files`, one after another.
pub fn merge(files: &[Vec<u8>]) -> Result<Vec<u8>, Error> {
    let mut out = lopdf::Document::with_version("1.7");
    let pages_id = out.new_object_id();
    let mut kids = Vec::new();
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
            kids.push(Object::Reference(page));
        }
        out.objects.extend(doc.objects);
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
}
