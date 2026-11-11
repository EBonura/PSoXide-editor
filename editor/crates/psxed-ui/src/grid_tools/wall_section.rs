//! The wall section of the Inspector: the four heights as numbers, each
//! property marked automatic or set, and a reset. It reads and writes through
//! the SDK's one wall API, so a wall reads the same whatever made it.

use super::*;

use psxed_project::grid::edit::wall_api::{
    clear_wall_property, set_wall_heights, walls_at, Prop, WallAddr, WallProp, WallView,
};
use psxed_project::grid::edit::{FaceRef, FaceTarget};

/// The wall `face` is, as the wall API sees it.
pub(super) fn view_of(world: &psxed_project::grid::GridWorld, face: FaceRef) -> Option<WallView> {
    let (side, authored_index, span, seg) = match face.target {
        FaceTarget::Wall { edge, index } => (edge, Some(index), None, 0),
        FaceTarget::Derived {
            edge, span, seg, ..
        } => (edge, None, Some(span), usize::from(seg)),
        FaceTarget::Surface { .. } => return None,
    };
    let views = walls_at(world, face.room, face.leaf, side);
    match authored_index {
        Some(i) => views
            .into_iter()
            .find(|v| v.addr.authored && v.addr.index == i),
        None => {
            let same: Vec<WallView> = views
                .into_iter()
                .filter(|v| !v.addr.authored && Some(v.addr.span) == span)
                .collect();
            same.get(seg).or_else(|| same.first()).cloned()
        }
    }
}

fn mark(ui: &mut egui::Ui, set: bool) {
    ui.label(
        RichText::new(if set { "set" } else { "automatic" })
            .size(10.5)
            .color(if set { STUDIO_ACCENT } else { STUDIO_TEXT_WEAK }),
    );
}

fn is_set<T>(p: &Prop<T>) -> bool {
    matches!(p, Prop::Set(_))
}

/// What the wall section asks for.
#[derive(Default)]
pub(crate) struct WallEdit {
    /// New corner heights (`None` keeps a corner).
    pub heights: Option<[Option<i32>; 4]>,
    /// Take every property back to automatic.
    pub reset: bool,
    pub addr: Option<WallAddr>,
}

/// Draw the wall section for `face` over `world`; the caller applies the
/// returned edit with [`apply`].
pub(super) fn draw(
    ui: &mut egui::Ui,
    world: &psxed_project::grid::GridWorld,
    face: FaceRef,
) -> WallEdit {
    let mut out = WallEdit::default();
    let Some(view) = view_of(world, face) else {
        return out;
    };
    let heights = view.heights.value();
    let mut edited = heights.map(Some);
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new("Heights").color(STUDIO_TEXT_WEAK));
        mark(ui, is_set(&view.heights));
    });
    for (i, name) in [
        "Bottom, near end",
        "Bottom, far end",
        "Top, far end",
        "Top, near end",
    ]
    .into_iter()
    .enumerate()
    {
        ui.horizontal(|ui| {
            ui.label(RichText::new(name).size(11.0).color(STUDIO_TEXT_WEAK));
            let mut clicks = heights[i] as f32 / psxed_project::grid::CLICK_UNITS as f32;
            if ui
                .add(
                    egui::DragValue::new(&mut clicks)
                        .speed(0.05)
                        .max_decimals(2)
                        .suffix(" clicks"),
                )
                .changed()
            {
                edited[i] = Some(psxed_project::grid::units::snap_to(
                    (clicks * psxed_project::grid::CLICK_UNITS as f32).round() as i32,
                    psxed_project::grid::FINE_UNITS,
                ));
                changed = true;
            }
        });
    }
    ui.horizontal(|ui| {
        ui.label(RichText::new("Material").color(STUDIO_TEXT_WEAK));
        mark(ui, is_set(&view.material));
        ui.label(RichText::new("Collision").color(STUDIO_TEXT_WEAK));
        mark(ui, is_set(&view.solid));
    });
    let any_set = is_set(&view.heights)
        || is_set(&view.material)
        || is_set(&view.solid)
        || is_set(&view.bands);
    ui.add_enabled_ui(any_set, |ui| {
        out.reset = ui
            .button("Reset wall to automatic")
            .on_hover_text("Take the heights, material, collision and bands back to what the floors and ceilings give")
            .clicked();
    });
    out.addr = Some(view.addr);
    if changed {
        out.heights = Some(edited);
    }
    out
}

/// Apply `edit` to `world`; the status line says what happened.
pub(crate) fn apply(world: &mut psxed_project::grid::GridWorld, edit: &WallEdit) -> Option<String> {
    let addr = edit.addr?;
    let mut status = None;
    if let Some(corners) = edit.heights {
        status = Some(match set_wall_heights(world, addr, corners) {
            Ok((_, w)) if w.is_empty() => "Wall heights set".to_string(),
            Ok(_) => "Wall heights set; a gap is left above or below it".to_string(),
            Err(e) => format!("The wall was not changed: {e:?}"),
        });
    }
    if edit.reset {
        let mut failed = false;
        for prop in [
            WallProp::Heights,
            WallProp::Material,
            WallProp::Solid,
            WallProp::Bands,
        ] {
            failed |= clear_wall_property(world, addr, prop).is_err();
        }
        status = Some(if failed {
            "Some of the wall could not be reset".to_string()
        } else {
            "Wall reset to automatic".to_string()
        });
    }
    status
}
