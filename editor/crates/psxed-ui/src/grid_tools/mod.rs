//! The grid world editing tools.
//!
//! One strip of tools in three groups (Pick, Draw, Build: Select, Height;
//! Floor, Ceiling, Wall, Plane, Paint, Split, Place, Bands; Raise and the
//! other level verbs), each written once and driven from both the Top view and
//! the 3D view through [`view::GridPointer`]. The document edits themselves
//! are the pure functions in `psxed_project::grid::edit`; this module owns
//! the gestures, the picking, the overlays and the panels around them.
//!
//! State lives in one [`GridEditState`] on the workspace, so the headless
//! tests drive exactly what the window does.
//!
//! * [`view`]: the pointer, projection and ray helpers.
//! * [`pick`]: picking faces, edges, vertices, cells and wall lines.
//! * [`events`]: routing pointer events from both views to the tools.

use super::*;

use std::cell::RefCell;

use psxed_project::grid::edit::{
    GridClipboard, GridSelection, HeightDrag, PlaneClipboard, PlaneId, SurfaceKind,
};
use psxed_project::grid::{GridPlane, PlaneCollision, WallEdge};

mod analysis;
mod band_panel;
mod bands_tool;
mod break_tool;
mod clipboard_tool;
mod cycle;
pub(crate) mod events;
mod face_tools;
mod heat_overlay;
mod height_tool;
mod help;
mod keys;
mod lasso;
mod mesh_overlay;
mod mesh_panel;
mod mesh_tool;
mod overlay;
mod panels;
pub(crate) mod pick;
mod place_tool;
mod plane_math;
mod plane_overlay;
mod plane_panel;
mod plane_pick;
mod plane_tool;
mod pose;
mod ramp_tool;
mod rect_tool;
mod rooms;
mod rooms_walls;
mod rough_panel;
mod rough_tool;
mod rules_panel;
mod select_tool;
mod shading_panel;
mod stairs_tool;
mod strip;
mod tiling_panel;
mod triangle_tool;
mod verb_overlay;
mod verb_panel;
pub(crate) mod verb_state;
pub(crate) mod verb_tool;
pub(crate) mod view;
mod walk_overlay;
mod wall_tool;

pub(crate) use events::{grid_tool_2d, grid_tool_3d};
use pick::PickCache;
pub use rooms::grid_preview_hidden_rooms;
use view::{GridPointer, GridView};

/// The grid tools, in strip order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GridTool {
    /// Pick, range-select, box-select, shift-drag heights.
    Select,
    /// Drag a rectangle of floor.
    Floor,
    /// Drag a rectangle of ceiling.
    Ceiling,
    /// Drag a run of walls along edges.
    Wall,
    /// Place free planes on any surface and stretch, turn and move them.
    Plane,
    /// Raise and lower vertices, edges and faces.
    Height,
    /// Apply the palette material to clicked faces.
    Paint,
    /// Flip a floor or ceiling's diagonal.
    Split,
    /// Place lights, spawns and actors on the grid.
    Place,
    /// Cut wall runs into horizontal bands.
    Bands,
    /// Place, move and turn static meshes (arches, columns, rubble).
    Mesh,
    /// Raise or lower a region of sectors.
    Raise,
    /// Slope a region from one side to the other.
    Ramp,
    /// Lay a flight of steps along a path.
    Stairs,
    /// Break the walls, floors and ceilings of a region into natural detail.
    Break,
    /// Give a region the rock look: render-only jitter of its faces.
    Rough,
}

/// The strip's groups, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolGroup {
    /// Choosing and moving what exists.
    Pick,
    /// Drawing by hand.
    Draw,
    /// The level verbs: region, then verb, then amount.
    Build,
}

impl ToolGroup {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Pick => "PICK",
            Self::Draw => "DRAW",
            Self::Build => "BUILD",
        }
    }
}

impl GridTool {
    /// Every tool, in strip order (grouped).
    pub(crate) const ALL: [Self; 16] = [
        Self::Select,
        Self::Height,
        Self::Floor,
        Self::Ceiling,
        Self::Wall,
        Self::Plane,
        Self::Mesh,
        Self::Paint,
        Self::Split,
        Self::Place,
        Self::Bands,
        Self::Raise,
        Self::Ramp,
        Self::Stairs,
        Self::Break,
        Self::Rough,
    ];

