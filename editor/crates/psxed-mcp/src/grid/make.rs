//! The Build group: rooms, corridors, openings, arcades, bridges, buildings,
//! cathedrals, terrain patches, ruins, doors and levers, with styles. Thin
//! wrappers over the SDK operations the editor's Build tools call, with the
//! operations' own parameters, defaults and refusal sentences.

use std::collections::BTreeSet;

use psxed_project::grid::edit::verbs::arcade::{
    build_arcade, build_colonnade, ArcadeParams, Colonnade, Layout,
};
use psxed_project::grid::edit::verbs::bridge::{build_bridge, BridgeParams};
use psxed_project::grid::edit::verbs::building::{build_building, BuildingParams};
use psxed_project::grid::edit::verbs::capture::capture_bay;
use psxed_project::grid::edit::verbs::cathedral::{build_cathedral, CathedralParams, Transept};
use psxed_project::grid::edit::verbs::corridor::{
    lay_corridor, CorridorParams, LegShape, Waypoint,
};
use psxed_project::grid::edit::verbs::doorway_door::{doorway_with_door, DoorwayDoor};
use psxed_project::grid::edit::verbs::opening::{
    arch_head, cut_opening_on_span, ArchShape, OpeningKind, OpeningParams, OpeningProfile,
};
use psxed_project::grid::edit::verbs::room::{lay_room, Footprint, RoomParams, RoomTarget};
use psxed_project::grid::edit::verbs::ruin::{ruin_region, RuinParams};
use psxed_project::grid::edit::verbs::style::{built_in, Roles, Style};
use psxed_project::grid::edit::verbs::styled::{
    lay_styled_corridor, lay_styled_room, StyledCorridor, StyledRoom,
};
use psxed_project::grid::edit::verbs::terrain::{lay_terrain, Edge, TerrainParams};
use psxed_project::grid::sector::WallEdge;
use psxed_project::grid::units::SECTOR_UNITS;
use psxed_project::grid::ArchProfile;
use psxed_project::{
    DoorDirection, DoorLock, DoorSide, InteractableKind, NodeId, NodeKind, ProjectDocument,
    SwitchBehavior,
};
use schemars::JsonSchema;
use serde::Deserialize;

use super::{check_rect, pick_room, Cells};

fn world_of(project: &mut ProjectDocument, scene: Option<usize>) -> Result<usize, String> {
    let index = crate::resolve_scene(project, scene)?;
    project.scenes[index]
        .world
        .grid()
        .ok_or_else(|| format!("scene {index} has no grid world"))?;
    Ok(index)
}

fn roles(project: &ProjectDocument) -> Roles {
    Roles::graybox(project)
}

/// The project's styles by name: the built-ins, then the `styles/` folder of
/// the project directory.
fn find_style(root: &std::path::Path, name: &str) -> Result<Style, String> {
    let want = name.trim().to_lowercase();
    if let Some(s) = built_in()
        .into_iter()
        .find(|s| s.name.to_lowercase() == want)
    {
        return Ok(s);
    }
    let path = root.join("styles").join(format!("{name}.style.ron"));
    let text = std::fs::read_to_string(&path).map_err(|_| {
        format!("no style called {name}; grid_style_list names the built-in and project styles")
    })?;
    Style::from_ron(&text)
}

fn target(room: Option<usize>, name: &str) -> RoomTarget {
    match room {
        Some(r) => RoomTarget::Existing(r),
        None => RoomTarget::New(name.to_string()),
    }
}

/// A style by name, or plain.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct StyleName {
    /// A built-in style (plain, sci-fi rib, gothic nave, cloister, crypt,
    /// domed chamber) or one stored with the project. Omit for plain.
    pub style: Option<String>,
}

/// A rectangular room with a ceiling and walls.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RoomReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Make the room inside this existing derived room. Omit for a new room.
    pub room: Option<usize>,
    /// Name of a new room (default "Room N").
    pub name: Option<String>,
    /// The rectangle of WORLD sectors.
    #[serde(flatten)]
    pub cells: Cells,
    /// `rectangle` (default), `octagon` (cut corners, see cut) or `polygon`
    /// (corners in sector coordinates, see polygon).
    pub shape: Option<String>,
    /// Octagon: sectors cut from each corner (default 2).
    pub cut: Option<i32>,
    /// Polygon: corners `[x, z]` in sector coordinates.
    pub polygon: Option<Vec<[f64; 2]>>,
    /// Floor height, grid units relative to the room's offset (default 0).
    pub base: Option<i32>,
    /// Floor to ceiling, grid units (default 3072).
    pub height: Option<i32>,
    /// A style name: the room is made with the style's vault over each bay.
    #[serde(flatten)]
    pub style: StyleName,
}

/// Make a room.
pub fn room(
    project: &mut ProjectDocument,
    root: &std::path::Path,
    r: &RoomReq,
) -> Result<String, String> {
    let index = world_of(project, r.scene)?;
    let rect = r.cells.rect();
    check_rect(&rect)?;
    let roles = roles(project);
    let tgt = target(
        r.room,
        r.name.as_deref().unwrap_or(&format!(
            "Room {}",
            project.scenes[index]
                .world
                .grid()
                .map_or(0, |w| w.rooms.len())
                + 1
        )),
    );
    let base = r.base.unwrap_or(0);
    let world = project.scenes[index].world.grid_mut().expect("checked");
    if let Some(style) = r
        .style
        .style
        .as_deref()
        .filter(|s| !s.eq_ignore_ascii_case("plain"))
    {
        let style = find_style(root, style)?;
        let out = lay_styled_room(
            world,
            &StyledRoom {
                target: tgt,
                rect,
                base,
                style,
                roles,
            },
        )
        .map_err(|e| e.describe())?;
        return Ok(format!(
            "Styled room: {} sectors, {} piers, room {}",
            out.styled, out.piers, out.room
        ));
    }
    let footprint = match r.shape.as_deref().unwrap_or("rectangle") {
        "rectangle" => Footprint::Rect(rect),
        "octagon" => Footprint::Octagon {
            rect,
            cut: r.cut.unwrap_or(2),
        },
        "polygon" => Footprint::Polygon(
            r.polygon
                .clone()
                .ok_or("shape polygon needs polygon corners")?,
        ),
        other => {
            return Err(format!(
                "unknown shape {other}; use rectangle, octagon or polygon"
            ))
        }
    };
    let mut p = RoomParams::new(footprint, "", r.height.unwrap_or(3072));
    p.target = tgt;
    p.base = base;
    p.floor_material = roles.floor;
    p.ceiling_material = roles.ceiling;
    p.wall_material = roles.wall;
    let out = lay_room(world, &p).map_err(|e| e.describe())?;
    Ok(format!(
        "{} room {}: {} sectors, {} leaves, {} overwritten",
        if out.created { "New" } else { "Existing" },
        out.room,
        out.cells,
        out.leaves,
        out.overwritten
    ))
}

