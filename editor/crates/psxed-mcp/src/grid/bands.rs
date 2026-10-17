//! Horizontal wall bands: cut lines, per-band material and tiling.

use psxed_project::face_uv::UvMode;
use psxed_project::grid::edit::bands::{
    anchor_height_at, clear_bands, edit_bands, effective_set, insert_index, line_ends, run_len,
    wall_spans, BandScope,
};
use psxed_project::grid::edit::UvOp;
use psxed_project::grid::{Anchor, BandV, GridWorld};
use schemars::JsonSchema;
use serde::Deserialize;

use super::walls::{resolve_wall, WallRef};
use super::{face_label, Materials};

/// Whether an edit reaches one run or the whole room's set.
#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// Just the wall's run: its walls get their own copy of the set when
    /// others share it. The default.
    #[default]
    Run,
    /// The set the wall uses, wherever the room uses it (and, for a wall
    /// with no bands yet, the room default).
    Room,
}

impl From<Scope> for BandScope {
    fn from(scope: Scope) -> Self {
        match scope {
            Scope::Run => Self::Run,
            Scope::Room => Self::Room,
        }
    }
}

/// Anchor kind of a cut line.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnchorKind {
    /// Above the wall's bottom edge: a trim that follows a sloped floor.
    Bottom,
    /// Below the wall's top edge: a frieze that hangs from a ceiling.
    Top,
    /// An absolute room-relative height: a datum line across terrain.
    Abs,
}

impl AnchorKind {
    fn make(self, value: i32) -> Anchor {
        match self {
            Self::Bottom => Anchor::Bottom(value),
            Self::Top => Anchor::Top(value),
            Self::Abs => Anchor::Abs(value),
        }
    }
}

fn describe_set(
    world: &GridWorld,
    face: psxed_project::grid::edit::FaceRef,
    mats: &Materials<'_>,
) -> String {
    let Some((index, set)) = effective_set(world, face) else {
        return "no bands (the wall is one plain face)".to_string();
    };
    let ends = line_ends(world, face);
    let mut out = format!(
        "band set {index} {:?}: {} line(s), {} band(s); applies to {} wall segment(s)\n",
        set.name,
        set.lines.len(),
        set.bands.len(),
        run_len(world, face)
    );
    // Top to bottom, as the Inspector lists them.
    for band in (0..set.bands.len()).rev() {
        let b = &set.bands[band];
        out.push_str(&format!(
            "band {band}: material {}, tiling {}, v {}\n",
            mats.label(b.material),
            if b.uv.mode == UvMode::Inherit && b.uv.is_identity() {
                "default".to_string()
            } else {
                format!("{:?}", b.uv.mode)
            },
            b.v.label()
        ));
        if band > 0 {
            let line = band - 1;
            let at = ends
                .get(line)
                .map_or(String::new(), |e| format!(" (heights at the ends {:?})", e));
            out.push_str(&format!("  line {line}: {:?}{at}\n", set.lines[line]));
        }
    }
    out
}

/// Describe the bands of a wall.
pub fn bands(world: &GridWorld, mats: &Materials<'_>, wall: &WallRef) -> Result<String, String> {
    let face = resolve_wall(world, wall)?;
    let mut out = format!("{}\n", face_label(&face));
    if let Some((span, _)) = wall_spans(world, face) {
        out.push_str(&format!(
            "span: u {:?}, bottom {:?}, top {:?} (room-relative grid units)\n",
            span.u, span.bottom, span.top
        ));
    }
    out.push_str(&describe_set(world, face, mats));
    let room = &world.rooms[face.room];
    out.push_str(&format!(
        "room has {} band set(s); room default set: {:?}\n",
        room.band_sets.len(),
        room.defaults.bands
    ));
    Ok(out)
}

/// Cut a wall run with a horizontal line.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BandCutReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// The wall.
    #[serde(flatten)]
    pub wall: WallRef,
    /// `bottom`, `top` or `abs`.
    pub anchor: AnchorKind,
    /// Grid units: above the wall's bottom (`bottom`), below its top
    /// (`top`), or the room-relative height (`abs`).
    pub value: i32,
    /// `run` (default) or `room`.
    pub scope: Option<Scope>,
}

