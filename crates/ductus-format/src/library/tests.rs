use super::*;
use ductus_core::{Command, History, InkPoint, Stroke, StrokeStyle};

fn lib() -> (tempfile::TempDir, Library) {
    let dir = tempfile::tempdir().unwrap();
    let lib = Library::open(dir.path().join("Notes"), &dir.path().join("app")).unwrap();
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
    let trash: Vec<_> = fs::read_dir(lib.data_dir().join("trash"))
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
        &dir.path().join(".app"),
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

    let mut lib = Library::open(dir.path(), &dir.path().join(".app")).unwrap();
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
    let history = fs::read_dir(lib.data_dir().join("history"))
        .unwrap()
        .count();
    assert_eq!(history, 1);
}

#[test]
fn data_lives_outside_the_library_and_old_data_moves_there() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Notes");
    let app = dir.path().join("app");
    fs::create_dir_all(root.join(".ductus/history/page-id")).unwrap();
    fs::write(root.join(".ductus/history/page-id/1.svg"), "old").unwrap();
    fs::write(
        root.join(".ductus/quick-access.json"),
        r#"{"paths":["Math"]}"#,
    )
    .unwrap();
    fs::create_dir_all(root.join("Math")).unwrap();

    let lib = Library::open(&root, &app).unwrap();
    assert!(!root.join(".ductus").exists());
    assert!(lib.data_dir().starts_with(&app));
    assert_eq!(
        fs::read_to_string(lib.data_dir().join("library.txt")).unwrap(),
        root.to_string_lossy()
    );
    assert_eq!(lib.history("page-id").unwrap().len(), 1);
    assert_eq!(lib.favorites(), ["Math"]);

    lib.create_folder("", "Old").unwrap();
    lib.delete("Old").unwrap();
    assert!(
        lib.data_dir()
            .join("trash")
            .read_dir()
            .unwrap()
            .next()
            .is_some()
    );
    assert!(!root.join(".ductus").exists());
    // Another library has data of its own.
    let other = Library::open(dir.path().join("Other"), &app).unwrap();
    assert_ne!(other.data_dir(), lib.data_dir());
    assert!(other.favorites().is_empty());
}

#[test]
fn folder_colors_and_order_live_outside_the_library() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Notes");
    let app = dir.path().join("app");
    fs::create_dir_all(root.join("Uni/Math")).unwrap();
    fs::write(
        root.join("Uni/.ductus.json"),
        r##"{"color":"#1f4fd1","order":["Math"]}"##,
    )
    .unwrap();
    let lib = Library::open(&root, &app).unwrap();
    assert!(lib.folder_color("Uni").is_some());
    assert!(
        !root.join("Uni/.ductus.json").exists(),
        "an old file moves out"
    );

    lib.create_folder("Uni", "Art").unwrap();
    lib.rename("Uni", "Study").unwrap();
    assert!(
        lib.folder_color("Study").is_some(),
        "the color follows a rename"
    );
    let reopened = Library::open(&root, &app).unwrap();
    assert!(reopened.folder_color("Study").is_some());
    reopened.delete("Study").unwrap();
    assert!(reopened.folder_color("Study").is_none());
    let mut leftovers = Vec::new();
    for entry in fs::read_dir(&root).unwrap() {
        leftovers.push(entry.unwrap().file_name());
    }
    assert!(
        leftovers.is_empty(),
        "the library holds only notes: {leftovers:?}"
    );
}

#[test]
fn trash_restores_entries_where_they_were() {
    let (_d, mut lib) = lib();
    lib.create_folder("", "Math").unwrap();
    lib.create_page("Math", "Limits", PageSettings::default())
        .unwrap();
    lib.create_page("Math", "Series", PageSettings::default())
        .unwrap();
    lib.save_asset("Math/Limits", "img-1.png", b"png").unwrap();

    let id = lib.delete("Math/Limits").unwrap();
    let trash = lib.trash().unwrap();
    assert_eq!(
        trash.len(),
        1,
        "the pictures folder isn't an entry of its own"
    );
    assert_eq!(trash[0].id, id);
    assert_eq!(trash[0].original, "Math/Limits");
    assert_eq!(trash[0].kind, NodeKind::Page);

    assert_eq!(lib.restore(&id).unwrap(), "Math/Limits");
    assert!(lib.exists("Math/Limits"));
    assert_eq!(lib.read_asset("Math/Limits", "img-1.png").unwrap(), b"png");
    assert!(lib.trash().unwrap().is_empty());

    // Its folder is gone and its name is taken again: both are dealt with.
    let id = lib.delete("Math/Limits").unwrap();
    lib.delete("Math").unwrap();
    lib.create_folder("", "Math").unwrap();
    lib.create_page("Math", "Limits", PageSettings::default())
        .unwrap();
    assert_eq!(lib.restore(&id).unwrap(), "Math/Limits 2");
}

#[test]
fn trash_purges_and_reads_entries_from_before_the_index() {
    let (_d, mut lib) = lib();
    lib.create_page("", "Gone", PageSettings::default())
        .unwrap();
    let id = lib.delete("Gone").unwrap();
    lib.purge(&id).unwrap();
    assert!(lib.trash().unwrap().is_empty());

    let trash = lib.data_dir().join("trash");
    fs::write(
        trash.join("1700000000 Old notes.svg"),
        write_page(&page_with_stroke("x")),
    )
    .unwrap();
    let entries = lib.trash().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].original, "Old notes");
    assert_eq!(entries[0].deleted, 1_700_000_000_000);
    assert_eq!(lib.restore(&entries[0].id).unwrap(), "Old notes");

    lib.delete("Old notes").unwrap();
    lib.empty_trash().unwrap();
    assert!(lib.trash().unwrap().is_empty());
}

#[test]
fn earlier_versions_can_be_restored_or_copied() {
    let dir = tempfile::tempdir().unwrap();
    let mut lib = Library::with_policy(
        dir.path().join("Notes"),
        &dir.path().join("app"),
        HistoryPolicy {
            interval: Duration::ZERO,
            keep: 10,
        },
    )
    .unwrap();
    let rel = lib
        .create_page("", "Page", PageSettings::default())
        .unwrap();
    let first = page_with_stroke("page-id");
    lib.save_page(&rel, &first).unwrap();
    std::thread::sleep(Duration::from_millis(2));
    let mut second = page_with_stroke("page-id");
    second.document = Document::new();
    lib.save_page(&rel, &second).unwrap();

    let versions = lib.versions(&rel).unwrap();
    assert!(versions.len() >= 2, "versions: {versions:?}");
    assert!(versions[0].saved >= versions[1].saved, "newest first");
    // The newest version is the page with the stroke.
    lib.restore_version(&rel, versions[0].saved).unwrap();
    assert_eq!(lib.load_page(&rel).unwrap().document.strokes().len(), 1);
    assert_eq!(
        lib.versions(&rel).unwrap().len(),
        versions.len() + 1,
        "undo is a version"
    );

    let copy = lib
        .copy_version(&rel, versions[0].saved, "Page (earlier)")
        .unwrap();
    assert_eq!(copy, "Page (earlier)");
    let copied = lib.load_page(&copy).unwrap();
    assert_eq!(copied.document.strokes().len(), 1);
    assert_ne!(copied.id, "page-id");
}
