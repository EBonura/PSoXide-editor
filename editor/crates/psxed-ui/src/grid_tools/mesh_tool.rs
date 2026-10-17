//! The Mesh tool: place, select, move and turn static meshes.
//!
//! * **Place**: pick an asset in the Meshes palette (right dock), click a
//!   floor. The instance stands on the floor under the pointer, snapped to the
//!   256 lattice (Alt 64, Shift+Alt free), turned by the tool's heading.
//! * **Select** an instance by clicking it (either view); the Inspector shows
//!   its numbers.
//! * **Move**: drag the selected (or any) instance; it follows the floor.
//! * **Turn**: R a quarter turn, `[` and `]` 15 degrees (the selected
//!   instance, or the heading of the next one).
//! * **Delete** removes the selected instance.
//!
//! Every edit goes through `psxed_project::grid::edit::meshes`, the functions
//! the MCP calls, so the two make meshes the same way.

use super::view::{cell_at_editor, ray_plane_y};
use super::*;

use psxed_project::grid::edit::{delete_meshes, place_mesh, update_mesh};
use psxed_project::grid::mesh::GridMesh;
use psxed_project::grid::units::{snap_to, CLICK_UNITS, FINE_UNITS};

/// A drag of one instance.
#[derive(Clone, Debug)]
pub(crate) struct MeshDrag {
    pub id: u16,
    /// The instance's position minus the snapped point under the pointer at
    /// the press, so the drag keeps the offset the grab had.
    pub grab: [i32; 2],
    /// The document at the press, recorded for undo at the first change.
    pub before: Box<ProjectDocument>,
    /// Whether the instance moved yet.
    pub changed: bool,
}

/// The room, in grid units, and the position a mesh would take under the
/// pointer.
pub(super) fn spot(ws: &EditorWorkspace, ptr: &GridPointer) -> Option<(usize, [i32; 3])> {
    let world = ws.project.active_scene().world.grid()?;
    let point = match ptr.view {
        GridView::Persp(rect) => ws
            .pick_face_3d(rect, ptr.pos)
            .map(|h| h.point)
            .or_else(|| {
                let (origin, dir) = ws.grid_ray(rect, ptr.pos)?;
                ray_plane_y(origin, dir, 0.0)
            })?,
        GridView::Top(transform) => {
            let [x, z] = transform.screen_to_world(ptr.pos);
            [x, f32::MAX, z]
        }
    };
    let step = if ptr.free() {
        1
    } else if ptr.alt() {
        FINE_UNITS
    } else {
        CLICK_UNITS
    };
    let gx = snap_to(world.scale.to_grid(point[0].round() as i32), step);
    let gz = snap_to(world.scale.to_grid(point[2].round() as i32), step);
    let cell = cell_at_editor(world.scale, point[0], point[2]);
    let target = ws.grid_target_room();
    let room = world
        .rooms
        .get(target)
        .filter(|r| r.local(cell).is_some())
        .map(|_| target)
        .or_else(|| world.rooms.iter().position(|r| r.local(cell).is_some()))
        .unwrap_or(target);
    let limit = if point[1] == f32::MAX {
        i32::MAX / 2
    } else {
        point[1].round() as i32 + 96
    };
    let floor = world
        .floor_height_at(world.scale.to_editor(gx), world.scale.to_editor(gz), limit)
        .map(|f| world.scale.to_grid(f));
    let y_world = floor.unwrap_or_else(|| {
        if point[1] == f32::MAX {
            0
        } else {
            world.scale.to_grid(point[1].round() as i32)
        }
    });
    let y = y_world - world.rooms.get(room).map_or(0, |r| r.y_offset);
    Some((room, [gx, y, gz]))
}

/// Ray against a box, both editor units: the distance along the ray.
fn ray_box(origin: [f32; 3], dir: [f32; 3], min: [f32; 3], max: [f32; 3]) -> Option<f32> {
    let (mut near, mut far) = (0.0f32, f32::MAX);
    for i in 0..3 {
        if dir[i].abs() < 1e-9 {
            if origin[i] < min[i] || origin[i] > max[i] {
                return None;
            }
            continue;
        }
        let (a, b) = ((min[i] - origin[i]) / dir[i], (max[i] - origin[i]) / dir[i]);
        let (lo, hi) = if a < b { (a, b) } else { (b, a) };
        near = near.max(lo);
        far = far.min(hi);
        if near > far {
            return None;
        }
    }
    Some(near)
}

