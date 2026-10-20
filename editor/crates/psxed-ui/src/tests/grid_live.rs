//! Findings from driving the real editor window with screen control
//! (2026-10-20, project dp-world): the tool changing without being chosen,
//! the 3D view not showing a floor just drawn in the plan, and a corridor
//! that did not connect two rooms.
//!
//! The rig below draws the whole editor (`EditorWorkspace::draw`) in a
//! 1456 x 819 window, the size the coordinator's monitor reported, and feeds
//! it real pointer events.

use super::grid_screens::{fonts, save_png};
use super::raster::{render, Textures};
use super::*;
use crate::grid_tools::GridTool;
use psxed_project::grid::{GridRoom, GridSurface, WallMode};

pub(super) const WINDOW: Vec2 = Vec2::new(1456.0, 819.0);

/// Stand-in for dp-world: the starter project, 650 palette materials, and a
/// second room "Machine Hall" (derived walls) at world sector [7, -1], 8 x 8.
pub(super) fn dp_world() -> (EditorWorkspace, ScratchProjectDir) {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "psxed-ui-live-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let guard = ScratchProjectDir::new(dir.clone());
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../projects/grid-starter");
    crate::starter_catalogue::copy_dir_recursive(&source, &dir).unwrap();
    let mut ws = EditorWorkspace::open_directory(&dir).unwrap();
    // Manny's dp-world still has the starter room as it was made before
    // derived walls: hand-placed walls round it, so raising a floor there
    // makes no risers. The tracked template has been converted since, so this
    // stand-in puts the legacy room back.
    let legacy = legacy_starter_room(&ws.project);
    ws.project
        .active_scene_mut()
        .world
        .grid_mut()
        .unwrap()
        .rooms[0] = legacy;
    for piece in 0..108 {
        for cell in 0..6 {
            ws.project.add_resource(
                format!("DP {piece}.{cell}"),
                psxed_project::ResourceData::Material(psxed_project::MaterialResource::opaque(
                    Some(format!("level_textures/{}.psxt", 10 + piece * 6 + cell)),
                )),
            );
        }
    }
    // The hall's floor takes another Graybox material than the starter room's.
    let hall_floor = ws
        .project
        .resources
        .iter()
        .find(|r| r.name == "Graybox / Wall 64")
        .map(|r| r.id);
    let mut hall = GridRoom::new("Machine Hall", 8, 8);
    hall.origin = [7, -1];
    hall.wall_mode = WallMode::Derived;
    for z in 0..8 {
        for x in 0..8 {
            let sector = hall.sector_or_insert(x, z).unwrap();
            sector.set_ground_floor(Some(GridSurface::flat(0, hall_floor)));
            sector.set_ground_ceiling(Some(GridSurface::flat(3072, None)));
        }
    }
    ws.project
        .active_scene_mut()
        .world
        .grid_mut()
        .unwrap()
        .rooms
        .push(hall);
    ws.active_workspace = WorkspaceView::Room;
    ws.frame_current_view();
    ws.status = String::new();
    (ws, guard)
}

/// The starter room the way it was authored before derived walls: 6 x 6
/// sectors of floor and ceiling (the starter's materials) with a hand-placed
/// wall on every edge of the outline, `Authored` wall mode.
pub(super) fn legacy_starter_room(project: &ProjectDocument) -> GridRoom {
    use psxed_project::grid::WallEdge;
    let material = |raw: u64| {
        project
            .resources
            .iter()
            .find(|r| r.id.raw() == raw)
            .map(|r| r.id)
    };
    let (floor, wall, ceiling) = (material(159), material(160), material(161));
    let mut room = GridRoom::new("Starter room", 6, 6);
    room.ambient = [96, 96, 96];
    for z in 0..6 {
        for x in 0..6 {
            let sector = room.sector_or_insert(x, z).unwrap();
            sector.set_ground_floor(Some(GridSurface::flat(0, floor)));
            sector.set_ground_ceiling(Some(GridSurface::flat(2048, ceiling)));
            for (edge, outside) in [
                (WallEdge::North, z == 0),
                (WallEdge::South, z == 5),
                (WallEdge::West, x == 0),
                (WallEdge::East, x == 5),
            ] {
                if outside {
                    sector
                        .walls_mut(edge)
                        .push(psxed_project::grid::GridWall::span(0, 2048, wall));
                }
            }
        }
    }
    room
}

