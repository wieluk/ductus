use super::*;
use ductus_core::{Command, History, InkPoint, Stroke, StrokeStyle};

fn lib() -> (tempfile::TempDir, Library) {
    let dir = tempfile::tempdir().unwrap();
    let lib = Library::open(dir.path().join("Notes")).unwrap();
    (dir, lib)
}

fn names(node: &Node) -> Vec<&str> {
    node.children.iter().map(|n| n.name.as_str()).collect()
}

fn find<'a>(node: &'a Node, path: &str) -> &'a Node {
    if node.path == path {
        return node;
    }
    node.children
        .iter()
        .find(|c| path == c.path || path.starts_with(&format!("{}/", c.path)))
        .map(|c| find(c, path))
        .unwrap_or_else(|| panic!("{path} not found"))
}

fn page_with_stroke(id: &str) -> PageFile {
    let mut doc = Document::new();
    let mut history = History::new();
    let stroke = Stroke {
        id: doc.alloc_id(),
        style: StrokeStyle::default(),
        points: vec![InkPoint {
            x: 10.0,
            y: 10.0,
            pressure: 0.5,
            tilt_x: 0.0,
            tilt_y: 0.0,
            t: 0,
        }],
    };
    history.apply(&mut doc, Command::AddStroke(stroke));
    PageFile {
        id: id.into(),
        document: doc,
        pdf: None,
        recognized: Vec::new(),
    }
}

#[test]
fn builds_a_notebook_tree_in_creation_order() {
    let (_d, mut lib) = lib();
    lib.create_folder("", "Physics").unwrap();
    lib.create_folder("", "Math").unwrap();
    lib.create_folder("Math", "Analysis").unwrap();
    lib.create_page("Math/Analysis", "Limits", PageSettings::default())
        .unwrap();
    lib.create_page("Math/Analysis", "Derivatives", PageSettings::default())
        .unwrap();

    let tree = lib.tree().unwrap();
    assert_eq!(
        names(&tree),
        ["Physics", "Math"],
        "creation order, not alphabetical"
    );
    let analysis = find(&tree, "Math/Analysis");
    assert_eq!(names(analysis), ["Limits", "Derivatives"]);
    assert_eq!(analysis.children[0].kind, NodeKind::Page);
    assert_eq!(analysis.children[0].path, "Math/Analysis/Limits");
    assert!(lib.root().join("Math/Analysis/Limits.svg").is_file());
}

#[test]
fn rename_keeps_position_and_contents() {
    let (_d, mut lib) = lib();
    lib.create_folder("", "A").unwrap();
    lib.create_folder("", "B").unwrap();
    lib.create_folder("", "C").unwrap();
    lib.create_page("B", "Page", PageSettings::default())
        .unwrap();
    let new = lib.rename("B", "Bee").unwrap();
    assert_eq!(new, "Bee");
    let tree = lib.tree().unwrap();
    assert_eq!(names(&tree), ["A", "Bee", "C"]);
    assert_eq!(names(find(&tree, "Bee")), ["Page"]);

    assert_eq!(lib.rename("Bee/Page", "Renamed").unwrap(), "Bee/Renamed");
    assert!(lib.load_page("Bee/Renamed").is_ok());
}

#[test]
fn names_are_validated_and_kept_unique() {
    let (_d, mut lib) = lib();
    lib.create_page("", "Untitled page", PageSettings::default())
        .unwrap();
    assert_eq!(lib.unique_name("", "Untitled page"), "Untitled page 2");
    assert!(matches!(
        lib.create_page("", "Untitled page", PageSettings::default()),
        Err(Error::Invalid(_))
    ));
    for bad in ["", "  ", ".hidden", "a/b", "a\\b", "what?", "trailing."] {
        assert!(lib.create_folder("", bad).is_err(), "{bad:?} accepted");
    }
    assert_eq!(validate_name("  Spaced  ").unwrap(), "Spaced");
}

#[test]
fn delete_moves_to_trash() {
    let (_d, mut lib) = lib();
    lib.create_folder("", "Old").unwrap();
    lib.create_page("Old", "Notes", PageSettings::default())
        .unwrap();
    lib.delete("Old").unwrap();
    assert!(lib.tree().unwrap().children.is_empty());
    let trash: Vec<_> = fs::read_dir(lib.root().join(".ductus/trash"))
        .unwrap()
        .collect();
    assert_eq!(trash.len(), 1);
    let trashed = trash[0].as_ref().unwrap().path();
    assert!(trashed.join("Notes.svg").is_file());
}

