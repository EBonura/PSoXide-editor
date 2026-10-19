//! The material palette and the inspector for selected grid faces, drawn
//! at the top of the Inspector column while a grid world is open.

use super::*;

use psxed_project::grid::edit::{
    edit_uv, material_of, reset_derived, set_derived_hidden, set_material, set_solid,
    set_wall_facing, toggle_split, uv_of, FaceRef, FaceTarget, UvOp,
};
use psxed_project::grid::surface::WallFacing;
use psxed_project::grid::GridSplit;

impl EditorWorkspace {
    /// Palette, then the properties of what the grid tools have picked.
    /// Returns whether a grid selection (faces, a plane, a mesh or a level
    /// verb) owns the rest of the Inspector: the World settings do not
    /// follow it. With nothing picked the node and World inspectors come
    /// after the palette, as before.
    pub(crate) fn draw_grid_panels(&mut self, ui: &mut egui::Ui) -> bool {
        self.draw_grid_palette(ui);
        ui.add_space(6.0);
        let picked_faces = self
            .project
            .active_scene()
            .world
            .grid()
            .is_some_and(|w| !self.grid.selection.faces(w).is_empty());
        let node_selected = self.selection.selected_node != NodeId::ROOT
            || !self.selection.selected_nodes.is_empty();
        let owns = if self.plane_primary().is_some() {
            self.draw_plane_inspector(ui);
            true
        } else if self.grid.tool == GridTool::Mesh || self.mesh_primary().is_some() {
            self.draw_mesh_panels(ui);
            true
        } else if self.grid.tool.is_verb() {
            self.draw_verb_inspector(ui);
            true
        } else if picked_faces || !node_selected {
            self.draw_grid_face_inspector(ui);
            picked_faces
        } else {
            false
        };
        ui.add_space(8.0);
        ui.separator();
        owns
    }

    /// Apply the palette material to every selected face.
    pub(crate) fn apply_grid_material_to_selection(&mut self, material: ResourceId) -> usize {
        let planes = self.apply_material_to_selected_planes(material);
        if planes > 0 {
            return planes;
        }
        let meshes = self.apply_material_to_selected_mesh(material);
        if meshes > 0 {
            return meshes;
        }
        // A band picked with Shift+click in the Bands tool takes the
        // material instead of the whole wall.
        if let Some((face, band)) = self.grid.band_sel {
            let before = self.project.clone();
            let scope = self.grid.band_scope;
            if let Some(world) = self.project.active_scene_mut().world.grid_mut() {
                if psxed_project::grid::edit::bands::paint_band(
                    world,
                    face,
                    band,
                    Some(material),
                    scope,
                ) {
                    self.history.record(before);
                    self.grid_changed();
                    return 1;
                }
            }
        }
        let Some(world) = self.project.active_scene().world.grid() else {
            return 0;
        };
        let faces = self.grid.selection.whole_faces(world);
        let triangles = self.grid.selection.triangles(world);
        if faces.is_empty() && triangles.is_empty() {
            return 0;
        }
        let before = self.project.clone();
        let Some(world) = self.project.active_scene_mut().world.grid_mut() else {
            return 0;
        };
        let mut changed = 0;
        for face in &faces {
            changed += usize::from(set_material(world, *face, None, Some(material)));
        }
        // A selected triangle takes the material on its own.
        for (face, tri) in &triangles {
            changed += usize::from(set_material(world, *face, Some(*tri), Some(material)));
        }
        if changed > 0 {
            self.history.record(before);
            self.grid_changed();
        }
        changed
    }

    fn draw_grid_face_inspector(&mut self, ui: &mut egui::Ui) {
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let faces = self.grid.selection.faces(world);
        let triangles = self.grid.selection.triangles(world);
        let only_triangle = match triangles.as_slice() {
            [(face, tri)] if self.grid.selection.whole_faces(world).is_empty() => {
                Some((*face, *tri))
            }
            _ => None,
        };
        egui::CollapsingHeader::new(RichText::new("Selection").strong())
            .default_open(true)
            .show(ui, |ui| {
                let Some(&primary) = faces.first() else {
                    ui.label(
                        RichText::new("Select faces with the Select tool").color(STUDIO_TEXT_WEAK),
                    );
                    return;
                };
                let (mut floors, mut ceilings, mut walls) = (0, 0, 0);
                for f in &faces {
                    match f.target {
                        FaceTarget::Surface {
                            kind: psxed_project::grid::edit::SurfaceKind::Floor,
                            ..
                        } => floors += 1,
                        FaceTarget::Surface { .. } => ceilings += 1,
                        FaceTarget::Wall { .. } | FaceTarget::Derived { .. } => walls += 1,
                    }
                }
                let plural = |n: usize, one: &str| {
                    if n == 1 {
                        format!("{n} {one}")
                    } else {
                        format!("{n} {one}s")
                    }
                };
                ui.label(format!(
                    "{}: {}, {}, {}",
                    plural(faces.len(), "face"),
                    plural(floors, "floor"),
                    plural(ceilings, "ceiling"),
                    plural(walls, "wall"),
                ));
                // One triangle selected on its own: the panel is that triangle's.
                if let Some((face, tri)) = only_triangle {
                    self.draw_triangle_controls(ui, face, tri);
                }
                self.draw_primary_face_controls(ui, primary, &faces);
            });
    }

