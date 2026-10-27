#[cxx_qt::bridge]
pub mod qobject {
    #[derive(Debug, Clone, Copy)]
    struct PointerSample {
        x: f64,
        y: f64,
        pressure: f64,
        has_pressure: bool,
        tilt_x: f64,
        tilt_y: f64,
        time_ms: u32,
    }

    /// `layer`: 0 = base, 1 = live overlay.
    #[derive(Debug, Clone, Copy)]
    struct TileInfo {
        layer: u8,
        x: i32,
        y: i32,
        generation: u64,
        rect_x: f64,
        rect_y: f64,
        rect_w: f64,
        rect_h: f64,
    }

    #[derive(Debug, Default)]
    struct TreeRow {
        depth: u32,
        is_folder: bool,
        is_pdf: bool,
        is_file: bool,
        name: String,
        path: String,
        subtitle: String,
        color: String,
        has_children: bool,
        expanded: bool,
        conflict: bool,
        pinned: bool,
        /// A shortcut under Favorites or Recent.
        quick: bool,
        /// Notebooks view: "favorites", "recent" or "notebooks".
        section: String,
        is_notebook: bool,
        /// A notebook folder's name under Notebooks.
        notebook_name: String,
        /// Stands in for an empty notebook's pages; `path` is the notebook's.
        placeholder: bool,
    }

    /// Something deleted; `deleted` is in milliseconds since the epoch.
    #[derive(Debug, Default)]
    struct TrashRow {
        id: String,
        name: String,
        /// The folder it was in, as "Math › Analysis"; "" for the top level.
        location: String,
        /// "page", "pdf", "folder" or "file".
        kind: String,
        deleted: u64,
    }

    #[derive(Debug, Default)]
    struct TextRow {
        id: u64,
        markdown: String,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        scale: f64,
        selected: bool,
    }

    /// `w` < 0: no box; `page` -1: no page.
    #[derive(Debug, Default)]
    struct SearchHit {
        path: String,
        title: String,
        location: String,
        kind: String,
        page: i64,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        /// The match is between \u{1} and \u{2}.
        snippet: String,
    }

    #[derive(Debug, Default)]
    struct RecognitionResult {
        page: String,
        key: String,
        text: String,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        lang: String,
    }

    #[derive(Debug, Default)]
    struct LineJob {
        key: String,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        png: Vec<u8>,
    }

    extern "Rust" {
        type RecognitionHandle;
        fn open_recognizer() -> Box<RecognitionHandle>;
        /// `kind`: "off", "server" or "fixed:<text>".
        fn configure(self: &RecognitionHandle, kind: &str, url: &str, model: &str, key: &str);
        #[allow(clippy::too_many_arguments)]
        fn submit(
            self: &RecognitionHandle,
            page: &str,
            key: &str,
            x: f64,
            y: f64,
            w: f64,
            h: f64,
            png: &[u8],
            lang: &str,
        );
        fn pending(self: &RecognitionHandle) -> usize;
        fn take_results(self: &RecognitionHandle) -> Vec<RecognitionResult>;
        fn last_error(self: &RecognitionHandle) -> String;
    }

    #[derive(Debug, Default)]
    struct UpdateState {
        busy: bool,
        checked: bool,
        latest: String,
        notes: String,
        page: String,
        error: String,
        received: u64,
        total: u64,
        file: String,
    }

    /// `root` is the page or folder created; `page` the first page in it, if any.
    #[derive(Debug, Default)]
    struct ImportResult {
        root: String,
        page: String,
        warnings: String,
    }

    /// An import that has been read; `kind` is "notebook", "section", "page" or "file".
    #[derive(Debug, Default)]
    struct ImportSummary {
        ready: bool,
        error: String,
        kind: String,
        name: String,
        pages: u32,
        sections: u32,
    }

    #[derive(Debug, Default)]
    struct ConvertState {
        busy: bool,
        done: usize,
        total: usize,
        result: String,
        error: String,
    }

    #[derive(Debug, Default)]
    struct ModelRow {
        id: String,
        title: String,
        size: u64,
        installed: bool,
    }

    /// A handwriting language: its `hwr/lang` code, English and own name, the model reading it.
    #[derive(Debug, Default)]
    struct LanguageRow {
        code: String,
        english: String,
        native: String,
        model: String,
    }

