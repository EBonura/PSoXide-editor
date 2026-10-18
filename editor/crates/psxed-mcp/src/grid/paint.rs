//! Materials and tiling on faces.

use std::collections::BTreeSet;

use psxed_project::face_uv::UvMode;
use psxed_project::grid::edit::bands::run_faces;
use psxed_project::grid::edit::{
    edit_uv, faces_of_cell, set_material, uv_of, FaceRef, FaceTarget, SurfaceKind, UvOp,
};
use psxed_project::grid::uv::{report_for, ProjectUvDefaults};
use psxed_project::grid::uv_region::fit_across;
use psxed_project::grid::{derive, GridWorld, WallEdge};
use psxed_project::{ProjectDocument, ResourceData};
use schemars::JsonSchema;
use serde::Deserialize;

use super::walls::{resolve_wall, WallRef};
use super::{check_rect, face_label, parse_cardinal, pick_room, Cells, Materials};

/// Which faces of a rectangle a call reaches.
fn parse_faces(list: &Option<Vec<String>>) -> Result<(bool, bool, bool, bool), String> {
    let default = vec![
        "floor".to_string(),
        "ceiling".to_string(),
        "walls".to_string(),
    ];
    let (mut floor, mut ceiling, mut authored, mut derived) = (false, false, false, false);
    for name in list.as_ref().unwrap_or(&default) {
        match name.to_ascii_lowercase().as_str() {
            "floor" | "floors" => floor = true,
            "ceiling" | "ceilings" => ceiling = true,
            "walls" | "wall" => {
                authored = true;
                derived = true;
            }
            "authored" => authored = true,
            "derived" => derived = true,
            "all" => {
                floor = true;
                ceiling = true;
                authored = true;
                derived = true;
            }
            other => {
                return Err(format!(
                "unknown face group {other:?}; use floor, ceiling, walls, authored, derived or all"
            ))
            }
        }
    }
    Ok((floor, ceiling, authored, derived))
}

/// The faces a selection reaches in a rectangle.
///
/// A derived wall belongs to the sector that owns the step (a platform's own
/// cells own its sides) but is listed under the sector that sees it. `by_host`
/// picks walls by the sector they are listed under; otherwise by their owner,
/// which is what "paint this platform's sides" means.
#[allow(clippy::too_many_arguments)]
pub fn select_faces(
    world: &GridWorld,
    room: usize,
    rect: &psxed_project::grid::edit::CellRect,
    groups: &Option<Vec<String>>,
    edge: Option<WallEdge>,
    by_host: bool,
) -> Result<Vec<FaceRef>, String> {
    let (floor, ceiling, authored, derived) = parse_faces(groups)?;
    let r = &world.rooms[room];
    let mut out: BTreeSet<FaceRef> = BTreeSet::new();
    for cell in rect.cells() {
        for face in faces_of_cell(world, room, cell) {
            let keep = match face.target {
                FaceTarget::Surface(SurfaceKind::Floor) => floor,
                FaceTarget::Surface(SurfaceKind::Ceiling) => ceiling,
                FaceTarget::Wall { edge: e, .. } => authored && edge.is_none_or(|w| w == e),
                FaceTarget::Derived { .. } => false,
            };
            if keep {
                out.insert(face);
            }
        }
    }
    if derived {
        // Hosts and owners can be one cell outside the rectangle.
        let lo = [rect.min[0] - 1, rect.min[1] - 1];
        let hi = [rect.max[0] + 1, rect.max[1] + 1];
        let wide = psxed_project::grid::edit::CellRect::from_corners(lo, hi);
        for cell in wide.cells() {
            for wall in derive::cell_walls(r, cell) {
                let anchor = if by_host { wall.host } else { wall.owner };
                if rect.contains(anchor) && edge.is_none_or(|w| w == wall.edge) {
                    out.insert(FaceRef::derived(room, wall.host, wall.edge, wall.kind));
                }
            }
        }
    }
    Ok(out.into_iter().collect())
}

