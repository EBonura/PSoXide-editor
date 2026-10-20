//! The core workflow of a new level, driven the way a designer drives it
//! (real pointer events in the plan): a new room, a raised corner, paint, a
//! flipped diagonal with one triangle selected, a light, undo and redo, then
//! save and reopen. If any step stops working this fails at that step.

use super::grid_rig::*;
use super::*;
use crate::grid_tools::GridTool;
use psxed_project::grid::edit::{FaceRef, FaceTarget, GridElement};
use psxed_project::grid::{GridSplit, LeafKey};

fn light_count(ws: &EditorWorkspace) -> usize {
    ws.project
        .active_scene()
        .nodes()
        .iter()
        .filter(|n| matches!(n.kind, psxed_project::NodeKind::PointLight { .. }))
        .count()
}

fn split_of(rig: &GridRig, cell: [i32; 2]) -> GridSplit {
    let room = rig.room(0);
    let [x, z] = room.local(cell).unwrap();
    room.sector(x, z).unwrap().ground_floor().unwrap().split
}

#[test]
fn a_new_level_from_nothing_to_a_saved_file() {
    let mut rig = GridRig::top("workflow", empty_project());
    let material = |rig: &mut GridRig, name: &str| {
        rig.ws.project.add_resource(
            name,
            psxed_project::ResourceData::Material(psxed_project::MaterialResource::opaque(None)),
        )
    };
    let brick = material(&mut rig, "Graybox / Brick");
    material(&mut rig, "Graybox / Moss");

    // 1. A new room: the Floor tool drags a 4 x 3 rectangle in an empty world.
    rig.tool(GridTool::Floor);
    rig.drag(rig.cell_centre(1, 1), rig.cell_centre(4, 3));
    let room = rig.room(0);
    assert_eq!((room.origin, room.width, room.depth), ([1, 1], 4, 3));
    assert_eq!(room.sector_count(), 12);

    // 2. Raise a floor: the Height tool drags the corner shared by four
    // sectors up two clicks, and the neighbours follow.
    rig.tool(GridTool::Height);
    let vertex = rig.top_at(2.0 * S, 2.0 * S);
    rig.drag(
        vertex,
        vertex + Vec2::new(0.0, -2.0 * crate::grid_tools::view::TOP_PIXELS_PER_CLICK),
    );
    assert_eq!(floor_heights(rig.world(), 0, [1, 1]).unwrap()[2], 512);
    assert_eq!(floor_heights(rig.world(), 0, [2, 2]).unwrap()[0], 512);
    assert_eq!(floor_heights(rig.world(), 0, [4, 3]).unwrap(), [0; 4]);

    // 3. Paint a sector with the palette's material.
    rig.ws.choose_palette_material(brick);
    rig.tool(GridTool::Paint);
    rig.click(rig.cell_centre(4, 1));
    let painted = |rig: &GridRig, cell: [i32; 2]| {
        let room = rig.room(0);
        let [x, z] = room.local(cell).unwrap();
        room.sector(x, z)
            .unwrap()
            .ground_floor()
            .unwrap()
            .tri_a
            .material
    };
    assert_eq!(painted(&rig, [4, 1]), Some(brick));
    assert_eq!(painted(&rig, [3, 1]), None);

    // 4. Diagonal: flip one sector's diagonal, then select one triangle of it.
    let cell = [2, 2]; // its corner was raised: the diagonal shows
    let before = split_of(&rig, cell);
    rig.tool(GridTool::Split);
    rig.click(rig.cell_centre(cell[0], cell[1]));
    assert_ne!(split_of(&rig, cell), before, "the diagonal flipped");
    rig.tool(GridTool::Select);
    let (x, z) = (cell[0] as f32, cell[1] as f32);
    rig.click(rig.top_at((x + 0.2) * S, (z + 0.5) * S));
    let items = rig.ws.grid.selection.items().to_vec();
    let face = FaceRef::new(0, LeafKey::sector(cell), FaceTarget::floor(0));
    assert_eq!(items.len(), 1, "{items:?}");
    assert!(
        matches!(items[0], GridElement::Triangle { face: f, .. } if f == face),
        "one triangle of the floor: {items:?}"
    );
    let first = items[0];
    rig.click(rig.top_at((x + 0.8) * S, (z + 0.5) * S));
    let other = rig.ws.grid.selection.items().to_vec();
    assert_eq!(other.len(), 1);
    assert_ne!(other[0], first, "the other half is the other triangle");

    // 5. A light: the Place tool drops a point light on the floor.
    rig.ws.place_kind = PlaceKind::PointLightMarker;
    rig.tool(GridTool::Place);
    assert_eq!(light_count(&rig.ws), 0);
    rig.click(rig.cell_centre(2, 3));
    assert_eq!(light_count(&rig.ws), 1, "{}", rig.ws.status);

    // 6. Undo and redo walk the light off and on again, then the rest back.
    rig.ws.do_undo();
    assert_eq!(light_count(&rig.ws), 0, "undo removes the light");
    rig.ws.do_redo();
    assert_eq!(light_count(&rig.ws), 1, "redo brings it back");

    // 7. Save and reopen: the file holds the same level.
    let dir = rig.ws.project_dir.clone();
    let _cleanup = ScratchProjectDir::new(dir.clone());
    rig.ws.save().unwrap_or_else(|e| panic!("save: {e}"));
    let reopened = EditorWorkspace::open_directory(&dir).expect("reopen");
    let (a, b) = (
        rig.world(),
        reopened.project.active_scene().world.grid().unwrap(),
    );
    assert_eq!(a.rooms.len(), b.rooms.len());
    assert_eq!(a.rooms[0].sector_count(), b.rooms[0].sector_count());
    for z in 0..3 {
        for x in 0..4 {
            let cell = [x + 1, z + 1];
            assert_eq!(
                floor_heights(a, 0, cell),
                floor_heights(b, 0, cell),
                "heights of {cell:?}"
            );
        }
    }
    assert_eq!(light_count(&reopened), 1, "the light is in the file");
    assert!(reopened
        .project
        .resources
        .iter()
        .any(|r| r.name == "Graybox / Brick"));
    let [x, z] = b.rooms[0].local([4, 1]).unwrap();
    let saved_material = b.rooms[0]
        .sector(x, z)
        .unwrap()
        .ground_floor()
        .unwrap()
        .tri_a
        .material;
    assert!(
        saved_material.is_some(),
        "the painted sector kept its material"
    );
}
