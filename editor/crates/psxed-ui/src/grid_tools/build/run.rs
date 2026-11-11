//! The documents the Build tools make: one function per kind that turns the
//! shape the pointer drew and the options of the options bar into the SDK
//! operation, and a refusal into a plain sentence.
//!
//! Everything here is pure over a [`ProjectDocument`], so the gesture can
//! restore the document and apply again at every pointer move, and the tests
//! can call it without a window.

use std::collections::BTreeSet;

use psxed_project::grid::edit::verbs::arcade::{
    build_arcade, build_colonnade, ArcadeParams, Colonnade, Layout,
};
use psxed_project::grid::edit::verbs::bridge::{build_bridge, BridgeParams};
use psxed_project::grid::edit::verbs::building::{build_building, BuildingParams};
use psxed_project::grid::edit::verbs::capture::{repeat_bay, Along};
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
use psxed_project::grid::edit::verbs::style::Roles;
use psxed_project::grid::edit::verbs::styled::{
    lay_styled_corridor, lay_styled_room, StyledCorridor, StyledRoom,
};
use psxed_project::grid::edit::verbs::terrain::{lay_terrain, Edge, TerrainParams};
use psxed_project::grid::edit::CellRect;
use psxed_project::grid::sector::WallEdge;
use psxed_project::grid::units::SECTOR_UNITS;
use psxed_project::grid::ArchProfile;
use psxed_project::{InteractableKind, NodeId, NodeKind, ProjectDocument, SwitchBehavior};

use super::options::{
    ArcProfile, BuildKind, DoorOpts, Lock, OpeningHead, Options, RoomShape, StyleResolved,
};

/// A run along a wall, as the Opening and Door tools read it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WallRun {
    pub room: usize,
    /// A sector the wall is seen from (its other axis fixes the wall).
    pub cell: [i32; 2],
    pub side: WallEdge,
    /// The storey (span) the wall belongs to.
    pub span: u8,
    /// Start and end along the wall, world clicks (end exclusive).
    pub from: i32,
    pub to: i32,
}

/// What the pointer drew.
#[derive(Clone, Debug, Default)]
pub(crate) struct Geo {
    /// The room the gesture was started in.
    pub room: usize,
    /// Floor height under the press, room relative, grid units.
    pub base: i32,
    /// Anchor and current cell of a rectangle, world sectors.
    pub a: [i32; 2],
    pub b: [i32; 2],
    /// Lasso corners in sector coordinates.
    pub poly: Vec<[f64; 2]>,
    /// Corridor waypoints and the floor height each names, if any.
    pub path: Vec<([i32; 2], Option<i32>)>,
    /// A wall run for an opening or a door.
    pub wall: Option<WallRun>,
    /// The two ends of a bridge deck: world sector, floor height.
    pub ends: Vec<([i32; 2], i32)>,
    /// The whole room instead of a rectangle (Ruin).
    pub whole_room: bool,
    /// Where a lever or checkpoint stands.
    pub spot: Option<[i32; 2]>,
    /// The vertical amount the pointer or a typed number gave, grid units.
    pub value: Option<i32>,
}

impl Geo {
    /// The rectangle from the anchor to the current cell.
    pub fn rect(&self) -> CellRect {
        CellRect::from_corners(self.a, self.b)
    }
}

/// What applying a kind did.
#[derive(Clone, Debug, Default)]
pub(crate) struct Outcome {
    /// The document changed.
    pub changed: bool,
    /// One line for the hint bar: what was made, or why not.
    pub text: String,
    /// `text` is a refusal.
    pub refused: bool,
}

impl Outcome {
    fn made(text: String) -> Self {
        Self {
            changed: true,
            text,
            refused: false,
        }
    }

    fn refusal(text: String) -> Self {
        Self {
            changed: false,
            text,
            refused: true,
        }
    }

