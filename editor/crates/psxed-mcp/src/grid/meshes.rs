//! Static meshes: arches, columns, rubble and imported props.
//!
//! A mesh is not grid geometry. An **asset** is a recipe (or imported
//! geometry) the world keeps once; a room places **instances** of it with a
//! position, a turn and a scale. The cook draws an instance as one run of quads
//! with the room's other free planes, collides it through the same records
//! planes use, and turns its sockets (the crown of an arch) into hook points.
//! `docs/grid/meshes.md` has the model.

use psxed_project::grid::edit::{
    add_asset, arch_span, delete_meshes, place_mesh, set_asset, solve_arch_span, solve_span,
    span_between, update_mesh, MeshRefusal,
};
use psxed_project::grid::mesh::{
    ArchProfile, ArchSpec, ColumnSpec, ImportProxy, ImportedMesh, MeshAsset, MeshKind, RubbleSpec,
    SpanSpec, SCALE_ONE,
};
use psxed_project::grid::{GridMesh, GridWorld};
use schemars::JsonSchema;
use serde::Deserialize;

use super::{pick_room, Materials};

fn refused(r: MeshRefusal) -> String {
    r.describe()
}

/// A mesh recipe: which generator and its numbers. Omitted numbers take the
/// generator's defaults. All lengths are GRID units (1024 to a sector).
#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
pub struct MeshRecipe {
    /// `arch`, `column`, `rubble`, `span` or `obj`.
    pub kind: String,
    /// Name shown in the palette and `grid_meshes`.
    pub name: Option<String>,
    /// arch: `round` (default), `segmental`, `pointed` or `flat`.
    pub profile: Option<String>,
    /// arch: clear opening between the piers (256 to 8192). Default 2048.
    pub span: Option<i32>,
    /// arch: height of the crown of the intrados above the springing line.
    /// Round wants half the span, segmental less, pointed at least half.
    /// span: height of the end above the start (negative descends).
    pub rise: Option<i32>,
    /// arch: height of the springing line above the floor (the piers). The
    /// character is 64 runtime units tall, so keep it above about 1000 grid
    /// units (at the default scale) if a body is to walk through.
    pub spring: Option<i32>,
    /// arch: thickness of the band and width of the piers (64 to 2048).
    /// span: vertical thickness of the deck (64 to 8192).
    pub thickness: Option<i32>,
    /// arch: depth of the band (64 to 4096).
    pub depth: Option<i32>,
    /// arch: pieces per half of the curve (1 to 8). Default 4.
    pub segments: Option<u8>,
    /// arch: put a hook socket at the crown so the player can perch on it.
    /// Default true.
    pub perch: Option<bool>,
    /// span: length between the two supports (128 to 16384). Default 4096.
    pub length: Option<i32>,
    /// span: width across the deck (64 to 8192). Default 1024.
    pub width: Option<i32>,
    /// column: shaft circumradius (64 to 1024).
    pub radius: Option<i32>,
    /// column: total height.
    pub height: Option<i32>,
    /// column: sides of the shaft (3 to 12).
    pub sides: Option<u8>,
    /// column: plinth height.
    pub plinth: Option<i32>,
    /// column: capital height.
    pub capital: Option<i32>,
    /// rubble: diameter of the scatter (128 to 4096).
    pub size: Option<i32>,
    /// rubble: number of stones (1 to 6).
    pub pieces: Option<u8>,
    /// rubble: shape seed.
    pub seed: Option<u16>,
    /// obj: path of the `.obj` file (relative to the repository root, or
    /// absolute). Quads and triangles are kept; `usemtl` names become
    /// material slots; `vt` becomes texels (64 per unit). The origin moves to
    /// the floor under the centre of the mesh.
    pub path: Option<String>,
    /// obj: grid units per OBJ unit. Default 1024 (an OBJ unit is a sector).
    pub scale: Option<f64>,
    /// obj: collision, `none` (default), `box`, `box_top` (a box with a
    /// surface to stand on) or `slab` (one thick platform).
    pub collision: Option<String>,
}

fn arch_from(r: &MeshRecipe, base: ArchSpec) -> Result<ArchSpec, String> {
    let profile = match &r.profile {
        Some(name) => psxed_project::grid::edit::meshes::parse_profile(name).map_err(refused)?,
        None => base.profile,
    };
    Ok(ArchSpec {
        profile,
        span: r.span.unwrap_or(base.span),
        rise: r.rise.unwrap_or(base.rise),
        spring: r.spring.unwrap_or(base.spring),
        thickness: r.thickness.unwrap_or(base.thickness),
        depth: r.depth.unwrap_or(base.depth),
        segments: r.segments.unwrap_or(base.segments),
        perch: r.perch.unwrap_or(base.perch),
    })
}

