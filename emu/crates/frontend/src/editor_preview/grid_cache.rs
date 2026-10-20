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
use psxed_project::grid::lighting::{GiField, GlobalLight, LightScene};

use super::grid_light::GlobalLightState;
use psxed_project::grid::mesh::MeshAsset;
use psxed_project::grid::{GridRoom, GridScale, GridWorld};
use psxed_project::Resource;
use psxed_ui::PaintGhost;

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
pub(super) struct GeometryContext {
    scale: GridScale,
    shading: FacingShade,
    global_light: GlobalLight,
    mesh_assets: Vec<MeshAsset>,
    /// The lights baked into the corners, runtime units.
    lights: Vec<GridLight>,
    /// Every material resource: tint, tiling, sidedness, blend and the name
    /// the fallback colour is chosen by.
    materials: Vec<Resource>,
    /// Texture slots, by the generation of `EditorTextures`.
    textures_generation: u64,
}

impl GeometryContext {
    fn of(
        world: &GridWorld,
        lights: Vec<GridLight>,
        project: &ProjectDocument,
        textures_generation: u64,
    ) -> Self {
        Self {
            scale: world.scale,
            shading: world.shading,
            global_light: world.global_light,
            mesh_assets: world.mesh_assets.clone(),
            lights,
            materials: Self::materials_of(project).cloned().collect(),
            textures_generation,
        }
    }

    fn materials_of(project: &ProjectDocument) -> impl Iterator<Item = &Resource> {
        project
            .resources
            .iter()
            .filter(|resource| matches!(resource.data, ResourceData::Material(_)))
    }

    /// Whether the context built from the arguments would equal this one,
    /// without building it (an edit that touches one room leaves the context
    /// alone, and must not pay to copy every material to find that out).
    fn is(
        &self,
        world: &GridWorld,
        lights: &[GridLight],
        project: &ProjectDocument,
        textures_generation: u64,
    ) -> bool {
        self.scale == world.scale
            && self.shading == world.shading
            && self.global_light == world.global_light
            && self.textures_generation == textures_generation
            && self.mesh_assets == world.mesh_assets
            && self.lights == lights
            && self.materials.iter().eq(Self::materials_of(project))
    }
}

/// A paint the author is about to make, shown as it will look: the room it
/// lands in, rebuilt from a copy of the world that took the paint through
/// the same write the click makes. The document is never touched.
pub(super) struct GhostRoom {
    paint: PaintGhost,
    /// Index of the room the paint lands in.
    pub(super) room: usize,
    /// The world with the paint written, for the faces of `prepared`.
    pub(super) world: GridWorld,
    pub(super) prepared: PreparedRoom,
}

