//! Lab for the world-space far impostors: clusters of the far level baked from the lattice's
//! eyes, judged against the level itself on a played route.
//!
//! Usage:
//!   `bb_lab <project.ron> info`
//!   `bb_lab <project.ron> eval <trace.log> <out dir> [every=N] [from=FRAME] [cell=SECTORS]
//!       [ftex=TEXELS] [excl=SECTORS] [worst=N] [mesh=0|1] [visible=0|1]`
//!
//! `trace.log` is a `card-trace` run's stderr (its `pt Q` lines give the camera of every frame).
//! For each sampled frame the level is rendered as the truth; the far view is then drawn from the
//! impostors of the nearest eye, the rooms the camera draws whole go over it, and the result is
//! compared with the truth on the pixels the far view owns.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use image::{Rgb, RgbImage};
use psxed_project::grid::cook::panels::{Camera, World};
use psxed_project::playtest::{cook_for_cards, panel_eyes, panel_textures};
use psxed_project::ProjectDocument;

const SKY: [f32; 3] = [50.0, 58.0, 56.0];
const FOCAL: f32 = 305.0;
const CELL_W: usize = 32;
const CELL_H: usize = 21;

fn arg(args: &[String], key: &str) -> Option<f32> {
    args.iter()
        .find_map(|a| a.strip_prefix(key).and_then(|v| v.strip_prefix('=')))
        .and_then(|v| v.parse().ok())
}

struct Pose {
    frame: u32,
    eye: [f32; 3],
    rot: [[i16; 3]; 3],
}

fn parse_poses(path: &Path) -> Vec<Pose> {
    let text = std::fs::read_to_string(path).expect("read the trace");
    let mut out = Vec::new();
    for line in text.lines() {
        let Some(at) = line.find("pt Q ") else {
            continue;
        };
        let frame: u32 = line
            .split("[guest f")
            .nth(1)
            .and_then(|s| s.split(' ').next())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let raw: Vec<i64> = line[at + 5..]
            .split_whitespace()
            .filter_map(|n| n.parse::<i64>().ok())
            .collect();
        if raw.len() < 13 {
            continue;
        }
        let v: Vec<i64> = raw[..12].iter().map(|n| n - 32768).collect();
        out.push(Pose {
            frame,
            eye: [v[0] as f32, v[1] as f32, v[2] as f32],
            rot: [
                [v[3] as i16, v[4] as i16, v[5] as i16],
                [v[6] as i16, v[7] as i16, v[8] as i16],
                [v[9] as i16, v[10] as i16, v[11] as i16],
            ],
        });
    }
    out
}

/// A cluster of the far level: the triangles whose centre lies in one cell of a plan grid.
struct Cluster {
    key: (i32, i32),
    lo: [f32; 3],
    hi: [f32; 3],
    /// The rooms with triangles in it.
    rooms: Vec<usize>,
}

impl Cluster {
    fn anchor(&self) -> [f32; 3] {
        [0, 1, 2].map(|k| (self.lo[k] + self.hi[k]) / 2.0)
    }
}

struct Setup {
    world: World,
    sector: f32,
    eyes: Vec<[f32; 3]>,
    room_box: Vec<Option<([f32; 3], [f32; 3])>>,
    clusters: Vec<Cluster>,
    cell: f32,
    ftex: f32,
    excl: f32,
    visible_only: bool,
}

fn centroid(t: &[[f32; 3]; 3]) -> [f32; 3] {
    [0, 1, 2].map(|k| (t[0][k] + t[1][k] + t[2][k]) / 3.0)
}

impl Setup {
    fn key_of(&self, t: &[[f32; 3]; 3]) -> (i32, i32) {
        let c = centroid(t);
        (
            (c[0] / (self.cell * self.sector)).floor() as i32,
            (c[2] / (self.cell * self.sector)).floor() as i32,
        )
    }

