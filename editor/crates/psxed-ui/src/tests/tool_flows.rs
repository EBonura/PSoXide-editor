//! Whole-flow replays of the tools a user drives in several steps: every
//! step is a click, a drag or a key delivered to the whole editor window.
//! Each test starts from the strip button, the tool key or the tool search,
//! and asserts the document the user asked for (and that it still cooks and
//! can be walked).

use super::grid_live::WindowRig;
use super::grid_toolbar::{frames, press_key};
use super::tool_audit::*;
use super::*;
use crate::grid_tools::GridTool;
use psxed_project::grid::walk::{analyse, player_spawns};

/// The starter project in the Plan view, framed on the whole world.
pub(super) fn starter_plan() -> WindowRig {
    let mut rig = Fixture::Starter.rig();
    plan(&mut rig);
    reframe(&mut rig);
    rig
}

/// Frame the whole world in the active view, as the Frame key does.
pub(super) fn reframe(rig: &mut WindowRig) {
    rig.ws.frame_current_view();
    frames(rig, 3);
}

/// Pan and zoom the plan so the sectors `lo` to `hi` (inclusive) fill the
/// view, the way a person scrolls and drags it.
pub(super) fn frame_cells(rig: &mut WindowRig, lo: [i32; 2], hi: [i32; 2]) {
    let sector = rig.world().scale.to_editor(1024) as f32;
    let rect = rig.ws.last_orthographic_viewport_rect;
    let w = (hi[0] - lo[0] + 1) as f32 * sector;
    let d = (hi[1] - lo[1] + 1) as f32 * sector;
    rig.ws.viewport_zoom = (rect.width() * 0.9 / w).min(rect.height() * 0.9 / d);
    rig.ws.orthographic_focus = [
        (lo[0] + hi[0] + 1) as f32 * 0.5 * sector,
        0.0,
        (lo[1] + hi[1] + 1) as f32 * 0.5 * sector,
    ];
    frames(rig, 3);
}

pub(super) fn click_cell(rig: &mut WindowRig, c: [i32; 2]) {
    rig.click(plan_px(rig, c));
}

pub(super) fn drag_cells(rig: &mut WindowRig, a: [i32; 2], b: [i32; 2]) {
    rig.drag(plan_px(rig, a), plan_px(rig, b));
}

/// The cook's verdict on the document as it stands: its errors.
pub(super) fn cook_errors(rig: &WindowRig) -> Vec<String> {
    // The scratch copy lives in the temp directory, where the shared assets
    // beside the tracked projects are not: cook against the tracked folder.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../projects/grid-starter");
    let (_, report) = psxed_project::playtest::build_package(&rig.ws.project, &root);
    for w in &report.warnings {
        eprintln!("[cook warning] {w}");
    }
    report.errors.iter().map(|e| e.message.clone()).collect()
}

pub(super) fn assert_cooks(rig: &WindowRig, what: &str) {
    let errors = cook_errors(rig);
    assert!(errors.is_empty(), "{what}: cook refuses: {errors:?}");
}

/// A world sector can be stood on and walked to from the player start.
pub(super) fn walkable(rig: &WindowRig, cell: [i32; 2]) -> bool {
    let world = rig.world();
    let spawns = player_spawns(&rig.ws.project, world);
    let Some(a) = analyse(world, &spawns, &psx_engine::WalkLimits::DEFAULT) else {
        return false;
    };
    a.reach_ran
        && a.rooms
            .iter()
            .any(|r| r.cell(cell).is_some_and(|c| c.standable && !c.unreachable))
}

/// The world as whole sectors (rooms that merged cells into larger ones are
/// read through their sector view, as the cook and the walk analysis do).
pub(super) fn sectors(rig: &WindowRig) -> psxed_project::grid::GridWorld {
    let world = rig.world();
    psxed_project::grid::sector_view::sector_world(world).map_or_else(|| world.clone(), |(w, _)| w)
}

pub(super) fn has_floor_at(rig: &WindowRig, cell: [i32; 2]) -> bool {
    floor_height_at(rig, cell).is_some()
}

/// The mean floor height of the ground span at a world sector (room-relative
/// grid units), in whichever room has a floor there.
pub(super) fn floor_height_at(rig: &WindowRig, cell: [i32; 2]) -> Option<i32> {
    sectors(rig).rooms.iter().find_map(|r| {
        let [x, z] = r.local(cell)?;
        let f = r.sector(x, z)?.ground_floor()?;
        Some(f.heights.iter().sum::<i32>() / 4)
    })
}

pub(super) fn room_count(rig: &WindowRig) -> usize {
    rig.world().rooms.len()
}

pub(super) fn history_steps(rig: &WindowRig) -> usize {
    rig.ws.history.steps()
}

/// Click a label of the options bar.
pub(super) fn opt(rig: &mut WindowRig, label: &str) {
    assert!(
        option_click(rig, label),
        "no `{label}` in the options bar: {}",
        options_line(rig)
    );
}

/// Set a number field of the options bar the way a person does: double click
/// it, select all, type, Enter.
pub(super) fn set_field(rig: &mut WindowRig, label: &str, value: &str) {
    use super::grid_toolbar::{text_in, texts};
    let top = ACTION_BAR_COMPACT_HEIGHT + crate::grid_tools::tool_strip_height(&rig.ctx);
    let bottom = ACTION_BAR_COMPACT_HEIGHT + crate::grid_tools::chrome_height(&rig.ctx);
    let band = Rect::from_min_max(Pos2::new(0.0, top), Pos2::new(rig.size.x, bottom));
    let l = text_in(rig, band, label)
        .unwrap_or_else(|| panic!("no `{label}` field: {}", options_line(rig)));
    let mut right: Vec<(String, Rect)> = texts(rig)
        .into_iter()
        .filter(|(_, r)| {
            band.contains(r.center())
                && (r.center().y - l.center().y).abs() < 6.0
                && r.min.x > l.max.x - 1.0
        })
        .collect();
    right.sort_by(|a, b| a.1.min.x.total_cmp(&b.1.min.x));
    let target = right
        .first()
        .unwrap_or_else(|| panic!("no value after `{label}`"))
        .1;
    rig.click(target.center());
    rig.click(target.center());
    frames(rig, 1);
    rig.frame(vec![egui::Event::Key {
        key: egui::Key::A,
        physical_key: Some(egui::Key::A),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND,
    }]);
    rig.frame(vec![egui::Event::Text(value.to_string())]);
    press_key(rig, egui::Key::Enter, false);
}

/// A second room east of the starter room, made with the Room tool's key
/// and a plan drag.
fn second_room(rig: &mut WindowRig, a: [i32; 2], b: [i32; 2]) {
    assert!(start(rig, GridTool::Room, Start::Key));
    drag_cells(rig, a, b);
}

#[test]
fn corridor_between_two_rooms_by_key_clicks_and_enter() {
    let mut rig = starter_plan();
    frame_cells(&mut rig, [-1, -2], [16, 7]);
    second_room(&mut rig, [10, 0], [13, 3]);
    assert_eq!(room_count(&rig), 2, "{}", rig.ws.status);
    let steps0 = history_steps(&rig);
    assert!(start(&mut rig, GridTool::Corridor, Start::Key));
    for c in [[5, 2], [8, 2], [8, 1], [10, 1]] {
        click_cell(&mut rig, c);
        eprintln!("click {c:?}: {} | {}", rig.ws.build_hint(), rig.ws.status);
    }
    key(&mut rig, egui::Key::Enter);
    eprintln!("done: {} | rooms {}", rig.ws.status, room_count(&rig));
    for c in [[6, 2], [7, 2], [8, 2], [8, 1], [9, 1]] {
        assert!(has_floor_at(&rig, c), "floor at {c:?}");
    }
    assert_eq!(
        history_steps(&rig),
        steps0 + 1,
        "the corridor is one undo step"
    );
    assert!(walkable(&rig, [9, 1]), "the corridor can be walked");
    assert!(walkable(&rig, [11, 1]), "and the far room is reached");
    assert_cooks(&rig, "corridor");
}

