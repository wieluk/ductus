mod annot;
mod blank;
mod engine;
mod export;
mod file;
mod merge;
mod overlay;
mod pictures;
#[cfg(test)]
mod testpdf;

pub use annot::{
    PageInk, PageObject, PlacedObject, flatten, read_ink, read_objects, strip_ductus_annotations,
    write_annotations, write_ink,
};
pub use blank::insert_blank_page;
pub use engine::{engine, pdfium_status};
pub use export::{
    document_objects, document_strokes, export_notes, ink_by_page, math_overlay, objects_by_page,
    page_areas,
};
pub use file::{PdfFile, PdfPainter};
pub use merge::{SheetSource, compose, merge};
pub use overlay::overlay_pages;
pub use pictures::{PagePicture, PdfImage, pictures_by_page};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("PDF support is unavailable: {0}")]
    Unavailable(String),
    #[error("could not read the PDF: {0}")]
    Pdfium(#[from] pdfium_render::prelude::PdfiumError),
    #[error("could not edit the PDF: {0}")]
    Lopdf(#[from] lopdf::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Invalid(String),
}