    /// The head of the panel when one triangle of a floor or ceiling is
    /// selected: which half, its heights, and Void or Restore.
    fn draw_triangle_controls(&mut self, ui: &mut egui::Ui, face: FaceRef, tri: u8) {
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let Some(surface) = face.surface(world) else {
            return;
        };
        let name = if tri == 0 { "A" } else { "B" };
        let void = surface.is_void(usize::from(tri));
        let heights = surface.heights_for(usize::from(tri));
        let corners = surface.split.triangle(usize::from(tri));
        let psxed_project::grid::edit::FaceTarget::Surface { kind, .. } = face.target else {
            return;
        };
        ui.separator();
        ui.label(
            RichText::new(format!(
                "Triangle {name} of {} at {},{}",
                kind.label(),
                face.cell()[0],
                face.cell()[1]
            ))
            .strong(),
        );
        ui.label(
            RichText::new(format!(
                "heights {} {} {} (corners {})",
                heights[corners[0]],
                heights[corners[1]],
                heights[corners[2]],
                corners
                    .iter()
                    .map(|c| ["NW", "NE", "SE", "SW"][*c])
                    .collect::<Vec<_>>()
                    .join(" ")
            ))
            .size(11.5)
            .color(STUDIO_TEXT_WEAK),
        );
        let label = if void {
            "Restore triangle"
        } else {
            "Void triangle"
        };
        if ui
            .button(label)
            .on_hover_text(
                "A void triangle is not drawn and the sector closes itself along the diagonal",
            )
            .clicked()
        {
            let before = self.project.clone();
            if let Some(world) = self.project.active_scene_mut().world.grid_mut() {
                let which = if tri == 0 {
                    psxed_project::grid::edit::natural::Triangles::A
                } else {
                    psxed_project::grid::edit::natural::Triangles::B
                };
                if psxed_project::grid::edit::natural::set_void_leaf(
                    world, face.room, face.leaf, kind, which, !void,
                ) {
                    self.history.record(before);
                    self.grid_changed();
                    self.status = format!(
                        "Triangle {name} {}",
                        if void { "restored" } else { "voided" }
                    );
                }
            }
        }
    }

