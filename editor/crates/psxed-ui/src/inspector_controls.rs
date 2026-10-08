use super::*;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct UvTransformEdit {
    pub(crate) offset: [bool; 2],
    pub(crate) span: [bool; 2],
    pub(crate) rotation: bool,
    pub(crate) flip_u: bool,
    pub(crate) flip_v: bool,
}

impl UvTransformEdit {
    pub(crate) const fn changed(self) -> bool {
        self.offset[0]
            || self.offset[1]
            || self.span[0]
            || self.span[1]
            || self.rotation
            || self.flip_u
            || self.flip_v
    }

    const fn all() -> Self {
        Self {
            offset: [true; 2],
            span: [true; 2],
            rotation: true,
            flip_u: true,
            flip_v: true,
        }
    }
}

pub(crate) fn uv_transform_controls(uv: &mut UvTransform, ui: &mut egui::Ui) -> UvTransformEdit {
    let mut edit = UvTransformEdit::default();
    ui.horizontal(|ui| {
        ui.label("Offset");
        ui.label("U");
        edit.offset[0] |= ui
            .add(egui::DragValue::new(&mut uv.offset[0]).speed(1.0))
            .changed();
        ui.label("V");
        edit.offset[1] |= ui
            .add(egui::DragValue::new(&mut uv.offset[1]).speed(1.0))
            .changed();
    });
    ui.horizontal(|ui| {
        ui.label("Span");
        ui.label("U");
        edit.span[0] |= ui
            .add(
                egui::DragValue::new(&mut uv.span[0])
                    .speed(1.0)
                    .range(0..=255),
            )
            .on_hover_text("0 uses the material's native U span.")
            .changed();
        ui.label("V");
        edit.span[1] |= ui
            .add(
                egui::DragValue::new(&mut uv.span[1])
                    .speed(1.0)
                    .range(0..=255),
            )
            .on_hover_text("0 uses the material's native V span.")
            .changed();
    });
    ui.horizontal(|ui| {
        ui.label("Rotate");
        for (rotation, label) in [
            (UvRotation::Deg0, "0"),
            (UvRotation::Deg45, "45"),
            (UvRotation::Deg90, "90"),
            (UvRotation::Deg135, "135"),
            (UvRotation::Deg180, "180"),
            (UvRotation::Deg225, "225"),
            (UvRotation::Deg270, "270"),
            (UvRotation::Deg315, "315"),
        ] {
            edit.rotation |= ui
                .selectable_value(&mut uv.rotation, rotation, label)
                .clicked();
        }
    });
    ui.horizontal(|ui| {
        edit.flip_u |= ui.checkbox(&mut uv.flip_u, "Flip U").changed();
        edit.flip_v |= ui.checkbox(&mut uv.flip_v, "Flip V").changed();
        if ui
            .small_button("Reset")
            .on_hover_text("Reset selected faces' UV offset, span, rotation, and flips.")
            .clicked()
        {
            *uv = UvTransform::IDENTITY;
            edit = UvTransformEdit::all();
        }
    });
    edit
}

pub(crate) fn material_picker(
    ui: &mut egui::Ui,
    label: &str,
    current: &mut Option<ResourceId>,
    options: &[(ResourceId, String)],
    jump_to: &mut Option<ResourceId>,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(label);
        let preview = current
            .and_then(|id| {
                options
                    .iter()
                    .find(|(rid, _)| *rid == id)
                    .map(|(_, name)| name.as_str())
            })
            .unwrap_or("(none)");
        changed |= searchable_picker(
            ui,
            ui.id().with(("material-picker", label)),
            current,
            preview,
            options,
            SearchablePickerConfig::optional("(none)")
                .with_popup_min_width(360.0)
                .with_search_hint("Search materials…"),
        );
        if let Some(id) = *current {
            if ui
                .small_button("→")
                .on_hover_text("Open this material in the inspector")
                .clicked()
            {
                *jump_to = Some(id);
            }
        }
    });
    changed
}
