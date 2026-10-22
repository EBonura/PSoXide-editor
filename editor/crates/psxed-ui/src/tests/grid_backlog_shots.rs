//! Captures for the review page of the 2026-10-22 backlog lane:
//! `PSXED_BACKLOG_CAPTURE=<dir> PSXED_BACKLOG_LABEL=before|after` writes one
//! PNG per item from the whole window (dp-world stand-in, 1456 x 819);
//! without the variable this does nothing. Uses only what the editor offered
//! before the fixes, so the same test runs on both sides.

use super::grid_extrudefix::{eu, over, HALL};
use super::grid_live::WindowRig;
use super::grid_pick_shots::{look_from, sector, settle, three_d, wire};
use super::*;
use crate::grid_tools::GridTool;
use psxed_project::grid::edit::bands::{edit_bands, BandScope};
use psxed_project::grid::edit::{FaceRef, GridElement};
use psxed_project::grid::{Anchor, GridSurface, WallEdge};

fn rig() -> WindowRig {
    let mut rig = WindowRig::with_size(Vec2::new(1456.0, 819.0));
    rig.ws.grid.walls_prompt_dismissed.insert(0);
    rig.ws.grid.walls_prompt_dismissed.insert(HALL);
    rig
}

#[test]
fn backlog_captures() {
    let Some(dir) = std::env::var_os("PSXED_BACKLOG_CAPTURE") else {
        return;
    };
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let label = std::env::var("PSXED_BACKLOG_LABEL").unwrap_or_else(|_| "after".into());

    // 1. The Cells tool's option row, and the Select tool with a cell picked.
    let mut rig = rig();
    rig.ws.apply_grid_pose("view=3d").unwrap();
    rig.ws.set_grid_tool(GridTool::Cells);
    settle(&mut rig);
    rig.capture(&dir.join(format!("cells-{label}.png")));
    let mut rig = self::rig();
    rig.ws.apply_grid_pose("view=3d;tool=select").unwrap();
    let cell = {
        let (key, _) = rig.world().rooms[HALL].leaves().next().unwrap();
        key.sector_range().0
    };
    rig.ws
        .grid
        .selection
        .replace(GridElement::Cell { room: HALL, cell });
    rig.ws.after_grid_selection_changed();
    settle(&mut rig);
    rig.capture(&dir.join(format!("cells-select-{label}.png")));

    // 2. A hand-placed wall cut into two bands that keep their own materials,
    // painted whole in the plan.
    let mut rig = self::rig();
    rig.ws.apply_grid_pose("view=plan").unwrap();
    settle(&mut rig);
    let (cell, edge) = {
        let room = &rig.world().rooms[0];
        room.leaves()
            .find_map(|(key, sector)| {
                let edge = WallEdge::ALL
                    .into_iter()
                    .find(|e| !e.is_diagonal() && !sector.walls(*e).is_empty())?;
                Some((key.sector_range().0, edge))
            })
            .expect("a hand-placed wall in the starter room")
    };
    let wall = FaceRef::wall(0, cell, edge, 0);
    let material = |rig: &WindowRig, name: &str| {
        rig.ws
            .project
            .resources
            .iter()
            .find(|r| r.name == name)
            .map(|r| r.id)
            .unwrap()
    };
    let (bottom, top, paint) = (
        material(&rig, "DP 0.0"),
        material(&rig, "DP 1.0"),
        material(&rig, "DP 3.2"),
    );
    {
        let world = rig.ws.project.active_scene_mut().world.grid_mut().unwrap();
        assert!(edit_bands(world, wall, BandScope::Piece, |set| {
            assert!(set.insert_line(0, Anchor::Abs(512)));
            set.set_band_material(0, Some(bottom));
            set.set_band_material(1, Some(top));
            true
        }));
    }
    rig.ws.grid_changed();
    rig.ws.set_grid_tool(GridTool::Paint);
    rig.ws.grid.material = Some(paint);
    settle(&mut rig);
    let centre = rig.cell(cell[0], cell[1]);
    let neighbour = {
        let step = match edge {
            WallEdge::North => [0, -1],
            WallEdge::South => [0, 1],
            WallEdge::East => [1, 0],
            _ => [-1, 0],
        };
        rig.cell(cell[0] + step[0], cell[1] + step[1])
    };
    let on_wall = centre + (neighbour - centre) * 0.5;
    rig.move_to(on_wall);
    rig.click(on_wall);
    settle(&mut rig);
    rig.capture(&dir.join(format!("plan-wall-{label}.png")));
    let bands: Vec<_> = psxed_project::grid::edit::bands::effective_set(rig.world(), wall)
        .map(|(_, set)| {
            set.bands
                .iter()
                .map(|b| b.material == Some(paint))
                .collect()
        })
        .unwrap_or_default();
    eprintln!("plan wall bands painted: {bands:?} ({label})");

    // 3. A box over the Machine Hall seen from the south, with a block of
    // raised floor in front: the floors behind it are hidden.
    let mut rig = self::rig();
    three_d(&mut rig);
    {
        let world = rig.ws.project.active_scene_mut().world.grid_mut().unwrap();
        for z in 4..=5 {
            for x in 10..=12 {
                let [lx, lz] = world.rooms[HALL].local([x, z]).unwrap();
                world.rooms[HALL]
                    .sector_or_insert(lx, lz)
                    .unwrap()
                    .set_ground_floor(Some(GridSurface::flat(2560, None)));
            }
        }
    }
    rig.ws.grid_changed();
    let s = sector(&rig);
    let high = eu(&rig, 5200);
    look_from(
        &mut rig,
        [11.5 * s, high, 11.0 * s],
        [11.5 * s, 0.0, 2.0 * s],
    );
    rig.ws.set_grid_tool(GridTool::Select);
    settle(&mut rig);
    let hidden = rig.persp_screen(over(&rig, [11, 1], 0)).unwrap();
    let top = rig.persp_screen(over(&rig, [11, 4], 2560)).unwrap();
    let a = Pos2::new(hidden.x.min(top.x) - 160.0, hidden.y.min(top.y) - 40.0);
    let b = Pos2::new(hidden.x.max(top.x) + 160.0, hidden.y.max(top.y) + 60.0);
    rig.drag(a, b);
    settle(&mut rig);
    wire(&mut rig);
    rig.capture(&dir.join(format!("box-occlusion-{label}.png")));
    let picked = |cell: [i32; 2]| {
        rig.ws
            .grid
            .selection
            .items()
            .contains(&GridElement::Face(FaceRef::floor(HALL, cell)))
    };
    eprintln!(
        "box: floor behind block picked {} block top picked {} ({label})",
        picked([11, 1]),
        picked([11, 4])
    );

    // 5. The Keys page, and the status bar under the Select tool.
    let mut rig = self::rig();
    rig.ws.apply_grid_pose("view=3d;tool=select").unwrap();
    settle(&mut rig);
    rig.capture(&dir.join(format!("hints-{label}.png")));
    rig.ws.grid.help_open = true;
    settle(&mut rig);
    rig.capture(&dir.join(format!("keys-{label}.png")));
}
