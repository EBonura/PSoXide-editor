//! Room-level edits: add, delete, raise, ambient, shading, wall policy.

use psxed_project::grid::edit::{clean_overrides, convert_to_derived, ConvertMode};
use psxed_project::grid::limits::MAX_ROOMS;
use psxed_project::grid::units::{snap_to, FINE_UNITS};
use psxed_project::grid::{FacingShade, GridRoom, GridWorld};
use schemars::JsonSchema;
use serde::Deserialize;

use super::{clicks, pick_room, Materials};

/// Add an empty derived room, the way the Rooms panel's `+` does.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddRoomReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room name. Default "Room N".
    pub name: Option<String>,
    /// World sector `[x, z]` of the room's first cell. Rooms may overlap in
    /// plan, which is how floors stack. Default `[0, 0]`.
    pub origin: Option<[i32; 2]>,
    /// Vertical offset of the whole room in grid units (256 per click),
    /// snapped to the 64 fine step. Default 0.
    pub y_offset: Option<i32>,
    /// Ambient light per channel, 128 shows textures unmodified. Default 96.
    pub ambient: Option<[u8; 3]>,
}

/// Add a room.
pub fn add_room(world: &mut GridWorld, req: &AddRoomReq) -> Result<String, String> {
    if world.rooms.len() >= MAX_ROOMS {
        return Err(format!("a world holds at most {MAX_ROOMS} rooms"));
    }
    let index = world.rooms.len();
    let name = req.name.clone().unwrap_or_else(|| format!("Room {index}"));
    let mut room = GridRoom::new_derived(name.clone(), 1, 1);
    room.origin = req.origin.unwrap_or([0, 0]);
    let wanted = req.y_offset.unwrap_or(0);
    room.y_offset = snap_to(wanted, FINE_UNITS);
    if let Some(ambient) = req.ambient {
        room.ambient = ambient;
    }
    let mut out = format!(
        "added room {index} {name:?}: derived walls, origin {:?}, y_offset {} ({} clicks), ambient {:?}. It has no floor yet; use grid_add_surface with room {index}.",
        room.origin,
        room.y_offset,
        clicks(room.y_offset),
        room.ambient
    );
    if room.y_offset != wanted {
        out.push_str(&format!(
            "\ny_offset {wanted} snapped to {} (fine step {FINE_UNITS})",
            room.y_offset
        ));
    }
    world.rooms.push(room);
    Ok(out)
}

/// Delete a room.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteRoomReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Later rooms shift down by one.
    pub room: usize,
}

/// Delete a room; the last room of a world stays, as in the editor.
pub fn delete_room(world: &mut GridWorld, req: &DeleteRoomReq) -> Result<String, String> {
    let index = pick_room(world, Some(req.room))?;
    if world.rooms.len() <= 1 {
        return Err("the last room of a world cannot be deleted".to_string());
    }
    let removed = world.rooms.remove(index);
    Ok(format!(
        "deleted room {index} {:?} ({} sectors). Rooms after it are renumbered: call grid_rooms.",
        removed.name,
        removed.sector_count()
    ))
}

/// Change a room's settings.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetRoomReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Required when the world has several rooms.
    pub room: Option<usize>,
    /// New name.
    pub name: Option<String>,
    /// Raise: vertical offset of the whole room, grid units, snapped to 64.
    pub y_offset: Option<i32>,
    /// Ambient light per channel, 128 shows textures unmodified.
    pub ambient: Option<[u8; 3]>,
    /// Material the room's derived walls wear when nothing more specific is
    /// set ("none" clears it). The palette's wall material.
    pub wall_material: Option<String>,
    /// Open wall height: how tall the closure wall around a floor with NO
    /// ceiling is, grid units. 0 leaves such edges open (the outdoor look).
    pub open_wall: Option<i32>,
    /// Drop wall overrides that no longer belong to any derived wall (after
    /// a platform was lowered again).
    pub clean_overrides: Option<bool>,
}

