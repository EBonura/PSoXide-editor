//! The final polish of the editor frame at small windows: the left dock, the
//! World form, the Library drawer, the top bar and the hint bar, and the
//! things the editor remembers.

use super::grid_live::WindowRig;
use super::*;

pub(super) const SMALL: Vec2 = Vec2::new(1280.0, 720.0);

/// Every text of the last frame with the rectangle it is drawn in, when that
/// rectangle lies whole inside the clip it is drawn under.
pub(super) fn visible_texts(rig: &WindowRig) -> Vec<(String, Rect)> {
    rig.out
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) => {
                let rect = t.visual_bounding_rect();
                let clip = s.clip_rect.expand(0.5);
                (clip.contains_rect(rect) && rect.width() > 0.0)
                    .then(|| (t.galley.text().to_string(), rect))
            }
            _ => None,
        })
        .collect()
}

pub(super) fn visible(rig: &WindowRig, text: &str) -> Option<Rect> {
    visible_texts(rig)
        .into_iter()
        .find(|(t, _)| t.trim_start_matches(|c: char| !c.is_alphanumeric()).trim() == text)
        .map(|(_, r)| r)
}

#[test]
fn capture_small() {
    let Some(dir) = std::env::var_os("PSXED_POLISH_CAPTURE") else {
        return;
    };
    let mut rig = WindowRig::starter(SMALL);
    rig.capture(&Path::new(&dir).join("small.png"));
    rig.ws
        .project
        .active_scene_mut()
        .world
        .grid_mut()
        .unwrap()
        .shading = psxed_project::grid::FacingShade::SUBTLE;
    rig.open_library();
    rig.capture(&Path::new(&dir).join("small-library.png"));
}

#[test]
fn at_1280_by_720_the_rooms_list_sits_clear_of_the_objects_panel() {
    let rig = WindowRig::starter(SMALL);
    let rooms = visible(&rig, "Rooms").expect("the Rooms header is drawn whole");
    let objects = visible(&rig, "Objects").expect("the Objects header is drawn whole");
    assert!(
        rooms.bottom() < objects.top(),
        "Rooms {rooms:?} above Objects {objects:?}"
    );
    assert!(
        visible_texts(&rig)
            .iter()
            .any(|(t, _)| t.contains("Starter room")),
        "the room's row is drawn whole"
    );
}

#[test]
fn the_structure_and_objects_handle_moves_the_split() {
    let mut rig = WindowRig::starter(SMALL);
    let before = rig.ws.grid.structure_fraction;
    let objects = visible(&rig, "Objects").unwrap();
    // The handle is the strip just above the Objects header.
    let handle = Pos2::new(objects.left() + 60.0, objects.top() - 16.0);
    rig.drag(handle, handle + Vec2::new(0.0, -60.0));
    assert!(
        rig.ws.grid.structure_fraction < before,
        "dragging the handle up gives Objects the room"
    );
    let rooms = visible(&rig, "Objects").unwrap();
    assert!(rooms.top() < objects.top());
}

#[test]
fn the_shading_fields_fit_the_inspector_at_every_dock_width() {
    let mut rig = WindowRig::starter(SMALL);
    rig.ws
        .project
        .active_scene_mut()
        .world
        .grid_mut()
        .unwrap()
        .shading = psxed_project::grid::FacingShade::STRONG;
    rig.frame(vec![]);
    rig.frame(vec![]);
    for label in ["Floor", "Ceil", "E-W", "N-S"] {
        let rect = visible(&rig, label).unwrap_or_else(|| panic!("`{label}` is clipped"));
        assert!(rect.right() < SMALL.x - 40.0, "{label} {rect:?}");
    }
    let texts = visible_texts(&rig);
    // Each percentage field is drawn too (four of them, the last at the right).
    assert!(
        texts.iter().filter(|(t, _)| t.ends_with('%')).count() >= 4,
        "{:?}",
        texts
            .iter()
            .filter(|(t, _)| t.ends_with('%'))
            .collect::<Vec<_>>()
    );
}

#[test]
fn the_library_drawer_opens_tall_enough_for_thumbnails_and_keeps_a_height_per_view() {
    let mut rig = WindowRig::starter(SMALL);
    rig.open_library();
    let tabs = visible(&rig, "Materials").expect("the Library tabs are drawn");
    let paint = visible_texts(&rig)
        .into_iter()
        .find(|(t, _)| t.contains(" of "))
        .map(|(_, r)| r)
        .expect("the count under the thumbnails is drawn");
    assert!(
        paint.top() - tabs.bottom() >= 90.0,
        "room for thumbnails between the tabs and the Paint line: {tabs:?} {paint:?}"
    );
}

#[test]
fn a_long_project_name_never_pushes_help_out_of_the_bar() {
    let mut rig = WindowRig::starter(SMALL);
    rig.ws.project.name = "A very long project name that nobody would type twice".to_string();
    rig.frame(vec![]);
    rig.frame(vec![]);
    let help = visible(&rig, "Help").expect("Help is drawn whole");
    let segments = visible(&rig, "World").expect("the workspace control is drawn");
    assert!(
        help.right() < segments.left() - 20.0,
        "{help:?} {segments:?}"
    );
}

#[test]
fn a_long_hint_keeps_its_first_and_last_words_and_gives_way_in_the_middle() {
    let ctx = egui::Context::default();
    let font = egui::FontId::proportional(12.0);
    let hint = "Over a face | click selects | Shift+click adds a range | Ctrl toggles | drag raises | Esc cancels the drag";
    let mut shown = Vec::new();
    for width in [900.0, 360.0, 220.0, 120.0] {
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                shown.push(crate::grid_tools::elide_hint(ui, hint, width, &font));
            });
        });
    }
    assert_eq!(shown[0], hint, "a hint that fits is left alone");
    assert!(shown[1].starts_with("Over a face"), "{}", shown[1]);
    assert!(shown[1].contains("..."), "{}", shown[1]);
    assert!(shown[1].ends_with("Esc cancels the drag"), "{}", shown[1]);
    assert!(shown[2].len() < shown[1].len(), "{} {}", shown[1], shown[2]);
    assert!(shown[3].len() < hint.len() && !shown[3].is_empty());
}

#[test]
fn the_world_form_says_how_many_save_slots_are_used() {
    let mut ws = EditorWorkspace::with_project(
        test_temp_dir("save-slots"),
        super::grid_rig::flat_project(2, 2, true),
    );
    assert_eq!(ws.save_slots_text(), "slots used 0 of 832");
    ws.last_slots_used = Some(3);
    assert_eq!(ws.save_slots_text(), "slots used 3 of 832");
}

#[test]
fn continue_from_save_is_its_own_choice_among_the_game_actions() {
    use crate::inspector_character_ui::{ui_action_same_variant, CONTINUE_ACTION_ID};
    use psxed_project::UiAction;
    assert!(!ui_action_same_variant(
        &UiAction::Game(0),
        &UiAction::Game(CONTINUE_ACTION_ID)
    ));
    assert!(ui_action_same_variant(
        &UiAction::Game(7),
        &UiAction::Game(0)
    ));
    assert_eq!(CONTINUE_ACTION_ID, 401);
}
