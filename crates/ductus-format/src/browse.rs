//! Libraries can be whole existing folder trees, so nothing here reads more than the visible rows need.

use crate::Error;
use crate::library::{APP_DIR, Library, Node, NodeKind, write_atomic};
use serde::{Deserialize, Serialize};
use std::fs;
use std::rc::Rc;
use std::time::{Duration, SystemTime};

/// Deeper folders are ignored, which also stops symlink loops.
const MAX_DEPTH: usize = 48;
/// Coarse file timestamps could hide a second change this soon after the first.
const SETTLE: Duration = Duration::from_secs(2);
const QUICK_ACCESS_FILE: &str = "quick-access.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Filter {
    pub pages: bool,
    pub pdfs: bool,
    pub other_files: bool,
    pub hide_empty_folders: bool,
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            pages: true,
            pdfs: true,
            other_files: false,
            hide_empty_folders: false,
        }
    }
}

impl Filter {
    pub fn shows(&self, kind: NodeKind) -> bool {
        match kind {
            NodeKind::Folder => true,
            NodeKind::Page => self.pages,
            NodeKind::Pdf => self.pdfs,
            NodeKind::File => self.other_files,
        }
    }
}

pub(crate) struct Listing {
    modified: SystemTime,
    read_at: SystemTime,
    nodes: Rc<Vec<Node>>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct QuickAccess {
    #[serde(default)]
    paths: Vec<String>,
}

fn depth(rel: &str) -> usize {
    rel.split('/').filter(|p| !p.is_empty()).count()
}

fn moved(path: &str, from: &str, to: &str) -> Option<String> {
    if path == from {
        Some(to.to_owned())
    } else {
        path.strip_prefix(&format!("{from}/"))
            .map(|rest| format!("{to}/{rest}"))
    }
}

impl Library {
    pub fn list(&self, folder: &str) -> Result<Rc<Vec<Node>>, Error> {
        let modified = fs::metadata(self.abs(folder))?.modified().ok();
        if let Some(modified) = modified {
            if let Some(cached) = self.listings.borrow().get(folder) {
                let settled = cached
                    .read_at
                    .duration_since(modified)
                    .is_ok_and(|d| d >= SETTLE);
                if cached.modified == modified && settled {
                    return Ok(Rc::clone(&cached.nodes));
                }
            }
        }
        let nodes = Rc::new(self.read_folder(folder)?);
        if let Some(modified) = modified {
            self.listings.borrow_mut().insert(
                folder.to_owned(),
                Listing {
                    modified,
                    read_at: SystemTime::now(),
                    nodes: Rc::clone(&nodes),
                },
            );
        }
        Ok(nodes)
    }

    pub fn visible(&self, folder: &str, filter: Filter) -> Vec<Node> {
        let Ok(nodes) = self.list(folder) else {
            return Vec::new();
        };
        nodes
            .iter()
            .filter(|n| self.is_shown(n, filter))
            .cloned()
            .collect()
    }

    pub fn has_visible(&self, folder: &str, filter: Filter) -> bool {
        self.list(folder)
            .is_ok_and(|nodes| nodes.iter().any(|n| self.is_shown(n, filter)))
    }

    fn is_shown(&self, node: &Node, filter: Filter) -> bool {
        match node.kind {
            NodeKind::Folder => !filter.hide_empty_folders || self.holds_files(&node.path, filter),
            kind => filter.shows(kind),
        }
    }

    fn holds_files(&self, folder: &str, filter: Filter) -> bool {
        if depth(folder) > MAX_DEPTH {
            return false;
        }
        let Ok(nodes) = self.list(folder) else {
            return false;
        };
        nodes
            .iter()
            .any(|n| n.kind != NodeKind::Folder && filter.shows(n.kind))
            || nodes
                .iter()
                .any(|n| n.kind == NodeKind::Folder && self.holds_files(&n.path, filter))
    }

    pub fn notebook_folders(&self, under: &str, filter: Filter) -> Vec<String> {
        fn walk(lib: &Library, folder: &str, filter: Filter, out: &mut Vec<String>) {
            if depth(folder) > MAX_DEPTH {
                return;
            }
            let Ok(nodes) = lib.list(folder) else {
                return;
            };
            if nodes
                .iter()
                .any(|n| n.kind != NodeKind::Folder && filter.shows(n.kind))
            {
                out.push(folder.to_owned());
            }
            for n in nodes.iter().filter(|n| n.kind == NodeKind::Folder) {
                walk(lib, &n.path, filter, out);
            }
        }
        let mut out = Vec::new();
        walk(self, under, filter, &mut out);
        out
    }