    /// `busy` is the id of the model being downloaded, or empty.
    #[derive(Debug, Default)]
    struct ModelsState {
        busy: String,
        received: u64,
        total: u64,
        error: String,
    }

    extern "Rust" {
        type ConvertHandle;
        fn open_converter() -> Box<ConvertHandle>;
        fn start(
            self: &ConvertHandle,
            kind: &str,
            url: &str,
            model: &str,
            key: &str,
            lang: &str,
            jobs: Vec<LineJob>,
        );
        fn state(self: &ConvertHandle) -> ConvertState;

        type ImportHandle;
        fn open_importer() -> Box<ImportHandle>;
        fn can_import(file: &str) -> bool;
        fn start(self: &ImportHandle, file: &str);
        fn busy(self: &ImportHandle) -> bool;
        fn summary(self: &ImportHandle) -> ImportSummary;
        fn discard(self: &ImportHandle);

        type ModelsHandle;
        fn open_models() -> Box<ModelsHandle>;
        fn model_rows(dir: &str) -> Vec<ModelRow>;
        fn remove_model(dir: &str, id: &str) -> String;
        fn languages() -> Vec<LanguageRow>;
        fn text_models(langs: &str) -> Vec<String>;
        fn locale_language(locale: &str) -> String;
        fn install(self: &ModelsHandle, dir: &str, id: &str);
        fn cancel(self: &ModelsHandle);
        fn state(self: &ModelsHandle) -> ModelsState;
    }

    extern "Rust" {
        type UpdateHandle;
        fn open_updater() -> Box<UpdateHandle>;
        fn update_is_newer(latest: &str, current: &str) -> bool;
        fn check(self: &UpdateHandle, feed: &str);
        fn download(self: &UpdateHandle, suffix: &str, dir: &str);
        fn cancel(self: &UpdateHandle);
        fn asset_url(self: &UpdateHandle, suffix: &str) -> String;
        fn state(self: &UpdateHandle) -> UpdateState;
    }

    extern "Rust" {
        type SearchHandle;
        fn open_search(cache_dir: &str, root: &str, data_dir: &str) -> Result<Box<SearchHandle>>;
        fn refresh(self: &SearchHandle);
        fn reindex(self: &SearchHandle, path: &str);
        fn indexing(self: &SearchHandle) -> bool;
        fn index_progress(self: &SearchHandle) -> Vec<u64>;
        fn index_error(self: &SearchHandle) -> String;
        fn search(self: &SearchHandle, query: &str, limit: usize) -> Vec<SearchHit>;
    }

