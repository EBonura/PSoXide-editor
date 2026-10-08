use super::*;
use psxed_project::brush::{Brush, Plane};
use psxed_project::terrain::{SculptMode, Terrain, TerrainShape};

/// A private working copy. Apply creates one document undo step; closing
/// the window discards the draft. No project mutation during sculpt strokes.
pub(crate) struct TerrainEditor {
    terrain: Terrain,
    history: Vec<Terrain>,
    redo: Vec<Terrain>,
    source: Vec<Brush>,
    group: Option<NodeId>,
    project_dir: PathBuf,
    scene_name: String,
    name: String,
    material: Option<ResourceId>,
    repaint_material: bool,
    enclose_sky: bool,
    sky_ceiling: i32,
    cells: [usize; 2],
    spacing: [i32; 2],
    origin: [i32; 3],
    amplitude: i32,
    seed: u32,
    roughness: f64,
    shape: TerrainShape,
    mode: SculptMode,
    radius: f64,
    strength: f64,
    flatten_y: f64,
    stroke: bool,
    last_dab: Option<[f64; 2]>,
    yaw: f32,
    error: Option<String>,
}
impl TerrainEditor {
    fn new(
        project_dir: PathBuf,
        scene_name: String,
        origin: [i32; 3],
        material: Option<ResourceId>,
    ) -> Self {
        let terrain =
            Terrain::generate([8; 2], [512; 2], origin, 768, 42, TerrainShape::Hills, 0.4).unwrap();
        Self {
            terrain,
            history: vec![],
            redo: vec![],
            source: vec![],
            group: None,
            project_dir,
            scene_name,
            name: "Terrain".into(),
            material,
            repaint_material: false,
            enclose_sky: false,
            sky_ceiling: origin[1].saturating_add(8192),
            cells: [8; 2],
            spacing: [512; 2],
            origin,
            amplitude: 768,
            seed: 42,
            roughness: 0.4,
            shape: TerrainShape::Hills,
            mode: SculptMode::Raise,
            radius: 2.0,
            strength: 512.0,
            flatten_y: f64::from(origin[1]),
            stroke: false,
            last_dab: None,
            yaw: 0.65,
            error: None,
        }
    }
    fn checkpoint(&mut self) {
        if self.history.len() >= 64 {
            self.history.remove(0);
        }
        self.history.push(self.terrain.clone());
        self.redo.clear();
    }
    fn undo(&mut self) {
        if let Some(previous) = self.history.pop() {
            self.redo
                .push(std::mem::replace(&mut self.terrain, previous));
        }
    }
    fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.history
                .push(std::mem::replace(&mut self.terrain, next));
        }
    }
    fn regenerate(&mut self) {
        match Terrain::generate(
            self.cells,
            self.spacing,
            self.origin,
            self.amplitude,
            self.seed,
            self.shape,
            self.roughness,
        ) {
            Ok(t) => {
                self.checkpoint();
                self.terrain = t;
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }
    fn draw(&mut self, ui: &mut egui::Ui, materials: &[(ResourceId, String)]) {
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(250.0);
                ui.heading("Generate");
                egui::ComboBox::from_id_salt("terrain_shape").selected_text(self.shape.label()).show_ui(ui,|ui| {for shape in TerrainShape::ALL {ui.selectable_value(&mut self.shape,shape,shape.label());}});
                egui::Grid::new("terrain_recipe").num_columns(2).show(ui,|ui| {
                    ui.label("Cells X / Z");ui.horizontal(|ui| {for n in &mut self.cells {ui.add(egui::DragValue::new(n).range(1..=16));}});ui.end_row();
                    ui.label("Cell size X / Z");ui.horizontal(|ui| {for n in &mut self.spacing {if ui.add(egui::DragValue::new(n).speed(16).range(16..=16384)).changed(){*n=((*n+8)/16)*16;}}});ui.end_row();
                    ui.label("Height");ui.add(egui::DragValue::new(&mut self.amplitude).speed(16).range(0..=8192));ui.end_row();
                    ui.label("Roughness");ui.add(egui::Slider::new(&mut self.roughness,0.0..=1.0).show_value(false));ui.end_row();
                    ui.label("Seed");ui.add(egui::DragValue::new(&mut self.seed));ui.end_row();
                });
                ui.label("Origin X / Y / Z (world units)");
                ui.horizontal(|ui| {for n in &mut self.origin {if ui.add(egui::DragValue::new(n).speed(16).range(-120000..=120000)).changed(){*n=(*n/16)*16;}}});
                ui.horizontal(|ui| {if ui.button("Generate").clicked(){self.regenerate();}if ui.button("New seed").clicked(){self.seed=self.seed.wrapping_add(1);self.regenerate();}});
                ui.weak("Generate replaces the draft. Undo restores your sculpting.");
                ui.separator();ui.heading("Sculpt");
                ui.horizontal_wrapped(|ui| {for mode in SculptMode::ALL {ui.selectable_value(&mut self.mode,mode,mode.label());}});
                ui.add(egui::Slider::new(&mut self.radius,0.5..=8.0).text("Radius (cells)"));
                ui.add(egui::Slider::new(&mut self.strength,32.0..=2048.0).text("Strength"));
                ui.horizontal(|ui| {ui.label("Flatten height");ui.add(egui::DragValue::new(&mut self.flatten_y).speed(16).range(-120000.0..=120000.0));});
                ui.weak("Drag on the map to sculpt. Shift: smooth. Alt: lower. Right-click: sample flatten height.");
                ui.horizontal(|ui| {if ui.add_enabled(!self.history.is_empty(),egui::Button::new("Undo stroke")).clicked(){self.undo();}if ui.add_enabled(!self.redo.is_empty(),egui::Button::new("Redo")).clicked(){self.redo();}});
                ui.separator();ui.label("Group name");ui.text_edit_singleline(&mut self.name);
                egui::ComboBox::from_id_salt("terrain_material").selected_text(materials.iter().find(|m|Some(m.0)==self.material).map_or("Default",|m|m.1.as_str())).show_ui(ui,|ui| {
                    if ui.selectable_value(&mut self.material,None,"Default").changed(){self.repaint_material=true;}
                    for (id,name) in materials {if ui.selectable_value(&mut self.material,Some(*id),name).changed(){self.repaint_material=true;}}
                });
                if self.group.is_some(){ui.weak("Existing face materials are kept unless you pick a material.");}
                else {
                    ui.separator();
                    ui.checkbox(&mut self.enclose_sky, "Enclose with sky");
                    if self.enclose_sky {
                        ui.horizontal(|ui| { ui.label("Ceiling Y"); if ui.add(egui::DragValue::new(&mut self.sky_ceiling).speed(16)).changed() { self.sky_ceiling=(self.sky_ceiling/16)*16; } });
                        ui.weak("Closes the four edges and ceiling. For a standalone patch; cut entrances when joining another area. Sky brushes remain separately editable.");
                    }
                }
            });
            ui.separator();
            ui.vertical(|ui| {
                ui.heading("Shape the landscape");
                ui.label("Top view · north is up");
                self.draw_sculpt_map(ui);
                ui.label("3D shape preview · drag to orbit");
                self.draw_preview(ui);
                let t=&self.terrain;
                ui.weak(format!("{} × {} units · {} surface triangles · {} solid brushes",t.cells[0] as i32*t.spacing[0],t.cells[1] as i32*t.spacing[1],t.cells[0]*t.cells[1]*2,t.cells[0]*t.cells[1]*2));
            });
        });
        if let Some(error) = &self.error {
            ui.colored_label(egui::Color32::from_rgb(255, 130, 105), error);
        }
    }
    fn draw_sculpt_map(&mut self, ui: &mut egui::Ui) {
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(470.0, 240.0), Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 5.0, egui::Color32::from_rgb(20, 28, 34));
        let scale = (rect.width() - 24.0) / (self.terrain.cells[0] as f32);
        let scale = scale.min((rect.height() - 24.0) / (self.terrain.cells[1] as f32));
        let size = Vec2::new(
            self.terrain.cells[0] as f32 * scale,
            self.terrain.cells[1] as f32 * scale,
        );
        let area = egui::Rect::from_center_size(rect.center(), size);
        let pos = |p: [i32; 3]| {
            egui::pos2(
                area.left()
                    + (p[0] - self.terrain.origin[0]) as f32 / self.terrain.spacing[0] as f32
                        * scale,
                area.top()
                    + (p[2] - self.terrain.origin[1]) as f32 / self.terrain.spacing[1] as f32
                        * scale,
            )
        };
        for triangle in self.terrain.triangles() {
            painter.add(egui::Shape::convex_polygon(
                triangle.map(pos).to_vec(),
                terrain_color(triangle),
                egui::Stroke::new(0.5, egui::Color32::from_black_alpha(85)),
            ));
        }
        let pointer = response
            .interact_pointer_pos()
            .or(response.hover_pos())
            .filter(|p| area.contains(*p));
        let down = response.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_down());
        if let Some(p) = pointer {
            let center = [
                f64::from((p.x - area.left()) / scale),
                f64::from((p.y - area.top()) / scale),
            ];
            painter.circle_stroke(
                p,
                self.radius as f32 * scale,
                egui::Stroke::new(1.5, egui::Color32::WHITE),
            );
            if response.secondary_clicked() {
                let x = (center[0].round() as usize).min(self.terrain.cells[0]);
                let z = (center[1].round() as usize).min(self.terrain.cells[1]);
                self.flatten_y = self.terrain.heights[z * (self.terrain.cells[0] + 1) + x];
                self.mode = SculptMode::Flatten;
            }
            if down {
                if !self.stroke {
                    self.checkpoint();
                    self.stroke = true;
                }
                let dt = ui.input(|i| i.stable_dt).clamp(0.001, 0.05) as f64;
                let mode = ui.input(|i| {
                    if i.modifiers.shift {
                        SculptMode::Smooth
                    } else if i.modifiers.alt {
                        SculptMode::Lower
                    } else {
                        self.mode
                    }
                });
                let from = self.last_dab.unwrap_or(center);
                let distance =
                    ((from[0] - center[0]).powi(2) + (from[1] - center[1]).powi(2)).sqrt();
                let steps = (distance / (self.radius * 0.25)).ceil().clamp(1.0, 64.0) as usize;
                let amount = if matches!(mode, SculptMode::Smooth | SculptMode::Flatten) {
                    self.strength / 128.0 * dt
                } else {
                    self.strength * dt
                };
                for step in 1..=steps {
                    let f = step as f64 / steps as f64;
                    self.terrain.sculpt(
                        [
                            from[0] + (center[0] - from[0]) * f,
                            from[1] + (center[1] - from[1]) * f,
                        ],
                        self.radius,
                        amount / steps as f64,
                        mode,
                        self.flatten_y,
                    );
                }
                self.last_dab = Some(center);
                ui.ctx().request_repaint();
            }
        } else {
            self.last_dab = None;
        }
        if !ui.input(|i| i.pointer.primary_down()) {
            self.stroke = false;
            self.last_dab = None;
        }
    }
    fn draw_preview(&mut self, ui: &mut egui::Ui) {
        let (rect, response) = ui.allocate_exact_size(Vec2::new(470.0, 215.0), Sense::drag());
        if response.dragged() {
            self.yaw += ui.input(|i| i.pointer.delta().x) * 0.012;
        }
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 5.0, egui::Color32::from_rgb(17, 24, 31));
        let t = &self.terrain;
        let w = t.cells[0] as f32 * t.spacing[0] as f32;
        let d = t.cells[1] as f32 * t.spacing[1] as f32;
        let high = t.heights.iter().copied().fold(f64::NEG_INFINITY, f64::max) as f32;
        let low = t.bottom as f32;
        let (sin, cos) = self.yaw.sin_cos();
        let scale = (rect.width() * 0.8 / (w + d))
            .min(rect.height() * 0.78 / ((w + d) * 0.32 + (high - low) * 0.75).max(1.0));
        let project = |p: [i32; 3]| {
            let x = (p[0] - t.origin[0]) as f32 - w * 0.5;
            let z = (p[2] - t.origin[1]) as f32 - d * 0.5;
            let depth = x * sin + z * cos;
            let y = p[1] as f32 - (high + low) * 0.5;
            (
                egui::pos2(
                    rect.center().x + (x * cos - z * sin) * scale,
                    rect.center().y + (depth * 0.5 - y * 0.866) * scale,
                ),
                depth * 0.866 + y * 0.5,
            )
        };
        let mut faces: Vec<_> = t
            .triangles()
            .into_iter()
            .map(|tri| (tri, terrain_color(tri)))
            .collect();
        // Solid outer skirt, matching the committed brushes.
        for tri in t.triangles() {
            for i in 0..3 {
                let a = tri[i];
                let b = tri[(i + 1) % 3];
                let boundary = (a[0] == b[0]
                    && (a[0] == t.origin[0] || a[0] == t.origin[0] + w as i32))
                    || (a[2] == b[2] && (a[2] == t.origin[1] || a[2] == t.origin[1] + d as i32));
                if boundary {
                    let c = [b[0], t.bottom, b[2]];
                    let e = [a[0], t.bottom, a[2]];
                    let color = egui::Color32::from_rgb(64, 73, 68);
                    faces.push(([b, a, e], color));
                    faces.push(([b, e, c], color));
                }
            }
        }
        faces.sort_by(|a, b| {
            let depth = |v: &[[i32; 3]; 3]| v.iter().map(|p| project(*p).1).sum::<f32>();
            depth(&a.0).total_cmp(&depth(&b.0))
        });
        // Emit triangles directly: outlined convex paths produce long miter
        // spikes when a steep terrain facet projects almost edge-on.
        let mut mesh = egui::Mesh::default();
        for (tri, color) in faces {
            let points = tri.map(|p| project(p).0);
            let a = points[1] - points[0];
            let b = points[2] - points[0];
            if a.x * b.y - a.y * b.x >= -0.05 {
                continue;
            }
            let first = mesh.vertices.len() as u32;
            for point in points {
                mesh.colored_vertex(point, color);
            }
            mesh.add_triangle(first, first + 1, first + 2);
        }
        painter.add(egui::Shape::mesh(mesh));
    }
}
fn terrain_color(tri: [[i32; 3]; 3]) -> egui::Color32 {
    let p = Plane::from_points(tri).unwrap();
    let n = p.normal.map(|v| v as f64);
    let len = (n.iter().map(|v| v * v).sum::<f64>()).sqrt();
    let slope = n[1] / len;
    let shade = (0.65 + (n[1] * 0.7 - n[0] * 0.35 - n[2] * 0.3) / len * 0.35).clamp(0.35, 1.0);
    let rgb = if slope < 0.7 {
        [142.0, 145.0, 137.0]
    } else {
        [117.0, 166.0, 119.0]
    };
    egui::Color32::from_rgb(
        (rgb[0] * shade) as u8,
        (rgb[1] * shade) as u8,
        (rgb[2] * shade) as u8,
    )
}
impl EditorWorkspace {
    pub(crate) fn open_terrain_editor(&mut self, edit: bool) {
        let origin = self
            .orthographic_focus
            .map(|n| ((n as i32 / 16) * 16).clamp(-110000, 110000));
        let mut draft = TerrainEditor::new(
            self.project_dir.clone(),
            self.project.active_scene().name.clone(),
            origin,
            self.paint_material_for("terrain"),
        );
        if edit {
            let group = self
                .selected_brush
                .and_then(|i| {
                    self.project
                        .active_scene()
                        .brushes
                        .get(i)
                        .and_then(|b| b.group)
                })
                .unwrap_or(self.selection.selected_node);
            if !self.node_is_group(group) {
                self.status = "Select a terrain group first.".into();
                return;
            }
            let brushes: Vec<_> = self
                .project
                .active_scene()
                .brushes
                .iter()
                .filter(|b| b.group == Some(group))
                .cloned()
                .collect();
            match Terrain::from_brushes(&brushes) {
                Ok(t) => {
                    draft.cells = t.cells;
                    draft.spacing = t.spacing;
                    draft.origin = [t.origin[0], t.bottom + 256, t.origin[1]];
                    draft.flatten_y = t.heights[0];
                    draft.terrain = t;
                    draft.group = Some(group);
                    draft.source = brushes;
                    draft.name = self
                        .project
                        .active_scene()
                        .node(group)
                        .unwrap()
                        .name
                        .clone();
                }
                Err(e) => {
                    self.status = e;
                    return;
                }
            }
        }
        self.terrain_editor = Some(draft);
    }
    pub(crate) fn draw_terrain_editor(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.terrain_editor.take() else {
            return;
        };
        if draft.project_dir != self.project_dir
            || draft.scene_name != self.project.active_scene().name
        {
            return;
        }
        let materials: Vec<_> = self
            .project
            .resources
            .iter()
            .filter(|r| matches!(r.data, ResourceData::Material(_)))
            .map(|r| (r.id, r.name.clone()))
            .collect();
        let mut open = true;
        let mut apply = false;
        egui::Window::new("Low-poly terrain")
            .id(egui::Id::new("terrain_editor"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size(Vec2::new(760.0, 710.0))
            .min_width(740.0)
            .default_pos(egui::pos2(190.0, 80.0))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height((ctx.screen_rect().height() - 180.0).clamp(250.0, 720.0))
                    .show(ui, |ui| {
                        draft.draw(ui, &materials);
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    apply = ui
                        .button(if draft.group.is_some() {
                            "Apply terrain edits"
                        } else {
                            "Add terrain to scene"
                        })
                        .clicked();
                    ui.weak("Solid collision included · one scene undo step");
                });
            });
        if apply {
            match self.commit_terrain(&draft) {
                Ok(()) => return,
                Err(e) => draft.error = Some(e),
            }
        }
        if open {
            self.terrain_editor = Some(draft);
        }
    }
    fn commit_terrain(&mut self, draft: &TerrainEditor) -> Result<(), String> {
        let mut brushes = draft.terrain.brushes(draft.material)?;
        if let Some(group) = draft.group {
            let current: Vec<_> = self
                .project
                .active_scene()
                .brushes
                .iter()
                .filter(|b| b.group == Some(group))
                .cloned()
                .collect();
            if current != draft.source || !self.node_is_group(group) {
                return Err("The source group changed while this draft was open. Close and reopen it before applying.".into());
            }
            if !draft.repaint_material {
                // Preserve per-face materials/UVs on sculpt edits with unchanged
                // topology, using footprint rather than vector order.
                for brush in &mut brushes {
                    let footprint = |b: &Brush| {
                        let mut p = b
                            .faces
                            .iter()
                            .find(|f| Plane::from_points(f.points).is_some_and(|p| p.normal[1] > 0))
                            .unwrap()
                            .points
                            .map(|p| [p[0], p[2]]);
                        p.sort();
                        p
                    };
                    if let Some(old) = current
                        .iter()
                        .find(|old| footprint(old) == footprint(brush))
                    {
                        for face in &mut brush.faces {
                            let normal = Plane::from_points(face.points).unwrap().normal;
                            let old_face = old.faces.iter().find(|f| {
                                let n = Plane::from_points(f.points).unwrap().normal;
                                if normal[1] != 0 {
                                    n[1].signum() == normal[1].signum()
                                } else {
                                    n[1] == 0
                                        && normal[0] as i128 * n[2] as i128
                                            == normal[2] as i128 * n[0] as i128
                                        && normal[0] as i128 * n[0] as i128
                                            + normal[2] as i128 * n[2] as i128
                                            > 0
                                }
                            });
                            if let Some(old_face) = old_face {
                                face.material = old_face.material;
                                face.uv = old_face.uv;
                            }
                        }
                    }
                }
            }
        }
        let sky_material = self.project.resources.iter().find_map(|r| match &r.data {
            ResourceData::Material(m) if m.sky_aperture => Some(r.id),
            _ => None,
        });
        let sky_brushes = if draft.enclose_sky && draft.group.is_none() {
            let material = sky_material
                .ok_or("Add a sky-aperture material in Material Lab before enclosing terrain.")?;
            Some(draft.terrain.sky_enclosure(draft.sky_ceiling, material)?)
        } else {
            None
        };
        self.push_undo();
        let group = if let Some(group) = draft.group {
            self.project
                .active_scene_mut()
                .brushes
                .retain(|b| b.group != Some(group));
            self.project
                .active_scene_mut()
                .node_mut(group)
                .unwrap()
                .name = draft.name.clone();
            group
        } else {
            let parent = self
                .open_group
                .filter(|g| self.node_is_group(*g))
                .unwrap_or(self.project.active_scene().root);
            self.project
                .active_scene_mut()
                .add_node(parent, draft.name.clone(), NodeKind::Group)
        };
        for brush in &mut brushes {
            brush.group = Some(group);
        }
        let count = brushes.len();
        self.project.active_scene_mut().brushes.extend(brushes);
        if let Some(mut sky) = sky_brushes {
            let scene = self.project.active_scene_mut();
            let enclosure =
                scene.add_node(group, "Sky enclosure (edit as brushes)", NodeKind::Group);
            for b in &mut sky {
                b.group = Some(enclosure);
            }
            scene.brushes.extend(sky);
        }
        self.clear_brush_selection();
        self.replace_node_selection(group);
        self.mark_dirty();
        self.status=format!("Terrain applied: {count} solid brushes. Use Terrain → Edit selected to keep sculpting.");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn workspace() -> EditorWorkspace {
        EditorWorkspace::with_project(
            PathBuf::from("/tmp/terrain-tests"),
            ProjectDocument::starter(),
        )
    }
    #[test]
    fn terrain_create_sculpt_reopen_and_document_undo() {
        let mut ws = workspace();
        let before = ws.project.clone();
        ws.open_terrain_editor(false);
        let mut draft = ws.terrain_editor.take().unwrap();
        draft
            .terrain
            .sculpt([3.0, 4.0], 2.0, 256.0, SculptMode::Raise, 0.0);
        ws.commit_terrain(&draft).unwrap();
        let created = ws.project.clone();
        let group = ws.selection.selected_node;
        assert_eq!(
            created
                .active_scene()
                .brushes
                .iter()
                .filter(|b| b.group == Some(group))
                .count(),
            128
        );
        ws.open_terrain_editor(true);
        let mut draft = ws.terrain_editor.take().unwrap();
        let quantized = draft.terrain.clone();
        draft.checkpoint();
        draft
            .terrain
            .sculpt([3.0, 4.0], 2.0, 256.0, SculptMode::Raise, 0.0);
        draft.undo();
        assert_eq!(draft.terrain, quantized);
        draft.redo();
        assert_ne!(draft.terrain, quantized);
        ws.commit_terrain(&draft).unwrap();
        assert_ne!(ws.project, created);
        ws.do_undo();
        assert_eq!(ws.project, created);
        ws.do_undo();
        assert_eq!(ws.project, before);
        ws.do_redo();
        assert_eq!(ws.project, created);
        // RON persistence carries all editable terrain geometry.
        let ron = created.to_ron_string().unwrap();
        let loaded = ProjectDocument::from_ron_str(&ron).unwrap();
        let brushes: Vec<_> = loaded
            .active_scene()
            .brushes
            .iter()
            .filter(|b| b.group == Some(group))
            .cloned()
            .collect();
        assert_eq!(Terrain::from_brushes(&brushes).unwrap(), quantized);
    }
    #[test]
    fn sky_enclosure_is_grouped_reopenable_and_undoable() {
        let mut ws = workspace();
        let mut m = psxed_project::MaterialResource::opaque(None);
        m.sky_aperture = true;
        ws.project
            .add_resource("Terrain sky", ResourceData::Material(m));
        let before = ws.project.clone();
        ws.open_terrain_editor(false);
        let mut draft = ws.terrain_editor.take().unwrap();
        draft.enclose_sky = true;
        ws.commit_terrain(&draft).unwrap();
        let terrain = ws.selection.selected_node;
        let scene = ws.project.active_scene();
        let sky = scene
            .nodes()
            .iter()
            .find(|n| n.parent == Some(terrain))
            .unwrap()
            .id;
        assert_eq!(
            scene
                .brushes
                .iter()
                .filter(|b| b.group == Some(sky))
                .count(),
            5
        );
        ws.open_terrain_editor(true);
        assert!(ws.terrain_editor.is_some());
        ws.terrain_editor = None;
        ws.do_undo();
        assert_eq!(ws.project, before);
    }

    #[test]
    fn stale_draft_does_not_overwrite_changed_geometry() {
        let mut ws = workspace();
        ws.open_terrain_editor(false);
        let draft = ws.terrain_editor.take().unwrap();
        ws.commit_terrain(&draft).unwrap();
        ws.open_terrain_editor(true);
        let draft = ws.terrain_editor.take().unwrap();
        ws.project
            .active_scene_mut()
            .brushes
            .last_mut()
            .unwrap()
            .translate([16, 0, 0]);
        let changed = ws.project.clone();
        assert!(ws.commit_terrain(&draft).is_err());
        assert_eq!(ws.project, changed);
    }
    #[test]
    fn sculpt_preserves_painted_faces() {
        let mut ws = workspace();
        ws.open_terrain_editor(false);
        let draft = ws.terrain_editor.take().unwrap();
        ws.commit_terrain(&draft).unwrap();
        let group = ws.selection.selected_node;
        let index = ws
            .project
            .active_scene()
            .brushes
            .iter()
            .position(|b| b.group == Some(group))
            .unwrap();
        ws.project.active_scene_mut().brushes[index].faces[0]
            .uv
            .offset_texels = [21, 45];
        ws.open_terrain_editor(true);
        let mut draft = ws.terrain_editor.take().unwrap();
        draft
            .terrain
            .sculpt([0.0, 0.0], 2.0, 96.0, SculptMode::Raise, 0.0);
        ws.commit_terrain(&draft).unwrap();
        let brush = ws
            .project
            .active_scene()
            .brushes
            .iter()
            .find(|b| b.group == Some(group))
            .unwrap();
        assert_eq!(brush.faces[0].uv.offset_texels, [21, 45]);
    }
}