/// Apply [`SetRoomReq`].
pub fn set_room(
    world: &mut GridWorld,
    mats: &Materials<'_>,
    req: &SetRoomReq,
) -> Result<String, String> {
    let index = pick_room(world, req.room)?;
    let mut notes = Vec::new();
    let material = match req.wall_material.as_deref() {
        None => None,
        Some(name) => Some(mats.resolve_opt(Some(name))?),
    };
    if let Some(open) = req.open_wall {
        if open < 0 {
            return Err("open_wall must be 0 or more".to_string());
        }
    }
    let room = &mut world.rooms[index];
    if let Some(name) = &req.name {
        room.name = name.clone();
        notes.push(format!("name {name:?}"));
    }
    if let Some(y) = req.y_offset {
        room.y_offset = snap_to(y, FINE_UNITS);
        notes.push(format!(
            "y_offset {} ({} clicks)",
            room.y_offset,
            clicks(room.y_offset)
        ));
    }
    if let Some(ambient) = req.ambient {
        room.ambient = ambient;
        notes.push(format!("ambient {ambient:?}"));
    }
    if let Some(material) = material {
        room.defaults.wall_material = material;
        notes.push(format!("wall material {}", mats.label(material)));
    }
    if let Some(open) = req.open_wall {
        room.defaults.open_wall = open;
        notes.push(format!("open wall height {open}"));
    }
    if req.clean_overrides.unwrap_or(false) {
        let removed = clean_overrides(world, index);
        notes.push(format!("removed {removed} unused wall override(s)"));
    }
    if notes.is_empty() {
        return Err("nothing to change: give at least one field".to_string());
    }
    Ok(format!("room {index}: {}", notes.join(", ")))
}

/// Convert a legacy room to derived walls.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ConvertRoomReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Room index. Required when the world has several rooms.
    pub room: Option<usize>,
    /// `preserve_look` (default) keeps the room looking and playing exactly
    /// as it did; `fill` also shows the walls the legacy room never had.
    pub mode: Option<String>,
}

/// Convert a room to derived walls (the only migration there is).
pub fn convert_room(world: &mut GridWorld, req: &ConvertRoomReq) -> Result<String, String> {
    let index = pick_room(world, req.room)?;
    let mode = match req.mode.as_deref() {
        None | Some("preserve_look") | Some("preserve") => ConvertMode::PreserveLook,
        Some("fill") | Some("fill_new_walls") => ConvertMode::FillNewWalls,
        Some(other) => return Err(format!("unknown mode {other:?}; use preserve_look or fill")),
    };
    if world.rooms[index].wall_mode == psxed_project::grid::overrides::WallMode::Derived {
        return Ok(format!("room {index} is already derived; nothing to do"));
    }
    let report = convert_to_derived(world, index, mode)
        .ok_or_else(|| format!("room {index} could not be converted"))?;
    Ok(format!(
        "room {index} is now derived: {} authored walls became derived faces, {} stayed authored, {} faces hidden to keep the look, {} new faces shown",
        report.converted, report.kept_authored, report.hidden, report.filled
    ))
}

/// Set the world's facing shade.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ShadingReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// `off`, `subtle` (what a new world gets) or `strong`. With the numeric
    /// fields below, they override the preset one by one.
    pub preset: Option<String>,
    /// Factor for floors, Q8 (256 = unshaded).
    pub up: Option<u16>,
    /// Factor for ceilings, Q8.
    pub down: Option<u16>,
    /// Factor for walls facing east or west, Q8.
    pub side_x: Option<u16>,
    /// Factor for walls facing north or south, Q8.
    pub side_z: Option<u16>,
}

/// Set the world's streaming radii.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct StreamReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// Draw distance in sectors, walked over the portals: a room farther
    /// than this from the camera is not drawn and is not needed in memory.
    /// 0 is unbounded. Default 48.
    pub draw_far: Option<u8>,
    /// Entities in rooms within this many sectors of the player think.
    /// Default 32.
    pub active_radius: Option<u8>,
    /// Sectors past the draw distance a streamer prefetches. Default 8.
    pub lead_radius: Option<u8>,
}

/// Apply [`StreamReq`].
pub fn set_stream(world: &mut GridWorld, req: &StreamReq) -> Result<String, String> {
    if req.draw_far.is_none() && req.active_radius.is_none() && req.lead_radius.is_none() {
        return Err("nothing to change: give draw_far, active_radius or lead_radius".to_string());
    }
    let mut radii = world.stream;
    if let Some(v) = req.draw_far {
        radii.draw_far = v;
    }
    if let Some(v) = req.active_radius {
        radii.active_radius = v;
    }
    if let Some(v) = req.lead_radius {
        radii.lead_radius = v;
    }
    world.stream = radii;
    Ok(format!(
        "stream radii: draw {} sector(s){}, active {}, lead {}",
        radii.draw_far,
        if radii.draw_far == 0 {
            " (unbounded)"
        } else {
            ""
        },
        radii.active_radius,
        radii.lead_radius
    ))
}

