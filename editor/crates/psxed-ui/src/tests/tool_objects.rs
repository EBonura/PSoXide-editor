//! The Objects group: every chip placed, then moved by dragging in the Edit
//! tool, rotated in the Inspector, deleted with Delete and brought back with
//! undo, all through clicks and keys.

use super::grid_live::WindowRig;
use super::grid_toolbar::{frames, press_key, text_in, texts};
use super::tool_audit::*;
use super::tool_edit_table::click_text_lowest;
use super::tool_flows::*;
use super::*;
use crate::grid_tools::GridTool;
use psxed_project::NodeId;

fn node_ids(rig: &WindowRig) -> Vec<NodeId> {
    rig.ws
        .project
        .active_scene()
        .nodes()
        .iter()
        .map(|n| n.id)
        .collect()
}

fn strip_band(rig: &WindowRig) -> Rect {
    Rect::from_min_max(
        Pos2::new(0.0, ACTION_BAR_COMPACT_HEIGHT),
        Pos2::new(
            rig.size.x,
            ACTION_BAR_COMPACT_HEIGHT + crate::grid_tools::tool_strip_height(&rig.ctx),
        ),
    )
}

/// Click a chip or button of the strip by its name.
fn strip_click(rig: &mut WindowRig, label: &str) -> bool {
    let band = strip_band(rig);
    match text_in(rig, band, label) {
        Some(r) => {
            rig.click(r.center());
            frames(rig, 3);
            true
        }
        None => false,
    }
}

/// Set the Y rotation of the selected object in the Inspector.
fn set_rotation_y(rig: &mut WindowRig, degrees: &str) -> Result<(), String> {
    let all = texts(rig);
    let label = all
        .iter()
        .filter(|(t, _)| t.trim_start_matches(|c: char| !c.is_alphanumeric()).trim() == "Rotation")
        .max_by(|a, b| a.1.center().x.total_cmp(&b.1.center().x))
        .map(|(_, r)| *r)
        .ok_or("no Rotation row in the Inspector")?;
    let field = all
        .iter()
        .filter(|(t, r)| {
            t.trim().starts_with("Y ")
                && (r.center().y - label.center().y).abs() < 10.0
                && r.min.x > label.max.x
        })
        .map(|(_, r)| *r)
        .next()
        .ok_or("no Y rotation field")?;
    rig.click(field.center());
    rig.click(field.center());
    frames(rig, 1);
    rig.frame(vec![egui::Event::Key {
        key: egui::Key::A,
        physical_key: Some(egui::Key::A),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND,
    }]);
    rig.frame(vec![egui::Event::Text(degrees.to_string())]);
    press_key(rig, egui::Key::Enter, false);
    Ok(())
}

fn node_px(rig: &WindowRig, v: View, t: [f32; 3]) -> Option<Pos2> {
    if v.in_plan() {
        Some(
            ViewportTransform::from_focus(
                rig.ws.last_orthographic_viewport_rect,
                OrthographicView::Top,
                [rig.ws.orthographic_focus[0], rig.ws.orthographic_focus[2]],
                rig.ws.viewport_zoom,
            )
            .world_to_screen([t[0], t[2]]),
        )
    } else {
        px3(rig, t)
    }
}

/// Pick a resource in the Place picker of the options bar (the combo shows
/// `shown`; the popup lists resources by name).
fn pick_place_resource(
    rig: &mut WindowRig,
    shown: &str,
    filter: fn(&psxed_project::ResourceData) -> bool,
) -> Result<(), String> {
    let name = rig
        .ws
        .project
        .resources
        .iter()
        .find(|r| filter(&r.data))
        .map(|r| r.name.clone())
        .ok_or("no such resource in the project")?;
    if !option_click(rig, shown) {
        return Err(format!(
            "no `{shown}` picker in the options bar: {}",
            options_line(rig)
        ));
    }
    let picker = texts(rig)
        .into_iter()
        .find(|(t, _)| t.trim() == shown)
        .map(|(_, r)| r.center())
        .unwrap_or(Pos2::ZERO);
    let hit = texts(rig)
        .into_iter()
        .filter(|(t, _)| t.trim() == name)
        .min_by(|a, b| {
            a.1.center()
                .distance(picker)
                .total_cmp(&b.1.center().distance(picker))
        });
    match hit {
        Some((_, r)) => {
            rig.click(r.center());
            frames(rig, 3);
        }
        None => return Err(format!("the picker does not list `{name}`")),
    }
    if rig.ws.place_resource.is_none() {
        return Err("the picker did not take the resource".into());
    }
    Ok(())
}