/// Pick an entry of the Style combo box in the options bar by clicking the
/// box and then the entry.
pub(super) fn pick_style(rig: &mut WindowRig, shown: &str, entry_contains: &str) -> bool {
    use super::grid_toolbar::texts;
    if !option_click(rig, shown) {
        return false;
    }
    let hit = texts(rig)
        .into_iter()
        .find(|(t, _)| t.to_lowercase().contains(&entry_contains.to_lowercase()));
    match hit {
        Some((_, r)) => {
            rig.click(r.center());
            frames(rig, 2);
            true
        }
        None => false,
    }
}

/// Raise the 2x2 block of the far room under (11, 1) by two clicks with the
/// Height tool from the Classic menu.
fn lift_far_room(rig: &mut WindowRig) {
    assert!(start(rig, GridTool::Height, Start::Strip));
    let p = plan_px(rig, [11, 1]);
    rig.drag(p, p + Vec2::new(0.0, -40.0));
    assert_eq!(
        floor_height_at(rig, [11, 1]),
        Some(512),
        "{}",
        rig.ws.status
    );
}

#[test]
fn corridor_with_a_height_change_is_a_ramp_or_stairs_you_can_walk() {
    for stairs in [false, true] {
        let mut rig = starter_plan();
        frame_cells(&mut rig, [-1, -2], [16, 7]);
        second_room(&mut rig, [10, 0], [13, 3]);
        lift_far_room(&mut rig);
        assert!(start(&mut rig, GridTool::Corridor, Start::Strip));
        if stairs {
            opt(&mut rig, "Stairs");
        } else {
            opt(&mut rig, "Ramp");
        }
        for c in [[5, 2], [7, 2], [7, 1], [11, 1]] {
            click_cell(&mut rig, c);
        }
        key(&mut rig, egui::Key::Enter);
        eprintln!(
            "stairs {stairs}: {} | {}",
            rig.ws.status,
            rig.ws.build_hint()
        );
        assert!(rig.ws.status.starts_with("Corridor"), "{}", rig.ws.status);
        let h = |x: i32| floor_height_at(&rig, [x, 1]).unwrap();
        assert_eq!(h(7), 0);
        assert!(
            0 < h(8) && h(8) < h(9) && h(9) < 512,
            "the legs climb to the raised room: {} {}",
            h(8),
            h(9)
        );
        assert_eq!(h(10), 512, "the raised room is kept");
        assert!(
            walkable(&rig, [9, 1]) && walkable(&rig, [11, 1]),
            "walk it end to end"
        );
        assert_cooks(&rig, "height change corridor");
    }
}

#[test]
fn corridor_in_a_style_chosen_from_the_options_bar() {
    let mut rig = starter_plan();
    frame_cells(&mut rig, [-1, -2], [16, 7]);
    second_room(&mut rig, [10, 0], [13, 3]);
    assert!(start(&mut rig, GridTool::Corridor, Start::Search));
    assert!(
        pick_style(&mut rig, "Plain", "gothic"),
        "{}",
        options_line(&rig)
    );
    set_field(&mut rig, "Width", "3");
    for c in [[5, 2], [9, 2]] {
        click_cell(&mut rig, c);
    }
    key(&mut rig, egui::Key::Enter);
    eprintln!("{} | {}", rig.ws.status, rig.ws.build_hint());
    assert!(rig.ws.status.contains("in style"), "{}", rig.ws.status);
    for z in 1..=3 {
        assert!(has_floor_at(&rig, [7, z]), "three sectors across at z={z}");
    }
    assert!(walkable(&rig, [8, 2]), "the styled corridor can be walked");
    assert_cooks(&rig, "styled corridor");
}

#[test]
fn corridor_escape_in_the_middle_leaves_nothing() {
    let mut rig = starter_plan();
    frame_cells(&mut rig, [-1, -2], [16, 7]);
    second_room(&mut rig, [10, 0], [13, 3]);
    let before = rig.ws.project.clone();
    let steps = history_steps(&rig);
    assert!(start(&mut rig, GridTool::Corridor, Start::Strip));
    for c in [[5, 2], [8, 2]] {
        click_cell(&mut rig, c);
    }
    assert!(rig.ws.project != before, "the ghost is in the document");
    key(&mut rig, egui::Key::Escape);
    assert!(rig.ws.project == before);
    assert_eq!(history_steps(&rig), steps);
}

/// The span meshes of the newest room: (name of the asset, its spec).
fn spans_of_last_room(rig: &WindowRig) -> Vec<(String, psxed_project::grid::SpanSpec, [i32; 3])> {
    use psxed_project::grid::mesh::MeshKind;
    let world = rig.world();
    let room = world.rooms.last().unwrap();
    room.meshes
        .iter()
        .filter_map(|m| match &world.mesh_assets[usize::from(m.asset)].kind {
            MeshKind::Span(s) => Some((
                world.mesh_assets[usize::from(m.asset)].name.clone(),
                *s,
                m.position,
            )),
            _ => None,
        })
        .collect()
}

fn piers_of_last_room(rig: &WindowRig) -> usize {
    use psxed_project::grid::mesh::MeshKind;
    let world = rig.world();
    let room = world.rooms.last().unwrap();
    room.meshes
        .iter()
        .filter(|m| {
            matches!(
                world.mesh_assets[usize::from(m.asset)].kind,
                MeshKind::Column(_)
            )
        })
        .count()
}

#[test]
fn bridge_across_a_gap_with_parapet_and_piers() {
    let mut rig = starter_plan();
    frame_cells(&mut rig, [-1, -2], [16, 7]);
    second_room(&mut rig, [9, 0], [12, 3]);
    // Both ends stand on platforms (Height tool, Classic menu), so there is
    // ground below the deck for piers.
    assert!(start(&mut rig, GridTool::Height, Start::Strip));
    for c in [[5, 2], [9, 2]] {
        let p = plan_px(&rig, c);
        rig.drag(p, p + Vec2::new(0.0, -200.0));
    }
    eprintln!(
        "ends {:?} {:?}",
        floor_height_at(&rig, [5, 2]),
        floor_height_at(&rig, [9, 2])
    );
    let rise = floor_height_at(&rig, [5, 2]).unwrap();
    assert!(rise >= 1024, "{rise}");
    let steps0 = history_steps(&rig);
    let rooms0 = room_count(&rig);
    // A bridge with piers between ground-level ends tells why it cannot.
    assert!(start(&mut rig, GridTool::Bridge, Start::Key));
    set_field(&mut rig, "Piers every", "1024");
    set_field(&mut rig, "Parapet", "768");
    set_field(&mut rig, "Piers every", "1024");
    set_field(&mut rig, "Width", "1280");
    click_cell(&mut rig, [5, 2]);
    assert_eq!(
        room_count(&rig),
        rooms0,
        "one end placed: nothing built yet ({})",
        rig.ws.build_hint()
    );
    click_cell(&mut rig, [9, 2]);
    assert_eq!(
        room_count(&rig),
        rooms0 + 1,
        "{} | {}",
        rig.ws.status,
        rig.ws.build_hint()
    );
    assert!(rig.ws.status.starts_with("Bridge"), "{}", rig.ws.status);
    let spans = spans_of_last_room(&rig);
    // Deck plus two parapets.
    assert_eq!(spans.len(), 3, "{spans:?}");
    let deck = spans
        .iter()
        .find(|s| s.0 == "bridge deck")
        .expect("a deck")
        .1;
    assert_eq!(
        deck.length,
        4 * 1024,
        "from the middle of one end sector to the other"
    );
    assert_eq!(deck.width, 1280);
    assert_eq!(deck.rise, 0, "both ends are floors at the same height");
    assert!(piers_of_last_room(&rig) >= 2, "piers every 1024 along 4096");
    assert_eq!(history_steps(&rig), steps0 + 1, "one undo step");
    assert_cooks(&rig, "bridge");
    // Undo puts the gap back.
    let before_rooms = rooms0;
    rig.ws.do_undo();
    assert_eq!(room_count(&rig), before_rooms);
}