    pub fn first_document(&self) -> Option<String> {
        fn find(lib: &Library, folder: &str) -> Option<String> {
            if depth(folder) > MAX_DEPTH {
                return None;
            }
            let nodes = lib.list(folder).ok()?;
            nodes.iter().find_map(|n| match n.kind {
                NodeKind::Page | NodeKind::Pdf => Some(n.path.clone()),
                NodeKind::Folder => find(lib, &n.path),
                NodeKind::File => None,
            })
        }
        find(self, "")
    }

    fn quick_access_file(&self) -> std::path::PathBuf {
        self.root().join(APP_DIR).join(QUICK_ACCESS_FILE)
    }

    fn stored_quick_access(&self) -> Vec<String> {
        fs::read_to_string(self.quick_access_file())
            .ok()
            .and_then(|s| serde_json::from_str::<QuickAccess>(&s).ok())
            .map(|q| q.paths)
            .unwrap_or_default()
    }

    /// Entries deleted or moved by other programs are left out.
    pub fn quick_access(&self) -> Vec<String> {
        self.stored_quick_access()
            .into_iter()
            .filter(|p| self.exists(p))
            .collect()
    }

    pub fn set_quick_access(&self, paths: &[String]) -> Result<(), Error> {
        fs::create_dir_all(self.root().join(APP_DIR))?;
        let json = serde_json::to_string_pretty(&QuickAccess {
            paths: paths.to_vec(),
        })?;
        write_atomic(&self.quick_access_file(), json.as_bytes())
    }

    pub fn set_pinned(&self, rel: &str, pinned: bool) -> Result<(), Error> {
        let mut paths = self.stored_quick_access();
        paths.retain(|p| p != rel);
        if pinned {
            paths.push(rel.to_owned());
        }
        self.set_quick_access(&paths)
    }

    pub(crate) fn quick_access_moved(&self, from: &str, to: &str) -> Result<(), Error> {
        let paths = self.stored_quick_access();
        if !paths.iter().any(|p| moved(p, from, to).is_some()) {
            return Ok(());
        }
        let paths: Vec<String> = paths
            .iter()
            .map(|p| moved(p, from, to).unwrap_or_else(|| p.clone()))
            .collect();
        self.set_quick_access(&paths)
    }