    fn wait(text: &str) -> Self {
        Self {
            changed: false,
            text: text.to_string(),
            refused: false,
        }
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// The next free room name: "Room 2", "Corridor 3".
fn name_of(project: &ProjectDocument, stem: &str) -> String {
    let n = project
        .active_scene()
        .world
        .grid()
        .map_or(0, |w| w.rooms.len());
    format!("{stem} {}", n + 1)
}

fn target(project: &ProjectDocument, into: bool, room: usize, stem: &str) -> RoomTarget {
    if into {
        RoomTarget::Existing(room)
    } else {
        RoomTarget::New(name_of(project, stem))
    }
}

/// Apply `kind` over `geo` to `project`. The document is untouched when the
/// outcome is a refusal or when there is not enough drawn yet.
pub(crate) fn apply(
    project: &mut ProjectDocument,
    kind: BuildKind,
    o: &Options,
    geo: &Geo,
) -> Outcome {
    if project.active_scene().world.grid().is_none() {
        return Outcome::refusal("This scene has no grid world".to_string());
    }
    match kind {
        BuildKind::Room => room(project, o, geo),
        BuildKind::Corridor => corridor(project, o, geo),
        BuildKind::Opening => opening(project, o, geo),
        BuildKind::Arcade | BuildKind::Colonnade => arcade(project, kind, o, geo),
        BuildKind::Bridge => bridge(project, o, geo),
        BuildKind::Building => building(project, o, geo),
        BuildKind::Cathedral => cathedral(project, o, geo),
        BuildKind::Terrain => terrain(project, o, geo),
        BuildKind::Ruin => ruin(project, o, geo),
        BuildKind::Door => door(project, o, geo),
        BuildKind::Lever | BuildKind::Checkpoint => place_object(project, kind, o, geo),
    }
}

fn with_world<R>(
    project: &mut ProjectDocument,
    f: impl FnOnce(&mut psxed_project::grid::GridWorld, &Roles) -> R,
) -> R {
    let roles = Roles::graybox(project);
    let world = project
        .active_scene_mut()
        .world
        .grid_mut()
        .expect("checked by apply");
    f(world, &roles)
}

fn room(project: &mut ProjectDocument, o: &Options, geo: &Geo) -> Outcome {
    let rect = geo.rect();
    let opts = &o.room;
    let height = geo.value.unwrap_or(opts.height);
    let tgt = target(project, opts.into_this, geo.room, "Room");
    let styled = match &opts.style {
        StyleResolved::Plain => None,
        other => Some(other.clone()),
    };
    let base = geo.base;
    if let Some(StyleResolved::Style(style)) = styled {
        let out = with_world(project, |w, roles| {
            lay_styled_room(
                w,
                &StyledRoom {
                    target: tgt,
                    rect,
                    base,
                    style,
                    roles: roles.clone(),
                },
            )
        });
        return match out {
            Ok(r) => Outcome::made(format!(
                "Room of {} in style, {}",
                plural(r.styled, "sector", "sectors"),
                plural(r.piers, "pier", "piers")
            )),
            Err(e) => Outcome::refusal(e.describe()),
        };
    }
    let footprint = match opts.shape {
        RoomShape::Rectangle => Footprint::Rect(rect),
        RoomShape::Octagon => Footprint::Octagon {
            rect,
            cut: opts.cut,
        },
        RoomShape::Lasso => {
            if geo.poly.len() < 3 {
                return Outcome::wait("Click the corners of the room, Enter closes it");
            }
            Footprint::Polygon(geo.poly.clone())
        }
    };
    let out = with_world(project, |w, roles| {
        let mut p = RoomParams::new(footprint, "", height);
        p.target = tgt;
        p.base = base;
        p.floor_material = roles.floor;
        p.ceiling_material = roles.ceiling;
        p.wall_material = roles.wall;
        lay_room(w, &p)
    });
    match out {
        Ok(r) => Outcome::made(format!(
            "{} of {}, {} high",
            if r.created { "New room" } else { "Room" },
            plural(r.cells, "sector", "sectors"),
            clicks(height)
        )),
        Err(e) => Outcome::refusal(e.describe()),
    }
}

/// Clicks as words: "2 clicks", "1.5 clicks".
pub(crate) fn clicks(units: i32) -> String {
    let c = units as f32 / psxed_project::grid::CLICK_UNITS as f32;
    if (c - c.round()).abs() < 1e-3 {
        format!("{} clicks", c as i32)
    } else {
        format!("{c:.2} clicks")
    }
}

fn corridor(project: &mut ProjectDocument, o: &Options, geo: &Geo) -> Outcome {
    if geo.path.len() < 2 {
        return Outcome::wait("Click the next waypoint, Enter finishes");
    }
    let opts = &o.corridor;
    let waypoints: Vec<Waypoint> = geo
        .path
        .iter()
        .map(|&(cell, floor)| match floor {
            Some(f) => Waypoint::at_height(cell, f),
            None => Waypoint::at(cell),
        })
        .collect();
    let width = geo.value.map_or(opts.width, |v| v.max(1));
    let leg = if opts.stairs {
        LegShape::Stairs
    } else {
        LegShape::Ramp
    };
    let tgt = target(project, opts.into_this, geo.room, "Corridor");
    match &opts.style {
        StyleResolved::Plain => {
            let out = with_world(project, |w, roles| {
                let mut p = CorridorParams::new(tgt, waypoints, opts.headroom);
                p.width = width;
                p.base = geo.base;
                p.leg = leg;
                p.floor_material = roles.floor;
                p.ceiling_material = roles.ceiling;
                p.wall_material = roles.wall;
                lay_corridor(w, &p)
            });
            match out {
                Ok(r) => Outcome::made(format!(
                    "Corridor of {}, {} legs",
                    plural(r.cells, "sector", "sectors"),
                    r.legs.len()
                )),
                Err(e) => Outcome::refusal(e.describe()),
            }
        }
        StyleResolved::Style(style) => {
            let style = style.clone();
            let out = with_world(project, |w, roles| {
                lay_styled_corridor(
                    w,
                    &StyledCorridor {
                        target: tgt,
                        waypoints,
                        width,
                        base: geo.base,
                        leg,
                        style,
                        roles: roles.clone(),
                    },
                )
            });
            match out {
                Ok(r) => Outcome::made(format!(
                    "Corridor in style, {} sectors, {}",
                    r.styled,
                    plural(r.piers, "pier", "piers")
                )),
                Err(e) => Outcome::refusal(e.describe()),
            }
        }
        StyleResolved::Bay(template) => {
            let (first, last) = (geo.path[0].0, geo.path[geo.path.len() - 1].0);
            let (along, len) = if (last[0] - first[0]).abs() >= (last[1] - first[1]).abs() {
                (Along::X, (last[0] - first[0]).abs() + 1)
            } else {
                (Along::Z, (last[1] - first[1]).abs() + 1)
            };
            let step = match along {
                Along::X => template.size[0].max(1),
                Along::Z => template.size[1].max(1),
            };
            let count = (len / step).max(1);
            let origin = [first[0].min(last[0]), first[1].min(last[1])];
            let template = template.clone();
            let room = geo.room;
            let out = with_world(project, |w, _| {
                repeat_bay(w, room, &template, origin, along, count, geo.base)
            });
            match out {
                Ok(n) => Outcome::made(format!("{} copies of the bay, {n} cells", count)),
                Err(e) => Outcome::refusal(e.describe()),
            }
        }
    }
}

fn opening(project: &mut ProjectDocument, o: &Options, geo: &Geo) -> Outcome {
    let Some(run) = geo.wall else {
        return Outcome::wait("Drag along a wall");
    };
    let opts = &o.opening;
    let top = geo.value.unwrap_or(opts.top);
    let params = opening_params(run, opts.window, opts.head, top, opts.sill);
    let out = with_world(project, |w, _| cut_opening_on_span(w, &params, run.span));
    match out {
        Ok(r) => Outcome::made(
            format!(
                "{} cut, {} high",
                if opts.window { "Window" } else { "Door" },
                clicks(top)
            ) + &if r.mirrored {
                ", and the room next door".to_string()
            } else {
                String::new()
            },
        ),
        Err(e) => Outcome::refusal(e.describe()),
    }
}

fn opening_params(
    run: WallRun,
    window: bool,
    head: OpeningHead,
    top: i32,
    sill: i32,
) -> OpeningParams {
    let clicks = run.to - run.from;
    OpeningParams {
        profile: match head {
            OpeningHead::Rectangular => OpeningProfile::Rectangular,
            OpeningHead::Pointed => arch_head(ArchShape::Pointed, clicks, top),
            OpeningHead::Round => arch_head(ArchShape::Round, clicks, top),
        },
        room: run.room,
        cell: run.cell,
        side: run.side,
        from: run.from,
        to: run.to,
        kind: if window {
            OpeningKind::Window { sill }
        } else {
            OpeningKind::Door
        },
        top,
        material: None,
    }
}

fn arcade(project: &mut ProjectDocument, kind: BuildKind, o: &Options, geo: &Geo) -> Outcome {
    let opts = &o.arcade;
    // The line runs along the dominant axis, from the centre of the first
    // sector to the centre of the last.
    let (a, b) = (geo.a, geo.b);
    let at = |c: [i32; 2]| {
        [
            c[0] * SECTOR_UNITS + SECTOR_UNITS / 2,
            c[1] * SECTOR_UNITS + SECTOR_UNITS / 2,
        ]
    };
    let (from, to) = if (b[0] - a[0]).abs() >= (b[1] - a[1]).abs() {
        let z = at(a)[1];
        ([at(a)[0], z], [at(b)[0], z])
    } else {
        let x = at(a)[0];
        ([x, at(a)[1]], [x, at(b)[1]])
    };
    if from == to {
        return Outcome::wait("Drag a line along the floor");
    }
    let bays = geo.value.map_or(opts.bays, |v| v.clamp(1, 32));
    let room = geo.room;
    let base = geo.base;
    let out = if kind == BuildKind::Arcade {
        let mut p = ArcadeParams::line(room, from, to, base, bays);
        p.span = (opts.span > 0).then_some(opts.span);
        p.pier = opts.pier;
        p.spring = opts.spring;
        p.rise = (opts.rise > 0).then_some(opts.rise);
        p.profile = match opts.profile {
            ArcProfile::Round => ArchProfile::Round,
            ArcProfile::Pointed => ArchProfile::Pointed,
            ArcProfile::Segmental => ArchProfile::Segmental,
            ArcProfile::Flat => ArchProfile::Flat,
        };
        p.perch = opts.perch;
        with_world(project, |w, _| build_arcade(w, &p)).map_err(|e| e.describe())
    } else {
        let c = Colonnade {
            room,
            layout: Layout::Line { from, to },
            base,
            bays,
            column: Default::default(),
        };
        with_world(project, |w, _| build_colonnade(w, &c)).map_err(|e| e.describe())
    };
    match out {
        Ok(r) => Outcome::made(format!(
            "{} of {}",
            if kind == BuildKind::Arcade {
                "Arcade"
            } else {
                "Colonnade"
            },
            plural(
                if kind == BuildKind::Arcade {
                    bays as usize
                } else {
                    r.centres.len()
                },
                if kind == BuildKind::Arcade {
                    "bay"
                } else {
                    "column"
                },
                if kind == BuildKind::Arcade {
                    "bays"
                } else {
                    "columns"
                },
            )
        )),
        Err(e) => Outcome::refusal(e),
    }
}

fn bridge(project: &mut ProjectDocument, o: &Options, geo: &Geo) -> Outcome {
    if geo.ends.len() < 2 {
        return Outcome::wait("Click the far end of the deck");
    }
    let opts = &o.bridge;
    let end = |(cell, y): ([i32; 2], i32)| {
        [
            cell[0] * SECTOR_UNITS + SECTOR_UNITS / 2,
            y,
            cell[1] * SECTOR_UNITS + SECTOR_UNITS / 2,
        ]
    };
    let width = geo.value.map_or(opts.width, |v| v.max(256));
    let mut p = BridgeParams::deck(
        RoomTarget::New(name_of(project, "Bridge")),
        end(geo.ends[0]),
        end(geo.ends[1]),
        width,
        opts.thickness,
    );
    p.parapet = (opts.parapet > 0).then_some(opts.parapet);
    p.pier_spacing = (opts.pier_spacing > 0).then_some(opts.pier_spacing);
    match with_world(project, |w, _| build_bridge(w, &p)) {
        Ok(r) => Outcome::made(format!(
            "Bridge {} long, {} piers",
            clicks(r.length),
            r.piers.len()
        )),
        Err(e) => Outcome::refusal(e.describe()),
    }
}

fn building(project: &mut ProjectDocument, o: &Options, geo: &Geo) -> Outcome {
    let opts = &o.building;
    let rect = geo.rect();
    let tgt = RoomTarget::New(name_of(project, "Building"));
    let storeys = geo.value.map_or(opts.storeys, |v| v.clamp(1, 4));
    let out = with_world(project, |w, roles| {
        let mut p = BuildingParams::new(tgt, Footprint::Rect(rect));
        p.base = geo.base;
        p.storeys = storeys;
        p.storey_height = opts.storey_height;
        p.door_sides = if opts.door {
            vec![WallEdge::South]
        } else {
            Vec::new()
        };
        p.window_every = opts.window_every;
        p.window_arch = match opts.window_head {
            OpeningHead::Rectangular => None,
            OpeningHead::Pointed => Some(ArchShape::Pointed),
            OpeningHead::Round => Some(ArchShape::Round),
        };
        p.stair = opts.stair && storeys > 1;
        p.roof_parapet = opts.parapet.then_some(768);
        p.seed = opts.seed;
        p.floor_material = roles.floor;
        p.ceiling_material = roles.ceiling;
        p.wall_material = roles.wall;
        build_building(w, &p)
    });
    match out {
        Ok(r) => Outcome::made(format!(
            "Building of {}, {} windows",
            plural(storeys as usize, "storey", "storeys"),
            r.windows
        )),
        Err(e) => Outcome::refusal(e.describe()),
    }
}

fn cathedral(project: &mut ProjectDocument, o: &Options, geo: &Geo) -> Outcome {
    let opts = &o.cathedral;
    let rect = geo.rect();
    let bay = geo.value.map_or(opts.bay, |v| v.clamp(1, 4));
    // The long side is the nave: its length is the bays, its depth minus the
    // aisles the nave's width.
    let bays = (rect.width() / bay).clamp(1, 16);
    let aisles = opts.aisle_width * 2;
    let nave_width = (rect.depth() - aisles).max(1);
    let tgt = RoomTarget::New(name_of(project, "Cathedral"));
    let style = match &opts.style {
        StyleResolved::Style(s) => Some(s.clone()),
        _ => None,
    };
    let out = with_world(project, |w, roles| {
        let mut p = CathedralParams::new(tgt, [rect.min[0], rect.min[1]]);
        p.base = geo.base;
        p.bays = bays;
        p.bay = bay;
        p.nave_width = nave_width;
        p.aisle_width = opts.aisle_width;
        p.nave_height = opts.nave_height;
        p.transept = opts.transept.then_some(Transept {
            at_bay: (bays / 2).max(1),
            arm: 2,
        });
        p.apse = opts.apse;
        p.west_door = opts.west_door;
        p.roles = style.is_some().then(|| roles.clone());
        p.style = style;
        p.floor_material = roles.floor;
        p.ceiling_material = roles.ceiling;
        p.wall_material = roles.wall;
        build_cathedral(w, &p)
    });
    match out {
        Ok(r) => Outcome::made(format!(
            "Cathedral of {}, {} arches, {} windows",
            plural(bays as usize, "bay", "bays"),
            r.arches,
            r.windows
        )),
        Err(e) => Outcome::refusal(e.describe()),
    }
}

fn terrain(project: &mut ProjectDocument, o: &Options, geo: &Geo) -> Outcome {
    let opts = &o.terrain;
    let cells: BTreeSet<[i32; 2]> = geo.rect().cells().collect();
    let room = geo.room;
    let out = with_world(project, |w, _| {
        let mut p = TerrainParams::new(room, cells, opts.seed);
        p.amplitude_clicks = geo.value.map_or(opts.amplitude, |v| (v / 256).max(0));
        p.octaves = opts.octaves;
        p.wavelength = opts.wavelength;
        p.slope = [opts.slope_x, opts.slope_z];
        p.keep_walkable = opts.keep_walkable;
        p.edge = if opts.blend > 0 {
            Edge::Blend(opts.blend)
        } else {
            Edge::Cliff
        };
        lay_terrain(w, &p)
    });
    match out {
        Ok(r) => Outcome::made(format!(
            "Terrain over {}, steepest {} per sector",
            plural(r.cells, "sector", "sectors"),
            r.steepest
        )),
        Err(e) => Outcome::refusal(e.describe()),
    }
}

fn ruin(project: &mut ProjectDocument, o: &Options, geo: &Geo) -> Outcome {
    let opts = &o.ruin;
    let room = geo.room;
    let cells: BTreeSet<[i32; 2]> = if geo.whole_room {
        project
            .active_scene()
            .world
            .grid()
            .and_then(|w| w.rooms.get(room))
            .map(|r| {
                r.leaves()
                    .flat_map(|(k, _)| {
                        let n = k.edge_clicks();
                        let s = psxed_project::grid::leaf::CLICKS_PER_SECTOR;
                        let (x0, z0) = (k.x.div_euclid(s), k.z.div_euclid(s));
                        let (x1, z1) = ((k.x + n - 1).div_euclid(s), (k.z + n - 1).div_euclid(s));
                        (x0..=x1).flat_map(move |x| (z0..=z1).map(move |z| [x, z]))
                    })
                    .collect()
            })
            .unwrap_or_default()
    } else {
        geo.rect().cells().collect()
    };
    let intensity = geo
        .value
        .map_or(opts.intensity, |v| (v / 256).clamp(0, 100) as u32);
    let out = with_world(project, |w, _| {
        let mut p = RuinParams::new(room, cells, opts.seed, intensity);
        p.keep = opts.keep.clone();
        p.crumble = opts.crumble;
        p.sink = opts.sink;
        p.floor_bays = opts.floor_bays;
        p.roof_bays = opts.roof_bays;
        p.breaches = opts.breaches;
        p.fallen_arches = opts.fallen_arches;
        p.rubble = opts.rubble;
        ruin_region(w, &p)
    });
    match out {
        Ok(r) => Outcome::made(format!(
            "Ruin at {intensity}: {} sunk, {} floor bays, {} roof bays, {} breaches",
            r.sunk, r.floor_bays, r.roof_bays, r.breaches
        )),
        Err(e) => Outcome::refusal(e.describe()),
    }
}

/// The door's own name: "Door 1", "Door 2", the first free one.
pub(crate) fn free_name(project: &ProjectDocument, stem: &str) -> String {
    let scene = project.active_scene();
    let mut n = 1;
    loop {
        let name = format!("{stem} {n}");
        if scene.nodes().iter().all(|node| node.name != name) {
            return name;
        }
        n += 1;
    }
}

fn door(project: &mut ProjectDocument, o: &Options, geo: &Geo) -> Outcome {
    let Some(run) = geo.wall else {
        return Outcome::wait("Drag along a wall");
    };
    let opts: &DoorOpts = &o.door;
    let top = geo.value.unwrap_or(opts.height);
    let opening = opening_params(run, false, opts.head, top, 0);
    let name = free_name(project, "Door");
    let (lock, refusal) = match opts.lock {
        Lock::Open => (psxed_project::DoorLock::Unlocked, String::new()),
        Lock::Locked => (psxed_project::DoorLock::Locked, opts.refusal.clone()),
        Lock::OneWay(side) => (
            psxed_project::DoorLock::OneWay { refuse_from: side },
            opts.refusal.clone(),
        ),
    };
    let params = DoorwayDoor {
        opening,
        name: name.clone(),
        material: Roles::graybox(project).accent,
        direction: opts.direction,
        ticks: opts.ticks,
        lock,
        refusal,
        persistent: opts.persistent,
        lever: None,
    };
    let scene = project.active_scene_mut();
    let Some(mut world) = scene.world.grid().cloned() else {
        return Outcome::refusal("This scene has no grid world".to_string());
    };
    let result = doorway_with_door(&mut world, scene, &params);
    match result {
        Ok(out) => {
            *scene
                .world
                .grid_mut()
                .expect("the world was there a line ago") = world;
            // The distance the slab slides is the one the options ask for.
            if let Some(node) = scene.node_mut(out.door) {
                if let NodeKind::Logic {
                    kind: psxed_project::LogicNodeKind::Door { slide: Some(s), .. },
                    ..
                } = &mut node.kind
                {
                    if opts.distance > 0 {
                        s.distance = opts.distance.clamp(1, i32::from(u16::MAX)) as u16;
                    }
                }
            }
            Outcome::made(format!("{name}: doorway cut and a sliding door placed"))
        }
        Err(e) => Outcome::refusal(e.describe()),
    }
}

/// A lever or a checkpoint standing in a sector. The lever is made unbound;
/// the tool binds it to a door with the next click.
fn place_object(project: &mut ProjectDocument, kind: BuildKind, o: &Options, geo: &Geo) -> Outcome {
    let Some(cell) = geo.spot else {
        return Outcome::wait("Click a floor");
    };
    let Some(world) = project.active_scene().world.grid() else {
        return Outcome::refusal("This scene has no grid world".to_string());
    };
    let scale = world.scale;
    let y = world.rooms.get(geo.room).map_or(0, |r| r.y_offset) + geo.base;
    let ed = |g: i32| scale.to_editor(g) as f32;
    let at = [
        ed(cell[0] * SECTOR_UNITS + SECTOR_UNITS / 2),
        ed(y),
        ed(cell[1] * SECTOR_UNITS + SECTOR_UNITS / 2),
    ];
    let name = free_name(
        project,
        if kind == BuildKind::Lever {
            "Lever"
        } else {
            "Checkpoint"
        },
    );
    let (component, prompt, radius) = if kind == BuildKind::Lever {
        (
            InteractableKind::Switch {
                target: String::new(),
                behavior: o.lever.behavior,
                unlocks: o.lever.unlocks,
                persistent: o.lever.persistent,
            },
            "PULL".to_string(),
            640,
        )
    } else {
        (
            InteractableKind::Checkpoint {
                checkpoint_id: name.clone(),
                title: "SYNC RELAY".to_string(),
                body: "Relay synchronized.".to_string(),
            },
            "SYNCHRONIZE".to_string(),
            640,
        )
    };
    let scene = project.active_scene_mut();
    let id = scene.add_node(NodeId::ROOT, &name, NodeKind::Entity);
    if let Some(n) = scene.node_mut(id) {
        n.transform.translation = at;
    }
    scene.add_node(
        id,
        "Interactable",
        NodeKind::Interactable {
            kind: component,
            prompt,
            radius,
            enabled: true,
        },
    );
    Outcome::made(if kind == BuildKind::Lever {
        format!("{name} placed: click the door it operates")
    } else {
        format!("{name} placed")
    })
}

/// The door (a Logic Door node) nearest `cell` within a few sectors, by
/// name. The lever tool binds with it.
pub(crate) fn nearest_door(project: &ProjectDocument, cell: [i32; 2]) -> Option<(NodeId, String)> {
    let world = project.active_scene().world.grid()?;
    let scene = project.active_scene();
    let per_sector = world.scale.to_editor(SECTOR_UNITS).max(1) as f32;
    let mut best: Option<(f32, NodeId, String)> = None;
    for node in scene.nodes() {
        let NodeKind::Logic {
            kind: psxed_project::LogicNodeKind::Door { box_prop, .. },
            ..
        } = &node.kind
        else {
            continue;
        };
        // A door stands where its slab does.
        let slab = scene
            .nodes()
            .iter()
            .find(|n| &n.name == box_prop)
            .map_or(node.transform.translation, |n| n.transform.translation);
        let d = ((slab[0] / per_sector - (cell[0] as f32 + 0.5)).powi(2)
            + (slab[2] / per_sector - (cell[1] as f32 + 0.5)).powi(2))
        .sqrt();
        if d <= 3.0 && best.as_ref().is_none_or(|b| d < b.0) {
            best = Some((d, node.id, node.name.clone()));
        }
    }
    best.map(|(_, id, name)| (id, name))
}

/// Point the lever `lever` (the entity node) at the door called `door`.
pub(crate) fn bind_lever(project: &mut ProjectDocument, lever: NodeId, door: &str, o: &Options) {
    let scene = project.active_scene_mut();
    let kids: Vec<NodeId> = scene
        .node(lever)
        .map_or_else(Vec::new, |n| n.children.clone());
    for id in kids {
        if let Some(node) = scene.node_mut(id) {
            if let NodeKind::Interactable {
                kind:
                    InteractableKind::Switch {
                        target,
                        behavior,
                        unlocks,
                        persistent,
                    },
                ..
            } = &mut node.kind
            {
                *target = door.to_string();
                *behavior = o.lever.behavior;
                *unlocks = o.lever.unlocks;
                *persistent = o.lever.persistent;
            }
        }
    }
}

#[allow(dead_code)]
const _: SwitchBehavior = SwitchBehavior::Toggle;