    extern "Rust" {
        type LibraryHandle;

        /// `data_dir` holds each library's history and trash.
        fn open_library(root: &str, data_dir: &str) -> Result<Box<LibraryHandle>>;
        /// Moves a library's notes to another folder, with their history and trash.
        fn relocate_library(from: &str, to: &str, data_dir: &str) -> Result<()>;
        /// Joins exported page PDFs into one file.
        fn merge_pdf_files(files: Vec<String>, target: &str) -> Result<()>;
        /// This library's own folder inside the app's data folder.
        fn data_dir(self: &LibraryHandle) -> String;
        fn root(self: &LibraryHandle) -> String;
        fn is_empty(self: &LibraryHandle) -> Result<bool>;
        /// `flags`: 1 notes, 2 PDFs, 4 other files, 8 hide folders without those; `focus` "" for all.
        /// `keep` shows a new folder whatever the filter.
        fn rows(
            self: &LibraryHandle,
            expanded: &str,
            flags: u8,
            notebooks: bool,
            focus: &str,
            keep: &str,
        ) -> Result<Vec<TreeRow>>;
        fn set_pinned(self: &LibraryHandle, path: &str, pinned: bool) -> Result<()>;
        /// Reorders the notebook list only; `focus` as for `rows`.
        fn move_notebook(self: &LibraryHandle, path: &str, delta: i32, focus: &str) -> Result<()>;
        /// `name` "" makes it a plain folder again.
        fn set_notebook(self: &LibraryHandle, folder: &str, name: &str) -> Result<()>;
        fn notebook_name(self: &LibraryHandle, folder: &str) -> String;
        /// The notebook a page or folder belongs to, or "".
        fn notebook_of(self: &LibraryHandle, path: &str) -> String;
        fn add_recent(self: &LibraryHandle, page: &str) -> Result<()>;
        /// Where a new page can go: notebooks and their sections, as "path\tlabel" lines.
        fn page_targets(self: &LibraryHandle) -> String;
        /// Every notebook, as "path\tname" lines in the user's order.
        fn notebook_list(self: &LibraryHandle) -> String;
        /// The subfolders of `folder`, as "name\tnotebook name" lines.
        fn subfolders(self: &LibraryHandle, folder: &str) -> String;
        fn absolute_path(self: &LibraryHandle, path: &str) -> String;
        fn import_page(self: &LibraryHandle, parent: &str, file: &str) -> Result<String>;
        fn unique_name(self: &LibraryHandle, parent: &str, base: &str) -> String;
        fn create_folder(self: &LibraryHandle, parent: &str, name: &str) -> Result<String>;
        fn create_page(
            self: &mut LibraryHandle,
            parent: &str,
            name: &str,
            like: &CanvasEngine,
        ) -> Result<String>;
        fn create_page_styled(
            self: &mut LibraryHandle,
            parent: &str,
            name: &str,
            kind: &str,
            landscape: bool,
            pattern: &str,
        ) -> Result<String>;
        fn rename(self: &mut LibraryHandle, path: &str, name: &str) -> Result<String>;
        /// `index` < 0 appends.
        fn move_to(
            self: &mut LibraryHandle,
            path: &str,
            parent: &str,
            index: i32,
        ) -> Result<String>;
        /// Returns the trash id `restore` takes.
        fn remove(self: &mut LibraryHandle, path: &str) -> Result<String>;
        /// The pages and PDFs in a folder and its subfolders, in tree order.
        fn pages_in(self: &LibraryHandle, folder: &str) -> Result<Vec<String>>;
        /// Newest first.
        fn trash_rows(self: &LibraryHandle) -> Result<Vec<TrashRow>>;
        fn restore(self: &mut LibraryHandle, id: &str) -> Result<String>;
        fn purge(self: &LibraryHandle, id: &str) -> Result<()>;
        fn empty_trash(self: &LibraryHandle) -> Result<()>;
        /// Earlier versions of a page or PDF, newest first, in milliseconds since the epoch.
        fn versions(self: &LibraryHandle, path: &str) -> Result<Vec<u64>>;
        fn restore_version(self: &mut LibraryHandle, path: &str, saved: u64) -> Result<()>;
        fn copy_version(self: &LibraryHandle, path: &str, saved: u64, name: &str)
        -> Result<String>;
        fn set_color(self: &LibraryHandle, path: &str, color: &str) -> Result<()>;
        fn open_page(self: &LibraryHandle) -> String;
        fn page_file(self: &LibraryHandle, path: &str) -> String;
        fn load_into(self: &mut LibraryHandle, path: &str, canvas: &mut CanvasEngine)
        -> Result<()>;
        fn save_from(self: &mut LibraryHandle, canvas: &CanvasEngine) -> Result<()>;
        fn close_page(self: &mut LibraryHandle);
        fn accepts_pictures(self: &LibraryHandle) -> bool;
        /// After the PDF page at `view_y`; reload the page afterwards.
        fn insert_blank_sheet(
            self: &mut LibraryHandle,
            canvas: &CanvasEngine,
            view_y: f64,
        ) -> Result<()>;
        fn recognition_jobs(self: &LibraryHandle, canvas: &CanvasEngine) -> Vec<LineJob>;
        /// True when the line is on the open page, which then needs saving.
        #[allow(clippy::too_many_arguments)]
        fn apply_recognition(
            self: &mut LibraryHandle,
            page: &str,
            key: &str,
            text: &str,
            x: f64,
            y: f64,
            w: f64,
            h: f64,
            lang: &str,
        ) -> Result<bool>;
        /// Saves what was read into `parent` as `name`; a taken name gets a number.
        fn finish_import(
            self: &mut LibraryHandle,
            importer: &ImportHandle,
            parent: &str,
            name: &str,
        ) -> Result<ImportResult>;
        fn import_pdf(
            self: &mut LibraryHandle,
            parent: &str,
            file: &str,
            annotate: bool,
        ) -> Result<String>;
        fn export_pdf(
            self: &LibraryHandle,
            target: &str,
            canvas: &CanvasEngine,
            overlay: &[u8],
        ) -> Result<()>;
        fn export_areas(self: &LibraryHandle, canvas: &CanvasEngine) -> Vec<f64>;
        fn pdf_support_problem() -> String;
        fn pdf_text(path: &str) -> String;

        type PrintJob;
        fn print_job(
            self: &LibraryHandle,
            canvas: &CanvasEngine,
            overlay: &[u8],
        ) -> Result<Box<PrintJob>>;
        fn print_page_count(self: &PrintJob) -> usize;
        fn print_page_aspect(self: &PrintJob, index: usize) -> f64;
        fn print_page_pixels(
            self: &PrintJob,
            index: usize,
            width: u32,
            height: u32,
        ) -> Result<Vec<u8>>;
    }

