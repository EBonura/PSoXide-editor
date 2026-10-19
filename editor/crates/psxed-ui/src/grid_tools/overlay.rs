//! Drawing the grid tools over the views: selection and hover highlights,
//! ghost previews of what a gesture will create, the marquee.
//!
//! The 3D view is the emulator's picture in a texture, so these are egui
//! shapes on top of it. They are clipped to the camera's near plane and
//! drawn without a depth test, which is why faces that point away from the
//! camera are left out of the highlights.

use super::pick::Cam;
use super::view::{editor_xz, editor_y};
use super::*;

use psxed_project::grid::edit::{CellRect, CornerRef, EdgeRef, FaceRef, FaceTarget, GridElement};
use psxed_project::grid::{GridWorld, SECTOR_UNITS};
use std::collections::BTreeSet;

/// Draws a polygon given as editor-space points: colour, fill alpha, line width.
pub(super) type PolygonFn<'a> = dyn Fn(&[[f32; 3]], Color32, u8, f32) + 'a;

pub(super) const SELECT: Color32 = Color32::from_rgb(255, 214, 64);
pub(super) const HOVER: Color32 = Color32::from_rgb(110, 220, 255);
pub(super) const CREATE: Color32 = Color32::from_rgb(96, 232, 132);
pub(super) const ERASE: Color32 = Color32::from_rgb(244, 96, 84);
pub(super) const GHOST: Color32 = Color32::from_rgb(240, 240, 250);

pub(super) fn fill(c: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
}

/// Editor-space position of a corner.
fn corner_pos(world: &GridWorld, corner: &CornerRef) -> Option<[f32; 3]> {
    let g = corner.grid_pos(world)?;
    Some([
        world.scale.to_editor(g[0]) as f32,
        world.scale.to_editor(g[1]) as f32,
        world.scale.to_editor(g[2]) as f32,
    ])
}

fn edge_ends(world: &GridWorld, edge: &EdgeRef) -> Option<([f32; 3], [f32; 3])> {
    let [a, b] = edge.corners();
    Some((corner_pos(world, &a)?, corner_pos(world, &b)?))
}

/// Faces an element stands for.
fn element_faces(world: &GridWorld, element: &GridElement) -> Vec<FaceRef> {
    match element {
        GridElement::Cell { room, cell } => {
            psxed_project::grid::edit::faces_of_cell(world, *room, *cell)
        }
        GridElement::Face(f) => vec![*f],
        // Drawn as the triangle, not as the whole face.
        GridElement::Triangle { .. } | GridElement::Edge(_) | GridElement::Vertex(_) => Vec::new(),
    }
}

/// The three corners (editor units) of triangle `tri` of a floor or ceiling.
fn tri_corners(world: &GridWorld, face: FaceRef, tri: u8) -> Option<[[f32; 3]; 3]> {
    let surface = face.surface(world)?;
    let room = world.rooms.get(face.room)?;
    let heights = surface.heights_for(usize::from(tri));
    let offsets = [[0, 0], [1, 0], [1, 1], [0, 1]];
    let ids = surface.split.triangle(usize::from(tri));
    let ([x0, z0], size) = super::view::leaf_square(world.scale, face.leaf);
    Some(ids.map(|i| {
        let o = offsets[i];
        [
            x0 + size * o[0] as f32,
            editor_y(world, room.y_offset, heights[i]),
            z0 + size * o[1] as f32,
        ]
    }))
}

/// Plan-view outline of a cell in editor units: `[nw, ne, se, sw]`.
/// Editor-space corners of a cell of `unit` grid units across.
fn cell_corners_xz(world: &GridWorld, unit: i32, cell: [i32; 2]) -> [[f32; 2]; 4] {
    let s = world.scale;
    let (x0, z0) = (cell[0] * unit, cell[1] * unit);
    [
        editor_xz(s, x0, z0),
        editor_xz(s, x0 + unit, z0),
        editor_xz(s, x0 + unit, z0 + unit),
        editor_xz(s, x0, z0 + unit),
    ]
}

/// Cells a ghost draws one by one; bigger rectangles show their outline only.
const GHOST_CELL_LIMIT: usize = 2048;