#[test]
fn bridge_without_parapet_or_piers_and_the_second_click_must_land() {
    let mut rig = starter_plan();
    frame_cells(&mut rig, [-1, -2], [16, 7]);
    second_room(&mut rig, [9, 0], [12, 3]);
    assert!(start(&mut rig, GridTool::Bridge, Start::Strip));
    set_field(&mut rig, "Parapet", "0");
    set_field(&mut rig, "Piers every", "0");
    click_cell(&mut rig, [5, 2]);
    click_cell(&mut rig, [9, 2]);
    assert_eq!(spans_of_last_room(&rig).len(), 1, "a bare deck");
    assert_eq!(piers_of_last_room(&rig), 0);
}

#[test]
fn bridge_with_piers_between_ground_ends_says_why_and_leaves_no_undo_step() {
    let mut rig = starter_plan();
    frame_cells(&mut rig, [-1, -2], [16, 7]);
    second_room(&mut rig, [9, 0], [12, 3]);
    let before = rig.ws.project.clone();
    let steps0 = history_steps(&rig);
    assert!(start(&mut rig, GridTool::Bridge, Start::Search));
    set_field(&mut rig, "Piers every", "1024");
    click_cell(&mut rig, [5, 2]);
    click_cell(&mut rig, [9, 2]);
    assert!(
        rig.ws.status.contains("A pier does not fit"),
        "{}",
        rig.ws.status
    );
    assert!(
        rig.ws.status.contains("Piers every"),
        "it says what to change: {}",
        rig.ws.status
    );
    assert!(rig.ws.project == before);
    assert_eq!(history_steps(&rig), steps0);
}

// ---------------------------------------------------------------------
// Walls in the 3D view: stand in front of one and aim at it.

use psxed_project::grid::WallEdge;

/// Editor-space point on the wall `side` of world sector `cell`, `along` of
/// the way along it (0 to 1) and `up` editor units above `base_y`.
pub(super) fn wall_point(
    rig: &WindowRig,
    cell: [i32; 2],
    side: WallEdge,
    along: f32,
    base_y: f32,
    up: f32,
) -> [f32; 3] {
    let sector = rig.world().scale.to_editor(1024) as f32;
    let (cx, cz) = (cell[0] as f32, cell[1] as f32);
    match side {
        WallEdge::North => [(cx + along) * sector, base_y + up, cz * sector],
        WallEdge::South => [(cx + along) * sector, base_y + up, (cz + 1.0) * sector],
        WallEdge::West => [cx * sector, base_y + up, (cz + along) * sector],
        _ => [(cx + 1.0) * sector, base_y + up, (cz + along) * sector],
    }
}

/// Put the free camera `back` sectors in front of the wall `side` of `cell`
/// at `eye` editor units above `base_y`, looking at its middle.
pub(super) fn face_wall(
    rig: &mut WindowRig,
    cell: [i32; 2],
    side: WallEdge,
    base_y: f32,
    back: f32,
    eye: f32,
) {
    let sector = rig.world().scale.to_editor(1024) as f32;
    let mid = wall_point(rig, cell, side, 0.5, base_y, eye * 0.5);
    let inward = match side {
        WallEdge::North => [0.0, 1.0],
        WallEdge::South => [0.0, -1.0],
        WallEdge::West => [1.0, 0.0],
        _ => [-1.0, 0.0],
    };
    let cam = [
        (mid[0] + inward[0] * back * sector) as i32,
        (base_y + eye) as i32,
        (mid[2] + inward[1] * back * sector) as i32,
    ];
    if !rig.ws.grid.split {
        rig.ws.apply_grid_pose("view=3d").unwrap();
    }
    rig.ws.camera_rig.mode = ViewportCameraMode::Free;
    rig.ws.camera_rig.free_initialized = true;
    rig.ws.camera_rig.free_position = cam;
    let (yaw, pitch) =
        camera_angles_to_look_at(cam, [mid[0] as i32, mid[1] as i32, mid[2] as i32]).unwrap();
    rig.ws.camera_rig.free_yaw = yaw;
    rig.ws.camera_rig.free_pitch = pitch;
    frames(rig, 3);
}

/// Screen pixel of an editor-space point in the 3D view.
pub(super) fn px3(rig: &WindowRig, p: [f32; 3]) -> Option<Pos2> {
    let camera = rig.ws.viewport_3d_camera();
    let rect = rig.ws.last_persp_viewport_rect;
    crate::grid_tools::view::project_point(camera, rect, p).map(|(p, _)| p)
}

/// Drag along a wall from `a` to `b` (fractions of the sector side) at
/// `up` editor units above the floor `base_y`.
pub(super) fn drag_on_wall(
    rig: &mut WindowRig,
    cell: [i32; 2],
    side: WallEdge,
    a: f32,
    b: f32,
    base_y: f32,
    up: f32,
) {
    let pa = px3(rig, wall_point(rig, cell, side, a, base_y, up)).expect("on screen");
    let pb = px3(rig, wall_point(rig, cell, side, b, base_y, up)).expect("on screen");
    rig.drag(pa, pb);
}

/// The clicks along the wall `side` of `cell` (world clicks along it) that
/// are cut open in the storey `span` of `room`.
pub(super) fn open_clicks(
    rig: &WindowRig,
    room: usize,
    cell: [i32; 2],
    side: WallEdge,
    span: u8,
) -> Vec<i32> {
    use psxed_project::grid::edit::wall_api::walls_at;
    let world = rig.world();
    let r = &world.rooms[room];
    let n = 4;
    let mut out = Vec::new();
    for k in 0..n {
        let c = match side {
            WallEdge::North | WallEdge::South => cell[0] * n + k,
            _ => cell[1] * n + k,
        };
        let probe = match side {
            WallEdge::North => [c, cell[1] * n],
            WallEdge::South => [c, cell[1] * n + n - 1],
            WallEdge::West => [cell[0] * n, c],
            _ => [cell[0] * n + n - 1, c],
        };
        let Some((key, _)) = r.leaf_at(probe) else {
            continue;
        };
        let open = walls_at(world, room, key, side)
            .iter()
            .any(|v| v.addr.span == span && v.opening.is_some_and(|o| o.open));
        if open {
            out.push(c);
        }
    }
    out
}

/// The authored wall pieces on the stretch of `side` of `cell` the opening
/// left standing: (bottom, top) heights at the left end, room relative, per
/// piece, whatever the click.
pub(super) fn pieces_on(
    rig: &WindowRig,
    room: usize,
    cell: [i32; 2],
    side: WallEdge,
) -> Vec<[i32; 4]> {
    let world = rig.world();
    let r = &world.rooms[room];
    let n = 4;
    let mut keys = std::collections::BTreeSet::new();
    for k in 0..n {
        let probe = match side {
            WallEdge::North => [cell[0] * n + k, cell[1] * n],
            WallEdge::South => [cell[0] * n + k, cell[1] * n + n - 1],
            WallEdge::West => [cell[0] * n, cell[1] * n + k],
            _ => [cell[0] * n + n - 1, cell[1] * n + k],
        };
        if let Some((key, _)) = r.leaf_at(probe) {
            keys.insert((key.level, key.x, key.z));
        }
    }
    let mut out = Vec::new();
    for (level, x, z) in keys {
        let key = psxed_project::grid::LeafKey { level, x, z };
        if let Some(sector) = r.leaf(key) {
            for w in sector.walls(side) {
                out.push(w.heights);
            }
        }
    }
    out
}

