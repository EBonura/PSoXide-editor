//! Picking in the grid world: faces, edges and vertices in the 3D view, and
//! cells and wall lines in the Top view.
//!
//! The 3D pick works the way the bonnie-32 editor's does, in screen space:
//! a vertex within a few pixels of the pointer wins, then an edge, then the
//! face under the pointer. Depth keeps it honest: a corner or edge behind
//! the face the pointer is over is not offered. Everything is built from a
//! cache of the document's faces, corners and edges that is rebuilt only
//! when the document changes.

use super::view::{ray_triangle, GridPointer};
use super::*;

use psxed_project::grid::edit::{
    faces_of_cell, faces_of_leaf, CornerRef, EdgeRef, FaceRef, FaceTarget, GridElement, SurfaceKind,
};
use psxed_project::grid::faces::{all_faces, GridFaceKind};
use psxed_project::grid::{GridWorld, WallEdge, WallMode};

/// A face, as the picker sees it.
pub(crate) struct PickFace {
    pub face: FaceRef,
    /// Which triangle of a split surface, when it is split.
    pub tri: Option<u8>,
    pub points: Vec<[f32; 3]>,
    pub normal: [f32; 3],
    pub double_sided: bool,
    /// A void triangle: pickable, but nothing is drawn there.
    pub void: bool,
    /// The band of its wall this face is a piece of, for a banded wall.
    pub band: Option<u8>,
}

impl PickFace {
    /// Whether the camera at `eye` sees this face's front (or the face is
    /// drawn from both sides). The renderer culls by the side of the face's
    /// plane the eye is on.
    pub fn faces(&self, eye: [f32; 3]) -> bool {
        if self.double_sided {
            return true;
        }
        let n = self.points.len().max(1) as f32;
        let centre = self.points.iter().fold([0.0; 3], |acc, p| {
            [acc[0] + p[0] / n, acc[1] + p[1] / n, acc[2] + p[2] / n]
        });
        let to_eye = [eye[0] - centre[0], eye[1] - centre[1], eye[2] - centre[2]];
        self.normal[0] * to_eye[0] + self.normal[1] * to_eye[1] + self.normal[2] * to_eye[2] > 0.0
    }
}

/// One corner of a face in editor space.
pub(crate) struct PickCorner {
    pub corner: CornerRef,
    pub pos: [f32; 3],
}

/// One edge of a face in editor space.
pub(crate) struct PickEdge {
    pub edge: EdgeRef,
    pub a: [f32; 3],
    pub b: [f32; 3],
}

/// Everything the picker needs, for one document revision.
pub(crate) struct PickCache {
    pub rev: u64,
    pub faces: Vec<PickFace>,
    pub corners: Vec<PickCorner>,
    pub edges: Vec<PickEdge>,
    pub planes: Vec<super::plane_pick::PickPlane>,
}

fn to_f32(p: [i32; 3]) -> [f32; 3] {
    p.map(|v| v as f32)
}