    fn find_clusters(&mut self) {
        let mut map: HashMap<(i32, i32), Cluster> = HashMap::new();
        for room in 0..self.world.rooms() {
            let tris: Vec<[[f32; 3]; 3]> = self.world.room_triangles(room).copied().collect();
            for t in &tris {
                let key = self.key_of(t);
                let c = map.entry(key).or_insert(Cluster {
                    key,
                    lo: [f32::MAX; 3],
                    hi: [f32::MIN; 3],
                    rooms: Vec::new(),
                });
                for p in t {
                    for k in 0..3 {
                        c.lo[k] = c.lo[k].min(p[k]);
                        c.hi[k] = c.hi[k].max(p[k]);
                    }
                }
                if !c.rooms.contains(&room) {
                    c.rooms.push(room);
                }
            }
        }
        let mut v: Vec<Cluster> = map.into_values().collect();
        v.sort_by_key(|c| c.key);
        self.clusters = v;
    }

    /// Rooms the camera could be drawing whole from anywhere in the eye's cell: the eye's pictures
    /// leave them out (the cook's flood, here by plan distance to the room's box).
    fn excluded(&self, eye: [f32; 3]) -> Vec<bool> {
        let r = self.excl * self.sector;
        self.room_box
            .iter()
            .map(|b| {
                b.is_some_and(|(lo, hi)| {
                    let dx = (lo[0] - eye[0]).max(0.0).max(eye[0] - hi[0]);
                    let dz = (lo[2] - eye[2]).max(0.0).max(eye[2] - hi[2]);
                    (dx * dx + dz * dz).sqrt() <= r
                })
            })
            .collect()
    }
}

/// One baked picture of a cluster from an eye.
struct Pic {
    /// Where the optical axis meets the picture's plane.
    q: [f32; 3],
    /// Distance of the plane along the aim.
    d: f32,
    /// Pixels the crop's top left lies from the principal point.
    u0: i32,
    v0: i32,
    w: usize,
    h: usize,
    rgb: Vec<[f32; 3]>,
    cov: Vec<bool>,
    /// The eye and the aim (plan) the picture was baked along.
    eye: [f32; 3],
    fwd: [f32; 2],
    /// Depth along the aim at each vertex of a mesh of `CELL_W` by `CELL_H` texel cells.
    mesh: Vec<f32>,
    mesh_w: usize,
    mesh_h: usize,
}

impl Pic {
    fn covered(&self) -> usize {
        self.cov.iter().filter(|&&c| c).count()
    }

    /// Mesh cells holding any covered texel.
    fn cells(&self) -> usize {
        let mut n = 0;
        for cj in 0..self.mesh_h - 1 {
            for ci in 0..self.mesh_w - 1 {
                let mut any = false;
                for y in cj * CELL_H..((cj + 1) * CELL_H).min(self.h) {
                    for x in ci * CELL_W..((ci + 1) * CELL_W).min(self.w) {
                        any |= self.cov[y * self.w + x];
                    }
                }
                n += usize::from(any);
            }
        }
        n
    }

    /// Tiles of 64 by 63 texels (from the crop's corner) holding any covered texel.
    fn tiles(&self) -> usize {
        let (tw, th) = (self.w.div_ceil(64), self.h.div_ceil(63));
        let mut n = 0;
        for ty in 0..th {
            for tx in 0..tw {
                let mut any = false;
                for y in ty * 63..((ty + 1) * 63).min(self.h) {
                    for x in tx * 64..((tx + 1) * 64).min(self.w) {
                        any |= self.cov[y * self.w + x];
                    }
                }
                n += usize::from(any);
            }
        }
        n
    }
}

fn aim_camera(
    eye: [f32; 3],
    fwd: [f32; 2],
    focal: f32,
    x0: f32,
    y0: f32,
    w: usize,
    h: usize,
) -> Camera {
    let right = [-fwd[1], 0.0, fwd[0]];
    Camera {
        eye,
        rows: [right, [0.0, -1.0, 0.0], [fwd[0], 0.0, fwd[1]]],
        focal,
        width: w,
        height: h,
        cx: -x0,
        cy: -y0,
    }
}