/// Paint a material on faces.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PaintReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Required when the world has several rooms.
    pub room: Option<usize>,
    /// The rectangle of WORLD sectors.
    #[serde(flatten)]
    pub cells: Cells,
    /// Material name or id; "none" gives the untextured fallback.
    pub material: String,
    /// Which faces: any of `floor`, `ceiling`, `walls` (authored and
    /// derived), `authored`, `derived`, `all`. Default floor, ceiling, walls.
    pub faces: Option<Vec<String>>,
    /// Only walls on this edge: north, east, south or west.
    pub edge: Option<String>,
    /// Derived walls are picked by the sector that OWNS the step (a
    /// platform's own cells own its sides). true picks them by the sector
    /// they are listed under (the lower neighbour) instead.
    pub by_host: Option<bool>,
    /// Floors and ceilings only: paint one triangle (0 or 1) of a split
    /// surface. Omit for both.
    pub tri: Option<u8>,
}

/// Apply [`PaintReq`].
pub fn paint(
    world: &mut GridWorld,
    mats: &Materials<'_>,
    req: &PaintReq,
) -> Result<String, String> {
    let room = pick_room(world, req.room)?;
    let rect = req.cells.rect();
    check_rect(&rect)?;
    let material = mats.resolve_opt(Some(&req.material))?;
    let edge = req.edge.as_deref().map(parse_cardinal).transpose()?;
    let faces = select_faces(
        world,
        room,
        &rect,
        &req.faces,
        edge,
        req.by_host.unwrap_or(false),
    )?;
    if faces.is_empty() {
        return Err("no faces of that kind in the rectangle".to_string());
    }
    let (mut changed, mut floors, mut ceilings, mut walls) = (0usize, 0usize, 0usize, 0usize);
    for face in &faces {
        match face.target {
            FaceTarget::Surface(SurfaceKind::Floor) => floors += 1,
            FaceTarget::Surface(SurfaceKind::Ceiling) => ceilings += 1,
            _ => walls += 1,
        }
        if set_material(world, *face, req.tri, material) {
            changed += 1;
        }
    }
    Ok(format!(
        "room {room}: painted {} on {floors} floor(s), {ceilings} ceiling(s), {walls} wall(s); {changed} actually changed",
        mats.label(material)
    ))
}

/// Paint one wall, or its whole run.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PaintWallReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// The wall.
    #[serde(flatten)]
    pub wall: WallRef,
    /// Material name or id; "none" gives the untextured fallback.
    pub material: String,
    /// Paint the whole run of collinear faces the wall belongs to (what a
    /// fit mapping treats as one face), not just this segment.
    pub run: Option<bool>,
}

/// Apply [`PaintWallReq`].
pub fn paint_wall(
    world: &mut GridWorld,
    mats: &Materials<'_>,
    req: &PaintWallReq,
) -> Result<String, String> {
    let face = resolve_wall(world, &req.wall)?;
    let material = mats.resolve_opt(Some(&req.material))?;
    let faces = if req.run.unwrap_or(false) {
        run_faces(world, face)
    } else {
        vec![face]
    };
    let mut changed = 0;
    for f in &faces {
        if set_material(world, *f, None, material) {
            changed += 1;
        }
    }
    Ok(format!(
        "painted {} on {} wall segment(s) ({}); {changed} changed",
        mats.label(material),
        faces.len(),
        face_label(&face)
    ))
}