fn span_from(r: &MeshRecipe, base: SpanSpec) -> SpanSpec {
    SpanSpec {
        length: r.length.unwrap_or(base.length),
        width: r.width.unwrap_or(base.width),
        thickness: r.thickness.unwrap_or(base.thickness),
        rise: r.rise.unwrap_or(base.rise),
    }
}

fn column_from(r: &MeshRecipe, base: ColumnSpec) -> ColumnSpec {
    ColumnSpec {
        radius: r.radius.unwrap_or(base.radius),
        height: r.height.unwrap_or(base.height),
        sides: r.sides.unwrap_or(base.sides),
        plinth: r.plinth.unwrap_or(base.plinth),
        capital: r.capital.unwrap_or(base.capital),
    }
}

fn rubble_from(r: &MeshRecipe, base: RubbleSpec) -> RubbleSpec {
    RubbleSpec {
        size: r.size.unwrap_or(base.size),
        pieces: r.pieces.unwrap_or(base.pieces),
        seed: r.seed.unwrap_or(base.seed),
    }
}

/// The recipe as a `MeshKind`, over the numbers of `current` (an asset being
/// changed) when it is the same kind.
fn kind_of(r: &MeshRecipe, current: Option<&MeshKind>) -> Result<MeshKind, String> {
    match r.kind.to_ascii_lowercase().as_str() {
        "arch" => {
            let base = match current {
                Some(MeshKind::Arch(s)) => *s,
                _ => ArchSpec::default(),
            };
            Ok(MeshKind::Arch(arch_from(r, base)?))
        }
        "column" | "pillar" => {
            let base = match current {
                Some(MeshKind::Column(s)) => *s,
                _ => ColumnSpec::default(),
            };
            Ok(MeshKind::Column(column_from(r, base)))
        }
        "span" | "deck" | "bridge" => {
            let base = match current {
                Some(MeshKind::Span(s)) => *s,
                _ => SpanSpec::default(),
            };
            Ok(MeshKind::Span(span_from(r, base)))
        }
        "rubble" => {
            let base = match current {
                Some(MeshKind::Rubble(s)) => *s,
                _ => RubbleSpec::default(),
            };
            Ok(MeshKind::Rubble(rubble_from(r, base)))
        }
        "obj" => {
            let path = r
                .path
                .as_deref()
                .ok_or("an obj recipe needs a path to the .obj file")?;
            let text =
                std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
            let proxy = match r
                .collision
                .as_deref()
                .map(str::to_ascii_lowercase)
                .as_deref()
            {
                None | Some("none") => ImportProxy::None,
                Some("box") => ImportProxy::Box,
                Some("box_top") => ImportProxy::BoxTop,
                Some("slab") => ImportProxy::Slab,
                Some(other) => {
                    return Err(format!(
                        "unknown collision {other:?}; use none, box, box_top or slab"
                    ))
                }
            };
            let source = std::path::Path::new(path)
                .file_name()
                .map_or(path.to_string(), |n| n.to_string_lossy().into_owned());
            Ok(MeshKind::Imported(ImportedMesh::from_obj(
                &text,
                r.scale.unwrap_or(1024.0),
                &source,
                proxy,
            )?))
        }
        other => Err(format!(
            "unknown mesh kind {other:?}; use arch, column, rubble, span or obj"
        )),
    }
}

fn default_name(kind: &MeshKind) -> String {
    match kind {
        MeshKind::Arch(s) => format!("{} arch {}", s.profile.label(), s.span),
        MeshKind::Column(s) => format!("column {}", s.height),
        MeshKind::Rubble(s) => format!("rubble {}", s.seed),
        MeshKind::Span(s) => format!("span {} x {} x {}", s.length, s.width, s.thickness),
        MeshKind::Imported(m) => {
            if m.source.is_empty() {
                "imported mesh".to_string()
            } else {
                m.source.clone()
            }
        }
    }
}

