//! The Meshes palette and the Inspector section for the selected instance.
//!
//! One home for meshes, like the Materials palette above it: the right dock.
//! The palette lists the world's assets, makes new ones (arch, column,
//! rubble, or an OBJ file) and edits the recipe of the chosen asset (every
//! copy follows). The Inspector section edits one instance: position, turn,
//! scale, a material per slot, collision. The Inspector wraps the frame in an
//! undo step, so a drag coalesces.

use super::*;

use psxed_project::grid::edit::{add_asset, set_asset, update_mesh};
use psxed_project::grid::mesh::{
    ArchProfile, ArchSpec, ColumnSpec, ImportProxy, ImportedMesh, MeshAsset, MeshKind, RubbleSpec,
    SCALE_ONE, SCALE_RANGE,
};

fn kind_label(kind: &MeshKind) -> &'static str {
    match kind {
        MeshKind::Arch(_) => "arch",
        MeshKind::Column(_) => "column",
        MeshKind::Rubble(_) => "rubble",
        MeshKind::Imported(_) => "imported",
    }
}

/// Edit the numbers of a recipe; returns whether anything changed.
fn edit_recipe(ui: &mut egui::Ui, kind: &mut MeshKind) -> bool {
    let mut changed = false;
    fn drag(
        ui: &mut egui::Ui,
        changed: &mut bool,
        label: &str,
        value: &mut i32,
        speed: f32,
        range: (i32, i32),
    ) {
        ui.horizontal(|ui| {
            ui.label(label);
            *changed |= ui
                .add(
                    egui::DragValue::new(value)
                        .speed(speed)
                        .range(range.0..=range.1),
                )
                .changed();
        });
    }
    match kind {
        MeshKind::Arch(spec) => {
            ui.horizontal(|ui| {
                ui.label("Profile");
                egui::ComboBox::from_id_salt("mesh_arch_profile")
                    .selected_text(spec.profile.label())
                    .show_ui(ui, |ui| {
                        for profile in ArchProfile::ALL {
                            if ui
                                .selectable_value(&mut spec.profile, profile, profile.label())
                                .changed()
                            {
                                // Land on a rise the profile allows.
                                let c = spec.span / 2;
                                spec.rise = match profile {
                                    ArchProfile::Round => c,
                                    ArchProfile::Segmental => c / 2,
                                    ArchProfile::Pointed => c * 3 / 2,
                                    ArchProfile::Flat => spec.rise,
                                };
                                changed = true;
                            }
                        }
                    });
            });
            drag(ui, &mut changed, "Span", &mut spec.span, 16.0, (256, 8192));
            if spec.profile != ArchProfile::Flat {
                drag(ui, &mut changed, "Rise", &mut spec.rise, 16.0, (64, 8192));
            }
            drag(
                ui,
                &mut changed,
                "Spring",
                &mut spec.spring,
                16.0,
                (0, 8192),
            );
            drag(
                ui,
                &mut changed,
                "Thickness",
                &mut spec.thickness,
                8.0,
                (64, 2048),
            );
            drag(ui, &mut changed, "Depth", &mut spec.depth, 16.0, (64, 4096));
            let mut segments = i32::from(spec.segments);
            drag(ui, &mut changed, "Segments", &mut segments, 0.1, (1, 8));
            spec.segments = segments.clamp(1, 8) as u8;
            changed |= ui
                .checkbox(&mut spec.perch, "Hook socket at the crown")
                .on_hover_text("The player can perch under the crown of every placed copy")
                .changed();
        }
        MeshKind::Column(spec) => {
            drag(
                ui,
                &mut changed,
                "Radius",
                &mut spec.radius,
                8.0,
                (64, 1024),
            );
            drag(
                ui,
                &mut changed,
                "Height",
                &mut spec.height,
                16.0,
                (256, 16384),
            );
            let mut sides = i32::from(spec.sides);
            drag(ui, &mut changed, "Sides", &mut sides, 0.1, (3, 12));
            spec.sides = sides.clamp(3, 12) as u8;
            drag(ui, &mut changed, "Plinth", &mut spec.plinth, 8.0, (0, 2048));
            drag(
                ui,
                &mut changed,
                "Capital",
                &mut spec.capital,
                8.0,
                (0, 2048),
            );
        }
        MeshKind::Rubble(spec) => {
            drag(ui, &mut changed, "Size", &mut spec.size, 16.0, (128, 4096));
            let mut pieces = i32::from(spec.pieces);
            drag(ui, &mut changed, "Stones", &mut pieces, 0.1, (1, 6));
            spec.pieces = pieces.clamp(1, 6) as u8;
            let mut seed = i32::from(spec.seed);
            drag(
                ui,
                &mut changed,
                "Seed",
                &mut seed,
                0.2,
                (0, i32::from(u16::MAX)),
            );
            spec.seed = seed.clamp(0, i32::from(u16::MAX)) as u16;
        }
        MeshKind::Imported(mesh) => {
            ui.label(
                RichText::new(format!(
                    "{} faces from {}",
                    mesh.faces.len(),
                    if mesh.source.is_empty() {
                        "an OBJ file"
                    } else {
                        &mesh.source
                    }
                ))
                .size(11.0)
                .color(STUDIO_TEXT_WEAK),
            );
            ui.horizontal(|ui| {
                ui.label("Collision");
                egui::ComboBox::from_id_salt("mesh_import_proxy")
                    .selected_text(match mesh.proxy {
                        ImportProxy::None => "none",
                        ImportProxy::Box => "box",
                        ImportProxy::BoxTop => "box, standable top",
                    })
                    .show_ui(ui, |ui| {
                        for (value, label) in [
                            (ImportProxy::None, "none"),
                            (ImportProxy::Box, "box"),
                            (ImportProxy::BoxTop, "box, standable top"),
                        ] {
                            changed |= ui.selectable_value(&mut mesh.proxy, value, label).changed();
                        }
                    });
            });
        }
    }
    changed
}