/// How a tiling request is laid out.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TilingReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Required when the world has several rooms.
    pub room: Option<usize>,
    /// A rectangle of WORLD sectors whose faces change. Give this or `wall`.
    pub from: Option<[i32; 2]>,
    /// Opposite corner of the rectangle, inclusive.
    pub to: Option<[i32; 2]>,
    /// Which faces of the rectangle: floor, ceiling, walls, authored,
    /// derived, all. Default floor and ceiling.
    pub faces: Option<Vec<String>>,
    /// Only walls on this edge.
    pub edge: Option<String>,
    /// Pick derived walls by host instead of owner (see grid_paint).
    pub by_host: Option<bool>,
    /// A single wall instead of a rectangle.
    pub wall: Option<WallRef>,
    /// With `wall`: the whole run of collinear faces.
    pub run: Option<bool>,
    /// Floors and ceilings: one triangle (0 or 1). Omit for both.
    pub tri: Option<u8>,
    /// `world` (the texture follows the world: neighbours line up, a bigger
    /// face tiles more), `fit` (a fixed number of tiles across EACH face:
    /// 2 x 2 over a 4 x 2 patch of cells is eight small tilings),
    /// `fit_across` (a fixed number of tiles across the WHOLE selected
    /// rectangle: `repeats` [2, 2] over 4 x 2 cells is one 2 x 2 tiling,
    /// each cell showing its share; floors and ceilings, each its own patch;
    /// walls fit per run) or `inherit` (the material's own tiling default,
    /// else the legacy 32 texels per sector).
    pub mode: Option<String>,
    /// For `world`: texels per sector along the frame's two axes. 32 is the
    /// legacy density (half a 64 texel tile per sector); 64 is one tile.
    pub density: Option<[u16; 2]>,
    /// For `fit`: tiles across the face, horizontal and vertical, in steps
    /// of 1/16 up to 16. `[2, 2]` is "2 x 2 on this face". For `fit_across`:
    /// tiles across the whole patch, up to 64. A primitive holds at most 4
    /// tiles per axis, so more than 4 per cell makes the cook cut faces (the
    /// reply says how many).
    pub repeats: Option<[f32; 2]>,
    /// Quarter turns of the texture, clockwise, 0..3.
    pub turns: Option<u8>,
    /// Mirror the texture horizontally.
    pub flip_h: Option<bool>,
    /// Mirror the texture vertically.
    pub flip_v: Option<bool>,
    /// Offset in texels `[u, v]` (fine adjustment after the mode).
    pub offset: Option<[i16; 2]>,
    /// Scale, Q8 per axis (256 = 1.0, larger = the texture looks bigger,
    /// negative mirrors).
    pub scale_q8: Option<[i16; 2]>,
    /// Free rotation in degrees.
    pub rotation_deg: Option<i16>,
    /// Back to the default mapping (clears every other field first).
    pub reset: Option<bool>,
}

/// Whether the request fits across a whole patch (`fit_across`).
fn is_across(req: &TilingReq) -> bool {
    req.mode
        .as_deref()
        .is_some_and(|m| matches!(m.to_ascii_lowercase().as_str(), "fit_across" | "fit-across"))
}

fn parse_mode(req: &TilingReq) -> Result<Option<UvMode>, String> {
    let mode = req.mode.as_deref().map(str::to_ascii_lowercase);
    match mode.as_deref() {
        Some("fit_across" | "fit-across") => Ok(None),
        None => {
            if req.density.is_some() {
                return Ok(Some(UvMode::World {
                    texels_per_sector: req.density.unwrap_or([32, 32]),
                }));
            }
            if req.repeats.is_some() {
                return fit(req.repeats).map(Some);
            }
            Ok(None)
        }
        Some("inherit") => Ok(Some(UvMode::Inherit)),
        Some("world") => Ok(Some(UvMode::World {
            texels_per_sector: req.density.unwrap_or([32, 32]),
        })),
        Some("fit") => fit(req.repeats.or(Some([1.0, 1.0]))).map(Some),
        Some(other) => Err(format!(
            "unknown mode {other:?}; use world, fit, fit_across or inherit"
        )),
    }
}

fn fit(repeats: Option<[f32; 2]>) -> Result<UvMode, String> {
    let [h, v] = repeats.unwrap_or([1.0, 1.0]);
    let q = |value: f32| -> Result<u16, String> {
        if !(value > 0.0 && value <= 16.0) {
            return Err(format!("fit repeats must be in (0, 16], got {value}"));
        }
        Ok((value * 16.0).round().max(1.0) as u16)
    };
    Ok(UvMode::Fit {
        repeats_q4: [q(h)?, q(v)?],
    })
}