/// Apply [`ShadingReq`].
pub fn set_shading(world: &mut GridWorld, req: &ShadingReq) -> Result<String, String> {
    let mut shade = match req.preset.as_deref() {
        None => world.shading,
        Some("off") => FacingShade::OFF,
        Some("subtle") => FacingShade::SUBTLE,
        Some("strong") => FacingShade::STRONG,
        Some(other) => {
            return Err(format!(
                "unknown preset {other:?}; use off, subtle or strong"
            ))
        }
    };
    if let Some(v) = req.up {
        shade.up = v;
    }
    if let Some(v) = req.down {
        shade.down = v;
    }
    if let Some(v) = req.side_x {
        shade.side_x = v;
    }
    if let Some(v) = req.side_z {
        shade.side_z = v;
    }
    if req.preset.is_none()
        && req.up.is_none()
        && req.down.is_none()
        && req.side_x.is_none()
        && req.side_z.is_none()
    {
        return Err("nothing to change: give a preset or a factor".to_string());
    }
    world.shading = shade;
    Ok(format!(
        "facing shade: up {} down {} side_x {} side_z {} (Q8, 256 = unshaded){}",
        shade.up,
        shade.down,
        shade.side_x,
        shade.side_z,
        if shade.is_off() { " = off" } else { "" }
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use psxed_project::ProjectDocument;

    fn world() -> (ProjectDocument, GridWorld) {
        let p = crate::grid::test_support::new_grid_project();
        let w = p.active_scene().world.grid().cloned().unwrap();
        (p, w)
    }

    #[test]
    fn rooms_are_added_raised_and_deleted() {
        let (p, mut w) = world();
        let mats = Materials::new(&p);
        let report = add_room(
            &mut w,
            &AddRoomReq {
                scene: None,
                name: Some("Loft".into()),
                origin: Some([2, 2]),
                y_offset: Some(1000),
                ambient: None,
            },
        )
        .unwrap();
        assert!(report.contains("snapped"), "{report}");
        assert_eq!(w.rooms.len(), 2);
        assert_eq!(w.rooms[1].y_offset, 1024);
        set_room(
            &mut w,
            &mats,
            &SetRoomReq {
                scene: None,
                room: Some(1),
                name: None,
                y_offset: Some(2048),
                ambient: Some([128; 3]),
                wall_material: None,
                open_wall: None,
                clean_overrides: None,
            },
        )
        .unwrap();
        assert_eq!((w.rooms[1].y_offset, w.rooms[1].ambient), (2048, [128; 3]));
        assert!(delete_room(
            &mut w,
            &DeleteRoomReq {
                scene: None,
                room: 1
            }
        )
        .is_ok());
        assert!(delete_room(
            &mut w,
            &DeleteRoomReq {
                scene: None,
                room: 0
            }
        )
        .is_err());
    }

    #[test]
    fn a_legacy_room_converts_and_keeps_its_look() {
        use psxed_project::grid::{GridSurface, GridWall};
        let mut room = GridRoom::new("old", 2, 1);
        for x in 0..2 {
            let sector = room.sector_or_insert(x, 0).unwrap();
            sector.set_floor(Some(GridSurface::flat(0, None)));
            sector.set_ceiling(Some(GridSurface::flat(1024, None)));
            sector.north.push(GridWall::span(0, 1024, None));
        }
        let mut world = GridWorld {
            rooms: vec![room],
            ..GridWorld::default()
        };
        let before = psxed_project::grid::faces::all_faces(&world).len();
        let report = convert_room(
            &mut world,
            &ConvertRoomReq {
                scene: None,
                room: None,
                mode: None,
            },
        )
        .unwrap();
        assert!(report.contains("now derived"), "{report}");
        assert!(
            report.contains("2 authored walls became derived faces"),
            "{report}"
        );
        assert_eq!(
            world.rooms[0].wall_mode,
            psxed_project::grid::overrides::WallMode::Derived
        );
        // The look is kept: nothing new appears (new walls are hidden).
        let visible = psxed_project::grid::faces::all_faces(&world).len();
        assert!(visible <= before, "{visible} faces after, {before} before");
        let again = convert_room(
            &mut world,
            &ConvertRoomReq {
                scene: None,
                room: None,
                mode: Some("fill".into()),
            },
        )
        .unwrap();
        assert!(again.contains("already derived"), "{again}");
        assert!(convert_room(
            &mut world,
            &ConvertRoomReq {
                scene: None,
                room: None,
                mode: Some("sideways".into()),
            },
        )
        .is_err());
    }

    #[test]
    fn shading_presets_apply() {
        let (_, mut w) = world();
        let r = set_shading(
            &mut w,
            &ShadingReq {
                scene: None,
                preset: Some("off".into()),
                up: None,
                down: None,
                side_x: None,
                side_z: None,
            },
        )
        .unwrap();
        assert!(r.contains("= off"), "{r}");
        assert!(w.shading.is_off());
    }
}