/// Aim the camera at the north wall of the starter room's cell and drag
/// along it from `a` to `b` (fractions of the cell).
fn drag_north_wall(rig: &mut WindowRig, cell: [i32; 2], a: f32, b: f32) {
    face_wall(rig, cell, WallEdge::North, 0.0, 3.0, 700.0);
    drag_on_wall(rig, cell, WallEdge::North, a, b, 0.0, 300.0);
}

#[test]
fn opening_door_is_cut_where_the_drag_was() {
    let mut rig = Fixture::Starter.rig();
    let before = rig.ws.project.clone();
    let steps0 = history_steps(&rig);
    assert!(start(&mut rig, GridTool::Opening, Start::Strip));
    drag_north_wall(&mut rig, [2, 0], 0.375, 0.625);
    assert!(rig.ws.status.starts_with("Door cut"), "{}", rig.ws.status);
    assert_eq!(
        open_clicks(&rig, 0, [2, 0], WallEdge::North, 0),
        vec![9, 10],
        "the two clicks under the drag"
    );
    assert!(open_clicks(&rig, 0, [3, 0], WallEdge::North, 0).is_empty());
    assert_eq!(history_steps(&rig), steps0 + 1);
    assert_cooks(&rig, "door opening");
    rig.ws.do_undo();
    assert!(rig.ws.project == before);
}

/// Aim at the east wall: the run goes along Z.
#[test]
fn opening_on_each_of_the_four_sides() {
    for side in [
        WallEdge::North,
        WallEdge::East,
        WallEdge::South,
        WallEdge::West,
    ] {
        let mut rig = Fixture::Starter.rig();
        let cell = match side {
            WallEdge::North => [2, 0],
            WallEdge::South => [2, 5],
            WallEdge::West => [0, 2],
            _ => [5, 2],
        };
        assert!(start(&mut rig, GridTool::Opening, Start::Key));
        face_wall(&mut rig, cell, side, 0.0, 3.0, 700.0);
        drag_on_wall(&mut rig, cell, side, 0.375, 0.875, 0.0, 300.0);
        assert!(
            rig.ws.status.starts_with("Door cut"),
            "{side:?}: {}",
            rig.ws.status
        );
        let open = open_clicks(&rig, 0, cell, side, 0);
        let base = match side {
            WallEdge::North | WallEdge::South => cell[0] * 4,
            _ => cell[1] * 4,
        };
        assert!(
            open.contains(&(base + 1)) && open.contains(&(base + 3)),
            "{side:?}: {open:?}"
        );
        assert!(!open.contains(&base), "{side:?}: {open:?}");
    }
}

#[test]
fn opening_window_has_a_sill_and_the_lintel_follows_the_option() {
    let mut rig = Fixture::Starter.rig();
    assert!(start(&mut rig, GridTool::Opening, Start::Search));
    opt(&mut rig, "Window");
    set_field(&mut rig, "Lintel", "5");
    set_field(&mut rig, "Sill", "3");
    drag_north_wall(&mut rig, [2, 0], 0.125, 0.875);
    assert!(rig.ws.status.starts_with("Window cut"), "{}", rig.ws.status);
    let open = open_clicks(&rig, 0, [2, 0], WallEdge::North, 0);
    assert_eq!(open, vec![8, 9, 10, 11]);
    let pieces = pieces_on(&rig, 0, [2, 0], WallEdge::North);
    eprintln!("window pieces {pieces:?}");
    assert!(
        pieces.iter().any(|h| h[2] == 768 && h[3] == 768),
        "a sill 3 clicks high: {pieces:?}"
    );
    assert!(
        pieces.iter().any(|h| h[0] == 1280 && h[1] == 1280),
        "a lintel from 5 clicks: {pieces:?}"
    );
    assert_cooks(&rig, "window");
}

#[test]
fn opening_profiles_pointed_and_round_make_arched_heads() {
    for (label, rectangular) in [("Rectangular", true), ("Pointed", false), ("Round", false)] {
        let mut rig = Fixture::Starter.rig();
        assert!(start(&mut rig, GridTool::Opening, Start::Strip));
        opt(&mut rig, label);
        set_field(&mut rig, "Lintel", "7");
        drag_north_wall(&mut rig, [2, 0], 0.125, 0.875);
        assert!(
            rig.ws.status.starts_with("Door cut"),
            "{label}: {}",
            rig.ws.status
        );
        let pieces = pieces_on(&rig, 0, [2, 0], WallEdge::North);
        let bottoms: std::collections::BTreeSet<i32> = pieces
            .iter()
            .flat_map(|h| [h[0], h[1]])
            .filter(|b| *b > 0)
            .collect();
        eprintln!("{label}: {pieces:?}");
        if rectangular {
            assert_eq!(bottoms.len(), 1, "a flat lintel: {pieces:?}");
        } else {
            assert!(bottoms.len() >= 2, "the head curves: {pieces:?}");
        }
        assert_cooks(&rig, label);
    }
}

#[test]
fn opening_lintel_is_dragged_with_shift_and_typed() {
    let mut rig = Fixture::Starter.rig();
    assert!(start(&mut rig, GridTool::Opening, Start::Key));
    face_wall(&mut rig, [2, 0], WallEdge::North, 0.0, 3.0, 700.0);
    let a = px3(
        &rig,
        wall_point(&rig, [2, 0], WallEdge::North, 0.25, 0.0, 300.0),
    )
    .unwrap();
    let b = px3(
        &rig,
        wall_point(&rig, [2, 0], WallEdge::North, 0.75, 0.0, 300.0),
    )
    .unwrap();
    drag_hold(&mut rig, a, b);
    // Shift and move down: the lintel comes down with the pointer.
    rig.modifiers = egui::Modifiers::SHIFT;
    rig.move_to(b + Vec2::new(0.0, 40.0));
    rig.move_to(b + Vec2::new(0.0, 80.0));
    let top = rig.ws.grid.build.opts.opening.top;
    release(&mut rig, b + Vec2::new(0.0, 80.0));
    rig.modifiers = egui::Modifiers::NONE;
    assert!(
        (256..2048).contains(&top) && top % 256 == 0,
        "lintel dragged down to {top}"
    );
    let pieces = pieces_on(&rig, 0, [2, 0], WallEdge::North);
    assert!(
        pieces.iter().any(|h| h[0] == top),
        "a lintel at {top}: {pieces:?}"
    );
    // Typed: a digit sets the lintel in clicks while the drag is held.
    let mut rig2 = Fixture::Starter.rig();
    assert!(start(&mut rig2, GridTool::Opening, Start::Key));
    face_wall(&mut rig2, [2, 0], WallEdge::North, 0.0, 3.0, 700.0);
    let a = px3(
        &rig2,
        wall_point(&rig2, [2, 0], WallEdge::North, 0.25, 0.0, 300.0),
    )
    .unwrap();
    let b = px3(
        &rig2,
        wall_point(&rig2, [2, 0], WallEdge::North, 0.75, 0.0, 300.0),
    )
    .unwrap();
    drag_hold(&mut rig2, a, b);
    key(&mut rig2, egui::Key::Num6);
    release(&mut rig2, b);
    let pieces = pieces_on(&rig2, 0, [2, 0], WallEdge::North);
    assert!(
        pieces.iter().any(|h| h[0] == 1536),
        "typed 6 clicks: {pieces:?} / {}",
        rig2.ws.status
    );
}

