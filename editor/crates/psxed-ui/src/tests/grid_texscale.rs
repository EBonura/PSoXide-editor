//! The texture density and the quick scale picker (Manny, 2026-10-20: "the
//! unit should be for 64x64 textures ... we should have a quick command to
//! select the scale at 0.25/0.5/1/2/4x").
//!
//! The Paint brush carries a scale (buttons in the options bar, `[` and `]`);
//! the Select tool scales the selected faces (the Tiling section's row,
//! `-` and `=`); the World's Inspector holds the texels per sector.

use super::grid_live::WindowRig;
use super::grid_rig::*;
use super::grid_ui::{draw_editor, draw_with_events, full_context};
use super::*;
use crate::grid_tools::GridTool;
use psxed_project::face_uv::FaceUv;
use psxed_project::grid::edit::{edit_uv, FaceRef, UvOp};
use psxed_project::grid::SectorTexels;
use psxed_project::{MaterialResource, ResourceData, ResourceId};

fn material(rig: &mut GridRig, name: &str) -> ResourceId {
    rig.ws
        .project
        .add_resource(name, ResourceData::Material(MaterialResource::opaque(None)))
}

/// A 4 x 1 hall in the Top view, the Paint tool chosen, `brick` the paint.
fn painting() -> (GridRig, ResourceId) {
    let mut rig = GridRig::top("grid-texscale", flat_project(4, 1, false));
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

fn published(rig: &GridRig) -> Option<crate::PaintGhost> {
    rig.ws.publish_grid_view();
    crate::grid_preview_view().paint
}

// ---------------------------------------------------------------------
// The Paint brush.

#[test]
fn brackets_step_the_brush_scale_and_a_click_stores_it_on_the_face() {
    let (mut rig, _) = painting();
    assert_eq!(rig.ws.grid.paint.orient.zoom, 0, "the brush starts at 1x");
    rig.key(egui::Key::CloseBracket);
    assert_eq!(rig.ws.grid.paint.orient.zoom_q8(), 512);
    rig.click(rig.cell_centre(0, 0));
    assert_eq!(floor_uv(&rig, 0).scale_q8, [512, 512]);
    // `[` steps down: 1x, 0.5x, 0.25x, and stops there.
    for _ in 0..5 {
        rig.key(egui::Key::OpenBracket);
    }
    assert_eq!(rig.ws.grid.paint.orient.zoom_q8(), 64);
    rig.click(rig.cell_centre(1, 0));
    assert_eq!(floor_uv(&rig, 1).scale_q8, [64, 64]);
    // And up to 4x.
    for _ in 0..9 {
        rig.key(egui::Key::CloseBracket);
    }
    assert_eq!(rig.ws.grid.paint.orient.zoom_q8(), 1024);
    rig.click(rig.cell_centre(2, 0));
    assert_eq!(floor_uv(&rig, 2).scale_q8, [1024, 1024]);
    // The keys changed the brush, not the faces painted before.
    assert_eq!(floor_uv(&rig, 0).scale_q8, [512, 512]);
}

#[test]
fn the_brackets_belong_to_paint_only_while_paint_is_active() {
    let (mut rig, _) = painting();
    rig.key(egui::Key::CloseBracket);
    assert_eq!(rig.ws.grid.paint.orient.zoom, 1);
    // In Select the brackets keep nudging the selection's height.
    rig.tool(GridTool::Select);
    rig.key(egui::Key::CloseBracket);
    assert_eq!(rig.ws.grid.paint.orient.zoom, 1, "Select leaves the brush");
}

#[test]
fn a_plain_brush_leaves_the_scale_a_face_already_has() {
    let (mut rig, _) = painting();
    let world = rig.ws.project.active_scene_mut().world.grid_mut().unwrap();
    edit_uv(world, FaceRef::floor(0, [1, 0]), None, UvOp::SetZoom(512));
    rig.click(rig.cell_centre(1, 0));
    assert_eq!(
        floor_uv(&rig, 1).scale_q8,
        [512, 512],
        "painting is not a reset"
    );
}

#[test]
fn a_scale_only_brush_leaves_the_turn_and_flip_a_face_has() {
    let (mut rig, _) = painting();
    let world = rig.ws.project.active_scene_mut().world.grid_mut().unwrap();
    edit_uv(world, FaceRef::floor(0, [1, 0]), None, UvOp::Turn(true));
    edit_uv(world, FaceRef::floor(0, [1, 0]), None, UvOp::ToggleFlip(0));
    rig.key(egui::Key::CloseBracket);
    rig.click(rig.cell_centre(1, 0));
    let uv = floor_uv(&rig, 1);
    assert_eq!((uv.turns, uv.flip), (1, [true, false]));
    assert_eq!(uv.scale_q8, [512, 512]);
}

#[test]
fn a_scaled_brush_keeps_the_mirror_of_a_negative_scale_and_is_one_undo_step() {
    let (mut rig, brick) = painting();
    let world = rig.ws.project.active_scene_mut().world.grid_mut().unwrap();
    edit_uv(world, FaceRef::floor(0, [0, 0]), None, UvOp::FlipU);
    rig.key(egui::Key::R);
    rig.key(egui::Key::OpenBracket);
    rig.click(rig.cell_centre(0, 0));
    assert_eq!(floor_uv(&rig, 0).scale_q8, [-128, 128]);
    assert_eq!(floor_uv(&rig, 0).turns, 1);
    rig.ws.do_undo();
    assert_eq!(
        floor_uv(&rig, 0).scale_q8,
        [-256, 256],
        "only the paint is undone"
    );
    assert_ne!(
        rig.room(0)
            .sector(0, 0)
            .unwrap()
            .ground_floor()
            .unwrap()
            .tri_a
            .material,
        Some(brick)
    );
    assert!(!rig.ws.history.can_undo(), "one step for the paint");
}

#[test]
fn the_preview_shows_the_scale_and_is_what_the_click_writes() {
    let (mut rig, _) = painting();
    rig.key(egui::Key::CloseBracket);
    rig.key(egui::Key::CloseBracket);
    rig.move_to(rig.cell_centre(1, 0), egui::Modifiers::NONE);
    let ghost = published(&rig).expect("a preview");
    assert_eq!(ghost.orient.zoom_q8(), 1024);
    let mut copy = rig.world().clone();
    assert!(ghost.apply(&mut copy));
    assert!(!rig.ws.history.can_undo(), "hovering writes nothing");
    rig.click(rig.cell_centre(1, 0));
    assert_eq!(&copy, rig.world());
}

#[test]
fn the_plan_texels_of_the_preview_follow_the_scale() {
    // The preview's faces come from the painted copy: 2x spans half a tile
    // across the sector, 0.5x two.
    let (mut rig, _) = painting();
    rig.key(egui::Key::CloseBracket);
    rig.move_to(rig.cell_centre(1, 0), egui::Modifiers::NONE);
    let ghost = published(&rig).expect("a preview");
    let mut painted = rig.world().clone();
    ghost.apply(&mut painted);
    let faces = psxed_project::grid::faces::all_faces(&painted);
    let face = faces
        .iter()
        .find(|f| f.points.iter().all(|p| (1024..=2048).contains(&p[0])))
        .unwrap();
    let us: Vec<f64> = face.texels.iter().map(|t| t[0]).collect();
    let span =
        us.iter().cloned().fold(f64::MIN, f64::max) - us.iter().cloned().fold(f64::MAX, f64::min);
    assert_eq!(span, 32.0, "a sector at 2x is half a 64 texel tile");
}

// ---------------------------------------------------------------------
// The options bar.

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

/// The options bar's button labelled `label`, by its text in the row of the
/// Paint options (the row of "Flip H (F)").
fn option(rig: &WindowRig, label: &str) -> Option<Rect> {
    let anchor = texts(rig)
        .into_iter()
        .find(|(t, _)| t.trim_start_matches(|c: char| !c.is_alphanumeric()).trim() == "Flip H (F)")?
        .1;
    texts(rig)
        .into_iter()
        .filter(|(_, r)| (r.center().y - anchor.center().y).abs() < 8.0)
        .find(|(t, _)| t.trim() == label)
        .map(|(_, r)| r)
}

#[test]
fn the_options_bar_has_a_button_for_each_scale_and_the_current_one_is_marked() {
    let mut rig = paint_window();
    for label in ["0.25x", "0.5x", "1x", "2x", "4x"] {
        assert!(
            option(&rig, label).is_some(),
            "no `{label}` in the options bar: {:?}",
            texts(&rig)
                .into_iter()
                .filter(|(_, r)| r.center().y > 120.0 && r.center().y < 170.0)
                .map(|(t, r)| (t, r.min.x as i32, r.max.x as i32))
                .collect::<Vec<_>>()
        );
    }
    let click = |rig: &mut WindowRig, label: &str| {
        let at = option(rig, label).unwrap().center();
        rig.click(at);
        frames(rig, 2);
    };
    click(&mut rig, "2x");
    assert_eq!(rig.ws.grid.paint.orient.zoom, 1);
    click(&mut rig, "0.25x");
    assert_eq!(rig.ws.grid.paint.orient.zoom, -2);
    click(&mut rig, "1x");
    assert_eq!(rig.ws.grid.paint.orient.zoom, 0);
    click(&mut rig, "4x");
    assert_eq!(rig.ws.grid.paint.orient.zoom_q8(), 1024);
    assert_eq!(rig.ws.grid.tool, GridTool::Paint);
    assert!(!rig.ws.history.can_undo(), "the brush is not the document");
}

#[test]
fn the_scale_buttons_fit_in_the_options_bar_at_1456_wide() {
    let rig = paint_window();
    let snap = texts(&rig)
        .into_iter()
        .find(|(t, r)| t.starts_with("Snap ") && r.min.y > 60.0 && r.max.y < 240.0)
        .expect("Snap in the options bar")
        .1;
    let four = option(&rig, "4x").expect("the 4x button");
    assert!(
        four.max.x < snap.min.x,
        "4x {four:?} runs into Snap {snap:?}"
    );
}

#[test]
fn a_long_material_name_does_not_push_the_scale_buttons_out_of_the_bar() {
    let mut rig = paint_window();
    let paint = rig.ws.grid.material.unwrap();
    rig.ws.project.resource_mut(paint).unwrap().name =
        "Cortex / Concrete rebar with a very long name indeed".to_string();
    frames(&mut rig, 3);
    let snap = texts(&rig)
        .into_iter()
        .find(|(t, r)| t.starts_with("Snap ") && r.min.y > 60.0 && r.max.y < 240.0)
        .expect("Snap in the options bar")
        .1;
    let four = option(&rig, "4x").expect("the 4x button");
    assert!(
        four.max.x < snap.min.x,
        "4x {four:?} runs into Snap {snap:?}"
    );
}

// ---------------------------------------------------------------------
// Select: scale the selected faces.

fn text_of(output: &egui::FullOutput) -> Vec<(Pos2, String)> {
    output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) => {
                Some((t.pos + t.galley.size() / 2.0, t.galley.text().to_string()))
            }
            _ => None,
        })
        .collect()
}

