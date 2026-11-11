//! Autosave and crash recovery.
//!
//! While a project has unsaved edits the editor writes a copy of it to a
//! sidecar file inside the project folder, never over `project.ron`: a moment
//! after the last change (so a drag is never interrupted), and at the latest
//! when the edits have waited unwritten for a while. A normal save, a reload
//! and a discard remove the sidecar. When a project opens and a sidecar newer
//! than `project.ron` is there, the editor asks whether to restore it.

use std::time::{Duration, Instant};

use super::*;

/// The sidecar, inside the project folder, beside `project.ron`.
pub(crate) const AUTOSAVE_FILE: &str = ".psxed-autosave.ron";
/// How long the document must sit unchanged before it is written.
pub(crate) const QUIET_BEFORE_WRITE: Duration = Duration::from_secs(2);
/// The longest edits wait unwritten while the editor keeps being changed.
pub(crate) const LONGEST_UNWRITTEN: Duration = Duration::from_secs(30);

/// What the autosave remembers between frames.
#[derive(Default)]
pub(crate) struct AutosaveState {
    /// The document revision last seen.
    seen_rev: u64,
    /// Whether the project was unsaved at the last tick.
    was_dirty: bool,
    /// When the document last changed.
    changed_at: Option<Instant>,
    /// When the oldest change not yet written was made.
    pending_since: Option<Instant>,
    /// The revision the sidecar holds.
    written_rev: Option<u64>,
    /// How many times the sidecar has been written (for tests and logs).
    pub(crate) writes: u32,
    /// The last write failed and was reported; do not repeat it every tick.
    failed: bool,
}

impl AutosaveState {
    /// Whether edits are waiting for the clock (the window then asks for a
    /// repaint a second on, so an idle editor still writes them).
    pub(crate) fn needs_wakeup(&self) -> bool {
        self.pending_since.is_some()
    }
}

impl EditorWorkspace {
    /// Where this project's autosave lives.
    pub(crate) fn autosave_path(&self) -> PathBuf {
        self.project_dir.join(AUTOSAVE_FILE)
    }

    /// One look at the clock: write the autosave when the edits have gone
    /// quiet, or have waited long enough. `pointer_down` holds it back while
    /// a drag or a stroke is in progress.
    pub(crate) fn autosave_tick(&mut self, now: Instant, pointer_down: bool) {
        if !self.dirty {
            self.autosave.was_dirty = false;
            self.autosave.changed_at = None;
            self.autosave.pending_since = None;
            return;
        }
        // Waiting on the recovery answer: the sidecar holds work that is not
        // in the editor yet.
        if matches!(self.modal, Modal::RecoverAutosave { .. }) {
            return;
        }
        let state = &mut self.autosave;
        if !state.was_dirty || state.seen_rev != self.grid.rev {
            state.was_dirty = true;
            state.seen_rev = self.grid.rev;
            state.changed_at = Some(now);
            state.pending_since.get_or_insert(now);
        }
        if pointer_down || state.written_rev == Some(self.grid.rev) {
            return;
        }
        let quiet = state
            .changed_at
            .is_some_and(|at| now.saturating_duration_since(at) >= QUIET_BEFORE_WRITE);
        let overdue = state
            .pending_since
            .is_some_and(|at| now.saturating_duration_since(at) >= LONGEST_UNWRITTEN);
        if !(quiet || overdue) {
            return;
        }
        match self.write_autosave() {
            Ok(()) => {
                let rev = self.grid.rev;
                let state = &mut self.autosave;
                state.written_rev = Some(rev);
                state.pending_since = None;
                state.writes += 1;
                state.failed = false;
            }
            Err(error) => {
                if !self.autosave.failed {
                    self.status = format!("Autosave failed: {error}");
                }
                self.autosave.failed = true;
                // Try again after the next quiet moment, not every frame.
                self.autosave.changed_at = Some(now);
                self.autosave.pending_since = Some(now);
            }
        }
    }

