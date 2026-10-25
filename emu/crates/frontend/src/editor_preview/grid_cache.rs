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
use psxed_project::grid::lighting::{Gather, GiField, GlobalLight, LightScene, ProbeStore};

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

/// What a frame draws a grid world from: the kept rooms, and the lights, the
/// shadow scene and the gathered light its faces are baked with.
pub(super) struct RoomsToDraw<'a> {
    pub(super) rooms: &'a mut [RoomSlot],
    pub(super) lights: &'a [GridLight],
    pub(super) scene: Option<&'a LightScene>,
    pub(super) field: Option<&'a GiField>,
    pub(super) ghost: Option<&'a mut GhostRoom>,
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
    /// The light probes models are lit by, baked on the first model of a frame
    /// after the world or the gathered light changed. `Err` holds why they
    /// could not be, so a broken world does not retry every frame.
    probes: Option<Result<ProbeStore, String>>,
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
        self.probes = None;
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
            let lights = self
                .context
                .as_ref()
                .map_or_else(Vec::new, |c| c.lights.clone());
            self.gi
                .restart(project, world, lights, self.project_root.clone());
        }
    }

    /// The light probes of `world`, for the models of a frame to be lit by:
    /// baked through the same scene, lamps and gathered field as the faces,
    /// so a model stands in the light the cooked level gives it. `None` while
    /// the world has no probes (no floor) or its collision cannot be built.
    pub(super) fn probe_store(&mut self, world: &GridWorld) -> Option<&ProbeStore> {
        if self.probes.is_none() {
            let started = std::time::Instant::now();
            let scene = self.scene.get_or_insert_with(|| LightScene::new(world));
            let lights = self.context.as_ref().map_or(&[][..], |c| &c.lights[..]);
            let gather = world.global_light.enabled.then(|| Gather {
                field: self.gi.field(),
            });
            let built = match scene {
                Ok(scene) => scene.bake_probes(world, lights, gather),
                Err(error) => Err(error.clone()),
            };
            if let Err(error) = &built {
                eprintln!("[grid light] light probes: {error}");
            } else {
                eprintln!(
                    "[grid light] light probes baked in {:.1} ms",
                    started.elapsed().as_secs_f64() * 1000.0
                );
            }
            self.probes = Some(built);
        }
        self.probes.as_ref().and_then(|p| p.as_ref().ok())
    }

    /// The rooms and the lights baked into them, for a frame to draw, and
    /// the paint preview that stands in for one of the rooms.
    pub(super) fn rooms_lights_ghost(&mut self, world: &GridWorld) -> RoomsToDraw<'_> {
        let scene = self.scene.get_or_insert_with(|| {
            let started = std::time::Instant::now();
            let scene = LightScene::new(world);
            eprintln!(
                "[grid light] shadow scene (collision of {} rooms) built in {:.1} ms",
                world.rooms.len(),
                started.elapsed().as_secs_f64() * 1000.0
            );
            scene
        });
        let lights = self.context.as_ref().map_or(&[][..], |c| &c.lights[..]);
        RoomsToDraw {
            rooms: &mut self.rooms,
            lights,
            scene: scene.as_ref().ok(),
            field: self.gi.field(),
            ghost: self.ghost.as_mut(),
        }
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
        self.probes = None;
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

    fn gathering_project() -> ProjectDocument {
        let mut project = two_room_project();
        project
            .active_scene_mut()
            .world
            .grid_mut()
            .unwrap()
            .global_light = GlobalLight::NEW_WORLD;
        project
    }

    #[test]
    fn a_world_that_gathers_its_light_is_solved_in_the_background_and_baked_again() {
        let project = gathering_project();
        let mut geometry = GridGeometry::default();
        geometry.sync(&project, 1, 0);
        assert!(geometry.global_light_pending(), "the solve starts at once");
        let before = geometry.global_light_generation();
        prepare_all(&mut geometry);
        geometry.finish_global_light();
        assert!(!geometry.global_light_pending());
        assert_ne!(geometry.global_light_generation(), before);
        let drawn = geometry.rooms_lights_ghost(project.active_scene().world.grid().unwrap());
        assert!(drawn.field.is_some_and(|f| !f.is_empty()), "a field landed");
        // The faces baked without it are baked again.
        assert_eq!(prepared_rooms(&geometry), [false, false]);
    }

    #[test]
    fn an_edit_while_a_solve_runs_replaces_it_and_the_old_field_serves_until_then() {
        let mut project = gathering_project();
        let mut geometry = GridGeometry::default();
        geometry.sync(&project, 1, 0);
        geometry.finish_global_light();
        let landed = geometry.global_light_generation();
        project.active_scene_mut().world.grid_mut().unwrap().rooms[0]
            .sector_mut(0, 0)
            .and_then(|s| s.ground_floor_mut())
            .unwrap()
            .heights = [512; 4];
        geometry.sync(&project, 2, 0);
        assert!(geometry.global_light_pending());
        // Nothing replaced the field yet: the frame still uses it.
        assert_eq!(geometry.global_light_generation(), landed);
        geometry.finish_global_light();
        assert_ne!(geometry.global_light_generation(), landed);
        // Switching it off drops the field and bumps the generation.
        project
            .active_scene_mut()
            .world
            .grid_mut()
            .unwrap()
            .global_light
            .enabled = false;
        geometry.sync(&project, 3, 0);
        assert!(!geometry.global_light_pending());
        let drawn = geometry.rooms_lights_ghost(project.active_scene().world.grid().unwrap());
        assert!(drawn.field.is_none());
    }

    #[test]
    fn moving_a_lamp_keeps_the_shadow_scene_but_resolves_the_light() {
        let mut project = gathering_project();
        let mut geometry = GridGeometry::default();
        geometry.sync(&project, 1, 0);
        geometry.finish_global_light();
        let world = project.active_scene().world.grid().unwrap().clone();
        let _ = geometry.rooms_lights_ghost(&world);
        assert!(geometry.scene.as_ref().is_some_and(Result::is_ok));
        // A document change that is not geometry: a different scene name.
        project.active_scene_mut().name = "renamed".into();
        geometry.sync(&project, 2, 0);
        assert!(
            geometry.scene.as_ref().is_some_and(Result::is_ok),
            "the collision build is kept"
        );
    }

    /// A 4 by 1 hall with a wall across it after the second sector and a lamp
    /// over the first: the lit end and the walled-off end of one room.
    fn walled_hall_project() -> ProjectDocument {
        use psxed_project::grid::sector::WallEdge;
        use psxed_project::grid::GridWall;
        let mut room = GridRoom::new("hall", 4, 1);
        room.ambient = [32; 3];
        for x in 0..4 {
            room.sector_or_insert(x, 0)
                .unwrap()
                .set_ground_floor(Some(GridSurface::flat(0, None)));
        }
        room.sector_mut(1, 0)
            .unwrap()
            .walls_mut(WallEdge::East)
            .push(GridWall::span(0, 1024, None));
        let mut project = ProjectDocument::new("walled-hall");
        project.active_scene_mut().world = SceneWorld::Grid(GridWorld {
            rooms: vec![room],
            ..GridWorld::new()
        });
        let root = project.active_scene().root;
        let lamp = project.active_scene_mut().add_node(
            root,
            "Lamp",
            psxed_project::NodeKind::PointLight {
                color: [255, 255, 255],
                intensity: 1.0,
                radius: 4.0,
            },
        );
        // Over the middle of the first sector (1344 editor units a sector at
        // the default scale), at the height of a chest.
        project
            .active_scene_mut()
            .node_mut(lamp)
            .unwrap()
            .transform
            .translation = [672.0, 640.0, 672.0];
        project
    }

    #[test]
    fn the_probes_the_models_are_lit_by_carry_the_lamps_shadows() {
        let project = walled_hall_project();
        let mut geometry = GridGeometry::default();
        geometry.sync(&project, 1, 0);
        let world = project.active_scene().world.grid().unwrap().clone();
        let store = geometry.probe_store(&world).expect("the world has probes");
        // Runtime units: 84 a sector, a chest 32 up.
        let at = |sector: i32| store.light_at([sector * 84 + 42, 32, 42]).expect("a probe");
        let (under, beside, behind, far) = (at(0), at(1), at(2), at(3));
        assert!(under[0] > 200, "under the lamp: {under:?}");
        assert!(
            beside[0] > behind[0] + 60,
            "beside {beside:?}, behind the wall {behind:?}"
        );
        assert_eq!(behind, far, "past the wall only the ambient reaches");
        assert_eq!(behind, [32; 3]);
        // The probes are kept until something changes.
        assert!(geometry.probes.is_some());
        geometry.sync(&project, 2, 0);
        assert!(geometry.probes.is_none(), "an edit bakes them again");
    }
}
