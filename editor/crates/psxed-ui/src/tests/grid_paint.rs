//! The Paint tool: a preview of the paint under the pointer, and turning and
//! flipping it before it lands (Manny, 2026-10-20: "when we paint we should
//! show a preview of what the paint will be, and we should be able to rotate
//! the UV").
//!
//! The logic runs in the Top and 3D rigs (`GridRig`); the options bar and the
//! plan overlay run in the whole window (`WindowRig`).

use super::grid_live::WindowRig;
use super::grid_rig::*;
use super::*;
use crate::grid_tools::GridTool;
use psxed_project::face_uv::FaceUv;
use psxed_project::grid::edit::{edit_uv, FaceRef, UvOp};
use psxed_project::{MaterialResource, ResourceData, ResourceId};

fn shift() -> egui::Modifiers {
    egui::Modifiers::SHIFT
}

fn ctrl() -> egui::Modifiers {
    egui::Modifiers::COMMAND
}

fn material(rig: &mut GridRig, name: &str) -> ResourceId {
    rig.ws
        .project
        .add_resource(name, ResourceData::Material(MaterialResource::opaque(None)))
}

/// A 4 x 1 hall in the Top view, the Paint tool chosen, `brick` the paint.
fn painting() -> (GridRig, ResourceId) {
    let mut rig = GridRig::top("grid-paint-uv", flat_project(4, 1, false));
    let brick = material(&mut rig, "brick");
    rig.ws.grid.material = Some(brick);
    rig.tool(GridTool::Paint);
    (rig, brick)
}

fn floor_uv(rig: &GridRig, x: usize) -> FaceUv {
    rig.room(0)
        .sector(x, 0)
        .unwrap()
        .ground_floor()
        .unwrap()
        .tri_a
        .uv
}

fn floor_material(rig: &GridRig, x: usize) -> Option<ResourceId> {
    rig.room(0)
        .sector(x, 0)
        .unwrap()
        .ground_floor()
        .unwrap()
        .tri_a
        .material
}

/// The paint the preview is told about now, as the 3D preview reads it.
fn published(rig: &GridRig) -> Option<crate::PaintGhost> {
    rig.ws.publish_grid_view();
    crate::grid_preview_view().paint
}

// ---------------------------------------------------------------------
// Turning and flipping the paint.

#[test]
fn r_and_f_turn_and_flip_the_paint_and_a_click_stores_them_on_the_face() {
    let (mut rig, brick) = painting();
    // R: a quarter turn clockwise.
    rig.key(egui::Key::R);
    rig.click(rig.cell_centre(0, 0));
    assert_eq!(floor_material(&rig, 0), Some(brick));
    assert_eq!(floor_uv(&rig, 0).turns, 1);
    assert_eq!(floor_uv(&rig, 0).flip, [false, false]);
    // Shift+R twice: back to nothing, then a quarter counter-clockwise.
    rig.key_with(egui::Key::R, shift());
    rig.key_with(egui::Key::R, shift());
    rig.click(rig.cell_centre(1, 0));
    assert_eq!(floor_uv(&rig, 1).turns, 3);
    // F flips horizontally, on top of the turn the brush still has.
    rig.key(egui::Key::F);
    rig.click(rig.cell_centre(2, 0));
    assert_eq!(floor_uv(&rig, 2).turns, 3);
    assert_eq!(floor_uv(&rig, 2).flip, [true, false]);
    // Shift+F flips vertically.
    rig.key_with(egui::Key::F, shift());
    rig.click(rig.cell_centre(3, 0));
    assert_eq!(floor_uv(&rig, 3).flip, [true, true]);
    // The keys changed the brush, not the faces painted before.
    assert_eq!(floor_uv(&rig, 0).turns, 1);
    assert_eq!(floor_uv(&rig, 0).flip, [false, false]);
}

#[test]
fn the_turn_is_part_of_the_paints_one_undo_step() {
    let (mut rig, _) = painting();
    rig.key(egui::Key::R);
    rig.key(egui::Key::F);
    rig.click(rig.cell_centre(0, 0));
    assert_eq!(floor_uv(&rig, 0).turns, 1);
    rig.ws.do_undo();
    assert_eq!(floor_material(&rig, 0), None);
    assert!(floor_uv(&rig, 0).is_identity(), "{:?}", floor_uv(&rig, 0));
    assert!(!rig.ws.history.can_undo(), "one step, not two");
}