/// An asset named by index or by (a unique part of) its name.
fn find_asset(world: &GridWorld, needle: &str) -> Result<usize, String> {
    if let Ok(i) = needle.trim().parse::<usize>() {
        if i < world.mesh_assets.len() {
            return Ok(i);
        }
    }
    let lowered = needle.to_ascii_lowercase();
    let hits: Vec<usize> = world
        .mesh_assets
        .iter()
        .enumerate()
        .filter(|(_, a)| a.name.to_ascii_lowercase().contains(&lowered))
        .map(|(i, _)| i)
        .collect();
    match hits.as_slice() {
        [only] => Ok(*only),
        [] => Err(format!(
            "no mesh asset matches {needle:?}; the world has {} (grid_meshes lists them)",
            world.mesh_assets.len()
        )),
        many => Err(format!(
            "{needle:?} matches assets {many:?}; name one by index"
        )),
    }
}

/// Q8 scale from one number (uniform) or three.
fn scale_q8(values: &[f64]) -> Result<[u16; 3], String> {
    let one = |v: f64| -> Result<u16, String> {
        if !(0.125..=8.0).contains(&v) {
            return Err(format!("scale {v} is outside 0.125 to 8"));
        }
        Ok((v * f64::from(SCALE_ONE)).round() as u16)
    };
    match values {
        [v] => Ok([one(*v)?; 3]),
        [x, y, z] => Ok([one(*x)?, one(*y)?, one(*z)?]),
        _ => Err("scale is one number (uniform) or three (x, y, z)".to_string()),
    }
}

/// Describe one asset in a block.
pub fn describe_asset(index: usize, asset: &MeshAsset, world: &GridWorld) -> String {
    let used = world
        .rooms
        .iter()
        .flat_map(|r| &r.meshes)
        .filter(|m| usize::from(m.asset) == index)
        .count();
    match asset.build() {
        Ok(built) => format!(
            "asset {index} {:?}: {}\n  {} face(s), {} collision shape(s), {} socket(s), material slots {:?}, placed {used} time(s)",
            asset.name,
            asset.describe(),
            built.faces.len(),
            built.proxies.len(),
            built.sockets.len(),
            built.slots,
        ),
        Err(e) => format!("asset {index} {:?}: does not build: {e}", asset.name),
    }
}

/// Describe one instance in a block.
pub fn describe_mesh(
    room: usize,
    mesh: &GridMesh,
    world: &GridWorld,
    mats: &Materials<'_>,
) -> String {
    let asset = world.mesh_assets.get(usize::from(mesh.asset));
    let (cells, kind) = match asset.and_then(|a| a.build().ok()) {
        Some(built) => {
            let (lo, hi) = mesh.cell_span(&built);
            (format!("sectors {lo:?}..{hi:?}"), String::new())
        }
        None => (
            "does not build".to_string(),
            " (its asset is missing or invalid)".to_string(),
        ),
    };
    let materials: Vec<String> = (0..asset
        .and_then(|a| a.build().ok())
        .map_or(0, |b| b.slots.len()))
        .map(|slot| mats.short(mesh.material(slot as u8)))
        .collect();
    format!(
        "mesh {} (room {room}): asset {} {:?}{kind}, position {:?}, rotation [yaw, pitch, roll] {:?}, scale {:?} (Q8)\n  materials {materials:?}{}{}{}; covers {cells}",
        mesh.id,
        mesh.asset,
        asset.map_or("?", |a| a.name.as_str()),
        mesh.position,
        mesh.rotation,
        mesh.scale,
        if mesh.collide { "" } else { ", no collision" },
        if mesh.fullbright { ", fullbright" } else { "" },
        if mesh.enabled { "" } else { ", DISABLED" },
    )
}

/// List the assets and the placed instances.
pub fn list(
    world: &GridWorld,
    mats: &Materials<'_>,
    room: Option<usize>,
) -> Result<String, String> {
    let rooms: Vec<usize> = match room {
        Some(_) => vec![pick_room(world, room)?],
        None => (0..world.rooms.len()).collect(),
    };
    let mut out = String::new();
    if world.mesh_assets.is_empty() {
        out.push_str("no mesh assets yet\n");
    }
    for (i, asset) in world.mesh_assets.iter().enumerate() {
        out.push_str(&describe_asset(i, asset, world));
        out.push('\n');
    }
    let mut placed = 0;
    for r in rooms {
        for mesh in &world.rooms[r].meshes {
            placed += 1;
            out.push_str(&describe_mesh(r, mesh, world, mats));
            out.push('\n');
        }
    }
    if placed == 0 {
        out.push_str("no meshes placed\n");
    }
    out.push_str(
        "\nNew assets: grid_place_mesh with a recipe, or grid_arch_span / grid_span. Kinds: arch (round, segmental, pointed, flat; span, rise, spring, thickness, depth, segments, perch), column (radius, height, sides, plinth, capital), rubble (size, pieces, seed), span (length, width, thickness, rise; also grid_span), obj (path, scale, collision: none, box, box_top, slab). Lengths are grid units, 1024 to a sector.\n",
    );
    Ok(out)
}

