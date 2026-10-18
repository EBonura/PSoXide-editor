//! Scripted poses for headless captures and tests.
//!
//! A pose is a few `;`-separated commands that put the grid editor into a
//! state a person would reach with the mouse: a tool chosen, faces selected,
//! a rectangle or a run of walls half dragged, a paste ghost hovering. It
//! writes the same state the pointer events write, so the picture shows the
//! real overlays. Used by `dump-editor-ui --grid-pose` for screenshots.
//!
//! Commands (cells are world sectors, `x,z`):
//!
//! * `tool=select|floor|ceiling|wall|height|paint|split|place|bands|raise|ramp|stairs|break|rough`
//!   (the tool labels, any case)
//! * `room=N` the active room; `hide=N` hides room N
//! * `help` opens the key help overlay
//! * `select=cells:x0,z0:x1,z1`, `select=floors:...`, `select=ceilings:...`,
//!   `select=walls:...`, `select=derived:...` (the derived walls hosted by
//!   those sectors), `select=vertex:x,z,corner`, `select=edge:x,z,edge`,
//!   `select=plane:i[,j...]` planes of the active room by index
//! * `select=mesh:ID` a placed mesh by its stable id; `mesh-asset=N` the asset
//!   the Mesh tool places; `mesh-turn=degrees` the heading of the next one
//! * `hover=plane:i` a plane under the pointer; `hover=meshspot:gx,gz` where
//!   the Mesh tool would place its asset (grid units, on the floor)
//! * `rect=floor|ceiling:x0,z0:x1,z1[:clicks][:erase]`
//! * `walls=north|east|south|west|nwse|nesw:x0,z0:x1,z1`
//! * `hover=cell:x,z`, `hover=wall:edge:x,z`, `hover=face:floor|ceiling:x,z`,
//!   `hover=vertex:x,z,corner`, `hover=spot:x,z`
//! * `region=x0,z0:x1,z1` the level verbs' region (a marquee, committed)
//! * `overlay=walk` the walkability overlay (Alt+W); `overlay=perf` the
//!   performance heatmap (Alt+P)
//! * `lasso=x,z:x,z:...` a half-drawn lasso (the region source), corners in
//!   sector coordinates that may be decimals (an integer is a lattice corner,
//!   sector `i,j` has its centre at `i+0.5,j+0.5`); the overlay shows the
//!   shape and the sectors it would take, Enter-closing is not posed
//! * `stairs-path=x,z:x,z:...` the Stairs tool's finished path (waypoints),
//!   and the region it covers
//! * `ramp-from=north|east|south|west` the A edge a Ramp keeps; `ramp-clamp`
//!   holds Ramp to the steepest walkable slope
//! * `verb-drag=clicks` a level verb's amount drag in flight at `clicks`
//!   (the active verb tool and its region)
//! * `break-drag=intensity[:seed]` a Break drag in flight on the region at
//!   `intensity` (0 to 100), with `seed` when given (the seed of the Break
//!   option row otherwise); switches to the Break tool
//! * `rough-drag=level` a Rough drag in flight on the region at `level`
//!   (0 to 3); switches to the Rough tool
//! * `triangle-drag=x,z,tri,clicks` a Height tool triangle drag in flight on
//!   the floor of sector `x,z` (ceiling when it has no floor): `tri` is `0` or
//!   `a` for triangle A, `1` or `b` for B
//! * `raise=x,z,corner,clicks` raise a floor corner, like a finished drag
//! * `material=N` the Nth material of the palette
//! * `copy` copies the selection; `paste=x,z` shows the ghost at `x,z`
//! * `marquee=x0,y0:x1,y1` a screen-space box
//! * `camera=yaw,pitch,radius,tx,ty,tz` an orbit camera (editor units)
//! * `bands-hover=x,z,edge,y` the Bands tool hovering the derived wall hosted
//!   by sector `x,z` on `edge` at room height `y` (grid units), with the rule
//!   a click would cut; `bands-drag=x,z,edge,line,y` that wall's line `line`
//!   dragged to height `y` (a drag in flight)

use super::*;

