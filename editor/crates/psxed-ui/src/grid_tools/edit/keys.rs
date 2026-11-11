//! The keyboard of the Edit tool.
//!
//! * During a gesture the tool takes digits, minus, the decimal point,
//!   Backspace, Delete, Enter and Esc before anything else sees them: a
//!   typed number sets the exact amount in clicks, Enter applies it, Esc
//!   puts the document back exactly. Backspace edits the number and never
//!   deletes the selection.
//! * With a selection and no gesture, digits start a number too; Enter
//!   applies it as a height offset of the whole selection.
//! * `R` repeats the last action on the current selection.
//! * The backtick key cycles what is under the pointer, finest first.

use super::*;

fn press(ctx: &egui::Context, key: egui::Key) -> bool {
    ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key))
}

const DIGITS: [(egui::Key, u8); 10] = [
    (egui::Key::Num0, 0),
    (egui::Key::Num1, 1),
    (egui::Key::Num2, 2),
    (egui::Key::Num3, 3),
    (egui::Key::Num4, 4),
    (egui::Key::Num5, 5),
    (egui::Key::Num6, 6),
    (egui::Key::Num7, 7),
    (egui::Key::Num8, 8),
    (egui::Key::Num9, 9),
];

/// Take this frame's Edit keys. Returns whether the frame's grid shortcuts
/// stop here.
pub(in crate::grid_tools) fn handle(ws: &mut EditorWorkspace, ctx: &egui::Context) -> bool {
    if ws.grid.tool != GridTool::Edit {
        return false;
    }
    if ws.grid.edit.drag.is_some() {
        return during_gesture(ws, ctx);
    }
    if ws.grid.edit.moving.is_some() {
        if press(ctx, egui::Key::Escape) {
            cancel(ws);
        }
        // Everything else waits for the end of the gesture.
        return true;
    }
    if press(ctx, egui::Key::Backtick) {
        ws.grid.edit.cycle.0 = (ws.grid.edit.cycle.0 + 1) % 3;
        ws.grid.edit.cycle.1 = ctx.input(|i| i.pointer.latest_pos());
        ws.status = match ws.grid.edit.cycle.0 {
            0 => "Picking the finest element under the pointer".to_string(),
            1 => "Picking the face under the pointer".to_string(),
            _ => "Picking the face under the pointer, coarser".to_string(),
        };
    }
    if !ws.is_grid_world() || ws.grid.selection.is_empty() {
        return false;
    }
    let typing = !ws.grid.edit.amount.is_empty();
    let mut changed = false;
    for (key, digit) in DIGITS {
        if press(ctx, key) {
            ws.grid.edit.amount.digit(digit);
            changed = true;
        }
    }
    if press(ctx, egui::Key::Minus) {
        ws.grid.edit.amount.minus();
        changed = true;
    }
    // A point frames the selection (the editor's key) unless a number is on
    // its way.
    if typing && press(ctx, egui::Key::Period) {
        ws.grid.edit.amount.point();
        changed = true;
    }
    if typing {
        if press(ctx, egui::Key::Backspace) {
            ws.grid.edit.amount.backspace();
            changed = true;
        }
        if press(ctx, egui::Key::Escape) {
            ws.grid.edit.amount.clear();
            ws.status = "Offset cleared".to_string();
            ws.grid.edit.hint.clear();
            return true;
        }
        if press(ctx, egui::Key::Enter) {
            let text = ws.grid.edit.amount.clone();
            ws.grid.edit.amount.clear();
            match text.units() {
                Some(units) => repeat_move::offset_selection(ws, units),
                None => ws.status = "Not a number".to_string(),
            }
            return true;
        }
    }
    if changed || typing {
        let text = ws.grid.edit.amount.text().to_string();
        ws.grid.edit.hint =
            format!("Offset the selection by {text} clicks | Enter applies | Esc clears");
        ws.status = ws.grid.edit.hint.clone();
        return true;
    }
    if ws.grid.plane_sel.is_empty() && press(ctx, egui::Key::R) {
        repeat_move::repeat(ws);
        return true;
    }
    false
}

fn during_gesture(ws: &mut EditorWorkspace, ctx: &egui::Context) -> bool {
    if press(ctx, egui::Key::Escape) {
        cancel(ws);
        return true;
    }
    let mut changed = false;
    for (key, digit) in DIGITS {
        if press(ctx, key) {
            ws.grid.edit.amount.digit(digit);
            changed = true;
        }
    }
    if press(ctx, egui::Key::Minus) {
        ws.grid.edit.amount.minus();
        changed = true;
    }
    if press(ctx, egui::Key::Period) {
        ws.grid.edit.amount.point();
        changed = true;
    }
    // Backspace and Delete never reach the selection in the middle of a
    // gesture.
    if press(ctx, egui::Key::Backspace) {
        ws.grid.edit.amount.backspace();
        changed = true;
    }
    press(ctx, egui::Key::Delete);
    if changed {
        apply_typed(ws);
    }
    if press(ctx, egui::Key::Enter) {
        apply_typed(ws);
        finish(ws);
    }
    true
}

/// Apply the typed number to the gesture in flight, when it reads as one.
fn apply_typed(ws: &mut EditorWorkspace) {
    let Some(steps) = ws.grid.edit.drag.as_ref().map(|d| d.plan.counts_steps()) else {
        return;
    };
    let amount = &ws.grid.edit.amount;
    let value = if steps {
        amount.steps()
    } else {
        amount.units()
    };
    if let Some(value) = value {
        set_amount(ws, value);
    }
    let text = ws.grid.edit.amount.text().to_string();
    if !text.is_empty() {
        let hint = format!("{} | typed {text} | Enter applies, Esc cancels", ws.status);
        ws.grid.edit.hint = hint;
    }
}