#[test]
fn a_stroke_gives_every_face_the_turn_and_is_one_undo_step() {
    let (mut rig, brick) = painting();
    rig.key(egui::Key::R);
    rig.key(egui::Key::R);
    rig.drag(rig.cell_centre(0, 0), rig.cell_centre(3, 0));
    for x in 0..4 {
        assert_eq!(floor_material(&rig, x), Some(brick), "x {x}");
        assert_eq!(floor_uv(&rig, x).turns, 2, "x {x}");
    }
    rig.ws.do_undo();
    for x in 0..4 {
        assert_eq!(floor_material(&rig, x), None, "x {x}");
        assert!(floor_uv(&rig, x).is_identity(), "x {x}");
    }
}

#[test]
fn a_plain_brush_leaves_the_turn_a_face_already_has() {
    let (mut rig, brick) = painting();
    let world = rig.ws.project.active_scene_mut().world.grid_mut().unwrap();
    edit_uv(world, FaceRef::floor(0, [1, 0]), None, UvOp::Turn(true));
    edit_uv(world, FaceRef::floor(0, [1, 0]), None, UvOp::Turn(true));
    rig.click(rig.cell_centre(1, 0));
    assert_eq!(floor_material(&rig, 1), Some(brick));
    assert_eq!(floor_uv(&rig, 1).turns, 2, "painting alone is not a reset");
}

#[test]
fn a_turn_on_a_triangle_lands_on_that_triangle_only() {
    let (mut rig, brick) = painting();
    rig.key(egui::Key::R);
    // Shift names the triangle under the pointer on a flat quad too.
    rig.click_with(rig.top_at(0.9 * S, 0.1 * S), shift());
    let floor = rig.room(0).sector(0, 0).unwrap().ground_floor().unwrap();
    let (a, b) = (floor.tri(0), floor.tri(1));
    assert_ne!(a.material, b.material, "{a:?} {b:?}");
    let painted = if a.material == Some(brick) { a } else { b };
    let other = if a.material == Some(brick) { b } else { a };
    assert_eq!(painted.uv.turns, 1);
    assert_eq!(other.uv.turns, 0);
}

#[test]
fn keys_during_a_drag_are_taken_and_ignored() {
    let (mut rig, _) = painting();
    rig.drag_hold(
        rig.cell_centre(0, 0),
        rig.cell_centre(2, 0),
        egui::Modifiers::NONE,
    );
    rig.key(egui::Key::R);
    rig.key(egui::Key::F);
    rig.release(rig.cell_centre(2, 0), egui::Modifiers::NONE);
    assert_eq!(rig.ws.grid.paint.orient, crate::PaintOrient::default());
    for x in 0..3 {
        assert!(floor_uv(&rig, x).is_identity(), "x {x}");
    }
    assert_eq!(rig.ws.grid.tool, GridTool::Paint, "F did not pick Floor");
}

#[test]
fn r_and_f_collide_with_no_other_binding_while_paint_is_active() {
    let (mut rig, _) = painting();
    // F flips the paint; it does not pick the Floor tool.
    rig.key(egui::Key::F);
    assert_eq!(rig.ws.grid.tool, GridTool::Paint);
    assert!(rig.ws.grid.paint.orient.flip[0]);
    // Shift+F is the vertical flip, not Floor either.
    rig.key_with(egui::Key::F, shift());
    assert_eq!(rig.ws.grid.tool, GridTool::Paint);
    assert_eq!(rig.ws.grid.paint.orient.flip, [true, true]);
    // The other keys keep their meaning: L links, U picks Raise.
    let link = rig.ws.grid.link;
    rig.key(egui::Key::L);
    assert_ne!(rig.ws.grid.link, link);
    rig.key(egui::Key::U);
    assert_eq!(rig.ws.grid.tool, GridTool::Extrude);
    // Away from Paint, F is the Floor tool again and R does not touch the
    // brush.
    rig.tool(GridTool::Select);
    let brush = rig.ws.grid.paint.orient;
    rig.key(egui::Key::R);
    assert_eq!(rig.ws.grid.paint.orient, brush);
    rig.key(egui::Key::F);
    assert_eq!(rig.ws.grid.tool, GridTool::Floor);
}