fn bake(setup: &Setup, eye: [f32; 3], cluster: &Cluster, excluded: &[bool]) -> Option<Pic> {
    let a = cluster.anchor();
    let (dx, dz) = (a[0] - eye[0], a[2] - eye[2]);
    let len = (dx * dx + dz * dz).sqrt();
    if len < setup.sector * 0.5 {
        return None;
    }
    let fwd = [dx / len, dz / len];
    let probe = aim_camera(eye, fwd, setup.ftex, 0.0, 0.0, 1, 1);
    let near = setup.sector * 0.25;
    let (mut x0, mut x1, mut y0, mut y1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    let rooms: Vec<usize> = cluster
        .rooms
        .iter()
        .copied()
        .filter(|&r| !excluded[r])
        .collect();
    let mut any = false;
    for &room in &rooms {
        for t in setup.world.room_triangles(room) {
            if setup.key_of(t) != cluster.key {
                continue;
            }
            for p in t {
                let c = probe.to_camera(*p);
                if c[2] < near {
                    continue;
                }
                any = true;
                let px = setup.ftex * c[0] / c[2];
                let py = setup.ftex * c[1] / c[2];
                x0 = x0.min(px);
                x1 = x1.max(px);
                y0 = y0.min(py);
                y1 = y1.max(py);
            }
        }
    }
    if !any {
        return None;
    }
    let (x0, y0) = (x0.floor() - 1.0, y0.floor() - 1.0);
    let w = ((x1.ceil() + 1.0 - x0) as usize).clamp(2, 700);
    let h = ((y1.ceil() + 1.0 - y0) as usize).clamp(2, 700);
    let cam = aim_camera(eye, fwd, setup.ftex, x0, y0, w, h);
    let key = cluster.key;
    let alone = setup
        .world
        .render_with(&cam, 2, &|r| !excluded[r], &|t| setup.key_of(t) == key)
        .resolve(2);
    let all = setup
        .visible_only
        .then(|| setup.world.render(&cam, 2, &|r| !excluded[r]).resolve(2));
    // A texel counts when the cluster has something there (and, when only what the eye sees is
    // kept, nothing nearer hides it).
    let kept = |at: usize| -> bool {
        alone.coverage[at] > 0.0
            && all
                .as_ref()
                .is_none_or(|all| alone.depth[at] <= all.depth[at] * 1.03)
    };
    let (mut bx0, mut bx1, mut by0, mut by1) = (w, 0usize, h, 0usize);
    let mut depths = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let at = y * w + x;
            if kept(at) {
                bx0 = bx0.min(x);
                bx1 = bx1.max(x);
                by0 = by0.min(y);
                by1 = by1.max(y);
                depths.push(alone.depth[at]);
            }
        }
    }
    if depths.is_empty() {
        return None;
    }
    depths.sort_by(f32::total_cmp);
    let d = depths[depths.len() / 2];
    let (cw, ch) = (bx1 - bx0 + 1, by1 - by0 + 1);
    let mut rgb = Vec::with_capacity(cw * ch);
    let mut cov = Vec::with_capacity(cw * ch);
    let mut zs_all = Vec::with_capacity(cw * ch);
    for y in by0..=by1 {
        for x in bx0..=bx1 {
            let at = y * w + x;
            rgb.push(alone.rgb[at]);
            cov.push(kept(at));
            zs_all.push(alone.depth[at]);
        }
    }
    // The depth mesh: at each vertex the median depth of the covered samples within half a cell,
    // holes filled from the neighbours.
    let (mw, mh) = (cw.div_ceil(CELL_W) + 1, ch.div_ceil(CELL_H) + 1);
    let mut mesh = vec![f32::NAN; mw * mh];
    for j in 0..mh {
        for i in 0..mw {
            let (cx, cy) = ((i * CELL_W) as isize, (j * CELL_H) as isize);
            let mut zs = Vec::new();
            for y in (cy - CELL_H as isize / 2).max(0)..(cy + CELL_H as isize / 2).min(ch as isize)
            {
                for x in
                    (cx - CELL_W as isize / 2).max(0)..(cx + CELL_W as isize / 2).min(cw as isize)
                {
                    let at = y as usize * cw + x as usize;
                    if cov[at] {
                        zs.push(zs_all[at]);
                    }
                }
            }
            if !zs.is_empty() {
                zs.sort_by(f32::total_cmp);
                mesh[j * mw + i] = zs[zs.len() / 2];
            }
        }
    }
    for _ in 0..6 {
        let snap = mesh.clone();
        for j in 0..mh {
            for i in 0..mw {
                if !snap[j * mw + i].is_nan() {
                    continue;
                }
                let (mut sum, mut n) = (0.0f32, 0);
                for (di, dj) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                    let (a, b) = (i as i32 + di, j as i32 + dj);
                    if a < 0 || b < 0 || a >= mw as i32 || b >= mh as i32 {
                        continue;
                    }
                    let v = snap[b as usize * mw + a as usize];
                    if !v.is_nan() {
                        sum += v;
                        n += 1;
                    }
                }
                if n > 0 {
                    mesh[j * mw + i] = sum / n as f32;
                }
            }
        }
    }
    for z in &mut mesh {
        if z.is_nan() {
            *z = d;
        }
    }
    Some(Pic {
        q: [eye[0] + d * fwd[0], eye[1], eye[2] + d * fwd[1]],
        d,
        u0: (x0 as i32) + bx0 as i32,
        v0: (y0 as i32) + by0 as i32,
        w: cw,
        h: ch,
        rgb,
        cov,
        eye,
        fwd,
        mesh,
        mesh_w: mw,
        mesh_h: mh,
    })
}

