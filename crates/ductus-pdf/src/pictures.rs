use crate::Error;
use crate::annot::PageGeometry;
use ductus_core::{Document, SHEET_GAP};
use kurbo::{Point, Rect};
use lopdf::content::{Content, Operation};
use lopdf::{Dictionary, Object, ObjectId, Stream, dictionary};

pub struct PdfImage {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl PdfImage {
    pub fn from_png(bytes: &[u8]) -> Option<Self> {
        let pixmap = tiny_skia::Pixmap::decode_png(bytes).ok()?;
        let rgba = pixmap
            .pixels()
            .iter()
            .flat_map(|c| {
                let c = c.demultiply();
                [c.red(), c.green(), c.blue(), c.alpha()]
            })
            .collect();
        Some(Self {
            width: pixmap.width(),
            height: pixmap.height(),
            rgba,
        })
    }

    fn add_to(&self, pdf: &mut lopdf::Document) -> ObjectId {
        let (w, h) = (i64::from(self.width), i64::from(self.height));
        let image = |color_space: &str| {
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => w,
                "Height" => h,
                "ColorSpace" => color_space,
                "BitsPerComponent" => 8,
            }
        };
        let compressed = |dict: Dictionary, data: Vec<u8>| {
            let mut stream = Stream::new(dict, data);
            // Uncompressed is still valid, just larger.
            let _ = stream.compress();
            stream
        };
        let rgb: Vec<u8> = self
            .rgba
            .chunks_exact(4)
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect();
        let mut dict = image("DeviceRGB");
        if self.rgba.chunks_exact(4).any(|p| p[3] < 255) {
            let alpha: Vec<u8> = self.rgba.chunks_exact(4).map(|p| p[3]).collect();
            let mask = pdf.add_object(compressed(image("DeviceGray"), alpha));
            dict.set("SMask", mask);
        }
        pdf.add_object(compressed(dict, rgb))
    }
}

pub struct PagePicture {
    pub page: usize,
    pub rect: Rect,
    pub image: PdfImage,
}

pub fn pictures_by_page(
    doc: &Document,
    image: &dyn Fn(&str) -> Option<PdfImage>,
) -> Vec<PagePicture> {
    let sheets = ductus_core::stack_sheets(doc.pdf_sheets());
    doc.images()
        .iter()
        .filter_map(|picture| {
            let index = sheets
                .iter()
                .position(|&(_, y, _, h)| picture.y < y + h + SHEET_GAP / 2.0)
                .unwrap_or(sheets.len().checked_sub(1)?);
            let (x0, y0, _, _) = sheets[index];
            Some(PagePicture {
                page: index,
                rect: Rect::new(
                    picture.x - x0,
                    picture.y - y0,
                    picture.x - x0 + picture.width,
                    picture.y - y0 + picture.height,
                ),
                image: image(&picture.source)?,
            })
        })
        .collect()
}

pub(crate) fn draw_ops(name: &str, origin: Point, x_axis: Point, y_axis: Point) -> Vec<Operation> {
    let n = |v: f64| Object::Real(v as f32);
    vec![
        Operation::new("q", vec![]),
        Operation::new(
            "cm",
            vec![
                n(x_axis.x),
                n(x_axis.y),
                n(y_axis.x),
                n(y_axis.y),
                n(origin.x),
                n(origin.y),
            ],
        ),
        Operation::new("Do", vec![Object::Name(name.as_bytes().to_vec())]),
        Operation::new("Q", vec![]),
    ]
}

pub(crate) fn notes_page_pictures(
    pdf: &mut lopdf::Document,
    doc: &Document,
    area: Rect,
    scale: f64,
    image: &dyn Fn(&str) -> Option<PdfImage>,
    ops: &mut Vec<Operation>,
) -> Dictionary {
    let mut xobjects = Dictionary::new();
    for (i, picture) in doc.images().iter().enumerate() {
        let r = Rect::new(
            picture.x,
            picture.y,
            picture.x + picture.width,
            picture.y + picture.height,
        );
        if !r.overlaps(area) {
            continue;
        }
        let Some(pixels) = image(&picture.source) else {
            continue;
        };
        let name = format!("DxIm{i}");
        xobjects.set(name.as_bytes(), pixels.add_to(pdf));
        let (x, y) = ((r.x0 - area.x0) * scale, (area.y1 - r.y1) * scale);
        ops.extend(draw_ops(
            &name,
            Point::new(x, y),
            Point::new(r.width() * scale, 0.0),
            Point::new(0.0, r.height() * scale),
        ));
    }
    xobjects
}

