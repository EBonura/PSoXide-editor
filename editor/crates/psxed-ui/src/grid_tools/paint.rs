//! The Paint tool's brush: the material and the turn and flip it gives a
//! face.
//!
//! One function ([`PaintGhost::apply`]) writes a paint onto a world. The tool
//! calls it on the document when the author clicks; the previews call it on a
//! copy, so what the preview shows is what the click writes.

use super::*;

use psxed_project::face_uv::ZOOM_STEPS_Q8;
use psxed_project::grid::edit::bands::{edit_bands, is_bandable, paint_whole_wall, BandScope};
use psxed_project::grid::edit::{
    edit_plane_uv, edit_uv, set_material, set_plane_material, FaceRef, FaceTarget, SurfaceKind,
    UvOp,
};
use psxed_project::grid::faces::{cell_faces_with, GridFace, GridFaceKind};
use psxed_project::grid::uv::{ProjectUvDefaults, TILE_TEXELS};
use psxed_project::grid::GridWorld;
use std::cell::RefCell;

/// The turn, flip and size the brush gives a face: a quarter-turn count
/// (clockwise), a mirror on each axis and a zoom step, in the terms of the
/// Inspector's Tiling section. The identity leaves a face's own turn, flip
/// and scale alone, so painting with a plain brush never undoes a placement
/// the author made.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PaintOrient {
    /// Quarter turns clockwise, `0..=3`.
    pub turns: u8,
    /// Mirror horizontally / vertically.
    pub flip: [bool; 2],
    /// Steps from 1x along 1/4, 1/2, 1, 2, 4 (`-2..=2`): how big the paint
    /// is laid, in the terms of the face's scale. 0 is 1x, which leaves a
    /// face's own scale alone.
    pub zoom: i8,
}

/// Most steps the brush zooms either way (1/4x to 4x).
const MAX_ZOOM_STEP: i8 = 2;

/// Widest the paint material's name gets in the options bar, so the scale
/// buttons stay in view at 1456 wide.
const NAME_WIDTH: f32 = 76.0;

/// What a scale preset means, for the tooltips.
pub(super) const ZOOM_HINT: &str = "1x is the tiling as it stands (a new world: one 64x64 texture per sector); 2x draws the texture twice as big, 0.5x half as big. It sets the Scale fields of the Tiling.";

impl PaintOrient {
    /// Whether the brush turns, flips and zooms nothing.
    pub fn is_identity(self) -> bool {
        self == Self::default()
    }

    /// Whether the brush turns and flips nothing (its zoom aside).
    pub fn orientation_is_identity(self) -> bool {
        self.turns & 3 == 0 && self.flip == [false; 2]
    }

    /// The brush's scale, Q8 (256 is 1x, 512 twice as big).
    pub fn zoom_q8(self) -> i16 {
        let step = self.zoom.clamp(-MAX_ZOOM_STEP, MAX_ZOOM_STEP);
        ZOOM_STEPS_Q8[(step + MAX_ZOOM_STEP) as usize]
    }

    /// The brush at zoom `step` (clamped to the presets).
    pub fn at_zoom(self, step: i8) -> Self {
        Self {
            zoom: step.clamp(-MAX_ZOOM_STEP, MAX_ZOOM_STEP),
            ..self
        }
    }

    /// The step of a preset scale, if `q8` is one.
    pub fn step_of(q8: i16) -> Option<i8> {
        ZOOM_STEPS_Q8
            .iter()
            .position(|&s| s == q8)
            .map(|i| i as i8 - MAX_ZOOM_STEP)
    }