    /// Write the sidecar (through a temporary file, so a crash mid-write
    /// leaves the last good one).
    pub(crate) fn write_autosave(&self) -> Result<(), String> {
        let mut project = self.project.clone();
        project.normalize_loaded();
        let text = project.to_ron_string().map_err(|e| e.to_string())?;
        let path = self.autosave_path();
        let tmp = path.with_extension("ron.tmp");
        std::fs::write(&tmp, text).map_err(|e| format!("write {}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, &path).map_err(|e| format!("rename {}: {e}", path.display()))
    }

    /// Remove the sidecar (a save, a reload or a discard made it stale).
    pub(crate) fn discard_autosave(&mut self) {
        let _ = std::fs::remove_file(self.autosave_path());
        self.autosave.written_rev = None;
        self.autosave.pending_since = None;
    }

    /// On open: when a sidecar newer than `project.ron` holds something the
    /// saved project does not, ask whether to restore it.
    pub(crate) fn check_autosave_on_open(&mut self) {
        let path = self.autosave_path();
        let Ok(side_meta) = std::fs::metadata(&path) else {
            return;
        };
        let saved_at = std::fs::metadata(self.project_dir.join("project.ron"))
            .and_then(|m| m.modified())
            .ok();
        let side_at = side_meta.modified().ok();
        // Older than the project file: the project was saved after it.
        if let (Some(saved), Some(side)) = (saved_at, side_at) {
            if side <= saved {
                let _ = std::fs::remove_file(&path);
                return;
            }
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            return;
        };
        let Ok(mut recovered) = ProjectDocument::from_ron_str(&text) else {
            // Unreadable: keep it for a person to look at, out of the way.
            let _ = std::fs::rename(&path, path.with_extension("bad.ron"));
            self.status = "An autosave was unreadable and was set aside".to_string();
            return;
        };
        recovered.normalize_loaded();
        let mut saved = self.project.clone();
        saved.normalize_loaded();
        if recovered == saved {
            let _ = std::fs::remove_file(&path);
            return;
        }
        let age = side_at
            .and_then(|at| at.elapsed().ok())
            .map_or_else(|| "earlier".to_string(), age_text);
        self.modal = Modal::RecoverAutosave { age, error: None };
    }

    /// Replace the open project with the autosave. The project is then
    /// unsaved (the autosave stays until it is saved or discarded).
    pub(crate) fn restore_autosave(&mut self) -> Result<(), String> {
        let text = std::fs::read_to_string(self.autosave_path())
            .map_err(|e| format!("read the autosave: {e}"))?;
        let mut recovered = ProjectDocument::from_ron_str(&text).map_err(|e| e.to_string())?;
        recovered.normalize_loaded();
        self.interaction = Interaction::Idle;
        self.node_drag_2d = None;
        self.inspector_undo_transaction = None;
        self.history.clear();
        self.renaming = None;
        self.project = recovered;
        self.grid.abandon();
        self.grid.selection.clear();
        self.grid.plane_sel.clear();
        self.grid.rev = self.grid.rev.wrapping_add(1);
        self.grid.fit_pending = true;
        self.selection.selected_node = NodeId::ROOT;
        self.selection.selected_nodes.clear();
        self.selection.selected_resource = None;
        self.selection.selected_resources.clear();
        self.reconcile_selection_after_document_change();
        self.dirty = true;
        self.autosave.written_rev = None;
        self.status = "Restored the autosaved work. Save to keep it.".to_string();
        Ok(())
    }

    /// The question asked when a project opens with a newer autosave.
    pub(crate) fn draw_recover_autosave_dialog(&mut self, ctx: &egui::Context) {
        let (age, error_text) = match &self.modal {
            Modal::RecoverAutosave { age, error } => (age.clone(), error.clone()),
            _ => return,
        };
        let name = self.project.name.clone();
        #[derive(PartialEq)]
        enum Choice {
            Restore,
            Keep,
        }
        let mut choice = None;
        egui::Window::new("Recover unsaved work")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_min_width(420.0);
                ui.label(
                    RichText::new(format!(
                        "\"{name}\" has changes that were never saved, kept {age}."
                    ))
                    .strong(),
                );
                ui.label(
                    RichText::new(
                        "The editor keeps a copy while you work. Restore puts those changes back (nothing is saved until you save). Keep the saved version throws the copy away.",
                    )
                    .color(STUDIO_TEXT_WEAK),
                );
                if let Some(error) = &error_text {
                    ui.label(RichText::new(error).color(Color32::from_rgb(0xE0, 0x60, 0x60)));
                }
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Keep the saved version").clicked() {
                        choice = Some(Choice::Keep);
                    }
                    if ui.button("Restore the unsaved work").clicked() {
                        choice = Some(Choice::Restore);
                    }
                });
            });
        match choice {
            Some(Choice::Keep) => {
                self.discard_autosave();
                self.modal = Modal::None;
                self.status = "Kept the saved version".to_string();
            }
            Some(Choice::Restore) => match self.restore_autosave() {
                Ok(()) => self.modal = Modal::None,
                Err(error) => {
                    self.modal = Modal::RecoverAutosave {
                        age,
                        error: Some(error),
                    }
                }
            },
            None => {}
        }
    }
}

/// "3 minutes ago", for how old an autosave is.
fn age_text(age: Duration) -> String {
    let secs = age.as_secs();
    let (n, unit) = if secs < 90 {
        return "a moment ago".to_string();
    } else if secs < 3600 {
        (secs / 60, "minute")
    } else if secs < 86_400 {
        (secs / 3600, "hour")
    } else {
        (secs / 86_400, "day")
    };
    format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" })
}