use super::{break_tool, rough_tool, triangle_tool};
use psxed_project::grid::edit::verbs::ramp::RampDir;
use psxed_project::grid::edit::{
    faces_of_cell, wall_gap, CellRect, CornerRef, EdgeRef, FaceRef, FaceTarget, GridClipboard,
    GridElement,
};
use psxed_project::grid::{CLICK_UNITS, SECTOR_UNITS};

fn ints(text: &str) -> Result<Vec<i32>, String> {
    text.split(',')
        .map(|v| v.trim().parse::<i32>().map_err(|e| format!("{v:?}: {e}")))
        .collect()
}

fn floats(text: &str) -> Result<Vec<f64>, String> {
    text.split(',')
        .map(|v| v.trim().parse::<f64>().map_err(|e| format!("{v:?}: {e}")))
        .collect()
}

fn cell(text: &str) -> Result<[i32; 2], String> {
    match ints(text)?.as_slice() {
        [x, z] => Ok([*x, *z]),
        _ => Err(format!("expected x,z in {text:?}")),
    }
}

fn parse_edge(name: &str) -> Result<WallEdge, String> {
    Ok(match name {
        "north" => WallEdge::North,
        "east" => WallEdge::East,
        "south" => WallEdge::South,
        "west" => WallEdge::West,
        "nwse" => WallEdge::Nwse,
        "nesw" => WallEdge::Nesw,
        other => return Err(format!("unknown edge {other:?}")),
    })
}

fn parse_tool(name: &str) -> Result<GridTool, String> {
    GridTool::ALL
        .into_iter()
        .find(|t| t.label().eq_ignore_ascii_case(name))
        .ok_or_else(|| format!("unknown tool {name:?}"))
}

impl EditorWorkspace {
    /// Apply a pose (see the module docs).
    pub fn apply_grid_pose(&mut self, spec: &str) -> Result<(), String> {
        if !self.is_grid_world() {
            return Err("the project is not a grid world".to_string());
        }
        for command in spec.split(';').map(str::trim).filter(|c| !c.is_empty()) {
            let (name, value) = command.split_once('=').unwrap_or((command, ""));
            self.pose_command(name, value)
                .map_err(|e| format!("{command}: {e}"))?;
        }
        Ok(())
    }

