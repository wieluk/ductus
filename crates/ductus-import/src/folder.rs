//! A whole folder as one notebook: a OneNote notebook folder, or notes from other apps.

use crate::{Entry, Import};
use ductus_format::Error;
use std::path::{Path, PathBuf};

/// OneNote's deleted pages; they stay behind.
const RECYCLE_BIN: &str = "OneNote_RecycleBin";

pub(crate) fn read(dir: &Path) -> Result<Import, Error> {
    let mut warnings = Vec::new();
    match folder(dir, true, &mut warnings)? {
        Some(entry) => Ok(Import { entry, warnings }),
        None => Err(Error::Invalid(
            "There are no Xournal++, Rnote or OneNote notes in this folder.".into(),
        )),
    }
}

/// The folder's notes, or nothing when it has none.
fn folder(dir: &Path, notebook: bool, warnings: &mut Vec<String>) -> Result<Option<Entry>, Error> {
    // OneNote's table of contents knows the section order and colours.
    if let Some(toc) = files(dir)?.into_iter().find(|f| is(f, "onetoc2"))
        && let Ok(mut import) = crate::onenote::read(&toc)
    {
        if let Entry::Folder { notebook: n, .. } = &mut import.entry {
            *n = notebook;
        }
        warnings.append(&mut import.warnings);
        return Ok(Some(import.entry));
    }
    let name = dir
        .canonicalize()
        .ok()
        .as_deref()
        .and_then(Path::file_name)
        .map_or_else(|| "Imported".into(), |n| n.to_string_lossy().into_owned());
    let entries = entries(dir, warnings)?;
    Ok((!entries.is_empty()).then_some(Entry::Folder {
        name,
        notebook,
        color: None,
        entries,
    }))
}

fn is(path: &Path, extension: &str) -> bool {
    crate::extension(path).as_deref() == Some(extension)
}

/// Sorted by name, like a file manager shows them.
fn files(dir: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)?
        .flatten()
        .map(|e| e.path())
        .collect();
    paths.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
    Ok(paths)
}

/// Sections and notes in `dir`; subfolders with notes become folders of their own.
fn entries(dir: &Path, warnings: &mut Vec<String>) -> Result<Vec<Entry>, Error> {
    let mut out = Vec::new();
    for path in files(dir)? {
        let Some(file_name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            continue;
        };
        if file_name.starts_with('.') && !is(&path, "onetoc2") || file_name == RECYCLE_BIN {
            continue;
        }
        // A linked folder can lead back up: links are followed as files only.
        let linked = path
            .symlink_metadata()
            .is_ok_and(|m| m.file_type().is_symlink());
        if path.is_dir() && !linked {
            out.extend(folder(&path, false, warnings)?);
        } else if crate::supports(&path) && !is(&path, "onetoc2") {
            // One unreadable file shouldn't stop the rest.
            match crate::read(&path) {
                Ok(import) => {
                    out.push(import.entry);
                    warnings.extend(import.warnings);
                }
                Err(e) => warnings.push(format!("{file_name} was left out: {e}")),
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_and_subfolders_become_a_notebook() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Physics");
        let xopp = r##"<xournal><page width="595.27559" height="841.88976"><layer>
            <stroke tool="pen" color="#000000ff" width="1.4">10 10 50 50</stroke></layer></page></xournal>"##;
        std::fs::create_dir_all(root.join("Week 2")).unwrap();
        std::fs::create_dir_all(root.join(RECYCLE_BIN)).unwrap();
        std::fs::create_dir_all(root.join("Empty")).unwrap();
        std::fs::write(root.join("Week 1.xopp"), xopp).unwrap();
        std::fs::write(root.join("Week 2/Lab.xopp"), xopp).unwrap();
        std::fs::write(root.join(RECYCLE_BIN).join("Old.xopp"), xopp).unwrap();
        std::fs::write(root.join("broken.rnote"), "not gzip").unwrap();
        std::fs::write(root.join("readme.txt"), "ignored").unwrap();

        let import = read(&root).unwrap();
        assert_eq!(import.warnings.len(), 1, "{:?}", import.warnings);
        assert!(import.warnings[0].starts_with("broken.rnote"));
        let Entry::Folder {
            name,
            notebook,
            entries,
            ..
        } = &import.entry
        else {
            panic!("a folder");
        };
        assert_eq!((name.as_str(), *notebook), ("Physics", true));
        let names: Vec<&str> = entries
            .iter()
            .map(|e| match e {
                Entry::Page(p) => p.name.as_str(),
                Entry::Folder { name, .. } => name.as_str(),
                Entry::File { name, .. } => name.as_str(),
            })
            .collect();
        assert_eq!(names, ["Week 1", "Week 2"]);
    }

    #[test]
    fn folders_without_notes_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("readme.txt"), "ignored").unwrap();
        assert!(read(dir.path()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_linking_back_to_itself_ends() {
        let dir = tempfile::tempdir().unwrap();
        let notes = dir.path().join("Notes");
        std::fs::create_dir(&notes).unwrap();
        let xopp = r##"<xournal><page width="595.27559" height="841.88976"><layer>
            <stroke tool="pen" color="#000000ff" width="1.4">10 10 50 50</stroke></layer></page></xournal>"##;
        std::fs::write(notes.join("Week 1.xopp"), xopp).unwrap();
        std::os::unix::fs::symlink(&notes, notes.join("loop")).unwrap();
        assert!(read(&notes).is_ok());
    }
}
