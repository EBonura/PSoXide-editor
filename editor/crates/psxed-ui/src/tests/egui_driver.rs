//! Real-egui driving helpers shared by the workspace tests: draw whole
//! `workspace.draw` frames, find a rendered label by its galley text, and
//! deliver pointer presses at it.

use super::*;

/// Every rendered widget whose galley text equals `label`. Tests that click
/// a label by name use this to prove the label is unambiguous first: the
/// Inspector renders several same-named buttons, and `text_shape_centers`
/// silently returns whichever one is painted first.
pub(super) fn text_shape_centers(shapes: &[egui::epaint::ClippedShape], label: &str) -> Vec<Pos2> {
    fn collect(shape: &egui::Shape, label: &str, found: &mut Vec<Pos2>) {
        match shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                found.push(text.pos + text.galley.rect.center().to_vec2());
            }
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    collect(shape, label, found);
                }
            }
            _ => {}
        }
    }
    let mut found = Vec::new();
    for shape in shapes {
        collect(&shape.shape, label, &mut found);
    }
    found
}

/// Real-egui context plus edit-mode viewport for driving whole
/// `workspace.draw` frames. Shared by the Inspector click/type helpers so
/// both see the same font set (the `lucide` icon family is aliased onto the
/// proportional family, otherwise icon labels render as tofu and cannot be
/// located by galley text).
pub(super) fn real_egui_workspace_ctx(name: &str) -> (egui::Context, EditorViewport3dPresentation) {
    let ctx = egui::Context::default();
    let mut fonts = egui::FontDefinitions::default();
    let proportional = fonts
        .families
        .get(&egui::FontFamily::Proportional)
        .cloned()
        .expect("default proportional font family");
    fonts
        .families
        .insert(egui::FontFamily::Name("lucide".into()), proportional);
    ctx.set_fonts(fonts);
    let texture = ctx.load_texture(
        name,
        egui::ColorImage::new([1, 1], egui::Color32::BLACK),
        egui::TextureOptions::NEAREST,
    );
    (
        ctx,
        EditorViewport3dPresentation::edit(texture.id(), Vec::new()),
    )
}

/// One full `workspace.draw` frame at `time` with `events` delivered.
pub(super) fn real_egui_workspace_frame(
    ctx: &egui::Context,
    workspace: &mut EditorWorkspace,
    viewport: &EditorViewport3dPresentation,
    time: f64,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1800.0, 1000.0))),
            time: Some(time),
            events,
            ..egui::RawInput::default()
        },
        |ctx| workspace.draw(ctx, viewport.clone(), EditorPlaytestStatus::Idle),
    )
}

/// Locate the one widget whose rendered galley text equals `label`, and
/// prove the pointer can actually reach it.
///
/// Panics when the label is missing, ambiguous (the Inspector paints several
/// same-named buttons, so taking the first match could drive a control from
/// another section), or painted outside the screen. That last check is not
/// pedantry: an Inspector row wide enough to overflow its panel still emits
/// its text shapes, but egui clips the widget out of the interact layer, so
/// the control becomes permanently unclickable in the real editor.
fn locate_unique_label(
    ctx: &egui::Context,
    workspace: &mut EditorWorkspace,
    viewport: &EditorViewport3dPresentation,
    label: &str,
) -> Pos2 {
    // One frame to settle: egui lays a window out on its first frame and draws
    // it from the second (which one a window needs depends on the panels
    // around it), and the editor draws frames all the time.
    let _ = real_egui_workspace_frame(ctx, workspace, viewport, 0.0, vec![]);
    let frame = real_egui_workspace_frame(ctx, workspace, viewport, 0.0, vec![]);
    let found = text_shape_centers(&frame.shapes, label);
    assert_eq!(
        found.len(),
        1,
        "label {label:?} must be visible exactly once in the drawn frame, saw {found:?}"
    );
    let point = found[0];
    let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1800.0, 1000.0));
    assert!(
        screen.contains(point),
        "label {label:?} is painted at {point:?}, outside the {screen:?} screen: \
         its row overflows the panel and the control is unreachable"
    );
    point
}

pub(super) fn press_release(point: Pos2) -> (Vec<egui::Event>, Vec<egui::Event>) {
    (
        vec![
            egui::Event::PointerMoved(point),
            egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        vec![egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    )
}

/// Click the widget whose rendered galley text equals `label`, through full
/// `workspace.draw` frames so the Inspector transaction wrapper owns history
/// exactly as it does in production.
pub(super) fn run_real_egui_workspace_click_on_label(workspace: &mut EditorWorkspace, label: &str) {
    let (ctx, viewport) = real_egui_workspace_ctx("workspace-label-click");
    let point = locate_unique_label(&ctx, workspace, &viewport, label);
    let (press, release) = press_release(point);
    let _ = real_egui_workspace_frame(&ctx, workspace, &viewport, 1.0 / 60.0, press);
    let _ = real_egui_workspace_frame(&ctx, workspace, &viewport, 2.0 / 60.0, release);
    let _ = real_egui_workspace_frame(&ctx, workspace, &viewport, 3.0 / 60.0, vec![]);
}
