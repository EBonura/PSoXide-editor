//! Grid geometry tools.
//!
//! Every function here is a thin wrapper over the edit functions the grid
//! editor itself calls (`psxed_project::grid::edit`), so a level built through
//! the MCP is made by exactly the code a mouse gesture reaches. Nothing in this
//! module re-implements an edit: it parses a request, calls the editor
//! function, and reports what happened in numbers the agent can check.
//!
//! Units, everywhere a request or a report mentions one:
//!
//! * a **sector** is 1024 grid units; cells are addressed by their WORLD sector
//!   `[x, z]`, the same lattice in every room;
//! * heights are **grid units**, relative to their room's `y_offset`. A click
//!   is 256 and the fine step is 64; heights always sit on the 64 lattice;
//! * the world's scale (5/4 for a new project) turns grid units into editor
//!   units, which is what nodes and screenshots use.

pub mod bands;
pub mod build;
pub mod paint;
pub mod plan;
pub mod planes;
pub mod read;
pub mod rooms;
pub mod verbs;
pub mod walls;

use std::collections::BTreeSet;
use std::fmt::Write as _;

use psxed_project::grid::edit::{CellRect, FaceRef};
use psxed_project::grid::limits::{self, GridIssue};
use psxed_project::grid::units::{snap_to, CLICK_UNITS, FINE_UNITS};
use psxed_project::grid::{GridWorld, WallEdge};
use psxed_project::{ProjectDocument, ResourceData, ResourceId};
use schemars::JsonSchema;
use serde::Deserialize;

/// A rectangle of world sectors, both corners inclusive, in any order.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema)]
pub struct Cells {
    /// One corner, WORLD sector `[x, z]`.
    pub from: [i32; 2],
    /// The opposite corner, inclusive. Omit for the single cell `from`.
    pub to: Option<[i32; 2]>,
}

impl Cells {
    /// The rectangle as the editor's `CellRect`.
    pub fn rect(&self) -> CellRect {
        CellRect::from_corners(self.from, self.to.unwrap_or(self.from))
    }
}

/// Largest rectangle a single call will walk, so a typo cannot freeze a tool.
pub const MAX_CELLS_PER_CALL: usize = 4096;

/// Refuse rectangles too large to be deliberate.
pub fn check_rect(rect: &CellRect) -> Result<(), String> {
    if rect.count() > MAX_CELLS_PER_CALL {
        return Err(format!(
            "that rectangle is {} sectors ({}x{}); one call handles at most {MAX_CELLS_PER_CALL}",
            rect.count(),
            rect.width(),
            rect.depth()
        ));
    }
    Ok(())
}

/// Which snap a height value goes through before it is stored.
#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Snap {
    /// Nearest click, 256 grid units. The editor's default vertical snap.
    #[default]
    Click,
    /// Nearest fine step, 64 grid units (Alt in the editor).
    Fine,
}

impl Snap {
    /// The step in grid units.
    pub const fn step(self) -> i32 {
        match self {
            Self::Click => CLICK_UNITS,
            Self::Fine => FINE_UNITS,
        }
    }

    /// Snap `value`, returning the stored value and a note when it moved.
    pub fn apply(self, value: i32) -> (i32, Option<String>) {
        let snapped = snap_to(value, self.step());
        let note = (snapped != value).then(|| {
            format!(
                "{value} snapped to {snapped} ({} step of {})",
                match self {
                    Self::Click => "click",
                    Self::Fine => "fine",
                },
                self.step()
            )
        });
        (snapped, note)
    }
}

/// The room a request means: its index, or the only room there is.
pub fn pick_room(world: &GridWorld, room: Option<usize>) -> Result<usize, String> {
    match room {
        Some(index) if index < world.rooms.len() => Ok(index),
        Some(index) => Err(format!(
            "room {index} does not exist; the world has {} room(s), 0..{}. Call grid_rooms.",
            world.rooms.len(),
            world.rooms.len().saturating_sub(1)
        )),
        None if world.rooms.len() == 1 => Ok(0),
        None => Err(format!(
            "room is required: the world has {} rooms. Call grid_rooms.",
            world.rooms.len()
        )),
    }
}

/// Parse a wall edge name: north, east, south, west, nwse, nesw.
pub fn parse_edge(name: &str) -> Result<WallEdge, String> {
    match name.to_ascii_lowercase().as_str() {
        "n" | "north" => Ok(WallEdge::North),
        "e" | "east" => Ok(WallEdge::East),
        "s" | "south" => Ok(WallEdge::South),
        "w" | "west" => Ok(WallEdge::West),
        "nwse" | "diagonal_nwse" => Ok(WallEdge::Nwse),
        "nesw" | "diagonal_nesw" => Ok(WallEdge::Nesw),
        other => Err(format!(
            "unknown edge {other:?}; use north, east, south, west, nwse or nesw (north is -Z, east is +X)"
        )),
    }
}

/// Parse a cardinal edge only.
pub fn parse_cardinal(name: &str) -> Result<WallEdge, String> {
    let edge = parse_edge(name)?;
    if edge.is_diagonal() {
        return Err("this needs a cardinal edge: north, east, south or west".to_string());
    }
    Ok(edge)
}

/// Resolves material names against a project.
///
/// A name matches exactly, then as a unique case-insensitive substring; a
/// numeric string is a resource id. Anything else is an error that names a
/// few plausible candidates, not all 130 materials a project carries.
#[derive(Clone, Copy)]
pub struct Materials<'a> {
    project: &'a ProjectDocument,
}

impl<'a> Materials<'a> {
    /// A resolver over `project`.
    pub fn new(project: &'a ProjectDocument) -> Self {
        Self { project }
    }

