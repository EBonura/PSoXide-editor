//! A top-down plan of the grid world as a PNG.
//!
//! Heights are colours (blue low, orange high), derived and authored walls are
//! lines on the sector edges, rooms are outlined and numbered, planes are
//! drawn as their footprint, and nodes are numbered markers. The 3D preview is
//! poor at showing how much space there is and where things sit; this is the
//! picture for layout, the screenshot is the picture for mood.

use std::fmt::Write as _;

use psxed_project::grid::derive::{self, DerivedKind};
use psxed_project::grid::{GridWorld, PlaneCollision, WallEdge};
use psxed_project::{NodeId, NodeKind, ProjectDocument};
use schemars::JsonSchema;
use serde::Deserialize;

use super::{check_rect, clicks, Cells};

/// Plan options.
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct PlanReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Only this room.
    pub room: Option<usize>,
    /// Frame a rectangle of WORLD sectors. Omit to frame everything.
    #[serde(flatten)]
    pub cells: Option<PlanCells>,
    /// Pixels per sector, 8 to 96. Default 32.
    pub pixels_per_sector: Option<u32>,
    /// Draw numbered node markers (player spawn, lights, entities). Default true.
    pub nodes: Option<bool>,
    /// Draw the borders of the cells that are not whole sectors (violet 4
    /// sectors, blue 2, amber 1/2, red 1/4). Default true.
    pub leaves: Option<bool>,
}

/// The framing rectangle.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema)]
pub struct PlanCells {
    /// One corner, WORLD sector `[x, z]`.
    pub from: [i32; 2],
    /// The opposite corner, inclusive.
    pub to: Option<[i32; 2]>,
}

type Rgb = [u8; 3];

struct Canvas {
    w: u32,
    h: u32,
    px: Vec<u8>,
}

impl Canvas {
    fn new(w: u32, h: u32, fill: Rgb) -> Self {
        let mut px = Vec::with_capacity((w * h * 3) as usize);
        for _ in 0..w * h {
            px.extend_from_slice(&fill);
        }
        Self { w, h, px }
    }