    /// The label of a zoom step: `0.25x`, `0.5x`, `1x`, `2x`, `4x`.
    pub fn zoom_label(step: i8) -> &'static str {
        match step {
            ..=-2 => "0.25x",
            -1 => "0.5x",
            0 => "1x",
            1 => "2x",
            _ => "4x",
        }
    }

    /// A quarter turn on, clockwise or not.
    pub fn turned(self, clockwise: bool) -> Self {
        Self {
            turns: if clockwise {
                (self.turns + 1) & 3
            } else {
                (self.turns + 3) & 3
            },
            ..self
        }
    }

    /// Mirrored on `axis` (0 horizontal, 1 vertical).
    pub fn flipped(mut self, axis: usize) -> Self {
        if let Some(flip) = self.flip.get_mut(axis) {
            *flip = !*flip;
        }
        self
    }

    /// The turn in degrees, clockwise.
    pub fn degrees(self) -> u32 {
        u32::from(self.turns & 3) * 90
    }

    /// The edits that store this on a face's tiling: the turn and flip when
    /// the brush has any, the scale when it is not 1x. A brush with neither
    /// writes nothing.
    pub fn ops(self) -> Vec<UvOp> {
        let mut ops = Vec::new();
        if !self.orientation_is_identity() {
            ops.push(UvOp::SetOrientation {
                turns: self.turns,
                flip: self.flip,
            });
        }
        if self.zoom != 0 {
            ops.push(UvOp::SetZoom(self.zoom_q8()));
        }
        ops
    }
}

/// What a paint lands on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaintTarget {
    /// Faces of the grid (all of one triangle's worth when `tri` names it).
    Faces {
        /// The faces under the pointer.
        faces: Vec<FaceRef>,
        /// The triangle of a split floor or ceiling, when one is named.
        tri: Option<u8>,
    },
    /// A free plane.
    Plane(psxed_project::grid::edit::PlaneId),
    /// One band of one wall segment: the rest of the wall's run keeps its
    /// own paint.
    Band {
        /// The wall.
        face: FaceRef,
        /// Index of the band in the wall's set.
        band: usize,
    },
}

/// A paint about to land: where, with what material, turned and flipped how.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaintGhost {
    /// What it lands on.
    pub target: PaintTarget,
    /// The material.
    pub material: psxed_project::ResourceId,
    /// The turn and flip.
    pub orient: PaintOrient,
}

impl PaintGhost {
    /// The room the paint lands in.
    pub fn room(&self) -> Option<usize> {
        match &self.target {
            PaintTarget::Faces { faces, .. } => faces.first().map(|f| f.room),
            PaintTarget::Plane(id) => Some(id.room),
            PaintTarget::Band { face, .. } => Some(face.room),
        }
    }

    /// Write the paint onto `world`. Returns whether anything changed.
    pub fn apply(&self, world: &mut GridWorld) -> bool {
        self.apply_counting(world).0
    }

    /// [`Self::apply`], also saying how many bands of a painted wall took the
    /// paint because they kept a material of their own.
    pub fn apply_counting(&self, world: &mut GridWorld) -> (bool, usize) {
        let material = Some(self.material);
        let orient = self.orient;
        match &self.target {
            PaintTarget::Faces { faces, tri } => {
                let mut changed = false;
                let mut bands = 0;
                for face in faces {
                    // A whole wall takes its bands with it: a band with a
                    // material of its own would otherwise keep the old look
                    // under a paint meant for the wall.
                    if tri.is_none() && is_bandable(face) {
                        let done = paint_whole_wall(world, *face, material);
                        changed |= done.changed();
                        bands += done.bands;
                    } else {
                        changed |= set_material(world, *face, *tri, material);
                    }
                    for op in orient.ops() {
                        changed |= edit_uv(world, *face, *tri, op);
                    }
                }
                (changed, bands)
            }
            PaintTarget::Plane(id) => {
                let mut changed = set_plane_material(world, *id, material);
                for op in orient.ops() {
                    changed |= edit_plane_uv(world, *id, op);
                }
                (changed, 0)
            }
            PaintTarget::Band { face, band } => (
                edit_bands(world, *face, BandScope::Piece, |set| {
                    let mut changed = set.set_band_material(*band, material);
                    if let Some(band) = set.bands.get_mut(*band) {
                        let before = band.uv;
                        for op in orient.ops() {
                            op.apply(&mut band.uv);
                        }
                        changed |= band.uv != before;
                    }
                    changed
                }),
                0,
            ),
        }
    }
}

/// What the Paint tool tracks besides the document.
#[derive(Default)]
pub(crate) struct PaintState {
    /// The turn and flip of the brush.
    pub orient: PaintOrient,
    /// Ctrl is held over the pointer: a click samples, so nothing would be
    /// painted.
    pub sampling: bool,
    /// The band of a banded wall under the pointer, when the hover is on one
    /// (a band is painted, not the wall).
    pub band: Option<(FaceRef, usize)>,
    /// The faces the preview shows in the plan: the painted copy of the
    /// target's faces, kept for the preview and document revision they were
    /// made for.
    pub(super) faces: RefCell<Option<(PaintGhost, u64, Vec<GridFace>)>>,
    /// The paint material's picture with a repeating wrap, for the plan.
    pub(super) texture: RefCell<Option<(ResourceId, String, egui::TextureHandle)>>,
}