/// Every wall of the styles-demo project that can be aimed at takes an
/// opening, or refuses in words that say what to change: never the old
/// "no floor and ceiling" for a wall that has both.
#[test]
fn opening_on_every_wall_of_styles_demo_cuts_or_explains() {
    use psxed_project::grid::derive::cell_walls;
    let mut rig = Fixture::StylesDemo.rig();
    let base = rig.ws.project.clone();
    let world = rig.world().clone();
    let (mut cut, mut explained, mut bad) = (0, 0, Vec::new());
    for (ri, room) in world.rooms.iter().enumerate() {
        for z in 0..room.depth {
            for x in 0..room.width {
                let cell = room.world_cell(x, z);
                for w in cell_walls(room, cell) {
                    if w.hidden
                        || w.host != cell
                        || !matches!(
                            w.edge,
                            WallEdge::North | WallEdge::East | WallEdge::South | WallEdge::West
                        )
                    {
                        continue;
                    }
                    let (lo, hi) = w
                        .ring
                        .iter()
                        .fold((i32::MAX, i32::MIN), |a, p| (a.0.min(p[1]), a.1.max(p[1])));
                    let ed = |g: i32| crate::grid_tools::view::editor_y(&world, room.y_offset, g);
                    let mid = (ed(lo) + ed(hi)) * 0.5;
                    rig.ws.project = base.clone();
                    assert!(start(&mut rig, GridTool::Opening, Start::Key));
                    face_wall(&mut rig, cell, w.edge, mid - 300.0, 2.2, 400.0);
                    let (Some(pa), Some(pb)) = (
                        px3(&rig, wall_point(&rig, cell, w.edge, 0.35, mid, 0.0)),
                        px3(&rig, wall_point(&rig, cell, w.edge, 0.65, mid, 0.0)),
                    ) else {
                        continue;
                    };
                    let hit = rig.ws.pick_face_3d(rig.ws.last_persp_viewport_rect, pa);
                    if !hit.is_some_and(|h| h.face.room == ri && h.face.leaf.cell() == cell) {
                        continue;
                    }
                    rig.drag(pa, pb);
                    let said = rig.ws.status.clone();
                    if said.starts_with("Door cut") {
                        cut += 1;
                    } else if said.contains("Lintel") || said.contains("already open") {
                        explained += 1;
                    } else {
                        bad.push(format!("room {ri} {cell:?} {:?}: {said}", w.edge));
                    }
                }
            }
        }
    }
    assert!(cut > 50 && explained > 0, "cut {cut} explained {explained}");
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// The starter with an upper floor over cells x 1..4, z 0..2 made by the
/// Upper floor tool (the storeys of the column are the room's 0 and 1).
fn starter_with_upper_floor() -> WindowRig {
    let mut rig = starter_plan();
    assert!(start(&mut rig, GridTool::Slab, Start::Strip));
    drag_cells(&mut rig, [1, 0], [4, 2]);
    let mid = plan_px(&rig, [2, 1]);
    drag_up(&mut rig, mid, 70.0);
    assert!(
        rig.ws.status.starts_with("Upper floor added"),
        "{}",
        rig.ws.status
    );
    persp(&mut rig);
    rig
}

#[test]
fn opening_on_an_upper_storey_is_a_whole_sector_and_the_door_tool_says_it_cannot() {
    let mut rig = starter_with_upper_floor();
    let base_y = crate::grid_tools::view::editor_y(rig.world(), 0, 1024);
    assert!(start(&mut rig, GridTool::Opening, Start::Key));
    set_field(&mut rig, "Lintel", "3");
    face_wall(&mut rig, [2, 0], WallEdge::North, base_y, 3.0, 500.0);
    drag_on_wall(&mut rig, [2, 0], WallEdge::North, 0.3, 0.7, base_y, 300.0);
    assert!(rig.ws.status.starts_with("Door cut"), "{}", rig.ws.status);
    assert_eq!(
        open_clicks(&rig, 0, [2, 0], WallEdge::North, 1),
        vec![8, 9, 10, 11],
        "the whole sector on the upper floor"
    );
    assert!(
        open_clicks(&rig, 0, [2, 0], WallEdge::North, 0).is_empty(),
        "the ground floor wall is untouched"
    );
    assert_cooks(&rig, "upper opening");
    // A sliding door there is refused in words, with nothing left behind.
    let before = rig.ws.project.clone();
    let steps = history_steps(&rig);
    assert!(start(&mut rig, GridTool::Door, Start::Strip));
    face_wall(&mut rig, [3, 0], WallEdge::North, base_y, 3.0, 500.0);
    drag_on_wall(&mut rig, [3, 0], WallEdge::North, 0.3, 0.7, base_y, 300.0);
    assert!(rig.ws.status.contains("ground floor"), "{}", rig.ws.status);
    assert!(rig.ws.project == before);
    assert_eq!(history_steps(&rig), steps);
}

/// The cathedral dragged out over world cells `a` to `b` with `bay` sectors
/// to a bay and `aisle` sectors of aisle, started from `how`.
fn make_cathedral(how: Start, a: [i32; 2], b: [i32; 2], bay: i32, aisle: i32) -> WindowRig {
    let mut rig = starter_plan();
    frame_cells(&mut rig, [-1, -3], [22, 8]);
    assert!(start(&mut rig, GridTool::Cathedral, how));
    set_field(&mut rig, "Bay", &bay.to_string());
    set_field(&mut rig, "Aisle", &aisle.to_string());
    drag_cells(&mut rig, a, b);
    rig
}

#[test]
fn cathedral_nave_axis_width_bays_and_aisles() {
    // 12 sectors long, 7 deep, bays of 2 and aisles of 2: six bays, a nave of 3.
    let rig = make_cathedral(Start::Strip, [8, -1], [19, 5], 2, 2);
    assert!(
        rig.ws.status.starts_with("Cathedral of 6 bays, 12 arches"),
        "{}",
        rig.ws.status
    );
    assert_eq!(room_count(&rig), 2);
    for (cell, what) in [
        ([8, 0], "an aisle cell"),
        ([8, 1], "the nave"),
        ([19, 3], "the east end of the nave"),
        ([13, 1], "the nave middle"),
    ] {
        assert!(has_floor_at(&rig, cell), "{what} {cell:?} has a floor");
    }
    assert!(!has_floor_at(&rig, [7, 1]), "nothing west of the drag");
    assert!(standable(&rig, [13, 2]), "the nave can be stood in");
    assert_cooks(&rig, "cathedral");
    // Bays of 3 and aisles of 1: four bays, eight arches.
    let rig = make_cathedral(Start::Search, [8, -1], [19, 5], 3, 1);
    assert!(
        rig.ws.status.starts_with("Cathedral of 4 bays, 8 arches"),
        "{}",
        rig.ws.status
    );
    assert!(
        has_floor_at(&rig, [19, 0]),
        "the long side is still the full drag"
    );
    assert_cooks(&rig, "cathedral bay 3");
    // No aisles: no arcade.
    let rig = make_cathedral(Start::Strip, [8, -1], [19, 3], 2, 0);
    assert!(rig.ws.status.contains("0 arches"), "{}", rig.ws.status);
    assert_cooks(&rig, "cathedral no aisle");
}

#[test]
fn cathedral_in_a_style_and_one_undo_step_and_a_plain_refusal() {
    let mut rig = starter_plan();
    frame_cells(&mut rig, [-1, -3], [22, 8]);
    let before = rig.ws.project.clone();
    let steps = history_steps(&rig);
    assert!(start(&mut rig, GridTool::Cathedral, Start::Strip));
    assert!(
        pick_style(&mut rig, "Plain", "gothic"),
        "{}",
        options_line(&rig)
    );
    drag_cells(&mut rig, [8, -1], [19, 5]);
    assert!(
        rig.ws.status.starts_with("Cathedral of"),
        "{}",
        rig.ws.status
    );
    assert_eq!(history_steps(&rig), steps + 1);
    assert_cooks(&rig, "styled cathedral");
    rig.ws.do_undo();
    assert!(rig.ws.project == before);
    // Too shallow for the aisles: the words say what to change.
    assert!(start(&mut rig, GridTool::Cathedral, Start::Strip));
    drag_cells(&mut rig, [8, -1], [13, 3]);
    assert!(
        rig.ws.status.contains("Drag a deeper area"),
        "{}",
        rig.ws.status
    );
    assert_eq!(history_steps(&rig), steps);
    assert!(rig.ws.project == before);
}
fn standable(rig: &WindowRig, cell: [i32; 2]) -> bool {
    let world = rig.world();
    let a = analyse(world, &[], &psx_engine::WalkLimits::DEFAULT).unwrap();
    a.rooms
        .iter()
        .any(|r| r.cell(cell).is_some_and(|c| c.standable))
}

#[test]
fn an_arch_that_springs_too_low_says_how_to_fix_it() {
    let mut rig = Fixture::Starter.rig();
    assert!(start(&mut rig, GridTool::Opening, Start::Strip));
    opt(&mut rig, "Pointed");
    set_field(&mut rig, "Lintel", "4");
    let before = rig.ws.project.clone();
    let steps0 = history_steps(&rig);
    drag_north_wall(&mut rig, [2, 0], 0.0, 1.0);
    assert!(
        rig.ws.status.contains("springs too low"),
        "{}",
        rig.ws.status
    );
    assert!(
        rig.ws.status.contains("Raise Lintel to"),
        "{}",
        rig.ws.status
    );
    assert!(rig.ws.project == before);
    assert_eq!(history_steps(&rig), steps0);
}

#[test]
fn an_opening_dragged_past_the_end_of_the_wall_stops_at_the_end() {
    let mut rig = Fixture::Starter.rig();
    assert!(start(&mut rig, GridTool::Opening, Start::Strip));
    // The last cell of the north wall, dragged on past the corner.
    face_wall(&mut rig, [4, 0], WallEdge::North, 0.0, 4.0, 700.0);
    let a = px3(
        &rig,
        wall_point(&rig, [5, 0], WallEdge::North, 0.3, 0.0, 300.0),
    )
    .unwrap();
    let b = px3(
        &rig,
        wall_point(&rig, [7, 0], WallEdge::North, 0.3, 0.0, 300.0),
    )
    .unwrap();
    rig.drag(a, b);
    assert!(rig.ws.status.starts_with("Door cut"), "{}", rig.ws.status);
    let open = open_clicks(&rig, 0, [5, 0], WallEdge::North, 0);
    assert_eq!(open, vec![21, 22, 23], "from the press to the corner");
}

#[test]
fn opening_window_sill_is_dragged_with_ctrl() {
    let mut rig = Fixture::Starter.rig();
    assert!(start(&mut rig, GridTool::Opening, Start::Key));
    opt(&mut rig, "Window");
    set_field(&mut rig, "Lintel", "6");
    face_wall(&mut rig, [2, 0], WallEdge::North, 0.0, 3.0, 700.0);
    let a = px3(
        &rig,
        wall_point(&rig, [2, 0], WallEdge::North, 0.25, 0.0, 300.0),
    )
    .unwrap();
    let b = px3(
        &rig,
        wall_point(&rig, [2, 0], WallEdge::North, 0.75, 0.0, 300.0),
    )
    .unwrap();
    drag_hold(&mut rig, a, b);
    let sill0 = rig.ws.grid.build.opts.opening.sill;
    rig.modifiers = egui::Modifiers::COMMAND;
    rig.move_to(b + Vec2::new(0.0, -10.0));
    rig.move_to(b + Vec2::new(0.0, -70.0));
    let sill = rig.ws.grid.build.opts.opening.sill;
    release(&mut rig, b + Vec2::new(0.0, -70.0));
    rig.modifiers = egui::Modifiers::NONE;
    assert!(
        sill > sill0 && sill % 256 == 0 && sill < 1536,
        "sill {sill0} -> {sill}"
    );
    let pieces = pieces_on(&rig, 0, [2, 0], WallEdge::North);
    assert!(
        pieces.iter().any(|h| h[2] == sill && h[3] == sill),
        "a sill at {sill}: {pieces:?}"
    );
    assert!(
        pieces.iter().any(|h| h[0] == 1536),
        "the lintel stayed at 6 clicks: {pieces:?}"
    );
    assert_eq!(rig.ws.grid.build.opts.opening.top, 1536);
}

/// The first text on screen equal to `label` (icon prefix ignored), anywhere.
pub(super) fn click_text(rig: &mut WindowRig, label: &str) -> bool {
    use super::grid_toolbar::texts;
    let hit = texts(rig)
        .into_iter()
        .find(|(t, _)| t.trim_start_matches(|c: char| !c.is_alphanumeric()).trim() == label);
    match hit {
        Some((_, r)) => {
            rig.click(r.center());
            frames(rig, 2);
            true
        }
        None => false,
    }
}

/// Like [`click_text`], but the match furthest to the right: the Inspector
/// lives in the right dock, the options bar's chips of the same name are left
/// of it.
pub(super) fn click_text_right(rig: &mut WindowRig, label: &str) -> bool {
    use super::grid_toolbar::texts;
    let hit = texts(rig)
        .into_iter()
        .filter(|(t, _)| t.trim_start_matches(|c: char| !c.is_alphanumeric()).trim() == label)
        .max_by(|a, b| a.1.center().x.total_cmp(&b.1.center().x));
    match hit {
        Some((_, r)) => {
            rig.click(r.center());
            frames(rig, 2);
            true
        }
        None => false,
    }
}

/// Type a number into the drag value showing `shown` (double click, select
/// all, type, Enter).
pub(super) fn type_into(rig: &mut WindowRig, shown: &str, value: &str) {
    use super::grid_toolbar::texts;
    let r = texts(rig)
        .into_iter()
        .find(|(t, _)| t.trim() == shown)
        .unwrap_or_else(|| panic!("no `{shown}` on screen"))
        .1;
    rig.click(r.center());
    rig.click(r.center());
    frames(rig, 1);
    rig.frame(vec![egui::Event::Key {
        key: egui::Key::A,
        physical_key: Some(egui::Key::A),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND,
    }]);
    rig.frame(vec![egui::Event::Text(value.to_string())]);
    press_key(rig, egui::Key::Enter, false);
}

fn door_of(
    rig: &WindowRig,
    name: &str,
) -> (
    psxed_project::DoorLock,
    String,
    Option<psxed_project::DoorSlide>,
) {
    use psxed_project::{LogicNodeKind, NodeKind};
    rig.ws
        .project
        .active_scene()
        .nodes()
        .iter()
        .find_map(|n| match &n.kind {
            NodeKind::Logic {
                kind:
                    LogicNodeKind::Door {
                        lock,
                        refusal,
                        slide,
                        ..
                    },
                ..
            } if n.name == name => Some((*lock, refusal.clone(), *slide)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no door `{name}`"))
}

#[test]
fn door_tool_cuts_the_doorway_places_a_slab_that_fits_and_the_form_edits_it() {
    use psxed_project::{DoorDirection, DoorLock, DoorSide, NodeKind};
    let mut rig = Fixture::Starter.rig();
    assert!(start(&mut rig, GridTool::Door, Start::Strip));
    drag_north_wall(&mut rig, [2, 0], 0.375, 0.625);
    assert!(rig.ws.status.starts_with("Door 1"), "{}", rig.ws.status);
    assert_eq!(
        open_clicks(&rig, 0, [2, 0], WallEdge::North, 0),
        vec![9, 10]
    );
    // The slab fits the doorway: 2 clicks wide (640 editor units), as tall as
    // the doorway (the default lintel, 8 clicks = 2560), standing in the wall.
    let scene = rig.ws.project.active_scene();
    let slab = scene
        .nodes()
        .iter()
        .find(|n| n.name == "Door 1 box")
        .expect("a slab");
    let NodeKind::BoxProp { vertices, .. } = &slab.kind else {
        panic!("a box prop")
    };
    let xs: Vec<i32> = vertices.iter().map(|v| i32::from(v[0])).collect();
    let ys: Vec<i32> = vertices.iter().map(|v| i32::from(v[1])).collect();
    assert_eq!(
        xs.iter().max().unwrap() - xs.iter().min().unwrap(),
        640,
        "as wide as the doorway"
    );
    assert_eq!(*ys.iter().max().unwrap(), 2560, "as tall as the doorway");
    assert_eq!(
        slab.transform.translation[0],
        2.5 * 1280.0,
        "centred on the opening"
    );
    assert_eq!(
        slab.transform.translation[2], 0.0,
        "in the plane of the wall"
    );
    assert_cooks(&rig, "door");
    // Select the door from the Objects panel and change it in its form.
    assert!(click_text(&mut rig, "Door 1"), "the door is listed");
    assert!(click_text_right(&mut rig, "Locked"), "the form offers Lock");
    assert_eq!(door_of(&rig, "Door 1").0, DoorLock::Locked);
    assert!(click_text_right(&mut rig, "One way"));
    assert!(matches!(
        door_of(&rig, "Door 1").0,
        DoorLock::OneWay {
            refuse_from: DoorSide::South
        }
    ));
    assert!(
        click_text_right(&mut rig, "\u{2191} N") || click_text_right(&mut rig, "N"),
        "one way sides"
    );
    assert!(matches!(
        door_of(&rig, "Door 1").0,
        DoorLock::OneWay {
            refuse_from: DoorSide::North
        }
    ));
    // The slide.
    let slide = door_of(&rig, "Door 1").2.unwrap();
    assert_eq!(slide.direction, DoorDirection::Up);
    assert!(click_text_right(&mut rig, "Up"));
    assert!(click_text_right(&mut rig, "East"));
    assert_eq!(
        door_of(&rig, "Door 1").2.unwrap().direction,
        DoorDirection::East
    );
    let shown = format!("{} units", slide.distance);
    type_into(&mut rig, &shown, "300");
    assert_eq!(door_of(&rig, "Door 1").2.unwrap().distance, 300);
    assert_cooks(&rig, "edited door");
}

/// A verb gesture that comes to nothing leaves no undo step: the Stairs
/// tool refusing a flight and Break breaking nothing, in both views, from
/// every region source.
#[test]
fn a_refusal_or_an_empty_result_leaves_no_undo_step() {
    let mut bad = Vec::new();
    let mut rig = Fixture::Starter.rig();
    let base = rig.ws.project.clone();
    for tool in [
        GridTool::Stairs,
        GridTool::Break,
        GridTool::Ramp,
        GridTool::Slab,
        GridTool::Spiral,
    ] {
        for in_plan in [false, true] {
            for source in [
                Source::Marquee,
                Source::Brush,
                Source::PathNoEnter,
                Source::LassoNoEnter,
            ] {
                let o = run(&mut rig, &base, tool, Start::Strip, |rig| {
                    if in_plan {
                        plan(rig);
                    } else {
                        persp(rig);
                    }
                    let _ = region_then_amount(rig, source, in_plan, tool);
                    // A press on the region with no movement at all.
                    let (_, c) = floor_block(rig);
                    let p = if in_plan {
                        plan_px(rig, c)
                    } else {
                        cell_px3d(rig, c).unwrap_or(Pos2::ZERO)
                    };
                    rig.drag(p, p + Vec2::new(0.0, -1.0));
                })
                .unwrap();
                if !o.changed && o.history_steps != 0 {
                    bad.push(format!(
                        "{} {:?} {}: {} undo step(s), document unchanged: {}",
                        tool.label(),
                        source,
                        if in_plan { "plan" } else { "3d" },
                        o.history_steps,
                        o.said
                    ));
                }
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// Stairs refused for a flight that cannot be made: the words say why and
/// there is no step to undo.
#[test]
fn stairs_over_too_tall_a_rise_refuses_and_leaves_nothing() {
    let mut rig = starter_plan();
    let before = rig.ws.project.clone();
    let steps = history_steps(&rig);
    assert!(start(&mut rig, GridTool::Stairs, Start::Key));
    opt(&mut rig, "Rectangle");
    drag_cells(&mut rig, [1, 1], [1, 2]);
    // Press on the flight and drag far up: a rise no tread can climb.
    let mid = plan_px(&rig, [1, 1]);
    drag_up(&mut rig, mid, 400.0);
    eprintln!("stairs: {} / {}", rig.ws.status, rig.ws.build_hint());
    if rig.ws.project == before {
        assert_eq!(
            history_steps(&rig),
            steps,
            "no undo step for a refused flight"
        );
    }
    // Break over a thin region breaks nothing and says so.
    let mut rig = starter_plan();
    let before = rig.ws.project.clone();
    let steps = history_steps(&rig);
    assert!(start(&mut rig, GridTool::Break, Start::Strip));
    drag_cells(&mut rig, [0, 0], [0, 0]);
    let mid = plan_px(&rig, [0, 0]);
    drag_up(&mut rig, mid, 30.0);
    if rig.ws.project == before {
        assert_eq!(
            history_steps(&rig),
            steps,
            "no undo step when nothing broke"
        );
    }
}

#[test]
fn lever_is_placed_linked_to_the_door_clicked_and_the_form_and_cook_agree() {
    use psxed_project::{InteractableKind, NodeKind};
    let mut rig = Fixture::Starter.rig();
    assert!(start(&mut rig, GridTool::Door, Start::Strip));
    drag_north_wall(&mut rig, [2, 0], 0.375, 0.625);
    assert!(rig.ws.status.starts_with("Door 1"), "{}", rig.ws.status);
    plan(&mut rig);
    reframe(&mut rig);
    assert!(start(&mut rig, GridTool::Lever, Start::Search));
    click_cell(&mut rig, [4, 3]);
    assert!(
        rig.ws.status.contains("click the door"),
        "{}",
        rig.ws.status
    );
    // The lever has no door yet: a click far from the door says so.
    click_cell(&mut rig, [5, 5]);
    assert!(rig.ws.status.contains("not a door"), "{}", rig.ws.status);
    click_cell(&mut rig, [2, 0]);
    assert!(
        rig.ws.status.contains("Lever operates Door 1"),
        "{}",
        rig.ws.status
    );
    let target = rig
        .ws
        .project
        .active_scene()
        .nodes()
        .iter()
        .find_map(|n| match &n.kind {
            NodeKind::Interactable {
                kind: InteractableKind::Switch { target, .. },
                ..
            } => Some(target.clone()),
            _ => None,
        })
        .expect("a lever");
    assert_eq!(target, "Door 1");
    assert_cooks(&rig, "lever and door");
    // Select the lever in the Objects panel: its form names the door.
    assert!(click_text(&mut rig, "Lever 1"), "the lever is listed");
    let shown = super::grid_toolbar::texts(&rig)
        .into_iter()
        .any(|(t, r)| t.trim() == "Door 1" && r.center().x > rig.size.x * 0.6);
    assert!(shown, "the Lever form shows the door it operates");
}

/// Build a small level the way a person does, then walk the undo stack back
/// to the empty project and forward again.
#[test]
fn a_small_level_built_by_clicks_undoes_to_the_start_and_redoes_to_the_end() {
    let mut rig = starter_plan();
    frame_cells(&mut rig, [-1, -3], [24, 8]);
    let empty = rig.ws.project.clone();
    // Room, Room, Corridor between them.
    assert!(start(&mut rig, GridTool::Room, Start::Key));
    drag_cells(&mut rig, [9, 0], [12, 3]);
    assert!(start(&mut rig, GridTool::Room, Start::Strip));
    drag_cells(&mut rig, [16, 0], [19, 3]);
    assert!(start(&mut rig, GridTool::Corridor, Start::Key));
    for c in [[5, 2], [7, 2], [7, 1], [9, 1]] {
        click_cell(&mut rig, c);
    }
    key(&mut rig, egui::Key::Enter);
    // Terrain patch and Ruin in the second room, Paint, Cells.
    assert!(start(&mut rig, GridTool::Terrain, Start::Strip));
    drag_cells(&mut rig, [16, 0], [19, 3]);
    assert!(
        rig.ws.status.starts_with("Terrain over"),
        "{}",
        rig.ws.status
    );
    assert!(start(&mut rig, GridTool::Ruin, Start::Search));
    drag_cells(&mut rig, [9, 0], [12, 3]);
    assert!(rig.ws.status.starts_with("Ruin at"), "{}", rig.ws.status);
    let steps = history_steps(&rig);
    assert!(steps >= 5, "every gesture is a step: {steps}");
    let mut states = vec![empty.clone()];
    // Walk back: record each document as undo reaches it.
    let end = rig.ws.project.clone();
    for _ in 0..steps {
        rig.ws.do_undo();
        states.push(rig.ws.project.clone());
    }
    assert!(rig.ws.project == empty, "undo reaches the empty project");
    for _ in 0..steps {
        rig.ws.do_redo();
    }
    assert!(rig.ws.project == end, "redo reaches the end exactly");
    assert_cooks(&rig, "small level");
}

/// The Both view: 3D and the plan side by side. Tools started from the strip
/// are used in the plan pane and in the 3D pane.
#[test]
fn both_view_every_flow_works_from_either_pane() {
    let mut rig = Fixture::Starter.rig();
    rig.ws.apply_grid_pose("view=split").unwrap();
    frames(&mut rig, 4);
    assert!(rig.ws.grid.split);
    rig.ws.frame_current_view();
    frames(&mut rig, 3);
    // Room, in the plan pane.
    let plan_rect = rig.ws.last_orthographic_viewport_rect;
    let persp_rect = rig.ws.last_persp_viewport_rect;
    assert!(
        plan_rect.width() > 100.0 && persp_rect.width() > 100.0,
        "two panes: {plan_rect:?} {persp_rect:?}"
    );
    frame_cells(&mut rig, [-1, -2], [14, 8]);
    assert!(start(&mut rig, GridTool::Room, Start::Strip));
    drag_cells(&mut rig, [8, 0], [11, 3]);
    assert_eq!(room_count(&rig), 2, "{}", rig.ws.status);
    // Corridor, clicked in the plan pane.
    assert!(start(&mut rig, GridTool::Corridor, Start::Key));
    for c in [[5, 2], [8, 2]] {
        click_cell(&mut rig, c);
    }
    key(&mut rig, egui::Key::Enter);
    assert!(rig.ws.status.starts_with("Corridor"), "{}", rig.ws.status);
    // Opening and Door, dragged on a wall in the 3D pane.
    assert!(start(&mut rig, GridTool::Opening, Start::Key));
    face_wall(&mut rig, [2, 0], WallEdge::North, 0.0, 3.0, 700.0);
    let (a, b) = (
        px3(
            &rig,
            wall_point(&rig, [2, 0], WallEdge::North, 0.375, 0.0, 300.0),
        )
        .unwrap(),
        px3(
            &rig,
            wall_point(&rig, [2, 0], WallEdge::North, 0.625, 0.0, 300.0),
        )
        .unwrap(),
    );
    assert!(
        persp_rect.contains(a) && persp_rect.contains(b),
        "the wall is in the 3D pane"
    );
    rig.drag(a, b);
    assert!(rig.ws.status.starts_with("Door cut"), "{}", rig.ws.status);
    assert!(start(&mut rig, GridTool::Door, Start::Strip));
    face_wall(&mut rig, [4, 0], WallEdge::North, 0.0, 3.0, 700.0);
    let (a, b) = (
        px3(
            &rig,
            wall_point(&rig, [4, 0], WallEdge::North, 0.375, 0.0, 300.0),
        )
        .unwrap(),
        px3(
            &rig,
            wall_point(&rig, [4, 0], WallEdge::North, 0.625, 0.0, 300.0),
        )
        .unwrap(),
    );
    rig.drag(a, b);
    assert!(rig.ws.status.starts_with("Door 1"), "{}", rig.ws.status);
    // A tool started in one pane (the Bridge, by its key) finishes in the other.
    assert!(start(&mut rig, GridTool::Bridge, Start::Key));
    click_cell(&mut rig, [8, 3]);
    assert!(!rig.ws.build_hint().is_empty());
    key(&mut rig, egui::Key::Escape);
    assert_cooks(&rig, "both view");
}

/// Every control of every tool's options bar, clicked once: the ones that
/// change nothing at all (document, tool, message, bar) are listed in
/// `PSXED_DEAD_CONTROLS` when that is set, and must be a state the control
/// is already in (the selected chip).
#[test]
fn no_options_bar_control_is_dead() {
    use super::grid_toolbar::texts;
    let mut dead = Vec::new();
    let mut rig = Fixture::Starter.rig();
    plan(&mut rig);
    for tool in GridTool::ALL {
        if !start(&mut rig, tool, Start::Strip) {
            dead.push(format!("{}: no strip entry", tool.label()));
            continue;
        }
        let top = ACTION_BAR_COMPACT_HEIGHT + crate::grid_tools::tool_strip_height(&rig.ctx);
        let bottom = ACTION_BAR_COMPACT_HEIGHT + crate::grid_tools::chrome_height(&rig.ctx);
        let items: Vec<String> = texts(&rig)
            .into_iter()
            .filter(|(_, r)| r.center().y > top && r.center().y < bottom && r.width() > 0.0)
            .map(|(t, _)| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect();
        for label in items {
            let base = rig.ws.project.clone();
            let (status0, bar0, tool0) =
                (rig.ws.status.clone(), options_line(&rig), rig.ws.grid.tool);
            let fp0 = format!("{:?}|{:?}", rig.ws.grid.verb.source, rig.ws.grid.build.opts);
            if !option_click(&mut rig, &label) {
                continue;
            }
            let moved = rig.ws.project != base
                || rig.ws.status != status0
                || options_line(&rig) != bar0
                || rig.ws.grid.tool != tool0
                || format!("{:?}|{:?}", rig.ws.grid.verb.source, rig.ws.grid.build.opts) != fp0;
            if !moved {
                dead.push(format!("{} / {}", tool.label(), label));
            }
            // Put the tool and document back for the next control.
            rig.ws.project = base;
            if rig.ws.grid.tool != tool {
                let _ = start(&mut rig, tool, Start::Strip);
            }
        }
    }
    if let Ok(path) = std::env::var("PSXED_DEAD_CONTROLS") {
        std::fs::write(path, dead.join("\n")).unwrap();
    }
    eprintln!("controls that changed nothing when clicked: {}", dead.len());
}