/// A screen layer the impostors are drawn into.
struct Layer {
    rgb: Vec<[f32; 3]>,
    covered: Vec<bool>,
}

/// Draw `pic` as a vertical quad through `pic.q`, turned to face `cam`; the caller orders them.
fn draw_flat(layer: &mut Layer, cam: &Camera, pic: &Pic, ftex: f32) {
    let q = pic.q;
    let (gx, gz) = (q[0] - cam.eye[0], q[2] - cam.eye[2]);
    let glen = (gx * gx + gz * gz).sqrt();
    if glen < 1.0 {
        return;
    }
    let g = [gx / glen, gz / glen];
    let r = [-g[1], g[0]];
    let scale = pic.d / ftex;
    let along = (q[0] - cam.eye[0]) * g[0] + (q[2] - cam.eye[2]) * g[1];
    for y in 0..cam.height {
        for x in 0..cam.width {
            let (sx, sy) = (
                (x as f32 + 0.5 - cam.cx) / cam.focal,
                (y as f32 + 0.5 - cam.cy) / cam.focal,
            );
            let ray = [0, 1, 2].map(|k| cam.rows[0][k] * sx + cam.rows[1][k] * sy + cam.rows[2][k]);
            let den = ray[0] * g[0] + ray[2] * g[1];
            if den <= 1e-6 {
                continue;
            }
            let t = along / den;
            if t <= 0.0 {
                continue;
            }
            let hit = [
                cam.eye[0] + t * ray[0],
                cam.eye[1] + t * ray[1],
                cam.eye[2] + t * ray[2],
            ];
            let a = (hit[0] - q[0]) * r[0] + (hit[2] - q[2]) * r[1];
            let b = hit[1] - q[1];
            let u = a / scale - pic.u0 as f32;
            let v = -b / scale - pic.v0 as f32;
            if u < 0.0 || v < 0.0 {
                continue;
            }
            let (iu, iv) = (u as usize, v as usize);
            if iu >= pic.w || iv >= pic.h {
                continue;
            }
            let at = iv * pic.w + iu;
            if pic.cov[at] {
                let o = y * cam.width + x;
                layer.rgb[o] = pic.rgb[at];
                layer.covered[o] = true;
            }
        }
    }
}

/// One mesh cell of a picture, ready to fill.
struct CellDraw<'a> {
    z: f32,
    screen: [[f32; 2]; 4],
    /// Texel coordinates in the picture's crop, per corner.
    uv: [[f32; 2]; 4],
    pic: &'a Pic,
}