/// The whole editor in a window, driven by pointer and key events.
pub(super) struct WindowRig {
    pub(super) ws: EditorWorkspace,
    pub(super) ctx: egui::Context,
    pub(super) viewport: EditorViewport3dPresentation,
    pub(super) size: Vec2,
    pub(super) time: f64,
    pub(super) out: egui::FullOutput,
    pub(super) textures: Textures,
    pub(super) _guard: ScratchProjectDir,
}

impl WindowRig {
    pub(super) fn new() -> Self {
        Self::with_size(WINDOW)
    }

    pub(super) fn with_size(size: Vec2) -> Self {
        let (ws, guard) = dp_world();
        Self::with_world(ws, guard, size)
    }

    /// The project in directory `source` (copied to a scratch directory) in
    /// a window.
    pub(super) fn from_dir(source: &Path, size: Vec2) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "psxed-ui-live-dir-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let guard = ScratchProjectDir::new(dir.clone());
        crate::starter_catalogue::copy_dir_recursive(source, &dir).unwrap();
        let mut ws = EditorWorkspace::open_directory(&dir).unwrap();
        ws.active_workspace = WorkspaceView::Room;
        ws.frame_current_view();
        ws.status = String::new();
        Self::with_world(ws, guard, size)
    }

    /// The tracked starter project in a window.
    pub(super) fn starter(size: Vec2) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "psxed-ui-live-starter-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let guard = ScratchProjectDir::new(dir.clone());
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../projects/grid-starter");
        crate::starter_catalogue::copy_dir_recursive(&source, &dir).unwrap();
        let mut ws = EditorWorkspace::open_directory(&dir).unwrap();
        ws.active_workspace = WorkspaceView::Room;
        ws.frame_current_view();
        ws.status = String::new();
        Self::with_world(ws, guard, size)
    }

    fn with_world(ws: EditorWorkspace, guard: ScratchProjectDir, size: Vec2) -> Self {
        let ctx = egui::Context::default();
        ctx.set_fonts(fonts());
        let texture = ctx.load_texture(
            "live-viewport",
            egui::ColorImage::new([1, 1], egui::Color32::BLACK),
            egui::TextureOptions::NEAREST,
        );
        let viewport = EditorViewport3dPresentation::edit(texture.id(), Vec::new());
        let mut rig = Self {
            ws,
            ctx,
            viewport,
            size,
            time: 0.0,
            out: egui::FullOutput::default(),
            textures: Textures::default(),
            _guard: guard,
        };
        for _ in 0..4 {
            rig.frame(vec![]);
        }
        rig
    }

    pub(super) fn frame(&mut self, events: Vec<egui::Event>) {
        self.time += 1.0 / 60.0;
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
            time: Some(self.time),
            focused: true,
            events,
            ..egui::RawInput::default()
        };
        let ws = &mut self.ws;
        let viewport = self.viewport.clone();
        self.out = self.ctx.run(input, |ctx| {
            ws.draw(ctx, viewport.clone(), EditorPlaytestStatus::Idle);
        });
        self.textures.apply(&self.out.textures_delta);
    }

    /// Rasterise the last frame to a PNG (CPU, same renderer as the goldens).
    pub(super) fn capture(&self, path: &Path) {
        let primitives = self
            .ctx
            .tessellate(self.out.shapes.clone(), self.out.pixels_per_point);
        let frame = render(
            &primitives,
            &self.textures,
            self.size.x as usize,
            self.size.y as usize,
        );
        save_png(path, &frame);
    }

    /// Centre of the text `label` as the last frame drew it.
    pub(super) fn find(&self, label: &str) -> Option<Pos2> {
        self.out.shapes.iter().find_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t)
                if t.galley
                    .text()
                    .trim_start_matches(|c: char| !c.is_alphanumeric())
                    .trim()
                    == label =>
            {
                Some(t.visual_bounding_rect().center())
            }
            _ => None,
        })
    }

    pub(super) fn at(&self, label: &str) -> Pos2 {
        self.find(label)
            .unwrap_or_else(|| panic!("no `{label}` on screen"))
    }

    pub(super) fn move_to(&mut self, p: Pos2) {
        self.frame(vec![egui::Event::PointerMoved(p)]);
    }

    pub(super) fn press(&mut self, p: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// A click with a frame between each step, as a real mouse arrives.
    pub(super) fn click(&mut self, p: Pos2) {
        self.move_to(p);
        let e = self.press(p, true);
        self.frame(vec![e]);
        let e = self.press(p, false);
        self.frame(vec![e]);
        self.frame(vec![]);
    }

    /// A drag with a frame between the steps.
    pub(super) fn drag(&mut self, from: Pos2, to: Pos2) {
        self.move_to(from);
        let e = self.press(from, true);
        self.frame(vec![e]);
        for step in 1..=6 {
            self.move_to(from + (to - from) * (step as f32 / 6.0));
        }
        let e = self.press(to, false);
        self.frame(vec![e]);
        self.frame(vec![]);
    }

    /// The whole drag delivered in ONE frame, as a synthetic input driver
    /// does: press, move, release together.
    pub(super) fn drag_one_frame(&mut self, from: Pos2, to: Pos2) {
        self.move_to(from);
        let events = vec![
            egui::Event::PointerMoved(from),
            self.press(from, true),
            egui::Event::PointerMoved(to),
            self.press(to, false),
        ];
        self.frame(events);
        self.frame(vec![]);
    }

    /// A click delivered in ONE frame.
    pub(super) fn click_one_frame(&mut self, p: Pos2) {
        self.move_to(p);
        let events = vec![
            egui::Event::PointerMoved(p),
            self.press(p, true),
            self.press(p, false),
        ];
        self.frame(events);
        self.frame(vec![]);
    }

    /// Screen centre of world cell `(cx, cz)` in the plan.
    pub(super) fn cell(&self, cx: i32, cz: i32) -> Pos2 {
        let sector = self.world().scale.to_editor(1024) as f32;
        ViewportTransform::from_focus(
            self.ws.last_orthographic_viewport_rect,
            OrthographicView::Top,
            [self.ws.orthographic_focus[0], self.ws.orthographic_focus[2]],
            self.ws.viewport_zoom,
        )
        .world_to_screen([(cx as f32 + 0.5) * sector, (cz as f32 + 0.5) * sector])
    }

    pub(super) fn tool(&self) -> GridTool {
        self.ws.grid.tool
    }

    pub(super) fn world(&self) -> &psxed_project::grid::GridWorld {
        self.ws.project.active_scene().world.grid().unwrap()
    }
}