fn build_cache(world: &GridWorld, rev: u64) -> PickCache {
    let mut faces = Vec::new();
    for face in all_faces(world) {
        let (target, tri) = match face.kind {
            GridFaceKind::Floor { triangle } => (
                FaceTarget::Surface {
                    kind: SurfaceKind::Floor,
                    span: face.span,
                },
                triangle,
            ),
            GridFaceKind::Ceiling { triangle } => (
                FaceTarget::Surface {
                    kind: SurfaceKind::Ceiling,
                    span: face.span,
                },
                triangle,
            ),
            GridFaceKind::Wall { edge, index } => (FaceTarget::Wall { edge, index }, None),
            GridFaceKind::Derived { edge, kind } => {
                // A cell next to smaller ones has several faces of one rule on
                // one side: name which by where this one lies.
                let seg = world
                    .rooms
                    .get(face.room)
                    .filter(|r| !r.leaves.is_sectors_only())
                    .map_or(0, |r| {
                        let (n, sx, sz) = face
                            .grid
                            .iter()
                            .fold((0, 0, 0), |(n, x, z), g| (n + 1, x + g[0], z + g[2]));
                        psxed_project::grid::leaf_derive::derived_segment(
                            r,
                            face.leaf,
                            edge,
                            kind,
                            [sx / n.max(1), sz / n.max(1)],
                        )
                    });
                (
                    FaceTarget::Derived {
                        edge,
                        kind,
                        seg,
                        span: face.span,
                    },
                    None,
                )
            }
            // Free planes have their own entries below.
            GridFaceKind::Plane { .. } | GridFaceKind::Mesh { .. } => continue,
        };
        faces.push(PickFace {
            face: FaceRef::new(face.room, face.leaf, target),
            tri,
            points: face.points.iter().copied().map(to_f32).collect(),
            normal: face.normal.map(|v| v as f32),
            double_sided: face.double_sided,
            void: false,
            band: face.band,
        });
    }
    // A void triangle draws no face, but it can still be picked (and
    // restored), so it joins the pick cache as an invisible one.
    for (room_index, room) in world.rooms.iter().enumerate() {
        for (key, sector) in room.leaves() {
            for (index, span) in sector.spans().iter().enumerate() {
                let span_index = u8::try_from(index).unwrap_or(u8::MAX);
                for (kind, surface) in [
                    (SurfaceKind::Floor, span.floor.as_ref()),
                    (SurfaceKind::Ceiling, span.ceiling.as_ref()),
                ] {
                    let Some(surface) = surface.filter(|s| s.void != 0) else {
                        continue;
                    };
                    let face = FaceRef::new(
                        room_index,
                        key,
                        FaceTarget::Surface {
                            kind,
                            span: span_index,
                        },
                    );
                    for tri in 0..2u8 {
                        if !surface.is_void(usize::from(tri)) {
                            continue;
                        }
                        let points: Vec<[f32; 3]> = face
                            .triangle_corners(world, tri)
                            .iter()
                            .filter_map(|c| c.grid_pos(world))
                            .map(|g| {
                                [
                                    world.scale.to_editor(g[0]) as f32,
                                    world.scale.to_editor(g[1]) as f32,
                                    world.scale.to_editor(g[2]) as f32,
                                ]
                            })
                            .collect();
                        if points.len() == 3 {
                            faces.push(PickFace {
                                face,
                                tri: Some(tri),
                                points,
                                normal: [0.0, 1.0, 0.0],
                                double_sided: true,
                                void: true,
                                band: None,
                            });
                        }
                    }
                }
            }
        }
    }
    let mut corners = Vec::new();
    let mut edges = Vec::new();
    for (room_index, room) in world.rooms.iter().enumerate() {
        for [x, z] in room.face_cells() {
            for face in faces_of_cell(world, room_index, room.world_cell(x, z)) {
                let mut position = std::collections::HashMap::new();
                for corner in face.corners(world) {
                    let Some(g) = corner.grid_pos(world) else {
                        continue;
                    };
                    let pos = [
                        world.scale.to_editor(g[0]) as f32,
                        world.scale.to_editor(g[1]) as f32,
                        world.scale.to_editor(g[2]) as f32,
                    ];
                    position.insert((corner.layer, corner.corner), pos);
                    corners.push(PickCorner { corner, pos });
                }
                for layer in 0..face.layers(world) {
                    for edge in 0..4u8 {
                        let e = EdgeRef { face, layer, edge };
                        let (Some(a), Some(b)) = (
                            position.get(&(layer, edge)),
                            position.get(&(layer, (edge + 1) & 3)),
                        ) else {
                            continue;
                        };
                        if a != b {
                            edges.push(PickEdge {
                                edge: e,
                                a: *a,
                                b: *b,
                            });
                        }
                    }
                }
            }
        }
    }
    PickCache {
        rev,
        faces,
        corners,
        edges,
        planes: super::plane_pick::build_planes(world),
    }
}

