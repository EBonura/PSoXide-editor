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

// ---- Autosave and crash recovery ----

mod autosave {
    use super::super::grid_rig::flat_project;
    use super::*;
    use crate::workspace::autosave::{AUTOSAVE_FILE, LONGEST_UNWRITTEN, QUIET_BEFORE_WRITE};
    use std::time::{Duration, Instant};

    fn saved_workspace(name: &str) -> EditorWorkspace {
        let dir = test_temp_dir(name);
        let mut ws = EditorWorkspace::with_project(dir, flat_project(3, 3, true));
        ws.save().unwrap();
        ws
    }

    fn edit(ws: &mut EditorWorkspace) {
        let n = ws.project.resources.len();
        ws.project.add_resource(
            format!("Autosave test {n}"),
            psxed_project::ResourceData::Material(psxed_project::MaterialResource::opaque(None)),
        );
        ws.mark_dirty();
    }

    #[test]
    fn a_quiet_moment_after_a_change_writes_a_sidecar_and_never_touches_project_ron() {
        let mut ws = saved_workspace("autosave-quiet");
        let ron = ws.project_dir.join("project.ron");
        let before = std::fs::read(&ron).unwrap();
        let t0 = Instant::now();
        ws.autosave_tick(t0, false);
        assert!(
            !ws.autosave_path().exists(),
            "nothing unsaved, nothing written"
        );
        edit(&mut ws);
        ws.autosave_tick(t0, false);
        ws.autosave_tick(t0 + QUIET_BEFORE_WRITE / 2, false);
        assert!(!ws.autosave_path().exists(), "not yet quiet");
        ws.autosave_tick(t0 + QUIET_BEFORE_WRITE + Duration::from_millis(10), true);
        assert!(!ws.autosave_path().exists(), "never while a drag is held");
        ws.autosave_tick(t0 + QUIET_BEFORE_WRITE + Duration::from_millis(20), false);
        assert!(ws.autosave_path().ends_with(AUTOSAVE_FILE));
        assert!(ws.autosave_path().exists());
        assert_eq!(
            std::fs::read(&ron).unwrap(),
            before,
            "project.ron untouched"
        );
        assert_eq!(ws.autosave.writes, 1);
        // Idle ticks do not rewrite the same revision.
        ws.autosave_tick(t0 + Duration::from_secs(20), false);
        assert_eq!(ws.autosave.writes, 1);
    }

    #[test]
    fn edits_that_never_go_quiet_are_written_at_the_longest_wait() {
        let mut ws = saved_workspace("autosave-overdue");
        let t0 = Instant::now();
        edit(&mut ws);
        ws.autosave_tick(t0, false);
        // A change every second keeps it from going quiet.
        for s in 1..LONGEST_UNWRITTEN.as_secs() {
            edit(&mut ws);
            ws.autosave_tick(t0 + Duration::from_secs(s), false);
        }
        assert!(!ws.autosave_path().exists());
        edit(&mut ws);
        ws.autosave_tick(t0 + LONGEST_UNWRITTEN, false);
        assert!(ws.autosave_path().exists());
    }

    #[test]
    fn a_normal_save_removes_the_sidecar() {
        let mut ws = saved_workspace("autosave-save");
        let t0 = Instant::now();
        edit(&mut ws);
        ws.autosave_tick(t0, false);
        ws.autosave_tick(t0 + Duration::from_secs(3), false);
        assert!(ws.autosave_path().exists());
        ws.save().unwrap();
        assert!(!ws.autosave_path().exists());
    }