// ---------------------------------------------------------------------
// Finding 3: a corridor drawn beside a room joins it, walls and all.

impl WindowRig {
    /// Pick the Floor tool with its button and drag from world cell `a` to
    /// `b` in the plan, as the coordinator did.
    pub(super) fn draw_floor(&mut self, a: [i32; 2], b: [i32; 2]) {
        self.ws.apply_grid_pose("view=plan").unwrap();
        for _ in 0..3 {
            self.frame(vec![]);
        }
        let floor = self.at("Floor");
        self.click(floor);
        for _ in 0..3 {
            self.frame(vec![]);
        }
        assert_eq!(self.tool(), GridTool::Floor);
        let (from, to) = (self.cell(a[0], a[1]), self.cell(b[0], b[1]));
        self.drag(from, to);
    }

    pub(super) fn walls_on(
        &self,
        room: usize,
        cell: [i32; 2],
        edge: psxed_project::grid::WallEdge,
    ) -> usize {
        let room = &self.world().rooms[room];
        room.local(cell)
            .and_then(|[x, z]| room.sector(x, z))
            .map_or(0, |s| s.walls(edge).len())
    }

    pub(super) fn has_floor(&self, room: usize, cell: [i32; 2]) -> bool {
        let room = &self.world().rooms[room];
        room.local(cell)
            .and_then(|[x, z]| room.sector(x, z))
            .is_some_and(|s| s.ground_floor().is_some())
    }
}

