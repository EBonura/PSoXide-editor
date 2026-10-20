//! What the preview remembers from one frame to the next.
//!
//! * the document generation: a snapshot of the project the last frame was
//!   built from, compared with the live one (a deep compare of an unchanged
//!   document costs microseconds, and unlike a revision counter it cannot be
//!   forgotten by one of the many places that edit the document);
//! * the kept grid geometry ([`super::grid_cache`]);
//! * a memo of the last frame: when nothing it was built from has changed and
//!   nothing in it moves with the clock, the frame is handed back as it was.

use super::*;

use super::grid_cache::GridGeometry;

/// Editor-space bounds of the grid world.
type GridBounds = Option<([f64; 3], [f64; 3])>;

/// Everything besides the document that a frame is built from.
#[derive(PartialEq)]
pub(super) struct FrameKey {
    document: u64,
    camera: ViewportCameraState,
    preview_bounds: bool,
    show_lights: bool,
    hidden_scene_nodes: HashSet<NodeId>,
    selected: NodeId,
    character_motion: Option<psxed_ui::EditorCharacterMotionPreview>,
    selected_bounds: Option<([f32; 3], [f32; 3])>,
    entity_bounds: Vec<psxed_ui::EntityBounds>,
    hovered_entity_node: Option<NodeId>,
    textures_generation: u64,
    assets_generation: u64,
    /// Moves when a global light solve lands.
    global_light_generation: u64,
    pub(super) grid_view: psxed_ui::GridPreviewView,
}

struct FrameMemo {
    key: FrameKey,
    /// The `serial` stamped on the frame the memo describes.
    serial: u64,
    /// The frame read the animation clock, so it is stale a vblank later.
    clock_dependent: bool,
    dropped: u32,
}

#[derive(Default)]
pub(super) struct PreviewState {
    snapshot: Option<ProjectDocument>,
    generation: u64,
    pub(super) geometry: GridGeometry,
    /// `(document generation, bounds)` of the grid world, for the depth range.
    grid_bounds: Option<(u64, GridBounds)>,
    memo: Option<FrameMemo>,
    next_serial: u64,
}

static STATE: OnceLock<Mutex<PreviewState>> = OnceLock::new();

pub(super) fn preview_state() -> &'static Mutex<PreviewState> {
    STATE.get_or_init(|| Mutex::new(PreviewState::default()))
}

/// A number that changes whenever `project` differs from the one this was
/// last asked about. Callers that cache something derived from the document
/// keep the number with it.
pub(crate) fn document_generation(project: &ProjectDocument) -> u64 {
    preview_state()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .observe(project)
}

impl PreviewState {
    pub(super) fn observe(&mut self, project: &ProjectDocument) -> u64 {
        if self.snapshot.as_ref() != Some(project) {
            self.generation = self.generation.wrapping_add(1);
            self.snapshot = Some(project.clone());
        }
        self.generation
    }

    /// Bounds of the active grid world, kept for the document generation.
    pub(super) fn grid_bounds(&mut self, project: &ProjectDocument) -> GridBounds {
        match self.grid_bounds {
            Some((generation, bounds)) if generation == self.generation => bounds,
            _ => {
                let bounds = grid_world_bounds(project);
                self.grid_bounds = Some((self.generation, bounds));
                bounds
            }
        }
    }

    /// Whether the frame in `output` is still what building would produce.
    pub(super) fn memo_hit(&self, key: &FrameKey, output: &EditorPreviewFrame) -> bool {
        self.memo.as_ref().is_some_and(|memo| {
            !memo.clock_dependent && memo.serial == output.serial && memo.key == *key
        })
    }

    pub(super) fn memo_dropped(&self) -> u32 {
        self.memo.as_ref().map_or(0, |memo| memo.dropped)
    }

    pub(super) fn remember(
        &mut self,
        key: FrameKey,
        clock_dependent: bool,
        dropped: u32,
        output: &mut EditorPreviewFrame,
    ) {
        self.next_serial = self.next_serial.wrapping_add(1).max(1);
        output.serial = self.next_serial;
        self.memo = Some(FrameMemo {
            key,
            serial: self.next_serial,
            clock_dependent,
            dropped,
        });
    }
}

impl FrameKey {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        document: u64,
        global_light_generation: u64,
        camera: ViewportCameraState,
        preview_bounds: bool,
        show_lights: bool,
        hidden_scene_nodes: &HashSet<NodeId>,
        selected: NodeId,
        character_motion: Option<psxed_ui::EditorCharacterMotionPreview>,
        selected_bounds: Option<([f32; 3], [f32; 3])>,
        entity_bounds: &[psxed_ui::EntityBounds],
        hovered_entity_node: Option<NodeId>,
        textures: &EditorTextures,
        assets: &crate::editor_assets::EditorAssets,
    ) -> Self {
        Self {
            document,
            camera,
            preview_bounds,
            show_lights,
            hidden_scene_nodes: hidden_scene_nodes.clone(),
            selected,
            character_motion,
            selected_bounds,
            entity_bounds: entity_bounds.to_vec(),
            hovered_entity_node,
            textures_generation: textures.generation(),
            assets_generation: assets.generation(),
            global_light_generation,
            grid_view: psxed_ui::grid_preview_view(),
        }
    }
}

/// Forget everything (tests that compare an incremental build with a cold one).
#[cfg(test)]
pub(super) fn reset_for_tests() {
    *preview_state()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = PreviewState::default();
}
