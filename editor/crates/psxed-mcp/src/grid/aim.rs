//! Which span of a column a surface or level tool edits.
//!
//! A column of several spans has several floors, and the tools that make and
//! shape surfaces act on one. They take `y` (grid units relative to the room)
//! or `storey` (an ordinal of `grid_storeys`) to say which: the span open at
//! that height is the one edited. With neither, a column of one span is edited
//! as always and a column of several is an error that lists what to pass
//! (`docs/grid/multi-storey.md`, 6.1: ambiguity is an error, not a guess).

use psxed_project::grid::edit::focus::Focus;
use psxed_project::grid::edit::CellRect;
use psxed_project::grid::storeys::storeys;
use psxed_project::grid::GridWorld;
use schemars::JsonSchema;
use serde::Deserialize;

use super::storeys::refuse_several_spans_in;

/// The span of a column a tool edits.
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct SpanAim {
    /// Edit the span open at this height: grid units relative to the room
    /// (256 a click), its floor at or below and its ceiling above. Needed
    /// where a column has several spans; omit for single-span columns.
    pub y: Option<i32>,
    /// The same as `y`, by the ordinal of a storey in `grid_storeys`: its
    /// floor height. Give one of the two, not both.
    pub storey: Option<usize>,
}

impl SpanAim {
    /// The room-relative height the aim names, if any.
    fn height(&self, world: &GridWorld, room: usize) -> Result<Option<i32>, String> {
        match (self.y, self.storey) {
            (Some(_), Some(_)) => Err("give y or storey, not both".to_string()),
            (Some(y), None) => Ok(Some(y)),
            (None, Some(ordinal)) => {
                let list = storeys(world);
                let storey = list.get(ordinal).ok_or_else(|| {
                    format!(
                        "no storey {ordinal}: grid_storeys lists {} storey(s)",
                        list.len()
                    )
                })?;
                let offset = world.rooms.get(room).map_or(0, |r| r.y_offset);
                Ok(Some(storey.height - offset))
            }
            (None, None) => Ok(None),
        }
    }
}

/// The focus an aim names over `cells`, or `None` without an aim (columns of
/// one span are used as they are, a column of several is an error).
pub fn aim_focus(
    world: &GridWorld,
    room: usize,
    cells: impl IntoIterator<Item = [i32; 2]>,
    aim: &SpanAim,
) -> Result<Option<Focus>, String> {
    let cells: Vec<[i32; 2]> = cells.into_iter().collect();
    let Some(height) = aim.height(world, room)? else {
        refuse_several_spans_in(world, room, cells.iter().copied())?;
        return Ok(None);
    };
    Focus::capture(world, room, cells, Some(height))
        .map(Some)
        .map_err(|refusal| refusal.describe())
}

/// Run `edit` on the span `aim` names in every column of `rect`, or, with no
/// aim, on the columns as they are when none holds several spans. A span the
/// edit leaves too thin to stand in is filled solid and the reply says so.
pub fn aimed(
    world: &mut GridWorld,
    room: usize,
    rect: CellRect,
    aim: &SpanAim,
    edit: impl FnOnce(&mut GridWorld) -> Result<String, String>,
) -> Result<String, String> {
    aimed_cells(world, room, rect.cells(), aim, edit)
}

