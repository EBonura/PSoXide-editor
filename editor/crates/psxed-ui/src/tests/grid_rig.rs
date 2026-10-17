//! A headless rig that drives the grid tools through real egui input, in
//! the Top view (`draw_viewport`) or the 3D view (`draw_viewport_3d_body`).
//! Keys go through the same `handle_grid_shortcuts` the window runs.

use super::*;
use psxed_project::grid::{GridRoom, GridScale, GridSurface, GridWall, GridWorld, SceneWorld};

/// The viewport rect `allocate_centered_preview_rect` produces for the
/// 800x600 test screen.
pub(crate) const RIG_VIEWPORT: Rect = Rect {
    min: Pos2::new(400.0 - 778.6667 / 2.0, 300.0 - 584.0 / 2.0),
    max: Pos2::new(400.0 + 778.6667 / 2.0, 300.0 + 584.0 / 2.0),
};

/// Which viewport the rig drives.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum RigView {
    Top,
    Persp,
}

pub(crate) struct GridRig {
    pub(crate) ws: EditorWorkspace,
    ctx: egui::Context,
    viewport: EditorViewport3dPresentation,
    view: RigView,
    time: f64,
    _texture: egui::TextureHandle,
}

/// Editor units per sector in the rig's worlds (scale 1/1).
pub(crate) const S: f32 = 1024.0;

/// A scale 1:1 grid project with one `width` x `depth` room of flat floor
/// at height 0, and optionally a ceiling at 3072.
pub(crate) fn flat_project(width: usize, depth: usize, ceiling: bool) -> ProjectDocument {
    let mut room = GridRoom::new("hall", width, depth);
    for z in 0..depth {
        for x in 0..width {
            let sector = room.sector_or_insert(x, z).unwrap();
            sector.floor = Some(GridSurface::flat(0, None));
            if ceiling {
                sector.ceiling = Some(GridSurface::flat(3072, None));
            }
        }
    }
    let mut project = ProjectDocument::new("grid tools");
    project.active_scene_mut().world = SceneWorld::Grid(GridWorld {
        scale: GridScale { num: 1, den: 1 },
        rooms: vec![room],
        ..GridWorld::default()
    });
    project
}

/// A project with an empty grid world (no rooms yet).
pub(crate) fn empty_project() -> ProjectDocument {
    let mut project = ProjectDocument::new("empty grid");
    project.active_scene_mut().world = SceneWorld::Grid(GridWorld {
        scale: GridScale { num: 1, den: 1 },
        rooms: Vec::new(),
        ..GridWorld::default()
    });
    project
}

impl GridRig {
    pub(crate) fn new(label: &str, project: ProjectDocument, view: RigView) -> Self {
        let mut ws = EditorWorkspace::with_project(test_temp_dir(label), project);
        ws.active_workspace = WorkspaceView::Room;
        // Tests place the view themselves.
        ws.grid.fit_pending = false;
        ws.view_2d = view == RigView::Top;
        ws.orthographic_view = OrthographicView::Top;
        ws.last_viewport_size = Vec2::new(1280.0, 720.0);
        ws.orthographic_focus = [3.0 * S, 0.0, 3.0 * S];
        ws.viewport_zoom = 0.1;
        ws.camera_rig.mode = ViewportCameraMode::Free;
        ws.camera_rig.free_initialized = true;
        ws.camera_rig.free_position = [3 * S as i32, 5200, 8 * S as i32];
        let (yaw, pitch) =
            camera_angles_to_look_at(ws.camera_rig.free_position, [3 * S as i32, 0, 2 * S as i32])
                .unwrap();
        ws.camera_rig.free_yaw = yaw;
        ws.camera_rig.free_pitch = pitch;
        let ctx = egui::Context::default();
        // The strip draws lucide icons; bind the family to a stock font.
        let mut fonts = egui::FontDefinitions::default();
        let proportional = fonts.families[&egui::FontFamily::Proportional].clone();
        fonts
            .families
            .insert(egui::FontFamily::Name("lucide".into()), proportional);
        ctx.set_fonts(fonts);
        let texture = ctx.load_texture(
            "grid-rig-viewport",
            egui::ColorImage::new([1, 1], egui::Color32::BLACK),
            egui::TextureOptions::NEAREST,
        );
        let viewport = EditorViewport3dPresentation::edit(texture.id(), Vec::new());
        let mut rig = Self {
            ws,
            ctx,
            viewport,
            view,
            time: 0.0,
            _texture: texture,
        };
        rig.pump(vec![], egui::Modifiers::NONE);
        rig
    }

    pub(crate) fn top(label: &str, project: ProjectDocument) -> Self {
        Self::new(label, project, RigView::Top)
    }

    pub(crate) fn persp(label: &str, project: ProjectDocument) -> Self {
        Self::new(label, project, RigView::Persp)
    }