/// A camera reduced to what projecting many points needs.
#[derive(Clone, Copy)]
pub(crate) struct Cam {
    position: [f32; 3],
    forward: [f32; 3],
    right: [f32; 3],
    up: [f32; 3],
    center: Pos2,
    half: Vec2,
}

impl Cam {
    pub fn new(camera: ViewportCameraState, rect: Rect) -> Self {
        let basis = camera.basis();
        Self {
            position: basis.position,
            forward: basis.forward,
            right: basis.right,
            up: basis.up,
            center: rect.center(),
            half: rect.size() * 0.5,
        }
    }

    /// Screen position and depth, or `None` at or behind the camera plane.
    pub fn project(&self, p: [f32; 3]) -> Option<(Pos2, f32)> {
        let v = [
            p[0] - self.position[0],
            p[1] - self.position[1],
            p[2] - self.position[2],
        ];
        let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let depth = dot(v, self.forward);
        if depth <= 1.0 {
            return None;
        }
        let (half_fov_x, half_fov_y) = (0.5_f32, 0.5 * 240.0 / 320.0);
        let nx = dot(v, self.right) / depth / half_fov_x;
        let ny = -dot(v, self.up) / depth / half_fov_y;
        Some((
            Pos2::new(
                self.center.x + nx * self.half.x,
                self.center.y + ny * self.half.y,
            ),
            depth,
        ))
    }
}

/// What the pointer is over on a face.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FaceHit {
    pub face: FaceRef,
    /// Triangle of a split surface under the pointer, when split.
    pub tri: Option<u8>,
    /// Hit point, editor units.
    pub point: [f32; 3],
    /// Distance along the ray.
    pub depth: f32,
    /// Unit normal of the face, on the side the ray came from.
    pub normal: [f32; 3],
    /// The band of a banded wall under the pointer.
    pub band: Option<u8>,
}

/// The finest thing a tool wants to pick.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum PickLevel {
    /// Faces only.
    Face,
    /// Vertices, then edges, then faces.
    Vertex,
}

/// Pixels within which a vertex is picked.
pub(crate) const VERTEX_PICK_PIXELS: f32 = 9.0;
/// Pixels within which an edge is picked.
pub(crate) const EDGE_PICK_PIXELS: f32 = 6.0;

fn distance_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_sq();
    if len2 < 1e-6 {
        return (p - a).length();
    }
    let t = (((p - a).dot(ab)) / len2).clamp(0.0, 1.0);
    (p - (a + ab * t)).length()
}

impl EditorWorkspace {
    /// Run `visit` on the pick cache, rebuilding it first if the document
    /// changed. `None` for a project with no grid world.
    pub(crate) fn with_pick_cache<R>(&self, visit: impl FnOnce(&PickCache) -> R) -> Option<R> {
        let world = self.project.active_scene().world.grid()?;
        let mut slot = self.grid.cache.borrow_mut();
        if slot.as_ref().map(|c| c.rev) != Some(self.grid.rev) {
            *slot = Some(build_cache(world, self.grid.rev));
        }
        slot.as_ref().map(visit)
    }