/// A corridor through waypoints.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CorridorReq {
    /// Scene index.
    pub scene: Option<usize>,
    /// Lay it in this existing derived room. Omit for a new room.
    pub room: Option<usize>,
    /// Name of a new room.
    pub name: Option<String>,
    /// Waypoints, WORLD sectors `[x, z]`; legs run along X or Z.
    pub waypoints: Vec<[i32; 2]>,
    /// Floor height at each waypoint (grid units); omit an entry with null to
    /// keep the running height. Same length as waypoints when given.
    pub heights: Option<Vec<Option<i32>>>,
    /// Width in sectors (default 1).
    pub width: Option<i32>,
    /// Floor to ceiling, grid units (default 2048; a style brings its own).
    pub headroom: Option<i32>,
    /// Floor height of the first waypoint when it names none.
    pub base: Option<i32>,
    /// Where heights change: `ramp` (default) or `stairs`.
    pub leg: Option<String>,
    /// A style name; the corridor takes its profile, rhythm and vault.
    #[serde(flatten)]
    pub style: StyleName,
}

/// Lay a corridor.
pub fn corridor(
    project: &mut ProjectDocument,
    root: &std::path::Path,
    r: &CorridorReq,
) -> Result<String, String> {
    let index = world_of(project, r.scene)?;
    let roles = roles(project);
    let n = project.scenes[index]
        .world
        .grid()
        .map_or(0, |w| w.rooms.len());
    let tgt = target(
        r.room,
        r.name.as_deref().unwrap_or(&format!("Corridor {}", n + 1)),
    );
    let waypoints: Vec<Waypoint> = r
        .waypoints
        .iter()
        .enumerate()
        .map(
            |(i, &c)| match r.heights.as_ref().and_then(|h| h.get(i)).copied().flatten() {
                Some(f) => Waypoint::at_height(c, f),
                None => Waypoint::at(c),
            },
        )
        .collect();
    let leg = match r.leg.as_deref().unwrap_or("ramp") {
        "ramp" => LegShape::Ramp,
        "stairs" => LegShape::Stairs,
        other => return Err(format!("unknown leg {other}; use ramp or stairs")),
    };
    let world = project.scenes[index].world.grid_mut().expect("checked");
    if let Some(style) = r
        .style
        .style
        .as_deref()
        .filter(|s| !s.eq_ignore_ascii_case("plain"))
    {
        let style = find_style(root, style)?;
        let out = lay_styled_corridor(
            world,
            &StyledCorridor {
                target: tgt,
                waypoints,
                width: r.width.unwrap_or(1),
                base: r.base.unwrap_or(0),
                leg,
                style,
                roles,
            },
        )
        .map_err(|e| e.describe())?;
        return Ok(format!(
            "Styled corridor in room {}: {} sectors, {} piers",
            out.room, out.styled, out.piers
        ));
    }
    let mut p = CorridorParams::new(tgt, waypoints, r.headroom.unwrap_or(2048));
    p.width = r.width.unwrap_or(1);
    p.base = r.base.unwrap_or(0);
    p.leg = leg;
    p.floor_material = roles.floor;
    p.ceiling_material = roles.ceiling;
    p.wall_material = roles.wall;
    let out = lay_corridor(world, &p).map_err(|e| e.describe())?;
    Ok(format!(
        "Corridor in room {}: {} sectors, {} kept, {} legs",
        out.room,
        out.cells,
        out.kept,
        out.legs.len()
    ))
}

/// A door or window cut in a wall.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct OpeningReq {
    /// Scene index.
    pub scene: Option<usize>,
    /// The room that holds the wall.
    pub room: usize,
    /// A sector the wall is seen from, WORLD `[x, z]`.
    pub cell: [i32; 2],
    /// `north`, `east`, `south` or `west`.
    pub side: String,
    /// Start along the wall, world clicks (4 a sector).
    pub from: i32,
    /// End along the wall, exclusive.
    pub to: i32,
    /// `door` (default) or `window`.
    pub kind: Option<String>,
    /// `rectangular` (default), `pointed` or `round`.
    pub profile: Option<String>,
    /// Lintel height above the floor, grid units (default 2048); the apex of
    /// an arch.
    pub top: Option<i32>,
    /// Window sill, grid units (default 768).
    pub sill: Option<i32>,
    /// The storey (span) of a column of several floors (default 0).
    pub span: Option<u8>,
}

fn edge_of(s: &str) -> Result<WallEdge, String> {
    Ok(match s.to_lowercase().as_str() {
        "north" => WallEdge::North,
        "east" => WallEdge::East,
        "south" => WallEdge::South,
        "west" => WallEdge::West,
        other => {
            return Err(format!(
                "unknown side {other}; use north, east, south or west"
            ))
        }
    })
}