    fn put(&mut self, x: i32, y: i32, c: Rgb) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return;
        }
        let i = ((y as u32 * self.w + x as u32) * 3) as usize;
        self.px[i..i + 3].copy_from_slice(&c);
    }

    fn blend(&mut self, x: i32, y: i32, c: Rgb, alpha: f32) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return;
        }
        let i = ((y as u32 * self.w + x as u32) * 3) as usize;
        for (k, channel) in c.iter().enumerate() {
            let old = f32::from(self.px[i + k]);
            self.px[i + k] = (old + (f32::from(*channel) - old) * alpha) as u8;
        }
    }

    fn rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgb) {
        for y in y0..y1 {
            for x in x0..x1 {
                self.put(x, y, c);
            }
        }
    }

    fn line(&mut self, a: (i32, i32), b: (i32, i32), c: Rgb, thick: i32) {
        let (dx, dy) = ((b.0 - a.0) as f32, (b.1 - a.1) as f32);
        let n = dx.abs().max(dy.abs()).max(1.0) as i32;
        for i in 0..=n {
            let t = i as f32 / n as f32;
            let (x, y) = (
                (a.0 as f32 + dx * t).round() as i32,
                (a.1 as f32 + dy * t).round() as i32,
            );
            for ox in 0..thick {
                for oy in 0..thick {
                    self.put(x + ox - thick / 2, y + oy - thick / 2, c);
                }
            }
        }
    }

    fn disc(&mut self, cx: i32, cy: i32, r: i32, c: Rgb) {
        for y in -r..=r {
            for x in -r..=r {
                if x * x + y * y <= r * r {
                    self.put(cx + x, cy + y, c);
                }
            }
        }
    }

    fn ring(&mut self, cx: i32, cy: i32, r: i32, c: Rgb) {
        let steps = (r * 8).max(16);
        for i in 0..steps {
            let a = i as f32 / steps as f32 * std::f32::consts::TAU;
            self.put(
                cx + (a.cos() * r as f32).round() as i32,
                cy + (a.sin() * r as f32).round() as i32,
                c,
            );
        }
    }

    fn fill_polygon(&mut self, pts: &[(f32, f32)], c: Rgb, alpha: f32) {
        let (minx, maxx) = pts
            .iter()
            .fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.0), b.max(p.0)));
        let (miny, maxy) = pts
            .iter()
            .fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.1), b.max(p.1)));
        for y in miny.floor() as i32..=maxy.ceil() as i32 {
            for x in minx.floor() as i32..=maxx.ceil() as i32 {
                let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                let mut inside = false;
                let mut j = pts.len() - 1;
                for i in 0..pts.len() {
                    let (pi, pj) = (pts[i], pts[j]);
                    if (pi.1 > fy) != (pj.1 > fy)
                        && fx < (pj.0 - pi.0) * (fy - pi.1) / (pj.1 - pi.1) + pi.0
                    {
                        inside = !inside;
                    }
                    j = i;
                }
                if inside {
                    self.blend(x, y, c, alpha);
                }
            }
        }
    }

    /// 3x5 digits and a minus sign, scaled.
    fn text(&mut self, x: i32, y: i32, s: &str, scale: i32, c: Rgb) {
        const GLYPHS: [(char, [u8; 5]); 11] = [
            ('0', [0b111, 0b101, 0b101, 0b101, 0b111]),
            ('1', [0b010, 0b110, 0b010, 0b010, 0b111]),
            ('2', [0b111, 0b001, 0b111, 0b100, 0b111]),
            ('3', [0b111, 0b001, 0b111, 0b001, 0b111]),
            ('4', [0b101, 0b101, 0b111, 0b001, 0b001]),
            ('5', [0b111, 0b100, 0b111, 0b001, 0b111]),
            ('6', [0b111, 0b100, 0b111, 0b101, 0b111]),
            ('7', [0b111, 0b001, 0b010, 0b010, 0b010]),
            ('8', [0b111, 0b101, 0b111, 0b101, 0b111]),
            ('9', [0b111, 0b101, 0b111, 0b001, 0b111]),
            ('-', [0b000, 0b000, 0b111, 0b000, 0b000]),
        ];
        let mut cx = x;
        for ch in s.chars() {
            if let Some((_, rows)) = GLYPHS.iter().find(|(g, _)| *g == ch) {
                for (ry, row) in rows.iter().enumerate() {
                    for rx in 0..3 {
                        if row & (0b100 >> rx) != 0 {
                            self.rect(
                                cx + rx * scale,
                                y + ry as i32 * scale,
                                cx + (rx + 1) * scale,
                                y + (ry as i32 + 1) * scale,
                                c,
                            );
                        }
                    }
                }
            }
            cx += 4 * scale;
        }
    }

    fn png(&self) -> Result<Vec<u8>, String> {
        let mut out = Vec::new();
        image::ImageEncoder::write_image(
            image::codecs::png::PngEncoder::new(&mut out),
            &self.px,
            self.w,
            self.h,
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|e| format!("encode PNG: {e}"))?;
        Ok(out)
    }
}

const ROOM_COLORS: [Rgb; 8] = [
    [255, 255, 255],
    [255, 120, 200],
    [120, 255, 160],
    [255, 255, 120],
    [120, 200, 255],
    [255, 170, 90],
    [200, 150, 255],
    [150, 255, 255],
];

fn height_colour(t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let low = [40.0, 90.0, 190.0];
    let mid = [70.0, 170.0, 120.0];
    let high = [245.0, 170.0, 50.0];
    let (a, b, u) = if t < 0.5 {
        (low, mid, t * 2.0)
    } else {
        (mid, high, (t - 0.5) * 2.0)
    };
    [
        (a[0] + (b[0] - a[0]) * u) as u8,
        (a[1] + (b[1] - a[1]) * u) as u8,
        (a[2] + (b[2] - a[2]) * u) as u8,
    ]
}

