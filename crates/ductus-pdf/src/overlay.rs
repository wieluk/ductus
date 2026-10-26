//! The app lays text boxes out into a matching PDF drawn over the pages, so text stays vector text.

use crate::Error;
use crate::annot::PageGeometry;
use crate::pictures::{draw_on_page, draw_ops, fresh_xobject_name, own_resources};
use ductus_core::UNITS_PER_POINT;
use kurbo::Point;
use lopdf::{Object, ObjectId, Stream, dictionary};

pub fn overlay_pages(base: &[u8], overlay: &[u8]) -> Result<Vec<u8>, Error> {
    let mut pdf = lopdf::Document::load_mem(base)?;
    let mut over = lopdf::Document::load_mem(overlay)?;
    over.renumber_objects_with(pdf.max_id + 1);

    let mut forms: Vec<(ObjectId, f64, f64)> = Vec::new();
    for page in over.get_pages().into_values() {
        let (w, h) = PageGeometry::of(&over, page).size();
        let content = over.get_page_content(page);
        let resources = own_resources(&over, page);
        let form = Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Form",
                "BBox" => vec![0.into(), 0.into(), Object::Real(w as f32), Object::Real(h as f32)],
                "Resources" => resources,
            },
            content,
        );
        let id = (over.max_id + 1 + forms.len() as u32, 0);
        over.objects.insert(id, Object::Stream(form));
        forms.push((id, w, h));
    }
    let new_max = over.max_id + forms.len() as u32;
    pdf.objects.extend(over.objects);
    pdf.max_id = pdf.max_id.max(new_max);

    let pages: Vec<ObjectId> = pdf.get_pages().into_values().collect();
    for (&page, &(form, _, h)) in pages.iter().zip(&forms) {
        let geometry = PageGeometry::of(&pdf, page);
        // Form space: points, y up. Display: document units, y down.
        let shown = |x: f64, y: f64| {
            geometry.to_user(Point::new(x * UNITS_PER_POINT, (h - y) * UNITS_PER_POINT))
        };
        let origin = shown(0.0, 0.0);
        let x_axis = (shown(1.0, 0.0) - origin).to_point();
        let y_axis = (shown(0.0, 1.0) - origin).to_point();
        let name = fresh_xobject_name(&pdf, page, "DxText", &mut Vec::new());
        let ops = draw_ops(&name, origin, x_axis, y_axis);
        draw_on_page(&mut pdf, page, vec![(name, form)], ops)?;
    }
    // The overlay's own page tree is no longer referenced.
    pdf.prune_objects();
    let mut out = Vec::with_capacity(base.len() + overlay.len());
    pdf.save_to(&mut out)?;
    Ok(out)
}