/// Cut. At most six lines a set.
pub fn band_cut(
    world: &mut GridWorld,
    mats: &Materials<'_>,
    req: &BandCutReq,
) -> Result<String, String> {
    let face = resolve_wall(world, &req.wall)?;
    let anchor = req.anchor.make(req.value);
    let (span, _) = wall_spans(world, face).ok_or("that wall has no span")?;
    let u = (span.u[0] + span.u[1]) / 2;
    let y = anchor_height_at(world, face, anchor, u).ok_or("the cut has no height")?;
    let index = insert_index(world, face, u, y);
    let scope = req.scope.unwrap_or_default();
    if !edit_bands(world, face, scope.into(), |set| {
        set.insert_line(index, anchor)
    }) {
        return Err("a band set holds at most six lines".to_string());
    }
    Ok(format!(
        "cut {} at {anchor:?} (about height {y} at the middle), line {index}\n{}",
        face_label(&face),
        describe_set(world, face, mats)
    ))
}

/// Move a cut line.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BandMoveReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// The wall.
    #[serde(flatten)]
    pub wall: WallRef,
    /// Which line, bottom up from 0 (grid_bands lists them).
    pub line: usize,
    /// New value in grid units (see grid_band_cut).
    pub value: i32,
    /// Change the anchor kind too. Omit to keep it.
    pub anchor: Option<AnchorKind>,
    /// `run` (default) or `room`.
    pub scope: Option<Scope>,
}

/// Move the line.
pub fn band_move(
    world: &mut GridWorld,
    mats: &Materials<'_>,
    req: &BandMoveReq,
) -> Result<String, String> {
    let face = resolve_wall(world, &req.wall)?;
    let (_, set) = effective_set(world, face).ok_or("that wall has no bands")?;
    let old = *set
        .lines
        .get(req.line)
        .ok_or_else(|| format!("no line {} (the set has {})", req.line, set.lines.len()))?;
    let anchor = match req.anchor {
        Some(kind) => kind.make(req.value),
        None => old.with_value(req.value),
    };
    let scope = req.scope.unwrap_or_default();
    if !edit_bands(world, face, scope.into(), |set| {
        set.set_line(req.line, anchor)
    }) {
        return Err("the line is already there".to_string());
    }
    Ok(format!(
        "line {} moved from {old:?} to {anchor:?}\n{}",
        req.line,
        describe_set(world, face, mats)
    ))
}

/// Delete a cut line, or every band.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BandDeleteReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// The wall.
    #[serde(flatten)]
    pub wall: WallRef,
    /// The line to remove; the two bands around it join (the lower band's
    /// material wins). Omit to take ALL bands off the run.
    pub line: Option<usize>,
    /// `run` (default) or `room`.
    pub scope: Option<Scope>,
}

/// Delete.
pub fn band_delete(
    world: &mut GridWorld,
    mats: &Materials<'_>,
    req: &BandDeleteReq,
) -> Result<String, String> {
    let face = resolve_wall(world, &req.wall)?;
    let scope = req.scope.unwrap_or_default();
    match req.line {
        Some(line) => {
            if !edit_bands(world, face, scope.into(), |set| set.remove_line(line)) {
                return Err(format!("no line {line} on that wall"));
            }
            Ok(format!(
                "removed line {line}\n{}",
                describe_set(world, face, mats)
            ))
        }
        None => {
            if !clear_bands(world, face, scope.into()) {
                return Err("that wall has no bands to clear".to_string());
            }
            Ok("bands removed".to_string())
        }
    }
}

/// Style one band: its material, tiling and where V sits.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BandSetReq {
    /// Scene index. Omit for the first scene with a grid world.
    pub scene: Option<usize>,
    /// The wall.
    #[serde(flatten)]
    pub wall: WallRef,
    /// Which band, bottom up from 0. A wall with no bands has band 0 only,
    /// which this creates (a one band set in the wall's own material).
    pub band: usize,
    /// Material name or id; "none" keeps the wall's own.
    pub material: Option<String>,
    /// `world`, `fit` or `inherit`.
    pub mode: Option<String>,
    /// For `world`: texels per sector. Default 32.
    pub density: Option<[u16; 2]>,
    /// For `fit`: tiles across the run horizontally and between the two
    /// lines vertically.
    pub repeats: Option<[f32; 2]>,
    /// Quarter turns clockwise, 0..3.
    pub turns: Option<u8>,
    /// Where V = 0 sits in a world mapping: `world`, `from_bottom` (the
    /// texture's top row on the band's lower line, so a trim sits on the
    /// floor) or `from_top`.
    pub v: Option<String>,
    /// `run` (default) or `room`.
    pub scope: Option<Scope>,
}