    /// Nearest face under a 3D pointer that is facing the camera (or
    /// double-sided), skipping hidden rooms.
    pub(crate) fn pick_face_3d(&self, rect: Rect, pos: Pos2) -> Option<FaceHit> {
        let (origin, dir) = self.camera_ray_for_pointer(rect, pos)?;
        let cut = self.cutaway_plane().map(|y| y as f32);
        self.with_pick_cache(|cache| {
            let mut best: Option<FaceHit> = None;
            for pf in &cache.faces {
                if self.grid_hidden_rooms.contains(&pf.face.room) {
                    continue;
                }
                // The cutaway hides what lies above the level: not pickable.
                if cut.is_some_and(|cut| pf.points.iter().all(|p| p[1] >= cut)) {
                    continue;
                }
                let facing = pf.normal[0] * dir[0] + pf.normal[1] * dir[1] + pf.normal[2] * dir[2];
                if facing >= 0.0 && !pf.double_sided {
                    continue;
                }
                let p = &pf.points;
                let mut hit = ray_triangle(origin, dir, p[0], p[1], p[2]);
                if hit.is_none() && p.len() == 4 {
                    hit = ray_triangle(origin, dir, p[0], p[2], p[3]);
                }
                let Some(depth) = hit else { continue };
                if best.is_none_or(|b| depth < b.depth) {
                    let flip = if facing > 0.0 { -1.0 } else { 1.0 };
                    best = Some(FaceHit {
                        face: pf.face,
                        tri: pf.tri,
                        band: pf.band,
                        normal: pf.normal.map(|v| v * flip),
                        point: [
                            origin[0] + dir[0] * depth,
                            origin[1] + dir[1] * depth,
                            origin[2] + dir[2] * depth,
                        ],
                        depth,
                    });
                }
            }
            best
        })
        .flatten()
    }

    /// The vertex, edge or face under a 3D pointer, finest first, limited to
    /// `level` or coarser. Corners and edges behind the face under the
    /// pointer are not offered.
    pub(crate) fn pick_element_3d(
        &self,
        rect: Rect,
        pos: Pos2,
        level: PickLevel,
    ) -> Option<(GridElement, Option<FaceHit>)> {
        let face_hit = self.pick_face_3d(rect, pos);
        let cam = Cam::new(self.viewport_3d_camera(), rect);
        let limit = face_hit.map(|h| h.depth);
        // Depth is along the ray, the camera's is along its axis; the
        // tolerance covers the difference and neighbouring geometry.
        let visible = |depth: f32| limit.is_none_or(|l| depth <= l * 1.02 + 24.0);
        let cut = self.cutaway_plane().map(|y| y as f32);
        let eye = self.viewport_3d_camera().position_f32();
        let found = self
            .with_pick_cache(|cache| {
                // A face the camera does not draw (a ceiling seen from above,
                // the back of a one-sided wall) offers no corner or edge.
                // Faces the cache does not know are left alone.
                let mut known = std::collections::HashSet::new();
                let mut drawn = std::collections::HashSet::new();
                if level != PickLevel::Face {
                    for pf in cache.faces.iter().filter(|pf| !pf.void) {
                        known.insert(pf.face);
                        if pf.faces(eye) {
                            drawn.insert(pf.face);
                        }
                    }
                }
                let culled = |face: &FaceRef| known.contains(face) && !drawn.contains(face);
                if level == PickLevel::Vertex {
                    let mut best: Option<(f32, f32, CornerRef)> = None;
                    for c in &cache.corners {
                        if self.grid_hidden_rooms.contains(&c.corner.face.room)
                            || cut.is_some_and(|cut| c.pos[1] >= cut)
                            || culled(&c.corner.face)
                        {
                            continue;
                        }
                        let Some((screen, depth)) = cam.project(c.pos) else {
                            continue;
                        };
                        let distance = screen.distance(pos);
                        if distance > VERTEX_PICK_PIXELS || !visible(depth) {
                            continue;
                        }
                        let better = best.is_none_or(|(d, z, _)| {
                            distance < d - 1.5 || (distance < d + 1.5 && depth < z)
                        });
                        if better {
                            best = Some((distance, depth, c.corner));
                        }
                    }
                    if let Some((_, _, corner)) = best {
                        return Some(GridElement::Vertex(corner));
                    }
                }
                if level != PickLevel::Face {
                    let mut best: Option<(f32, EdgeRef)> = None;
                    for e in &cache.edges {
                        if self.grid_hidden_rooms.contains(&e.edge.face.room)
                            || cut.is_some_and(|cut| e.a[1].min(e.b[1]) >= cut)
                            || culled(&e.edge.face)
                        {
                            continue;
                        }
                        let (Some((a, da)), Some((b, db))) = (cam.project(e.a), cam.project(e.b))
                        else {
                            continue;
                        };
                        let distance = distance_to_segment(pos, a, b);
                        if distance > EDGE_PICK_PIXELS || !visible(da.min(db)) {
                            continue;
                        }
                        if best.is_none_or(|(d, _)| distance < d) {
                            best = Some((distance, e.edge));
                        }
                    }
                    if let Some((_, edge)) = best {
                        return Some(GridElement::Edge(edge));
                    }
                }
                None
            })
            .flatten();
        match (found, face_hit) {
            (Some(element), hit) => Some((element, hit)),
            (None, Some(hit)) => Some((GridElement::Face(hit.face), Some(hit))),
            (None, None) => None,
        }
    }

