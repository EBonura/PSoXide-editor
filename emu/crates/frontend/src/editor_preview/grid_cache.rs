//! Grid geometry the preview keeps between frames.
//!
//! Turning a grid world into drawable faces (deriving walls from height
//! differences, tiling the textures, baking the lights into the corners) does
//! not depend on where the camera is, yet the preview used to redo all of it
//! for every visible cell on every frame. Here the result is kept per room and
//! per box (a cell, a free plane or a placed mesh), and thrown away only when
//! something it was made from changes:
//!
//! * the room itself (compared with a snapshot of it, so an edit that touches
//!   one room leaves the others alone);
//! * the world's scale, facing shade or mesh assets, the project's materials,
//!   the scene's lights, or the texture slots (the [`GeometryContext`]), which
//!   every face of every room is made from.
//!
//! A room is prepared in two steps. Its boxes and their cull bounds come
//! first, because the view needs the bounds of every box to decide what is on
//! screen. The faces of a box are made the first time the camera sees it, so
//! an edit that drags across a large room pays for what is visible, no more
//! than the old per-frame path did.

use super::*;

use psxed_project::grid::light::{FacingShade, GridLight};
use psxed_project::grid::mesh::MeshAsset;
use psxed_project::grid::{GridRoom, GridScale, GridWorld};
use psxed_project::Resource;

/// One drawable polygon: everything `emit_uv_polygon` needs.
pub(super) struct PreparedPiece {
    pub(super) shade: FaceShade,
    pub(super) verts: Vec<[f64; 3]>,
    pub(super) uvs: Vec<[f64; 2]>,
    pub(super) colors: Vec<(u8, u8, u8)>,
}

/// A face of the world as the cook cuts it into pieces.
pub(super) struct PreparedFace {
    /// Lowest corner of the uncut face, editor units. The cutaway keeps or
    /// drops the whole face by it.
    pub(super) low_y: i32,
    pub(super) pieces: Vec<PreparedPiece>,
}

/// Where the faces of a [`LazyBox`] come from.
#[derive(Clone, Copy)]
pub(super) enum BoxSource {
    Cell(usize, usize),
    Plane(usize),
    Mesh(usize),
}

/// A cell, free plane or placed mesh: its cull bounds now, its faces on first
/// sight.
pub(super) struct LazyBox {
    pub(super) bounds: PreviewCellBounds,
    pub(super) source: BoxSource,
    pub(super) faces: Option<Vec<PreparedFace>>,
}

impl LazyBox {
    pub(super) fn new(bounds: PreviewCellBounds, source: BoxSource) -> Self {
        Self {
            bounds,
            source,
            faces: None,
        }
    }

    pub(super) fn is_cell(&self) -> bool {
        matches!(self.source, BoxSource::Cell(..))
    }
}

/// The boxes of one room, in the order the preview draws them.
#[derive(Default)]
pub(super) struct PreparedRoom {
    pub(super) cells: Vec<LazyBox>,
    pub(super) planes: Vec<LazyBox>,
    pub(super) meshes: Vec<LazyBox>,
}

/// A room as it was when its boxes were prepared, and the boxes.
pub(super) struct RoomSlot {
    source: GridRoom,
    pub(super) prepared: Option<PreparedRoom>,
}

/// What every face of every room is made from besides its own room.
#[derive(PartialEq)]
pub(super) struct GeometryContext {
    scale: GridScale,
    shading: FacingShade,
    mesh_assets: Vec<MeshAsset>,
    /// The lights baked into the corners.
    lights: Vec<GridLight>,
    /// Every material resource: tint, tiling, sidedness, blend and the name
    /// the fallback colour is chosen by.
    materials: Vec<Resource>,
    /// Texture slots, by the generation of `EditorTextures`.
    textures_generation: u64,
}

impl GeometryContext {
    fn of(project: &ProjectDocument, world: &GridWorld, textures_generation: u64) -> Self {
        Self {
            scale: world.scale,
            shading: world.shading,
            mesh_assets: world.mesh_assets.clone(),
            lights: collect_grid_bake_lights(project),
            materials: project
                .resources
                .iter()
                .filter(|resource| matches!(resource.data, ResourceData::Material(_)))
                .cloned()
                .collect(),
            textures_generation,
        }
    }
}

/// The kept geometry of the active scene's grid world.
#[derive(Default)]
pub(super) struct GridGeometry {
    context: Option<GeometryContext>,
    rooms: Vec<RoomSlot>,
    /// `(document generation, textures generation)` the rooms were last
    /// checked against.
    synced: Option<(u64, u64)>,
}