    /// The strip group of the tool.
    pub(crate) const fn group(self) -> ToolGroup {
        match self {
            Self::Select | Self::Height => ToolGroup::Pick,
            Self::Raise | Self::Ramp | Self::Stairs | Self::Break | Self::Rough => ToolGroup::Build,
            _ => ToolGroup::Draw,
        }
    }

    /// Whether the tool is one of the level verbs (region, then verb, then
    /// amount).
    pub(crate) const fn is_verb(self) -> bool {
        matches!(
            self,
            Self::Raise | Self::Ramp | Self::Stairs | Self::Break | Self::Rough
        )
    }

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Floor => "Floor",
            Self::Ceiling => "Ceiling",
            Self::Wall => "Wall",
            Self::Plane => "Plane",
            Self::Mesh => "Mesh",
            Self::Height => "Height",
            Self::Paint => "Paint",
            Self::Split => "Split",
            Self::Place => "Place",
            Self::Bands => "Bands",
            Self::Raise => "Raise",
            Self::Ramp => "Ramp",
            Self::Stairs => "Stairs",
            Self::Break => "Break",
            Self::Rough => "Rough",
        }
    }

    pub(crate) const fn key(self) -> egui::Key {
        match self {
            Self::Select => egui::Key::V,
            Self::Floor => egui::Key::F,
            Self::Ceiling => egui::Key::C,
            Self::Wall => egui::Key::W,
            Self::Plane | Self::Mesh => egui::Key::N,
            Self::Height => egui::Key::H,
            Self::Paint => egui::Key::P,
            Self::Split => egui::Key::D,
            Self::Place => egui::Key::O,
            Self::Bands => egui::Key::E,
            Self::Raise => egui::Key::U,
            Self::Ramp => egui::Key::A,
            Self::Stairs => egui::Key::T,
            Self::Break => egui::Key::X,
            Self::Rough => egui::Key::M,
        }
    }

    /// Whether the tool's key is pressed with Shift (the Mesh tool is Shift+N,
    /// the solid sibling of the Plane tool's N: every plain letter is taken).
    pub(crate) const fn shift(self) -> bool {
        matches!(self, Self::Mesh)
    }

    pub(crate) const fn key_label(self) -> &'static str {
        match self {
            Self::Select => "V",
            Self::Floor => "F",
            Self::Ceiling => "C",
            Self::Wall => "W",
            Self::Plane => "N",
            Self::Mesh => "Shift+N",
            Self::Height => "H",
            Self::Paint => "P",
            Self::Split => "D",
            Self::Place => "O",
            Self::Bands => "E",
            Self::Raise => "U",
            Self::Ramp => "A",
            Self::Stairs => "T",
            Self::Break => "X",
            Self::Rough => "M",
        }
    }

    pub(crate) const fn icon(self) -> char {
        match self {
            Self::Select => icons::POINTER,
            Self::Floor => icons::SQUARE,
            Self::Ceiling => icons::LAYERS,
            Self::Wall => icons::BRICK_WALL,
            Self::Plane => icons::SCALE_3D,
            Self::Mesh => icons::BOX,
            Self::Height => icons::MOVE,
            Self::Paint => icons::PALETTE,
            Self::Split => icons::SCAN,
            Self::Place => icons::MAP_PIN,
            Self::Bands => icons::ROWS_3,
            Self::Raise => icons::ARROW_UP_FROM_LINE,
            Self::Ramp => icons::TRENDING_UP,
            Self::Stairs => icons::STAIRS,
            Self::Break => icons::HAMMER,
            Self::Rough => icons::MOUNTAIN,
        }
    }

    pub(crate) const fn tooltip(self) -> &'static str {
        match self {
            Self::Select => {
                "Click a vertex, edge or face. Shift extends a range, Ctrl toggles, drag boxes, Shift-drag raises and lowers."
            }
            Self::Floor => "Drag a rectangle to create floor. Shift sets the height. Ctrl deletes.",
            Self::Ceiling => {
                "Drag a rectangle to create ceiling. Shift sets the height. Ctrl deletes."
            }
            Self::Wall => {
                "Drag along an edge to place a run of walls. R turns the direction, Ctrl removes."
            }
            Self::Plane => {
                "Click a surface to put a free plane on it; drag to size it. Select a plane to move, turn and stretch it with its handles."
            }
            Self::Mesh => {
                "Pick a mesh in the Meshes palette (right dock), click a floor to place it. Drag to move, R turns a quarter, [ and ] 15 degrees, Delete removes. Arches carry a perch at the crown."
            }
            Self::Height => {
                "Drag a vertex, edge or face up and down. Alt snaps finer, Alt+drag a face moves one triangle. L unlinks."
            }
            Self::Paint => "Click faces to apply the material chosen in the palette. Ctrl samples.",
            Self::Split => {
                "Click a floor or ceiling to flip its diagonal. Shift+click voids or restores a triangle."
            }
            Self::Place => "Click to place the chosen node at a sector centre on the floor.",
            Self::Bands => {
                "Hover a wall, click to cut it into bands at the rule. Drag a line to move it, Delete removes it, Shift+click picks a band for the palette."
            }
            Self::Raise => {
                "Pick sectors (marquee, brush or path), then press on them and drag up or down. Tab switches Offset and Level, Alt snaps finer, Esc cancels."
            }
            Self::Ramp => {
                "Pick sectors, then press near the side that keeps its height and drag up or down: the opposite side rises. R turns the side, Tab holds the slope to what the character can walk."
            }
            Self::Stairs => {
                "Click the waypoints of a flight, Enter finishes, then press on it and drag up or down for the total rise. Each sector is a tread, a turn is a landing. Tab switches steps and ramp."
            }
            Self::Break => {
                "Pick sectors, then press on them and drag up for the intensity: lips crumble, edges bite, tiles crack. Tab changes the target, Shift+X re-rolls the seed, Enter commits, Esc cancels."
            }
            Self::Rough => {
                "Pick sectors, then press on them and drag up for the rock level (0 to 3). The faces jitter in the render only; collision stays on the grid."
            }
        }
    }
}

