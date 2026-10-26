use crate::{Error, engine};
use ductus_core::UNITS_PER_POINT;
use ductus_render::{SheetPainter, Viewport};
use kurbo::Rect;
use pdfium_render::prelude::*;
use std::rc::Rc;
use tiny_skia::Pixmap;

pub struct PdfFile {
    document: PdfDocument<'static>,
    sizes: Vec<(f64, f64)>,
}

impl PdfFile {
    pub fn open(bytes: Vec<u8>) -> Result<Self, Error> {
        let document = engine()?.load_pdf_from_byte_vec(bytes, None)?;
        let sizes = document
            .pages()
            .iter()
            .map(|page| {
                (
                    f64::from(page.width().value) * UNITS_PER_POINT,
                    f64::from(page.height().value) * UNITS_PER_POINT,
                )
            })
            .collect();
        Ok(Self { document, sizes })
    }

    pub fn page_count(&self) -> usize {
        self.sizes.len()
    }

    pub fn page_sizes(&self) -> &[(f64, f64)] {
        &self.sizes
    }

    pub fn render_into(
        &self,
        index: usize,
        pixmap: &mut Pixmap,
        origin: (f64, f64),
        scale: f64,
    ) -> Result<(), Error> {
        let (w, h) = self.sizes[index];
        let page = self.document.pages().get(index as PdfPageIndex)?;
        let (pw, ph) = ((w * scale).round() as i32, (h * scale).round() as i32);
        let (ox, oy) = (origin.0.round() as i32, origin.1.round() as i32);
        let (bw, bh) = (pixmap.width() as i32, pixmap.height() as i32);
        let (x0, y0) = (ox.max(0), oy.max(0));
        let (x1, y1) = ((ox + pw).min(bw), (oy + ph).min(bh));
        if x0 >= x1 || y0 >= y1 {
            return Ok(());
        }
        let mut bitmap = PdfBitmap::empty(bw, bh, PdfBitmapFormat::BGRA)?;
        let config = PdfRenderConfig::new()
            .set_target_size(pw, ph)
            .set_origin(ox, oy)
            .render_annotations(true)
            .set_clear_color(PdfColor::WHITE)
            // Explicit although it is the default: the copy below depends on it.
            .set_reverse_byte_order(true);
        page.render_into_bitmap_with_config(&mut bitmap, &config)?;

        let src = bitmap.as_raw_bytes();
        let stride = bw as usize * 4;
        let dst = pixmap.data_mut();
        for y in y0..y1 {
            let row = y as usize * stride;
            for x in x0..x1 {
                let i = row + x as usize * 4;
                let a = u16::from(src[i + 3]);
                let pm = |c: u8| ((u16::from(c) * a + 127) / 255) as u8;
                dst[i] = pm(src[i]);
                dst[i + 1] = pm(src[i + 1]);
                dst[i + 2] = pm(src[i + 2]);
                dst[i + 3] = a as u8;
            }
        }
        Ok(())
    }

    pub fn page_text(&self, index: usize) -> Result<String, Error> {
        let page = self.document.pages().get(index as PdfPageIndex)?;
        Ok(page.text()?.all())
    }
}

pub struct PdfPainter(pub Rc<PdfFile>);

impl SheetPainter for PdfPainter {
    fn paint(&self, index: usize, sheet: Rect, pixmap: &mut Pixmap, vp: &Viewport) {
        if index >= self.0.page_count() {
            return;
        }
        let origin = (
            (sheet.x0 - vp.origin.x) * vp.scale,
            (sheet.y0 - vp.origin.y) * vp.scale,
        );
        // A page that fails to render stays blank.
        let _ = self.0.render_into(index, pixmap, origin, vp.scale);
    }
}