impl GridGeometry {
    /// Bring the kept geometry up to date with `project`: drop what an
    /// edit made stale, keep the rest. Cheap when nothing changed.
    pub(super) fn sync(
        &mut self,
        project: &ProjectDocument,
        document_generation: u64,
        textures_generation: u64,
    ) {
        if self.synced == Some((document_generation, textures_generation)) {
            return;
        }
        self.synced = Some((document_generation, textures_generation));
        let Some(world) = project.active_scene().world.grid() else {
            self.context = None;
            self.rooms.clear();
            return;
        };
        let context = GeometryContext::of(project, world, textures_generation);
        if self.context.as_ref() != Some(&context) {
            // Everything is made from the context: start over.
            self.rooms.clear();
            self.context = Some(context);
        }
        self.rooms.truncate(world.rooms.len());
        for (index, room) in world.rooms.iter().enumerate() {
            match self.rooms.get_mut(index) {
                Some(slot) if slot.source == *room => {}
                Some(slot) => {
                    slot.source = room.clone();
                    slot.prepared = None;
                }
                None => self.rooms.push(RoomSlot {
                    source: room.clone(),
                    prepared: None,
                }),
            }
        }
    }

    /// The rooms and the lights baked into them, for a frame to draw.
    pub(super) fn rooms_and_lights(&mut self) -> (&mut [RoomSlot], &[GridLight]) {
        let lights = self.context.as_ref().map_or(&[][..], |c| &c.lights[..]);
        (&mut self.rooms, lights)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use psxed_project::grid::{GridSurface, SceneWorld};

    fn two_room_project() -> ProjectDocument {
        let rooms = (0..2)
            .map(|index| {
                let mut room = GridRoom::new(format!("room {index}"), 2, 2);
                for x in 0..2 {
                    for z in 0..2 {
                        room.sector_or_insert(x, z)
                            .unwrap()
                            .set_ground_floor(Some(GridSurface::flat(0, None)));
                    }
                }
                room
            })
            .collect();
        let mut project = ProjectDocument::new("two rooms");
        project.active_scene_mut().world = SceneWorld::Grid(GridWorld {
            rooms,
            ..GridWorld::new()
        });
        project
    }

    fn prepared_rooms(geometry: &GridGeometry) -> Vec<bool> {
        geometry
            .rooms
            .iter()
            .map(|slot| slot.prepared.is_some())
            .collect()
    }

    fn prepare_all(geometry: &mut GridGeometry) {
        for slot in &mut geometry.rooms {
            slot.prepared = Some(PreparedRoom::default());
        }
    }

    #[test]
    fn an_edit_to_one_room_leaves_the_other_prepared() {
        let mut project = two_room_project();
        let mut geometry = GridGeometry::default();
        geometry.sync(&project, 1, 0);
        prepare_all(&mut geometry);
        assert_eq!(prepared_rooms(&geometry), [true, true]);

        project.active_scene_mut().world.grid_mut().unwrap().rooms[1]
            .sector_mut(0, 0)
            .and_then(|s| s.ground_floor_mut())
            .unwrap()
            .heights = [256; 4];
        geometry.sync(&project, 2, 0);
        assert_eq!(prepared_rooms(&geometry), [true, false]);
    }

    #[test]
    fn a_change_to_what_every_face_is_made_from_drops_every_room() {
        let mut project = two_room_project();
        let mut geometry = GridGeometry::default();
        geometry.sync(&project, 1, 0);
        prepare_all(&mut geometry);

        // The texture slots moved.
        geometry.sync(&project, 1, 1);
        assert_eq!(prepared_rooms(&geometry), [false, false]);

        // The world's facing shade changed.
        prepare_all(&mut geometry);
        project
            .active_scene_mut()
            .world
            .grid_mut()
            .unwrap()
            .shading
            .up = 100;
        geometry.sync(&project, 2, 1);
        assert_eq!(prepared_rooms(&geometry), [false, false]);
    }

    #[test]
    fn nothing_changed_keeps_everything() {
        let project = two_room_project();
        let mut geometry = GridGeometry::default();
        geometry.sync(&project, 1, 0);
        prepare_all(&mut geometry);
        geometry.sync(&project, 1, 0);
        assert_eq!(prepared_rooms(&geometry), [true, true]);
    }
}