fn opening_params(r: &OpeningReq) -> Result<OpeningParams, String> {
    let top = r.top.unwrap_or(2048);
    let clicks = r.to - r.from;
    Ok(OpeningParams {
        profile: match r.profile.as_deref().unwrap_or("rectangular") {
            "rectangular" => OpeningProfile::Rectangular,
            "pointed" => arch_head(ArchShape::Pointed, clicks, top),
            "round" => arch_head(ArchShape::Round, clicks, top),
            other => {
                return Err(format!(
                    "unknown profile {other}; use rectangular, pointed or round"
                ))
            }
        },
        room: r.room,
        cell: r.cell,
        side: edge_of(&r.side)?,
        from: r.from,
        to: r.to,
        kind: match r.kind.as_deref().unwrap_or("door") {
            "door" => OpeningKind::Door,
            "window" => OpeningKind::Window {
                sill: r.sill.unwrap_or(768),
            },
            other => return Err(format!("unknown kind {other}; use door or window")),
        },
        top,
        material: None,
    })
}

/// Cut an opening.
pub fn opening(project: &mut ProjectDocument, r: &OpeningReq) -> Result<String, String> {
    let index = world_of(project, r.scene)?;
    let params = opening_params(r)?;
    let world = project.scenes[index].world.grid_mut().expect("checked");
    let out = cut_opening_on_span(world, &params, r.span.unwrap_or(0)).map_err(|e| e.describe())?;
    Ok(format!(
        "Cut {} segments, {} pieces{}",
        out.segments,
        out.pieces,
        if out.mirrored {
            ", and the room next door"
        } else {
            ""
        }
    ))
}

/// A run of arches on piers, or a row of columns.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ArcadeReq {
    /// Scene index.
    pub scene: Option<usize>,
    /// The room the instances go in.
    pub room: Option<usize>,
    /// Start of the line, grid units `[x, z]` (a sector is 1024).
    pub from: [i32; 2],
    /// End of the line.
    pub to: [i32; 2],
    /// Floor height under the piers (default 0).
    pub base: Option<i32>,
    /// Bays (default 4).
    pub bays: Option<i32>,
    /// Clear opening of a bay, grid units; omit to fill the pitch.
    pub span: Option<i32>,
    /// Pier width (default 256).
    pub pier: Option<i32>,
    /// Springing line height above the base (default 1536).
    pub spring: Option<i32>,
    /// Crown rise; omit for the profile's.
    pub rise: Option<i32>,
    /// `round` (default), `pointed`, `segmental` or `flat`.
    pub profile: Option<String>,
    /// A hook socket on each crown.
    pub perch: Option<bool>,
}

fn profile_of(s: Option<&str>) -> Result<ArchProfile, String> {
    Ok(match s.unwrap_or("round") {
        "round" => ArchProfile::Round,
        "pointed" => ArchProfile::Pointed,
        "segmental" => ArchProfile::Segmental,
        "flat" => ArchProfile::Flat,
        other => {
            return Err(format!(
                "unknown profile {other}; use round, pointed, segmental or flat"
            ))
        }
    })
}

/// Build an arcade.
pub fn arcade(project: &mut ProjectDocument, r: &ArcadeReq) -> Result<String, String> {
    let index = world_of(project, r.scene)?;
    let world = project.scenes[index].world.grid_mut().expect("checked");
    let room = pick_room(world, r.room)?;
    let mut p = ArcadeParams::line(room, r.from, r.to, r.base.unwrap_or(0), r.bays.unwrap_or(4));
    p.span = r.span;
    p.pier = r.pier.unwrap_or(p.pier);
    p.spring = r.spring.unwrap_or(p.spring);
    p.rise = r.rise;
    p.profile = profile_of(r.profile.as_deref())?;
    p.perch = r.perch.unwrap_or(false);
    let out = build_arcade(world, &p).map_err(|e| e.describe())?;
    Ok(format!(
        "Arcade: {} instances, pitch {}",
        out.instances.len(),
        out.pitch
    ))
}

/// Build a colonnade.
pub fn colonnade(project: &mut ProjectDocument, r: &ArcadeReq) -> Result<String, String> {
    let index = world_of(project, r.scene)?;
    let world = project.scenes[index].world.grid_mut().expect("checked");
    let room = pick_room(world, r.room)?;
    let c = Colonnade {
        room,
        layout: Layout::Line {
            from: r.from,
            to: r.to,
        },
        base: r.base.unwrap_or(0),
        bays: r.bays.unwrap_or(4),
        column: Default::default(),
    };
    let out = build_colonnade(world, &c).map_err(|e| e.describe())?;
    Ok(format!("Colonnade: {} columns", out.instances.len()))
}

/// A bridge deck with parapets and piers.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BridgeReq {
    /// Scene index.
    pub scene: Option<usize>,
    /// Name of the bridge's own room.
    pub name: Option<String>,
    /// Start of the deck top, grid units `[x, y, z]`.
    pub from: [i32; 3],
    /// End of the deck top.
    pub to: [i32; 3],
    /// Deck width (default 1024).
    pub width: Option<i32>,
    /// Deck thickness (default 256).
    pub thickness: Option<i32>,
    /// Parapet height above the deck; omit for none.
    pub parapet: Option<i32>,
    /// Distance between piers; omit for none.
    pub pier_spacing: Option<i32>,
}

/// Build a bridge.
pub fn bridge(project: &mut ProjectDocument, r: &BridgeReq) -> Result<String, String> {
    let index = world_of(project, r.scene)?;
    let world = project.scenes[index].world.grid_mut().expect("checked");
    let name = r
        .name
        .clone()
        .unwrap_or_else(|| format!("Bridge {}", world.rooms.len() + 1));
    let mut p = BridgeParams::deck(
        RoomTarget::New(name),
        r.from,
        r.to,
        r.width.unwrap_or(1024),
        r.thickness.unwrap_or(256),
    );
    p.parapet = r.parapet;
    p.pier_spacing = r.pier_spacing;
    let out = build_bridge(world, &p).map_err(|e| e.describe())?;
    Ok(format!(
        "Bridge in room {}: length {}, rise {}, {} piers",
        out.room,
        out.length,
        out.rise,
        out.piers.len()
    ))
}

