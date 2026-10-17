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
//! * `tool=select|floor|ceiling|wall|height|paint|split|place`
//! * `room=N` the active room; `hide=N` hides room N
//! * `help` opens the key help overlay
//! * `select=cells:x0,z0:x1,z1`, `select=floors:...`, `select=ceilings:...`,
//!   `select=walls:...`, `select=derived:...` (the derived walls hosted by
//!   those sectors), `select=vertex:x,z,corner`, `select=edge:x,z,edge`,
//!   `select=plane:i[,j...]` planes of the active room by index
//! * `hover=plane:i` a plane under the pointer
//! * `rect=floor|ceiling:x0,z0:x1,z1[:clicks][:erase]`
//! * `walls=north|east|south|west|nwse|nesw:x0,z0:x1,z1`
//! * `hover=cell:x,z`, `hover=wall:edge:x,z`, `hover=face:floor|ceiling:x,z`,
//!   `hover=vertex:x,z,corner`, `hover=spot:x,z`
//! * `raise=x,z,corner,clicks` raise a floor corner, like a finished drag
//! * `material=N` the Nth material of the palette
//! * `copy` copies the selection; `paste=x,z` shows the ghost at `x,z`
//! * `marquee=x0,y0:x1,y1` a screen-space box
//! * `camera=yaw,pitch,radius,tx,ty,tz` an orbit camera (editor units)

use super::*;

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

    fn pose_select(&mut self, room: usize, parts: &[&str]) -> Result<(), String> {
        let kind = *parts.first().ok_or("missing kind")?;
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