/// The kept geometry of the active scene's grid world.
#[derive(Default)]
pub(super) struct GridGeometry {
    context: Option<GeometryContext>,
    rooms: Vec<RoomSlot>,
    /// The paint preview, while the Paint tool hovers a face.
    ghost: Option<GhostRoom>,
    /// What the light bake is shot against (the world's collision), built on
    /// the first bake after the world changed. `Err` holds why it could not
    /// be built, so a broken world does not retry every frame.
    scene: Option<Result<LightScene, String>>,
    /// The ambient occlusion, sky and bounce of a world that gathers its
    /// light, solved on a worker.
    gi: GlobalLightState,
    /// The project directory the textures came from, for the bounce colours.
    project_root: Option<std::path::PathBuf>,
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
        // The preview was made from a document that has changed.
        self.ghost = None;
        let Some(world) = project.active_scene().world.grid() else {
            self.context = None;
            self.rooms.clear();
            self.gi.clear();
            return;
        };
        let lights = collect_grid_bake_lights(project);
        // The shadows depend on the geometry alone: a moved lamp or a repainted
        // material leaves the scene (and its collision build) as it is.
        if !self
            .context
            .as_ref()
            .is_some_and(|c| c.scale == world.scale && c.mesh_assets == world.mesh_assets)
        {
            self.scene = None;
        }
        let unchanged = self
            .context
            .as_ref()
            .is_some_and(|context| context.is(world, &lights, project, textures_generation));
        if !unchanged {
            // Everything is made from the context: start over.
            self.rooms.clear();
            self.context = Some(GeometryContext::of(
                world,
                lights,
                project,
                textures_generation,
            ));
        }
        let mut gi_dirty = !unchanged || self.rooms.len() != world.rooms.len();
        if self.rooms.len() > world.rooms.len() {
            self.scene = None;
        }
        self.rooms.truncate(world.rooms.len());
        for (index, room) in world.rooms.iter().enumerate() {
            match self.rooms.get_mut(index) {
                Some(slot) if slot.source == *room => {}
                Some(slot) => {
                    slot.source = room.clone();
                    slot.prepared = None;
                    self.scene = None;
                    gi_dirty = true;
                }
                None => {
                    self.rooms.push(RoomSlot {
                        source: room.clone(),
                        prepared: None,
                    });
                    self.scene = None;
                    gi_dirty = true;
                }
            }
        }
        // A world that gathers its light solves it off this thread; anything
        // the solution is made from changing starts another solve.
        if !world.global_light.enabled {
            self.gi.clear();
        } else if gi_dirty || (self.gi.field().is_none() && !self.gi.pending()) {
            let lights = self.context.as_ref().map_or_else(Vec::new, |c| c.lights.clone());
            self.gi
                .restart(project, world, lights, self.project_root.clone());
        }
    }

    /// The rooms and the lights baked into them, for a frame to draw, and
    /// the paint preview that stands in for one of the rooms.
    pub(super) fn rooms_lights_ghost(
        &mut self,
        world: &GridWorld,
    ) -> (
        &mut [RoomSlot],
        &[GridLight],
        Option<&LightScene>,
        Option<&GiField>,
        Option<&mut GhostRoom>,
    ) {
        let scene = self.scene.get_or_insert_with(|| LightScene::new(world));
        let lights = self.context.as_ref().map_or(&[][..], |c| &c.lights[..]);
        (
            &mut self.rooms,
            lights,
            scene.as_ref().ok(),
            self.gi.field(),
            self.ghost.as_mut(),
        )
    }

    /// Tell the geometry where the project's texture files are, for the
    /// bounce colours of the global light.
    pub(super) fn set_project_root(&mut self, root: Option<&std::path::Path>) {
        if self.project_root.as_deref() != root {
            self.project_root = root.map(std::path::Path::to_path_buf);
        }
    }

    /// Take a finished global light solve, if one has landed, and drop the
    /// faces baked without it. True when the field changed.
    pub(super) fn poll_global_light(&mut self) -> bool {
        let changed = self.gi.poll();
        if changed {
            self.discard_prepared_faces();
        }
        changed
    }

    /// Wait for the global light solve in flight: for a one-shot frame.
    pub(super) fn finish_global_light(&mut self) {
        // The world may have changed since the last frame, so look first.
        if self.gi.wait() {
            self.discard_prepared_faces();
        }
    }

    fn discard_prepared_faces(&mut self) {
        for slot in &mut self.rooms {
            slot.prepared = None;
        }
        self.ghost = None;
    }

    /// Moves whenever the solved global light changes.
    pub(super) fn global_light_generation(&self) -> u64 {
        self.gi.generation()
    }

    /// Whether a global light solve is still running.
    pub(super) fn global_light_pending(&self) -> bool {
        self.gi.pending()
    }

    /// Show `paint` (or nothing): rebuild the preview room when the paint is
    /// not the one already kept. A paint that changes nothing leaves no
    /// preview.
    pub(super) fn set_ghost(&mut self, world: &GridWorld, paint: Option<&PaintGhost>) {
        let Some(paint) = paint else {
            self.ghost = None;
            return;
        };
        if self.ghost.as_ref().is_some_and(|g| g.paint == *paint) {
            return;
        }
        self.ghost = None;
        let Some(room) = paint.room().filter(|room| *room < world.rooms.len()) else {
            return;
        };
        let mut painted = world.clone();
        if !paint.apply(&mut painted) {
            return;
        }
        let prepared = super::grid::prepare_room(&painted, room);
        self.ghost = Some(GhostRoom {
            paint: paint.clone(),
            room,
            world: painted,
            prepared,
        });
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