#[test]
fn p_and_the_tool_row_still_choose_paint_and_the_brush_survives_a_tool_change() {
    let (mut rig, _) = painting();
    rig.key(egui::Key::R);
    rig.tool(GridTool::Select);
    rig.key(egui::Key::P);
    assert_eq!(rig.ws.grid.tool, GridTool::Paint);
    assert_eq!(rig.ws.grid.paint.orient.turns, 1);
}

// ---------------------------------------------------------------------
// The preview: shown on hover, written never.

#[test]
fn hovering_a_face_previews_the_paint_and_writes_nothing() {
    let (mut rig, brick) = painting();
    let before = rig.ws.project.clone();
    let rev = rig.ws.grid.rev;
    let epoch = rig.ws.history.epoch();
    rig.move_to(rig.cell_centre(1, 0), egui::Modifiers::NONE);
    let ghost = published(&rig).expect("a preview over the face");
    assert_eq!(ghost.material, brick);
    assert!(ghost.orient.is_identity());
    assert_eq!(rig.ws.project, before, "the document is untouched");
    assert_eq!(rig.ws.grid.rev, rev);
    assert_eq!(rig.ws.history.epoch(), epoch);
    assert!(!rig.ws.history.can_undo());
    // The preview names the face under the pointer.
    match &ghost.target {
        crate::PaintTarget::Faces { faces, .. } => {
            assert_eq!(faces, &vec![FaceRef::floor(0, [1, 0])]);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_preview_follows_the_pointer_and_the_keys_and_goes_when_it_leaves() {
    let (mut rig, _) = painting();
    rig.move_to(rig.cell_centre(0, 0), egui::Modifiers::NONE);
    rig.key(egui::Key::R);
    rig.key(egui::Key::F);
    let ghost = published(&rig).expect("preview");
    assert_eq!(ghost.orient.turns, 1);
    assert_eq!(ghost.orient.flip, [true, false]);
    // On to the next face: the same brush, another target.
    rig.move_to(rig.cell_centre(2, 0), egui::Modifiers::NONE);
    let ghost = published(&rig).expect("preview");
    assert_eq!(ghost.orient.turns, 1);
    match &ghost.target {
        crate::PaintTarget::Faces { faces, .. } => {
            assert_eq!(faces, &vec![FaceRef::floor(0, [2, 0])]);
        }
        other => panic!("{other:?}"),
    }
    // Off the floor: no preview.
    rig.move_to(rig.top_at(2.5 * S, 5.0 * S), egui::Modifiers::NONE);
    assert_eq!(published(&rig), None);
    // Nothing was painted by any of it.
    for x in 0..4 {
        assert_eq!(floor_material(&rig, x), None);
    }
}

#[test]
fn only_the_paint_tool_with_a_material_previews_and_ctrl_samples_instead() {
    let (mut rig, _) = painting();
    rig.move_to(rig.cell_centre(1, 0), egui::Modifiers::NONE);
    assert!(published(&rig).is_some());
    // Ctrl samples: nothing would be painted.
    rig.move_to(rig.cell_centre(1, 0), ctrl());
    assert_eq!(published(&rig), None);
    rig.move_to(rig.cell_centre(1, 0), egui::Modifiers::NONE);
    assert!(published(&rig).is_some());
    // Another tool: no paint preview.
    rig.tool(GridTool::Select);
    rig.move_to(rig.cell_centre(1, 0), egui::Modifiers::NONE);
    assert_eq!(published(&rig), None);
    // No material: nothing to preview.
    rig.tool(GridTool::Paint);
    rig.ws.grid.material = None;
    rig.move_to(rig.cell_centre(2, 0), egui::Modifiers::NONE);
    assert_eq!(published(&rig), None);
}

#[test]
fn the_preview_is_what_the_click_writes() {
    let (mut rig, _) = painting();
    rig.key(egui::Key::R);
    rig.key_with(egui::Key::F, shift());
    rig.move_to(rig.cell_centre(1, 0), egui::Modifiers::NONE);
    let ghost = published(&rig).expect("preview");
    // Apply the preview to a copy of the world: the same bytes as the click.
    let mut copy = rig.world().clone();
    assert!(ghost.apply(&mut copy));
    rig.click(rig.cell_centre(1, 0));
    assert_eq!(&copy, rig.world());
}

#[test]
fn the_preview_does_not_show_during_a_stroke() {
    let (mut rig, _) = painting();
    rig.drag_hold(
        rig.cell_centre(0, 0),
        rig.cell_centre(2, 0),
        egui::Modifiers::NONE,
    );
    assert_eq!(published(&rig), None, "the stroke paints as it goes");
    rig.release(rig.cell_centre(2, 0), egui::Modifiers::NONE);
}

// ---------------------------------------------------------------------
// The 3D view hovers the same way.

#[test]
fn the_3d_view_previews_the_face_under_the_pointer() {
    let mut rig = GridRig::persp("grid-paint-uv-3d", flat_project(4, 4, false));
    let brick = material(&mut rig, "brick");
    rig.ws.grid.material = Some(brick);
    rig.tool(GridTool::Paint);
    rig.key(egui::Key::R);
    let at = rig.screen_of([1.5 * S, 0.0, 1.5 * S]);
    rig.move_to(at, egui::Modifiers::NONE);
    let ghost = published(&rig).expect("preview in 3D");
    assert_eq!(ghost.orient.turns, 1);
    match &ghost.target {
        crate::PaintTarget::Faces { faces, .. } => {
            assert_eq!(faces, &vec![FaceRef::floor(0, [1, 1])]);
        }
        other => panic!("{other:?}"),
    }
    assert!(!rig.ws.history.can_undo());
    rig.click(at);
    let floor = rig.room(0).sector(1, 1).unwrap().ground_floor().unwrap();
    assert_eq!(floor.tri_a.uv.turns, 1);
    assert_eq!(floor.tri_a.material, Some(brick));
}

// ---------------------------------------------------------------------
// The whole window: the options bar and the plan.

/// Every text the last frame drew, with its bounds.
fn texts(rig: &WindowRig) -> Vec<(String, Rect)> {
    rig.out
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) => {
                Some((t.galley.text().to_string(), t.visual_bounding_rect()))
            }
            _ => None,
        })
        .collect()
}