/// Click the label `label` of the drawn editor, on the row of `row_of`.
fn click_label(ws: &mut EditorWorkspace, label: &str, row_of: Option<&str>) {
    let ctx = full_context();
    ws.last_viewport_size = Vec2::new(1200.0, 700.0);
    let texts = text_of(&draw_editor(ws, &ctx, true));
    let row_y = row_of.map(|anchor| {
        texts
            .iter()
            .find(|(_, t)| t == anchor)
            .unwrap_or_else(|| panic!("no {anchor:?} on screen"))
            .0
            .y
    });
    let pos = texts
        .iter()
        .find(|(p, t)| t == label && p.y > 90.0 && row_y.is_none_or(|y| (p.y - y).abs() < 3.0))
        .unwrap_or_else(|| panic!("no {label:?} on screen"))
        .0;
    let press = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    draw_with_events(
        ws,
        &ctx,
        vec![egui::Event::PointerMoved(pos), press(true), press(false)],
    );
}

fn scale_of(ws: &EditorWorkspace, cell: [i32; 2]) -> [i16; 2] {
    psxed_project::grid::edit::uv_of(
        ws.project.active_scene().world.grid().unwrap(),
        FaceRef::floor(0, cell),
        None,
    )
    .unwrap()
    .scale_q8
}