    fn draw_primary_face_controls(
        &mut self,
        ui: &mut egui::Ui,
        primary: FaceRef,
        faces: &[FaceRef],
    ) {
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let name_of = |ws: &Self, id: Option<ResourceId>| {
            id.and_then(|id| ws.project.resource_name(id))
                .unwrap_or("none")
                .to_string()
        };
        let is_surface = matches!(primary.target, FaceTarget::Surface { .. });
        let split = primary.surface(world).map(|s| {
            (
                s.split,
                s.tri_b.is_some() || s.heights_b.is_some() || !s.is_planar(),
            )
        });
        // With one triangle selected the material row is that triangle's.
        let shown_tri = match self.grid.selection.triangles(world).as_slice() {
            [(_, tri)] if self.grid.selection.whole_faces(world).is_empty() => *tri,
            _ => 0,
        };
        let mat_a = material_of(world, primary, Some(shown_tri));
        let mat_b = material_of(world, primary, Some(1));
        let uv = uv_of(world, primary, None);
        let heights = primary.heights(world, 0);
        ui.separator();
        ui.label(
            RichText::new(match primary.target {
                FaceTarget::Surface { kind: k, .. } => {
                    format!(
                        "{} at {},{}",
                        k.label(),
                        primary.cell()[0],
                        primary.cell()[1]
                    )
                }
                FaceTarget::Wall { edge, index } => format!(
                    "{} wall {index} at {},{}",
                    edge.label(),
                    primary.cell()[0],
                    primary.cell()[1]
                ),
                FaceTarget::Derived {
                    edge, kind, span, ..
                } => format!(
                    "{} {} (derived{}) at {},{}",
                    edge.label(),
                    kind.label(),
                    if span > 0 {
                        format!(", floor {}", span + 1)
                    } else {
                        String::new()
                    },
                    primary.cell()[0],
                    primary.cell()[1]
                ),
            })
            .strong(),
        );
        if let Some(h) = heights {
            ui.label(
                RichText::new(format!("heights {} {} {} {}", h[0], h[1], h[2], h[3]))
                    .size(11.5)
                    .color(STUDIO_TEXT_WEAK),
            );
        }
        let mut ops: Vec<UvOp> = Vec::new();
        let mut fit_across: Option<[f32; 2]> = None;
        let mut toggle = false;
        let mut facing: Option<WallFacing> = None;
        let mut solid: Option<bool> = None;
        let mut hidden: Option<bool> = None;
        let mut reset_override = false;
        ui.horizontal(|ui| {
            ui.label("Material");
            ui.label(RichText::new(name_of(self, mat_a)).color(STUDIO_ACCENT));
        });
        if is_surface {
            if let Some((s, bent)) = split {
                ui.horizontal(|ui| {
                    ui.label("Split");
                    if ui
                        .button(match s {
                            GridSplit::NwSe => "NW to SE",
                            GridSplit::NeSw => "NE to SW",
                        })
                        .on_hover_text("Flip the diagonal")
                        .clicked()
                    {
                        toggle = true;
                    }
                    if !bent {
                        ui.label(
                            RichText::new("(flat: no visible change)")
                                .color(STUDIO_TEXT_WEAK)
                                .size(10.0),
                        );
                    }
                });
                if mat_a != mat_b {
                    ui.label(
                        RichText::new(format!(
                            "Triangles: {} / {}",
                            name_of(self, mat_a),
                            name_of(self, mat_b)
                        ))
                        .size(11.0)
                        .color(STUDIO_TEXT_WEAK),
                    );
                }
            }
        }
        if let Some(uv) = uv {
            let report = psxed_project::grid::uv::report_for(
                world,
                &psxed_project::grid::uv::ProjectUvDefaults(&self.project),
                primary,
                None,
            );
            ops.extend(super::tiling_panel::draw_tiling(ui, uv, report, None));
            let surfaces = faces
                .iter()
                .filter(|f| matches!(f.target, FaceTarget::Surface { .. }))
                .count();
            fit_across = super::tiling_panel::draw_fit_across(ui, surfaces);
        }
        match primary.target {
            FaceTarget::Wall { .. } => {
                if let Some(w) = primary.wall_of(world) {
                    let mut current = w.facing;
                    let mut solid_now = w.solid;
                    ui.horizontal(|ui| {
                        ui.label("Faces");
                        egui::ComboBox::from_id_salt("grid_wall_facing")
                            .selected_text(match current {
                                WallFacing::Front => "inside",
                                WallFacing::Back => "outside",
                                WallFacing::Both => "both sides",
                            })
                            .show_ui(ui, |ui| {
                                for (value, label) in [
                                    (WallFacing::Front, "inside"),
                                    (WallFacing::Back, "outside"),
                                    (WallFacing::Both, "both sides"),
                                ] {
                                    if ui.selectable_label(current == value, label).clicked() {
                                        current = value;
                                    }
                                }
                            });
                    });
                    if current != w.facing {
                        facing = Some(current);
                    }
                    if ui.checkbox(&mut solid_now, "Solid").changed() {
                        solid = Some(solid_now);
                    }
                }
            }
            FaceTarget::Surface { .. } => {
                if let Some(s) = primary.surface(world) {
                    let mut walkable = s.walkable;
                    if ui.checkbox(&mut walkable, "Walkable").changed() {
                        solid = Some(walkable);
                    }
                }
            }
            FaceTarget::Derived { .. } => {
                if let Some(d) = primary.derived_wall(world) {
                    ui.label(
                        RichText::new(format!(
                            "from the height difference with {},{}",
                            d.owner[0], d.owner[1]
                        ))
                        .size(11.0)
                        .color(STUDIO_TEXT_WEAK),
                    );
                    let mut solid_now = d.blocks();
                    if ui
                        .checkbox(&mut solid_now, "Solid")
                        .on_hover_text(
                            "A wall record that blocks movement. Closure walls are solid by default; risers are left to the step rule.",
                        )
                        .changed()
                    {
                        solid = Some(solid_now);
                    }
                    let mut hidden_now = d.hidden;
                    if ui
                        .checkbox(&mut hidden_now, "Hidden")
                        .on_hover_text("Do not draw this face (it keeps blocking if it is solid)")
                        .changed()
                    {
                        hidden = Some(hidden_now);
                    }
                    if ui
                        .button("Back to automatic")
                        .on_hover_text("Forget the material, texture and flags set on this face")
                        .clicked()
                    {
                        reset_override = true;
                    }
                }
            }
        }
        if matches!(
            primary.target,
            FaceTarget::Wall { .. } | FaceTarget::Derived { .. }
        ) {
            self.draw_wall_run_bands(ui, primary);
        }
        // Apply what changed to every selected face.
        if toggle
            || !ops.is_empty()
            || fit_across.is_some()
            || facing.is_some()
            || solid.is_some()
            || hidden.is_some()
            || reset_override
        {
            let faces = faces.to_vec();
            let before = self.project.clone();
            if let Some(world) = self.project.active_scene_mut().world.grid_mut() {
                if let Some(repeats) = fit_across {
                    // The patch is the selection's bounding rectangle. The
                    // material defaults come from the project as it was.
                    let defaults = psxed_project::grid::uv::ProjectUvDefaults(&before);
                    match psxed_project::grid::uv_region::fit_across(
                        world, &defaults, &faces, None, repeats,
                    ) {
                        Ok(report) => self.status = report.describe(repeats).replace('\n', "; "),
                        Err(error) => self.status = error,
                    }
                }
                for face in &faces {
                    if toggle {
                        toggle_split(world, *face);
                    }
                    for op in &ops {
                        edit_uv(world, *face, None, *op);
                    }
                    if let Some(f) = facing {
                        set_wall_facing(world, *face, f);
                    }
                    if let Some(v) = solid {
                        set_solid(world, *face, v);
                    }
                    if let Some(v) = hidden {
                        set_derived_hidden(world, *face, v);
                    }
                    if reset_override {
                        reset_derived(world, *face);
                    }
                }
            }
            // The Inspector records the undo step, coalescing a drag.
            if self.project != before {
                self.grid_changed();
            }
        }
    }
}