/// Apply the requested ops to `face`. Returns whether anything changed.
fn apply_tiling(
    world: &mut GridWorld,
    face: FaceRef,
    req: &TilingReq,
    mode: Option<UvMode>,
) -> bool {
    let tri = req.tri;
    let mut changed = false;
    if req.reset.unwrap_or(false) {
        changed |= edit_uv(world, face, tri, UvOp::Reset);
    }
    if let Some(mode) = mode {
        changed |= edit_uv(world, face, tri, UvOp::SetMode(mode));
    }
    if let Some(want) = req.turns {
        let want = want & 3;
        let now = uv_of(world, face, tri).map_or(0, |uv| uv.turns & 3);
        for _ in 0..(want.wrapping_sub(now) & 3) {
            changed |= edit_uv(world, face, tri, UvOp::Turn(true));
        }
    }
    for (axis, flag) in [(0usize, req.flip_h), (1, req.flip_v)] {
        if let Some(want) = flag {
            let now = uv_of(world, face, tri).is_some_and(|uv| uv.flip[axis]);
            if now != want {
                changed |= edit_uv(world, face, tri, UvOp::ToggleFlip(axis));
            }
        }
    }
    if let Some(offset) = req.offset {
        changed |= edit_uv(world, face, tri, UvOp::SetOffset(offset));
    }
    if let Some(scale) = req.scale_q8 {
        changed |= edit_uv(world, face, tri, UvOp::SetScale(scale));
    }
    if let Some(rotation) = req.rotation_deg {
        changed |= edit_uv(world, face, tri, UvOp::SetRotation(rotation));
    }
    changed
}

/// Apply [`TilingReq`], then report what the tiling of the first face comes to.
pub fn tiling(
    world: &mut GridWorld,
    project: &ProjectDocument,
    req: &TilingReq,
) -> Result<String, String> {
    let mode = parse_mode(req)?;
    let faces: Vec<FaceRef> = match (&req.wall, req.from) {
        (Some(wall), None) => {
            let face = resolve_wall(world, wall)?;
            if req.run.unwrap_or(false) {
                run_faces(world, face)
            } else {
                vec![face]
            }
        }
        (None, Some(from)) => {
            let room = pick_room(world, req.room)?;
            let rect = Cells { from, to: req.to }.rect();
            check_rect(&rect)?;
            let groups = req
                .faces
                .clone()
                .or_else(|| Some(vec!["floor".to_string(), "ceiling".to_string()]));
            let edge = req.edge.as_deref().map(parse_cardinal).transpose()?;
            select_faces(
                world,
                room,
                &rect,
                &groups,
                edge,
                req.by_host.unwrap_or(false),
            )?
        }
        _ => return Err("give either a wall or a from/to rectangle".to_string()),
    };
    if faces.is_empty() {
        return Err("no faces of that kind there".to_string());
    }
    let across = is_across(req);
    let any = req.reset.is_some()
        || mode.is_some()
        || across
        || req.turns.is_some()
        || req.flip_h.is_some()
        || req.flip_v.is_some()
        || req.offset.is_some()
        || req.scale_q8.is_some()
        || req.rotation_deg.is_some();
    if !any {
        return Err("nothing to change: give mode, density, repeats, turns, flip_h, flip_v, offset, scale_q8, rotation_deg or reset".to_string());
    }
    if across && req.wall.is_some() {
        return Err(
            "fit_across spans a rectangle of floors or ceilings; a wall's frame is already its whole run (use mode fit with run: true)"
                .to_string(),
        );
    }
    let defaults = ProjectUvDefaults(project);
    let mut changed = 0;
    for face in &faces {
        if apply_tiling(world, *face, req, mode) {
            changed += 1;
        }
    }
    let mut across_text = None;
    if across {
        // After the other ops: a reset clears the frame, the mode sets it.
        let repeats = req.repeats.unwrap_or([1.0, 1.0]);
        let fitted = fit_across(world, &defaults, &faces, req.tri, repeats)?;
        changed = changed.max(fitted.changed);
        across_text = Some(fitted.describe(repeats));
    }
    let first = faces[0];
    let mut out = format!("tiling set on {} face(s), {changed} changed", faces.len());
    if let Some(text) = across_text {
        out.push('\n');
        out.push_str(&text);
    }
    if let Some(report) = report_for(world, &defaults, first, req.tri) {
        out.push_str(&format!(
            "\n{}: effective {}",
            face_label(&first),
            report.effective_text()
        ));
        if let Some(warning) = report.split_warning() {
            out.push_str(&format!("\nwarning: {warning}"));
        }
    }
    Ok(out)
}

/// The tiling default of a material.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct MaterialTilingReq {
    /// Material name or id.
    pub material: String,
    /// `world`, `fit` or `inherit` (the legacy 32 texels per sector).
    pub mode: String,
    /// For `world`: texels per sector along the two axes.
    pub density: Option<[u16; 2]>,
    /// For `fit`: tiles across a face, horizontal and vertical.
    pub repeats: Option<[f32; 2]>,
}