fn selecting(label: &str, pose: &str) -> EditorWorkspace {
    let mut ws = EditorWorkspace::with_project(test_temp_dir(label), flat_project(6, 4, true));
    ws.apply_grid_pose(pose).unwrap();
    ws
}

#[test]
fn the_tiling_scale_row_scales_every_selected_face_in_one_undo_step() {
    let mut ws = selecting("texscale-select", "tool=select;select=floors:1,1:3,1");
    for label in ["0.25x", "0.5x", "1x", "2x", "4x"] {
        let drawn = text_of(&draw_editor(&mut ws, &full_context(), true));
        assert!(
            drawn.iter().any(|(_, t)| t == label),
            "no `{label}` in the Tiling"
        );
    }
    click_label(&mut ws, "2x", None);
    for x in 1..=3 {
        assert_eq!(scale_of(&ws, [x, 1]), [512, 512], "x {x}");
    }
    // A face outside the selection is untouched.
    assert_eq!(scale_of(&ws, [0, 1]), [256, 256]);
    click_label(&mut ws, "0.25x", None);
    assert_eq!(scale_of(&ws, [2, 1]), [64, 64]);
    // One undo step per click.
    ws.do_undo();
    for x in 1..=3 {
        assert_eq!(scale_of(&ws, [x, 1]), [512, 512], "x {x}");
    }
    ws.do_undo();
    for x in 1..=3 {
        assert_eq!(scale_of(&ws, [x, 1]), [256, 256], "x {x}");
    }
}

