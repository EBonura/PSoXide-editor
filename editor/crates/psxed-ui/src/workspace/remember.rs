//! Restoring and saving what the editor remembers (see `session_state`).

use super::*;
use crate::session_state::{self, ProjectSession, UserLayout};

/// The drawer views, in the order the layout file counts them.
const DRAWER_VIEWS: [ContentBrowserView; 3] = [
    ContentBrowserView::Resources,
    ContentBrowserView::Debug,
    ContentBrowserView::Library,
];

fn view_index(view: ContentBrowserView) -> usize {
    DRAWER_VIEWS.iter().position(|v| *v == view).unwrap_or(0)
}

/// The sizes the panels measured on their last frame.
#[derive(Default)]
pub(crate) struct MeasuredSizes {
    pub left_width: Option<f32>,
    pub inspector_width: Option<f32>,
    /// Drawer heights per view (Assets, Console, Library).
    pub drawer_heights: [Option<f32>; 3],
}

impl EditorWorkspace {
    /// The layout as it is now.
    pub(crate) fn current_user_layout(&self) -> UserLayout {
        UserLayout {
            left_open: self.left_dock_open,
            inspector_open: self.inspector_open,
            drawer_open: self.resources_open,
            drawer_view: view_index(self.content_browser_view) as u8,
            library_tab: crate::grid_tools::LibraryTab::ALL
                .iter()
                .position(|t| *t == self.grid.library_tab)
                .unwrap_or(0) as u8,
            left_width: self.measured.left_width,
            inspector_width: self.measured.inspector_width,
            drawer_heights: self.measured.drawer_heights,
            structure_fraction: self.grid.structure_fraction,
        }
    }

    /// Put the remembered layout on screen.
    pub(crate) fn apply_user_layout(&mut self, layout: &UserLayout) {
        self.left_dock_open = layout.left_open;
        self.inspector_open = layout.inspector_open;
        self.measured.left_width = layout.left_width;
        self.measured.inspector_width = layout.inspector_width;
        self.measured.drawer_heights = layout.drawer_heights;
        self.grid.structure_fraction = layout.structure_fraction;
        if self.is_grid_world() {
            self.resources_open = layout.drawer_open;
            self.content_browser_view = DRAWER_VIEWS[usize::from(layout.drawer_view).min(2)];
            if let Some(tab) =
                crate::grid_tools::LibraryTab::ALL.get(usize::from(layout.library_tab))
            {
                self.grid.library_tab = *tab;
            }
        }
    }

    /// What the project remembers: the tool and the storey.
    pub(crate) fn current_session(&self) -> ProjectSession {
        ProjectSession {
            tool: Some(self.grid.tool.label().to_string()),
            storey: self.grid.level.y,
        }
    }

    /// Return to the tool and the storey the project was left on.
    pub(crate) fn apply_session(&mut self, session: &ProjectSession) {
        if !self.is_grid_world() {
            return;
        }
        let status = self.status.clone();
        if let Some(tool) = session.tool.as_deref().and_then(|name| {
            crate::grid_tools::GridTool::ALL
                .iter()
                .find(|t| t.label() == name)
        }) {
            self.set_grid_tool(*tool);
        }
        if let Some(y) = session.storey {
            if self.level_storeys().iter().any(|s| s.height == y) {
                self.set_level_quiet(y);
            }
        }
        self.status = status;
    }

    /// On open: read the user's layout and the project's session.
    pub(crate) fn restore_user_state(&mut self) {
        self.restore_user_state_from(session_state::user_layout_path());
    }

    /// [`Self::restore_user_state`] with the layout file named (tests name a
    /// scratch one).
    pub(crate) fn restore_user_state_from(&mut self, layout_path: Option<PathBuf>) {
        self.user_layout_path = layout_path;
        if let Some(layout) = self
            .user_layout_path
            .as_deref()
            .and_then(session_state::read_layout)
        {
            self.apply_user_layout(&layout);
        }
        if let Some(session) = ProjectSession::read(&self.project_dir) {
            self.apply_session(&session);
        }
        self.layout_saved = self.current_user_layout();
        self.session_saved = self.current_session();
    }

    /// Write what changed, once the pointer is up (so a dock being dragged
    /// is written when it lands, not on every frame).
    pub(crate) fn persist_user_state(&mut self, pointer_down: bool) {
        if pointer_down {
            return;
        }
        let layout = self.current_user_layout();
        if layout != self.layout_saved {
            if let Some(path) = &self.user_layout_path {
                // A folder that cannot be written is not worth a message on
                // every frame: try again only when the layout changes again.
                let _ = session_state::write_layout(path, &layout);
            }
            self.layout_saved = layout;
        }
        if self.is_grid_world() {
            let session = self.current_session();
            if session != self.session_saved {
                let _ = session.write(&self.project_dir);
                self.session_saved = session;
            }
        }
    }
}