/// Place a mesh.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PlaceMeshReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Required when the world has several rooms. The room grows
    /// to hold the mesh.
    pub room: Option<usize>,
    /// An existing asset, by index or name (grid_meshes lists them).
    pub asset: Option<String>,
    /// Or a recipe that makes (or reuses an equal) asset.
    pub recipe: Option<MeshRecipe>,
    /// Where the asset's origin (the floor under the middle of its base)
    /// goes, grid units: x and z on the world lattice (1024 per sector), y
    /// relative to the room's y_offset.
    pub position: [i32; 3],
    /// `[yaw, pitch, roll]` degrees (the plane convention: yaw turns about
    /// the vertical, 90 turns +x toward -z). Default none.
    pub rotation: Option<[i32; 3]>,
    /// Yaw alone, degrees (ignored when `rotation` is given).
    pub yaw: Option<i32>,
    /// Scale: one number (uniform) or three (x, y, z), 0.125 to 8.
    pub scale: Option<Vec<f64>>,
    /// Material for every slot, name or id.
    pub material: Option<String>,
    /// Material per slot (arch: fascia, soffit, extrados, pier), in order;
    /// shorter lists leave the rest on `material`.
    pub materials: Option<Vec<String>>,
    /// Collide (the asset's collision shapes cook). Default true.
    pub collide: Option<bool>,
    /// Draw at the material tint with no light baked in.
    pub fullbright: Option<bool>,
}

fn apply_look(
    mesh: &mut GridMesh,
    mats: &Materials<'_>,
    material: Option<&str>,
    materials: Option<&[String]>,
    slots: usize,
) -> Result<(), String> {
    let base = match material {
        Some(name) => Some(mats.resolve_opt(Some(name))?),
        None => None,
    };
    for slot in 0..slots {
        let named = materials.and_then(|m| m.get(slot));
        let value = match named {
            Some(name) => Some(mats.resolve_opt(Some(name))?),
            None => base,
        };
        if let Some(value) = value {
            mesh.set_material(slot, value);
        }
    }
    Ok(())
}

/// Apply [`PlaceMeshReq`].
pub fn place(
    world: &mut GridWorld,
    mats: &Materials<'_>,
    req: &PlaceMeshReq,
) -> Result<String, String> {
    let room = pick_room(world, req.room)?;
    let asset = match (&req.asset, &req.recipe) {
        (Some(_), Some(_)) => return Err("give an asset or a recipe, not both".to_string()),
        (None, None) => return Err("give an asset (index or name) or a recipe".to_string()),
        (Some(name), None) => find_asset(world, name)? as u16,
        (None, Some(recipe)) => {
            let kind = kind_of(recipe, None)?;
            let name = recipe.name.clone().unwrap_or_else(|| default_name(&kind));
            add_asset(world, MeshAsset::new(name, kind)).map_err(refused)?
        }
    };
    let slots = world.mesh_assets[usize::from(asset)]
        .build()
        .map_err(|e| e.to_string())?
        .slots
        .len();
    let mut mesh = GridMesh::new(0, asset, req.position);
    mesh.rotation = req.rotation.unwrap_or([req.yaw.unwrap_or(0), 0, 0]);
    if let Some(scale) = &req.scale {
        mesh.scale = scale_q8(scale)?;
    }
    apply_look(
        &mut mesh,
        mats,
        req.material.as_deref(),
        req.materials.as_deref(),
        slots,
    )?;
    mesh.collide = req.collide.unwrap_or(true);
    mesh.fullbright = req.fullbright.unwrap_or(false);
    let id = place_mesh(world, room, mesh).map_err(refused)?;
    let (r, i) = world.find_mesh(id).ok_or("the mesh vanished")?;
    Ok(format!(
        "placed {}\n{}",
        describe_mesh(r, &world.rooms[r].meshes[i], world, mats),
        sockets_note(world, r, i)
    ))
}