/// A building with storeys, doors, windows and a stair.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BuildingReq {
    /// Scene index.
    pub scene: Option<usize>,
    /// Name of its room.
    pub name: Option<String>,
    /// The footprint, WORLD sectors.
    #[serde(flatten)]
    pub cells: Cells,
    /// Ground floor height (default 0).
    pub base: Option<i32>,
    /// Storeys, 1 to 4 (default 2).
    pub storeys: Option<i32>,
    /// Floor to floor, grid units (default 3072, at least 2048).
    pub storey_height: Option<i32>,
    /// Sides with a door: any of north, east, south, west (default south).
    pub door_sides: Option<Vec<String>>,
    /// Door height (default 2048).
    pub door_height: Option<i32>,
    /// A window every this many sectors, 0 for none (default 2).
    pub window_every: Option<i32>,
    /// `rectangular` (default), `pointed` or `round` window heads.
    pub window_profile: Option<String>,
    /// An interior stair (default true).
    pub stair: Option<bool>,
    /// Roof parapet height; 0 for none (default 768).
    pub parapet: Option<i32>,
    /// Moves the doors and the window phase.
    pub seed: Option<u32>,
}

/// Build a building.
pub fn building(project: &mut ProjectDocument, r: &BuildingReq) -> Result<String, String> {
    let index = world_of(project, r.scene)?;
    let roles = roles(project);
    let rect = r.cells.rect();
    check_rect(&rect)?;
    let world = project.scenes[index].world.grid_mut().expect("checked");
    let name = r
        .name
        .clone()
        .unwrap_or_else(|| format!("Building {}", world.rooms.len() + 1));
    let mut p = BuildingParams::new(RoomTarget::New(name), Footprint::Rect(rect));
    p.base = r.base.unwrap_or(0);
    p.storeys = r.storeys.unwrap_or(2);
    p.storey_height = r.storey_height.unwrap_or(3072);
    if let Some(sides) = &r.door_sides {
        p.door_sides = sides.iter().map(|s| edge_of(s)).collect::<Result<_, _>>()?;
    }
    p.door_height = r.door_height.unwrap_or(2048);
    p.window_every = r.window_every.unwrap_or(2);
    p.window_arch = match r.window_profile.as_deref().unwrap_or("rectangular") {
        "rectangular" => None,
        "pointed" => Some(ArchShape::Pointed),
        "round" => Some(ArchShape::Round),
        other => return Err(format!("unknown window_profile {other}")),
    };
    p.stair = r.stair.unwrap_or(true);
    p.roof_parapet = match r.parapet.unwrap_or(768) {
        0 => None,
        v => Some(v),
    };
    p.seed = r.seed.unwrap_or(0);
    p.floor_material = roles.floor;
    p.ceiling_material = roles.ceiling;
    p.wall_material = roles.wall;
    let out = build_building(world, &p).map_err(|e| e.describe())?;
    Ok(format!(
        "Building in room {}: {} sectors, {} slabs, {} treads, {} doors, {} windows{}",
        out.room,
        out.cells,
        out.slabs,
        out.treads,
        out.doors.len(),
        out.windows,
        if out.notes.is_empty() {
            String::new()
        } else {
            format!("; {}", out.notes.join("; "))
        }
    ))
}

/// A cathedral.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CathedralReq {
    /// Scene index.
    pub scene: Option<usize>,
    /// Name of its room.
    pub name: Option<String>,
    /// North-west corner sector of the whole plan (the north aisle).
    pub origin: [i32; 2],
    /// Nave floor height (default 0).
    pub base: Option<i32>,
    /// Bays of the nave (default 5).
    pub bays: Option<i32>,
    /// Sectors per bay (default 2).
    pub bay: Option<i32>,
    /// Nave width in sectors (default 4).
    pub nave_width: Option<i32>,
    /// Aisle width in sectors, 0 for none (default 2).
    pub aisle_width: Option<i32>,
    /// Nave height (default 9216).
    pub nave_height: Option<i32>,
    /// Aisle height (default 5120).
    pub aisle_height: Option<i32>,
    /// `round`, `pointed` (default), `segmental` or `flat` arcade.
    pub profile: Option<String>,
    /// Clerestory windows (default true).
    pub clerestory: Option<bool>,
    /// Transept: the bay it crosses at and the arm length.
    pub transept_at_bay: Option<i32>,
    /// Transept arm length in sectors (default 2).
    pub transept_arm: Option<i32>,
    /// A polygonal apse (default true).
    pub apse: Option<bool>,
    /// A west door (default true).
    pub west_door: Option<bool>,
    /// Chancel rise (default 512).
    pub chancel_rise: Option<i32>,
    /// A nave style (see grid_style_list).
    #[serde(flatten)]
    pub style: StyleName,
}

