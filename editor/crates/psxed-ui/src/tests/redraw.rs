//! The editor tells the shell how soon it needs the next redraw.

use super::*;
use crate::redraw::{REDRAW_ACTIVE, REDRAW_ANIMATED, REDRAW_IDLE};

#[test]
fn the_workspace_follows_what_the_frame_saw() {
    let ctx = egui::Context::default();
    let mut ws = EditorWorkspace::with_project(
        test_temp_dir("redraw"),
        psxed_project::ProjectDocument::new("redraw"),
    );
    // Never touched: idle.
    assert_eq!(ws.redraw_interval(), REDRAW_IDLE);
    // A frame with an input event.
    let _ = ctx.run(
        egui::RawInput {
            events: vec![egui::Event::PointerMoved(egui::pos2(10.0, 10.0))],
            ..egui::RawInput::default()
        },
        |ctx| ws.note_frame_activity(ctx, EditorPlaytestStatus::Idle),
    );
    assert_eq!(ws.redraw_interval(), REDRAW_ACTIVE);
    // A build in progress while nothing is touched.
    ws.redraw.last_input = None;
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        ws.note_frame_activity(ctx, EditorPlaytestStatus::Building)
    });
    assert_eq!(ws.redraw_interval(), REDRAW_ANIMATED);
}
