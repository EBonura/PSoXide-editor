//! What the 3D preview must honour that is not part of the project.
//!
//! The preview is rendered by the frontend from the project alone, while the
//! hidden rooms and the storey cutaway are view settings of the editor that
//! are never saved with it. The workspace publishes them once a frame as one
//! typed [`GridPreviewView`]; the preview reads the snapshot once at its edge
//! and hands it down as an ordinary argument. (The frontend's render entry
//! points are the pinned emulator's, so the snapshot cannot ride on their
//! argument list; the editor draws and previews on one thread.)

use std::cell::RefCell;

use super::*;

/// The editor's view settings the 3D preview draws by.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GridPreviewView {
    /// Rooms the author hid in the Rooms panel, ascending.
    pub hidden_rooms: Vec<usize>,
    /// The storey cutaway: editor-unit height from which faces whose lowest
    /// point is at or above it are not drawn. `None` draws everything.
    pub cutaway: Option<i32>,
    /// The paint the Paint tool is about to make under the pointer: the 3D
    /// preview draws the faces it lands on as they will look. Never part of
    /// the document.
    pub paint: Option<PaintGhost>,
}

impl GridPreviewView {
    /// Whether room `index` is hidden.
    pub fn hides_room(&self, index: usize) -> bool {
        self.hidden_rooms.contains(&index)
    }

    /// Whether something whose lowest point is at editor height `lowest` is
    /// cut away.
    pub fn cuts(&self, lowest: f64) -> bool {
        self.cutaway.is_some_and(|cut| lowest >= f64::from(cut))
    }
}

thread_local! {
    static PUBLISHED: RefCell<GridPreviewView> = RefCell::new(GridPreviewView::default());
}

/// The view the workspace published this frame.
pub fn grid_preview_view() -> GridPreviewView {
    PUBLISHED.with(|view| view.borrow().clone())
}

impl EditorWorkspace {
    /// Publish this frame's view settings to the 3D preview. A scene that is
    /// not a grid world publishes none.
    pub(crate) fn publish_grid_view(&self) {
        let view = if self.is_grid_world() {
            let mut hidden_rooms: Vec<usize> = self.grid_hidden_rooms.iter().copied().collect();
            hidden_rooms.sort_unstable();
            GridPreviewView {
                hidden_rooms,
                cutaway: self.cutaway_plane(),
                paint: self.paint_ghost(),
            }
        } else {
            GridPreviewView::default()
        };
        PUBLISHED.with(|slot| {
            let mut slot = slot.borrow_mut();
            if *slot != view {
                *slot = view;
            }
        });
    }
}