    /// Room a Top view click at `cell` lands in: the active room when it has
    /// geometry there, else the highest visible room that does.
    pub(crate) fn top_room_at(&self, cell: [i32; 2]) -> Option<usize> {
        let world = self.project.active_scene().world.grid()?;
        // A sector the Cells tool divided (or a cell of several sectors) has
        // no level-2 leaf of its own, so ask for every leaf that touches it.
        let has = |room: usize| {
            world.rooms.get(room).is_some_and(|r| {
                r.local(cell).is_some()
                    && r.leaves
                        .in_sector(cell)
                        .iter()
                        .any(|(_, sector)| !sector.is_empty())
            })
        };
        if let Some(active) = self.grid_active_room {
            if !self.grid_hidden_rooms.contains(&active) && has(active) {
                return Some(active);
            }
        }
        let mut rooms: Vec<usize> = (0..world.rooms.len())
            .filter(|r| !self.grid_hidden_rooms.contains(r) && has(*r))
            .collect();
        rooms.sort_by_key(|r| core::cmp::Reverse(world.rooms[*r].y_offset));
        rooms.first().copied()
    }

    /// The wall column nearest the pointer in the Top view: the cell it
    /// belongs to and the edge, when a wall edge lies within `pixels`.
    pub(crate) fn top_wall_at(
        &self,
        transform: ViewportTransform,
        pos: Pos2,
        pixels: f32,
    ) -> Option<(usize, [i32; 2], WallEdge)> {
        let world = self.project.active_scene().world.grid()?;
        let [wx, wz] = transform.screen_to_world(pos);
        let scale = world.scale;
        let center = super::view::cell_at_editor(scale, wx, wz);
        let mut best: Option<(f32, usize, [i32; 2], WallEdge)> = None;
        for dz in -1..=1 {
            for dx in -1..=1 {
                let cell = [center[0] + dx, center[1] + dz];
                let Some(room_index) = self.top_room_at(cell) else {
                    continue;
                };
                let room = &world.rooms[room_index];
                let Some(sector) = room.local(cell).and_then(|[x, z]| room.sector(x, z)) else {
                    continue;
                };
                let derived = room.wall_mode == WallMode::Derived;
                for edge in WallEdge::ALL {
                    let level = self.grid.level.y;
                    let has_derived = derived
                        && psxed_project::grid::derive::edge_walls(room, cell, edge)
                            .iter()
                            .any(|w| crate::grid_view::wall_in_slice(room, w, level));
                    if sector.walls(edge).is_empty() && !has_derived {
                        continue;
                    }
                    let (a, b) = edge.endpoints();
                    let at = |p: [i32; 2]| {
                        transform.world_to_screen(super::view::editor_xz(
                            scale,
                            (cell[0] + p[0]) * psxed_project::grid::SECTOR_UNITS,
                            (cell[1] + p[1]) * psxed_project::grid::SECTOR_UNITS,
                        ))
                    };
                    let d = distance_to_segment(pos, at(a), at(b));
                    if d <= pixels && best.is_none_or(|(bd, ..)| d < bd) {
                        best = Some((d, room_index, cell, edge));
                    }
                }
            }
        }
        best.map(|(_, room, cell, edge)| (room, cell, edge))
    }