/// Colour of a rectangle ghost and a warning when it cannot be made: a
/// create that would take the room past the size cap shows red.
fn rect_ghost(world: &GridWorld, g: &RectGesture, rect: CellRect) -> (Color32, Option<String>) {
    if g.erase {
        return (ERASE, None);
    }
    if let Some(room) = world.rooms.get(g.room) {
        let side = psxed_project::grid::limits::MAX_ROOM_SIDE as i32;
        let low = [
            room.origin[0].min(rect.min[0]),
            room.origin[1].min(rect.min[1]),
        ];
        let high = [
            (room.origin[0] + room.width as i32 - 1).max(rect.max[0]),
            (room.origin[1] + room.depth as i32 - 1).max(rect.max[1]),
        ];
        let (w, d) = (high[0] - low[0] + 1, high[1] - low[1] + 1);
        if w > side || d > side {
            return (
                ERASE,
                Some(format!(
                    "room would be {} sectors: the cap is {side}",
                    w.max(d)
                )),
            );
        }
    }
    (CREATE, None)
}

/// A short label on a dark chip so it reads over any picture.
pub(super) fn label_chip(painter: &egui::Painter, at: Pos2, align: Align2, text: &str) {
    let galley =
        painter.layout_no_wrap(text.to_string(), FontId::proportional(13.0), Color32::WHITE);
    let rect = align
        .anchor_size(at, galley.size())
        .expand2(Vec2::new(6.0, 3.0));
    painter.rect_filled(rect, 3.0, Color32::from_rgba_unmultiplied(12, 16, 22, 215));
    painter.galley(rect.min + Vec2::new(6.0, 3.0), galley, Color32::WHITE);
}

impl EditorWorkspace {
    fn erase_held(painter: &egui::Painter) -> bool {
        painter
            .ctx()
            .input(|i| i.modifiers.command || i.modifiers.ctrl)
    }

    // ---- Top view ----