/// Build a cathedral.
pub fn cathedral(
    project: &mut ProjectDocument,
    root: &std::path::Path,
    r: &CathedralReq,
) -> Result<String, String> {
    let index = world_of(project, r.scene)?;
    let rl = roles(project);
    let style = match r
        .style
        .style
        .as_deref()
        .filter(|s| !s.eq_ignore_ascii_case("plain"))
    {
        Some(s) => Some(find_style(root, s)?),
        None => None,
    };
    let world = project.scenes[index].world.grid_mut().expect("checked");
    let name = r
        .name
        .clone()
        .unwrap_or_else(|| format!("Cathedral {}", world.rooms.len() + 1));
    let mut p = CathedralParams::new(RoomTarget::New(name), r.origin);
    p.base = r.base.unwrap_or(0);
    p.bays = r.bays.unwrap_or(p.bays);
    p.bay = r.bay.unwrap_or(p.bay);
    p.nave_width = r.nave_width.unwrap_or(p.nave_width);
    p.aisle_width = r.aisle_width.unwrap_or(p.aisle_width);
    p.nave_height = r.nave_height.unwrap_or(p.nave_height);
    p.aisle_height = r.aisle_height.unwrap_or(p.aisle_height);
    p.profile = profile_of(r.profile.as_deref().or(Some("pointed")))?;
    p.clerestory = r.clerestory.unwrap_or(true);
    p.transept = r.transept_at_bay.map(|at_bay| Transept {
        at_bay,
        arm: r.transept_arm.unwrap_or(2),
    });
    p.apse = r.apse.unwrap_or(true);
    p.west_door = r.west_door.unwrap_or(true);
    p.chancel_rise = r.chancel_rise.unwrap_or(p.chancel_rise);
    p.roles = style.is_some().then_some(rl);
    p.style = style;
    p.floor_material = rl.floor;
    p.ceiling_material = rl.ceiling;
    p.wall_material = rl.wall;
    let out = build_cathedral(world, &p).map_err(|e| e.describe())?;
    Ok(format!(
        "Cathedral in room {}: {} sectors, {} arches, {} windows",
        out.room, out.cells, out.arches, out.windows
    ))
}

/// A terrain patch.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TerrainReq {
    /// Scene index.
    pub scene: Option<usize>,
    /// Room index.
    pub room: Option<usize>,
    /// The region, WORLD sectors.
    #[serde(flatten)]
    pub cells: Cells,
    /// Another seed is a re-roll (default 1).
    pub seed: Option<u32>,
    /// Peak relief of the first octave, clicks (default 1).
    pub amplitude_clicks: Option<i32>,
    /// Octaves, 1 to 4 (default 2).
    pub octaves: Option<u32>,
    /// Wavelength of the first octave, sectors (default 4).
    pub wavelength: Option<i32>,
    /// Constant slope, grid units per sector along x.
    pub slope_x: Option<i32>,
    /// Constant slope along z.
    pub slope_z: Option<i32>,
    /// Floor height of the patch; omit to add relief to the floors that exist.
    pub base: Option<i32>,
    /// Make floors where a cell has none (needs base).
    pub create: Option<bool>,
    /// Clamp neighbouring corners to the walkable slope (default true).
    pub keep_walkable: Option<bool>,
    /// Sectors the relief fades over at the border; 0 for a cliff (default 2).
    pub edge_blend: Option<i32>,
}

/// Lay a terrain patch.
pub fn terrain(project: &mut ProjectDocument, r: &TerrainReq) -> Result<String, String> {
    let index = world_of(project, r.scene)?;
    let rect = r.cells.rect();
    check_rect(&rect)?;
    let world = project.scenes[index].world.grid_mut().expect("checked");
    let room = pick_room(world, r.room)?;
    let cells: BTreeSet<[i32; 2]> = rect.cells().collect();
    let mut p = TerrainParams::new(room, cells, r.seed.unwrap_or(1));
    p.amplitude_clicks = r.amplitude_clicks.unwrap_or(p.amplitude_clicks);
    p.octaves = r.octaves.unwrap_or(p.octaves);
    p.wavelength = r.wavelength.unwrap_or(p.wavelength);
    p.slope = [r.slope_x.unwrap_or(0), r.slope_z.unwrap_or(0)];
    p.base = r.base;
    p.create = r.create.unwrap_or(false);
    p.keep_walkable = r.keep_walkable.unwrap_or(true);
    p.edge = match r.edge_blend.unwrap_or(2) {
        0 => Edge::Cliff,
        n => Edge::Blend(n),
    };
    let out = lay_terrain(world, &p).map_err(|e| e.describe())?;
    Ok(format!(
        "Terrain over {} sectors ({} created), range {}..{}, steepest {} per sector, {} corners clamped",
        out.cells, out.created, out.range.0, out.range.1, out.steepest, out.clamped
    ))
}

/// A ruin over a region or a whole room.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RuinReq {
    /// Scene index.
    pub scene: Option<usize>,
    /// Room index.
    pub room: Option<usize>,
    /// The region, WORLD sectors. Omit for the whole room.
    #[serde(flatten)]
    pub cells: Option<Cells>,
    /// Another seed is a re-roll (default 1).
    pub seed: Option<u32>,
    /// 0 to 100 (default 40).
    pub intensity: Option<u32>,
    /// Sectors no damage touches: the path kept clear.
    pub keep: Option<Vec<[i32; 2]>>,
    /// Toggles, all on by default: crumble, sink, floor_bays, roof_bays,
    /// breaches, fallen_arches, rubble.
    pub off: Option<Vec<String>>,
}

/// Ruin a region.
pub fn ruin(project: &mut ProjectDocument, r: &RuinReq) -> Result<String, String> {
    let index = world_of(project, r.scene)?;
    let world = project.scenes[index].world.grid_mut().expect("checked");
    let room = pick_room(world, r.room)?;
    let cells: BTreeSet<[i32; 2]> = match &r.cells {
        Some(c) => {
            let rect = c.rect();
            check_rect(&rect)?;
            rect.cells().collect()
        }
        None => {
            let s = psxed_project::grid::leaf::CLICKS_PER_SECTOR;
            world.rooms[room]
                .leaves()
                .flat_map(|(k, _)| {
                    let n = k.edge_clicks();
                    let (x0, z0) = (k.x.div_euclid(s), k.z.div_euclid(s));
                    let (x1, z1) = ((k.x + n - 1).div_euclid(s), (k.z + n - 1).div_euclid(s));
                    (x0..=x1).flat_map(move |x| (z0..=z1).map(move |z| [x, z]))
                })
                .collect()
        }
    };
    let mut p = RuinParams::new(room, cells, r.seed.unwrap_or(1), r.intensity.unwrap_or(40));
    p.keep = r.keep.clone().unwrap_or_default().into_iter().collect();
    for off in r.off.iter().flatten() {
        match off.as_str() {
            "crumble" => p.crumble = false,
            "sink" => p.sink = false,
            "floor_bays" => p.floor_bays = false,
            "roof_bays" => p.roof_bays = false,
            "breaches" => p.breaches = false,
            "fallen_arches" => p.fallen_arches = false,
            "rubble" => p.rubble = false,
            other => return Err(format!("unknown toggle {other} in off")),
        }
    }
    let out = ruin_region(world, &p).map_err(|e| e.describe())?;
    Ok(format!(
        "Ruin: {} sunk, {} floor bays, {} roof bays, {} breaches, {} arches fallen, {} rubble, {} reverted",
        out.sunk, out.floor_bays, out.roof_bays, out.breaches, out.arches, out.rubble, out.reverted
    ))
}