/// [`aimed`] over any set of world sectors (a path, a ring).
pub fn aimed_cells(
    world: &mut GridWorld,
    room: usize,
    cells: impl IntoIterator<Item = [i32; 2]>,
    aim: &SpanAim,
    edit: impl FnOnce(&mut GridWorld) -> Result<String, String>,
) -> Result<String, String> {
    let cells: Vec<[i32; 2]> = cells.into_iter().collect();
    let Some(height) = aim.height(world, room)? else {
        // No aim: columns of one span are edited as they always were, and a
        // column of several is an error that lists the way out.
        refuse_several_spans_in(world, room, cells.iter().copied())?;
        return edit(world);
    };
    let focus =
        Focus::capture(world, room, cells, Some(height)).map_err(|refusal| refusal.describe())?;
    let done = focus
        .run(world, edit)
        .map_err(|refusal| refusal.describe())?;
    let mut text = done.value?;
    if done.slivers > 0 {
        text.push_str(&format!(
            "\n{} span(s) left under {} clicks of headroom were filled solid",
            done.slivers,
            psxed_project::grid::storeys::MIN_HEADROOM / psxed_project::grid::CLICK_UNITS
        ));
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use psxed_project::grid::edit::spans::insert_slab_column;
    use psxed_project::grid::{GridRoom, GridScale, GridSector, GridSurface};
    use serde_json::json;

    /// A 4 by 3 derived room whose every column is cut into two spans by a
    /// slab at 1536: two storeys, at 0 and 1536.
    fn two_storeys() -> GridWorld {
        let mut room = GridRoom::new_derived("hall", 4, 3);
        for z in 0..3 {
            for x in 0..4 {
                let mut column = GridSector::with_surfaces(
                    Some(GridSurface::flat(0, None)),
                    Some(GridSurface::flat(3072, None)),
                );
                insert_slab_column(&mut column, 1536, 256, None).unwrap();
                *room.sector_or_insert(x, z).unwrap() = column;
            }
        }
        GridWorld {
            scale: GridScale { num: 1, den: 1 },
            rooms: vec![room],
            ..GridWorld::default()
        }
    }

    fn floors(world: &GridWorld, x: usize, z: usize) -> Vec<i32> {
        world.rooms[0]
            .sector(x, z)
            .unwrap()
            .spans()
            .iter()
            .filter_map(|s| s.floor.as_ref().map(|f| f.heights[0]))
            .collect()
    }

    fn rect() -> CellRect {
        CellRect::from_corners([0, 0], [3, 2])
    }

    fn lift(aim: &SpanAim, world: &mut GridWorld) -> Result<String, String> {
        aimed(world, 0, rect(), aim, |world| {
            for z in 0..3 {
                for x in 0..4 {
                    let sector = world.rooms[0].sector_mut(x, z).unwrap();
                    sector.ground_floor_mut().unwrap().heights = [1792; 4];
                }
            }
            Ok(String::new())
        })
    }

    #[test]
    fn without_an_aim_a_column_of_several_spans_is_refused_with_the_way_out() {
        let mut world = two_storeys();
        let before = world.clone();
        let refused = lift(&SpanAim::default(), &mut world).unwrap_err();
        assert!(refused.contains("has 2 spans"), "{refused}");
        assert!(refused.contains("storey"), "{refused}");
        assert_eq!(world, before);
    }

    #[test]
    fn y_and_storey_aim_the_edit_at_the_span_open_there() {
        for aim in [
            SpanAim {
                y: Some(1536),
                storey: None,
            },
            SpanAim {
                y: None,
                storey: Some(1),
            },
        ] {
            let mut world = two_storeys();
            lift(&aim, &mut world).unwrap();
            // The upper floor moved, the ground floor did not.
            assert_eq!(floors(&world, 2, 1), vec![0, 1792], "{aim:?}");
        }
    }

    #[test]
    fn a_bad_aim_is_an_error_and_changes_nothing() {
        let mut world = two_storeys();
        let before = world.clone();
        let both = SpanAim {
            y: Some(0),
            storey: Some(0),
        };
        assert!(lift(&both, &mut world).unwrap_err().contains("not both"));
        let far = SpanAim {
            y: None,
            storey: Some(9),
        };
        assert!(lift(&far, &mut world).unwrap_err().contains("no storey 9"));
        // Inside the slab nothing is open.
        let inside = SpanAim {
            y: Some(1400),
            storey: None,
        };
        let refused = lift(&inside, &mut world).unwrap_err();
        assert!(refused.contains("no span open"), "{refused}");
        assert_eq!(world, before);
    }

    #[test]
    fn the_surface_tools_take_y_or_storey_in_their_requests() {
        use super::super::build::{set_heights, SetHeightsReq};
        let mut world = two_storeys();
        let req: SetHeightsReq = serde_json::from_value(json!({
            "kind": "floor", "from": [0, 0], "to": [3, 2], "height": 1792, "storey": 1
        }))
        .unwrap();
        set_heights(&mut world, &req).unwrap();
        assert_eq!(floors(&world, 0, 0), vec![0, 1792]);
        // The same request without the aim is refused, naming a cell.
        let bare: SetHeightsReq = serde_json::from_value(json!({
            "kind": "floor", "from": [0, 0], "to": [3, 2], "height": 2048
        }))
        .unwrap();
        let refused = set_heights(&mut world, &bare).unwrap_err();
        assert!(refused.contains("has 2 spans"), "{refused}");
        assert_eq!(floors(&world, 0, 0), vec![0, 1792]);
    }
}
