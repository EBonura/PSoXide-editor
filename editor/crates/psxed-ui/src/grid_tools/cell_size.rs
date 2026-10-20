//! The cell size of the tools that act on cells: a whole sector (the
//! default), a half or a quarter of one.
//!
//! The strip's option row picks it for Floor, Ceiling, Wall and the level
//! verbs. At a smaller size the tool snaps to cells of that size and runs the
//! same library calls the MCP makes with `size: half|quarter`: a rectangle of
//! cells of that level, leaves split to fit, and the verbs through the
//! resolution lens. The choice is editor state, not saved with the project.

use super::*;

use psxed_project::grid::leaf::edge_clicks;
use psxed_project::grid::units::CLICK_UNITS;
use psxed_project::grid::GridScale;

/// How big a cell is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum CellSize {
    /// A whole sector, 1024 grid units.
    #[default]
    Sector,
    /// Half a sector, 512 grid units.
    Half,
    /// A quarter of a sector, 256 grid units (one click).
    Quarter,
}

impl CellSize {
    pub(crate) const ALL: [Self; 3] = [Self::Sector, Self::Half, Self::Quarter];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Sector => "Sector",
            Self::Half => "1/2",
            Self::Quarter => "1/4",
        }
    }

    /// The cell level the library reads: 2 sectors, 3 halves, 4 quarters.
    pub(crate) const fn level(self) -> u8 {
        match self {
            Self::Sector => 2,
            Self::Half => 3,
            Self::Quarter => 4,
        }
    }
}

/// Whether the tool's options row shows the cell size.
pub(crate) const fn takes_size(tool: GridTool) -> bool {
    matches!(
        tool,
        GridTool::Floor
            | GridTool::Ceiling
            | GridTool::Wall
            | GridTool::Extrude
            | GridTool::Ramp
            | GridTool::Stairs
            | GridTool::Break
    )
}

/// Grid units across one cell of `level`.
pub(crate) fn cell_units(level: u8) -> i32 {
    edge_clicks(level) * CLICK_UNITS
}

/// The cell of `level` containing editor-space `(x, z)`. At a sector this is
/// the same cell the sector tools always read.
pub(crate) fn cell_at(scale: GridScale, x: f32, z: f32, level: u8) -> [i32; 2] {
    if level == 2 {
        return super::view::cell_at_editor(scale, x, z);
    }
    let click = super::view::click_at_editor(scale, x, z);
    let edge = edge_clicks(level);
    [click[0].div_euclid(edge), click[1].div_euclid(edge)]
}

/// The room-relative cell-level origin in grid units of cell `cell` of
/// `level`.
pub(crate) fn cell_origin_units(level: u8, cell: [i32; 2]) -> [i32; 2] {
    let unit = cell_units(level);
    [cell[0] * unit, cell[1] * unit]
}

/// Switch the cell size. A region picked at another size means other cells,
/// so it goes; the status line says so.
pub(crate) fn set(ws: &mut EditorWorkspace, size: CellSize) {
    if ws.grid.cell_size == size {
        return;
    }
    ws.grid.cell_size = size;
    if ws
        .grid
        .verb
        .region
        .as_ref()
        .is_some_and(|r| r.level() != size.level())
    {
        ws.grid.verb.clear();
    }
    ws.status = format!("Cell size {}", size.label());
}

impl EditorWorkspace {
    /// The Cell size selector, in the option row of the tools that take it.
    pub(crate) fn draw_cell_size_selector(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Cell").color(STUDIO_TEXT_WEAK).size(11.0));
        for size in CellSize::ALL {
            let on = self.grid.cell_size == size;
            let hint = match size {
                CellSize::Sector => "Whole sectors (the default)",
                CellSize::Half => "Half a sector: the tools snap to cells of 512 units",
                CellSize::Quarter => "A quarter sector: the tools snap to cells of 256 units",
            };
            if ui
                .selectable_label(on, size.label())
                .on_hover_text(hint)
                .clicked()
            {
                set(self, size);
            }
        }
        ui.separator();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use psxed_project::grid::SECTOR_UNITS;

    #[test]
    fn a_cell_is_a_sector_over_its_per_sector_count() {
        assert_eq!(cell_units(2), SECTOR_UNITS);
        assert_eq!(cell_units(3), SECTOR_UNITS / 2);
        assert_eq!(cell_units(4), SECTOR_UNITS / 4);
        assert_eq!(CellSize::default().level(), 2);
    }
}