fn frames(rig: &mut WindowRig, n: usize) {
    for _ in 0..n {
        rig.frame(vec![]);
    }
}

/// The options bar's row: where the Snap text sits.
fn options_row(rig: &WindowRig) -> (f32, f32) {
    let snap = texts(rig)
        .into_iter()
        .find(|(t, r)| t.starts_with("Snap ") && r.min.y > 60.0 && r.max.y < 240.0)
        .expect("Snap in the options bar")
        .1;
    (snap.center().y, snap.min.x)
}

/// The texts of the options bar left of Snap.
fn bar_texts(rig: &WindowRig) -> Vec<(String, Rect)> {
    let (y, right) = options_row(rig);
    texts(rig)
        .into_iter()
        .filter(|(_, r)| (r.center().y - y).abs() < 8.0 && r.min.x > 290.0 && r.max.x < right)
        .collect()
}

fn bar_text(rig: &WindowRig, label: &str) -> Option<Rect> {
    bar_texts(rig)
        .into_iter()
        .find(|(t, _)| t.trim_start_matches(|c: char| !c.is_alphanumeric()).trim() == label)
        .map(|(_, r)| r)
}

fn paint_window() -> WindowRig {
    let mut rig = WindowRig::starter(Vec2::new(1456.0, 819.0));
    rig.ws.apply_grid_pose("view=3d;tool=paint").unwrap();
    let material = rig
        .ws
        .project
        .resources
        .iter()
        .find(|r| r.name == "Graybox / Ground 64")
        .map(|r| r.id)
        .expect("a Graybox material");
    rig.ws.grid.material = Some(material);
    frames(&mut rig, 4);
    rig
}