#[test]
fn the_current_scale_of_a_face_is_marked_in_the_row() {
    // Reading it back: the face is at 2x, so the picker says so.
    let mut ws = selecting("texscale-mark", "tool=select;select=floors:2,2:2,2");
    click_label(&mut ws, "2x", None);
    let uv = psxed_project::grid::edit::uv_of(
        ws.project.active_scene().world.grid().unwrap(),
        FaceRef::floor(0, [2, 2]),
        None,
    )
    .unwrap();
    assert_eq!(uv.zoom_q8(), Some(512));
}

#[test]
fn minus_and_equals_step_the_selected_faces_and_stop_at_the_ends() {
    let mut ws = selecting("texscale-keys", "tool=select;select=floors:1,1:2,1");
    let ctx = full_context();
    let press = |ws: &mut EditorWorkspace, key: egui::Key| {
        draw_with_events(
            ws,
            &ctx,
            vec![
                egui::Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed: false,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    };
    press(&mut ws, egui::Key::Equals);
    assert_eq!(scale_of(&ws, [1, 1]), [512, 512]);
    assert_eq!(scale_of(&ws, [2, 1]), [512, 512]);
    press(&mut ws, egui::Key::Equals);
    press(&mut ws, egui::Key::Equals);
    assert_eq!(scale_of(&ws, [1, 1]), [1024, 1024], "4x is the top");
    press(&mut ws, egui::Key::Minus);
    assert_eq!(scale_of(&ws, [1, 1]), [512, 512]);
    // Each key press is its own undo step.
    ws.do_undo();
    assert_eq!(scale_of(&ws, [1, 1]), [1024, 1024]);
}

// ---------------------------------------------------------------------
// The World's texels per sector.

fn world_inspector(label: &str, texels: SectorTexels) -> EditorWorkspace {
    let mut ws = EditorWorkspace::with_project(test_temp_dir(label), flat_project(4, 4, true));
    ws.project
        .active_scene_mut()
        .world
        .grid_mut()
        .unwrap()
        .texels_per_sector = texels;
    ws.select_world();
    ws
}

fn density(ws: &EditorWorkspace) -> u16 {
    ws.project
        .active_scene()
        .world
        .grid()
        .unwrap()
        .texels_per_sector
        .get()
}

#[test]
fn the_worlds_inspector_holds_the_texels_per_sector_and_switches_it_in_one_undo_step() {
    let mut ws = world_inspector("texscale-world", SectorTexels::LEGACY);
    let drawn = text_of(&draw_editor(&mut ws, &full_context(), true));
    assert!(
        drawn.iter().any(|(_, t)| t == "Texels per sector"),
        "no density in the World's Inspector: {drawn:?}"
    );
    assert_eq!(density(&ws), 32);
    click_label(&mut ws, "64", Some("Texels per sector"));
    assert_eq!(density(&ws), 64);
    // Faces follow it: a default floor spans a tile per sector now.
    let world = ws.project.active_scene().world.grid().unwrap();
    let faces = psxed_project::grid::faces::all_faces(world);
    let us: Vec<f64> = faces[0].texels.iter().map(|t| t[0]).collect();
    let span =
        us.iter().cloned().fold(f64::MIN, f64::max) - us.iter().cloned().fold(f64::MAX, f64::min);
    assert_eq!(span, 64.0);
    ws.do_undo();
    assert_eq!(density(&ws), 32);
}

#[test]
fn a_new_workspace_world_is_64() {
    let ws = world_inspector("texscale-new", SectorTexels::DEFAULT);
    assert_eq!(density(&ws), 64);
}

#[test]
fn the_inspector_counts_tiles_at_the_worlds_density() {
    let mut ws = selecting("texscale-report", "tool=select;select=floors:2,2:2,2");
    let said = |ws: &mut EditorWorkspace| {
        text_of(&draw_editor(ws, &full_context(), true))
            .into_iter()
            .map(|(_, t)| t)
            .collect::<Vec<_>>()
            .join("\n")
    };
    // A new world: a tile per sector. Scaled to 2x: half a tile.
    assert!(said(&mut ws).contains("effective: 1 x 1 tiles on this face"));
    click_label(&mut ws, "2x", None);
    assert!(said(&mut ws).contains("effective: 0.5 x 0.5 tiles on this face"));
    // A world made before the setting: half a tile per sector at 1x.
    let mut old = selecting("texscale-report-old", "tool=select;select=floors:2,2:2,2");
    old.project
        .active_scene_mut()
        .world
        .grid_mut()
        .unwrap()
        .texels_per_sector = SectorTexels::LEGACY;
    assert!(said(&mut old).contains("effective: 0.5 x 0.5 tiles on this face"));
}

// ---------------------------------------------------------------------
// Captures for the review pages.

/// `PSXED_TEXSCALE_PROJECT=<dir>` with `PSXED_TEXSCALE_SOURCE=<a dp-world
/// copy>` writes a copy of that project with a "Capture" room beside the
/// world: a strip of floor in a 64x64 Cortex material at 1x, 0.5x and 2x,
/// then a 32x32 DP material at 1x (DP / 062). `PSXED_TEXSCALE_SWITCH=64` then switches
/// the copy to 64 texels per sector the way the World's Inspector does (keep
/// faces with other texture sizes), so the same project can be shot before and
/// after. Without the variables this does nothing. Prints where the strip is.
#[test]
fn texscale_capture_project() {
    use psxed_project::grid::density::{set_texels_per_sector, KeepLook};
    use psxed_project::grid::{GridRoom, GridSurface};
    let (Some(dir), Some(source)) = (
        std::env::var_os("PSXED_TEXSCALE_PROJECT"),
        std::env::var_os("PSXED_TEXSCALE_SOURCE"),
    ) else {
        return;
    };
    let (dir, source) = (PathBuf::from(dir), PathBuf::from(source));
    let _ = std::fs::remove_dir_all(&dir);
    crate::starter_catalogue::copy_dir_recursive(&source, &dir).unwrap();
    if std::fs::symlink_metadata(dir.join("assets"))
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
    {
        std::fs::remove_file(dir.join("assets")).unwrap();
        let tracked = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../projects");
        crate::starter_catalogue::copy_dir_recursive(
            &tracked.join("graybox-reach/assets"),
            &dir.join("assets"),
        )
        .unwrap();
    }
    let mut project = ProjectDocument::load_from_path(dir.join("project.ron")).expect("loads");
    let by_name = |project: &ProjectDocument, name: &str| {
        project
            .resources
            .iter()
            .find(|r| r.name == name)
            .map(|r| r.id)
            .unwrap_or_else(|| panic!("no material {name}"))
    };
    let cortex = by_name(&project, "Cortex / Concrete rebar");
    let dp = by_name(&project, "DP / 062");
    let world = project.active_scene_mut().world.grid_mut().unwrap();
    // Beside the world: past the east edge of every room, at its first row.
    let east = world
        .rooms
        .iter()
        .map(|r| r.origin[0] + r.width as i32)
        .max()
        .unwrap_or(0);
    let north = world.rooms.iter().map(|r| r.origin[1]).min().unwrap_or(0);
    let mut room = GridRoom::new("Capture", 4, 1);
    room.origin = [east + 2, north];
    for (x, material) in [(0usize, cortex), (1, cortex), (2, cortex), (3, dp)] {
        room.sector_or_insert(x, 0)
            .unwrap()
            .set_ground_floor(Some(GridSurface::flat(0, Some(material))));
    }
    let origin = room.origin;
    world.rooms.push(room);
    let index = world.rooms.len() - 1;
    for (x, q8) in [(1, 128), (2, 512)] {
        let face = FaceRef::floor(index, [origin[0] + x, origin[1]]);
        edit_uv(world, face, None, UvOp::SetZoom(q8));
    }
    let root = dir.clone();
    if let Some(texels) = std::env::var("PSXED_TEXSCALE_SWITCH")
        .ok()
        .and_then(|t| t.parse::<u16>().ok())
    {
        let width = move |p: &ProjectDocument, id| -> Option<u16> {
            let (_, bytes) = psxed_project::resolve_material_texture_psxt(p, id, &root)
                .ok()
                .flatten()?;
            psx_asset::Texture::from_bytes(&bytes)
                .ok()
                .map(|t| t.width())
        };
        let change = set_texels_per_sector(
            &mut project,
            SectorTexels::new(texels).unwrap(),
            KeepLook::OtherTextureSizes,
            &width,
        )
        .unwrap();
        println!("SWITCHED {change:?}");
    }
    project.save_to_path(dir.join("project.ron")).unwrap();
    let palette_index = project
        .material_options()
        .iter()
        .position(|(id, _)| *id == cortex)
        .unwrap();
    println!(
        "TEXSCALE_CAPTURE origin {origin:?} room {index} cortex_material_index={palette_index}"
    );
}