/// Direction of the next wall: the edge nearest the pointer, or a fixed one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WallDirection {
    /// The cardinal edge of the sector nearest the pointer.
    Auto,
    /// Always this edge, diagonals included.
    Edge(WallEdge),
}

impl WallDirection {
    pub(crate) fn next(self) -> Self {
        match self {
            Self::Auto => Self::Edge(WallEdge::North),
            Self::Edge(WallEdge::North) => Self::Edge(WallEdge::East),
            Self::Edge(WallEdge::East) => Self::Edge(WallEdge::South),
            Self::Edge(WallEdge::South) => Self::Edge(WallEdge::West),
            Self::Edge(WallEdge::West) => Self::Edge(WallEdge::Nwse),
            Self::Edge(WallEdge::Nwse) => Self::Edge(WallEdge::Nesw),
            Self::Edge(WallEdge::Nesw) => Self::Auto,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Edge(edge) => edge.label(),
        }
    }
}

/// A floor or ceiling rectangle being dragged.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RectGesture {
    pub kind: SurfaceKind,
    pub room: usize,
    pub anchor: [i32; 2],
    pub current: [i32; 2],
    /// Room-relative height of the surface being made, grid units.
    pub height: i32,
    /// Pointer height and surface height at the moment Shift was pressed.
    pub adjust: Option<(f32, i32)>,
    pub erase: bool,
}

/// A run of walls being dragged.
#[derive(Clone, Copy, Debug)]
pub(crate) struct WallGesture {
    pub room: usize,
    pub edge: WallEdge,
    pub anchor: [i32; 2],
    pub current: [i32; 2],
    /// Room-relative height that picked the gap, `None` in the Top view.
    pub pick_y: Option<i32>,
    pub erase: bool,
    /// Camera position `[x, z]` in world grid units when started in 3D, so
    /// each wall faces the side it was placed from.
    pub viewer: Option<[i64; 2]>,
}

/// A height drag in progress.
#[derive(Clone, Debug)]
pub(crate) struct HeightGesture {
    pub drag: HeightDrag,
    pub press_y: f32,
    pub pixels_per_unit: f32,
    pub delta: i32,
    /// The document before the drag, recorded for undo at the first change.
    pub before: Option<Box<ProjectDocument>>,
    /// Set when the drag moves one triangle of a floor or ceiling.
    pub triangle: Option<triangle_tool::TriangleGrab>,
}

/// The one gesture in flight, if any.
#[derive(Clone, Debug, Default)]
pub(crate) enum Gesture {
    #[default]
    Idle,
    Rect(RectGesture),
    Walls(WallGesture),
    Height(HeightGesture),
    /// A marquee over the view, from `start` to `current`.
    Marquee {
        start: Pos2,
        current: Pos2,
        additive: bool,
    },
    /// Painting faces by dragging; one undo step for the stroke.
    Paint {
        undo_pushed: bool,
    },
    /// A node (light, spawn, actor) stroke handed to the node tools.
    Node,
    /// A plane being placed or one of its handles being dragged.
    Plane(plane_tool::PlaneGesture),
    /// A mesh being dragged.
    Mesh(mesh_tool::MeshDrag),
    /// A band line being dragged.
    Band(bands_tool::BandDrag),
    /// A level verb's region being picked.
    Region(verb_state::RegionGesture),
    /// A level verb's amount being dragged.
    Verb(Box<verb_state::VerbGesture>),
}