/// One object kind, placed by its chip and put through the whole life.
pub(super) fn object_life(v: View, chip: &str, tool: GridTool) -> Result<String, String> {
    let mut rig = v.rig(Fixture::Starter);
    v.aim_near(&mut rig, [-1, -1], [7, 6], 0.55);
    let ids0 = node_ids(&rig);
    let door_first = matches!(chip, "Lever");
    let chip_label = chip;
    if door_first {
        if !strip_click(&mut rig, "Door") {
            return Err("no Door in the strip".into());
        }
        let (a, b) = v.wall(
            &mut rig,
            [2, 0],
            psxed_project::grid::WallEdge::North,
            0.375,
            0.625,
            0.0,
            300.0,
        );
        rig.drag(a, b);
        v.aim_near(&mut rig, [-1, -1], [7, 6], 0.55);
    }
    let ids_before = node_ids(&rig);
    if !strip_click(&mut rig, chip) {
        return Err(format!("no `{chip}` in the strip"));
    }
    if rig.ws.grid.tool != tool {
        return Err(format!(
            "`{chip}` armed {:?}, not {tool:?}",
            rig.ws.grid.tool
        ));
    }
    match chip {
        "Prop" => pick_place_resource(&mut rig, "Auto Model", |d| {
            matches!(d, psxed_project::ResourceData::Model(_))
        })?,
        "Actor" => pick_place_resource(&mut rig, "Auto Profile", |d| {
            matches!(d, psxed_project::ResourceData::Character(_))
        })?,
        _ => {}
    }
    let spot = [4, 2];
    if chip == "Door" {
        let (a, b) = v.wall(
            &mut rig,
            [2, 0],
            psxed_project::grid::WallEdge::North,
            0.375,
            0.625,
            0.0,
            300.0,
        );
        rig.drag(a, b);
        v.aim_near(&mut rig, [-1, -1], [7, 6], 0.55);
    } else {
        v.click_cell(&mut rig, spot);
        // The picker's list can cover a spot or eat the first click: a person
        // clicks somewhere else.
        for other in [[4, 2], [4, 4], [1, 4], [1, 1], [5, 1]] {
            if node_ids(&rig).len() != ids_before.len() || !matches!(chip, "Prop" | "Actor") {
                break;
            }
            v.click_cell(&mut rig, other);
        }
        if chip == "Lever" {
            v.click_cell(&mut rig, [2, 0]);
        }
    }
    let new: Vec<NodeId> = node_ids(&rig)
        .into_iter()
        .filter(|i| !ids_before.contains(i))
        .collect();
    // The thing to move is what can be seen: a door's slab, else the first.
    let id = new
        .iter()
        .copied()
        .find(|i| {
            chip != "Door"
                || rig
                    .ws
                    .project
                    .active_scene()
                    .node(*i)
                    .is_some_and(|n| n.name.ends_with(" box"))
        })
        .ok_or_else(|| format!("nothing placed: {}", rig.ws.status))?;
    let _ = (ids0, chip_label);
    let name = rig
        .ws
        .project
        .active_scene()
        .node(id)
        .map(|n| n.name.clone())
        .unwrap();
    // Move it by dragging it in the Edit tool.
    if !start(&mut rig, GridTool::Edit, Start::Strip) {
        return Err("Edit did not start".into());
    }
    if chip == "Door" && !v.in_plan() {
        // The door stands in the north wall, at the edge of the framing: look at it.
        face_wall(
            &mut rig,
            [2, 0],
            psxed_project::grid::WallEdge::North,
            0.0,
            4.0,
            1200.0,
        );
    }
    let t0 = rig
        .ws
        .project
        .active_scene()
        .node(id)
        .unwrap()
        .transform
        .translation;
    let p = node_px(&rig, v, t0).ok_or("the object is off screen")?;
    let to = if chip == "Door" && !v.in_plan() {
        p + Vec2::new(60.0, 0.0)
    } else {
        p + (v.cell(&rig, [4, 4]) - v.cell(&rig, [4, 2]))
    };
    // A click picks the object, then a drag moves it.
    rig.click(p);
    if !v.in_plan() && name.ends_with(" box") {
        // A box prop has no body to grab in 3D: its gizmo's X arrow moves it.
        let rect = rig.ws.last_persp_viewport_rect;
        let axis = rig
            .ws
            .node_gizmo_screen_axes(rect)
            .into_iter()
            .find(|a| a.axis == crate::gizmo::PrimitiveGizmoAxis::X)
            .ok_or("no gizmo X arrow")?;
        let grab = axis.start + (axis.end - axis.start) * 0.8;
        rig.drag(grab, grab + Vec2::new(150.0, 0.0));
        let t1 = rig
            .ws
            .project
            .active_scene()
            .node(id)
            .unwrap()
            .transform
            .translation;
        if t1 == t0 {
            return Err(format!("the gizmo arrow moved nothing: {} grab {grab:?} rect {rect:?} axes {:?} p {p:?} mode {:?}", rig.ws.status, rig.ws.node_gizmo_screen_axes(rect).iter().map(|a| (a.axis, a.start, a.end)).collect::<Vec<_>>(), rig.ws.transform_gizmo_mode));
        }
        return finish_life(&mut rig, id, name);
    }
    let picked = format!(
        "selected_node {:?} wanted {:?} grid-sel {}",
        rig.ws.selection.selected_node,
        id,
        rig.ws.grid.selection.items().len()
    );
    rig.drag(p, to);
    let t1 = rig
        .ws
        .project
        .active_scene()
        .node(id)
        .unwrap()
        .transform
        .translation;
    if t1 == t0 {
        return Err(format!(
            "`{name}` did not move when dragged: {} ({picked})",
            rig.ws.status
        ));
    }
    finish_life(&mut rig, id, name)
}

