use lopdf::content::{Content, Operation};
use lopdf::{Dictionary, Document, Object, Stream, dictionary};

/// Each page has a red 20 pt square 50 pt from its top-left corner (unrotated) and a line of text.
pub fn make_pdf(pages: &[(f64, f64, i64)]) -> Vec<u8> {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font = doc.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
    });
    let resources = doc.add_object(dictionary! {
        "Font" => dictionary! { "F1" => font },
    });
    let mut kids = Vec::new();
    for &(w, h, rotate) in pages {
        let content = Content {
            operations: vec![
                Operation::new("rg", vec![1.into(), 0.into(), 0.into()]),
                Operation::new(
                    "re",
                    vec![50.into(), (h - 70.0).into(), 20.into(), 20.into()],
                ),
                Operation::new("f", vec![]),
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 18.into()]),
                Operation::new("rg", vec![0.into(), 0.into(), 0.into()]),
                Operation::new("Td", vec![100.into(), (h - 70.0).into()]),
                Operation::new("Tj", vec![Object::string_literal("Lecture 3")]),
                Operation::new("ET", vec![]),
            ],
        };
        let content_id = doc.add_object(Stream::new(Dictionary::new(), content.encode().unwrap()));
        let mut page = dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), w.into(), h.into()],
            "Contents" => content_id,
            "Resources" => resources,
        };
        if rotate != 0 {
            page.set("Rotate", rotate);
        }
        kids.push(doc.add_object(page).into());
    }
    let count = kids.len() as i64;
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count }),
    );
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog);
    let mut out = Vec::new();
    doc.save_to(&mut out).unwrap();
    out
}