/// What the pointer is over, for hover highlights and ghosts.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) enum GridHover {
    #[default]
    None,
    /// A sector, with the room-relative height of the plane it was found on.
    Cell {
        room: usize,
        cell: [i32; 2],
        height: i32,
    },
    /// A wall about to be placed, with its heights or `None` when covered.
    Wall {
        room: usize,
        cell: [i32; 2],
        edge: WallEdge,
        heights: Option<[i32; 4]>,
    },
    /// A selectable element.
    Elements(Vec<psxed_project::grid::edit::GridElement>),
    /// A face and triangle for painting and splitting.
    Face {
        face: psxed_project::grid::edit::FaceRef,
        tri: Option<u8>,
    },
    /// A sector centre and height where a node would land.
    Spot { position: [f32; 3] },
    /// A wall under the Bands tool: where a click would cut it and the line
    /// the pointer is on, if any.
    Band {
        face: psxed_project::grid::edit::FaceRef,
        anchor: Option<psxed_project::grid::Anchor>,
        y: i32,
        line: Option<usize>,
    },
    /// A free plane under the pointer.
    Plane(PlaneId),
    /// A placed mesh under the pointer, by stable id.
    Mesh(u16),
    /// Where the Mesh tool would put a new instance: its room and position
    /// (grid units, `y` relative to the room).
    MeshSpot {
        room: usize,
        position: [i32; 3],
    },
    /// A handle of the selected plane under the pointer.
    Grip(PlaneId, plane_math::Handle),
    /// Where the Plane tool would put a new plane, in room `0`.
    PlaneSpot(usize, Box<GridPlane>),
}

/// A clipboard ghost following the pointer.
#[derive(Clone, Debug)]
pub(crate) struct PasteGhost {
    pub clip: GridClipboard,
    pub at: Option<[i32; 2]>,
}

/// A plane clipboard ghost following the pointer.
#[derive(Clone, Debug)]
pub(crate) struct PlanePaste {
    pub clip: PlaneClipboard,
    /// World `[x, z]` grid units the group's anchor hangs at.
    pub at: Option<[i32; 2]>,
}

/// What the Plane tool gives a new plane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NewPlane {
    pub collision: PlaneCollision,
    pub double_sided: bool,
}

/// All grid editing state of the workspace. Not saved with the project.
pub(crate) struct GridEditState {
    pub tool: GridTool,
    pub selection: GridSelection,
    pub hover: GridHover,
    pub gesture: Gesture,
    /// Material the Paint tool and new surfaces use.
    pub material: Option<ResourceId>,
    /// Room-relative height of new floors, grid units.
    pub floor_height: i32,
    /// Room-relative height of new ceilings, grid units.
    pub ceiling_height: i32,
    /// Whether coincident corners move together.
    pub link: bool,
    pub wall_dir: WallDirection,
    pub clipboard: Option<GridClipboard>,
    pub paste: Option<PasteGhost>,
    /// The selected free planes, the last one primary. A plane selection and
    /// a face selection never exist together.
    pub plane_sel: Vec<PlaneId>,
    pub plane_clip: Option<PlaneClipboard>,
    pub plane_paste: Option<PlanePaste>,
    /// Collision and sidedness of the next placed plane.
    pub new_plane: NewPlane,
    /// The selected mesh instance, by stable id.
    pub mesh_sel: Option<u16>,
    /// The asset the Mesh tool places (index into the world's assets).
    pub mesh_asset: Option<u16>,
    /// Heading, degrees, of the next placed mesh.
    pub mesh_turn: i32,
    /// Path typed in the Meshes palette for Import OBJ.
    pub mesh_import_path: String,
    /// The last press was taken by a plane, so its click is too.
    pub plane_press: bool,
    /// Which thing a repeated Top view click takes next.
    pub cycle: cycle::CycleState,
    /// How far a band edit reaches: this run or the whole room.
    pub band_scope: psxed_project::grid::edit::bands::BandScope,
    /// The band line the Bands tool has selected: the wall it was picked on
    /// and its index in the wall's set.
    pub band_line: Option<(psxed_project::grid::edit::FaceRef, usize)>,
    /// The band picked with Shift+click, for the palette to paint.
    pub band_sel: Option<(psxed_project::grid::edit::FaceRef, usize)>,
    /// The level verbs: region, options and the last readout.
    pub verb: verb_state::VerbState,
    /// Which overlays are on (Alt+W, Alt+P).
    pub overlays: analysis::Overlays,
    /// The overlays' analyses, per document revision.
    pub analysis: RefCell<analysis::AnalysisCache>,
    /// The room tools create in when none is marked active.
    pub last_room: usize,
    /// Bumped by every document change; the pick cache and budget follow it.
    pub rev: u64,
    pub cache: RefCell<Option<PickCache>>,
    /// Palette entry to scroll into view next frame.
    pub palette_scroll_to: Option<ResourceId>,
    /// Material counts for the status bar, per revision.
    pub budget: RefCell<Option<(u64, psxed_project::grid::GridBudget)>>,
    /// The Top view still has to fit the world to the real pane size.
    pub fit_pending: bool,
    /// Where the 3D view was drawn last, for the in-view face count.
    pub view_rect: std::cell::Cell<Option<Rect>>,
    /// A scripted hover (see `pose`) stays until a real pointer replaces it.
    pub hover_pinned: bool,
    /// The key and gesture help overlay (`?`) is open.
    pub help_open: bool,
    /// The palette lists every project material, not only world surfaces.
    pub palette_all: bool,
    /// The room whose name is being edited in the Rooms panel.
    pub renaming_room: Option<usize>,
}