/// The styles a corridor, room or cathedral can take.
pub fn style_list(root: &std::path::Path) -> String {
    let mut out = String::from("Built in:\n");
    for s in built_in() {
        out += &format!("  {} (headroom {})\n", s.name, s.headroom);
    }
    out += "Stored with the project (styles/ folder):\n";
    let mut any = false;
    if let Ok(read) = std::fs::read_dir(root.join("styles")) {
        for e in read.flatten() {
            if let Some(f) = e.file_name().to_str() {
                if let Some(n) = f.strip_suffix(".style.ron") {
                    out += &format!("  {n}\n");
                    any = true;
                } else if let Some(n) = f.strip_suffix(".bay.ron") {
                    out += &format!("  {n} (captured bay)\n");
                    any = true;
                }
            }
        }
    }
    if !any {
        out += "  none\n";
    }
    out
}

/// Capture a bay as a style.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CaptureReq {
    /// Scene index.
    pub scene: Option<usize>,
    /// Room index.
    pub room: Option<usize>,
    /// The bay's rectangle, WORLD sectors.
    #[serde(flatten)]
    pub cells: Cells,
    /// The name it is stored under.
    pub name: String,
}

/// Capture the bay and store it in the project's `styles/` folder.
pub fn capture_style(
    project: &ProjectDocument,
    root: &std::path::Path,
    r: &CaptureReq,
) -> Result<String, String> {
    let index = crate::resolve_scene(project, r.scene)?;
    let world = project.scenes[index].world.grid().ok_or("no grid world")?;
    let room = pick_room(world, r.room)?;
    let bay = capture_bay(world, room, r.cells.rect()).map_err(|e| e.describe())?;
    let name: String = r
        .name
        .trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if name.is_empty() {
        return Err("name the style".to_string());
    }
    let text = ron::ser::to_string_pretty(&bay, ron::ser::PrettyConfig::default())
        .map_err(|e| e.to_string())?;
    let dir = root.join("styles");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(format!("{name}.bay.ron")), text).map_err(|e| e.to_string())?;
    Ok(format!(
        "Captured {} leaves as \"{name}\" in {}",
        bay.leaves.len(),
        dir.display()
    ))
}

/// A doorway with its sliding door.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DoorReq {
    /// The opening: room, cell, side, from, to, profile, top, span.
    #[serde(flatten)]
    pub opening: OpeningReq,
    /// The door's name (default "Door N").
    pub name: Option<String>,
    /// `up` (default), `down`, `east`, `west`, `south` or `north`.
    pub direction: Option<String>,
    /// Slide distance, editor units; omit to slide clear of the doorway.
    pub distance: Option<u16>,
    /// 60 Hz ticks from closed to open (default 60).
    pub ticks: Option<u16>,
    /// `open` (default), `locked` or `one_way`.
    pub lock: Option<String>,
    /// one_way: the side it refuses from (north, east, south, west).
    pub refuse_from: Option<String>,
    /// The text shown when it refuses.
    pub refusal: Option<String>,
    /// Remember its state across death and saves (default true).
    pub persistent: Option<bool>,
}

fn side_of(s: &str) -> Result<DoorSide, String> {
    Ok(match s.to_lowercase().as_str() {
        "north" => DoorSide::North,
        "east" => DoorSide::East,
        "south" => DoorSide::South,
        "west" => DoorSide::West,
        other => return Err(format!("unknown side {other}")),
    })
}

/// Cut the doorway and place the door.
pub fn door(project: &mut ProjectDocument, r: &DoorReq) -> Result<String, String> {
    let index = world_of(project, r.opening.scene)?;
    let opening = opening_params(&r.opening)?;
    let name = r.name.clone().unwrap_or_else(|| {
        let scene = &project.scenes[index];
        (1..)
            .map(|n| format!("Door {n}"))
            .find(|n| scene.nodes().iter().all(|node| &node.name != n))
            .unwrap_or_default()
    });
    let direction = match r.direction.as_deref().unwrap_or("up") {
        "up" => DoorDirection::Up,
        "down" => DoorDirection::Down,
        "east" => DoorDirection::East,
        "west" => DoorDirection::West,
        "south" => DoorDirection::South,
        "north" => DoorDirection::North,
        other => return Err(format!("unknown direction {other}")),
    };
    let lock = match r.lock.as_deref().unwrap_or("open") {
        "open" => DoorLock::Unlocked,
        "locked" => DoorLock::Locked,
        "one_way" => DoorLock::OneWay {
            refuse_from: side_of(
                r.refuse_from
                    .as_deref()
                    .ok_or("one_way needs refuse_from")?,
            )?,
        },
        other => return Err(format!("unknown lock {other}; use open, locked or one_way")),
    };
    let material = Roles::graybox(project).accent;
    let params = DoorwayDoor {
        opening,
        name: name.clone(),
        material,
        direction,
        ticks: r.ticks.unwrap_or(60),
        lock,
        refusal: r.refusal.clone().unwrap_or_default(),
        persistent: r.persistent.unwrap_or(true),
        lever: None,
    };
    let scene = &mut project.scenes[index];
    let mut world = scene.world.grid().cloned().ok_or("no grid world")?;
    let out = doorway_with_door(&mut world, scene, &params).map_err(|e| e.describe())?;
    *scene.world.grid_mut().expect("there a line ago") = world;
    if let Some(d) = r.distance {
        if let Some(node) = scene.node_mut(out.door) {
            if let NodeKind::Logic {
                kind: psxed_project::LogicNodeKind::Door { slide: Some(s), .. },
                ..
            } = &mut node.kind
            {
                s.distance = d.max(1);
            }
        }
    }
    Ok(format!(
        "{name}: doorway cut, sliding door placed (slab and door nodes)"
    ))
}