    pub(crate) fn quick_access_removed(&self, rel: &str) -> Result<(), Error> {
        let paths = self.stored_quick_access();
        let kept: Vec<String> = paths
            .iter()
            .filter(|p| moved(p, rel, "").is_none())
            .cloned()
            .collect();
        if kept.len() == paths.len() {
            return Ok(());
        }
        self.set_quick_access(&kept)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ductus_core::PageSettings;

    fn library() -> (tempfile::TempDir, Library) {
        let dir = tempfile::tempdir().unwrap();
        let lib = Library::open(dir.path()).unwrap();
        (dir, lib)
    }

    fn touch(lib: &Library, rel: &str) {
        let path = lib.abs(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"x").unwrap();
    }

    #[test]
    fn filter_can_hide_folders_without_documents() {
        let (_dir, mut lib) = library();
        touch(&lib, "Uni/Admin/form.docx");
        touch(&lib, "Uni/Math/Week 1/notes.pdf");
        lib.create_page("Uni/Math", "Summary", PageSettings::default())
            .unwrap();
        let hide = Filter {
            hide_empty_folders: true,
            ..Filter::default()
        };
        let names = |f: Filter| -> Vec<String> {
            lib.visible("Uni", f).into_iter().map(|n| n.name).collect()
        };
        assert_eq!(names(Filter::default()), ["Admin", "Math"]);
        assert_eq!(names(hide), ["Math"]);
        let pdfs_only = Filter {
            pages: false,
            ..hide
        };
        assert!(lib.has_visible("Uni/Math", pdfs_only));
        assert_eq!(lib.visible("Uni/Math", pdfs_only).len(), 1, "only Week 1");
        assert_eq!(
            lib.notebook_folders("", Filter::default()),
            ["Uni/Math", "Uni/Math/Week 1"]
        );
        assert_eq!(
            lib.notebook_folders("Uni/Math/Week 1", Filter::default()),
            ["Uni/Math/Week 1"]
        );
    }

    #[test]
    fn only_ductus_svgs_are_pages() {
        let (_dir, mut lib) = library();
        let plain = r#"<svg xmlns="http://www.w3.org/2000/svg"><path d="M0,0L1,1"/></svg>"#;
        fs::create_dir_all(lib.abs("Thesis/base")).unwrap();
        fs::write(lib.abs("Thesis/base/logo.svg"), plain).unwrap();
        lib.create_page("Thesis", "Ideas", PageSettings::default())
            .unwrap();
        let kinds: Vec<_> = lib
            .list("Thesis/base")
            .unwrap()
            .iter()
            .map(|n| (n.name.clone(), n.path.clone(), n.kind))
            .collect();
        assert_eq!(
            kinds,
            [(
                "logo.svg".to_owned(),
                "Thesis/base/logo.svg".to_owned(),
                NodeKind::File
            )]
        );
        assert_eq!(lib.list("Thesis").unwrap()[1].kind, NodeKind::Page);
        let notes = Filter {
            pdfs: false,
            ..Filter::default()
        };
        assert_eq!(lib.notebook_folders("", notes), ["Thesis"]);
        assert!(lib.exists("Thesis/base/logo.svg"));
        lib.delete("Thesis/base/logo.svg").unwrap();
        assert!(!lib.abs("Thesis/base/logo.svg").exists());
    }

    #[test]
    fn listings_follow_changes() {
        let (_dir, lib) = library();
        touch(&lib, "A/one.pdf");
        assert_eq!(lib.list("A").unwrap().len(), 1);
        touch(&lib, "A/two.pdf");
        assert_eq!(lib.list("A").unwrap().len(), 2);
    }

    #[test]
    fn quick_access_follows_renames_and_deletes() {
        let (_dir, mut lib) = library();
        lib.create_folder("", "Math").unwrap();
        let page = lib
            .create_page("Math", "Limits", PageSettings::default())
            .unwrap();
        lib.set_pinned(&page, true).unwrap();
        lib.set_pinned("Math", true).unwrap();
        assert_eq!(lib.quick_access(), ["Math/Limits", "Math"]);
        lib.rename("Math", "Analysis").unwrap();
        assert_eq!(lib.quick_access(), ["Analysis/Limits", "Analysis"]);
        lib.delete("Analysis/Limits").unwrap();
        assert_eq!(lib.quick_access(), ["Analysis"]);
        lib.set_pinned("Analysis", false).unwrap();
        assert!(lib.quick_access().is_empty());
    }

    #[test]
    fn pictures_are_saved_linked_and_retired() {
        let (_dir, mut lib) = library();
        let rel = lib
            .create_page("", "Limits", PageSettings::default())
            .unwrap();
        let mut page = lib.load_page(&rel).unwrap();
        lib.save_asset(&rel, "img-a.png", b"png-a").unwrap();
        lib.save_asset(&rel, "img-b.png", b"png-b").unwrap();
        let image = |source: &str| ductus_core::Image {
            id: ductus_core::StrokeId(0),
            source: source.into(),
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        page.document = page.document.with_images([image("img-a.png")]);
        lib.save_page(&rel, &page).unwrap();
        let svg = fs::read_to_string(lib.abs("Limits.svg")).unwrap();
        assert!(svg.contains(r#"href="Limits.assets/img-a.png""#));
        assert!(!lib.abs("Limits.assets/img-b.png").exists());
        assert!(
            lib.root()
                .join(APP_DIR)
                .join("history")
                .join(&page.id)
                .join("assets/img-b.png")
                .is_file()
        );
        lib.rename("Limits", "Grenzwerte").unwrap();
        let svg = fs::read_to_string(lib.abs("Grenzwerte.svg")).unwrap();
        assert!(svg.contains(r#"href="Grenzwerte.assets/img-a.png""#));
        assert_eq!(lib.read_asset("Grenzwerte", "img-a.png").unwrap(), b"png-a");
    }
}