/// What the cook will spawn for an instance.
fn sockets_note(world: &GridWorld, room: usize, index: usize) -> String {
    let mesh = &world.rooms[room].meshes[index];
    let Some(built) = world
        .mesh_assets
        .get(usize::from(mesh.asset))
        .and_then(|a| a.build().ok())
    else {
        return String::new();
    };
    let mut out = String::new();
    for socket in &built.sockets {
        let psxed_project::grid::mesh::Socket::Hook { anchor, .. } = *socket;
        let g = mesh.place(anchor);
        out.push_str(&format!(
            "hook socket: the player can perch under grid point {g:?} (a hook point is cooked 96 runtime units below it)\n"
        ));
    }
    out
}

/// Change a mesh.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetMeshReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// The instance id (grid_meshes lists them; ids are stable).
    pub id: u16,
    /// Move the instance (grid units).
    pub position: Option<[i32; 3]>,
    /// `[yaw, pitch, roll]` degrees.
    pub rotation: Option<[i32; 3]>,
    /// Yaw alone, degrees.
    pub yaw: Option<i32>,
    /// Scale: one number or three.
    pub scale: Option<Vec<f64>>,
    /// Use another existing asset, by index or name.
    pub asset: Option<String>,
    /// Change the instance's ASSET recipe in place (every instance of that
    /// asset changes): the numbers given replace the current ones.
    pub asset_recipe: Option<MeshRecipe>,
    /// Material for every slot.
    pub material: Option<String>,
    /// Material per slot, in order.
    pub materials: Option<Vec<String>>,
    /// Collide.
    pub collide: Option<bool>,
    /// Fullbright.
    pub fullbright: Option<bool>,
    /// Hide (false) or show (true) without deleting.
    pub enabled: Option<bool>,
}

/// Apply [`SetMeshReq`].
pub fn set(
    world: &mut GridWorld,
    mats: &Materials<'_>,
    req: &SetMeshReq,
) -> Result<String, String> {
    let (room, index) = world
        .find_mesh(req.id)
        .ok_or_else(|| format!("no mesh with id {}; grid_meshes lists them", req.id))?;
    if let Some(recipe) = &req.asset_recipe {
        let asset = usize::from(world.rooms[room].meshes[index].asset);
        let current = world.mesh_assets.get(asset).ok_or("the asset is missing")?;
        let kind = kind_of(recipe, Some(&current.kind))?;
        set_asset(world, asset, kind).map_err(refused)?;
    }
    let new_asset = match &req.asset {
        Some(name) => Some(find_asset(world, name)? as u16),
        None => None,
    };
    let scale = match &req.scale {
        Some(s) => Some(scale_q8(s)?),
        None => None,
    };
    let asset_slots = |world: &GridWorld, asset: u16| {
        world
            .mesh_assets
            .get(usize::from(asset))
            .and_then(|a| a.build().ok())
            .map_or(0, |b| b.slots.len())
    };
    let target_asset = new_asset.unwrap_or(world.rooms[room].meshes[index].asset);
    let slots = asset_slots(world, target_asset);
    let mut failure = None;
    let changed = update_mesh(world, req.id, |mesh| {
        if let Some(a) = new_asset {
            mesh.asset = a;
        }
        if let Some(p) = req.position {
            mesh.position = p;
        }
        if let Some(r) = req.rotation {
            mesh.rotation = r;
        } else if let Some(y) = req.yaw {
            mesh.rotation[0] = y;
        }
        if let Some(s) = scale {
            mesh.scale = s;
        }
        if let Err(e) = apply_look(
            mesh,
            mats,
            req.material.as_deref(),
            req.materials.as_deref(),
            slots,
        ) {
            failure = Some(e);
        }
        if let Some(v) = req.collide {
            mesh.collide = v;
        }
        if let Some(v) = req.fullbright {
            mesh.fullbright = v;
        }
        if let Some(v) = req.enabled {
            mesh.enabled = v;
        }
    })
    .map_err(refused)?;
    if let Some(e) = failure {
        return Err(e);
    }
    let (r, i) = world.find_mesh(req.id).ok_or("the mesh vanished")?;
    Ok(format!(
        "{}{}",
        if changed || req.asset_recipe.is_some() {
            "updated "
        } else {
            "no change: "
        },
        describe_mesh(r, &world.rooms[r].meshes[i], world, mats)
    ))
}

/// Delete meshes.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteMeshReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Instance ids to remove (grid_meshes lists them).
    pub ids: Vec<u16>,
}

