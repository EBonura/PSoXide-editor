//! The material palette and the inspector for selected grid faces, drawn
//! at the top of the Inspector column while a grid world is open.

use super::*;

use psxed_project::grid::edit::{
    edit_uv, material_of, reset_derived, set_derived_hidden, set_material, set_solid,
    set_wall_facing, toggle_split, uv_of, FaceRef, FaceTarget, UvOp,
};
use psxed_project::grid::surface::WallFacing;
use psxed_project::grid::GridSplit;

/// Sectors along a side of a room made with the Rooms plus button (the
/// starter room's size).
const NEW_ROOM_SIDE: usize = 6;
/// Ceiling height of such a room when the room beside it has no ceiling to
/// copy: two sectors, like the starter.
const NEW_ROOM_HEIGHT: i32 = 2 * psxed_project::grid::SECTOR_UNITS;

impl EditorWorkspace {
    /// Palette, then the properties of what the grid tools have picked.
    /// Returns whether a grid selection (faces, a plane, a mesh or a level
    /// verb) owns the rest of the Inspector: the World settings do not
    /// follow it. With nothing picked the node and World inspectors come
    /// after the palette, as before.
    pub(crate) fn draw_grid_panels(&mut self, ui: &mut egui::Ui) -> bool {
        // A project opens with a paint material, whether or not the Library
        // is open.
        self.ensure_paint_material();
        let picked_faces = self
            .project
            .active_scene()
            .world
            .grid()
            .is_some_and(|w| !self.grid.selection.faces(w).is_empty());
        let node_selected = self.selection.selected_node != NodeId::ROOT
            || !self.selection.selected_nodes.is_empty();
        // A face or an object picked after a room stands the room down.
        if picked_faces || node_selected {
            self.selection.selected_room = None;
        }
        let room = self.selection.selected_room;
        let what = self.properties_subject(picked_faces, node_selected);
        self.draw_properties_header(ui, &what);
        let owns = if let Some(room) = room {
            self.draw_grid_room_inspector(ui, room);
            true
        } else if self.plane_primary().is_some() {
            self.draw_plane_inspector(ui);
            true
        } else if self.grid.tool == GridTool::Mesh || self.mesh_primary().is_some() {
            self.draw_mesh_panels(ui);
            true
        } else if self.grid.tool.is_verb() {
            self.draw_verb_inspector(ui);
            true
        } else if picked_faces {
            self.draw_grid_face_inspector(ui);
            true
        } else if !node_selected || self.world_is_selected() {
            // Nothing picked: the Inspector is about the world.
            self.draw_world_inspector(ui);
            true
        } else {
            false
        };
        ui.add_space(8.0);
        ui.separator();
        owns
    }

    /// What the Properties section edits, in a few words.
    fn properties_subject(&self, picked_faces: bool, node_selected: bool) -> String {
        if let Some(room) = self.selection.selected_room {
            let name = self
                .project
                .active_scene()
                .world
                .grid()
                .and_then(|w| w.rooms.get(room))
                .map_or("?", |r| r.name.as_str());
            return format!("Room: {name}");
        }
        if let Some(id) = self.plane_primary() {
            let _ = id;
            return "Plane".to_string();
        }
        if self.grid.tool == GridTool::Mesh || self.mesh_primary().is_some() {
            return "Mesh".to_string();
        }
        if self.grid.tool.is_verb() {
            return format!("{} region", self.grid.tool.label());
        }
        if picked_faces {
            let Some(world) = self.project.active_scene().world.grid() else {
                return "Faces".to_string();
            };
            let triangles = self.grid.selection.triangles(world);
            let whole = self.grid.selection.whole_faces(world);
            return match (triangles.as_slice(), whole.len()) {
                ([(_, tri)], 0) => format!("Triangle {}", if *tri == 0 { "A" } else { "B" }),
                (_, 1) => "Face".to_string(),
                _ => format!("{} faces", self.grid.selection.faces(world).len()),
            };
        }
        if node_selected {
            let id = self.selection.selected_node;
            if id == NodeId::ROOT {
                return "World".to_string();
            }
            // "Light: Torch", "Actor: Aletha": what it is, then its name. A
            // part of an actor is the actor.
            let scene = self.project.active_scene();
            let id = owner_object(scene, id);
            return scene.node(id).map_or_else(
                || "Node".to_string(),
                |n| {
                    let kind = match (n.kind.label(), classify(scene, n)) {
                        ("Point Light", _) => "Light",
                        ("Entity", ObjectKind::ModelActor) => "Prop",
                        ("Entity", ObjectKind::Interaction) => "Interaction",
                        ("Entity", _) => "Actor",
                        (other, _) => other,
                    };
                    format!("{kind}: {}", compact_middle(&n.name, 24))
                },
            );
        }
        "World".to_string()
    }