/// Set a material's grid tiling default. Faces whose own mode is `inherit`
/// follow it; a face that names its own mode ignores it.
pub fn material_tiling(
    project: &mut ProjectDocument,
    req: &MaterialTilingReq,
) -> Result<String, String> {
    let id = Materials::new(project).resolve(&req.material)?;
    let mode = match req.mode.to_ascii_lowercase().as_str() {
        "inherit" => UvMode::Inherit,
        "world" => UvMode::World {
            texels_per_sector: req.density.unwrap_or([32, 32]),
        },
        "fit" => fit(req.repeats)?,
        other => return Err(format!("unknown mode {other:?}; use world, fit or inherit")),
    };
    let name = project
        .resource(id)
        .map(|r| r.name.clone())
        .unwrap_or_default();
    let resource = project
        .resource_mut(id)
        .ok_or_else(|| "material vanished".to_string())?;
    let ResourceData::Material(material) = &mut resource.data else {
        return Err(format!("{name:?} is not a material"));
    };
    material.grid_uv = mode;
    Ok(format!("{name:?}: grid tiling default is now {mode:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::build::{set_heights, SetHeightsReq, Surface};

    use crate::grid::test_support::fixture;

    fn some_material(p: &ProjectDocument, name: &str) -> String {
        Materials::new(p).resolve(name).unwrap().raw().to_string()
    }

    #[test]
    fn painting_a_platform_reaches_its_derived_sides_by_owner() {
        let (p, mut w) = fixture();
        let mats = Materials::new(&p);
        set_heights(
            &mut w,
            &SetHeightsReq {
                aim: Default::default(),
                scene: None,
                room: None,
                kind: Surface::Floor,
                cells: Cells {
                    from: [2, 2],
                    to: Some([3, 3]),
                },
                corners: None,
                edge: None,
                height: Some(512),
                delta: None,
                snap: None,
                link: None,
                layer: None,
            },
        )
        .unwrap();
        let cliff = some_material(&p, "Graybox / Cliff 64");
        let out = paint(
            &mut w,
            &mats,
            &PaintReq {
                scene: None,
                room: None,
                cells: Cells {
                    from: [2, 2],
                    to: Some([3, 3]),
                },
                material: cliff,
                faces: Some(vec!["derived".into()]),
                edge: None,
                by_host: None,
                tri: None,
            },
        )
        .unwrap();
        // Four sides of two cells each.
        assert!(out.contains("8 wall"), "{out}");
        assert_eq!(w.rooms[0].edge_overrides.len(), 8);
    }

    #[test]
    fn fit_tiling_reports_the_effective_repeat() {
        let (p, mut w) = fixture();
        let out = tiling(
            &mut w,
            &p,
            &TilingReq {
                scene: None,
                room: None,
                from: Some([1, 1]),
                to: Some([2, 2]),
                faces: None,
                edge: None,
                by_host: None,
                wall: None,
                run: None,
                tri: None,
                mode: Some("fit".into()),
                density: None,
                repeats: Some([2.0, 2.0]),
                turns: Some(1),
                flip_h: Some(true),
                flip_v: None,
                offset: None,
                scale_q8: None,
                rotation_deg: None,
                reset: None,
            },
        )
        .unwrap();
        assert!(out.contains("2 x 2 on this face"), "{out}");
        let uv = w.rooms[0]
            .sector(1, 1)
            .unwrap()
            .ground_floor()
            .unwrap()
            .tri(0)
            .uv;
        assert_eq!(uv.turns, 1);
        assert!(uv.flip[0] && !uv.flip[1]);
        assert!(matches!(
            uv.mode,
            UvMode::Fit {
                repeats_q4: [32, 32]
            }
        ));
    }

    #[test]
    fn a_material_default_is_a_resource_edit() {
        let (mut p, _) = fixture();
        let out = material_tiling(
            &mut p,
            &MaterialTilingReq {
                material: "Graybox / Ground 64".into(),
                mode: "world".into(),
                density: Some([64, 64]),
                repeats: None,
            },
        )
        .unwrap();
        assert!(out.contains("64"), "{out}");
    }
}