impl EditorWorkspace {
    /// The paint the pointer is about to make, for the previews: none unless
    /// the Paint tool is idle over a face with a material to paint.
    pub(crate) fn paint_ghost(&self) -> Option<PaintGhost> {
        if self.grid.tool != GridTool::Paint
            || !matches!(self.grid.gesture, Gesture::Idle)
            || self.grid.paint.sampling
            || !self.is_grid_world()
        {
            return None;
        }
        let material = self.grid.material?;
        let faces: Vec<FaceRef> = match &self.grid.hover {
            GridHover::Plane(id) => {
                return Some(PaintGhost {
                    target: PaintTarget::Plane(*id),
                    material,
                    orient: self.grid.paint.orient,
                });
            }
            GridHover::Face { face, .. } => vec![*face],
            GridHover::Elements(list) => list
                .iter()
                .filter_map(|e| match e {
                    psxed_project::grid::edit::GridElement::Face(f) => Some(*f),
                    _ => None,
                })
                .collect(),
            _ => return None,
        };
        if faces.is_empty() {
            return None;
        }
        let target = match self.grid.paint.band {
            Some((wall, band)) if faces.contains(&wall) => PaintTarget::Band { face: wall, band },
            _ => PaintTarget::Faces {
                tri: match &self.grid.hover {
                    GridHover::Face { tri, .. } => *tri,
                    _ => None,
                },
                faces,
            },
        };
        Some(PaintGhost {
            target,
            material,
            orient: self.grid.paint.orient,
        })
    }

    /// Turn the brush a quarter, clockwise or not.
    pub(crate) fn paint_turn(&mut self, clockwise: bool) {
        self.grid.paint.orient = self.grid.paint.orient.turned(clockwise);
        self.say_paint_orient();
    }

    /// Mirror the brush on `axis` (0 horizontal, 1 vertical).
    pub(crate) fn paint_flip(&mut self, axis: usize) {
        self.grid.paint.orient = self.grid.paint.orient.flipped(axis);
        self.say_paint_orient();
    }

    /// Zoom the brush `delta` steps along 1/4x, 1/2x, 1x, 2x, 4x (positive
    /// bigger), stopping at the ends.
    pub(crate) fn paint_zoom(&mut self, delta: i8) {
        let orient = self.grid.paint.orient;
        self.set_paint_zoom(orient.zoom.saturating_add(delta));
    }

    /// Set the brush to zoom step `step` (0 is 1x).
    pub(crate) fn set_paint_zoom(&mut self, step: i8) {
        let zoomed = self.grid.paint.orient.at_zoom(step);
        self.grid.paint.orient = zoomed;
        self.status = format!("Paint scale {}", PaintOrient::zoom_label(zoomed.zoom));
    }

    fn say_paint_orient(&mut self) {
        let orient = self.grid.paint.orient;
        let flips = match orient.flip {
            [true, true] => ", flipped both ways",
            [true, false] => ", flipped horizontally",
            [false, true] => ", flipped vertically",
            [false, false] => "",
        };
        self.status = format!("Paint turned {} deg{flips}", orient.degrees());
    }
}

/// A square of `texture` over `rect`, turned and flipped the way a face
/// with `orient` shows it (one tile, so the corners carry the texture corners
/// the real mapping gives a one-tile face).
pub(crate) fn oriented_quad(
    texture: egui::TextureId,
    rect: Rect,
    orient: PaintOrient,
) -> egui::Shape {
    let mut mesh = egui::epaint::Mesh::with_texture(texture);
    for (a, b, pos) in [
        (0.0, 0.0, rect.left_top()),
        (1.0, 0.0, rect.right_top()),
        (1.0, 1.0, rect.right_bottom()),
        (0.0, 1.0, rect.left_bottom()),
    ] {
        let [u, v] = psxed_project::grid::uv::orient_unit(orient.turns, orient.flip, a, b);
        mesh.vertices.push(egui::epaint::Vertex {
            pos,
            uv: Pos2::new(u as f32, v as f32),
            color: Color32::WHITE,
        });
    }
    mesh.indices = vec![0, 1, 2, 0, 2, 3];
    egui::Shape::mesh(mesh)
}