    /// The header of the Properties section: the name, then what it is about.
    fn draw_properties_header(&self, ui: &mut egui::Ui, what: &str) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Properties").strong().size(12.5));
            ui.label(RichText::new(what).color(STUDIO_ACCENT).size(12.0));
        });
        ui.separator();
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
        let bands = self.grid.selection.bands(world);
        if faces.is_empty() && triangles.is_empty() && bands.is_empty() {
            return 0;
        }
        let before = self.project.clone();
        let Some(world) = self.project.active_scene_mut().world.grid_mut() else {
            return 0;
        };
        let mut changed = 0;
        for face in &faces {
            // A whole wall takes the paint on its bands too (those that
            // kept a material of their own).
            if psxed_project::grid::edit::bands::is_bandable(face) {
                let done = psxed_project::grid::edit::bands::paint_whole_wall(
                    world,
                    *face,
                    Some(material),
                );
                changed += usize::from(done.changed());
            } else {
                changed += usize::from(set_material(world, *face, None, Some(material)));
            }
        }
        // A selected triangle takes the material on its own.
        for (face, tri) in &triangles {
            changed += usize::from(set_material(world, *face, Some(*tri), Some(material)));
        }
        // So does a selected band: the material goes into the band set of
        // that wall segment, on that band only (the rest of the run keeps
        // its own).
        for (face, band) in &bands {
            changed += usize::from(psxed_project::grid::edit::bands::paint_band(
                world,
                *face,
                *band,
                Some(material),
                psxed_project::grid::edit::bands::BandScope::Piece,
            ));
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
                        RichText::new("Select a face to see its heights and material")
                            .color(STUDIO_TEXT_WEAK),
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
                    "{} {}{} at {},{}",
                    edge.label(),
                    kind.label(),
                    if span > 0 {
                        format!(" (floor {})", span + 1)
                    } else {
                        String::new()
                    },
                    primary.cell()[0],
                    primary.cell()[1]
                ),
            })
            .strong(),
        );
        let mut wall_edit = super::wall_section::WallEdit::default();
        if matches!(
            primary.target,
            FaceTarget::Wall { .. } | FaceTarget::Derived { .. }
        ) {
            wall_edit = super::wall_section::draw(ui, world, primary);
        } else if let Some(h) = heights {
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
            let world_uv = world.default_uv();
            ops.extend(super::tiling_panel::draw_tiling(
                ui, uv, report, None, world_uv,
            ));
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
                            "follows the floors beside it ({},{})",
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
                        .checkbox(&mut hidden_now, "Open (not drawn)")
                        .on_hover_text("Do not draw this wall (it keeps blocking if it is solid)")
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
            || wall_edit.heights.is_some()
            || wall_edit.reset
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
                if let Some(text) = super::wall_section::apply(world, &wall_edit) {
                    self.status = text;
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
        // Beside the room being worked in (the selected one, else the target),
        // on its floor level, walled by the room's own derived closure walls,
        // with a floor and a ceiling in the look of that room.
        let reference = self
            .selection
            .selected_room
            .or(self.grid_active_room)
            .unwrap_or(self.grid.last_room)
            .min(world.rooms.len().saturating_sub(1));
        let look = world.rooms.get(reference).map(|r| {
            let sector = r
                .leaves
                .iter()
                .find_map(|(_, sector)| sector.ground_floor().map(|_| sector));
            (
                r.origin,
                r.width as i32,
                r.y_offset,
                r.defaults.wall_material,
                sector
                    .and_then(|s| s.ground_floor())
                    .and_then(|f| f.tri_a.material),
                sector
                    .and_then(|s| s.ground_ceiling())
                    .map(|c| (c.heights[0], c.tri_a.material)),
            )
        });
        let (ref_origin, ref_width, y_offset, wall, floor, ceiling) =
            look.unwrap_or(([0, 0], 0, 0, None, None, None));
        let side = NEW_ROOM_SIDE;
        // East of the reference with a sector of gap, further east while that
        // footprint would overlap another room.
        let mut origin = [ref_origin[0] + ref_width + 1, ref_origin[1]];
        while world.rooms.iter().any(|r| {
            origin[0] < r.origin[0] + r.width as i32
                && r.origin[0] < origin[0] + side as i32
                && origin[1] < r.origin[1] + r.depth as i32
                && r.origin[1] < origin[1] + side as i32
        }) {
            origin[0] += 1;
        }
        let (ceiling_height, ceiling_material) = ceiling.unwrap_or((NEW_ROOM_HEIGHT, floor));
        let mut room = psxed_project::grid::GridRoom::new_derived(
            format!("Room {}", world.rooms.len()),
            side,
            side,
        );
        room.origin = origin;
        room.y_offset = y_offset;
        room.defaults.wall_material = wall.or(floor);
        for z in 0..side {
            for x in 0..side {
                if let Some(sector) = room.sector_or_insert(x, z) {
                    sector.set_ground_floor(Some(psxed_project::grid::GridSurface::flat_auto(
                        0, floor,
                    )));
                    sector.set_ground_ceiling(Some(psxed_project::grid::GridSurface::flat_auto(
                        ceiling_height,
                        ceiling_material,
                    )));
                }
            }
        }
        world.rooms.push(room);
        let index = world.rooms.len() - 1;
        self.grid_active_room = Some(index);
        self.grid.last_room = index;
        self.history.record(before);
        self.status = format!("Added room {index}: {NEW_ROOM_SIDE} by {NEW_ROOM_SIDE} sectors with a floor, a ceiling and walls");
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
        let name = if world.rooms[index].name.is_empty() {
            format!("{index}")
        } else {
            world.rooms[index].name.clone()
        };
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
        self.grid_changed();
        self.offer_undo(format!("Deleted room {name}."));
    }

    fn grid_room_count(&self) -> usize {
        self.project
            .active_scene()
            .world
            .grid()
            .map_or(1, |w| w.rooms.len().max(1))
    }
}