#[test]
fn the_options_bar_shows_the_turn_in_degrees_and_a_button_for_each_action() {
    let mut rig = paint_window();
    for label in [
        "0 deg",
        "Turn left (Shift+R)",
        "Turn right (R)",
        "Flip H (F)",
        "Flip V (Shift+F)",
    ] {
        assert!(
            bar_text(&rig, label).is_some(),
            "no `{label}` in the options bar: {:?}",
            bar_texts(&rig)
        );
    }
    rig.ws.grid.paint.orient = crate::PaintOrient {
        turns: 3,
        flip: [false, false],
        ..Default::default()
    };
    frames(&mut rig, 2);
    assert!(bar_text(&rig, "270 deg").is_some(), "{:?}", bar_texts(&rig));
}

#[test]
fn the_options_bar_buttons_turn_and_flip_the_paint() {
    let mut rig = paint_window();
    let click = |rig: &mut WindowRig, label: &str| {
        let at = bar_text(rig, label)
            .unwrap_or_else(|| panic!("no `{label}`"))
            .center();
        rig.click(at);
        frames(rig, 2);
    };
    click(&mut rig, "Turn right (R)");
    assert_eq!(rig.ws.grid.paint.orient.turns, 1);
    click(&mut rig, "Turn left (Shift+R)");
    click(&mut rig, "Turn left (Shift+R)");
    assert_eq!(rig.ws.grid.paint.orient.turns, 3);
    click(&mut rig, "Flip H (F)");
    assert_eq!(rig.ws.grid.paint.orient.flip, [true, false]);
    click(&mut rig, "Flip V (Shift+F)");
    assert_eq!(rig.ws.grid.paint.orient.flip, [true, true]);
    assert!(bar_text(&rig, "270 deg").is_some(), "{:?}", bar_texts(&rig));
    assert_eq!(rig.ws.grid.tool, GridTool::Paint);
    assert!(!rig.ws.history.can_undo(), "the brush is not the document");
}

#[test]
fn the_options_bar_draws_the_swatch_at_the_current_turn_and_flip() {
    // The swatch is a textured quad; its corners carry the texture corners
    // the real mapping gives a one-tile face turned and flipped that way.
    let mut rig = paint_window();
    let quad = |rig: &WindowRig| -> Option<Vec<Pos2>> {
        let (y, right) = options_row(rig);
        rig.out.shapes.iter().find_map(|s| match &s.shape {
            egui::epaint::Shape::Mesh(m)
                if m.vertices.len() == 4
                    && (m.calc_bounds().center().y - y).abs() < 10.0
                    && m.calc_bounds().max.x < right
                    && m.calc_bounds().width() > 12.0
                    && m.calc_bounds().width() < 40.0 =>
            {
                Some(m.vertices.iter().map(|v| v.uv).collect())
            }
            _ => None,
        })
    };
    let plain = quad(&rig).expect("a swatch in the options bar");
    assert_eq!(
        plain,
        vec![
            Pos2::new(0.0, 0.0),
            Pos2::new(1.0, 0.0),
            Pos2::new(1.0, 1.0),
            Pos2::new(0.0, 1.0)
        ]
    );
    rig.ws.grid.paint.orient = crate::PaintOrient {
        turns: 1,
        flip: [true, false],
        ..Default::default()
    };
    frames(&mut rig, 2);
    let moved = quad(&rig).expect("a swatch");
    let expected: Vec<Pos2> = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]
        .iter()
        .map(|&(a, b)| {
            let [u, v] = psxed_project::grid::uv::orient_unit(1, [true, false], a, b);
            Pos2::new(u as f32, v as f32)
        })
        .collect();
    assert_eq!(moved, expected);
    assert_ne!(moved, plain);
}

/// The first floor sector of the starter room, as a world cell.
fn first_floor_cell(rig: &WindowRig) -> [i32; 2] {
    let room = &rig.world().rooms[0];
    room.leaves()
        .find_map(|(key, sector)| {
            sector.ground_floor()?;
            Some([key.x.div_euclid(4), key.z.div_euclid(4)])
        })
        .expect("a floor")
}

/// Meshes the last frame drew inside `rect` (textured quads from the plan).
fn meshes_in(rig: &WindowRig, rect: Rect) -> Vec<egui::epaint::Mesh> {
    rig.out
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::epaint::Shape::Mesh(m) if rect.contains_rect(m.calc_bounds().shrink(1.0)) => {
                Some((**m).clone())
            }
            _ => None,
        })
        .collect()
}

