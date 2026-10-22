use super::*;
use ductus_core::{PageSettings, StrokeId, TextBox};
use ductus_format::{PageFile, RecognizedLine};

fn library() -> (tempfile::TempDir, Library) {
    let dir = tempfile::tempdir().unwrap();
    let lib = Library::open(dir.path().join("Notes")).unwrap();
    (dir, lib)
}

fn write_page(lib: &mut Library, rel: &str, text: &str, recognized: &str) {
    let (folder, name) = rel.rsplit_once('/').unwrap();
    lib.create_folder("", folder).ok();
    let path = lib
        .create_page(folder, name, PageSettings::default())
        .unwrap();
    let mut page: PageFile = lib.load_page(&path).unwrap();
    page.document = page.document.with_texts([TextBox {
        id: StrokeId(0),
        markdown: text.into(),
        x: 10.0,
        y: 20.0,
        width: 200.0,
        height: 40.0,
        scale: 1.0,
    }]);
    page.recognized = vec![RecognizedLine {
        key: "k".into(),
        text: recognized.into(),
        rect: (10.0, 300.0, 250.0, 30.0),
        lang: Some("de".into()),
    }];
    lib.save_page(&path, &page).unwrap();
}

#[test]
fn finds_names_text_and_handwriting_with_their_place() {
    let (dir, mut lib) = library();
    write_page(
        &mut lib,
        "Analysis/Grenzwerte",
        "**Konvergenz** einer Folge",
        "Cauchy-Kriterium",
    );
    write_page(
        &mut lib,
        "Algebra/Gruppen",
        "Ein *Homomorphismus*",
        "Kern und Bild",
    );
    let mut index = Index::open(&dir.path().join("index.sqlite")).unwrap();
    let mut seen = Progress::default();
    index
        .update(&lib, &AtomicBool::new(false), |p| seen = p)
        .unwrap();
    assert_eq!(seen, Progress { done: 2, total: 2 });

    let hits = index.search("vergenz", 10).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].path, "Analysis/Grenzwerte");
    assert_eq!(hits[0].kind, Kind::Text);
    assert_eq!(hits[0].rect, Some((10.0, 20.0, 200.0, 40.0)));
    assert!(hits[0].snippet.contains("\u{1}vergenz\u{2}") || hits[0].snippet.contains("\u{1}"));

    let hits = index.search("cauchy", 10).unwrap();
    assert_eq!(hits[0].kind, Kind::Ink);
    assert_eq!(hits[0].rect, Some((10.0, 300.0, 250.0, 30.0)));

    let hits = index.search("algebra", 10).unwrap();
    assert_eq!(hits[0].kind, Kind::Title);
    assert_eq!(hits[0].path, "Algebra/Gruppen");

    assert_eq!(index.search("kern bild", 10).unwrap().len(), 1);
    assert_eq!(index.search("ein ho", 10).unwrap().len(), 1);
    assert!(index.search("kern xyz", 10).unwrap().is_empty());
    assert_eq!(index.search("Konvérgenz", 10).unwrap().len(), 1);
}

#[test]
fn updates_follow_changes_and_deletions() {
    let (dir, mut lib) = library();
    write_page(&mut lib, "Physik/Optik", "Brechung", "Linse");
    let mut index = Index::open(&dir.path().join("index.sqlite")).unwrap();
    let cancel = AtomicBool::new(false);
    index.update(&lib, &cancel, |_| {}).unwrap();
    assert_eq!(index.search("brechung", 10).unwrap().len(), 1);
    lib.delete("Physik/Optik").unwrap();
    index.update(&lib, &cancel, |_| {}).unwrap();
    assert!(index.search("brechung", 10).unwrap().is_empty());
}