#[test]
fn a_corridor_drawn_beside_a_room_opens_that_rooms_authored_walls_on_the_shared_edges() {
    use psxed_project::grid::WallEdge;
    let mut rig = WindowRig::new();
    assert_eq!(rig.walls_on(0, [5, 2], WallEdge::East), 1, "the fixture");
    assert_eq!(rig.walls_on(0, [5, 3], WallEdge::East), 1);
    rig.draw_floor([6, 2], [6, 3]);
    assert!(
        rig.has_floor(0, [6, 2]) && rig.has_floor(0, [6, 3]),
        "{}",
        rig.ws.status
    );
    assert_eq!(
        rig.walls_on(0, [5, 2], WallEdge::East),
        0,
        "the east wall still closes the corridor ({})",
        rig.ws.status
    );
    assert_eq!(rig.walls_on(0, [5, 3], WallEdge::East), 0);
    // The walls elsewhere on that side stay.
    assert_eq!(rig.walls_on(0, [5, 1], WallEdge::East), 1);
    assert_eq!(rig.walls_on(0, [5, 4], WallEdge::East), 1);
}

impl WindowRig {
    /// Derived walls of room `room` on `edge` of `cell` that still close it:
    /// shown, or hidden but still solid.
    fn closing_derived_walls(
        &self,
        room: usize,
        cell: [i32; 2],
        edge: psxed_project::grid::WallEdge,
    ) -> usize {
        psxed_project::grid::leaf_derive::derived_walls_of(
            &self.world().rooms[room],
            psxed_project::grid::LeafKey::sector(cell),
        )
        .iter()
        .filter(|w| w.edge == edge && (!w.hidden || w.blocks()))
        .count()
    }
}

#[test]
fn the_corridor_leaves_no_wall_at_either_end() {
    use psxed_project::grid::WallEdge;
    let mut rig = WindowRig::new();
    // Before: the starter room's east wall closes one end, the hall's derived
    // west wall the other.
    assert_eq!(rig.closing_derived_walls(1, [7, 2], WallEdge::West), 1);
    assert_eq!(rig.closing_derived_walls(1, [7, 3], WallEdge::West), 1);
    rig.draw_floor([6, 2], [6, 3]);
    assert_eq!(
        rig.walls_on(0, [5, 2], WallEdge::East),
        0,
        "{}",
        rig.ws.status
    );
    assert_eq!(rig.walls_on(0, [5, 3], WallEdge::East), 0);
    assert_eq!(
        rig.closing_derived_walls(1, [7, 2], WallEdge::West),
        0,
        "the hall still closes the corridor ({})",
        rig.ws.status
    );
    assert_eq!(rig.closing_derived_walls(1, [7, 3], WallEdge::West), 0);
    // The rest of the hall's west side stays closed.
    for z in [1, 4] {
        assert_eq!(rig.closing_derived_walls(1, [7, z], WallEdge::West), 1);
    }
}

#[test]
fn the_status_counts_the_walls_opened_in_each_room() {
    let mut rig = WindowRig::new();
    rig.draw_floor([6, 2], [6, 3]);
    assert_eq!(
        rig.ws.status,
        "Created 2 floors in Starter room, opened 2 walls in Starter room and 2 in Machine Hall"
    );
}

#[test]
fn a_floor_much_higher_than_its_neighbours_keeps_their_walls() {
    use psxed_project::grid::WallEdge;
    let mut rig = WindowRig::new();
    // 2048 units up is well past what the player steps.
    rig.ws.grid.floor_height = 2048;
    rig.draw_floor([6, 2], [6, 3]);
    assert!(rig.has_floor(0, [6, 2]), "{}", rig.ws.status);
    assert_eq!(rig.walls_on(0, [5, 2], WallEdge::East), 1);
    assert_eq!(rig.closing_derived_walls(1, [7, 2], WallEdge::West), 1);
    assert!(rig.ws.status.contains("kept 4 walls"), "{}", rig.ws.status);
}

#[test]
fn opening_the_walls_is_one_undo_step() {
    use psxed_project::grid::WallEdge;
    let mut rig = WindowRig::new();
    let before = rig.world().clone();
    rig.draw_floor([6, 2], [6, 3]);
    assert_ne!(*rig.world(), before);
    rig.ws.do_undo();
    assert_eq!(*rig.world(), before, "one undo restores floors and walls");
    assert_eq!(rig.walls_on(0, [5, 2], WallEdge::East), 1);
    assert_eq!(rig.closing_derived_walls(1, [7, 2], WallEdge::West), 1);
    rig.ws.do_redo();
    assert_eq!(rig.walls_on(0, [5, 2], WallEdge::East), 0);
    assert_eq!(rig.closing_derived_walls(1, [7, 2], WallEdge::West), 0);
}

