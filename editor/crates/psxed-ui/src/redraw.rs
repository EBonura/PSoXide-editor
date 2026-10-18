//! How often the shell should redraw the editor window.
//!
//! An immediate-mode UI redraws whatever it is asked to, so an idle editor
//! costs whatever the shell's tick costs. The shell asks
//! [`EditorWorkspace::redraw_interval`] how long it may wait before the next
//! redraw when nothing else (a pointer or key event, a repaint egui itself
//! asked for) wakes it sooner:
//!
//! * a frame's worth after the last input, so a gesture, a camera fly or the
//!   hover highlight follow the hand at the display rate;
//! * the console's frame rate (30 Hz) while a cook or build is running, or
//!   while the focused 3D view shows something that moves with the clock (a
//!   model's idle clip, a beacon, particles), because the console draws those
//!   at 30 Hz as well;
//! * a tenth of a second otherwise. That still drains queued MCP calls, notices
//!   a project file changed on disk and picks up a finished background job
//!   within a tenth of a second, and costs a few hundredths of a core instead
//!   of a whole one.

use super::*;

use std::cell::Cell;
use std::time::{Duration, Instant};

/// How long after the last input event the editor keeps redrawing at the
/// display rate.
pub const ACTIVE_AFTER_INPUT: Duration = Duration::from_millis(1500);
/// Redraw interval while the user is working (60 Hz).
pub const REDRAW_ACTIVE: Duration = Duration::from_micros(16_667);
/// Redraw interval for a cook, a build or an animated preview (30 Hz).
pub const REDRAW_ANIMATED: Duration = Duration::from_micros(33_333);
/// Redraw interval when nothing is happening (10 Hz).
pub const REDRAW_IDLE: Duration = Duration::from_millis(100);

thread_local! {
    static PREVIEW_ANIMATED: Cell<bool> = const { Cell::new(false) };
}

/// The frontend says whether the 3D preview it just drew moves with the
/// clock (an autoplaying model, a beacon, a particle emitter, a layered sky).
/// Such a preview needs a steady redraw even when nobody touches the editor.
pub fn publish_preview_animated(animated: bool) {
    PREVIEW_ANIMATED.with(|flag| flag.set(animated));
}

fn preview_animated() -> bool {
    PREVIEW_ANIMATED.with(Cell::get)
}

/// What the editor knows between frames to choose its redraw interval.
pub(crate) struct RedrawState {
    pub(crate) last_input: Option<Instant>,
    pub(crate) focused: bool,
    pub(crate) busy: bool,
}

impl Default for RedrawState {
    fn default() -> Self {
        Self {
            last_input: None,
            focused: true,
            busy: false,
        }
    }
}

/// The interval, from the facts it depends on.
pub(crate) fn redraw_interval_for(
    since_input: Option<Duration>,
    busy: bool,
    animated_preview_visible: bool,
    focused: bool,
) -> Duration {
    if since_input.is_some_and(|elapsed| elapsed < ACTIVE_AFTER_INPUT) {
        REDRAW_ACTIVE
    } else if busy || (focused && animated_preview_visible) {
        REDRAW_ANIMATED
    } else {
        REDRAW_IDLE
    }
}

impl EditorWorkspace {
    /// Record what this frame says about how busy the editor is.
    pub(crate) fn note_frame_activity(
        &mut self,
        ctx: &egui::Context,
        playtest_status: EditorPlaytestStatus,
    ) {
        let (input, focused) = ctx.input(|input| {
            (
                !input.events.is_empty() || input.pointer.any_down(),
                input.viewport().focused.unwrap_or(true),
            )
        });
        if input {
            self.redraw.last_input = Some(Instant::now());
        }
        self.redraw.focused = focused;
        self.redraw.busy =
            playtest_status.is_active() || !matches!(self.interaction, Interaction::Idle);
    }

    /// How long the shell may wait before redrawing the editor, absent any
    /// input event or repaint request (see the module docs).
    pub fn redraw_interval(&self) -> Duration {
        redraw_interval_for(
            self.redraw.last_input.map(|at| at.elapsed()),
            self.redraw.busy,
            preview_animated() && self.editor_3d_preview_visible(),
            self.redraw.focused,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECOND: Duration = Duration::from_secs(1);

    #[test]
    fn input_keeps_the_display_rate_for_a_moment() {
        assert_eq!(
            redraw_interval_for(Some(Duration::ZERO), false, false, true),
            REDRAW_ACTIVE
        );
        assert_eq!(
            redraw_interval_for(Some(SECOND), false, true, true),
            REDRAW_ACTIVE
        );
        assert_eq!(
            redraw_interval_for(Some(2 * SECOND), false, false, true),
            REDRAW_IDLE
        );
    }

    #[test]
    fn a_still_editor_ticks_slowly_and_an_animated_one_at_the_console_rate() {
        assert_eq!(redraw_interval_for(None, false, false, true), REDRAW_IDLE);
        assert_eq!(
            redraw_interval_for(None, false, true, true),
            REDRAW_ANIMATED
        );
        // Nobody is looking at an unfocused window: its animation can wait.
        assert_eq!(redraw_interval_for(None, false, true, false), REDRAW_IDLE);
    }

    #[test]
    fn a_cook_or_build_is_followed_at_the_console_rate_even_unfocused() {
        assert_eq!(
            redraw_interval_for(None, true, false, false),
            REDRAW_ANIMATED
        );
    }
}