impl Default for GridEditState {
    fn default() -> Self {
        Self {
            tool: GridTool::Select,
            selection: GridSelection::new(),
            hover: GridHover::None,
            gesture: Gesture::Idle,
            material: None,
            floor_height: 0,
            ceiling_height: 3 * psxed_project::grid::SECTOR_UNITS,
            link: true,
            wall_dir: WallDirection::Auto,
            clipboard: None,
            paste: None,
            plane_sel: Vec::new(),
            plane_clip: None,
            plane_paste: None,
            new_plane: NewPlane {
                collision: PlaneCollision::Decor,
                double_sided: false,
            },
            plane_press: false,
            mesh_sel: None,
            mesh_asset: None,
            mesh_turn: 0,
            mesh_import_path: String::new(),
            cycle: cycle::CycleState::default(),
            band_scope: psxed_project::grid::edit::bands::BandScope::Run,
            band_line: None,
            band_sel: None,
            verb: verb_state::VerbState::default(),
            overlays: analysis::Overlays::default(),
            analysis: RefCell::new(analysis::AnalysisCache::default()),
            last_room: 0,
            rev: 1,
            cache: RefCell::new(None),
            palette_scroll_to: None,
            budget: RefCell::new(None),
            fit_pending: true,
            view_rect: std::cell::Cell::new(None),
            hover_pinned: false,
            help_open: false,
            palette_all: false,
            renaming_room: None,
        }
    }
}

impl GridEditState {
    /// Forget any in-flight gesture and ghost (a document swap, a tool change).
    pub(crate) fn abandon(&mut self) {
        self.gesture = Gesture::Idle;
        self.paste = None;
        self.plane_paste = None;
        self.plane_press = false;
        self.cycle.reset();
        self.band_line = None;
        self.band_sel = None;
        self.verb.path.clear();
        self.verb.lasso.clear();
        self.verb.lasso_cursor = None;
        self.verb.preview.clear();
        self.hover_pinned = false;
        self.hover = GridHover::None;
    }
}

impl EditorWorkspace {
    /// Whether the active scene is a grid world.
    pub(crate) fn is_grid_world(&self) -> bool {
        self.project.active_scene().world.grid().is_some()
    }

    /// The room tools create in.
    pub(crate) fn grid_target_room(&self) -> usize {
        let count = self
            .project
            .active_scene()
            .world
            .grid()
            .map_or(0, |w| w.rooms.len());
        let room = self.grid_active_room.unwrap_or(self.grid.last_room);
        room.min(count.saturating_sub(1))
    }