    /// The material `needle` names.
    pub fn resolve(&self, needle: &str) -> Result<ResourceId, String> {
        let materials = || {
            self.project
                .resources
                .iter()
                .filter(|r| matches!(r.data, ResourceData::Material(_)))
        };
        if let Ok(raw) = needle.trim().parse::<u64>() {
            if let Some(found) = materials().find(|r| r.id.raw() == raw) {
                return Ok(found.id);
            }
        }
        if let Some(found) = materials().find(|r| r.name == needle) {
            return Ok(found.id);
        }
        let lowered = needle.to_ascii_lowercase();
        let hits: Vec<_> = materials()
            .filter(|r| r.name.to_ascii_lowercase().contains(&lowered))
            .collect();
        match hits.as_slice() {
            [only] => Ok(only.id),
            [] => {
                let words: Vec<String> = lowered
                    .split(|c: char| !c.is_alphanumeric())
                    .filter(|w| w.len() > 2)
                    .map(str::to_string)
                    .collect();
                let near: Vec<&str> = materials()
                    .filter(|r| {
                        let name = r.name.to_ascii_lowercase();
                        words.iter().any(|w| name.contains(w.as_str()))
                    })
                    .map(|r| r.name.as_str())
                    .take(8)
                    .collect();
                Err(format!(
                    "no material matches {needle:?}.{} Call `materials` (palette_only: true) for the world palette.",
                    if near.is_empty() {
                        String::new()
                    } else {
                        format!(" Close names: {}.", near.join(", "))
                    }
                ))
            }
            many => Err(format!(
                "{needle:?} is ambiguous between: {}",
                many.iter()
                    .take(10)
                    .map(|r| format!("{:?} (id {})", r.name, r.id.raw()))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }

    /// Resolve an optional name, treating "none" or an empty string as the
    /// untextured fallback.
    pub fn resolve_opt(&self, needle: Option<&str>) -> Result<Option<ResourceId>, String> {
        match needle.map(str::trim) {
            None => Ok(None),
            Some("") | Some("none") => Ok(None),
            Some(name) => self.resolve(name).map(Some),
        }
    }

    /// `"name (id)"`, or `none`.
    pub fn label(&self, id: Option<ResourceId>) -> String {
        match id {
            None => "none".to_string(),
            Some(id) => self.project.resource(id).map_or_else(
                || format!("<missing {}>", id.raw()),
                |r| format!("{} ({})", r.name, id.raw()),
            ),
        }
    }

    /// Just the name, for tight tables.
    pub fn short(&self, id: Option<ResourceId>) -> String {
        match id {
            None => "-".to_string(),
            Some(id) => self
                .project
                .resource(id)
                .map_or_else(|| format!("#{}", id.raw()), |r| r.name.clone()),
        }
    }
}

/// Validation messages, as `Vec<String>`.
pub fn issue_texts(world: &GridWorld) -> Vec<String> {
    limits::validate(world)
        .iter()
        .map(|issue: &GridIssue| {
            let severity = match issue.severity() {
                limits::Severity::Error => "error",
                limits::Severity::Warning => "warning",
            };
            format!("{severity}: {}", issue.describe())
        })
        .collect()
}

/// New validation findings an edit introduced, for appending to its report.
pub fn new_issues(before: &BTreeSet<String>, world: &GridWorld) -> String {
    let fresh: Vec<String> = issue_texts(world)
        .into_iter()
        .filter(|text| !before.contains(text))
        .collect();
    if fresh.is_empty() {
        return String::new();
    }
    let mut out = String::from("\n\nThis edit introduced validation findings:");
    for text in fresh.iter().take(6) {
        let _ = write!(out, "\n- {text}");
    }
    if fresh.len() > 6 {
        let _ = write!(
            out,
            "\n- ... and {} more (grid_budget lists all)",
            fresh.len() - 6
        );
    }
    out
}

/// A compact label for a face: `floor`, `ceiling`, `north wall 0`,
/// `south riser`.
pub fn face_label(face: &FaceRef) -> String {
    use psxed_project::grid::edit::{FaceTarget, SurfaceKind};
    match face.target {
        FaceTarget::Surface(SurfaceKind::Floor) => "floor".to_string(),
        FaceTarget::Surface(SurfaceKind::Ceiling) => "ceiling".to_string(),
        FaceTarget::Wall { edge, index } => format!("{} wall {index}", edge.label()),
        FaceTarget::Derived { edge, kind } => format!("{} {}", edge.label(), kind.label()),
    }
}

/// Heights in clicks, one decimal at most: 512 -> `2`, 320 -> `1.25`.
pub fn clicks(height: i32) -> String {
    let value = f64::from(height) / f64::from(CLICK_UNITS);
    if (value - value.round()).abs() < 1e-9 {
        format!("{}", value.round() as i64)
    } else {
        format!("{value:.2}")
    }
}

/// Fixtures shared by the unit tests.
#[cfg(test)]
pub(crate) mod test_support {
    use psxed_project::grid::starter;
    use psxed_project::ProjectDocument;

    use super::GridWorld;

    /// What File > New Grid Project makes, in memory: the Graybox Reach base
    /// with the starter room, so the Graybox materials are there to paint with.
    pub fn new_grid_project() -> ProjectDocument {
        let base = psxed_project::projects_dir().join(starter::STARTER_BASE);
        let mut project = ProjectDocument::load_from_path(base.join("project.ron"))
            .expect("the graybox-reach base project loads");
        starter::into_starter(&mut project).expect("the base has a floor to stand on");
        project
    }

    /// The project and its grid world.
    pub fn fixture() -> (ProjectDocument, GridWorld) {
        let project = new_grid_project();
        let world = project
            .active_scene()
            .world
            .grid()
            .cloned()
            .expect("a grid world");
        (project, world)
    }
}