impl EditorWorkspace {
    /// Add an empty room one storey above the highest room and make it the
    /// active one. One undo step.
    pub(crate) fn grid_add_room(&mut self) {
        let before = self.project.clone();
        let Some(world) = self.project.active_scene_mut().world.grid_mut() else {
            return;
        };
        if world.rooms.len() >= psxed_project::grid::limits::MAX_ROOMS {
            self.status = "Too many rooms".to_string();
            return;
        }
        let top = world.rooms.iter().map(|r| r.y_offset).max().unwrap_or(0);
        let origin = world.rooms.last().map_or([0, 0], |r| r.origin);
        let mut room =
            psxed_project::grid::GridRoom::new_derived(format!("Room {}", world.rooms.len()), 1, 1);
        room.origin = origin;
        room.y_offset = top + 3 * psxed_project::grid::SECTOR_UNITS;
        world.rooms.push(room);
        let index = world.rooms.len() - 1;
        self.grid_active_room = Some(index);
        self.grid.last_room = index;
        self.history.record(before);
        self.status = format!("Added room {index}; draw its floor with the Floor tool");
        self.grid_changed();
    }

    /// Remove room `index`, keeping the hidden set, the active room and the
    /// selection pointing at the right rooms. One undo step.
    pub(crate) fn grid_delete_room(&mut self, index: usize) {
        let before = self.project.clone();
        let Some(world) = self.project.active_scene_mut().world.grid_mut() else {
            return;
        };
        if world.rooms.len() <= 1 || index >= world.rooms.len() {
            return;
        }
        world.rooms.remove(index);
        let shift = |i: usize| if i > index { i - 1 } else { i };
        self.grid_hidden_rooms = self
            .grid_hidden_rooms
            .iter()
            .filter(|i| **i != index)
            .map(|i| shift(*i))
            .collect();
        self.grid_active_room = self.grid_active_room.filter(|i| *i != index).map(shift);
        self.grid.last_room = shift(self.grid.last_room).min(self.grid_room_count() - 1);
        self.grid.selection.clear();
        self.history.record(before);
        self.status = format!("Deleted room {index}");
        self.grid_changed();
    }

    fn grid_room_count(&self) -> usize {
        self.project
            .active_scene()
            .world
            .grid()
            .map_or(1, |w| w.rooms.len().max(1))
    }
}
