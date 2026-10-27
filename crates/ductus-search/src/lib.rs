//! The index is only a cache of the library: it can be deleted any time and is rebuilt from the files.

use ductus_format::{Library, NodeKind};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::UNIX_EPOCH;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Db(rusqlite::Error),
    Io(std::io::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Db(e) => write!(f, "search index: {e}"),
            Error::Io(e) => write!(f, "search index: {e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Db(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

/// Bumped when what is indexed changes; older indexes are rebuilt.
const SCHEMA: i64 = 1;
/// Deeper folders are skipped, which also stops symlink loops.
const MAX_DEPTH: usize = 48;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Title,
    Text,
    Math,
    Ink,
    Pdf,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Title => "title",
            Kind::Text => "text",
            Kind::Math => "math",
            Kind::Ink => "ink",
            Kind::Pdf => "pdf",
        }
    }

    fn from_name(name: &str) -> Kind {
        match name {
            "text" => Kind::Text,
            "math" => Kind::Math,
            "ink" => Kind::Ink,
            "pdf" => Kind::Pdf,
            _ => Kind::Title,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub kind: Kind,
    pub text: String,
    pub page: Option<usize>,
    pub rect: Option<(f64, f64, f64, f64)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub path: String,
    pub kind: Kind,
    pub page: Option<usize>,
    pub rect: Option<(f64, f64, f64, f64)>,
    pub snippet: String,
}

/// Where a search looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope<'a> {
    /// Names and contents of every page and PDF.
    All,
    /// Page and PDF names only, not the folders they are in.
    Names,
    /// The contents of one page.
    Page(&'a str),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
}

pub struct Index {
    conn: Connection,
    /// The PDF text read last: a page being written on is saved again and again.
    pdf_memo: Option<PdfMemo>,
}

struct PdfMemo {
    file: PathBuf,
    stamp: i64,
    pages: usize,
    entries: Vec<Entry>,
}

pub fn markdown_text(markdown: &str) -> String {
    use pulldown_cmark::{Event, Parser};
    let mut out = String::new();
    for event in Parser::new(markdown) {
        match event {
            Event::Text(t) | Event::Code(t) => out.push_str(&t),
            Event::SoftBreak | Event::HardBreak | Event::End(_) => out.push(' '),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn modified(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as i64)
}

fn pdf_entries(file: &ductus_pdf::PdfFile) -> Vec<Entry> {
    (0..file.page_count())
        .filter_map(|i| {
            let text = file.page_text(i).ok()?;
            let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
            (!text.is_empty()).then_some(Entry {
                kind: Kind::Pdf,
                text,
                page: Some(i),
                rect: None,
            })
        })
        .collect()
}

fn fts_query(query: &str) -> String {
    query
        .split_whitespace()
        .map(|w| format!("\"{}\"", w.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" ")
}

impl Index {
    pub fn open(db: &Path) -> Result<Self> {
        if let Some(dir) = db.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(db)?;
        // Readers (searches) never wait for the indexer.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let index = Self {
            conn,
            pdf_memo: None,
        };
        index.ensure_schema()?;
        Ok(index)
    }

    fn ensure_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS meta(key TEXT PRIMARY KEY, value INTEGER);",
        )?;
        let version: Option<i64> = self
            .conn
            .query_row("SELECT value FROM meta WHERE key = 'schema'", [], |r| {
                r.get(0)
            })
            .optional()?;
        if version != Some(SCHEMA) {
            self.conn.execute_batch(
                "DROP TABLE IF EXISTS files;
                 DROP TABLE IF EXISTS entries;
                 CREATE TABLE files(path TEXT PRIMARY KEY, stamp INTEGER NOT NULL);
                 CREATE VIRTUAL TABLE entries USING fts5(
                     text, path UNINDEXED, kind UNINDEXED, page UNINDEXED,
                     x UNINDEXED, y UNINDEXED, w UNINDEXED, h UNINDEXED,
                     tokenize = 'trigram remove_diacritics 1');",
            )?;
            self.conn.execute(
                "INSERT OR REPLACE INTO meta(key, value) VALUES ('schema', ?1)",
                [SCHEMA],
            )?;
        }
        Ok(())
    }

    pub fn set_entries(&mut self, path: &str, stamp: i64, entries: &[Entry]) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM entries WHERE path = ?1", [path])?;
        {
            let mut insert = tx.prepare(
                "INSERT INTO entries(text, path, kind, page, x, y, w, h)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?;
            for e in entries.iter().filter(|e| !e.text.trim().is_empty()) {
                let (x, y, w, h) = match e.rect {
                    Some((x, y, w, h)) => (Some(x), Some(y), Some(w), Some(h)),
                    None => (None, None, None, None),
                };
                insert.execute(params![
                    e.text,
                    path,
                    e.kind.name(),
                    e.page.map(|p| p as i64),
                    x,
                    y,
                    w,
                    h
                ])?;
            }
        }
        tx.execute(
            "INSERT OR REPLACE INTO files(path, stamp) VALUES (?1, ?2)",
            params![path, stamp],
        )?;
        tx.commit()?;
        Ok(())
    }

    fn remove(&mut self, path: &str) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM entries WHERE path = ?1", [path])?;
        tx.execute("DELETE FROM files WHERE path = ?1", [path])?;
        tx.commit()?;
        Ok(())
    }

    fn stamp_of(&self, path: &str) -> Result<Option<i64>> {
        Ok(self
            .conn
            .query_row("SELECT stamp FROM files WHERE path = ?1", [path], |r| {
                r.get(0)
            })
            .optional()?)
    }

    fn page_entries(&mut self, library: &Library, rel: &str, saved: bool) -> Vec<Entry> {
        let Ok(page) = library.load_page(rel) else {
            return Vec::new();
        };
        let doc = &page.document;
        let mut out = Vec::new();
        for text in doc.texts() {
            out.push(Entry {
                kind: Kind::Text,
                text: markdown_text(&text.markdown),
                page: None,
                rect: Some((text.x, text.y, text.width, text.height)),
            });
        }
        for math in doc.maths() {
            out.push(Entry {
                kind: Kind::Math,
                text: math.source.clone(),
                page: None,
                rect: Some((math.x, math.y, math.width, math.height)),
            });
        }
        for line in &page.recognized {
            out.push(Entry {
                kind: Kind::Ink,
                text: line.text.clone(),
                page: None,
                rect: Some(line.rect),
            });
        }
        if let Some(name) = &page.pdf {
            out.extend(self.pdf_text(&library.assets_dir(rel).join(name), saved));
        }
        out
    }

    /// `saved`: Ductus wrote the file, which changes ink but not text, unless pages were added.
    fn pdf_text(&mut self, path: &Path, saved: bool) -> Vec<Entry> {
        let stamp = modified(path);
        if let Some(memo) = &self.pdf_memo {
            if memo.file == path && memo.stamp == stamp {
                return memo.entries.clone();
            }
        }
        let Ok(file) = std::fs::read(path)
            .map_err(|_| ())
            .and_then(|bytes| ductus_pdf::PdfFile::open(bytes).map_err(|_| ()))
        else {
            return Vec::new();
        };
        let entries = match self.pdf_memo.take() {
            Some(memo) if saved && memo.file == path && memo.pages == file.page_count() => {
                memo.entries
            }
            _ => pdf_entries(&file),
        };
        self.pdf_memo = Some(PdfMemo {
            file: path.to_owned(),
            stamp,
            pages: file.page_count(),
            entries: entries.clone(),
        });
        entries
    }

    /// `saved`: just saved by Ductus, so indexed again even if the timestamp looks unchanged.
    pub fn index_file(&mut self, library: &Library, path: &str, saved: bool) -> Result<()> {
        let is_pdf =
            ductus_format::library::pdf_stem(path.rsplit('/').next().unwrap_or(path)).is_some();
        let file = if is_pdf {
            library.abs(path)
        } else {
            library.abs(&format!(
                "{path}.{}",
                ductus_format::library::PAGE_EXTENSION
            ))
        };
        if !file.is_file() {
            return self.remove(path);
        }
        let stamp = modified(&file);
        if !saved && self.stamp_of(path)? == Some(stamp) {
            return Ok(());
        }
        let name = path.rsplit('/').next().unwrap_or(path);
        let title = ductus_format::library::pdf_stem(name).unwrap_or(name);
        let folders = path
            .rsplit_once('/')
            .map_or("", |(f, _)| f)
            .replace('/', " ");
        let mut entries = vec![Entry {
            kind: Kind::Title,
            text: format!("{title} {folders}"),
            page: None,
            rect: None,
        }];
        if is_pdf {
            entries.extend(self.pdf_text(&file, saved));
        } else {
            entries.extend(self.page_entries(library, path, saved));
        }
        self.set_entries(path, stamp, &entries)
    }

    pub fn update(
        &mut self,
        library: &Library,
        cancel: &AtomicBool,
        mut progress: impl FnMut(Progress),
    ) -> Result<()> {
        let mut files = Vec::new();
        collect(library, "", 0, &mut files);
        let total = files.len();
        for (done, path) in files.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                return Ok(());
            }
            self.index_file(library, path, false)?;
            progress(Progress {
                done: done + 1,
                total,
            });
        }
        let known: Vec<String> = {
            let mut stmt = self.conn.prepare("SELECT path FROM files")?;
            stmt.query_map([], |r| r.get(0))?
                .collect::<std::result::Result<_, _>>()?
        };
        let present: std::collections::HashSet<&String> = files.iter().collect();
        for path in known.iter().filter(|p| !present.contains(p)) {
            self.remove(path)?;
        }
        Ok(())
    }

    pub fn search(&self, query: &str, scope: Scope, limit: usize) -> Result<Vec<Hit>> {
        let words: Vec<&str> = query.split_whitespace().collect();
        if words.is_empty() {
            return Ok(Vec::new());
        }
        if scope == Scope::Names {
            return self.search_names(&words, limit);
        }
        let short = words.iter().any(|w| w.chars().count() < 3);
        // Trigrams need three characters; shorter words are matched with LIKE.
        let (mut conds, mut args): (Vec<String>, Vec<String>) = if short {
            let conds = (1..=words.len())
                .map(|i| format!("text LIKE ?{i} ESCAPE '\\'"))
                .collect();
            let args = words
                .iter()
                .map(|w| {
                    let w = w
                        .replace('\\', "\\\\")
                        .replace('%', "\\%")
                        .replace('_', "\\_");
                    format!("%{w}%")
                })
                .collect();
            (conds, args)
        } else {
            (vec!["entries MATCH ?1".to_owned()], vec![fts_query(query)])
        };
        if let Scope::Page(path) = scope {
            args.push(path.to_owned());
            conds.push(format!("path = ?{} AND kind != 'title'", args.len()));
        }
        let sql = if short {
            format!(
                "SELECT path, kind, page, x, y, w, h, text FROM entries WHERE {}
                 ORDER BY kind != 'title' LIMIT {limit}",
                conds.join(" AND ")
            )
        } else {
            format!(
                "SELECT path, kind, page, x, y, w, h,
                        snippet(entries, 0, char(1), char(2), '…', 48)
                 FROM entries WHERE {}
                 ORDER BY kind != 'title', bm25(entries) LIMIT {limit}",
                conds.join(" AND ")
            )
        };
        let mut stmt = self.conn.prepare(&sql)?;
        let hits = stmt
            .query_map(rusqlite::params_from_iter(args.iter()), |r| {
                let rect = match (
                    r.get::<_, Option<f64>>(3)?,
                    r.get::<_, Option<f64>>(4)?,
                    r.get::<_, Option<f64>>(5)?,
                    r.get::<_, Option<f64>>(6)?,
                ) {
                    (Some(x), Some(y), Some(w), Some(h)) => Some((x, y, w, h)),
                    _ => None,
                };
                Ok(Hit {
                    path: r.get(0)?,
                    kind: Kind::from_name(&r.get::<_, String>(1)?),
                    page: r.get::<_, Option<i64>>(2)?.map(|p| p as usize),
                    rect,
                    snippet: r.get(7)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(if short {
            hits.into_iter()
                .map(|h| Hit {
                    snippet: mark(&h.snippet, &words),
                    ..h
                })
                .collect()
        } else {
            hits
        })
    }

    fn search_names(&self, words: &[&str], limit: usize) -> Result<Vec<Hit>> {
        let words: Vec<String> = words.iter().map(|w| w.to_lowercase()).collect();
        let mut stmt = self.conn.prepare("SELECT path FROM files")?;
        let mut hits: Vec<(bool, String, String)> = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .filter_map(|p| {
                let path = p.ok()?;
                let file = path.rsplit('/').next().unwrap_or(&path);
                let name = ductus_format::library::pdf_stem(file)
                    .unwrap_or(file)
                    .to_lowercase();
                words
                    .iter()
                    .all(|w| name.contains(w.as_str()))
                    .then(|| (!name.starts_with(words[0].as_str()), name, path))
            })
            .collect();
        // Names starting with the query first, then alphabetical.
        hits.sort();
        Ok(hits
            .into_iter()
            .take(limit)
            .map(|(_, _, path)| Hit {
                path,
                kind: Kind::Title,
                page: None,
                rect: None,
                snippet: String::new(),
            })
            .collect())
    }
}

fn collect(library: &Library, folder: &str, depth: usize, out: &mut Vec<String>) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(nodes) = library.list(folder) else {
        return;
    };
    for node in nodes.iter() {
        match node.kind {
            NodeKind::Page | NodeKind::Pdf => out.push(node.path.clone()),
            NodeKind::Folder => collect(library, &node.path, depth + 1, out),
            NodeKind::File => {}
        }
    }
}

fn mark(text: &str, words: &[&str]) -> String {
    let lower = text.to_lowercase();
    let first = words
        .first()
        .and_then(|w| lower.find(&w.to_lowercase()))
        .unwrap_or(0);
    let chars: Vec<char> = text.chars().collect();
    let at = text[..first].chars().count();
    let start = at.saturating_sub(30);
    let end = (at + 50).min(chars.len());
    let mut excerpt: String = chars[start..end].iter().collect();
    for w in words {
        let lw = w.to_lowercase();
        if let Some(i) = excerpt.to_lowercase().find(&lw) {
            if excerpt.is_char_boundary(i) && excerpt.is_char_boundary(i + w.len()) {
                excerpt = format!(
                    "{}\u{1}{}\u{2}{}",
                    &excerpt[..i],
                    &excerpt[i..i + w.len()],
                    &excerpt[i + w.len()..]
                );
            }
        }
    }
    let mut out = String::new();
    if start > 0 {
        out.push('…');
    }
    out.push_str(&excerpt);
    if end < chars.len() {
        out.push('…');
    }
    out
}

pub fn index_path(cache_dir: &Path, library_root: &Path) -> PathBuf {
    cache_dir.join(format!(
        "search-{}.sqlite",
        ductus_format::library_key(library_root)
    ))
}

#[cfg(test)]
mod tests;