/// The cells of `pic` that show anything, projected for `cam`; cells with a corner behind the near
/// plane are counted in `dropped`.
fn collect_cells<'a>(
    out: &mut Vec<CellDraw<'a>>,
    cam: &Camera,
    pic: &'a Pic,
    ftex: f32,
    dropped: &mut usize,
) {
    let right = [-pic.fwd[1], 0.0, pic.fwd[0]];
    let vertex = |i: usize, j: usize| -> [f32; 3] {
        let z = pic.mesh[j * pic.mesh_w + i];
        let u = (pic.u0 + (i * CELL_W).min(pic.w) as i32) as f32;
        let v = (pic.v0 + (j * CELL_H).min(pic.h) as i32) as f32;
        let (a, b) = (u / ftex, v / ftex);
        [
            pic.eye[0] + z * (pic.fwd[0] + a * right[0]),
            pic.eye[1] - z * b,
            pic.eye[2] + z * (pic.fwd[1] + a * right[2]),
        ]
    };
    let near = 8.0;
    for cj in 0..pic.mesh_h - 1 {
        for ci in 0..pic.mesh_w - 1 {
            let (x0, y0) = (ci * CELL_W, cj * CELL_H);
            let (x1, y1) = (((ci + 1) * CELL_W).min(pic.w), ((cj + 1) * CELL_H).min(pic.h));
            let mut any = false;
            'scan: for y in y0..y1 {
                for x in x0..x1 {
                    if pic.cov[y * pic.w + x] {
                        any = true;
                        break 'scan;
                    }
                }
            }
            if !any {
                continue;
            }
            let corners = [(ci, cj), (ci + 1, cj), (ci + 1, cj + 1), (ci, cj + 1)];
            let cams: Vec<[f32; 3]> = corners
                .iter()
                .map(|&(a, b)| cam.to_camera(vertex(a, b)))
                .collect();
            if cams.iter().any(|c| c[2] < near) {
                *dropped += 1;
                continue;
            }
            let screen = [0, 1, 2, 3].map(|k| cam.pixel(cams[k]));
            let (mut lx, mut hx, mut ly, mut hy) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
            for p in &screen {
                lx = lx.min(p[0]);
                hx = hx.max(p[0]);
                ly = ly.min(p[1]);
                hy = hy.max(p[1]);
            }
            if hx < 0.0 || hy < 0.0 || lx > cam.width as f32 || ly > cam.height as f32 {
                continue;
            }
            let z = (cams[0][2] + cams[1][2] + cams[2][2] + cams[3][2]) / 4.0;
            let (fx0, fx1, fy0, fy1) = (x0 as f32, x1 as f32, y0 as f32, y1 as f32);
            out.push(CellDraw {
                z,
                screen,
                uv: [[fx0, fy0], [fx1, fy0], [fx1, fy1], [fx0, fy1]],
                pic,
            });
        }
    }
}