impl EditorWorkspace {
    /// The Meshes palette, then the selected instance.
    pub(crate) fn draw_mesh_panels(&mut self, ui: &mut egui::Ui) {
        self.retain_valid_mesh();
        self.draw_mesh_palette(ui);
        if self.mesh_primary().is_some() {
            ui.add_space(6.0);
            self.draw_mesh_inspector(ui);
        }
    }

    fn draw_mesh_palette(&mut self, ui: &mut egui::Ui) {
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let assets: Vec<(String, usize, MeshKind)> = world
            .mesh_assets
            .iter()
            .map(|a| {
                (
                    a.name.clone(),
                    a.build().map_or(0, |b| b.faces.len()),
                    a.kind.clone(),
                )
            })
            .collect();
        let mut choose = None;
        let mut make: Option<MeshAsset> = None;
        let mut edited: Option<(usize, MeshKind)> = None;
        egui::CollapsingHeader::new(RichText::new("Meshes").strong())
            .default_open(true)
            .show(ui, |ui| {
                ui.label(
                    RichText::new(
                        "Static pieces: pick one, then click a floor with the Mesh tool (Shift+N).",
                    )
                    .size(10.5)
                    .color(STUDIO_TEXT_WEAK),
                );
                if assets.is_empty() {
                    ui.label(RichText::new("No meshes yet: make one below.").color(STUDIO_TEXT_WEAK));
                }
                for (i, (name, faces, kind)) in assets.iter().enumerate() {
                    let on = self.grid.mesh_asset == Some(i as u16);
                    if ui
                        .selectable_label(on, format!("{name}  ({} {}, {faces} faces)", kind_label(kind), i))
                        .clicked()
                    {
                        choose = Some(i as u16);
                    }
                }
                ui.horizontal_wrapped(|ui| {
                    if ui.button("+ Arch").on_hover_text("A round arch with a perch at the crown").clicked() {
                        make = Some(MeshAsset::new("arch", MeshKind::Arch(ArchSpec::default())));
                    }
                    if ui.button("+ Column").clicked() {
                        make = Some(MeshAsset::new("column", MeshKind::Column(ColumnSpec::default())));
                    }
                    if ui.button("+ Rubble").clicked() {
                        make = Some(MeshAsset::new("rubble", MeshKind::Rubble(RubbleSpec::default())));
                    }
                });
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.grid.mesh_import_path)
                            .hint_text("path/to/mesh.obj")
                            .desired_width(150.0),
                    );
                    if ui
                        .button("Import OBJ")
                        .on_hover_text("Quads and triangles with their UVs; usemtl names become material slots. One OBJ unit is one sector.")
                        .clicked()
                    {
                        let path = self.grid.mesh_import_path.trim().to_string();
                        match std::fs::read_to_string(&path)
                            .map_err(|e| format!("cannot read {path}: {e}"))
                            .and_then(|text| {
                                let source = std::path::Path::new(&path)
                                    .file_name()
                                    .map_or(path.clone(), |n| n.to_string_lossy().into_owned());
                                ImportedMesh::from_obj(&text, 1024.0, &source, ImportProxy::None)
                                    .map(|m| (source, m))
                            }) {
                            Ok((source, mesh)) => {
                                make = Some(MeshAsset::new(source, MeshKind::Imported(mesh)));
                            }
                            Err(e) => self.status = e,
                        }
                    }
                });
                if let Some(index) = self
                    .grid
                    .mesh_asset
                    .map(usize::from)
                    .filter(|i| *i < assets.len())
                {
                    ui.separator();
                    ui.label(
                        RichText::new(format!("Recipe of {:?}: every copy follows", assets[index].0))
                            .size(11.0)
                            .color(STUDIO_TEXT_WEAK),
                    );
                    let mut kind = assets[index].2.clone();
                    if edit_recipe(ui, &mut kind) {
                        edited = Some((index, kind));
                    }
                }
            });
        if let Some(index) = choose {
            self.grid.mesh_asset = Some(index);
        }
        if let Some(asset) = make {
            let Some(world) = self.project.active_scene_mut().world.grid_mut() else {
                return;
            };
            match add_asset(world, asset) {
                Ok(index) => {
                    self.grid.mesh_asset = Some(index);
                    self.grid_changed();
                    self.set_grid_tool(GridTool::Mesh);
                    self.status = "Mesh ready: click a floor to place it".to_string();
                }
                Err(refusal) => self.status = refusal.describe(),
            }
        }
        if let Some((index, kind)) = edited {
            if let Some(world) = self.project.active_scene_mut().world.grid_mut() {
                match set_asset(world, index, kind) {
                    Ok(()) => self.grid_changed(),
                    Err(refusal) => self.status = refusal.describe(),
                }
            }
        }
    }

    fn draw_mesh_inspector(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.mesh_primary() else {
            return;
        };
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let Some((room, index)) = world.find_mesh(id) else {
            return;
        };
        let original = world.rooms[room].meshes[index].clone();
        let asset_names: Vec<String> = world.mesh_assets.iter().map(|a| a.name.clone()).collect();
        let built = world
            .mesh_assets
            .get(usize::from(original.asset))
            .and_then(|a| a.build().ok());
        let slots: Vec<String> = built.as_ref().map_or(Vec::new(), |b| b.slots.clone());
        let palette = self.grid.material;
        let palette_name = palette
            .and_then(|m| self.project.resource_name(m))
            .unwrap_or("none")
            .to_string();
        let names: Vec<String> = slots
            .iter()
            .enumerate()
            .map(|(i, _)| {
                original
                    .material(i as u8)
                    .and_then(|m| self.project.resource_name(m))
                    .unwrap_or("none")
                    .to_string()
            })
            .collect();
        let summary = built.as_ref().map_or_else(String::new, |b| {
            format!(
                "{} faces, {} collision shape(s), {} socket(s)",
                b.faces.len(),
                b.proxies.len(),
                b.sockets.len()
            )
        });
        let mut edited = original.clone();
        let mut delete = false;
        egui::CollapsingHeader::new(RichText::new("Mesh").strong())
            .default_open(true)
            .show(ui, |ui| {
                ui.label(RichText::new(format!("Mesh {id} in room {room}")).strong());
                ui.label(RichText::new(summary).size(11.0).color(STUDIO_TEXT_WEAK));
                ui.horizontal(|ui| {
                    ui.label("Asset");
                    egui::ComboBox::from_id_salt("mesh_inst_asset")
                        .selected_text(
                            asset_names
                                .get(usize::from(edited.asset))
                                .cloned()
                                .unwrap_or_default(),
                        )
                        .show_ui(ui, |ui| {
                            for (i, name) in asset_names.iter().enumerate() {
                                ui.selectable_value(&mut edited.asset, i as u16, name);
                            }
                        });
                });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Pos");
                    for (axis, name) in ["x", "y", "z"].into_iter().enumerate() {
                        ui.add(
                            egui::DragValue::new(&mut edited.position[axis])
                                .speed(16.0)
                                .prefix(format!("{name} ")),
                        );
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Turn");
                    for (axis, name) in ["yaw", "pitch", "roll"].into_iter().enumerate() {
                        ui.add(
                            egui::DragValue::new(&mut edited.rotation[axis])
                                .speed(1.0)
                                .range(-180..=180)
                                .prefix(format!("{name} "))
                                .suffix("\u{b0}"),
                        );
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Scale");
                    let mut uniform = edited.uniform();
                    let mut percent = f32::from(edited.scale[0]) * 100.0 / f32::from(SCALE_ONE);
                    if uniform {
                        if ui
                            .add(
                                egui::DragValue::new(&mut percent)
                                    .speed(1.0)
                                    .range(
                                        f32::from(SCALE_RANGE.0) * 100.0 / f32::from(SCALE_ONE)
                                            ..=f32::from(SCALE_RANGE.1) * 100.0 / f32::from(SCALE_ONE),
                                    )
                                    .suffix("%"),
                            )
                            .changed()
                        {
                            let q = (percent * f32::from(SCALE_ONE) / 100.0).round() as u16;
                            edited.scale = [q; 3];
                        }
                    } else {
                        for (axis, name) in ["x", "y", "z"].into_iter().enumerate() {
                            let mut p = f32::from(edited.scale[axis]) * 100.0 / f32::from(SCALE_ONE);
                            if ui
                                .add(
                                    egui::DragValue::new(&mut p)
                                        .speed(1.0)
                                        .prefix(format!("{name} "))
                                        .suffix("%"),
                                )
                                .changed()
                            {
                                edited.scale[axis] = (p * f32::from(SCALE_ONE) / 100.0).round() as u16;
                            }
                        }
                    }
                    if ui
                        .selectable_label(!uniform, "xyz")
                        .on_hover_text("Scale each axis on its own")
                        .clicked()
                    {
                        uniform = !uniform;
                        if uniform {
                            edited.scale = [edited.scale[0]; 3];
                        }
                    }
                });
                if ui
                    .button("Turn 90")
                    .on_hover_text("Quarter turn about the vertical axis (R)")
                    .clicked()
                {
                    edited.rotation[0] += 90;
                }
                ui.label(
                    RichText::new("Grid units: a sector is 1024, a click 256. Yaw 0 is the asset's front (+Z).")
                        .size(10.5)
                        .color(STUDIO_TEXT_WEAK),
                );
                ui.separator();
                for (i, slot) in slots.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(slot).color(STUDIO_TEXT_WEAK));
                        ui.label(RichText::new(&names[i]).color(STUDIO_ACCENT));
                        if ui
                            .small_button("Use palette")
                            .on_hover_text(format!("Apply {palette_name} to this slot"))
                            .clicked()
                        {
                            edited.set_material(i, palette);
                        }
                        if ui.small_button("Clear").clicked() {
                            edited.set_material(i, None);
                        }
                    });
                }
                ui.label(
                    RichText::new("Clicking a swatch above applies it to every slot.")
                        .size(10.5)
                        .color(STUDIO_TEXT_WEAK),
                );
                ui.separator();
                ui.checkbox(&mut edited.collide, "Collides")
                    .on_hover_text("Piers and columns block, flat tops and the crown are surfaces to stand on");
                ui.checkbox(&mut edited.fullbright, "Fullbright")
                    .on_hover_text("Show the material unlit instead of baking light into it");
                ui.checkbox(&mut edited.enabled, "Enabled")
                    .on_hover_text("A disabled mesh is neither drawn, collided with nor given its sockets");
                ui.separator();
                if ui.button(icons::label(icons::TRASH, "Delete mesh")).clicked() {
                    delete = true;
                }
            });
        if delete {
            super::mesh_tool::delete_selected(self);
            return;
        }
        if edited == original {
            return;
        }
        if let Some(world) = self.project.active_scene_mut().world.grid_mut() {
            match update_mesh(world, id, |m| *m = edited.clone()) {
                Ok(_) => {}
                Err(refusal) => self.status = refusal.describe(),
            }
        }
        // The Inspector records the undo step, coalescing a drag.
        self.grid_changed();
    }
}