#[test]
fn move_between_folders_and_reorder() {
    let (_d, mut lib) = lib();
    lib.create_folder("", "Math").unwrap();
    lib.create_folder("", "Physics").unwrap();
    for name in ["One", "Two", "Three"] {
        lib.create_page("Math", name, PageSettings::default())
            .unwrap();
    }
    lib.move_to("Math/Three", "Math", Some(0)).unwrap();
    assert_eq!(
        names(find(&lib.tree().unwrap(), "Math")),
        ["Three", "One", "Two"]
    );
    lib.create_page("Physics", "Waves", PageSettings::default())
        .unwrap();
    assert_eq!(
        lib.move_to("Math/One", "Physics", Some(0)).unwrap(),
        "Physics/One"
    );
    let tree = lib.tree().unwrap();
    assert_eq!(names(find(&tree, "Math")), ["Three", "Two"]);
    assert_eq!(names(find(&tree, "Physics")), ["One", "Waves"]);
    lib.create_folder("Math", "Inner").unwrap();
    assert!(lib.move_to("Math", "Math/Inner", None).is_err());
}

#[test]
fn saves_atomically_and_round_trips() {
    let (_d, mut lib) = lib();
    let rel = lib.create_page("", "Ink", PageSettings::default()).unwrap();
    let id = lib.load_page(&rel).unwrap().id;
    lib.save_page(&rel, &page_with_stroke(&id)).unwrap();
    let loaded = lib.load_page(&rel).unwrap();
    assert_eq!(loaded.id, id);
    assert_eq!(loaded.document.strokes().len(), 1);
    let leftovers: Vec<_> = fs::read_dir(lib.root())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains(".tmp-"))
        .collect();
    assert!(leftovers.is_empty(), "temporary files left behind");
}

#[test]
fn history_keeps_previous_versions_within_limits() {
    let dir = tempfile::tempdir().unwrap();
    let mut lib = Library::with_policy(
        dir.path(),
        HistoryPolicy {
            interval: Duration::ZERO,
            keep: 3,
        },
    )
    .unwrap();
    let rel = lib
        .create_page("", "Page", PageSettings::default())
        .unwrap();
    let page = page_with_stroke("page-id");
    for _ in 0..5 {
        lib.save_page(&rel, &page).unwrap();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(lib.history("page-id").unwrap().len(), 3);

    let mut lib = Library::open(dir.path()).unwrap();
    let before = lib.history("page-id").unwrap().len();
    lib.save_page(&rel, &page).unwrap();
    lib.save_page(&rel, &page).unwrap();
    assert_eq!(lib.history("page-id").unwrap().len(), before + 1);
}

#[test]
fn sync_conflict_copies_are_flagged() {
    let (_d, mut lib) = lib();
    lib.create_page("", "Page", PageSettings::default())
        .unwrap();
    let svg = fs::read(lib.root().join("Page.svg")).unwrap();
    fs::write(
        lib.root()
            .join("Page.sync-conflict-20261021-120000-ABC.svg"),
        &svg,
    )
    .unwrap();
    fs::write(
        lib.root().join("Page (conflicted copy 2026-10-21).svg"),
        &svg,
    )
    .unwrap();
    let tree = lib.tree().unwrap();
    let flagged: Vec<_> = tree
        .children
        .iter()
        .filter(|n| n.conflict)
        .map(|n| &n.name)
        .collect();
    assert_eq!(flagged.len(), 2);
    assert!(
        !tree
            .children
            .iter()
            .find(|n| n.name == "Page")
            .unwrap()
            .conflict
    );
}

#[test]
fn saving_a_pdf_keeps_the_previous_version() {
    let (_d, mut lib) = lib();
    let rel = lib.add_pdf("", "Doc", b"version 1").unwrap();
    lib.save_pdf(&rel, b"version 2").unwrap();
    assert_eq!(lib.read_pdf(&rel).unwrap(), b"version 2");
    let history = fs::read_dir(lib.root().join(".ductus/history"))
        .unwrap()
        .count();
    assert_eq!(history, 1);
}