struct Marker {
    index: usize,
    name: String,
    kind: &'static str,
    pos: [f32; 3],
    colour: Rgb,
    radius_sectors: Option<f32>,
}

fn world_translation(scene: &psxed_project::Scene, id: NodeId) -> [f32; 3] {
    let mut sum = [0.0f32; 3];
    let mut at = Some(id);
    let mut hops = 0;
    while let Some(current) = at {
        let Some(node) = scene.node(current) else {
            break;
        };
        for (total, part) in sum.iter_mut().zip(node.transform.translation) {
            *total += part;
        }
        at = node.parent;
        hops += 1;
        if hops > 32 {
            break;
        }
    }
    sum
}

fn markers(project: &ProjectDocument, scene_index: usize, sector_editor: f32) -> Vec<Marker> {
    let scene = &project.scenes[scene_index];
    // A light's radius is in the World node's sectors; the plan wants grid
    // sectors.
    let unit = scene
        .world_sector_size_for_node(NodeId::ROOT)
        .unwrap_or(1024)
        .max(1) as f32;
    let mut out = Vec::new();
    for node in scene.nodes() {
        let (kind, colour, radius) = match &node.kind {
            NodeKind::SpawnPoint { player: true, .. } => ("player spawn", [60, 255, 90], None),
            NodeKind::SpawnPoint { .. } => ("spawn", [60, 220, 220], None),
            NodeKind::PointLight { radius, .. } => (
                "light",
                [255, 240, 80],
                Some(*radius * unit / sector_editor),
            ),
            NodeKind::Entity => {
                // An entity is a host; its character controller child says
                // whether it is the player.
                let controller =
                    node.children
                        .iter()
                        .find_map(|c| match scene.node(*c).map(|n| &n.kind) {
                            Some(NodeKind::CharacterController { player, .. }) => Some(*player),
                            _ => None,
                        });
                match controller {
                    Some(true) => ("player", [60, 255, 90], None),
                    Some(false) => ("enemy", [255, 70, 70], None),
                    None => ("entity", [255, 140, 140], None),
                }
            }
            NodeKind::PointOfInterest { .. } => ("point of interest", [255, 120, 255], None),
            NodeKind::Interactable { .. } => ("interactable", [255, 160, 60], None),
            NodeKind::Destructible { .. } => ("destructible", [200, 120, 60], None),
            _ => continue,
        };
        // Entities are hosts of component children; only list the host.
        if node.kind.is_component() {
            continue;
        }
        let pos = world_translation(scene, node.id);
        out.push(Marker {
            index: out.len() + 1,
            name: node.name.clone(),
            kind,
            pos,
            colour,
            radius_sectors: radius,
        });
    }
    out
}

fn edge_colour(kind: DerivedKind, hidden: bool, ghost: bool) -> Rgb {
    if hidden {
        return [70, 70, 90];
    }
    if ghost {
        return [255, 230, 80];
    }
    match kind {
        DerivedKind::Closure => [235, 235, 245],
        DerivedKind::Riser => [140, 210, 255],
        DerivedKind::Drop => [200, 140, 255],
    }
}

