//! Fixed-size pages from other apps, laid out as the sheets of one Ductus page.

use crate::{ImportedPage, pdf_error};
use ductus_core::{
    Document, Image, MathBox, Orientation, PageBackground, PageKind, PageSettings, PaperSize,
    Stroke, TextBox, UNITS_PER_MM, UNITS_PER_POINT, stack_sheets,
};
use ductus_format::Error;
use ductus_pdf::SheetSource;

/// One page of the other app; positions are relative to its top-left corner, in Ductus units.
#[derive(Default)]
pub(crate) struct Sheet {
    pub width: f64,
    pub height: f64,
    pub background: Option<PageBackground>,
    /// A page of one of the PDFs handed to `assemble`, as (file, page index).
    pub pdf: Option<(usize, usize)>,
    pub strokes: Vec<Stroke>,
    pub images: Vec<Image>,
    pub texts: Vec<TextBox>,
    pub maths: Vec<MathBox>,
}

/// Paper when every sheet is the same standard size without a PDF, else a PDF with blank pages.
pub(crate) fn assemble(
    name: String,
    sheets: Vec<Sheet>,
    pdfs: &[Vec<u8>],
    pictures: Vec<(String, Vec<u8>)>,
) -> Result<ImportedPage, Error> {
    let mut sizes: Vec<(f64, f64)> = sheets.iter().map(|s| (s.width, s.height)).collect();
    let paper = match sizes.first() {
        Some(&first)
            if sheets
                .iter()
                .all(|s| s.pdf.is_none() && (s.width, s.height) == first) =>
        {
            paper_kind(first)
        }
        _ => None,
    };
    // Laid out like Ductus lays out its paper, so later sheets don't drift.
    if let Some(size) = paper.and_then(|kind| kind.sheet_size()) {
        sizes.fill(size);
    }
    let pdf = match paper {
        Some(_) => None,
        None if sheets.is_empty() => None,
        None => {
            let sources: Vec<SheetSource> = sheets
                .iter()
                .map(|s| match s.pdf {
                    Some((file, index)) => SheetSource::Page { file, index },
                    None => SheetSource::Blank {
                        width: s.width / UNITS_PER_POINT,
                        height: s.height / UNITS_PER_POINT,
                    },
                })
                .collect();
            Some(ductus_pdf::compose(pdfs, &sources).map_err(pdf_error)?)
        }
    };
    let settings = PageSettings {
        kind: match (paper, &pdf) {
            (Some(kind), _) => kind,
            (None, Some(_)) => PageKind::Pdf,
            (None, None) => PageKind::Endless,
        },
        background: sheets.iter().find_map(|s| s.background).unwrap_or_default(),
    };

    let (mut strokes, mut images, mut texts, mut maths) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for (sheet, (x, y, _, _)) in sheets.into_iter().zip(stack_sheets(&sizes)) {
        strokes.extend(sheet.strokes.into_iter().map(|mut s| {
            for p in &mut s.points {
                p.x += x;
                p.y += y;
            }
            s
        }));
        images.extend(sheet.images.into_iter().map(|i| Image {
            x: i.x + x,
            y: i.y + y,
            ..i
        }));
        texts.extend(sheet.texts.into_iter().map(|t| TextBox {
            x: t.x + x,
            y: t.y + y,
            ..t
        }));
        maths.extend(sheet.maths.into_iter().map(|m| MathBox {
            x: m.x + x,
            y: m.y + y,
            ..m
        }));
    }
    let mut document = Document::with_contents(settings, strokes)
        .with_images(images)
        .with_texts(texts)
        .with_maths(maths);
    if pdf.is_some() {
        document.set_pdf_sheets(sizes);
    }
    Ok(ImportedPage {
        name,
        document,
        pdf,
        pictures,
    })
}

fn paper_kind((w, h): (f64, f64)) -> Option<PageKind> {
    let close = |a: f64, b: f64| (a - b).abs() < 2.0 * UNITS_PER_MM;
    PaperSize::ALL.into_iter().find_map(|size| {
        let (pw, ph) = size.mm();
        let (pw, ph) = (pw * UNITS_PER_MM, ph * UNITS_PER_MM);
        let orientation = if close(w, pw) && close(h, ph) {
            Orientation::Portrait
        } else if close(w, ph) && close(h, pw) {
            Orientation::Landscape
        } else {
            return None;
        };
        Some(PageKind::Paper { size, orientation })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ductus_core::{InkPoint, StrokeId, StrokeStyle};

    fn sheet(width: f64, height: f64, y: f64) -> Sheet {
        let point = InkPoint {
            x: 10.0,
            y,
            pressure: 1.0,
            tilt_x: 0.0,
            tilt_y: 0.0,
            t: 0,
        };
        Sheet {
            width,
            height,
            strokes: vec![Stroke {
                id: StrokeId(0),
                style: StrokeStyle::default(),
                points: vec![point, point],
            }],
            ..Sheet::default()
        }
    }

    #[test]
    fn standard_sheets_become_paper() {
        let a4 = (595.28 * UNITS_PER_POINT, 841.89 * UNITS_PER_POINT);
        let page = assemble(
            "Notes".into(),
            vec![sheet(a4.0, a4.1, 5.0), sheet(a4.0, a4.1, 5.0)],
            &[],
            Vec::new(),
        )
        .unwrap();
        assert!(page.pdf.is_none());
        assert!(matches!(
            page.document.settings().kind,
            PageKind::Paper {
                size: PaperSize::A4,
                orientation: Orientation::Portrait
            }
        ));
        let second = &page.document.strokes()[1].points[0];
        let sheet_top = page.document.sheets(0)[1].1;
        assert_eq!(second.y, sheet_top + 5.0);
    }

    #[test]
    fn odd_sizes_become_blank_pdf_pages() {
        let page = assemble(
            "Notes".into(),
            vec![sheet(400.0, 300.0, 0.0), sheet(200.0, 300.0, 0.0)],
            &[],
            Vec::new(),
        )
        .unwrap();
        assert_eq!(page.document.settings().kind, PageKind::Pdf);
        assert_eq!(page.document.pdf_sheets(), [(400.0, 300.0), (200.0, 300.0)]);
        assert!(page.pdf.unwrap().starts_with(b"%PDF"));
        // The narrower sheet is centred under the wider one.
        assert_eq!(page.document.strokes()[1].points[0].x, 110.0);
    }
}