/// Apply [`DeleteMeshReq`].
pub fn delete(world: &mut GridWorld, req: &DeleteMeshReq) -> Result<String, String> {
    let removed = delete_meshes(world, &req.ids);
    if removed == 0 {
        return Err(format!("no mesh with id in {:?}", req.ids));
    }
    Ok(format!(
        "removed {removed} mesh(es); the other ids are unchanged (their assets stay in the palette)"
    ))
}

/// An arch between two points.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ArchSpanReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Required when the world has several rooms.
    pub room: Option<usize>,
    /// One end of the clear opening: the floor point under a springing,
    /// `[x, y, z]` grid units (x and z on the world lattice, y relative to
    /// the room). The piers stand OUTSIDE `from` and `to`.
    pub from: [i32; 3],
    /// The other end. The span is the distance between the two points (256
    /// to 8192) and the arch is turned to run along them.
    pub to: [i32; 3],
    /// `round` (default), `segmental`, `pointed` or `flat`.
    pub profile: Option<String>,
    /// Rise of the crown above the springing line. Default: half the span for
    /// round, a quarter for segmental, three quarters for pointed, none for
    /// flat.
    pub rise: Option<i32>,
    /// Height of the springing line above the floor (the piers). Default 1024
    /// (80 runtime units at the default scale, over the 48 unit body).
    pub spring: Option<i32>,
    /// Band thickness and pier width. Default a eighth of the span (64 to 512).
    pub thickness: Option<i32>,
    /// Depth of the band. Default 1024.
    pub depth: Option<i32>,
    /// Pieces per half of the curve (1 to 8). Default 4.
    pub segments: Option<u8>,
    /// A hook socket at the crown so the player can perch under it. Default
    /// true.
    pub perch: Option<bool>,
    /// Material for every slot.
    pub material: Option<String>,
    /// Material per slot: fascia, soffit, extrados, pier.
    pub materials: Option<Vec<String>>,
    /// Collide. Default true: piers block, the extrados is a surface to stand
    /// on, the haunches block as boxes.
    pub collide: Option<bool>,
    /// Name of the asset.
    pub name: Option<String>,
}

/// Apply [`ArchSpanReq`].
pub fn arch(
    world: &mut GridWorld,
    mats: &Materials<'_>,
    req: &ArchSpanReq,
) -> Result<String, String> {
    let room = pick_room(world, req.room)?;
    let profile = match &req.profile {
        Some(name) => psxed_project::grid::edit::meshes::parse_profile(name).map_err(refused)?,
        None => ArchProfile::Round,
    };
    // Proportions from the span, then the explicit numbers over them.
    let span = ((req.to[0] - req.from[0]) as f64).hypot((req.to[2] - req.from[2]) as f64);
    let mut spec = ArchSpec::spanning(profile, span.round() as i32);
    if let Some(v) = req.rise {
        spec.rise = v;
    }
    if let Some(v) = req.spring {
        spec.spring = v;
    }
    if let Some(v) = req.thickness {
        spec.thickness = v;
    }
    if let Some(v) = req.depth {
        spec.depth = v;
    }
    if let Some(v) = req.segments {
        spec.segments = v;
    }
    if let Some(v) = req.perch {
        spec.perch = v;
    }
    // Solve first so a bad number is reported before anything is staged.
    solve_arch_span(req.from, req.to, spec).map_err(refused)?;
    let (_, id, placed) =
        arch_span(world, room, req.from, req.to, spec, req.name.clone()).map_err(refused)?;
    let (r, i) = world.find_mesh(id).ok_or("the mesh vanished")?;
    let slots = 4;
    {
        let mut look = world.rooms[r].meshes[i].clone();
        apply_look(
            &mut look,
            mats,
            req.material.as_deref(),
            req.materials.as_deref(),
            slots,
        )?;
        look.collide = req.collide.unwrap_or(true);
        let (m, c) = (look.materials.clone(), look.collide);
        update_mesh(world, id, |mesh| {
            mesh.materials = m;
            mesh.collide = c;
        })
        .map_err(refused)?;
    }
    let (r, i) = world.find_mesh(id).ok_or("the mesh vanished")?;
    let mut text = format!(
        "arch spanning {:?} to {:?}: span {}, rise {}, spring {}, thickness {}, depth {}, yaw {} degrees\n{}\n{}",
        req.from,
        req.to,
        placed.spec.span,
        placed.spec.rise,
        placed.spec.spring,
        placed.spec.thickness,
        placed.spec.depth,
        placed.yaw,
        describe_mesh(r, &world.rooms[r].meshes[i], world, mats),
        sockets_note(world, r, i),
    );
    if req.from[1] != req.to[1] {
        text.push_str(&format!(
            "the two points differ in height ({} and {}); the arch stands at the lower\n",
            req.from[1], req.to[1]
        ));
    }
    Ok(text)
}