    pub(crate) fn pump(&mut self, events: Vec<egui::Event>, modifiers: egui::Modifiers) {
        self.time += 1.0 / 60.0;
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO,
                if self.view == RigView::Top {
                    Vec2::new(1280.0, 800.0)
                } else {
                    Vec2::new(800.0, 600.0)
                },
            )),
            time: Some(self.time),
            modifiers,
            events,
            ..egui::RawInput::default()
        };
        let ws = &mut self.ws;
        let viewport = self.viewport.clone();
        let view = self.view;
        let _ = self.ctx.run(input, |ctx| {
            ws.handle_grid_shortcuts(ctx, false);
            match view {
                RigView::Top => {
                    ws.draw_viewport(ctx, viewport.clone(), EditorPlaytestStatus::Idle);
                }
                RigView::Persp => {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ws.draw_viewport_3d_body(ui, viewport.clone());
                    });
                }
            }
        });
    }

    /// Screen position of editor-space `(x, z)` in the Top view.
    pub(crate) fn top_at(&self, x: f32, z: f32) -> Pos2 {
        ViewportTransform::from_focus(
            self.ws.last_orthographic_viewport_rect,
            OrthographicView::Top,
            [self.ws.orthographic_focus[0], self.ws.orthographic_focus[2]],
            self.ws.viewport_zoom,
        )
        .world_to_screen([x, z])
    }

    /// Centre of sector `(cx, cz)` in the Top view.
    pub(crate) fn cell_centre(&self, cx: i32, cz: i32) -> Pos2 {
        self.top_at((cx as f32 + 0.5) * S, (cz as f32 + 0.5) * S)
    }

    /// Screen position of an editor-space point in the 3D view.
    pub(crate) fn screen_of(&self, p: [f32; 3]) -> Pos2 {
        let camera = self.ws.viewport_3d_camera();
        let rect = self.persp_rect();
        crate::grid_tools::view::project_point(camera, rect, p)
            .expect("point projects")
            .0
    }

    /// The rectangle the 3D view allocates on the rig screen.
    pub(crate) fn persp_rect(&self) -> Rect {
        RIG_VIEWPORT
    }

    pub(crate) fn move_to(&mut self, pos: Pos2, mods: egui::Modifiers) {
        self.pump(vec![egui::Event::PointerMoved(pos)], mods);
    }

    pub(crate) fn button(&mut self, pos: Pos2, pressed: bool, mods: egui::Modifiers) {
        self.pump(
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: mods,
            }],
            mods,
        );
    }

    pub(crate) fn click(&mut self, pos: Pos2) {
        self.click_with(pos, egui::Modifiers::NONE);
    }

    pub(crate) fn click_with(&mut self, pos: Pos2, mods: egui::Modifiers) {
        self.move_to(pos, mods);
        self.button(pos, true, mods);
        self.move_to(pos + Vec2::new(1.0, 0.0), mods);
        self.button(pos + Vec2::new(1.0, 0.0), false, mods);
    }

    pub(crate) fn drag(&mut self, from: Pos2, to: Pos2) {
        self.drag_with(from, to, egui::Modifiers::NONE);
    }

    pub(crate) fn drag_with(&mut self, from: Pos2, to: Pos2, mods: egui::Modifiers) {
        self.move_to(from, mods);
        self.button(from, true, mods);
        for step in 1..=8 {
            self.move_to(from + (to - from) * (step as f32 / 8.0), mods);
        }
        self.button(to, false, mods);
    }

    /// Press at `from` and drag to `to` but keep the button down, for
    /// inspecting a gesture mid-flight.
    pub(crate) fn drag_hold(&mut self, from: Pos2, to: Pos2, mods: egui::Modifiers) {
        self.move_to(from, mods);
        self.button(from, true, mods);
        for step in 1..=8 {
            self.move_to(from + (to - from) * (step as f32 / 8.0), mods);
        }
    }

    pub(crate) fn release(&mut self, at: Pos2, mods: egui::Modifiers) {
        self.button(at, false, mods);
    }

    pub(crate) fn key(&mut self, key: egui::Key) {
        self.key_with(key, egui::Modifiers::NONE);
    }

    pub(crate) fn key_with(&mut self, key: egui::Key, mods: egui::Modifiers) {
        for pressed in [true, false] {
            self.pump(
                vec![egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: mods,
                }],
                mods,
            );
        }
    }

    pub(crate) fn tool(&mut self, tool: crate::grid_tools::GridTool) {
        self.ws.set_grid_tool(tool);
        self.pump(vec![], egui::Modifiers::NONE);
    }

    pub(crate) fn world(&self) -> &GridWorld {
        self.ws
            .project
            .active_scene()
            .world
            .grid()
            .expect("grid world")
    }

    pub(crate) fn room(&self, index: usize) -> &GridRoom {
        &self.world().rooms[index]
    }
}

/// A sector's floor heights, by world cell.
pub(crate) fn floor_heights(world: &GridWorld, room: usize, cell: [i32; 2]) -> Option<[i32; 4]> {
    let r = world.rooms.get(room)?;
    let [x, z] = r.local(cell)?;
    r.sector(x, z)?.floor.as_ref().map(|f| f.heights)
}

pub(crate) fn walls_on(
    world: &GridWorld,
    room: usize,
    cell: [i32; 2],
    edge: psxed_project::grid::WallEdge,
) -> Vec<GridWall> {
    let Some(r) = world.rooms.get(room) else {
        return Vec::new();
    };
    r.local(cell)
        .and_then(|[x, z]| r.sector(x, z))
        .map(|s| s.walls(edge).clone())
        .unwrap_or_default()
}