#[test]
fn a_floor_that_touches_no_room_opens_nothing() {
    let mut rig = WindowRig::new();
    // Cells (6,8) and (6,9) are south-east of the starter room, clear of both.
    let walls = rig.world().rooms[0].clone();
    rig.draw_floor([6, 8], [6, 9]);
    assert_eq!(rig.ws.status, "Created 2 floors in Starter room");
    // Only the room's size changed: every old wall is still there.
    let after = &rig.world().rooms[0];
    for (x, z, sector) in walls.iter() {
        let new = after
            .local(walls.world_cell(x, z))
            .and_then(|[x, z]| after.sector(x, z))
            .unwrap();
        assert_eq!(new.wall_count(), sector.wall_count());
    }
}

// ---------------------------------------------------------------------
// Finding 2: the floor drawn in the plan was not in view when the 3D button
// was pressed. (The preview itself is not stale: the frontend's
// `a_floor_added_between_two_live_frames_shows_in_the_second` covers that.)

impl WindowRig {
    /// Editor-space centre of world cells `a`..`b` on the floor.
    pub(super) fn editor_centre(&self, a: [i32; 2], b: [i32; 2]) -> [f32; 3] {
        let sector = self.world().scale.to_editor(1024) as f32;
        [
            (a[0] + b[0] + 1) as f32 * 0.5 * sector,
            0.0,
            (a[1] + b[1] + 1) as f32 * 0.5 * sector,
        ]
    }

    /// Where `point` is on screen in the 3D view, when it is in front of
    /// the camera.
    pub(super) fn persp_screen(&self, point: [f32; 3]) -> Option<Pos2> {
        let camera = self.ws.viewport_3d_camera();
        crate::grid_tools::view::project_point(camera, self.ws.last_persp_viewport_rect, point)
            .map(|(p, _)| p)
    }

    pub(super) fn click_3d(&mut self) {
        let button = self.at("3D");
        self.click(button);
        for _ in 0..3 {
            self.frame(vec![]);
        }
        assert!(!self.ws.view_2d, "the 3D button switched the view");
    }
}

#[test]
fn the_3d_view_after_a_plan_edit_looks_at_the_new_floor() {
    let mut rig = WindowRig::new();
    rig.draw_floor([6, 2], [6, 3]);
    rig.click_3d();
    let corridor = rig.editor_centre([6, 2], [6, 3]);
    let rect = rig.ws.last_persp_viewport_rect;
    let at = rig.persp_screen(corridor);
    assert!(
        at.is_some_and(|p| rect.shrink(rect.width() * 0.2).contains(p)),
        "the corridor is not in the middle of the 3D view: {at:?} in {rect:?}"
    );
    // Seen from above, not from behind the room beside it.
    let pitch_degrees = (4096 - u32::from(rig.ws.camera_rig.pitch)) * 360 / 4096;
    assert!(
        pitch_degrees >= 45,
        "the view looks down at {pitch_degrees} degrees"
    );
    assert!(rig.ws.status.contains("framed"), "{}", rig.ws.status);
}

#[test]
fn the_view_frames_an_edit_once_and_then_leaves_the_camera_alone() {
    let mut rig = WindowRig::new();
    rig.draw_floor([6, 2], [6, 3]);
    rig.click_3d();
    let framed = rig.ws.camera_rig.target;
    // The user orbits somewhere else, goes to the plan and back with no edit.
    rig.ws.camera_rig.target = [0, 0, 0];
    let plan = rig.at("Plan");
    rig.click(plan);
    assert!(rig.ws.view_2d);
    rig.click_3d();
    assert_eq!(
        rig.ws.camera_rig.target,
        [0, 0, 0],
        "was {framed:?} when framed"
    );
}

#[test]
fn an_edit_made_in_the_3d_view_does_not_move_its_camera() {
    let mut rig = WindowRig::new();
    rig.ws.apply_grid_pose("view=3d").unwrap();
    for _ in 0..3 {
        rig.frame(vec![]);
    }
    let floor = rig.at("Floor");
    rig.click(floor);
    let before = rig.ws.camera_rig.target;
    let a = rig.persp_screen(rig.editor_centre([6, 2], [6, 2])).unwrap();
    let b = rig.persp_screen(rig.editor_centre([6, 3], [6, 3])).unwrap();
    rig.drag(a, b);
    assert!(rig.ws.status.starts_with("Created"), "{}", rig.ws.status);
    let plan = rig.at("Plan");
    rig.click(plan);
    rig.click_3d();
    assert_eq!(rig.ws.camera_rig.target, before, "that edit was seen in 3D");
}