/// A lever that operates a door.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct LeverReq {
    /// Scene index.
    pub scene: Option<usize>,
    /// The sector it stands in, WORLD `[x, z]`.
    pub cell: [i32; 2],
    /// Floor height it stands at, grid units (default 0).
    pub height: Option<i32>,
    /// The name of the door (or other logic node) it operates.
    pub operates: String,
    /// `toggle` (default) or `one_shot`.
    pub behavior: Option<String>,
    /// Unlock the door instead of opening it.
    pub unlocks: Option<bool>,
    /// Remember across death and saves (default true).
    pub persistent: Option<bool>,
    /// Name of the lever (default "Lever N").
    pub name: Option<String>,
}

/// Place a lever.
pub fn lever(project: &mut ProjectDocument, r: &LeverReq) -> Result<String, String> {
    let index = world_of(project, r.scene)?;
    let scene = &mut project.scenes[index];
    if !scene.nodes().iter().any(|n| n.name == r.operates) {
        return Err(format!(
            "no node called {}; place the door first",
            r.operates
        ));
    }
    let scale = scene.world.grid().ok_or("no grid world")?.scale;
    let ed = |g: i32| scale.to_editor(g) as f32;
    let name = r.name.clone().unwrap_or_else(|| {
        (1..)
            .map(|n| format!("Lever {n}"))
            .find(|n| scene.nodes().iter().all(|node| &node.name != n))
            .unwrap_or_default()
    });
    let id = scene.add_node(NodeId::ROOT, &name, NodeKind::Entity);
    if let Some(n) = scene.node_mut(id) {
        n.transform.translation = [
            ed(r.cell[0] * SECTOR_UNITS + SECTOR_UNITS / 2),
            ed(r.height.unwrap_or(0)),
            ed(r.cell[1] * SECTOR_UNITS + SECTOR_UNITS / 2),
        ];
    }
    let behavior = match r.behavior.as_deref().unwrap_or("toggle") {
        "toggle" => SwitchBehavior::Toggle,
        "one_shot" => SwitchBehavior::OneShot,
        other => return Err(format!("unknown behavior {other}; use toggle or one_shot")),
    };
    scene.add_node(
        id,
        "Interactable",
        NodeKind::Interactable {
            kind: InteractableKind::Switch {
                target: r.operates.clone(),
                behavior,
                unlocks: r.unlocks.unwrap_or(false),
                persistent: r.persistent.unwrap_or(true),
            },
            prompt: "PULL".to_string(),
            radius: 640,
            enabled: true,
        },
    );
    Ok(format!("{name} placed, operates {}", r.operates))
}

/// A wall, addressed by the sector that sees it, its side and its position
/// among the walls on that side (storeys bottom to top).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct WallReq {
    /// Scene index.
    pub scene: Option<usize>,
    /// Room index.
    pub room: usize,
    /// The sector the wall is seen from, WORLD `[x, z]`.
    pub cell: [i32; 2],
    /// `north`, `east`, `south` or `west`.
    pub side: String,
    /// Which wall on that side (default 0, the first).
    pub index: Option<usize>,
    /// grid_wall_set: the four corner heights `[bottom near, bottom far, top
    /// far, top near]`, grid units relative to the room; null keeps a corner.
    pub heights: Option<[Option<i32>; 4]>,
    /// grid_wall_reset: which property to take back to automatic: `heights`,
    /// `material`, `collision`, `bands` or `all` (default all).
    pub property: Option<String>,
}

fn wall_view(
    world: &psxed_project::grid::GridWorld,
    r: &WallReq,
) -> Result<psxed_project::grid::edit::wall_api::WallView, String> {
    use psxed_project::grid::edit::wall_api::walls_at;
    use psxed_project::grid::LeafKey;
    let side = edge_of(&r.side)?;
    let views = walls_at(world, r.room, LeafKey::sector(r.cell), side);
    views
        .into_iter()
        .nth(r.index.unwrap_or(0))
        .ok_or_else(|| "no wall there; grid_wall_get lists the walls on a side".to_string())
}

fn prop_word<T>(p: &psxed_project::grid::edit::wall_api::Prop<T>) -> &'static str {
    match p {
        psxed_project::grid::edit::wall_api::Prop::Set(_) => "set",
        _ => "automatic",
    }
}

/// Read the walls on one side of a sector.
pub fn wall_get(project: &ProjectDocument, r: &WallReq) -> Result<String, String> {
    use psxed_project::grid::edit::wall_api::walls_at;
    use psxed_project::grid::LeafKey;
    let index = crate::resolve_scene(project, r.scene)?;
    let world = project.scenes[index].world.grid().ok_or("no grid world")?;
    let side = edge_of(&r.side)?;
    let views = walls_at(world, r.room, LeafKey::sector(r.cell), side);
    if views.is_empty() {
        return Ok("no wall on that side".to_string());
    }
    let mut out = String::new();
    for (i, v) in views.iter().enumerate() {
        out += &format!(
            "wall {i}: heights {:?} ({}), material {} ({}), collision {} ({}), floor {}{}\n",
            v.heights.value(),
            prop_word(&v.heights),
            if v.material.value().is_some() {
                "set"
            } else {
                "none"
            },
            prop_word(&v.material),
            if v.solid.value() { "solid" } else { "open" },
            prop_word(&v.solid),
            v.addr.span + 1,
            if v.both_sides {
                ", shared with the room next door"
            } else {
                ""
            },
        );
    }
    Ok(out)
}