    /// Switch the grid tool, abandoning any ghost, and keep the shared tool
    /// state the node tools read in step.
    pub(crate) fn set_grid_tool(&mut self, tool: GridTool) {
        if self.grid.tool != tool {
            self.grid.abandon();
        }
        self.grid.tool = tool;
        self.active_tool = match tool {
            GridTool::Select => ViewTool::Select,
            GridTool::Place => ViewTool::Place,
            _ => ViewTool::Select,
        };
        verb_tool::on_tool_changed(self);
        self.status = format!("Tool: {} ({})", tool.label(), tool.key_label());
        self.mark_shortcut_group_changed(ShortcutGroup::Tool);
    }

    /// A grid edit just happened: stale caches, dirty document.
    pub(crate) fn grid_changed(&mut self) {
        self.mark_dirty();
    }
}

impl EditorWorkspace {
    /// Centre and half extents, editor units, of the selected grid elements
    /// (what `.` frames). `None` without a grid selection.
    pub(crate) fn grid_selection_bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        let world = self.project.active_scene().world.grid()?;
        let corners = self.grid.selection.corners(world);
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        let mut any = false;
        for corner in corners {
            let Some(g) = corner.grid_pos(world) else {
                continue;
            };
            let p = [
                world.scale.to_editor(g[0]) as f32,
                world.scale.to_editor(g[1]) as f32,
                world.scale.to_editor(g[2]) as f32,
            ];
            for axis in 0..3 {
                min[axis] = min[axis].min(p[axis]);
                max[axis] = max[axis].max(p[axis]);
            }
            any = true;
        }
        any.then(|| {
            (
                core::array::from_fn(|i| (min[i] + max[i]) * 0.5),
                core::array::from_fn(|i| ((max[i] - min[i]) * 0.5).max(64.0)),
            )
        })
    }
}

impl EditorWorkspace {
    /// The mouse cursor the active tool wants over the viewport.
    pub(crate) fn grid_cursor(&self) -> egui::CursorIcon {
        use egui::CursorIcon as C;
        if self.grid.paste.is_some() || self.grid.plane_paste.is_some() {
            return C::Move;
        }
        if matches!(self.grid.hover, GridHover::Grip(..)) {
            return C::Grab;
        }
        match &self.grid.gesture {
            Gesture::Height(_) => return C::ResizeVertical,
            Gesture::Rect(_) | Gesture::Walls(_) | Gesture::Marquee { .. } => return C::Crosshair,
            Gesture::Plane(_) | Gesture::Mesh(_) => return C::Grabbing,
            Gesture::Paint { .. } => return C::Crosshair,
            Gesture::Band(_) => return C::ResizeVertical,
            Gesture::Verb(_) => return C::ResizeVertical,
            Gesture::Region(_) => return C::Crosshair,
            _ => {}
        }
        let over = !matches!(self.grid.hover, GridHover::None);
        match self.grid.tool {
            GridTool::Select if over => C::PointingHand,
            GridTool::Select => C::Default,
            GridTool::Height if over => C::ResizeVertical,
            GridTool::Height => C::Default,
            GridTool::Split if over => C::PointingHand,
            GridTool::Split => C::Default,
            GridTool::Bands if over => C::PointingHand,
            GridTool::Bands => C::Default,
            GridTool::Raise
            | GridTool::Ramp
            | GridTool::Stairs
            | GridTool::Break
            | GridTool::Rough
                if self.grid.verb.region.is_some() =>
            {
                C::ResizeVertical
            }
            GridTool::Floor
            | GridTool::Raise
            | GridTool::Ramp
            | GridTool::Stairs
            | GridTool::Break
            | GridTool::Rough
            | GridTool::Ceiling
            | GridTool::Wall
            | GridTool::Plane
            | GridTool::Mesh
            | GridTool::Paint
            | GridTool::Place => C::Crosshair,
        }
    }

    /// The material a new surface or wall takes: the palette choice, else
    /// the first one already used in the room, so new geometry matches its
    /// surroundings instead of showing the untextured fallback.
    pub(crate) fn grid_new_material(&self, room: usize, wall: bool) -> Option<ResourceId> {
        if self.grid.material.is_some() {
            return self.grid.material;
        }
        let world = self.project.active_scene().world.grid()?;
        let r = world.rooms.get(room)?;
        for (_, _, sector) in r.iter() {
            let from_walls = || {
                WallEdge::ALL
                    .iter()
                    .find_map(|e| sector.walls(*e).iter().find_map(|w| w.material))
            };
            let from_floor = || sector.floor.as_ref().and_then(|f| f.tri_a.material);
            let found = if wall {
                from_walls().or_else(from_floor)
            } else {
                from_floor().or_else(from_walls)
            };
            if found.is_some() {
                return found;
            }
        }
        None
    }
}