// ---------------------------------------------------------------------
// Finding 1: the tool changed without being chosen.
//
// Not reproduced by pointer input alone (see `no_pointer_gesture_changes_the_tool`),
// so these pin what can be pinned: the header's buttons do not sit on top of
// each other (the view buttons overlapped the workspace tabs at the coordinator's
// 1456-point window, so a click there was ambiguous), and a tool key cannot swap
// the tool in the middle of a drag.

impl WindowRig {
    /// Visible text of the viewport header's view buttons, as
    /// `(text, bounds)`: the first row under the tool row.
    pub(super) fn header_texts(&self) -> Vec<(String, Rect)> {
        let top = ACTION_BAR_COMPACT_HEIGHT + TOOL_ROW_HEIGHT;
        let tools_y = top + 36.0;
        self.out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) => {
                    Some((t.galley.text().to_string(), t.visual_bounding_rect()))
                }
                _ => None,
            })
            .filter(|(_, r)| {
                r.min.y > top
                    && r.max.y < tools_y
                    && r.min.x > 290.0
                    && r.max.x < self.size.x - 330.0
            })
            .collect()
    }
}

#[test]
fn the_header_buttons_never_overlap_at_any_window_width() {
    for width in [1280.0, 1456.0, 1920.0] {
        let mut rig = WindowRig::with_size(Vec2::new(width, 819.0));
        rig.ws.apply_grid_pose("view=plan").unwrap();
        for _ in 0..4 {
            rig.frame(vec![]);
        }
        let texts = rig.header_texts();
        assert!(texts.len() >= 7, "{width}: {texts:?}");
        for (i, (a, ra)) in texts.iter().enumerate() {
            for (b, rb) in &texts[i + 1..] {
                assert!(
                    !ra.intersects(*rb),
                    "at {width} points wide `{a}` {ra:?} is drawn over `{b}` {rb:?}"
                );
            }
        }
    }
}

/// Every pointer gesture the window can receive, anywhere, with one frame
/// per event or all of them in one: the tool changes only when the press and
/// the release land on the same tool button.
#[test]
fn no_pointer_gesture_changes_the_tool_unless_it_clicks_a_tool_button() {
    let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
    let mut next = move || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 33) as u32
    };
    for round in 0..24 {
        let mut rig = WindowRig::new();
        let view = ["view=plan", "view=3d", "view=split"][round % 3];
        rig.ws.apply_grid_pose(view).unwrap();
        rig.ws
            .set_grid_tool(GridTool::ALL[(round * 7) % GridTool::ALL.len()]);
        for _ in 0..3 {
            rig.frame(vec![]);
        }
        for step in 0..40 {
            let before = rig.tool();
            let a = Pos2::new(next() as f32 % 1456.0, next() as f32 % 819.0);
            let b = Pos2::new(next() as f32 % 1456.0, next() as f32 % 819.0);
            // The tool buttons' neighbourhood, from the frame before the gesture.
            let near_a_button = |rig: &WindowRig, p: Pos2| {
                GridTool::ALL.iter().any(|t| {
                    [
                        t.label(),
                        if *t == GridTool::Split {
                            "Diagonal"
                        } else {
                            ""
                        },
                    ]
                    .iter()
                    .filter(|l| !l.is_empty())
                    .filter_map(|l| rig.find(l))
                    .any(|c| (c.x - p.x).abs() < 60.0 && (c.y - p.y).abs() < 22.0)
                })
            };
            let on_strip = near_a_button(&rig, a) || near_a_button(&rig, b);
            match next() % 4 {
                0 => rig.click(a),
                1 => rig.drag(a, b),
                2 => rig.click_one_frame(a),
                _ => rig.drag_one_frame(a, b),
            }
            // A placement that lands returns to Select, by design.
            let placed = before == GridTool::Place && rig.ws.status.starts_with("Placed");
            assert!(
                rig.tool() == before || on_strip || placed,
                "round {round} step {step}: {before:?} became {:?} from a gesture at {a:?} / {b:?} ({})",
                rig.tool(),
                rig.ws.status
            );
        }
    }
}

