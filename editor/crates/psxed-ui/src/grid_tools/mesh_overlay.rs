//! Drawing placed meshes over the views: footprints in the Top view, boxes
//! in the 3D view (which already shows the mesh itself), the selection, the
//! hover, the ghost of the next placement and the hook sockets.

use super::overlay::{fill, label_chip, CREATE, HOVER, SELECT};
use super::pick::Cam;
use super::view::project_segment;
use super::*;

use psxed_project::grid::mesh::{GridMesh, Socket};
use psxed_project::grid::mesh_faces::Placed;
use psxed_project::grid::GridWorld;

const MESH: Color32 = Color32::from_rgb(235, 160, 70);
const HOOK: Color32 = Color32::from_rgb(240, 240, 255);

/// One instance, ready to draw: its box corners and sockets in editor units.
struct Drawn {
    id: u16,
    /// Bottom four then top four.
    corners: [[f32; 3]; 8],
    hooks: Vec<[f32; 3]>,
    collide: bool,
}

fn editor_point(world: &GridWorld, room_y: i32, g: [i32; 3]) -> [f32; 3] {
    [
        world.scale.to_editor(g[0]) as f32,
        world.scale.to_editor(room_y + g[1]) as f32,
        world.scale.to_editor(g[2]) as f32,
    ]
}

fn draw_of(
    world: &GridWorld,
    room_y: i32,
    mesh: &GridMesh,
    built: &psxed_project::grid::mesh::BuiltMesh,
) -> Option<Drawn> {
    let corners = mesh
        .oriented_box(built)?
        .map(|g| editor_point(world, room_y, g));
    let hooks = built
        .sockets
        .iter()
        .map(|s| {
            let Socket::Hook { anchor, .. } = *s;
            editor_point(world, room_y, mesh.place(anchor))
        })
        .collect();
    Some(Drawn {
        id: mesh.id,
        corners,
        hooks,
        collide: mesh.collide,
    })
}

impl EditorWorkspace {
    fn drawn_meshes(&self, world: &GridWorld) -> Vec<Drawn> {
        let mut out = Vec::new();
        for (room_index, room) in world.rooms.iter().enumerate() {
            if self.grid_hidden_rooms.contains(&room_index) {
                continue;
            }
            for index in 0..room.meshes.len() {
                let Some(Placed { mesh, built }) =
                    psxed_project::grid::mesh_faces::placed(world, room_index, index)
                else {
                    continue;
                };
                if let Some(d) = draw_of(world, room.y_offset, mesh, &built) {
                    out.push(d);
                }
            }
        }
        out
    }

    /// The box the Mesh tool would place at the hovered spot.
    fn mesh_ghost(&self, world: &GridWorld) -> Option<Drawn> {
        let GridHover::MeshSpot { room, position } = &self.grid.hover else {
            return None;
        };
        if self.grid.tool != GridTool::Mesh {
            return None;
        }
        let asset = world.mesh_assets.get(usize::from(self.grid.mesh_asset?))?;
        let built = asset.build().ok()?;
        let mut mesh = GridMesh::new(0, self.grid.mesh_asset?, *position);
        mesh.rotation[0] = self.grid.mesh_turn;
        let y = world.rooms.get(*room)?.y_offset;
        draw_of(world, y, &mesh, &built)
    }

    fn hovered_mesh(&self) -> Option<u16> {
        match &self.grid.hover {
            GridHover::Mesh(id) => Some(*id),
            _ => None,
        }
    }

    /// Footprints, selection, hover and ghost over the Top view.
    pub(crate) fn draw_mesh_overlay_2d(
        &self,
        painter: &egui::Painter,
        transform: ViewportTransform,
    ) {
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let screen = |p: &[f32; 3]| transform.world_to_screen([p[0], p[2]]);
        let footprint = |d: &Drawn, color: Color32, width: f32, alpha: u8| {
            let s: Vec<Pos2> = d.corners[..4].iter().map(screen).collect();
            painter.add(egui::Shape::convex_polygon(
                s.clone(),
                fill(color, alpha),
                Stroke::NONE,
            ));
            for i in 0..4 {
                painter.line_segment([s[i], s[(i + 1) % 4]], Stroke::new(width, color));
            }
        };
        let selected = self.grid.mesh_sel;
        let hovered = self.hovered_mesh();
        for d in self.drawn_meshes(world) {
            let color = if d.collide { MESH } else { fill(MESH, 150) };
            if Some(d.id) == selected {
                footprint(&d, SELECT, 3.0, 70);
            } else if Some(d.id) == hovered {
                footprint(&d, HOVER, 2.5, 55);
            } else {
                footprint(&d, color, 2.0, 38);
            }
            let centre = {
                let s: Vec<Pos2> = d.corners[..4].iter().map(screen).collect();
                s.iter().fold(Vec2::ZERO, |a, p| a + p.to_vec2()) / 4.0
            };
            for hook in &d.hooks {
                let at = screen(hook);
                painter.circle_stroke(at, 5.0, Stroke::new(2.0, HOOK));
                painter.circle_filled(at, 1.8, HOOK);
            }
            if Some(d.id) == selected || Some(d.id) == hovered {
                label_chip(
                    painter,
                    centre.to_pos2(),
                    Align2::CENTER_CENTER,
                    &format!("mesh {}", d.id),
                );
            }
        }
        if let Some(ghost) = self.mesh_ghost(world) {
            footprint(&ghost, CREATE, 2.5, 60);
        }
    }

    /// Boxes over the 3D view: all of them with the Mesh tool, else only the
    /// selected and hovered.
    pub(crate) fn draw_mesh_overlay_3d(&self, painter: &egui::Painter, rect: Rect) {
        let Some(world) = self.project.active_scene().world.grid() else {
            return;
        };
        let camera = self.viewport_3d_camera();
        let cam = Cam::new(camera, rect);
        let wire = |d: &Drawn, color: Color32, width: f32| {
            let c = &d.corners;
            for i in 0..4 {
                let j = (i + 1) % 4;
                for (a, b) in [(c[i], c[j]), (c[4 + i], c[4 + j]), (c[i], c[4 + i])] {
                    if let Some([pa, pb]) = project_segment(camera, rect, a, b) {
                        painter.line_segment([pa, pb], Stroke::new(width, color));
                    }
                }
            }
        };
        let all = self.grid.tool == GridTool::Mesh;
        let hovered = self.hovered_mesh();
        for d in self.drawn_meshes(world) {
            if Some(d.id) == self.grid.mesh_sel {
                wire(&d, SELECT, 2.5);
            } else if Some(d.id) == hovered {
                wire(&d, HOVER, 2.0);
            } else if all {
                wire(&d, fill(MESH, 150), 1.2);
            }
            if all || Some(d.id) == self.grid.mesh_sel {
                for hook in &d.hooks {
                    if let Some((at, _)) = cam.project(*hook) {
                        painter.circle_stroke(at, 6.0, Stroke::new(2.0, HOOK));
                        painter.circle_filled(at, 2.0, HOOK);
                    }
                }
            }
        }
        if let Some(ghost) = self.mesh_ghost(world) {
            wire(&ghost, CREATE, 2.0);
        }
    }
}