    extern "Rust" {
        type CanvasEngine;

        fn new_canvas_engine() -> Box<CanvasEngine>;

        fn set_viewport(self: &mut CanvasEngine, width: f64, height: f64, dpr: f64);
        fn frame(self: &mut CanvasEngine) -> Vec<TileInfo>;
        fn tile_pixels(self: &CanvasEngine, layer: u8, x: i32, y: i32) -> &[u8];
        fn tile_size(self: &CanvasEngine) -> u32;

        /// `device`: 0 = pen, 1 = pen eraser end, 2 = mouse, 3 = touch.
        fn pointer_down(
            self: &mut CanvasEngine,
            device: u8,
            sample: &PointerSample,
            erase_button: bool,
        ) -> bool;
        fn pointer_move(self: &mut CanvasEngine, samples: &[PointerSample]);
        fn pointer_up(self: &mut CanvasEngine);
        fn pointer_cancel(self: &mut CanvasEngine);
        fn is_drawing(self: &CanvasEngine) -> bool;

        fn pan_by(self: &mut CanvasEngine, dx: f64, dy: f64);
        fn zoom_around(self: &mut CanvasEngine, x: f64, y: f64, zoom: f64);
        fn set_view(self: &mut CanvasEngine, zoom: f64, offset_x: f64, offset_y: f64);
        fn zoom(self: &CanvasEngine) -> f64;
        fn offset_x(self: &CanvasEngine) -> f64;
        fn offset_y(self: &CanvasEngine) -> f64;
        fn reset_view(self: &mut CanvasEngine);
        fn needs_settle(self: &CanvasEngine) -> bool;
        fn settle(self: &mut CanvasEngine);

        /// `tool`: 0 = pen, 1 = highlighter, 2 = eraser.
        fn set_tool(self: &mut CanvasEngine, tool: u8);
        fn tool(self: &CanvasEngine) -> u8;
        fn set_eraser_whole_strokes(self: &mut CanvasEngine, on: bool);
        fn set_eraser_size(self: &mut CanvasEngine, radius: f64);
        fn set_pen_pattern(self: &mut CanvasEngine, name: &str);
        fn set_highlighter(self: &mut CanvasEngine, r: f32, g: f32, b: f32, width: f64);
        fn hold(self: &mut CanvasEngine) -> String;
        fn selection_count(self: &CanvasEngine) -> usize;
        fn text_rows(self: &CanvasEngine) -> Vec<TextRow>;
        fn has_selection_drag(self: &CanvasEngine) -> bool;
        fn focus_rect(self: &mut CanvasEngine, x: f64, y: f64, w: f64, h: f64);
        fn focus_sheet(self: &mut CanvasEngine, index: usize);
        fn clear_mark(self: &mut CanvasEngine);
        fn begin_math(
            self: &mut CanvasEngine,
            x: f64,
            y: f64,
            syntax: &str,
            r: f32,
            g: f32,
            b: f32,
        );
        fn edit_math(self: &mut CanvasEngine, id: u64) -> Vec<String>;
        fn set_math_draft(self: &mut CanvasEngine, source: &str, syntax: &str) -> String;
        fn commit_math(self: &mut CanvasEngine) -> bool;
        fn cancel_math(self: &mut CanvasEngine);
        fn math_at(self: &CanvasEngine, x: f64, y: f64) -> u64;
        fn math_draft_rect(self: &CanvasEngine) -> Vec<f64>;
        fn add_text(
            self: &mut CanvasEngine,
            x: f64,
            y: f64,
            width: f64,
            height: f64,
            markdown: &str,
        ) -> u64;
        fn set_text(self: &mut CanvasEngine, id: u64, markdown: &str, height: f64) -> bool;
        fn set_text_height(self: &mut CanvasEngine, id: u64, height: f64);
        fn text_at(self: &CanvasEngine, x: f64, y: f64) -> u64;
        fn to_doc(self: &CanvasEngine, x: f64, y: f64) -> Vec<f64>;
        fn texts_of_page(self: &CanvasEngine) -> Vec<TextRow>;
        fn selection_has_ink(self: &CanvasEngine) -> bool;
        fn selection_rect(self: &CanvasEngine) -> Vec<f64>;
        fn clear_selection(self: &mut CanvasEngine);
        fn select_all(self: &mut CanvasEngine);
        fn selection_jobs(self: &CanvasEngine, math: bool) -> Vec<LineJob>;
        fn replace_ink(self: &mut CanvasEngine, content: &str, math: bool) -> Vec<f64>;
        fn delete_selection(self: &mut CanvasEngine);
        fn copy_selection(self: &mut CanvasEngine);
        fn cut_selection(self: &mut CanvasEngine);
        fn paste(self: &mut CanvasEngine);
        fn can_paste(self: &CanvasEngine) -> bool;
        fn insert_picture(self: &mut CanvasEngine, png: &[u8], x: f64, y: f64) -> bool;
        fn recolor_selection(self: &mut CanvasEngine, r: f32, g: f32, b: f32, a: f32);
        fn set_pen_color(self: &mut CanvasEngine, r: f32, g: f32, b: f32, a: f32);
        fn set_pen_width(self: &mut CanvasEngine, width: f64);
        fn pen_width(self: &CanvasEngine) -> f64;
        fn set_finger_draws(self: &mut CanvasEngine, on: bool);
        fn finger_draws(self: &CanvasEngine) -> bool;

        fn undo(self: &mut CanvasEngine);
        fn redo(self: &mut CanvasEngine);
        fn clear(self: &mut CanvasEngine);
        fn unload(self: &mut CanvasEngine);
        fn can_undo(self: &CanvasEngine) -> bool;
        fn can_redo(self: &CanvasEngine) -> bool;
        fn revision(self: &CanvasEngine) -> u64;

        fn page_kind(self: &CanvasEngine) -> String;
        fn page_landscape(self: &CanvasEngine) -> bool;
        fn page_pattern(self: &CanvasEngine) -> String;
        fn set_page_style(self: &mut CanvasEngine, kind: &str, landscape: bool, pattern: &str);
        /// 0 narrow, 1 normal, 2 wide lines, grid or dots.
        fn page_spacing(self: &CanvasEngine) -> u8;
        fn set_page_spacing(self: &mut CanvasEngine, level: u8);
        fn set_desk_color(self: &mut CanvasEngine, r: f32, g: f32, b: f32);
        fn set_pressure_gamma(self: &mut CanvasEngine, gamma: f64);
        /// 0 = off … 3 = strong.
        fn set_smoothing_level(self: &mut CanvasEngine, level: u8);
    }

    unsafe extern "C++" {
        include!("ductus-qt/cpp/app.h");

        /// `program` is argv[0]: Qt for Android finds its plugins next to it.
        fn run_app(program: &str) -> i32;
    }
}

pub use crate::convert::{ConvertHandle, open_converter};
pub use crate::engine::{CanvasEngine, new_canvas_engine};
pub use crate::import::{ImportHandle, can_import, open_importer};
pub use crate::library::{
    LibraryHandle, PrintJob, merge_pdf_files, open_library, pdf_support_problem, pdf_text,
    relocate_library,
};
pub use crate::models::{
    ModelsHandle, languages, locale_language, model_rows, open_models, remove_model, text_models,
};
pub use crate::recognize::{RecognitionHandle, open_recognizer};
pub use crate::search::{SearchHandle, open_search};
pub use crate::update::{UpdateHandle, open_updater, update_is_newer};