    #[test]
    fn opening_a_project_with_a_newer_sidecar_asks_and_restore_brings_the_work_back() {
        let mut ws = saved_workspace("autosave-recover");
        let dir = ws.project_dir.clone();
        let t0 = Instant::now();
        edit(&mut ws);
        let edited = ws.project.clone();
        ws.autosave_tick(t0, false);
        ws.autosave_tick(t0 + Duration::from_secs(3), false);
        // The editor dies here. A newer file than project.ron is what the
        // sidecar is on disk; make that certain whatever the clock grain.
        let later = std::time::SystemTime::now() + Duration::from_secs(5);
        std::fs::File::options()
            .write(true)
            .open(dir.join(AUTOSAVE_FILE))
            .unwrap()
            .set_modified(later)
            .unwrap();
        drop(ws);
        let mut reopened = EditorWorkspace::open_directory(&dir).unwrap();
        assert!(
            matches!(reopened.modal, Modal::RecoverAutosave { .. }),
            "asks to recover"
        );
        assert!(!reopened.dirty);
        reopened.restore_autosave().unwrap();
        reopened.modal = Modal::None;
        assert!(reopened.dirty, "restored work is unsaved until saved");
        assert_eq!(reopened.project.resources.len(), edited.resources.len());
        reopened.save().unwrap();
        assert!(!dir.join(AUTOSAVE_FILE).exists());
    }

    #[test]
    fn keeping_the_saved_version_removes_the_sidecar_and_an_old_sidecar_is_ignored() {
        let mut ws = saved_workspace("autosave-keep");
        let dir = ws.project_dir.clone();
        let t0 = Instant::now();
        edit(&mut ws);
        ws.autosave_tick(t0, false);
        ws.autosave_tick(t0 + Duration::from_secs(3), false);
        let side = dir.join(AUTOSAVE_FILE);
        // Older than the saved project: stale, dropped without a question.
        let earlier = std::time::SystemTime::now() - Duration::from_secs(3600);
        std::fs::File::options()
            .write(true)
            .open(&side)
            .unwrap()
            .set_modified(earlier)
            .unwrap();
        let reopened = EditorWorkspace::open_directory(&dir).unwrap();
        assert!(matches!(reopened.modal, Modal::None));
        assert!(!side.exists());
    }

    #[test]
    fn an_autosave_that_cannot_be_read_is_set_aside_not_offered() {
        let ws = saved_workspace("autosave-bad");
        let dir = ws.project_dir.clone();
        let side = dir.join(AUTOSAVE_FILE);
        std::fs::write(&side, "this is not a project").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&side)
            .unwrap()
            .set_modified(std::time::SystemTime::now() + Duration::from_secs(5))
            .unwrap();
        drop(ws);
        let reopened = EditorWorkspace::open_directory(&dir).unwrap();
        assert!(matches!(reopened.modal, Modal::None));
        assert!(dir.join(".psxed-autosave.bad.ron").exists());
    }
}

// ---- The check before Play ----

mod play_check {
    use super::super::grid_rig::flat_project;
    use super::*;
    use crate::workspace::play_check::ProblemTarget;
    use psxed_project::grid::{GridRoom, GridSurface};

    /// A 4 x 4 hall with the player in it, and a 2 x 2 room far to the east
    /// that nothing joins to the hall.
    fn island_level() -> EditorWorkspace {
        let mut project = flat_project(4, 4, true);
        let mut island = GridRoom::new("Island", 2, 2);
        island.origin = [12, 0];
        for z in 0..2 {
            for x in 0..2 {
                let s = island.sector_or_insert(x, z).unwrap();
                s.set_ground_floor(Some(GridSurface::flat(0, None)));
                s.set_ground_ceiling(Some(GridSurface::flat(3072, None)));
            }
        }
        project
            .active_scene_mut()
            .world
            .grid_mut()
            .unwrap()
            .rooms
            .push(island);
        let spawn = project.active_scene_mut().add_node(
            psxed_project::NodeId::ROOT,
            "Player Spawn",
            psxed_project::NodeKind::SpawnPoint {
                player: true,
                character: None,
            },
        );
        project
            .active_scene_mut()
            .node_mut(spawn)
            .unwrap()
            .transform
            .translation = [1024.0, 0.0, 1024.0];
        EditorWorkspace::with_project(test_temp_dir("play-check"), project)
    }

