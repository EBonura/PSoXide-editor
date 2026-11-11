//! The words of the grid editor: the tool hints and tooltips, the help
//! overlay, the options bar and the Inspector never name how the document is
//! stored. A wall is a wall: no "authored", "derived", "hand-placed" or
//! "generated" walls, no "Select tool" now that Edit is the default, no
//! "Link" (it is Weld), no engine type names.

use super::grid_live::WindowRig;
use super::grid_toolbar::{frames, texts};
use crate::grid_tools::GridTool;

/// Words that must not appear in a label, hint or help line.
const BANNED: [&str; 14] = [
    "Select tool",
    "with the Select",
    "authored",
    "Authored",
    "derived",
    "Derived",
    "hand-placed",
    "hand placed",
    "generated",
    "Generated",
    "Link",
    "BSP",
    "PlaceKind",
    "EdgeOverride",
];

/// Further words that must not appear in the wall section of the Inspector.
const BANNED_IN_WALLS: [&str; 5] = ["override", "Override", "ghost", "Hidden", "Use palette"];

fn check(what: &str, text: &str, banned: &[&str], found: &mut Vec<String>) {
    for word in banned {
        if text.contains(word) {
            found.push(format!("{what}: `{word}` in `{text}`"));
        }
    }
}

#[test]
fn tool_hints_tooltips_help_options_and_inspector_use_the_users_words() {
    let mut found = Vec::new();
    for tool in GridTool::ALL {
        check("hint", tool.hint(), &BANNED, &mut found);
        check("tooltip", tool.tooltip(), &BANNED, &mut found);
        check("label", tool.label(), &BANNED, &mut found);
    }
    // The help overlay.
    let mut rig = WindowRig::new();
    rig.ws.apply_grid_pose("view=3d").unwrap();
    rig.ws.grid.help_open = true;
    frames(&mut rig, 3);
    for (text, _) in texts(&rig) {
        check("help", &text, &BANNED, &mut found);
    }
    rig.ws.grid.help_open = false;
    // Every tool on screen: strip, options bar, hint bar, Inspector.
    for tool in GridTool::ALL {
        rig.ws.set_grid_tool(tool);
        frames(&mut rig, 3);
        for (text, _) in texts(&rig) {
            check(
                &format!("{} screen", tool.label()),
                &text,
                &BANNED,
                &mut found,
            );
        }
    }
    assert!(
        found.is_empty(),
        "stale or internal wording:\n{}",
        found.join("\n")
    );
}

#[test]
fn the_wall_section_of_the_inspector_uses_one_vocabulary() {
    use psxed_project::grid::edit::{FaceRef, GridElement};
    use psxed_project::grid::WallEdge;
    let mut rig = WindowRig::new();
    rig.ws.apply_grid_pose("view=3d").unwrap();
    rig.ws.set_grid_tool(GridTool::Edit);
    // A wall of the first room that has one (the stand-in project has some).
    let face = {
        let world = rig.ws.project.active_scene().world.grid().unwrap();
        world.rooms.iter().enumerate().find_map(|(room, r)| {
            r.leaves().find_map(|(key, sector)| {
                [
                    WallEdge::North,
                    WallEdge::East,
                    WallEdge::South,
                    WallEdge::West,
                ]
                .into_iter()
                .find(|e| !sector.walls(*e).is_empty())
                .map(|edge| FaceRef::wall(room, key.cell(), edge, 0))
            })
        })
    };
    let Some(face) = face else { return };
    rig.ws.grid.selection.set([GridElement::Face(face)]);
    rig.ws.after_grid_selection_changed();
    frames(&mut rig, 3);
    let mut found = Vec::new();
    for (text, _) in texts(&rig) {
        check("wall section", &text, &BANNED, &mut found);
        check("wall section", &text, &BANNED_IN_WALLS, &mut found);
    }
    assert!(found.is_empty(), "wall wording:\n{}", found.join("\n"));
}