    /// The pointer's element for the Select tool: in the Top view a wall
    /// column or a whole cell, in 3D the finest element under it.
    pub(crate) fn grid_pick_for_select(
        &self,
        ptr: &GridPointer,
        level: PickLevel,
    ) -> Option<Vec<GridElement>> {
        match ptr.view {
            super::view::GridView::Top(transform) => {
                if let Some((room, cell, edge)) = self.top_wall_at(transform, ptr.pos, 6.0) {
                    let world = self.project.active_scene().world.grid()?;
                    let count = FaceRef::floor(room, cell)
                        .sector(world)
                        .map_or(0, |s| s.walls(edge).len());
                    let mut faces: Vec<GridElement> = (0..count)
                        .map(|index| GridElement::Face(FaceRef::wall(room, cell, edge, index)))
                        .collect();
                    if let Some(r) = world
                        .rooms
                        .get(room)
                        .filter(|r| r.wall_mode == WallMode::Derived)
                    {
                        faces.extend(
                            psxed_project::grid::derive::edge_walls(r, cell, edge)
                                .into_iter()
                                .filter(|w| {
                                    crate::grid_view::wall_in_slice(r, w, self.grid.level.y)
                                })
                                .map(|w| GridElement::Face(FaceRef::of_derived(room, &w))),
                        );
                    }
                    return Some(faces);
                }
                let world = self.project.active_scene().world.grid()?;
                let [wx, wz] = transform.screen_to_world(ptr.pos);
                let cell = super::view::cell_at_editor(world.scale, wx, wz);
                let room = self.top_room_at(cell)?;
                // A cracked, voided or flipped surface is picked a triangle at
                // a time (Alt names the triangle of a plain quad too).
                if let Some((faces, Some(tri))) =
                    super::face_tools::face_under_with(self, ptr, ptr.alt())
                {
                    if let Some(face) = faces
                        .first()
                        .filter(|f| matches!(f.target, FaceTarget::Surface { .. }))
                    {
                        return Some(vec![GridElement::Triangle { face: *face, tri }]);
                    }
                }
                // A divided sector, or a cell of several sectors, is picked
                // one leaf at a time: the leaf under the pointer and all of
                // its faces.
                if let Some(r) = world
                    .rooms
                    .get(room)
                    .filter(|r| !r.leaves.is_sectors_only())
                {
                    if let Some((key, _)) = super::view::leaf_at_editor(world.scale, r, wx, wz) {
                        if key != psxed_project::grid::LeafKey::sector(cell) {
                            let faces = faces_of_leaf(world, room, key);
                            if !faces.is_empty() {
                                return Some(faces.into_iter().map(GridElement::Face).collect());
                            }
                        }
                    }
                }
                Some(vec![GridElement::Cell { room, cell }])
            }
            super::view::GridView::Persp(rect) => {
                let (element, hit) = self.pick_element_3d(rect, ptr.pos, level)?;
                // One band of a banded wall is picked on its own.
                if let (
                    GridElement::Face(face),
                    Some(FaceHit {
                        band: Some(band), ..
                    }),
                ) = (element, hit)
                {
                    return Some(vec![GridElement::Band { face, band }]);
                }
                // A face that is cut into triangles is picked one triangle at
                // a time; Alt names the triangle of a plain quad too.
                if let (GridElement::Face(face), Some(hit)) = (element, hit) {
                    if matches!(face.target, FaceTarget::Surface { .. }) {
                        let world = self.project.active_scene().world.grid()?;
                        let tri = hit.tri.or_else(|| {
                            ptr.alt()
                                .then(|| {
                                    super::face_tools::tri_at(
                                        world,
                                        face,
                                        hit.point[0],
                                        hit.point[2],
                                    )
                                })
                                .flatten()
                        });
                        if let Some(tri) = tri {
                            return Some(vec![GridElement::Triangle { face, tri }]);
                        }
                    }
                }
                Some(vec![element])
            }
        }
    }
}
