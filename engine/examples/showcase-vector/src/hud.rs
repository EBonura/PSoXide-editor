//! The stance HUD composed from `stance_hud.psvg`: how game-side code uses the
//! vector runtime. A sketch of the Cortex swap (cx-7); it lives in the
//! showcase until the live HUD state is wired in.
//!
//! The composition is written once against a `draw` callback, so the same code
//! drives an immediate GPU draw, a packet build for an ordering table, and a
//! CPU-only cost probe.

use psx_vector::{GroupId, Placement, Style, VectorAsset};

/// Scratch capacity: the HUD asset's largest group has 215 vertices.
pub const SCRATCH_VERTICES: usize = 256;

/// Group handles looked up once at load.
pub struct HudIds {
    plate: GroupId,
    health: [GroupId; 10],
    emblem: GroupId,
    emblem_core: GroupId,
    rail: GroupId,
    glow: [GroupId; 10],
    energy: [GroupId; 10],
}

impl HudIds {
    /// Resolve every group the HUD draws. `None` if the asset lacks one.
    pub fn new(asset: &VectorAsset<'_>) -> Option<Self> {
        let numbered = |prefix: &str| -> Option<[GroupId; 10]> {
            let mut out = [asset.group("plate")?; 10];
            let mut name = [0u8; 12];
            for (i, slot) in out.iter_mut().enumerate() {
                let p = prefix.as_bytes();
                name[..p.len()].copy_from_slice(p);
                name[p.len()] = b'0' + i as u8;
                let text = core::str::from_utf8(&name[..p.len() + 1]).ok()?;
                *slot = asset.group(text)?;
            }
            Some(out)
        };
        Some(Self {
            plate: asset.group("plate")?,
            health: numbered("health_")?,
            emblem: asset.group("emblem")?,
            emblem_core: asset.group("emblem_core")?,
            rail: asset.group("rail")?,
            glow: numbered("glow_")?,
            energy: numbered("energy_")?,
        })
    }
}

/// One stance's colour and how much of its bar is filled.
#[derive(Clone, Copy)]
pub struct Bar {
    /// Stance colour (the tint multiplied into the grey-authored art).
    pub rgb: [u8; 3],
    /// Lit cells, 0 to 10.
    pub cells: u8,
}

fn scaled(rgb: [u8; 3], k: u16) -> [u8; 3] {
    [
        ((rgb[0] as u16 * k) >> 8) as u8,
        ((rgb[1] as u16 * k) >> 8) as u8,
        ((rgb[2] as u16 * k) >> 8) as u8,
    ]
}

fn draw_bar(
    ids: &HudIds,
    place: &Placement,
    bar: Bar,
    dim: bool,
    draw: &mut impl FnMut(GroupId, &Placement, &Style),
) {
    let (body, lit, empty, saturation) = if dim {
        (scaled(bar.rgb, 90), scaled(bar.rgb, 100), scaled(bar.rgb, 36), 110)
    } else {
        (bar.rgb, bar.rgb, scaled(bar.rgb, 52), 255)
    };
    let style = |tint| Style { tint, saturation, ..Style::DEFAULT };
    draw(ids.plate, place, &style(body));
    for (i, &cell) in ids.health.iter().enumerate() {
        let tint = if (i as u8) < bar.cells { lit } else { empty };
        draw(cell, place, &style(tint));
    }
}

/// Draw the whole cluster at `origin` (top-left of the asset's canvas), back
/// to front: the inactive bar behind and offset up, the active bar, the
/// emblem, then the energy rail with `energy` lit crystals.
pub fn draw_hud(
    ids: &HudIds,
    origin: (i32, i32),
    active: Bar,
    inactive: Bar,
    energy: u8,
    draw: &mut impl FnMut(GroupId, &Placement, &Style),
) {
    let front = Placement::at(origin.0, origin.1);
    let back = Placement::at(origin.0, origin.1 - 7);
    draw_bar(ids, &back, inactive, true, draw);
    draw_bar(ids, &front, active, false, draw);
    draw(ids.emblem, &front, &Style::tinted(active.rgb));
    draw(ids.emblem_core, &front, &Style::tinted(inactive.rgb));
    draw(ids.rail, &front, &Style::DEFAULT);
    for i in 0..energy.min(10) as usize {
        draw(ids.glow[i], &front, &Style::DEFAULT);
        draw(ids.energy[i], &front, &Style::DEFAULT);
    }
}