    /// Highlights and ghosts over the Top view.
    pub(crate) fn draw_grid_overlay_2d(
        &self,
        painter: &egui::Painter,
        transform: ViewportTransform,
    ) {
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let screen = |p: [f32; 2]| transform.world_to_screen(p);
        // Selected and pasted cells are sectors; hovers and ghosts use the
        // cell size the tools use now, and a gesture carries its own level.
        let unit_now = super::cell_size::cell_units(self.grid.cell_size.level());
        let cell_rect_at = |unit: i32, cell: [i32; 2]| {
            let c = cell_corners_xz(world, unit, cell);
            Rect::from_two_pos(screen(c[0]), screen(c[2]))
        };
        let cell_rect = |cell: [i32; 2]| cell_rect_at(SECTOR_UNITS, cell);
        let leaf_rect = |key: psxed_project::grid::LeafKey| {
            let ([x0, z0], size) = super::view::leaf_square(world.scale, key);
            Rect::from_two_pos(screen([x0, z0]), screen([x0 + size, z0 + size]))
        };
        let line = |a: [f32; 2], b: [f32; 2], color: Color32, width: f32| {
            painter.line_segment([screen(a), screen(b)], Stroke::new(width, color));
        };
        let wall_line =
            |room: usize, cell: [i32; 2], edge: WallEdge, color: Color32, width: f32| {
                let _ = room;
                let (a, b) = edge.endpoints();
                let at = |p: [i32; 2]| {
                    editor_xz(
                        world.scale,
                        (cell[0] + p[0]) * unit_now,
                        (cell[1] + p[1]) * unit_now,
                    )
                };
                line(at(a), at(b), color, width);
            };
        let leaf_wall_line =
            |key: psxed_project::grid::LeafKey, edge: WallEdge, color: Color32, width: f32| {
                let ([x0, z0], size) = super::view::leaf_square(world.scale, key);
                let (a, b) = edge.endpoints();
                let at = |p: [i32; 2]| [x0 + size * p[0] as f32, z0 + size * p[1] as f32];
                line(at(a), at(b), color, width);
            };
        // Selection.
        for element in self.grid.selection.items() {
            match element {
                GridElement::Cell { cell, .. } => {
                    let r = cell_rect(*cell);
                    painter.rect_filled(r, 0.0, fill(SELECT, 70));
                    painter.rect_stroke(r, 0.0, Stroke::new(2.0, SELECT), StrokeKind::Inside);
                }
                GridElement::Face(f) => match f.target {
                    FaceTarget::Surface { .. } => {
                        let r = leaf_rect(f.leaf);
                        painter.rect_filled(r, 0.0, fill(SELECT, 70));
                        painter.rect_stroke(r, 0.0, Stroke::new(2.0, SELECT), StrokeKind::Inside);
                    }
                    FaceTarget::Wall { edge, .. } | FaceTarget::Derived { edge, .. } => {
                        leaf_wall_line(f.leaf, edge, SELECT, 4.0)
                    }
                },
                GridElement::Triangle { face, tri } => {
                    if let Some(points) = tri_corners(world, *face, *tri) {
                        let points: Vec<Pos2> =
                            points.iter().map(|p| screen([p[0], p[2]])).collect();
                        painter.add(egui::Shape::convex_polygon(
                            points,
                            fill(SELECT, 110),
                            Stroke::new(2.5, SELECT),
                        ));
                    }
                }
                GridElement::Edge(e) => {
                    if let Some((a, b)) = edge_ends(world, e) {
                        line([a[0], a[2]], [b[0], b[2]], SELECT, 4.0);
                    }
                }
                GridElement::Vertex(v) => {
                    if let Some(p) = corner_pos(world, v) {
                        painter.circle_filled(screen([p[0], p[2]]), 5.0, SELECT);
                    }
                }
            }
        }
        // Hover and ghosts.
        let erase = Self::erase_held(painter);
        match &self.grid.hover {
            GridHover::Leaf { room, key } => {
                let color = if painter.ctx().input(|i| i.modifiers.alt) {
                    ERASE
                } else {
                    CREATE
                };
                let _ = room;
                let rect = leaf_rect(*key);
                painter.rect_filled(rect, 0.0, fill(color, 60));
                painter.rect_stroke(rect, 0.0, Stroke::new(2.0, color), StrokeKind::Inside);
            }
            GridHover::Cell { cell, .. } => {
                let color = if erase { ERASE } else { CREATE };
                let r = cell_rect_at(unit_now, *cell);
                painter.rect_filled(r, 0.0, fill(color, 60));
                painter.rect_stroke(r, 0.0, Stroke::new(2.0, color), StrokeKind::Inside);
            }
            GridHover::Wall {
                cell,
                edge,
                heights,
                room,
            } => {
                let color = match (erase, heights) {
                    (true, _) => ERASE,
                    (false, Some(_)) => CREATE,
                    (false, None) => Color32::from_rgb(150, 150, 160),
                };
                wall_line(*room, *cell, *edge, color, 4.0);
            }
            GridHover::Elements(list) => {
                for element in list {
                    match element {
                        GridElement::Cell { cell, .. } => {
                            let r = cell_rect(*cell);
                            painter.rect_stroke(
                                r,
                                0.0,
                                Stroke::new(2.0, HOVER),
                                StrokeKind::Inside,
                            );
                        }
                        GridElement::Face(f) => match f.target {
                            FaceTarget::Surface { .. } => {
                                painter.rect_stroke(
                                    leaf_rect(f.leaf),
                                    0.0,
                                    Stroke::new(2.0, HOVER),
                                    StrokeKind::Inside,
                                );
                            }
                            FaceTarget::Wall { edge, .. } | FaceTarget::Derived { edge, .. } => {
                                leaf_wall_line(f.leaf, edge, HOVER, 4.0)
                            }
                        },
                        GridElement::Triangle { face, tri } => {
                            if let Some(points) = tri_corners(world, *face, *tri) {
                                painter.add(egui::Shape::convex_polygon(
                                    points.iter().map(|p| screen([p[0], p[2]])).collect(),
                                    fill(HOVER, 70),
                                    Stroke::new(2.0, HOVER),
                                ));
                            }
                        }
                        GridElement::Edge(e) => {
                            if let Some((a, b)) = edge_ends(world, e) {
                                line([a[0], a[2]], [b[0], b[2]], HOVER, 4.0);
                            }
                        }
                        GridElement::Vertex(v) => {
                            if let Some(p) = corner_pos(world, v) {
                                painter.circle_stroke(
                                    screen([p[0], p[2]]),
                                    6.0,
                                    Stroke::new(2.0, HOVER),
                                );
                            }
                        }
                    }
                }
            }
            GridHover::Face { face, tri } => {
                if let Some(points) = tri.and_then(|t| tri_corners(world, *face, t)) {
                    painter.add(egui::Shape::convex_polygon(
                        points.iter().map(|p| screen([p[0], p[2]])).collect(),
                        fill(HOVER, 90),
                        Stroke::NONE,
                    ));
                }
                painter.rect_stroke(
                    leaf_rect(face.leaf),
                    0.0,
                    Stroke::new(2.0, HOVER),
                    StrokeKind::Inside,
                );
                if let Some(s) = face.surface(world) {
                    let ([x0, z0], size) = super::view::leaf_square(world.scale, face.leaf);
                    let c = [
                        [x0, z0],
                        [x0 + size, z0],
                        [x0 + size, z0 + size],
                        [x0, z0 + size],
                    ];
                    let (a, b) = match s.split {
                        psxed_project::grid::GridSplit::NwSe => (c[0], c[2]),
                        psxed_project::grid::GridSplit::NeSw => (c[1], c[3]),
                    };
                    line(a, b, HOVER, 2.0);
                }
            }
            GridHover::Spot { position } => {
                let p = screen([position[0], position[2]]);
                painter.circle_stroke(p, 7.0, Stroke::new(2.0, CREATE));
                painter.line_segment(
                    [p - Vec2::new(10.0, 0.0), p + Vec2::new(10.0, 0.0)],
                    Stroke::new(1.5, CREATE),
                );
                painter.line_segment(
                    [p - Vec2::new(0.0, 10.0), p + Vec2::new(0.0, 10.0)],
                    Stroke::new(1.5, CREATE),
                );
            }
            GridHover::None
            | GridHover::Band { .. }
            | GridHover::Plane(_)
            | GridHover::Mesh(_)
            | GridHover::MeshSpot { .. }
            | GridHover::Grip(..)
            | GridHover::PlaneSpot(..) => {}
        }
        match &self.grid.gesture {
            Gesture::Rect(g) => {
                let rect = CellRect::from_corners(g.anchor, g.current);
                let (color, warning) = rect_ghost(world, g, rect);
                let unit = super::cell_size::cell_units(g.level);
                if rect.count() <= GHOST_CELL_LIMIT {
                    for cell in rect.cells() {
                        let r = cell_rect_at(unit, cell);
                        painter.rect_filled(r, 0.0, fill(color, 56));
                        painter.rect_stroke(
                            r,
                            0.0,
                            Stroke::new(1.0, fill(color, 160)),
                            StrokeKind::Inside,
                        );
                    }
                }
                let outer = Rect::from_two_pos(
                    cell_rect_at(unit, rect.min).min,
                    cell_rect_at(unit, rect.max).max,
                );
                painter.rect_stroke(outer, 0.0, Stroke::new(2.5, color), StrokeKind::Inside);
                label_chip(
                    painter,
                    outer.center_top() - Vec2::new(0.0, 6.0),
                    Align2::CENTER_BOTTOM,
                    &warning.unwrap_or_else(|| format!("{} x {}", rect.width(), rect.depth())),
                );
            }
            Gesture::Walls(g) => {
                let color = if g.erase { ERASE } else { CREATE };
                for cell in psxed_project::grid::edit::wall_run_cells(g.anchor, g.current, g.edge) {
                    wall_line(g.room, cell, g.edge, color, 5.0);
                }
            }
            Gesture::Marquee { start, current, .. } => {
                draw_viewport_box_select_marquee(
                    painter,
                    Some(Rect::from_two_pos(*start, *current)),
                );
            }
            _ => {}
        }
        if let Some(ghost) = &self.grid.paste {
            if let Some(at) = ghost.at {
                for (offset, _) in ghost.clip.sectors() {
                    let cell = [at[0] + offset[0], at[1] + offset[1]];
                    let r = cell_rect(cell);
                    painter.rect_filled(r, 0.0, fill(GHOST, 50));
                    painter.rect_stroke(r, 0.0, Stroke::new(1.5, GHOST), StrokeKind::Inside);
                }
            }
        }
        self.draw_plane_overlay_2d(painter, transform);
        self.draw_mesh_overlay_2d(painter, transform);
        self.draw_verb_overlay_2d(painter, transform);
        self.draw_walk_overlay_2d(painter, transform);
        self.draw_heat_overlay_2d(painter, transform);
        self.draw_leaf_overlay_2d(painter, transform);
        self.draw_level_chip(painter, painter.clip_rect());
    }