#[test]
fn a_tool_key_in_the_middle_of_a_drag_does_not_swap_the_tool() {
    let mut rig = WindowRig::new();
    rig.ws.apply_grid_pose("view=plan").unwrap();
    for _ in 0..3 {
        rig.frame(vec![]);
    }
    let floor = rig.at("Floor");
    rig.click(floor);
    let (a, b) = (rig.cell(6, 2), rig.cell(6, 3));
    rig.move_to(a);
    let e = rig.press(a, true);
    rig.frame(vec![e]);
    rig.move_to(b);
    // E is the Bands tool and O the Place tool, while the button is down.
    for key in [egui::Key::E, egui::Key::O] {
        rig.frame(vec![egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
    }
    assert_eq!(rig.tool(), GridTool::Floor, "{}", rig.ws.status);
    let e = rig.press(b, false);
    rig.frame(vec![e]);
    assert!(
        rig.has_floor(0, [6, 2]) && rig.has_floor(0, [6, 3]),
        "the drag finished"
    );
    // With the button up the keys work again.
    rig.frame(vec![egui::Event::Key {
        key: egui::Key::E,
        physical_key: Some(egui::Key::E),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    assert_eq!(rig.tool(), GridTool::Bands);
}

/// Captures for the review pages: `PSXED_LIVE_CAPTURE=<dir> PSXED_LIVE_LABEL=before|after`
/// writes one PNG per finding; without the variable this does nothing.
#[test]
fn live_captures() {
    let Some(dir) = std::env::var_os("PSXED_LIVE_CAPTURE") else {
        return;
    };
    let dir = PathBuf::from(dir);
    let label = std::env::var("PSXED_LIVE_LABEL").unwrap_or_else(|_| "after".into());

    // Finding 3: the corridor in the plan, and what the status says.
    let mut rig = WindowRig::new();
    rig.draw_floor([6, 2], [6, 3]);
    for _ in 0..3 {
        rig.frame(vec![]);
    }
    rig.capture(&dir.join(format!("f3-corridor-plan-{label}.png")));

    // Finding 2: the same edit, then the 3D button. The project and its view
    // file go to disk so `frontend dump-editor-ui` can render the real 3D view.
    let three_d = rig.at("3D");
    rig.click(three_d);
    for _ in 0..3 {
        rig.frame(vec![]);
    }
    rig.capture(&dir.join(format!("f2-after-3d-click-ui-{label}.png")));
    rig.ws.save().expect("save");
    rig.ws.save_view_state().expect("view");
    let project = dir.join(format!("proj-f2-{label}"));
    let _ = std::fs::remove_dir_all(&project);
    crate::starter_catalogue::copy_dir_recursive(&rig.ws.project_dir, &project).unwrap();

    // Finding 1: the header at the coordinator's window and at a narrower one.
    for width in [1456.0, 1280.0] {
        let mut rig = WindowRig::with_size(Vec2::new(width, 819.0));
        rig.ws.apply_grid_pose("view=plan").unwrap();
        for _ in 0..4 {
            rig.frame(vec![]);
        }
        rig.capture(&dir.join(format!("f1-header-{width}-{label}.png")));
    }

    // Finding 3: the open corridor in 3D, from four sides (the projects go to
    // disk with their view file for `frontend dump-editor-ui`).
    let mut rig = WindowRig::new();
    rig.draw_floor([6, 2], [6, 3]);
    let centre = rig.editor_centre([6, 2], [6, 3]);
    rig.ws.camera_rig.mode = crate::ViewportCameraMode::Orbit;
    rig.ws.camera_rig.target = [centre[0] as i32, 600, centre[2] as i32];
    rig.ws.camera_rig.radius = 4200;
    rig.ws.camera_rig.pitch = 4096 - 150;
    rig.ws.save().expect("save");
    for (name, yaw) in [("a", 0u16), ("b", 1024), ("c", 2048), ("d", 3072)] {
        rig.ws.camera_rig.yaw = yaw;
        rig.ws.save_view_state().expect("view");
        let project = dir.join(format!("proj-f3-{name}-{label}"));
        let _ = std::fs::remove_dir_all(&project);
        crate::starter_catalogue::copy_dir_recursive(&rig.ws.project_dir, &project).unwrap();
    }
}