/// Draw the plan. Returns PNG bytes and a text legend.
pub fn render(
    project: &ProjectDocument,
    scene_index: usize,
    req: &PlanReq,
) -> Result<(Vec<u8>, String), String> {
    let world: &GridWorld = project.scenes[scene_index]
        .world
        .grid()
        .ok_or("the scene has no grid world")?;
    if world.rooms.is_empty() {
        return Err("the world has no rooms".to_string());
    }
    // Framing rectangle in world sectors.
    let mut lo = [i32::MAX; 2];
    let mut hi = [i32::MIN; 2];
    for (i, r) in world.rooms.iter().enumerate() {
        if req.room.is_some_and(|only| only != i) {
            continue;
        }
        lo = [lo[0].min(r.origin[0]), lo[1].min(r.origin[1])];
        hi = [
            hi[0].max(r.origin[0] + r.width as i32 - 1),
            hi[1].max(r.origin[1] + r.depth as i32 - 1),
        ];
    }
    if lo[0] > hi[0] {
        return Err("no such room".to_string());
    }
    let sector_editor = world.sector_editor_units() as f32;
    let mut frame = match &req.cells {
        Some(c) => {
            let cells = Cells {
                from: c.from,
                to: c.to,
            };
            let rect = cells.rect();
            check_rect(&rect)?;
            (rect.min, rect.max)
        }
        None => {
            // A margin of one sector so closure walls and markers show.
            ([lo[0] - 1, lo[1] - 1], [hi[0] + 1, hi[1] + 1])
        }
    };
    // Nodes outside the rooms still deserve to be on the map.
    let nodes_on = req.nodes.unwrap_or(true);
    let marks = if nodes_on {
        markers(project, scene_index, sector_editor)
    } else {
        Vec::new()
    };
    if req.cells.is_none() {
        for m in &marks {
            let s = [
                (m.pos[0] / sector_editor).floor() as i32,
                (m.pos[2] / sector_editor).floor() as i32,
            ];
            if (s[0] - frame.0[0]).abs() < 64 && (s[1] - frame.0[1]).abs() < 64 {
                frame.0 = [frame.0[0].min(s[0]), frame.0[1].min(s[1])];
                frame.1 = [frame.1[0].max(s[0]), frame.1[1].max(s[1])];
            }
        }
    }
    let (min, max) = frame;
    let cells_x = (max[0] - min[0] + 1).max(1) as u32;
    let cells_z = (max[1] - min[1] + 1).max(1) as u32;
    let mut ppx = req.pixels_per_sector.unwrap_or(32).clamp(8, 96);
    // Keep the image under 2400 px on a side.
    while (cells_x * ppx > 2400 || cells_z * ppx > 2400) && ppx > 4 {
        ppx -= 1;
    }
    let margin = 28u32;
    let (w, h) = (cells_x * ppx + margin + 4, cells_z * ppx + margin + 4);
    let mut canvas = Canvas::new(w, h, [18, 20, 26]);
    let origin = (margin as i32, margin as i32);
    let ppx_i = ppx as i32;
    let sx = |x: i32| origin.0 + (x - min[0]) * ppx_i;
    let sy = |z: i32| origin.1 + (z - min[1]) * ppx_i;
    let gx = |grid: f32| origin.0 as f32 + (grid / 1024.0 - min[0] as f32) * ppx as f32;
    let gz = |grid: f32| origin.1 as f32 + (grid / 1024.0 - min[1] as f32) * ppx as f32;

    // Grid lines and coordinate labels.
    let step = if ppx >= 48 {
        1
    } else if ppx >= 24 {
        2
    } else if ppx >= 12 {
        4
    } else {
        8
    };
    for x in min[0]..=max[0] + 1 {
        if x.rem_euclid(step) == 0 {
            canvas.rect(sx(x), origin.1, sx(x) + 1, h as i32, [34, 38, 48]);
            canvas.text(sx(x) + 2, 3, &x.to_string(), 1, [140, 150, 170]);
        }
    }
    for z in min[1]..=max[1] + 1 {
        if z.rem_euclid(step) == 0 {
            canvas.rect(origin.0, sy(z), w as i32, sy(z) + 1, [34, 38, 48]);
            canvas.text(2, sy(z) + 2, &z.to_string(), 1, [140, 150, 170]);
        }
    }

    // Floor colours by world height.
    let mut lowest = i32::MAX;
    let mut highest = i32::MIN;
    for (i, r) in world.rooms.iter().enumerate() {
        if req.room.is_some_and(|only| only != i) {
            continue;
        }
        for (_, s) in r.leaves() {
            if let Some(f) = s.ground_floor() {
                let (a, b) = f.height_range();
                lowest = lowest.min(r.y_offset + a);
                highest = highest.max(r.y_offset + b);
            }
        }
    }
    let span = (highest - lowest).max(1) as f32;
    let mut order: Vec<usize> = (0..world.rooms.len())
        .filter(|i| req.room.is_none_or(|only| only == *i))
        .collect();
    order.sort_by_key(|i| (world.rooms[*i].y_offset, *i));
    for &ri in &order {
        let r = &world.rooms[ri];
        for (key, s) in r.leaves() {
            if !r.contains_leaf(key) {
                continue;
            }
            let (lo, hi) = key.sector_range();
            if hi[0] < min[0] || lo[0] > max[0] || hi[1] < min[1] || lo[1] > max[1] {
                continue;
            }
            // Pixel position of a click (a quarter sector).
            let kx = |click: i32| {
                origin.0 + ((click * ppx_i) as f32 / 4.0).round() as i32 - min[0] * ppx_i
            };
            let kz = |click: i32| {
                origin.1 + ((click * ppx_i) as f32 / 4.0).round() as i32 - min[1] * ppx_i
            };
            let n = key.edge_clicks();
            let (x0, y0, x1, y1) = (kx(key.x), kz(key.z), kx(key.x + n), kz(key.z + n));
            let colour = match s.ground_floor() {
                Some(f) => {
                    let mean = r.y_offset as f32 + f.heights.iter().sum::<i32>() as f32 / 4.0;
                    height_colour((mean - lowest as f32) / span)
                }
                None => [28, 28, 34],
            };
            canvas.rect(x0 + 1, y0 + 1, x1, y1, colour);
            if let Some(f) = s.ground_floor() {
                // A sloped or bent floor gets its diagonal drawn.
                if !f.is_planar() || f.heights[0] != f.heights[2] || f.heights[1] != f.heights[3] {
                    let (a, b) = match f.split {
                        psxed_project::grid::GridSplit::NwSe => ((x0, y0), (x1, y1)),
                        psxed_project::grid::GridSplit::NeSw => ((x1, y0), (x0, y1)),
                    };
                    canvas.line(a, b, [0, 0, 0], 1);
                }
            }
            // Where the leaves are not whole sectors, their borders show.
            if req.leaves.unwrap_or(true) && !r.leaves.is_sectors_only() {
                let c = match key.level {
                    0 => [168, 124, 244],
                    1 => [104, 164, 244],
                    2 => [60, 64, 76],
                    3 => [255, 190, 84],
                    _ => [255, 108, 92],
                };
                canvas.line((x0, y0), (x1, y0), c, 1);
                canvas.line((x1, y0), (x1, y1), c, 1);
                canvas.line((x1, y1), (x0, y1), c, 1);
                canvas.line((x0, y1), (x0, y0), c, 1);
            }
        }
    }
    // Walls on top of every fill.
    for &ri in &order {
        let r = &world.rooms[ri];
        for z in (min[1] - 1)..=(max[1] + 1) {
            for x in (min[0] - 1)..=(max[0] + 1) {
                if !r.leaves.is_sectors_only() {
                    for seg in psxed_project::grid::leaf_derive::cell_wall_segments(r, [x, z]) {
                        let ghost = false;
                        let c = edge_colour(seg.kind, seg.hidden, ghost);
                        canvas.line(
                            (gx(seg.a[0] as f32) as i32, gz(seg.a[1] as f32) as i32),
                            (gx(seg.b[0] as f32) as i32, gz(seg.b[1] as f32) as i32),
                            c,
                            if seg.kind == DerivedKind::Closure {
                                3
                            } else {
                                2
                            },
                        );
                    }
                }
                for wall in derive::cell_walls(r, [x, z]) {
                    if !r.leaves.is_sectors_only() {
                        break;
                    }
                    let ghost = !wall.hidden
                        && wall.kind == DerivedKind::Closure
                        && wall.solid == Some(false);
                    let c = edge_colour(wall.kind, wall.hidden, ghost);
                    let (a, b) = wall.edge.endpoints();
                    canvas.line(
                        (sx(x + a[0]), sy(z + a[1])),
                        (sx(x + b[0]), sy(z + b[1])),
                        c,
                        if wall.kind == DerivedKind::Closure {
                            3
                        } else {
                            2
                        },
                    );
                }
                if r.local([x, z]).is_none() {
                    continue;
                }
                for (key, s) in r.leaves.in_sector([x, z]) {
                    if key.cell() != [x, z] || key.level < 2 {
                        continue;
                    }
                    let n = key.edge_clicks() as f32 / 4.0;
                    let at = |p: [i32; 2]| {
                        (
                            gx((key.x as f32 / 4.0 + p[0] as f32 * n) * 1024.0) as i32,
                            gz((key.z as f32 / 4.0 + p[1] as f32 * n) * 1024.0) as i32,
                        )
                    };
                    for edge in WallEdge::ALL {
                        if s.walls(edge).is_empty() {
                            continue;
                        }
                        let (a, b) = edge.endpoints();
                        canvas.line(at(a), at(b), [255, 140, 40], 2);
                    }
                }
            }
        }
    }
    // Room outlines and numbers.
    for &ri in &order {
        let r = &world.rooms[ri];
        let c = ROOM_COLORS[ri % ROOM_COLORS.len()];
        let (x0, z0) = (r.origin[0], r.origin[1]);
        let (x1, z1) = (x0 + r.width as i32, z0 + r.depth as i32);
        canvas.line((sx(x0), sy(z0)), (sx(x1), sy(z0)), c, 1);
        canvas.line((sx(x1), sy(z0)), (sx(x1), sy(z1)), c, 1);
        canvas.line((sx(x1), sy(z1)), (sx(x0), sy(z1)), c, 1);
        canvas.line((sx(x0), sy(z1)), (sx(x0), sy(z0)), c, 1);
        canvas.text(sx(x0) + 3, sy(z0) + 3, &ri.to_string(), 2, c);
    }
    // Planes: footprint polygon, coloured by collision class.
    let mut plane_lines = String::new();
    let mut mesh_lines = String::new();
    for &ri in &order {
        let r = &world.rooms[ri];
        for (pi, plane) in r.planes.iter().enumerate() {
            let c = match (plane.enabled, plane.collision) {
                (false, _) => [90, 90, 90],
                (_, PlaneCollision::Walkable) => [80, 255, 120],
                (_, PlaneCollision::Solid) => [255, 80, 80],
                (_, PlaneCollision::Decor) => [200, 200, 255],
            };
            let pts: Vec<(f32, f32)> = plane
                .corners()
                .iter()
                .map(|p| (gx(p[0] as f32), gz(p[2] as f32)))
                .collect();
            canvas.fill_polygon(&pts, c, 0.45);
            for k in 0..4 {
                let (a, b) = (pts[k], pts[(k + 1) % 4]);
                canvas.line((a.0 as i32, a.1 as i32), (b.0 as i32, b.1 as i32), c, 2);
            }
            let cx = pts.iter().map(|p| p.0).sum::<f32>() / 4.0;
            let cy = pts.iter().map(|p| p.1).sum::<f32>() / 4.0;
            canvas.text(
                cx as i32 - 3,
                cy as i32 - 4,
                &format!("p{pi}").replace('p', ""),
                2,
                [0, 0, 0],
            );
            let _ = writeln!(
                plane_lines,
                "plane {pi} (room {ri}): {:?} {:?}, centre {:?}",
                plane.collision, plane.size, plane.center
            );
        }
    }
    // Static meshes: the placed footprint in orange (crown sockets as a dot),
    // numbered by stable id.
    for &ri in &order {
        let r = &world.rooms[ri];
        for mesh in &r.meshes {
            let Some(placed) = psxed_project::grid::mesh_faces::placed(
                world,
                ri,
                r.meshes.iter().position(|m| m.id == mesh.id).unwrap_or(0),
            ) else {
                continue;
            };
            let Some(foot) = mesh.footprint(&placed.built) else {
                continue;
            };
            let c = if mesh.collide {
                [255, 170, 60]
            } else {
                [200, 160, 110]
            };
            let pts: Vec<(f32, f32)> = foot
                .iter()
                .map(|p| (gx(p[0] as f32), gz(p[1] as f32)))
                .collect();
            canvas.fill_polygon(&pts, c, 0.35);
            for k in 0..4 {
                let (a, b) = (pts[k], pts[(k + 1) % 4]);
                canvas.line((a.0 as i32, a.1 as i32), (b.0 as i32, b.1 as i32), c, 2);
            }
            let cx = pts.iter().map(|p| p.0).sum::<f32>() / 4.0;
            let cy = pts.iter().map(|p| p.1).sum::<f32>() / 4.0;
            canvas.text(
                cx as i32 - 3,
                cy as i32 - 4,
                &mesh.id.to_string(),
                2,
                [0, 0, 0],
            );
            for socket in &placed.built.sockets {
                let psxed_project::grid::mesh::Socket::Hook { anchor, .. } = *socket;
                let g = mesh.place(anchor);
                canvas.ring(
                    gx(g[0] as f32) as i32,
                    gz(g[2] as f32) as i32,
                    3,
                    [255, 255, 255],
                );
            }
            let _ = writeln!(
                mesh_lines,
                "mesh {} (room {ri}): {} at {:?}, rotation {:?}",
                mesh.id,
                world
                    .mesh_assets
                    .get(usize::from(mesh.asset))
                    .map_or("?", |a| a.name.as_str()),
                mesh.position,
                mesh.rotation
            );
        }
    }
    // Nodes.
    let mut legend = String::new();
    let mut draw_order: Vec<&Marker> = marks.iter().collect();
    draw_order.sort_by_key(|m| m.radius_sectors.is_none());
    for m in draw_order {
        let (px, py) = (
            gx(world.scale.to_grid(m.pos[0] as i32) as f32),
            gz(world.scale.to_grid(m.pos[2] as i32) as f32),
        );
        if let Some(radius) = m.radius_sectors {
            // Already converted to grid sectors by `markers`.
            canvas.ring(
                px as i32,
                py as i32,
                (radius * ppx as f32).round() as i32,
                [150, 140, 50],
            );
        }
        canvas.disc(px as i32, py as i32, 5, [0, 0, 0]);
        canvas.disc(px as i32, py as i32, 4, m.colour);
        canvas.text(
            px as i32 + 6,
            py as i32 - 3,
            &m.index.to_string(),
            1,
            [255, 255, 255],
        );
    }

    for m in &marks {
        let _ = writeln!(
            legend,
            "{}: {} {:?} at editor [{:.0}, {:.0}, {:.0}]",
            m.index, m.kind, m.name, m.pos[0], m.pos[1], m.pos[2]
        );
    }

    let mut text = format!(
        "plan of sectors x {}..{}, z {}..{}, {} px per sector; north (-Z) is up, east (+X) is right\n",
        min[0], max[0], min[1], max[1], ppx
    );
    if lowest <= highest {
        let _ = writeln!(
            text,
            "floor colour: blue = {} clicks (lowest) to orange = {} clicks (highest), world height; a black diagonal marks a sloped or bent floor",
            clicks(lowest),
            clicks(highest)
        );
    }
    text.push_str(
        "wall lines: white = closure (room edge), light blue = riser, purple = drop, yellow = ghost closure, dim = hidden, orange = authored wall\n\
         room outlines are numbered; planes: green = walkable, red = solid, lavender = decor; meshes: orange footprint numbered by id, white ring = hook socket\n",
    );
    if !mesh_lines.is_empty() {
        text.push_str(&mesh_lines);
    }
    if !plane_lines.is_empty() {
        text.push_str(&plane_lines);
    }
    if !legend.is_empty() {
        text.push_str(
            "markers (green = player, yellow = light with its reach ringed, red = enemy):\n",
        );
        text.push_str(&legend);
    }
    Ok((canvas.png()?, text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_starter_plan_is_a_png_with_a_legend() {
        let project = crate::grid::test_support::new_grid_project();
        let (png, text) = render(&project, 0, &PlanReq::default()).unwrap();
        assert!(png.starts_with(&[0x89, b'P', b'N', b'G']));
        assert!(text.contains("player"), "{text}");
        assert!(text.contains("north (-Z) is up"), "{text}");
    }
}
