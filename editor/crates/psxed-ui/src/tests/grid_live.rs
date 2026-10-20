//! Findings from driving the real editor window with screen control
//! (2026-10-20, project dp-world): the tool changing without being chosen,
//! the 3D view not showing a floor just drawn in the plan, and a corridor
//! that did not connect two rooms.
//!
//! The rig below draws the whole editor (`EditorWorkspace::draw`) in a
//! 1456 x 819 window, the size the coordinator's monitor reported, and feeds
//! it real pointer events.

use super::grid_rig::*;
use super::grid_screens::{fonts, save_png};
use super::raster::{render, Textures};
use super::*;
use crate::grid_tools::GridTool;
use psxed_project::grid::{GridRoom, GridSurface, WallMode};

const WINDOW: Vec2 = Vec2::new(1456.0, 819.0);

/// Stand-in for dp-world: the starter project, 650 palette materials, and a
/// second room "Machine Hall" (derived walls) at world sector [7, -1], 8 x 8.
fn dp_world() -> (EditorWorkspace, ScratchProjectDir) {
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
    for piece in 0..108 {
        for cell in 0..6 {
            ws.project.add_resource(
                &format!("DP {piece}.{cell}"),
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

/// The whole editor in a window, driven by pointer and key events.
struct WindowRig {
    ws: EditorWorkspace,
    ctx: egui::Context,
    viewport: EditorViewport3dPresentation,
    time: f64,
    out: egui::FullOutput,
    textures: Textures,
    _guard: ScratchProjectDir,
}

impl WindowRig {
    fn new() -> Self {
        let (ws, guard) = dp_world();
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

    fn frame(&mut self, events: Vec<egui::Event>) {
        self.time += 1.0 / 60.0;
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, WINDOW)),
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
    fn capture(&self, path: &Path) {
        let primitives = self
            .ctx
            .tessellate(self.out.shapes.clone(), self.out.pixels_per_point);
        let frame = render(
            &primitives,
            &self.textures,
            WINDOW.x as usize,
            WINDOW.y as usize,
        );
        save_png(path, &frame);
    }

    /// Centre of the text `label` as the last frame drew it.
    fn find(&self, label: &str) -> Option<Pos2> {
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

    fn at(&self, label: &str) -> Pos2 {
        self.find(label)
            .unwrap_or_else(|| panic!("no `{label}` on screen"))
    }

    fn texts(&self) -> Vec<String> {
        self.out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .collect()
    }

    fn move_to(&mut self, p: Pos2) {
        self.frame(vec![egui::Event::PointerMoved(p)]);
    }

    fn press(&mut self, p: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// A click with a frame between each step, as a real mouse arrives.
    fn click(&mut self, p: Pos2) {
        self.move_to(p);
        let e = self.press(p, true);
        self.frame(vec![e]);
        let e = self.press(p, false);
        self.frame(vec![e]);
        self.frame(vec![]);
    }

    /// A drag with a frame between the steps.
    fn drag(&mut self, from: Pos2, to: Pos2) {
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
    fn drag_one_frame(&mut self, from: Pos2, to: Pos2) {
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
    fn click_one_frame(&mut self, p: Pos2) {
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
    fn cell(&self, cx: i32, cz: i32) -> Pos2 {
        let sector = self.world().scale.to_editor(1024) as f32;
        ViewportTransform::from_focus(
            self.ws.last_orthographic_viewport_rect,
            OrthographicView::Top,
            [self.ws.orthographic_focus[0], self.ws.orthographic_focus[2]],
            self.ws.viewport_zoom,
        )
        .world_to_screen([(cx as f32 + 0.5) * sector, (cz as f32 + 0.5) * sector])
    }

    fn tool(&self) -> GridTool {
        self.ws.grid.tool
    }

    fn world(&self) -> &psxed_project::grid::GridWorld {
        self.ws.project.active_scene().world.grid().unwrap()
    }
}

// ---------------------------------------------------------------------
// Finding 3: a corridor drawn beside a room joins it, walls and all.

impl WindowRig {
    /// Pick the Floor tool with its button and drag from world cell `a` to
    /// `b` in the plan, as the coordinator did.
    fn draw_floor(&mut self, a: [i32; 2], b: [i32; 2]) {
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

    fn walls_on(&self, room: usize, cell: [i32; 2], edge: psxed_project::grid::WallEdge) -> usize {
        let room = &self.world().rooms[room];
        room.local(cell)
            .and_then(|[x, z]| room.sector(x, z))
            .map_or(0, |s| s.walls(edge).len())
    }

    fn has_floor(&self, room: usize, cell: [i32; 2]) -> bool {
        let room = &self.world().rooms[room];
        room.local(cell)
            .and_then(|[x, z]| room.sector(x, z))
            .is_some_and(|s| s.ground_floor().is_some())
    }
}

#[test]
fn a_corridor_drawn_beside_a_room_opens_that_rooms_walls_on_the_shared_edges() {
    use psxed_project::grid::WallEdge;
    let mut rig = WindowRig::new();
    assert_eq!(rig.walls_on(0, [5, 2], WallEdge::East), 1, "the fixture");
    assert_eq!(rig.walls_on(0, [5, 3], WallEdge::East), 1);
    let hall = rig.world().rooms[1].clone();
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
    // The other room is left alone.
    assert_eq!(rig.world().rooms[1], hall);
}

#[test]
fn the_status_says_what_was_opened_and_which_other_room_is_touched() {
    let mut rig = WindowRig::new();
    rig.draw_floor([6, 2], [6, 3]);
    let status = rig.ws.status.clone();
    assert!(
        status.contains("Created 2 floors in Starter room"),
        "{status}"
    );
    assert!(status.contains("opened 2 walls"), "{status}");
    assert!(status.contains("touches Machine Hall"), "{status}");
    assert!(status.contains("derived"), "{status}");
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
    rig.ws.do_redo();
    assert_eq!(rig.walls_on(0, [5, 2], WallEdge::East), 0);
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
    fn editor_centre(&self, a: [i32; 2], b: [i32; 2]) -> [f32; 3] {
        let sector = self.world().scale.to_editor(1024) as f32;
        [
            (a[0] + b[0] + 1) as f32 * 0.5 * sector,
            0.0,
            (a[1] + b[1] + 1) as f32 * 0.5 * sector,
        ]
    }

    /// Where `point` is on screen in the 3D view, when it is in front of
    /// the camera.
    fn persp_screen(&self, point: [f32; 3]) -> Option<Pos2> {
        let camera = self.ws.viewport_3d_camera();
        crate::grid_tools::view::project_point(camera, self.ws.last_persp_viewport_rect, point)
            .map(|(p, _)| p)
    }

    fn click_3d(&mut self) {
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
}