/// A thick deck between two points.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SpanReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Required when the world has several rooms.
    pub room: Option<usize>,
    /// Where the top surface meets the first support, `[x, y, z]` grid units
    /// (x and z on the world lattice, y relative to the room).
    pub from: [i32; 3],
    /// Where it meets the second. The deck is as long as their plan distance
    /// (128 to 16384), climbs by their height difference (at most its
    /// length) and is turned to run along them.
    pub to: [i32; 3],
    /// Width across the deck. Default a quarter of the length (64 to 8192).
    pub width: Option<i32>,
    /// Vertical thickness. Default an eighth of the length, at most 1024 (64
    /// to 8192).
    pub thickness: Option<i32>,
    /// Material for every slot.
    pub material: Option<String>,
    /// Material per slot: top, sides, underside.
    pub materials: Option<Vec<String>>,
    /// Collide. Default true: the deck is one thick platform, solid from the
    /// top to the underside and at its ends.
    pub collide: Option<bool>,
    /// Name of the asset.
    pub name: Option<String>,
}

/// Apply [`SpanReq`].
pub fn span(world: &mut GridWorld, mats: &Materials<'_>, req: &SpanReq) -> Result<String, String> {
    let room = pick_room(world, req.room)?;
    let length = ((req.to[0] - req.from[0]) as f64)
        .hypot((req.to[2] - req.from[2]) as f64)
        .round() as i32;
    let mut spec = SpanSpec::spanning(length);
    if let Some(v) = req.width {
        spec.width = v;
    }
    if let Some(v) = req.thickness {
        spec.thickness = v;
    }
    // Solve first so a bad number is reported before anything is staged.
    solve_span(req.from, req.to, spec).map_err(refused)?;
    let (_, id, placed) =
        span_between(world, room, req.from, req.to, spec, req.name.clone()).map_err(refused)?;
    let (r, i) = world.find_mesh(id).ok_or("the mesh vanished")?;
    {
        let mut look = world.rooms[r].meshes[i].clone();
        apply_look(
            &mut look,
            mats,
            req.material.as_deref(),
            req.materials.as_deref(),
            3,
        )?;
        look.collide = req.collide.unwrap_or(true);
        let (m, c) = (look.materials.clone(), look.collide);
        update_mesh(world, id, |mesh| {
            mesh.materials = m;
            mesh.collide = c;
        })
        .map_err(refused)?;
    }
    let (r, i) = world.find_mesh(id).ok_or("the mesh vanished")?;
    Ok(format!(
        "span from {:?} to {:?}: length {}, width {}, thickness {}, rise {}, yaw {} degrees\n{}",
        req.from,
        req.to,
        placed.spec.length,
        placed.spec.width,
        placed.spec.thickness,
        placed.spec.rise,
        placed.yaw,
        describe_mesh(r, &world.rooms[r].meshes[i], world, mats),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::test_support::fixture;

    #[test]
    fn a_span_reports_its_numbers_and_places_one_collision_slab() {
        let (p, mut w) = fixture();
        let mats = Materials::new(&p);
        let out = span(
            &mut w,
            &mats,
            &SpanReq {
                scene: None,
                room: None,
                from: [1024, 512, 2048],
                to: [5120, 512, 2048],
                width: None,
                thickness: None,
                material: Some("Graybox / Cliff 64".into()),
                materials: None,
                collide: None,
                name: None,
            },
        )
        .unwrap();
        assert!(out.contains("length 4096"), "{out}");
        assert_eq!(w.mesh_assets.len(), 1);
        let built = w.mesh_assets[0].build().unwrap();
        assert_eq!(built.proxies.len(), 1);
        assert!(w.rooms[0].meshes[0].materials.iter().all(Option::is_some));
        // The same deck again shares the asset.
        let again = SpanReq {
            scene: None,
            room: None,
            from: [1024, 512, 3072],
            to: [5120, 512, 3072],
            width: None,
            thickness: None,
            material: None,
            materials: None,
            collide: None,
            name: None,
        };
        span(&mut w, &mats, &again).unwrap();
        assert_eq!(w.mesh_assets.len(), 1);
        assert_eq!(w.rooms[0].meshes.len(), 2);
    }

    fn arch_req(from: [i32; 3], to: [i32; 3]) -> ArchSpanReq {
        ArchSpanReq {
            scene: None,
            room: None,
            from,
            to,
            profile: None,
            rise: None,
            spring: None,
            thickness: None,
            depth: None,
            segments: None,
            perch: None,
            material: Some("Graybox / Cliff 64".into()),
            materials: None,
            collide: None,
            name: None,
        }
    }

    #[test]
    fn an_arch_span_reports_its_numbers_and_the_hook() {
        let (p, mut w) = fixture();
        let mats = Materials::new(&p);
        let out = arch(&mut w, &mats, &arch_req([1024, 0, 2048], [3072, 0, 2048])).unwrap();
        assert!(out.contains("span 2048"), "{out}");
        assert!(out.contains("hook socket"), "{out}");
        assert_eq!(w.mesh_assets.len(), 1);
        assert_eq!(w.rooms[0].meshes.len(), 1);
        assert!(w.rooms[0].meshes[0].materials.iter().all(Option::is_some));
        assert!(list(&w, &mats, None).unwrap().contains("mesh 1"));
    }

    #[test]
    fn a_recipe_places_a_column_and_set_changes_the_asset_for_every_copy() {
        let (p, mut w) = fixture();
        let mats = Materials::new(&p);
        let place_req = |x: i32| PlaceMeshReq {
            scene: None,
            room: None,
            asset: None,
            recipe: Some(MeshRecipe {
                kind: "column".into(),
                ..MeshRecipe::default()
            }),
            position: [x, 0, 2048],
            rotation: None,
            yaw: Some(45),
            scale: Some(vec![1.0]),
            material: None,
            materials: None,
            collide: None,
            fullbright: None,
        };
        place(&mut w, &mats, &place_req(1024)).unwrap();
        place(&mut w, &mats, &place_req(3072)).unwrap();
        // Equal recipes share one asset.
        assert_eq!(w.mesh_assets.len(), 1);
        let out = set(
            &mut w,
            &mats,
            &SetMeshReq {
                scene: None,
                id: 1,
                position: None,
                rotation: None,
                yaw: None,
                scale: None,
                asset: None,
                asset_recipe: Some(MeshRecipe {
                    kind: "column".into(),
                    height: Some(3072),
                    ..MeshRecipe::default()
                }),
                material: None,
                materials: None,
                collide: None,
                fullbright: None,
                enabled: None,
            },
        )
        .unwrap();
        assert!(out.contains("updated"), "{out}");
        match &w.mesh_assets[0].kind {
            MeshKind::Column(c) => assert_eq!(c.height, 3072),
            other => panic!("{other:?}"),
        }
        let gone = delete(
            &mut w,
            &DeleteMeshReq {
                scene: None,
                ids: vec![1],
            },
        )
        .unwrap();
        assert!(gone.contains("removed 1"));
        assert_eq!(w.rooms[0].meshes.len(), 1);
        assert_eq!(w.rooms[0].meshes[0].id, 2);
    }

    #[test]
    fn bad_requests_say_what_to_do() {
        let (p, mut w) = fixture();
        let mats = Materials::new(&p);
        let none = PlaceMeshReq {
            scene: None,
            room: None,
            asset: None,
            recipe: None,
            position: [0; 3],
            rotation: None,
            yaw: None,
            scale: None,
            material: None,
            materials: None,
            collide: None,
            fullbright: None,
        };
        assert!(place(&mut w, &mats, &none).unwrap_err().contains("asset"));
        let mut pointed = arch_req([1024, 0, 2048], [3072, 0, 2048]);
        pointed.profile = Some("pointed".into());
        pointed.rise = Some(300);
        let err = arch(&mut w, &mats, &pointed).unwrap_err();
        assert!(err.contains("at least half"), "{err}");
        assert!(w.mesh_assets.is_empty() && w.rooms[0].meshes.is_empty());
        let mut weird = arch_req([1024, 0, 2048], [3072, 0, 2048]);
        weird.profile = Some("gothic".into());
        assert!(arch(&mut w, &mats, &weird).unwrap_err().contains("round"));
        assert!(scale_q8(&[20.0]).is_err());
        assert_eq!(scale_q8(&[2.0]).unwrap(), [512; 3]);
        assert!(scale_q8(&[1.0, 2.0]).is_err());
    }
}