#[test]
fn the_plan_shows_the_paint_on_the_face_under_the_pointer_and_takes_it_away() {
    let mut rig = paint_window();
    rig.ws.apply_grid_pose("view=plan;tool=paint").unwrap();
    frames(&mut rig, 4);
    let [cx, cz] = first_floor_cell(&rig);
    let centre = rig.cell(cx, cz);
    let half = rig.cell(cx + 1, cz).x - centre.x;
    let cell_rect = Rect::from_center_size(centre, Vec2::splat(half.abs() * 2.0));
    let before = meshes_in(&rig, cell_rect).len();
    rig.move_to(centre);
    let hovered = meshes_in(&rig, cell_rect);
    assert!(
        hovered.len() > before,
        "a textured face over the cell: {before} -> {}",
        hovered.len()
    );
    // Turned, the same face carries other texture corners.
    let uvs = |ms: &[egui::epaint::Mesh]| -> Vec<Vec<Pos2>> {
        ms.iter()
            .map(|m| m.vertices.iter().map(|v| v.uv).collect())
            .collect()
    };
    let plain = uvs(&hovered);
    let doc = rig.ws.project.clone();
    rig.ws.grid.paint.orient = crate::PaintOrient {
        turns: 1,
        flip: [false, false],
        ..Default::default()
    };
    rig.move_to(centre + Vec2::new(1.0, 1.0));
    let turned = uvs(&meshes_in(&rig, cell_rect));
    assert_ne!(plain, turned);
    assert_eq!(rig.ws.project, doc, "hovering writes nothing");
    // Off the face: the preview is gone.
    rig.move_to(rig.cell(cx + 40, cz + 40));
    assert_eq!(meshes_in(&rig, cell_rect).len(), before);
}

// ---------------------------------------------------------------------
// Captures for the review pages.

/// `PSXED_PAINT_PROJECT=<dir>` writes a copy of the starter project with a
/// material whose picture is not the same turned (so a turn shows), for
/// `frontend dump-editor-ui` captures; without the variable this does
/// nothing. Prints the material's palette index and the starter's floor.
#[test]
fn paint_capture_project() {
    let Some(dir) = std::env::var_os("PSXED_PAINT_PROJECT") else {
        return;
    };
    let dir = PathBuf::from(dir);
    let _ = std::fs::remove_dir_all(&dir);
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../projects");
    crate::starter_catalogue::copy_dir_recursive(&source.join("grid-starter"), &dir).unwrap();
    // The starter's `assets` is a link into another project: make it a real
    // copy, so writing the sample never reaches the source tree.
    if std::fs::symlink_metadata(dir.join("assets"))
        .unwrap()
        .file_type()
        .is_symlink()
    {
        std::fs::remove_file(dir.join("assets")).unwrap();
        crate::starter_catalogue::copy_dir_recursive(
            &source.join("graybox-reach/assets"),
            &dir.join("assets"),
        )
        .unwrap();
    }
    std::fs::copy(
        source.join("default/assets/textures").join(
            std::env::var("PSXED_PAINT_TEXTURE")
                .unwrap_or_else(|_| "dp_city_kit/dp_city_platform_edge.psxt".to_string()),
        ),
        dir.join("assets/paint-sample.psxt"),
    )
    .unwrap();
    let mut project =
        ProjectDocument::load_from_path(dir.join("project.ron")).expect("the starter loads");
    // One whole tile across each face, so a turn is plain to see.
    let mut sample = MaterialResource::opaque(Some("assets/paint-sample.psxt".to_string()));
    sample.grid_uv = psxed_project::face_uv::UvMode::fit(1, 1);
    project.add_resource("Paint sample", ResourceData::Material(sample));
    let index = project.material_options().len() - 1;
    project.save_to_path(dir.join("project.ron")).unwrap();
    let world = project.active_scene().world.grid().unwrap();
    let room = &world.rooms[0];
    println!(
        "PAINT_MATERIAL_INDEX={index} room origin {:?} size {}x{}",
        room.origin, room.width, room.depth
    );
}