    fn pose_command(&mut self, name: &str, value: &str) -> Result<(), String> {
        let room = self.grid_target_room();
        let parts: Vec<&str> = value.split(':').collect();
        match name {
            "tool" => self.set_grid_tool(parse_tool(value)?),
            "mesh-asset" => {
                self.grid.mesh_asset = Some(value.parse::<u16>().map_err(|e| e.to_string())?);
            }
            "mesh-turn" => {
                self.grid.mesh_turn = value.parse::<i32>().map_err(|e| e.to_string())?;
            }
            "room" => {
                let index = value.parse::<usize>().map_err(|e| e.to_string())?;
                self.grid_active_room = Some(index);
                self.grid.last_room = index;
            }
            "hide" => {
                let index = value.parse::<usize>().map_err(|e| e.to_string())?;
                self.grid_hidden_rooms.insert(index);
            }
            "help" => self.grid.help_open = true,
            "region" => {
                let (a, b) = (
                    cell(parts.first().ok_or("missing x0,z0")?)?,
                    cell(parts.get(1).ok_or("missing x1,z1")?)?,
                );
                self.grid.verb.region = Some(psxed_project::grid::edit::Region::from_rect(
                    room,
                    CellRect::from_corners(a, b),
                ));
            }
            "lasso" => {
                let points = parts
                    .iter()
                    .map(|p| match floats(p)?.as_slice() {
                        [x, z] => Ok([*x, *z]),
                        _ => Err(format!("expected x,z in {p:?}")),
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                self.grid.verb.source = verb_state::RegionSource::Lasso;
                self.grid.verb.lasso = points;
                self.grid.verb.lasso_room = room;
                self.grid.verb.lasso_cursor = None;
            }
            "stairs-path" => {
                let waypoints = parts
                    .iter()
                    .map(|p| cell(p))
                    .collect::<Result<Vec<_>, _>>()?;
                self.grid.verb.region = Some(psxed_project::grid::edit::Region::from_path(
                    room,
                    &waypoints,
                    self.grid.verb.path_width,
                ));
                self.grid.verb.stairs_path = waypoints;
            }
            "overlay" => match value {
                "walk" => self.grid.overlays.walk = true,
                "perf" => {
                    self.grid.overlays.perf = true;
                    // A capture has one frame: compute the heatmap now.
                    self.grid.overlays.heat_sync = true;
                }
                other => return Err(format!("unknown overlay {other:?}")),
            },
            "ramp-from" => {
                self.grid.verb.ramp_from = Some(match value {
                    "north" => RampDir::North,
                    "east" => RampDir::East,
                    "south" => RampDir::South,
                    "west" => RampDir::West,
                    other => return Err(format!("unknown side {other:?}")),
                });
            }
            "ramp-clamp" => self.grid.verb.ramp_clamp = true,
            "break-drag" => {
                let (intensity, seed) = match value.split_once(':') {
                    Some((i, seed)) => (i, Some(seed.parse::<u32>().map_err(|e| e.to_string())?)),
                    None => (value, None),
                };
                let intensity = intensity.parse::<i32>().map_err(|e| e.to_string())?;
                if self.grid.tool != GridTool::Break {
                    self.set_grid_tool(GridTool::Break);
                }
                break_tool::pose_drag(self, intensity, seed)?;
            }
            "rough-drag" => {
                let level = value.parse::<i32>().map_err(|e| e.to_string())?;
                if self.grid.tool != GridTool::Rough {
                    self.set_grid_tool(GridTool::Rough);
                }
                rough_tool::pose_drag(self, level)?;
            }
            "triangle-drag" => {
                let parts: Vec<&str> = value.split(',').map(str::trim).collect();
                let [x, z, tri, clicks] = parts[..] else {
                    return Err("expected x,z,tri,clicks".to_string());
                };
                let cell = [
                    x.parse::<i32>().map_err(|e| e.to_string())?,
                    z.parse::<i32>().map_err(|e| e.to_string())?,
                ];
                let tri = match tri {
                    "0" | "a" | "A" => 0,
                    "1" | "b" | "B" => 1,
                    other => return Err(format!("triangle 0/a or 1/b, not {other:?}")),
                };
                let clicks = clicks.parse::<f32>().map_err(|e| e.to_string())?;
                let has_floor = self
                    .project
                    .active_scene()
                    .world
                    .grid()
                    .and_then(|w| w.rooms.get(room))
                    .and_then(|r| r.local(cell).and_then(|[x, z]| r.sector(x, z)))
                    .is_some_and(|s| s.floor().is_some());
                let face = if has_floor {
                    FaceRef::floor(room, cell)
                } else {
                    FaceRef::ceiling(room, cell)
                };
                self.set_grid_tool(GridTool::Height);
                triangle_tool::pose_drag(self, face, tri, clicks)?;
            }
            "verb-drag" => {
                let clicks = value.parse::<f32>().map_err(|e| e.to_string())?;
                verb_tool::pose_drag(self, (clicks * CLICK_UNITS as f32).round() as i32)?;
            }
            "select" => self.pose_select(room, &parts)?,
            "rect" => {
                let kind = match parts.first().copied() {
                    Some("floor") => SurfaceKind::Floor,
                    Some("ceiling") => SurfaceKind::Ceiling,
                    other => return Err(format!("floor or ceiling, not {other:?}")),
                };
                let (a, b) = (
                    cell(parts.get(1).ok_or("missing x0,z0")?)?,
                    cell(parts.get(2).ok_or("missing x1,z1")?)?,
                );
                let height = match parts.get(3) {
                    Some(c) if *c != "erase" => {
                        (c.parse::<f32>().map_err(|e| e.to_string())? * CLICK_UNITS as f32) as i32
                    }
                    _ => match kind {
                        SurfaceKind::Floor => self.grid.floor_height,
                        SurfaceKind::Ceiling => self.grid.ceiling_height,
                    },
                };
                self.grid.gesture = Gesture::Rect(RectGesture {
                    kind,
                    room,
                    anchor: a,
                    current: b,
                    height,
                    adjust: None,
                    erase: parts.contains(&"erase"),
                });
            }
            "walls" => {
                let edge = parse_edge(parts.first().ok_or("missing edge")?)?;
                self.grid.gesture = Gesture::Walls(WallGesture {
                    room,
                    edge,
                    anchor: cell(parts.get(1).ok_or("missing start")?)?,
                    current: cell(parts.get(2).ok_or("missing end")?)?,
                    pick_y: None,
                    erase: false,
                    viewer: None,
                });
            }
            "hover" => {
                self.pose_hover(room, &parts)?;
                self.grid.hover_pinned = true;
            }
            "raise" => {
                let v = ints(value)?;
                let [x, z, n, clicks] = v[..] else {
                    return Err("expected x,z,corner,clicks".to_string());
                };
                let corner = CornerRef {
                    face: FaceRef::floor(room, [x, z]),
                    layer: 0,
                    corner: n as u8,
                };
                let link = self.grid.link;
                let world = self
                    .project
                    .active_scene_mut()
                    .world
                    .grid_mut()
                    .ok_or("no grid world")?;
                psxed_project::grid::edit::nudge(world, &[corner], clicks * CLICK_UNITS, link);
                self.grid.selection.replace(GridElement::Vertex(corner));
                self.grid_changed();
            }
            "material" => {
                let index = value.parse::<usize>().map_err(|e| e.to_string())?;
                let (id, _) = *self
                    .project
                    .material_options()
                    .get(index)
                    .ok_or("no such material")?;
                self.grid.material = Some(id);
                self.grid.palette_scroll_to = Some(id);
            }
            "copy" => {
                let world = self
                    .project
                    .active_scene()
                    .world
                    .grid()
                    .ok_or("no grid world")?;
                let faces = self.grid.selection.faces(world);
                self.grid.clipboard = GridClipboard::capture(world, &faces);
                if self.grid.clipboard.is_none() {
                    return Err("nothing selected to copy".to_string());
                }
            }
            "paste" => {
                let clip = self.grid.clipboard.clone().ok_or("copy first")?;
                self.grid.paste = Some(PasteGhost {
                    clip,
                    at: Some(cell(value)?),
                });
            }
            "marquee" => {
                let (a, b) = (
                    ints(parts.first().ok_or("missing start")?)?,
                    ints(parts.get(1).ok_or("missing end")?)?,
                );
                if a.len() != 2 || b.len() != 2 {
                    return Err("expected x,y:x,y".to_string());
                }
                self.grid.gesture = Gesture::Marquee {
                    start: Pos2::new(a[0] as f32, a[1] as f32),
                    current: Pos2::new(b[0] as f32, b[1] as f32),
                    additive: false,
                };
            }
            "bands-hover" | "bands-drag" => self.pose_bands(name, value)?,
            "camera" => {
                let v = ints(value)?;
                if v.len() != 6 {
                    return Err("expected yaw,pitch,radius,tx,ty,tz".to_string());
                }
                self.camera_rig.mode = ViewportCameraMode::Orbit;
                self.camera_rig.yaw = v[0] as u16;
                self.camera_rig.pitch = v[1] as u16;
                self.camera_rig.radius = v[2];
                self.camera_rig.target = [v[3], v[4], v[5]];
            }
            other => return Err(format!("unknown command {other:?}")),
        }
        Ok(())
    }

    /// `bands-hover` and `bands-drag`: the Bands tool over a derived wall.
    fn pose_bands(&mut self, name: &str, value: &str) -> Result<(), String> {
        use psxed_project::grid::edit::bands::{
            anchor_for, band_at, line_ends, move_line, position_on_wall, wall_spans,
        };
        let parts: Vec<&str> = value.split(',').map(str::trim).collect();
        let room = self.grid_target_room();
        self.set_grid_tool(GridTool::Bands);
        let at = [
            parts
                .first()
                .ok_or("missing x")?
                .parse::<i32>()
                .map_err(|e| e.to_string())?,
            parts
                .get(1)
                .ok_or("missing z")?
                .parse::<i32>()
                .map_err(|e| e.to_string())?,
        ];
        let edge = parse_edge(parts.get(2).ok_or("missing edge")?)?;
        let world = self
            .project
            .active_scene()
            .world
            .grid()
            .ok_or("no grid world")?;
        let wall = world
            .rooms
            .get(room)
            .and_then(|r| {
                psxed_project::grid::derive::edge_walls(r, at, edge)
                    .into_iter()
                    .find(|w| !w.hidden)
            })
            .ok_or("no derived wall there")?;
        let face = FaceRef::derived(room, at, edge, wall.kind);
        let (span, _) = wall_spans(world, face).ok_or("no wall span")?;
        let u = (span.u[0] + span.u[1]) / 2;
        let _ = (position_on_wall, band_at);
        let anchor = parts
            .get(3)
            .and_then(|y| y.parse::<i32>().ok())
            .and_then(|y| anchor_for(world, face, u, y));
        self.grid.selection.set(vec![GridElement::Face(face)]);
        if name == "bands-hover" {
            let y = parts
                .get(3)
                .ok_or("missing y")?
                .parse::<i32>()
                .map_err(|e| e.to_string())?;
            self.grid.hover = GridHover::Band {
                face,
                anchor,
                y,
                line: None,
            };
        } else {
            let line = parts
                .get(3)
                .ok_or("missing line")?
                .parse::<usize>()
                .map_err(|e| e.to_string())?;
            let y = parts
                .get(4)
                .ok_or("missing y")?
                .parse::<i32>()
                .map_err(|e| e.to_string())?;
            let before = self.project.clone();
            let scope = self.grid.band_scope;
            let world = self
                .project
                .active_scene_mut()
                .world
                .grid_mut()
                .ok_or("no grid world")?;
            move_line(world, face, line, u, y, scope);
            let ends = line_ends(world, face);
            let _ = ends;
            self.grid.band_line = Some((face, line));
            self.grid.gesture = Gesture::Band(super::bands_tool::BandDrag {
                face,
                line,
                before: Some(Box::new(before)),
            });
            self.grid_changed();
        }
        self.grid.hover_pinned = true;
        Ok(())
    }

    fn pose_select(&mut self, room: usize, parts: &[&str]) -> Result<(), String> {
        let kind = *parts.first().ok_or("missing kind")?;
        if kind == "mesh" {
            let id = ints(parts.get(1).ok_or("missing id")?)?[0];
            self.grid.mesh_sel = Some(u16::try_from(id).map_err(|e| e.to_string())?);
            return Ok(());
        }
        if kind == "plane" {
            let ids: Vec<psxed_project::grid::edit::PlaneId> =
                ints(parts.get(1).ok_or("missing i")?)?
                    .into_iter()
                    .map(|i| psxed_project::grid::edit::PlaneId {
                        room,
                        index: i as usize,
                    })
                    .collect();
            self.select_planes(ids);
            return Ok(());
        }
        let world = self
            .project
            .active_scene()
            .world
            .grid()
            .ok_or("no grid world")?;
        let mut picked: Vec<GridElement> = Vec::new();
        match kind {
            "vertex" | "edge" => {
                let v = ints(parts.get(1).ok_or("missing x,z,n")?)?;
                let [x, z, n] = v[..] else {
                    return Err("expected x,z,n".to_string());
                };
                let face = FaceRef::floor(room, [x, z]);
                picked.push(if kind == "vertex" {
                    GridElement::Vertex(CornerRef {
                        face,
                        layer: 0,
                        corner: n as u8,
                    })
                } else {
                    GridElement::Edge(EdgeRef {
                        face,
                        layer: 0,
                        edge: n as u8,
                    })
                });
            }
            _ => {
                let rect = CellRect::from_corners(
                    cell(parts.get(1).ok_or("missing x0,z0")?)?,
                    cell(parts.get(2).ok_or("missing x1,z1")?)?,
                );
                for c in rect.cells() {
                    let faces = faces_of_cell(world, room, c);
                    match kind {
                        "cells" => {
                            if !faces.is_empty() {
                                picked.push(GridElement::Cell { room, cell: c });
                            }
                        }
                        "derived" => {
                            if let Some(r) = world.rooms.get(room) {
                                picked.extend(
                                    psxed_project::grid::derive::cell_walls(r, c)
                                        .into_iter()
                                        .map(|w| {
                                            GridElement::Face(FaceRef::derived(
                                                room, c, w.edge, w.kind,
                                            ))
                                        }),
                                );
                            }
                        }
                        "floors" | "ceilings" | "walls" => {
                            picked.extend(
                                faces
                                    .into_iter()
                                    .filter(|f| {
                                        matches!(
                                            (kind, f.target),
                                            ("floors", FaceTarget::Surface(SurfaceKind::Floor))
                                                | (
                                                    "ceilings",
                                                    FaceTarget::Surface(SurfaceKind::Ceiling)
                                                )
                                                | ("walls", FaceTarget::Wall { .. })
                                        )
                                    })
                                    .map(GridElement::Face),
                            );
                        }
                        other => return Err(format!("unknown selection {other:?}")),
                    }
                }
            }
        }
        self.grid.selection.extend(picked);
        Ok(())
    }

    fn pose_hover(&mut self, room: usize, parts: &[&str]) -> Result<(), String> {
        let world = self
            .project
            .active_scene()
            .world
            .grid()
            .ok_or("no grid world")?;
        self.grid.hover = match *parts.first().ok_or("missing kind")? {
            "cell" => GridHover::Cell {
                room,
                cell: cell(parts.get(1).ok_or("missing x,z")?)?,
                height: match self.grid.tool {
                    GridTool::Ceiling => self.grid.ceiling_height,
                    _ => self.grid.floor_height,
                },
            },
            "wall" => {
                let edge = parse_edge(parts.get(1).ok_or("missing edge")?)?;
                let c = cell(parts.get(2).ok_or("missing x,z")?)?;
                let r = world.rooms.get(room).ok_or("no such room")?;
                GridHover::Wall {
                    room,
                    cell: c,
                    edge,
                    heights: wall_gap(r, c, edge, None),
                }
            }
            "face" => {
                let c = cell(parts.get(2).ok_or("missing x,z")?)?;
                let face = match parts.get(1).copied() {
                    Some("ceiling") => FaceRef::ceiling(room, c),
                    _ => FaceRef::floor(room, c),
                };
                GridHover::Face { face, tri: None }
            }
            "vertex" => {
                let v = ints(parts.get(1).ok_or("missing x,z,corner")?)?;
                let [x, z, n] = v[..] else {
                    return Err("expected x,z,corner".to_string());
                };
                GridHover::Elements(vec![GridElement::Vertex(CornerRef {
                    face: FaceRef::floor(room, [x, z]),
                    layer: 0,
                    corner: n as u8,
                })])
            }
            "plane" => {
                let index = ints(parts.get(1).ok_or("missing i")?)?[0] as usize;
                GridHover::Plane(psxed_project::grid::edit::PlaneId { room, index })
            }
            "meshspot" => {
                let g = ints(parts.get(1).ok_or("missing gx,gz")?)?;
                let [gx, gz] = g[..] else {
                    return Err("expected gx,gz".to_string());
                };
                let s = world.scale;
                let y = world
                    .floor_height_at(s.to_editor(gx), s.to_editor(gz), i32::MAX / 2)
                    .map_or(0, |f| s.to_grid(f))
                    - world.rooms.get(room).map_or(0, |r| r.y_offset);
                GridHover::MeshSpot {
                    room,
                    position: [gx, y, gz],
                }
            }
            "spot" => {
                let c = cell(parts.get(1).ok_or("missing x,z")?)?;
                let s = world.scale;
                let x = s.to_editor(c[0] * SECTOR_UNITS + SECTOR_UNITS / 2);
                let z = s.to_editor(c[1] * SECTOR_UNITS + SECTOR_UNITS / 2);
                let y = world.floor_height_at(x, z, i32::MAX / 2).unwrap_or(0);
                GridHover::Spot {
                    position: [x as f32, y as f32, z as f32],
                }
            }
            other => return Err(format!("unknown hover {other:?}")),
        };
        Ok(())
    }
}