/// Set the heights of a wall; the floors stay.
pub fn wall_set(project: &mut ProjectDocument, r: &WallReq) -> Result<String, String> {
    use psxed_project::grid::edit::wall_api::set_wall_heights;
    let index = world_of(project, r.scene)?;
    let corners = r.heights.ok_or("heights is required")?;
    let world = project.scenes[index].world.grid_mut().expect("checked");
    let view = wall_view(world, r)?;
    let (_, warnings) =
        set_wall_heights(world, view.addr, corners).map_err(|e| format!("{e:?}"))?;
    Ok(if warnings.is_empty() {
        "Wall heights set".to_string()
    } else {
        format!("Wall heights set; it leaves a gap ({warnings:?})")
    })
}

/// Take a wall back to automatic.
pub fn wall_reset(project: &mut ProjectDocument, r: &WallReq) -> Result<String, String> {
    use psxed_project::grid::edit::wall_api::{clear_wall_property, WallProp};
    let index = world_of(project, r.scene)?;
    let world = project.scenes[index].world.grid_mut().expect("checked");
    let view = wall_view(world, r)?;
    let props: Vec<WallProp> = match r.property.as_deref().unwrap_or("all") {
        "heights" => vec![WallProp::Heights],
        "material" => vec![WallProp::Material],
        "collision" => vec![WallProp::Solid],
        "bands" => vec![WallProp::Bands],
        "all" => vec![
            WallProp::Heights,
            WallProp::Material,
            WallProp::Solid,
            WallProp::Bands,
        ],
        other => return Err(format!("unknown property {other}")),
    };
    for p in props {
        clear_wall_property(world, view.addr, p).map_err(|e| format!("{e:?}"))?;
    }
    Ok("Wall reset to automatic".to_string())
}

/// Move cells, and what stands on them.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct MoveReq {
    /// Scene index.
    pub scene: Option<usize>,
    /// Room index.
    pub room: Option<usize>,
    /// The cells, WORLD sectors.
    #[serde(flatten)]
    pub cells: Cells,
    /// Whole sectors east (positive X) and south (positive Z).
    pub delta: [i32; 2],
    /// Lattice steps up (positive) or down; a step is 64 grid units.
    pub dy_steps: Option<i32>,
    /// Replace the cells already where the selection lands.
    pub overwrite: Option<bool>,
}

/// Move cells with their walls, meshes, planes and nodes.
pub fn move_cells(project: &mut ProjectDocument, r: &MoveReq) -> Result<String, String> {
    use psxed_project::grid::edit::move_cells::{move_cells, MoveParams};
    let index = world_of(project, r.scene)?;
    let rect = r.cells.rect();
    check_rect(&rect)?;
    let scene = &mut project.scenes[index];
    let mut world = scene.world.grid().cloned().ok_or("no grid world")?;
    let room = pick_room(&world, r.room)?;
    let p = MoveParams {
        room,
        cells: rect.cells().collect(),
        delta: r.delta,
        dy_steps: r.dy_steps.unwrap_or(0),
        overwrite: r.overwrite.unwrap_or(false),
    };
    let out = move_cells(&mut world, Some(scene), &p).map_err(|e| e.describe())?;
    *scene.world.grid_mut().expect("there a line ago") = world;
    Ok(format!(
        "Moved {} leaves ({} replaced), {} meshes, {} planes, {} wall overrides, {} nodes",
        out.leaves, out.replaced, out.meshes, out.planes, out.overrides, out.nodes
    ))
}

/// Lean a region by an offset ramp.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TiltReq {
    /// Scene index.
    pub scene: Option<usize>,
    /// Room index.
    pub room: Option<usize>,
    /// The cells, WORLD sectors.
    #[serde(flatten)]
    pub cells: Cells,
    /// `floor` (default), `ceiling` or `both`.
    pub target: Option<String>,
    /// The side that stays: `north`, `east`, `south` or `west`; the far side
    /// gets the whole rise.
    pub hinge: String,
    /// Offset at the far side, grid units (negative leans down).
    pub rise: i32,
    /// Apply even where a floor ends up too steep to stand on.
    pub force: Option<bool>,
}

/// Tilt cells by an offset, keeping their shape.
pub fn tilt(project: &mut ProjectDocument, r: &TiltReq) -> Result<String, String> {
    use psxed_project::grid::edit::tilt::{tilt_cells, Hinge, TiltParams, TiltTarget};
    use psxed_project::grid::edit::verbs::ramp::RampDir;
    let index = world_of(project, r.scene)?;
    let rect = r.cells.rect();
    check_rect(&rect)?;
    let world = project.scenes[index].world.grid_mut().expect("checked");
    let room = pick_room(world, r.room)?;
    let hinge = match r.hinge.to_lowercase().as_str() {
        "north" => RampDir::North,
        "east" => RampDir::East,
        "south" => RampDir::South,
        "west" => RampDir::West,
        other => {
            return Err(format!(
                "unknown hinge {other}; use north, east, south or west"
            ))
        }
    };
    let target = match r.target.as_deref().unwrap_or("floor") {
        "floor" => TiltTarget::Floor,
        "ceiling" => TiltTarget::Ceiling,
        "both" => TiltTarget::Both,
        other => return Err(format!("unknown target {other}")),
    };
    let p = TiltParams {
        room,
        cells: rect.cells().collect(),
        target,
        hinge: Hinge::Edge(hinge),
        rise: r.rise,
        force: r.force.unwrap_or(false),
    };
    let out = tilt_cells(world, &p).map_err(|e| e.describe())?;
    Ok(format!(
        "Tilted {} leaves, {} too steep",
        out.tilted, out.steep
    ))
}