/// A divider with little air round it, for the options bar's one row.
fn tight_separator(ui: &mut egui::Ui) {
    ui.add(egui::Separator::default().spacing(1.0));
}

impl EditorWorkspace {
    /// The Paint tool's options: the paint material as the brush will lay
    /// it, the turn in degrees and a button (with its key) for each of the
    /// four actions.
    pub(crate) fn draw_paint_options(&mut self, ui: &mut egui::Ui) {
        let material = self.grid.material;
        let name = material
            .and_then(|id| self.project.resource_name(id))
            .unwrap_or("none")
            .to_string();
        let texture = material
            .and_then(|id| self.texture_thumbs.get(&id))
            .map(|entry| entry.handle.id());
        let orient = self.grid.paint.orient;
        // The bar is one row of fixed width and the options are many: tight
        // spacing keeps the scale buttons in view at 1456 wide.
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.spacing_mut().button_padding = Vec2::new(5.0, 3.0);
        // The swatch: the picture the brush lays down.
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::hover());
        match texture {
            Some(texture) => {
                ui.painter().add(oriented_quad(texture, rect, orient));
            }
            None => {
                ui.painter()
                    .rect_filled(rect, 2.0, Color32::from_rgb(52, 56, 66));
            }
        }
        ui.painter().rect_stroke(
            rect,
            2.0,
            Stroke::new(1.0, STUDIO_BORDER),
            StrokeKind::Inside,
        );
        response.on_hover_text("The paint, turned and flipped as it will be laid");
        // A long material name is cut short: the bar has one row.
        ui.scope(|ui| {
            ui.set_max_width(NAME_WIDTH);
            ui.add(
                egui::Label::new(RichText::new(&name).color(STUDIO_TEXT_WEAK).size(11.0))
                    .truncate(),
            )
            .on_hover_text(format!("The paint material: {name}"));
        });
        tight_separator(ui);
        if ui
            .button("Turn left (Shift+R)")
            .on_hover_text("Turn the paint a quarter counter-clockwise")
            .clicked()
        {
            self.paint_turn(false);
        }
        if ui
            .button("Turn right (R)")
            .on_hover_text("Turn the paint a quarter clockwise")
            .clicked()
        {
            self.paint_turn(true);
        }
        ui.label(RichText::new(format!("{} deg", orient.degrees())).size(11.0));
        tight_separator(ui);
        for (axis, label, hint) in [
            (0usize, "Flip H (F)", "Mirror the paint left to right"),
            (1usize, "Flip V (Shift+F)", "Mirror the paint top to bottom"),
        ] {
            if ui
                .selectable_label(orient.flip[axis], label)
                .on_hover_text(hint)
                .clicked()
            {
                self.paint_flip(axis);
            }
        }
        tight_separator(ui);
        for step in -MAX_ZOOM_STEP..=MAX_ZOOM_STEP {
            let label = PaintOrient::zoom_label(step);
            if ui
                .selectable_label(orient.zoom == step, label)
                .on_hover_text(format!(
                    "Lay the paint at {label} ([ and ] step it). {ZOOM_HINT}"
                ))
                .clicked()
            {
                self.set_paint_zoom(step);
            }
        }
    }
}

impl EditorWorkspace {
    /// The picture of the paint material with a repeating wrap, for faces
    /// that show it more than once.
    fn paint_repeat_texture(
        &self,
        ctx: &egui::Context,
        material: ResourceId,
    ) -> Option<egui::TextureId> {
        let entry = self.texture_thumbs.get(&material)?;
        let mut slot = self.grid.paint.texture.borrow_mut();
        let stale = !matches!(
            &*slot,
            Some((id, signature, _)) if *id == material && *signature == entry.signature
        );
        if stale {
            let options = egui::TextureOptions {
                magnification: egui::TextureFilter::Nearest,
                minification: egui::TextureFilter::Nearest,
                wrap_mode: egui::TextureWrapMode::Repeat,
                mipmap_mode: None,
            };
            let handle = ctx.load_texture("paint-preview", entry.image.clone(), options);
            *slot = Some((material, entry.signature.clone(), handle));
        }
        slot.as_ref().map(|(_, _, handle)| handle.id())
    }