pub(crate) fn own_resources(pdf: &lopdf::Document, page: ObjectId) -> Dictionary {
    let mut id = page;
    loop {
        let Ok(dict) = pdf.get_dictionary(id) else {
            return Dictionary::new();
        };
        if let Ok(resources) = dict.get(b"Resources") {
            return match resources {
                Object::Reference(r) => pdf.get_dictionary(*r).cloned().unwrap_or_default(),
                Object::Dictionary(d) => d.clone(),
                _ => Dictionary::new(),
            };
        }
        match dict.get(b"Parent").and_then(Object::as_reference) {
            Ok(parent) => id = parent,
            Err(_) => return Dictionary::new(),
        }
    }
}

/// An XObject name the page doesn't use yet, so a later overlay never replaces an earlier one.
pub(crate) fn fresh_xobject_name(
    pdf: &lopdf::Document,
    page: ObjectId,
    base: &str,
    taken: &mut Vec<String>,
) -> String {
    let resources = own_resources(pdf, page);
    let existing = match resources.get(b"XObject") {
        Ok(Object::Reference(r)) => pdf.get_dictionary(*r).cloned().unwrap_or_default(),
        Ok(Object::Dictionary(d)) => d.clone(),
        _ => Dictionary::new(),
    };
    let name = (0..)
        .map(|i| format!("{base}{i}"))
        .find(|name| !existing.has(name.as_bytes()) && !taken.contains(name))
        .expect("an unused name");
    taken.push(name.clone());
    name
}

pub(crate) fn draw_on_page(
    pdf: &mut lopdf::Document,
    page: ObjectId,
    xobjects: Vec<(String, ObjectId)>,
    ops: Vec<Operation>,
) -> Result<(), Error> {
    let mut resources = own_resources(pdf, page);
    let mut dict = match resources.get(b"XObject") {
        Ok(Object::Reference(r)) => pdf.get_dictionary(*r).cloned().unwrap_or_default(),
        Ok(Object::Dictionary(d)) => d.clone(),
        _ => Dictionary::new(),
    };
    for (name, id) in xobjects {
        dict.set(name.as_bytes(), id);
    }
    resources.set("XObject", dict);
    // Wrap the existing content in q/Q so its state can't shift ours.
    let open = pdf.add_object(Stream::new(Dictionary::new(), b"q".to_vec()));
    let mut close = Content {
        operations: vec![Operation::new("Q", vec![])],
    };
    close.operations.extend(ops);
    let close = pdf.add_object(Stream::new(Dictionary::new(), close.encode()?));
    let mut contents = vec![Object::Reference(open)];
    match pdf.get_dictionary(page)?.get(b"Contents") {
        Ok(Object::Array(items)) => contents.extend(items.iter().cloned()),
        Ok(Object::Reference(r)) => match pdf.get_object(*r) {
            // A reference to an array of streams, not to a stream.
            Ok(Object::Array(items)) => contents.extend(items.iter().cloned()),
            _ => contents.push(Object::Reference(*r)),
        },
        Ok(other) => contents.push(other.clone()),
        Err(_) => {}
    }
    contents.push(Object::Reference(close));
    let dict = pdf.get_dictionary_mut(page)?;
    dict.set("Contents", contents);
    dict.set("Resources", resources);
    Ok(())
}

pub(crate) fn add_to_pdf(bytes: &[u8], pictures: &[PagePicture]) -> Result<Vec<u8>, Error> {
    if pictures.is_empty() {
        return Ok(bytes.to_vec());
    }
    let mut pdf = lopdf::Document::load_mem(bytes)?;
    let pages: Vec<ObjectId> = pdf.get_pages().into_values().collect();
    for (index, &page) in pages.iter().enumerate() {
        let on_page: Vec<&PagePicture> = pictures.iter().filter(|p| p.page == index).collect();
        if on_page.is_empty() {
            continue;
        }
        let geometry = PageGeometry::of(&pdf, page);
        let mut xobjects = Vec::new();
        let mut ops = Vec::new();
        let mut taken = Vec::new();
        for picture in &on_page {
            let name = fresh_xobject_name(&pdf, page, "DxIm", &mut taken);
            xobjects.push((name.clone(), picture.image.add_to(&mut pdf)));
            let r = picture.rect;
            let origin = geometry.to_user(Point::new(r.x0, r.y1));
            let right = geometry.to_user(Point::new(r.x1, r.y1));
            let top = geometry.to_user(Point::new(r.x0, r.y0));
            ops.extend(draw_ops(
                &name,
                origin,
                (right - origin).to_point(),
                (top - origin).to_point(),
            ));
        }
        draw_on_page(&mut pdf, page, xobjects, ops)?;
    }
    let mut out = Vec::with_capacity(bytes.len());
    pdf.save_to(&mut out)?;
    Ok(out)
}