/// Fill a screen triangle with texels of a picture, texel coordinates linear in screen space.
fn fill_cell(layer: &mut Layer, width: usize, p: [[f32; 2]; 3], uv: [[f32; 2]; 3], pic: &Pic) {
    let edge = |a: [f32; 2], b: [f32; 2], x: f32, y: f32| {
        (b[0] - a[0]) * (y - a[1]) - (b[1] - a[1]) * (x - a[0])
    };
    let area = edge(p[0], p[1], p[2][0], p[2][1]);
    if area.abs() < 1e-9 {
        return;
    }
    let lo = |f: fn(f32, f32) -> f32, k: usize| f(f(p[0][k], p[1][k]), p[2][k]);
    let height = layer.covered.len() / width;
    let x0 = (lo(f32::min, 0) - 0.5).floor().max(0.0) as usize;
    let x1 =
        ((lo(f32::max, 0) - 0.5).ceil().max(-1.0) as isize + 1).clamp(0, width as isize) as usize;
    let y0 = (lo(f32::min, 1) - 0.5).floor().max(0.0) as usize;
    let y1 =
        ((lo(f32::max, 1) - 0.5).ceil().max(-1.0) as isize + 1).clamp(0, height as isize) as usize;
    for y in y0..y1 {
        let sy = y as f32 + 0.5;
        for x in x0..x1 {
            let sx = x as f32 + 0.5;
            let w0 = edge(p[1], p[2], sx, sy) / area;
            let w1 = edge(p[2], p[0], sx, sy) / area;
            let w2 = 1.0 - w0 - w1;
            let eps = -1e-4;
            if w0 < eps || w1 < eps || w2 < eps {
                continue;
            }
            let u = w0 * uv[0][0] + w1 * uv[1][0] + w2 * uv[2][0];
            let v = w0 * uv[0][1] + w1 * uv[1][1] + w2 * uv[2][1];
            let (tu, tv) = (
                (u.floor() as isize).clamp(0, pic.w as isize - 1) as usize,
                (v.floor() as isize).clamp(0, pic.h as isize - 1) as usize,
            );
            let at = tv * pic.w + tu;
            if pic.cov[at] {
                let o = y * width + x;
                layer.rgb[o] = pic.rgb[at];
                layer.covered[o] = true;
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = Path::new(&args[0]);
    let text = std::fs::read_to_string(path).expect("read the project");
    let project = ProjectDocument::from_ron_str(&text).expect("parse the project");
    let root = path
        .parent()
        .map_or_else(|| PathBuf::from("."), PathBuf::from);
    let scene = project.active_scene();
    let grid = scene.world.grid().expect("a grid world");
    let inputs = cook_for_cards(&project, grid, &root).expect("cook the level");
    let (cooked, albedo) = (inputs.cooked, inputs.albedo);
    let sector = cooked.level.sector_units as f32;
    let world = World::new(
        &cooked,
        &albedo,
        Some(panel_textures(&project, &root, &cooked)),
    );
    let mut scaled = project.clone();
    psxed_project::units::scale_project_to_engine_units(&mut scaled);
    let sgrid = scaled.active_scene().world.grid().expect("a grid world");
    let eyes: Vec<[f32; 3]> = panel_eyes(&scaled, sgrid, &cooked, 10)
        .iter()
        .map(|e| e.eye)
        .collect();
    let room_box = (0..world.rooms()).map(|r| world.room_bounds(r)).collect();
    let mut setup = Setup {
        world,
        sector,
        eyes,
        room_box,
        clusters: Vec::new(),
        cell: arg(&args, "cell").unwrap_or(10.0),
        ftex: arg(&args, "ftex").unwrap_or(122.0),
        excl: arg(&args, "excl").unwrap_or(3.44),
        visible_only: arg(&args, "visible").unwrap_or(0.0) > 0.5,
    };
    setup.find_clusters();
    println!(
        "sector {sector} rooms {} eyes {} clusters {} (cell {} sectors)",
        setup.world.rooms(),
        setup.eyes.len(),
        setup.clusters.len(),
        setup.cell
    );
    match args.get(1).map(String::as_str) {
        Some("info") => {
            for (i, c) in setup.clusters.iter().enumerate() {
                println!(
                    "cluster {i:3} key {:?} rooms {:?} x {:.1}..{:.1} y {:.1}..{:.1} z {:.1}..{:.1}",
                    c.key,
                    c.rooms,
                    c.lo[0] / sector,
                    c.hi[0] / sector,
                    c.lo[1] / sector,
                    c.hi[1] / sector,
                    c.lo[2] / sector,
                    c.hi[2] / sector
                );
            }
        }
        Some("walk") => {
            use psxed_project::grid::walk::{analyse, player_spawns};
            let mut scaled = project.clone();
            psxed_project::units::scale_project_to_engine_units(&mut scaled);
            let sgrid = scaled.active_scene().world.grid().expect("a grid world");
            let spawns = player_spawns(&scaled, sgrid);
            let analysis = analyse(sgrid, &spawns, &psx_engine::WalkLimits::DEFAULT).expect("walk");
            for (ri, room) in sgrid.rooms.iter().enumerate() {
                let (mut floors, mut stand, mut unreach, mut ghost, mut steep, mut low) = (0, 0, 0, 0, 0, 0);
                for (lx, lz, sec) in room.iter() {
                    floors += usize::from(sec.ground_floor().is_some());
                    if let Some(w) = analysis.rooms.get(ri).and_then(|r| r.cell([room.origin[0] + lx as i32, room.origin[1] + lz as i32])) {
                        stand += usize::from(w.standable);
                        unreach += usize::from(w.unreachable);
                        ghost += usize::from(w.ghost);
                        steep += usize::from(w.steep);
                        low += usize::from(w.low_headroom);
                    }
                }
                println!("room {ri:3} floors {floors:4} standable {stand:4} unreachable {unreach:4} ghost {ghost:4} steep {steep:4} low {low:4}");
            }
        }
        Some("eyes") => {
            for (i, e) in setup.eyes.iter().enumerate() {
                println!("eye {i:3} at ({:6.1}, {:5.1}, {:6.1})", e[0] / sector, e[1] / sector, e[2] / sector);
            }
        }
        Some("eval") => eval(&setup, &args),
        _ => eprintln!("usage: bb_lab <project.ron> info|eval ..."),
    }
}

fn eval(setup: &Setup, args: &[String]) {
    let sector = setup.sector;
    let poses = {
        let mut p = parse_poses(Path::new(&args[2]));
        let from = arg(args, "from").unwrap_or(0.0) as u32;
        p.retain(|q| q.frame >= from);
        p
    };
    let out = PathBuf::from(&args[3]);
    std::fs::create_dir_all(&out).expect("out dir");
    let every = arg(args, "every").unwrap_or(12.0) as usize;
    let worst = arg(args, "worst").unwrap_or(10.0) as usize;
    let real_r = arg(args, "real").unwrap_or(12.0) * sector;
    let mesh = arg(args, "mesh").unwrap_or(1.0) > 0.5;
    println!("poses {} mesh {mesh} visible {}", poses.len(), setup.visible_only);
    let mut cache: HashMap<usize, Vec<Option<Pic>>> = HashMap::new();
    let mut results: Vec<(f64, u32, RgbImage)> = Vec::new();
    let (mut t_seen, mut t_holes, mut t_extra, mut t_err) = (0usize, 0usize, 0usize, 0.0f64);
    let mut buckets = [(0usize, 0usize, 0.0f64); 5];
    let mut near_dropped = 0usize;
    let mut data = (0usize, 0usize, 0usize, 0usize);
    for pose in poses.iter().step_by(every.max(1)) {
        let eye_idx = (0..setup.eyes.len())
            .min_by(|&a, &b| {
                let d = |i: usize| {
                    (0..3)
                        .map(|k| (setup.eyes[i][k] - pose.eye[k]).powi(2))
                        .sum::<f32>()
                };
                d(a).total_cmp(&d(b))
            })
            .expect("an eye");
        let fresh = !cache.contains_key(&eye_idx);
        let pics = cache.entry(eye_idx).or_insert_with(|| {
            let ex = setup.excluded(setup.eyes[eye_idx]);
            setup
                .clusters
                .iter()
                .map(|c| bake(setup, setup.eyes[eye_idx], c, &ex))
                .collect()
        });
        if fresh {
            let n = pics.iter().flatten().count();
            let texels: usize = pics.iter().flatten().map(Pic::covered).sum();
            let tiles: usize = pics.iter().flatten().map(Pic::tiles).sum();
            let rect: usize = pics.iter().flatten().map(|p| p.w * p.h).sum();
            let cells: usize = pics.iter().flatten().map(Pic::cells).sum();
            println!(
                "  baked eye {eye_idx}: {n} pictures, {texels} covered texels, {rect} in rects, {tiles} tiles, {cells} cells"
            );
            data = (data.0 + 1, data.1 + texels, data.2 + tiles, data.3 + rect);
        }
        let cam = Camera::game(pose.eye, pose.rot, FOCAL);
        let truth = setup.world.render(&cam, 2, &|_| true).resolve(2);
        let near = |room: usize| {
            setup.world.centres[room].is_some_and(|c| {
                ((c[0] - pose.eye[0]).powi(2) + (c[1] - pose.eye[2]).powi(2)).sqrt() < real_r
            })
        };
        let real = setup.world.render(&cam, 2, &near).resolve(2);
        let mut layer = Layer {
            rgb: vec![[0.0; 3]; 320 * 240],
            covered: vec![false; 320 * 240],
        };
        if mesh {
            let mut cells = Vec::new();
            for p in pics.iter().flatten() {
                collect_cells(&mut cells, &cam, p, setup.ftex, &mut near_dropped);
            }
            cells.sort_by(|a, b| b.z.total_cmp(&a.z));
            for cell in &cells {
                for tri in [[0usize, 1, 2], [0, 2, 3]] {
                    fill_cell(
                        &mut layer,
                        320,
                        [0, 1, 2].map(|k| cell.screen[tri[k]]),
                        [0, 1, 2].map(|k| cell.uv[tri[k]]),
                        cell.pic,
                    );
                }
            }
        } else {
            let mut order: Vec<(f32, &Pic)> = pics
                .iter()
                .flatten()
                .map(|p| (cam.to_camera(p.q)[2], p))
                .collect();
            order.sort_by(|a, b| b.0.total_cmp(&a.0));
            for (_, p) in order {
                draw_flat(&mut layer, &cam, p, setup.ftex);
            }
        }
        let mut image = RgbImage::new(320 * 3, 240);
        let (mut seen, mut holes, mut extra, mut err) = (0usize, 0usize, 0usize, 0.0f64);
        for y in 0..240usize {
            for x in 0..320usize {
                let at = y * 320 + x;
                let t_cov = truth.coverage[at] >= 0.5;
                let t_rgb = if t_cov { truth.rgb[at] } else { SKY };
                let r_cov = real.coverage[at] >= 0.5;
                let sys = if r_cov {
                    real.rgb[at]
                } else if layer.covered[at] {
                    layer.rgb[at]
                } else {
                    SKY
                };
                let px = |c: [f32; 3]| Rgb(c.map(|v| v.clamp(0.0, 255.0) as u8));
                image.put_pixel(x as u32, y as u32, px(t_rgb));
                image.put_pixel(320 + x as u32, y as u32, px(sys));
                if (y < 64 && x < 150) || (y > 90 && (110..210).contains(&x)) {
                    continue;
                }
                let far =
                    !t_cov || (truth.room[at] != u16::MAX && !near(usize::from(truth.room[at])));
                if !far || r_cov {
                    continue;
                }
                seen += 1;
                let e = (0..3).map(|i| (sys[i] - t_rgb[i]).abs()).sum::<f32>() / 3.0;
                err += f64::from(e);
                let hole = t_cov && !layer.covered[at];
                let spare = !t_cov && layer.covered[at];
                holes += usize::from(hole);
                extra += usize::from(spare);
                if t_cov {
                    let d = setup.world.centres[usize::from(truth.room[at])].map_or(0.0, |c| {
                        ((c[0] - pose.eye[0]).powi(2) + (c[1] - pose.eye[2]).powi(2)).sqrt()
                            / sector
                    });
                    let b = match d {
                        d if d < 18.0 => 0,
                        d if d < 24.0 => 1,
                        d if d < 36.0 => 2,
                        d if d < 48.0 => 3,
                        _ => 4,
                    };
                    buckets[b].0 += 1;
                    buckets[b].1 += usize::from(hole);
                    buckets[b].2 += f64::from(e);
                }
                let mark = if hole {
                    [255.0, 0.0, 0.0]
                } else if spare {
                    [0.0, 160.0, 255.0]
                } else {
                    let g = (e * 3.0).min(255.0);
                    [g, g, g]
                };
                image.put_pixel(640 + x as u32, y as u32, px(mark));
            }
        }
        t_seen += seen;
        t_holes += holes;
        t_extra += extra;
        t_err += err;
        let pct = 100.0 * holes as f64 / seen.max(1) as f64;
        println!(
            "F f{} eye {eye_idx} far {seen} holes {pct:.1}% extra {:.1}% err {:.1}",
            pose.frame,
            100.0 * extra as f64 / seen.max(1) as f64,
            err / seen.max(1) as f64
        );
        results.push((pct, pose.frame, image));
    }
    println!("cells dropped for the near plane: {near_dropped}");
    println!(
        "DATA per eye ({} eyes): covered texels {}, tiles {}, texels in rects {}",
        data.0,
        data.1 / data.0.max(1),
        data.2 / data.0.max(1),
        data.3 / data.0.max(1)
    );
    println!(
        "TOTAL far {t_seen} holes {:.2}% extra {:.2}% mean err {:.2}",
        100.0 * t_holes as f64 / t_seen.max(1) as f64,
        100.0 * t_extra as f64 / t_seen.max(1) as f64,
        t_err / t_seen.max(1) as f64
    );
    for (i, name) in ["12-18", "18-24", "24-36", "36-48", "48+"].iter().enumerate() {
        let (n, h, e) = buckets[i];
        println!(
            "BUCKET {name}: {n} px holes {:.1}% err {:.1}",
            100.0 * h as f64 / n.max(1) as f64,
            e / n.max(1) as f64
        );
    }
    results.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (pct, frame, image) in results.into_iter().take(worst) {
        image
            .save(out.join(format!("worst-f{frame}-{pct:.0}.png")))
            .expect("save");
    }
}