    // ---- 3D view ----

    /// Highlights and ghosts over the 3D view.
    pub(crate) fn draw_grid_overlay_3d(&self, painter: &egui::Painter, rect: Rect) {
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        self.grid.view_rect.set(Some(rect));
        let camera = self.viewport_3d_camera();
        let cam = Cam::new(camera, rect);
        let eye = camera.position_f32();
        let erase = Self::erase_held(painter);

        let segment = |a: [f32; 3], b: [f32; 3], color: Color32, width: f32| {
            if let Some([pa, pb]) = view::project_segment(camera, rect, a, b) {
                painter.line_segment([pa, pb], Stroke::new(width, color));
            }
        };
        let polygon = |points: &[[f32; 3]], color: Color32, fill_alpha: u8, width: f32| {
            let projected: Vec<Pos2> = points
                .iter()
                .filter_map(|p| cam.project(*p).map(|(s, _)| s))
                .collect();
            if projected.len() == points.len() && fill_alpha > 0 && points.len() >= 3 {
                painter.add(egui::Shape::convex_polygon(
                    projected,
                    fill(color, fill_alpha),
                    Stroke::NONE,
                ));
            }
            for i in 0..points.len() {
                segment(points[i], points[(i + 1) % points.len()], color, width);
            }
        };

        // Faces of the selection and of the hover, from the pick cache.
        let mut selected: BTreeSet<FaceRef> = BTreeSet::new();
        for element in self.grid.selection.items() {
            selected.extend(element_faces(world, element));
        }
        let mut hovered: BTreeSet<FaceRef> = BTreeSet::new();
        match &self.grid.hover {
            GridHover::Elements(list) => {
                for element in list {
                    hovered.extend(element_faces(world, element));
                }
            }
            GridHover::Face { face, .. } => {
                hovered.insert(*face);
            }
            _ => {}
        }
        if !selected.is_empty() || !hovered.is_empty() {
            self.with_pick_cache(|cache| {
                for pf in &cache.faces {
                    let (is_sel, is_hover) =
                        (selected.contains(&pf.face), hovered.contains(&pf.face));
                    if !is_sel && !is_hover {
                        continue;
                    }
                    let n = pf.points.len() as f32;
                    let centre = pf.points.iter().fold([0.0; 3], |acc, p| {
                        [acc[0] + p[0] / n, acc[1] + p[1] / n, acc[2] + p[2] / n]
                    });
                    let to_eye = [eye[0] - centre[0], eye[1] - centre[1], eye[2] - centre[2]];
                    let facing = pf.normal[0] * to_eye[0]
                        + pf.normal[1] * to_eye[1]
                        + pf.normal[2] * to_eye[2];
                    if facing <= 0.0 && !pf.double_sided {
                        continue;
                    }
                    if is_sel {
                        polygon(&pf.points, SELECT, 64, 2.0);
                    } else {
                        polygon(&pf.points, HOVER, 36, 1.5);
                    }
                }
            });
        }
        // A void triangle has no face; while a tool that can pick it is
        // active, show where it is.
        if matches!(self.grid.tool, GridTool::Select | GridTool::Split) {
            self.with_pick_cache(|cache| {
                for pf in cache.faces.iter().filter(|pf| pf.void) {
                    if !selected.contains(&pf.face) {
                        polygon(&pf.points, Color32::from_rgb(190, 150, 255), 26, 1.5);
                    }
                }
            });
        }
        if let GridHover::Face { face, tri } = &self.grid.hover {
            if let Some(points) = tri.and_then(|t| tri_corners(world, *face, t)) {
                polygon(&points, HOVER, 120, 2.5);
            }
            // The diagonal, so a split click shows what it will flip.
            if let (Some(surface), Some(room)) = (face.surface(world), world.rooms.get(face.room)) {
                let ([x0, z0], size) = super::view::leaf_square(world.scale, face.leaf);
                let at = |i: usize| {
                    let o = [[0, 0], [1, 0], [1, 1], [0, 1]][i];
                    let h = surface.heights[i];
                    [
                        x0 + size * o[0] as f32,
                        editor_y(world, room.y_offset, h),
                        z0 + size * o[1] as f32,
                    ]
                };
                let (a, b) = match surface.split {
                    psxed_project::grid::GridSplit::NwSe => (0, 2),
                    psxed_project::grid::GridSplit::NeSw => (1, 3),
                };
                segment(at(a), at(b), HOVER, 3.0);
            }
        }
        // Edges and vertices.
        let mark = |element: &GridElement, color: Color32| match element {
            GridElement::Triangle { face, tri } => {
                if let Some(points) = tri_corners(world, *face, *tri) {
                    polygon(&points, color, 100, 2.5);
                }
            }
            GridElement::Edge(e) => {
                if let Some((a, b)) = edge_ends(world, e) {
                    segment(a, b, color, 4.0);
                }
            }
            GridElement::Vertex(v) => {
                if let Some(p) = corner_pos(world, v) {
                    if let Some((s, _)) = cam.project(p) {
                        painter.circle_filled(s, 5.5, color);
                        painter.circle_stroke(s, 6.5, Stroke::new(1.5, Color32::BLACK));
                    }
                }
            }
            _ => {}
        };
        for element in self.grid.selection.items() {
            mark(element, SELECT);
        }
        if let GridHover::Elements(list) = &self.grid.hover {
            for element in list {
                mark(element, HOVER);
            }
        }

        // Ghosts.
        let rect_quad =
            |room: usize, level: u8, r: CellRect, height: i32| -> Option<[[f32; 3]; 4]> {
                let y_offset = world.rooms.get(room).map_or(0, |ro| ro.y_offset);
                let y = editor_y(world, y_offset, height);
                let s = world.scale;
                let unit = super::cell_size::cell_units(level);
                let (x0, z0) = (r.min[0] * unit, r.min[1] * unit);
                let (x1, z1) = ((r.max[0] + 1) * unit, (r.max[1] + 1) * unit);
                let p = |x: i32, z: i32| [s.to_editor(x) as f32, y, s.to_editor(z) as f32];
                Some([p(x0, z0), p(x1, z0), p(x1, z1), p(x0, z1)])
            };
        match &self.grid.hover {
            GridHover::Cell { room, cell, height }
                if matches!(self.grid.gesture, Gesture::Idle) =>
            {
                let level = self.grid.cell_size.level();
                if let Some(quad) =
                    rect_quad(*room, level, CellRect::from_corners(*cell, *cell), *height)
                {
                    let color = if erase { ERASE } else { CREATE };
                    polygon(&quad, color, 70, 2.0);
                }
            }
            GridHover::Wall {
                room,
                cell,
                edge,
                heights,
            } if matches!(self.grid.gesture, Gesture::Idle) => {
                self.draw_wall_ghost_3d(&polygon, world, *room, *cell, *edge, *heights, erase);
            }
            GridHover::Spot { position } => {
                let top = [position[0], position[1] + 600.0, position[2]];
                segment(*position, top, CREATE, 2.5);
                if let Some((s, _)) = cam.project(top) {
                    painter.circle_filled(s, 5.0, CREATE);
                }
                let r = 160.0;
                let ring = [
                    [position[0] - r, position[1], position[2] - r],
                    [position[0] + r, position[1], position[2] - r],
                    [position[0] + r, position[1], position[2] + r],
                    [position[0] - r, position[1], position[2] + r],
                ];
                polygon(&ring, CREATE, 40, 1.5);
            }
            _ => {}
        }
        match &self.grid.gesture {
            Gesture::Rect(g) => {
                let r = CellRect::from_corners(g.anchor, g.current);
                if let Some(quad) = rect_quad(g.room, g.level, r, g.height) {
                    let (color, warning) = rect_ghost(world, g, r);
                    polygon(&quad, color, 70, 2.5);
                    // Cell grid inside the ghost.
                    if r.count() <= GHOST_CELL_LIMIT {
                        for cell in r.cells() {
                            if let Some(q) = rect_quad(
                                g.room,
                                g.level,
                                CellRect::from_corners(cell, cell),
                                g.height,
                            ) {
                                polygon(&q, fill(color, 140), 0, 1.0);
                            }
                        }
                    }
                    let mid = [
                        (quad[0][0] + quad[2][0]) / 2.0,
                        quad[0][1],
                        (quad[0][2] + quad[2][2]) / 2.0,
                    ];
                    if let Some((s, _)) = cam.project(mid) {
                        label_chip(
                            painter,
                            s,
                            Align2::CENTER_CENTER,
                            &warning.unwrap_or_else(|| {
                                format!(
                                    "{} x {}   height {:+.2}",
                                    r.width(),
                                    r.depth(),
                                    g.height as f32 / psxed_project::grid::CLICK_UNITS as f32
                                )
                            }),
                        );
                    }
                }
            }
            Gesture::Walls(g) => {
                let color = if g.erase { ERASE } else { CREATE };
                let room = g.room;
                for cell in psxed_project::grid::edit::wall_run_cells(g.anchor, g.current, g.edge) {
                    let heights = world.rooms.get(room).and_then(|r| {
                        psxed_project::grid::edit::wall_gap(r, cell, g.edge, g.pick_y)
                    });
                    let _ = color;
                    self.draw_wall_ghost_3d(&polygon, world, room, cell, g.edge, heights, g.erase);
                }
            }
            Gesture::Marquee { start, current, .. } => {
                draw_viewport_box_select_marquee(
                    painter,
                    Some(Rect::from_two_pos(*start, *current)),
                );
            }
            _ => {}
        }
        if let Some(ghost) = &self.grid.paste {
            self.draw_paste_ghost_3d(&polygon, world, ghost);
        }
        self.draw_plane_overlay_3d(painter, rect);
        self.draw_mesh_overlay_3d(painter, rect);
        self.draw_band_overlay_3d(painter, rect);
        self.draw_verb_overlay_3d(painter, rect);
        self.draw_walk_overlay_3d(painter, rect);
        self.draw_heat_overlay_3d(painter, rect);
        self.draw_leaf_overlay_3d(painter, rect);
        self.draw_leaf_hover_3d(painter, rect);
        self.draw_level_chip(painter, rect);
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_wall_ghost_3d(
        &self,
        polygon: &PolygonFn,
        world: &GridWorld,
        room: usize,
        cell: [i32; 2],
        edge: WallEdge,
        heights: Option<[i32; 4]>,
        erase: bool,
    ) {
        let y_offset = world.rooms.get(room).map_or(0, |r| r.y_offset);
        let unit = super::cell_size::cell_units(self.grid.cell_size.level());
        let (a, b) = edge.endpoints();
        let at = |p: [i32; 2], h: i32| {
            [
                world.scale.to_editor((cell[0] + p[0]) * unit) as f32,
                editor_y(world, y_offset, h),
                world.scale.to_editor((cell[1] + p[1]) * unit) as f32,
            ]
        };
        match heights {
            Some(h) => {
                let quad = [at(a, h[0]), at(b, h[1]), at(b, h[2]), at(a, h[3])];
                polygon(&quad, if erase { ERASE } else { CREATE }, 80, 2.5);
            }
            None => {
                let quad = [at(a, 0), at(b, 0), at(b, 256), at(a, 256)];
                polygon(&quad, Color32::from_rgb(150, 150, 160), 30, 1.5);
            }
        }
    }

    fn draw_paste_ghost_3d(&self, polygon: &PolygonFn, world: &GridWorld, ghost: &PasteGhost) {
        let Some(at) = ghost.at else { return };
        let room = self.grid_target_room();
        let Some(target) = world.rooms.get(room) else {
            return;
        };
        let mut temp = psxed_project::grid::GridRoom::new("ghost", 1, 1);
        temp.origin = at;
        temp.y_offset = target.y_offset;
        let size = ghost.clip.size();
        temp.grow_to_include(i64::from(size[0] - 1), i64::from(size[1] - 1));
        for (offset, sector) in ghost.clip.sectors() {
            if let Some(slot) = temp.sector_or_insert(offset[0] as usize, offset[1] as usize) {
                *slot = sector.clone();
            }
        }
        let temp_world = GridWorld {
            scale: world.scale,
            rooms: vec![temp],
            ..GridWorld::default()
        };
        for face in psxed_project::grid::faces::all_faces(&temp_world) {
            let points: Vec<[f32; 3]> = face
                .points
                .iter()
                .map(|p| [p[0] as f32, p[1] as f32, p[2] as f32])
                .collect();
            polygon(&points, GHOST, 36, 1.5);
        }
    }
}