    #[test]
    fn a_room_the_player_cannot_walk_to_is_named_in_a_sentence_with_a_link() {
        let ws = island_level();
        let problems = ws.walkability_problems();
        assert_eq!(problems.len(), 1, "{problems:?}");
        let p = &problems[0];
        assert!(
            p.text
                .starts_with("In Island, 4 sectors cannot be walked to"),
            "{}",
            p.text
        );
        assert!(!p.blocking, "a lever may open the way: a note, not a stop");
        assert!(matches!(
            p.target,
            Some(ProblemTarget::Sector { room: 1, .. })
        ));
    }

    #[test]
    fn no_player_start_is_said_plainly() {
        let ws = EditorWorkspace::with_project(
            test_temp_dir("play-check-none"),
            flat_project(3, 3, true),
        );
        let problems = ws.walkability_problems();
        assert_eq!(problems.len(), 1);
        assert!(
            problems[0].text.contains("no player start"),
            "{}",
            problems[0].text
        );
    }

    #[test]
    fn reporting_puts_the_sentence_in_the_status_the_console_and_the_list_and_select_it_frames_the_room(
    ) {
        let mut ws = island_level();
        let problems = ws.walkability_problems();
        ws.report_play_problems(problems.clone());
        assert!(
            ws.status.starts_with("Heads up: In Island"),
            "{}",
            ws.status
        );
        assert!(ws
            .play_debug_terminal_lines
            .iter()
            .any(|l| l.starts_with("[check] Note: In Island")));
        assert_eq!(ws.play_problems, problems);
        assert!(ws.select_play_problem(problems[0].target.unwrap()));
        assert_eq!(ws.selection.selected_room, Some(1));
        // A blocking one says it stopped Play and leads the status.
        ws.report_play_problems(vec![crate::workspace::play_check::PlayProblem {
            text: "The level needs more memory than the PlayStation has.".to_string(),
            blocking: true,
            target: None,
        }]);
        assert!(ws
            .status
            .starts_with("Play stopped: The level needs more memory"));
    }
}

// ---- The ruler ----

mod ruler {
    use super::super::grid_rig::*;
    use super::*;
    use crate::grid_tools::GridTool;

    #[test]
    fn sizes_are_told_in_sectors_and_player_heights_and_a_rise_says_if_it_can_be_stepped() {
        let ws = EditorWorkspace::with_project(test_temp_dir("ruler"), flat_project(4, 4, true));
        let world = ws.project.active_scene().world.grid().unwrap();
        // The test world is scale 1:1: a sector is 1024 editor units, which is
        // exactly one player height.
        assert_eq!(
            crate::grid_tools::ruler_length_text(world, 3 * 1024),
            "3 sectors, 3.0 player heights"
        );
        let limit = crate::grid_tools::ruler_step_up(world).expect("a cookable scale");
        assert!(limit > 0 && limit < 1024, "{limit}");
        assert!(
            crate::grid_tools::ruler_rise_text(world, limit).ends_with("the player steps up it")
        );
        assert!(
            crate::grid_tools::ruler_rise_text(world, limit + 64).ends_with("too tall to step up")
        );
        assert!(!crate::grid_tools::ruler_rise_text(world, -256).contains("step"));
    }

    #[test]
    fn a_floor_drag_in_the_edit_tool_shows_its_size_in_the_hint() {
        let mut rig = GridRig::persp("ruler-drag", flat_project(5, 5, true));
        rig.tool(GridTool::Edit);
        let from = rig.screen_of([2.5 * S, 0.0, 2.5 * S]);
        let to = rig.screen_of([2.5 * S, 256.0, 2.5 * S]);
        rig.drag_hold(from, to, egui::Modifiers::NONE);
        let hint = rig.ws.grid.edit.hint.clone();
        assert!(hint.contains("player heights"), "{hint}");
        assert!(hint.contains("the player steps up it"), "{hint}");
        rig.release(to, egui::Modifiers::NONE);
    }

    #[test]
    fn the_player_ghost_is_off_until_the_overlay_is_switched_on() {
        let ws = EditorWorkspace::with_project(test_temp_dir("ghost"), flat_project(2, 2, true));
        assert!(!ws.grid.overlays.player_ghost);
    }
}
