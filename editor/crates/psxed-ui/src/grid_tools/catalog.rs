//! The one table of grid tools: their order on the strip, their group and the
//! hint the status bar shows for each. The strip, the `?` page and the status
//! bar all read it, so a tool added or moved here moves everywhere.

use super::{GridTool, ToolGroup};

/// One tool's place on the strip.
pub(crate) struct ToolEntry {
    pub tool: GridTool,
    pub group: ToolGroup,
    /// The keys and gestures of the tool, as one line for the status bar.
    pub hint: &'static str,
}

/// Every tool, in strip order.
pub(crate) const CATALOG: [ToolEntry; 33] = [
    ToolEntry {
        tool: GridTool::Edit,
        group: ToolGroup::Edit,
        hint: "Hover shows the drag | Click select | Shift+click range | Ctrl toggle | Shift+drag box | double-click room or run | type a number, Enter | R repeat | ` cycle | Esc cancel",
    },
    ToolEntry {
        tool: GridTool::Select,
        group: ToolGroup::Edit,
        hint: "Click pick | Shift range | Ctrl toggle | Drag box | Shift-drag height | Del delete | Ctrl+C/V copy",
    },
    ToolEntry {
        tool: GridTool::Height,
        group: ToolGroup::Edit,
        hint: "Drag up/down | Alt fine | Alt+drag a face: one triangle | L weld | PgUp/PgDn nudge | Esc cancel",
    },
    ToolEntry {
        tool: GridTool::Extrude,
        group: ToolGroup::Edit,
        hint: "Press a floor, ceiling or wall and drag (walls: along the normal) | Shift-drag a box picks many, Shift+Alt adds | Click Shift adds | = out, - in | Tab Offset/Level | Alt fine | Esc cancel",
    },
    ToolEntry {
        tool: GridTool::Room,
        group: ToolGroup::Build,
        hint: "Drag a rectangle | Shift+drag up or type a number for the height | Enter finishes a lasso | Esc cancel",
    },
    ToolEntry {
        tool: GridTool::Corridor,
        group: ToolGroup::Build,
        hint: "Click waypoints, Enter finishes | Click a higher floor for a ramp or stairs | type a number for the width | Esc cancel",
    },
    ToolEntry {
        tool: GridTool::Opening,
        group: ToolGroup::Build,
        hint: "Drag along a wall | Shift+drag up or type a number for the lintel | Ctrl+drag up for a window sill | Esc cancel",
    },
    ToolEntry {
        tool: GridTool::Arcade,
        group: ToolGroup::Build,
        hint: "Drag a line on the floor | type a number for the bays | Esc cancel",
    },
    ToolEntry {
        tool: GridTool::Colonnade,
        group: ToolGroup::Build,
        hint: "Drag a line on the floor | type a number for the bays | Esc cancel",
    },
    ToolEntry {
        tool: GridTool::Bridge,
        group: ToolGroup::Build,
        hint: "Click the two ends of the deck | type a number for the width | Esc cancel",
    },
    ToolEntry {
        tool: GridTool::Building,
        group: ToolGroup::Build,
        hint: "Drag a footprint | type a number for the storeys | Esc cancel",
    },
    ToolEntry {
        tool: GridTool::Cathedral,
        group: ToolGroup::Build,
        hint: "Drag the nave: its length sets the bays, its depth the width | type a number for the bay size | Esc cancel",
    },
    ToolEntry {
        tool: GridTool::Stairs,
        group: ToolGroup::Build,
        hint: "Drag a rectangle | Press on it, drag up | R turn the bottom side | Path: click waypoints, Enter, [ ] width | Tab steps/ramp | Esc cancel",
    },
    ToolEntry {
        tool: GridTool::Ramp,
        group: ToolGroup::Build,
        hint: "Drag a rectangle, or use your selection | Press near the side to keep, drag up/down | Path and Lasso close when you press on them | R turn the side | Tab hold to walkable | Esc cancel",
    },
    ToolEntry {
        tool: GridTool::Spiral,
        group: ToolGroup::Build,
        hint: "Drag a rectangle: the stairs wind round its outline | Press on it, drag up for the climb of a storey | Path: click the corners, Enter | Your selection works too | Storeys in the option row | Esc cancel",
    },
    ToolEntry {
        tool: GridTool::Slab,
        group: ToolGroup::Build,
        hint: "Drag a rectangle, or use your selection | Add floor above: press on them and drag up for the height | Remove upper floor, Fill opening: click them | Tab mode | Alt+Up/Down level | G go to level | Z cutaway | Esc cancel",
    },
    ToolEntry {
        tool: GridTool::Floor,
        group: ToolGroup::Build,
        hint: "Drag rectangle | Shift height | Ctrl delete | Esc cancel",
    },
    ToolEntry {
        tool: GridTool::Ceiling,
        group: ToolGroup::Build,
        hint: "Drag rectangle | Shift height | Ctrl delete | Esc cancel",
    },
    ToolEntry {
        tool: GridTool::Wall,
        group: ToolGroup::Build,
        hint: "Drag a run | R direction | Ctrl remove | Esc cancel",
    },
    ToolEntry {
        tool: GridTool::Plane,
        group: ToolGroup::Build,
        hint: "Click a surface to place a plane | Drag sizes it | Click a plane to select | Alt 64, Shift+Alt free",
    },
    ToolEntry {
        tool: GridTool::Mesh,
        group: ToolGroup::Build,
        hint: "Click a floor to place | Drag moves | R turns, [ ] 15 | - = scale | Del removes | Alt 64",
    },
    ToolEntry {
        tool: GridTool::Sculpt,
        group: ToolGroup::Terrain,
        hint: "Press and drag on the ground | Shift inverts | [ ] radius | Tab mode | Ramp: drag A to B | Dunes: drag across | Esc cancels the stroke",
    },
    ToolEntry {
        tool: GridTool::Break,
        group: ToolGroup::Terrain,
        hint: "Drag a rectangle, or use your selection | Press on it, drag up for intensity | Tab target | Shift+X re-roll | Enter commit | Esc cancel",
    },
    ToolEntry {
        tool: GridTool::Terrain,
        group: ToolGroup::Terrain,
        hint: "Drag a region | Shift+X or the dice re-roll the seed | type a number for the amplitude | Esc cancel",
    },
    ToolEntry {
        tool: GridTool::Ruin,
        group: ToolGroup::Terrain,
        hint: "Drag a region or take the whole room | Paint path keeps cells clear | dice re-roll | type a number for the intensity | Esc cancel",
    },
    ToolEntry {
        tool: GridTool::Cells,
        group: ToolGroup::Terrain,
        hint: "Paint to subdivide | Alt+paint merges | Shift+M merges the cell under the pointer | [ ] brush | Alt+L cell sizes",
    },
    ToolEntry {
        tool: GridTool::Split,
        group: ToolGroup::Terrain,
        hint: "Click a floor or ceiling flips its diagonal | Shift+click voids a triangle | Click again for the ceiling in Top",
    },
    ToolEntry {
        tool: GridTool::Paint,
        group: ToolGroup::Surface,
        hint: "Click or drag paint | R / Shift+R turn | M / Shift+M mirror | [ ] scale | Ctrl sample | pick in the palette",
    },
    ToolEntry {
        tool: GridTool::Bands,
        group: ToolGroup::Surface,
        hint: "Hover a wall, click to cut | Drag a line | Del removes it | Shift+click picks a band | Esc cancel",
    },
    ToolEntry {
        tool: GridTool::Place,
        group: ToolGroup::Objects,
        hint: "Click to place at the sector centre | Shift+click keeps placing | Alt 64 | Shift+Alt free",
    },
    ToolEntry {
        tool: GridTool::Door,
        group: ToolGroup::Objects,
        hint: "Drag along a wall: doorway and sliding door in one gesture | Shift+drag up or type a number for the height | Esc cancel",
    },
    ToolEntry {
        tool: GridTool::Lever,
        group: ToolGroup::Objects,
        hint: "Click a floor to place a lever, then click the door it operates | Esc cancel",
    },
    ToolEntry {
        tool: GridTool::Checkpoint,
        group: ToolGroup::Objects,
        hint: "Click a floor to place a checkpoint | Esc cancel",
    },
];

/// The tools of `group`, in strip order.
pub(crate) fn tools_of(group: ToolGroup) -> impl Iterator<Item = GridTool> {
    CATALOG
        .iter()
        .filter(move |e| e.group == group)
        .map(|e| e.tool)
}

/// The entry of `tool`.
pub(crate) const fn entry(tool: GridTool) -> &'static ToolEntry {
    let mut i = 0;
    while i < CATALOG.len() {
        if CATALOG[i].tool as usize == tool as usize {
            return &CATALOG[i];
        }
        i += 1;
    }
    panic!("every grid tool has a catalog entry")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_has_exactly_one_entry() {
        for tool in GridTool::ALL {
            let n = CATALOG.iter().filter(|e| e.tool == tool).count();
            assert_eq!(n, 1, "{tool:?}");
        }
    }

    #[test]
    fn the_groups_are_in_strip_order_without_gaps() {
        let mut seen: Vec<ToolGroup> = Vec::new();
        for e in &CATALOG {
            match seen.last() {
                Some(last) if *last == e.group => {}
                _ => {
                    assert!(!seen.contains(&e.group), "{:?} is split", e.group);
                    seen.push(e.group);
                }
            }
        }
        assert_eq!(seen, ToolGroup::ALL);
    }
}