/// Rotate in the Inspector, delete, undo.
fn finish_life(rig: &mut WindowRig, id: NodeId, name: String) -> Result<String, String> {
    // Rotate: select its row, set the heading.
    if !click_text_lowest(rig, &name) && !click_text(rig, &name) {
        return Err(format!("`{name}` is not listed in Objects"));
    }
    // A point light has no heading: its Inspector has no Rotation row, and
    // that is what it says.
    if name.starts_with("Light") && set_rotation_y(rig, "90").is_err() {
        return finish_delete(rig, id, name);
    }
    if !name.starts_with("Light") {
        set_rotation_y(rig, "90")?;
    }
    let rot = rig
        .ws
        .project
        .active_scene()
        .node(id)
        .unwrap()
        .transform
        .rotation_degrees;
    if !name.starts_with("Light") && rot[1].round() as i32 != 90 {
        return Err(format!("rotation is {rot:?}"));
    }
    finish_delete(rig, id, name)
}

fn finish_delete(rig: &mut WindowRig, id: NodeId, name: String) -> Result<String, String> {
    // Delete, then undo.
    let before_delete = rig.ws.project.clone();
    key(rig, egui::Key::Delete);
    if rig.ws.project.active_scene().node(id).is_some() {
        return Err(format!("Delete left `{name}`: {}", rig.ws.status));
    }
    rig.ws.do_undo();
    if rig.ws.project != before_delete {
        return Err("undo did not bring the object back".into());
    }
    Ok(name)
}

pub(super) const KINDS: [(&str, GridTool); 6] = [
    ("Light", GridTool::Place),
    ("Prop", GridTool::Place),
    ("Actor", GridTool::Place),
    ("Door", GridTool::Door),
    ("Lever", GridTool::Lever),
    ("Checkpoint", GridTool::Checkpoint),
];

#[test]
fn every_objects_chip_places_moves_rotates_and_deletes() {
    let mut bad = Vec::new();
    for (chip, tool) in KINDS {
        for v in View::ALL {
            if let Err(e) = object_life(v, chip, tool) {
                bad.push(format!("{chip} [{v:?}]: {e}"));
            }
        }
    }
    assert!(bad.is_empty(), "{} cells:\n{}", bad.len(), bad.join("\n"));
}