/// Whether `p` lies inside the quad `corners` (any winding).
fn inside(corners: &[[f64; 2]; 4], p: [f64; 2]) -> bool {
    let mut sign = 0.0f64;
    for i in 0..4 {
        let (a, b) = (corners[i], corners[(i + 1) % 4]);
        let cross = (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
        if cross != 0.0 {
            if sign != 0.0 && cross.signum() != sign {
                return false;
            }
            sign = cross.signum();
        }
    }
    true
}

impl EditorWorkspace {
    /// The instance under the pointer, topmost in the Top view and nearest in
    /// the 3D view: `(room, index, id)`.
    pub(super) fn mesh_under(&self, ptr: &GridPointer) -> Option<u16> {
        let world = self.project.active_scene().world.grid()?;
        let mut best: Option<(f32, u16)> = None;
        for (room_index, room) in world.rooms.iter().enumerate() {
            if self.grid_hidden_rooms.contains(&room_index) {
                continue;
            }
            for index in 0..room.meshes.len() {
                let Some(placed) =
                    psxed_project::grid::mesh_faces::placed(world, room_index, index)
                else {
                    continue;
                };
                let mesh = placed.mesh;
                match ptr.view {
                    GridView::Top(transform) => {
                        let Some(foot) = mesh.footprint(&placed.built) else {
                            continue;
                        };
                        let [x, z] = transform.screen_to_world(ptr.pos);
                        let p = [
                            f64::from(world.scale.to_grid(x.round() as i32)),
                            f64::from(world.scale.to_grid(z.round() as i32)),
                        ];
                        let quad = foot.map(|c| [f64::from(c[0]), f64::from(c[1])]);
                        if inside(&quad, p) {
                            // Later instances draw over earlier ones.
                            best = Some((0.0, mesh.id));
                        }
                    }
                    GridView::Persp(rect) => {
                        let Some((lo, hi)) = mesh.bounds(&placed.built) else {
                            continue;
                        };
                        let Some((origin, dir)) = self.grid_ray(rect, ptr.pos) else {
                            continue;
                        };
                        let y = |v: i32| world.scale.to_editor(room.y_offset + v) as f32;
                        let min = [
                            world.scale.to_editor(lo[0]) as f32,
                            y(lo[1]),
                            world.scale.to_editor(lo[2]) as f32,
                        ];
                        let max = [
                            world.scale.to_editor(hi[0]) as f32,
                            y(hi[1]),
                            world.scale.to_editor(hi[2]) as f32,
                        ];
                        if let Some(t) = ray_box(origin, dir, min, max) {
                            if best.is_none_or(|(b, _)| t < b) {
                                best = Some((t, mesh.id));
                            }
                        }
                    }
                }
            }
        }
        best.map(|(_, id)| id)
    }

    /// Forget a selected instance that no longer exists.
    pub(crate) fn retain_valid_mesh(&mut self) {
        let valid = self.grid.mesh_sel.is_some_and(|id| {
            self.project
                .active_scene()
                .world
                .grid()
                .is_some_and(|w| w.find_mesh(id).is_some())
        });
        if !valid {
            self.grid.mesh_sel = None;
        }
    }

    /// The selected instance, if it exists.
    pub(crate) fn mesh_primary(&self) -> Option<u16> {
        let id = self.grid.mesh_sel?;
        self.project
            .active_scene()
            .world
            .grid()?
            .find_mesh(id)
            .map(|_| id)
    }

    /// Apply the palette material to every slot of the selected instance.
    pub(crate) fn apply_material_to_selected_mesh(&mut self, material: ResourceId) -> usize {
        let Some(id) = self.mesh_primary() else {
            return 0;
        };
        let before = self.project.clone();
        let Some(world) = self.project.active_scene_mut().world.grid_mut() else {
            return 0;
        };
        let slots = world
            .find_mesh(id)
            .and_then(|(r, i)| {
                world
                    .mesh_assets
                    .get(usize::from(world.rooms[r].meshes[i].asset))
            })
            .and_then(|a| a.build().ok())
            .map_or(1, |b| b.slots.len());
        let changed = update_mesh(world, id, |m| {
            for slot in 0..slots {
                m.set_material(slot, Some(material));
            }
        })
        .unwrap_or(false);
        if changed {
            self.history.record(before);
            self.grid_changed();
            return 1;
        }
        0
    }
}

pub(super) fn hover(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    ws.grid.hover = if let Some(id) = ws.mesh_under(ptr) {
        GridHover::Mesh(id)
    } else {
        match spot(ws, ptr) {
            Some((room, position)) => GridHover::MeshSpot { room, position },
            None => GridHover::None,
        }
    };
}

pub(super) fn press(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    let Some(id) = ws.mesh_under(ptr) else {
        return;
    };
    ws.grid.mesh_sel = Some(id);
    let Some(world) = ws.project.active_scene().world.grid() else {
        return;
    };
    let Some((room, index)) = world.find_mesh(id) else {
        return;
    };
    let position = world.rooms[room].meshes[index].position;
    let Some((_, at)) = spot(ws, ptr) else {
        return;
    };
    ws.grid.gesture = Gesture::Mesh(MeshDrag {
        id,
        grab: [position[0] - at[0], position[2] - at[2]],
        before: Box::new(ws.project.clone()),
        changed: false,
    });
}

pub(super) fn drag(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    let Gesture::Mesh(drag) = &ws.grid.gesture else {
        return;
    };
    let (id, grab) = (drag.id, drag.grab);
    let Some((_, at)) = spot(ws, ptr) else {
        return;
    };
    let Some(world) = ws.project.active_scene_mut().world.grid_mut() else {
        return;
    };
    let target = [at[0] + grab[0], at[1], at[2] + grab[1]];
    let moved = update_mesh(world, id, |m| m.position = target);
    match moved {
        Ok(true) => {
            let mut record = None;
            if let Gesture::Mesh(drag) = &mut ws.grid.gesture {
                if !drag.changed {
                    drag.changed = true;
                    record = Some((*drag.before).clone());
                }
            }
            if let Some(before) = record {
                ws.history.record(before);
            }
            ws.grid_changed();
        }
        Ok(false) => {}
        Err(refusal) => ws.status = refusal.describe(),
    }
}

pub(super) fn release(ws: &mut EditorWorkspace) {
    if matches!(ws.grid.gesture, Gesture::Mesh(_)) {
        ws.grid.gesture = Gesture::Idle;
    }
}

pub(super) fn click(ws: &mut EditorWorkspace, ptr: &GridPointer) {
    if let Some(id) = ws.mesh_under(ptr) {
        ws.grid.mesh_sel = Some(id);
        return;
    }
    let Some(world) = ws.project.active_scene().world.grid() else {
        return;
    };
    let Some(asset) = ws
        .grid
        .mesh_asset
        .filter(|a| usize::from(*a) < world.mesh_assets.len())
    else {
        ws.status = "Pick or make a mesh in the Meshes palette (right dock), then click a floor"
            .to_string();
        return;
    };
    let slots = world.mesh_assets[usize::from(asset)]
        .build()
        .map_or(1, |b| b.slots.len());
    let Some((room, position)) = spot(ws, ptr) else {
        ws.status = "Place over a floor".to_string();
        return;
    };
    let mut mesh = GridMesh::new(0, asset, position);
    mesh.rotation[0] = ws.grid.mesh_turn;
    if let Some(material) = ws.grid_new_material(room, false) {
        for slot in 0..slots {
            mesh.set_material(slot, Some(material));
        }
    }
    let before = ws.project.clone();
    let Some(world) = ws.project.active_scene_mut().world.grid_mut() else {
        return;
    };
    match place_mesh(world, room, mesh) {
        Ok(id) => {
            ws.history.record(before);
            ws.grid.mesh_sel = Some(id);
            ws.status = format!(
                "Placed mesh {id} in room {room}: R turns it, drag moves it, the Inspector has its numbers"
            );
            ws.grid_changed();
        }
        Err(refusal) => ws.status = refusal.describe(),
    }
}

/// Turn the selected instance (or the heading of the next one) by `degrees`.
pub(crate) fn turn(ws: &mut EditorWorkspace, degrees: i32) {
    if let Some(id) = ws.mesh_primary() {
        let before = ws.project.clone();
        let Some(world) = ws.project.active_scene_mut().world.grid_mut() else {
            return;
        };
        if update_mesh(world, id, |m| m.rotation[0] += degrees).unwrap_or(false) {
            ws.history.record(before);
            ws.grid_changed();
        }
    } else {
        ws.grid.mesh_turn = psxed_project::grid::plane::wrap_degrees(ws.grid.mesh_turn + degrees);
        ws.status = format!("Next mesh turned to {} degrees", ws.grid.mesh_turn);
    }
}

/// Delete the selected instance. Returns whether there was one.
pub(crate) fn delete_selected(ws: &mut EditorWorkspace) -> bool {
    let Some(id) = ws.mesh_primary() else {
        return false;
    };
    let before = ws.project.clone();
    let Some(world) = ws.project.active_scene_mut().world.grid_mut() else {
        return false;
    };
    if delete_meshes(world, &[id]) == 0 {
        return false;
    }
    ws.history.record(before);
    ws.grid.mesh_sel = None;
    ws.status = format!("Deleted mesh {id}");
    ws.grid_changed();
    true
}

/// Scale the selected instance by `steps` eighths (Q8 steps of 32, so a
/// uniform scale stays on the lattice the Inspector shows as a multiple of
/// 12.5 percent). Without a selection, say what to do.
pub(crate) fn scale_step(ws: &mut EditorWorkspace, steps: i32) {
    let Some(id) = ws.mesh_primary() else {
        ws.status = "Select a mesh to scale it (the Inspector has the numbers)".to_string();
        return;
    };
    let before = ws.project.clone();
    let Some(world) = ws.project.active_scene_mut().world.grid_mut() else {
        return;
    };
    let step = i32::from(psxed_project::grid::mesh::SCALE_ONE) / 8 * steps;
    let changed = update_mesh(world, id, |m| {
        for s in &mut m.scale {
            *s = (i32::from(*s) + step).clamp(0, i32::from(u16::MAX)) as u16;
        }
    })
    .unwrap_or(false);
    if changed {
        ws.history.record(before);
        ws.grid_changed();
    }
}