/// Apply [`BandSetReq`].
pub fn band_set(
    world: &mut GridWorld,
    mats: &Materials<'_>,
    req: &BandSetReq,
) -> Result<String, String> {
    let face = resolve_wall(world, &req.wall)?;
    let material = match req.material.as_deref() {
        None => None,
        Some(name) => Some(mats.resolve_opt(Some(name))?),
    };
    let mode = match req.mode.as_deref().map(str::to_ascii_lowercase).as_deref() {
        None => None,
        Some("inherit") => Some(UvMode::Inherit),
        Some("world") => Some(UvMode::World {
            texels_per_sector: req.density.unwrap_or([32, 32]),
        }),
        Some("fit") => {
            let [h, v] = req.repeats.unwrap_or([1.0, 1.0]);
            let q = |x: f32| (x * 16.0).round().clamp(1.0, 256.0) as u16;
            Some(UvMode::Fit {
                repeats_q4: [q(h), q(v)],
            })
        }
        Some(other) => return Err(format!("unknown mode {other:?}; use world, fit or inherit")),
    };
    let v = match req.v.as_deref().map(str::to_ascii_lowercase).as_deref() {
        None => None,
        Some("world") => Some(BandV::World),
        Some("from_bottom") => Some(BandV::FromBottom),
        Some("from_top") => Some(BandV::FromTop),
        Some(other) => {
            return Err(format!(
                "unknown v {other:?}; use world, from_bottom or from_top"
            ))
        }
    };
    let scope = req.scope.unwrap_or_default();
    let band = req.band;
    let turns = req.turns;
    let changed = edit_bands(world, face, scope.into(), |set| {
        let Some(b) = set.bands.get_mut(band) else {
            return false;
        };
        let before = b.clone();
        if let Some(m) = material {
            b.material = m;
        }
        if let Some(mode) = mode {
            UvOp::SetMode(mode).apply(&mut b.uv);
        }
        if let Some(turns) = turns {
            b.uv.turns = turns & 3;
        }
        if let Some(v) = v {
            b.v = v;
        }
        *b != before
    });
    if !changed {
        let count = effective_set(world, face).map_or(1, |(_, set)| set.bands.len());
        if band >= count {
            return Err(format!(
                "no band {band}: the wall has {count} band(s) (a wall with no bands has band 0 only)"
            ));
        }
        return Ok(format!(
            "band {band} already has that style\n{}",
            describe_set(world, face, mats)
        ));
    }
    Ok(format!(
        "band {band} styled\n{}",
        describe_set(world, face, mats)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::grid::test_support::fixture;

    fn north_wall() -> WallRef {
        WallRef {
            room: None,
            cell: [2, 0],
            edge: "north".into(),
            kind: None,
            index: None,
        }
    }

    #[test]
    fn a_wall_is_cut_into_three_bands_with_three_materials() {
        let (p, mut w) = fixture();
        let mats = Materials::new(&p);
        band_cut(
            &mut w,
            &mats,
            &BandCutReq {
                scene: None,
                wall: north_wall(),
                anchor: AnchorKind::Bottom,
                value: 512,
                scope: Some(Scope::Room),
            },
        )
        .unwrap();
        band_cut(
            &mut w,
            &mats,
            &BandCutReq {
                scene: None,
                wall: north_wall(),
                anchor: AnchorKind::Top,
                value: 512,
                scope: Some(Scope::Room),
            },
        )
        .unwrap();
        let room = &w.rooms[0];
        let set = &room.band_sets[usize::from(room.defaults.bands.unwrap())];
        assert_eq!(set.bands.len(), 3);
        for (band, name) in [
            "Graybox / Cliff 64",
            "Graybox / Original 64",
            "Graybox / Ground 64",
        ]
        .iter()
        .enumerate()
        {
            band_set(
                &mut w,
                &mats,
                &BandSetReq {
                    scene: None,
                    wall: north_wall(),
                    band,
                    material: Some((*name).to_string()),
                    mode: None,
                    density: None,
                    repeats: None,
                    turns: None,
                    v: None,
                    scope: Some(Scope::Room),
                },
            )
            .unwrap();
        }
        let room = &w.rooms[0];
        let set = &room.band_sets[usize::from(room.defaults.bands.unwrap())];
        let ids: std::collections::HashSet<_> = set.bands.iter().map(|b| b.material).collect();
        assert_eq!(ids.len(), 3, "three different materials");
        let text = bands(&w, &mats, &north_wall()).unwrap();
        assert!(text.contains("3 band(s)"), "{text}");
        band_delete(
            &mut w,
            &mats,
            &BandDeleteReq {
                scene: None,
                wall: north_wall(),
                line: Some(0),
                scope: Some(Scope::Room),
            },
        )
        .unwrap();
    }
}