    /// The floor and ceiling faces `ghost` would paint, as they will be once
    /// painted (a copy of the world takes the paint, through the same
    /// function the click uses).
    fn paint_ghost_faces(&self, world: &GridWorld, ghost: &PaintGhost) -> Vec<GridFace> {
        let mut cache = self.grid.paint.faces.borrow_mut();
        let fresh = matches!(&*cache, Some((g, rev, _)) if g == ghost && *rev == self.grid.rev);
        if !fresh {
            let mut painted = world.clone();
            ghost.apply(&mut painted);
            let defaults = ProjectUvDefaults(&self.project);
            let mut faces = Vec::new();
            if let PaintTarget::Faces {
                faces: targets,
                tri,
            } = &ghost.target
            {
                for target in targets {
                    let FaceTarget::Surface { kind, span } = target.target else {
                        continue;
                    };
                    let Some(room) = painted.rooms.get(target.room) else {
                        continue;
                    };
                    let Some([x, z]) = room.local(target.cell()) else {
                        continue;
                    };
                    let mut cell = Vec::new();
                    cell_faces_with(&painted, &defaults, target.room, x, z, &mut cell);
                    faces.extend(cell.into_iter().filter(|face| {
                        let (is_kind, triangle) = match face.kind {
                            GridFaceKind::Floor { triangle } => {
                                (kind == SurfaceKind::Floor, triangle)
                            }
                            GridFaceKind::Ceiling { triangle } => {
                                (kind == SurfaceKind::Ceiling, triangle)
                            }
                            _ => (false, None),
                        };
                        is_kind
                            && face.leaf == target.leaf
                            && face.span == span
                            && tri.is_none_or(|t| triangle.is_none_or(|own| own == t))
                    }));
                }
            }
            *cache = Some((ghost.clone(), self.grid.rev, faces));
        }
        cache.as_ref().map_or_else(Vec::new, |(_, _, f)| f.clone())
    }

    /// The paint preview over the plan: the floor or ceiling under the
    /// pointer drawn with the paint material at the tiling, turn and flip it
    /// will get. A wall, band or plane is a line (or a slim shape) in the
    /// plan, so those show the paint as a swatch beside the pointer.
    pub(crate) fn draw_paint_ghost_2d(
        &self,
        painter: &egui::Painter,
        transform: ViewportTransform,
    ) {
        let Some(ghost) = self.paint_ghost() else {
            return;
        };
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let Some(texture) = self.paint_repeat_texture(painter.ctx(), ghost.material) else {
            return;
        };
        let faces = self.paint_ghost_faces(world, &ghost);
        for face in &faces {
            let mut mesh = egui::epaint::Mesh::with_texture(texture);
            for (point, texel) in face.points.iter().zip(&face.texels) {
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: transform.world_to_screen([point[0] as f32, point[2] as f32]),
                    uv: Pos2::new(
                        (texel[0] / TILE_TEXELS) as f32,
                        (texel[1] / TILE_TEXELS) as f32,
                    ),
                    color: Color32::WHITE,
                });
            }
            for i in 1..mesh.vertices.len().saturating_sub(1) {
                mesh.indices.extend([0, i as u32, i as u32 + 1]);
            }
            painter.add(egui::Shape::mesh(mesh));
        }
        if faces.is_empty() {
            // Nothing with an area in the plan: the swatch beside the pointer.
            if let Some(pointer) = painter.ctx().input(|i| i.pointer.hover_pos()) {
                let chip = Rect::from_min_size(pointer + Vec2::new(14.0, -42.0), Vec2::splat(28.0));
                let thumb = self
                    .texture_thumbs
                    .get(&ghost.material)
                    .map(|entry| entry.handle.id());
                if let Some(thumb) = thumb {
                    painter.add(oriented_quad(thumb, chip, ghost.orient));
                    painter.rect_stroke(
                        chip,
                        2.0,
                        Stroke::new(1.5, STUDIO_ACCENT),
                        StrokeKind::Outside,
                    );
                }
            }
        }
    }
}
