//! Far panels: the world beyond the drawn rooms as a ring of depth-set cards.
//!
//! The cook bakes, for each room, what an eye at the room's centre sees
//! beyond a radius of the rooms the runtime draws whole. The picture is 24
//! vertical *strips* of 64 by 256 texels, side by side in the sky's
//! 1536 by 256 atlas (four strips to a 256 texel page, so a page keeps one
//! palette), each strip a flat card tangent to the circle around the eye at
//! its heading, `15` degrees apart. A strip is cut into four *cells* of 64 by
//! 64 texels, and each cell has its own depth: the median distance of what
//! its texels show. The runtime draws a cell as one textured quad at that
//! depth, so a cell's content slides against the live rooms the way
//! something at that depth does, not the way the infinitely distant cube sky
//! does. Texels no ray hit are palette index 0, which is transparent: the
//! backdrop shows through ([`write_backdrop`]).
//!
//! Strip `s` faces heading `s * 15` degrees, `n = (cos, 0, sin)` in the
//! world (`x` forward, `z` right at zero yaw, `y` up), with `t = (-sin, 0,
//! cos)` to its right. Texel column `x` of a strip is at `u = HALF_WIDTH *
//! ((x + 0.5) / 32 - 1)` and texel row `y` at `h = HALF_HEIGHT * (1 - (y +
//! 0.5) / 128)`, and the ray through it from the eye is `n + u t + h up`. A
//! cell of depth `d` is the quad whose corners are the eye plus `d` times
//! that ray at the cell's corner texels.

use psx_gpu::prim::QuadTextured;

use super::skin::{drawable, project_corners};
use super::GridScreen;

/// Strips around the eye.
pub const STRIPS: usize = 24;
/// Texels across a strip.
pub const STRIP_TEXELS: usize = 64;
/// Texels down a strip.
pub const ROWS: usize = 256;
/// Cells down a strip, each with its own depth.
pub const BANDS: usize = 4;
/// Texel rows of a cell.
pub const BAND_ROWS: usize = ROWS / BANDS;
/// Cells in a room's panel set.
pub const CELLS: usize = STRIPS * BANDS;
/// Strips on one 256 texel page of the atlas.
pub const STRIPS_PER_PAGE: usize = 256 / STRIP_TEXELS;
/// Pages of the atlas. A sky swap writes the atlas page by page (tile index
/// order is page-major), so a page holds one room's panels or the next's,
/// never a mix, once the walk has passed its last tile.
pub const PAGES: usize = STRIPS / STRIPS_PER_PAGE;
/// Runtime units one step of a cell's depth byte stands for.
pub const DEPTH_UNIT: i32 = 32;
/// Half the width of a strip's card at unit depth, Q12: `tan(8.25 degrees)`,
/// a little over the 7.5 degrees a strip owns so neighbours overlap.
pub const HALF_WIDTH_Q12: i32 = 594;
/// Half the height of a strip's card at unit depth, Q12 (0.6).
pub const HALF_HEIGHT_Q12: i32 = 2458;
/// Texel rows a cell's quad reaches into its neighbours, so the seam
/// between two cells at different depths does not open.
pub const SEAM_ROWS: usize = 2;

/// Heading `s * 15` degrees as Q12 `sin`.
pub const SIN_Q12: [i16; STRIPS] = [
    0, 1060, 2048, 2896, 3547, 3956, 4096, 3956, 3547, 2896, 2048, 1060, 0, -1060, -2048, -2896,
    -3547, -3956, -4096, -3956, -3547, -2896, -2048, -1060,
];
/// Heading `s * 15` degrees as Q12 `cos`.
pub const COS_Q12: [i16; STRIPS] = [
    4096, 3956, 3547, 2896, 2048, 1060, 0, -1060, -2048, -2896, -3547, -3956, -4096, -3956, -3547,
    -2896, -2048, -1060, 0, 1060, 2048, 2896, 3547, 3956,
];

/// Strips whose heading is farther than this from where the view looks (a
/// `cos` of 75 degrees, Q12) are not considered: 27 degrees of half view and
/// 8 of strip leave 40 for the eye standing off the bake point.
const CULL_COS_Q12: i32 = 1060;

/// Words of one panel packet: the tag and a textured quad.
pub const PANEL_PACKET_WORDS: usize = 10;
/// Words of one backdrop packet: the tag and a Gouraud quad.
pub const BACKDROP_PACKET_WORDS: usize = 9;
/// Most words [`draw_panels`] and [`write_backdrop`] write between them.
pub const PANEL_DRAW_WORDS: usize = CELLS * PANEL_PACKET_WORDS + 2 * BACKDROP_PACKET_WORDS;

/// The panel tables of a level, borrowed from its manifest.
#[derive(Copy, Clone, Debug)]
pub struct PanelTables<'a> {
    /// [`CELLS`] depth bytes per room, strip-major (cell `strip * BANDS +
    /// band`); 0 is a cell with nothing in it.
    pub depth: &'a [u8],
    /// Per room, the eye its panels were baked from, world runtime units.
    pub eye: &'a [[i16; 3]],
}

impl<'a> PanelTables<'a> {
    /// Tables over the given slices.
    pub const fn new(depth: &'a [u8], eye: &'a [[i16; 3]]) -> Self {
        Self { depth, eye }
    }

    /// Whether the level has no panels.
    pub const fn is_empty(&self) -> bool {
        self.depth.is_empty()
    }

    /// The depth bytes and eye of `room`, if it has panels.
    pub fn room(&self, room: usize) -> Option<(&'a [u8], [i16; 3])> {
        let depth = self.depth.get(room * CELLS..(room + 1) * CELLS)?;
        Some((depth, *self.eye.get(room)?))
    }
}

/// What a panel draw needs of the view and of the sky atlas.
#[derive(Copy, Clone, Debug)]
pub struct PanelView {
    /// Screen size in pixels.
    pub screen: [i32; 2],
    /// Smallest depth, in GTE `SZ` units, a corner may have.
    pub near_sz: u16,
    /// The view's forward axis in plan, `[x, z]` as a Q12 unit vector
    /// ([`super::plan_axes`]).
    pub forward: [i32; 2],
    /// Ordering-table slot of every packet.
    pub slot: u16,
    /// Texture-page word of the atlas's first page.
    pub tpage: u16,
    /// CLUT word of the atlas's first palette.
    pub clut: u16,
}

/// What one panel draw did.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct PanelDraw {
    /// Words written.
    pub words: usize,
    /// Packets written.
    pub packets: u32,
    /// Strips that passed the heading test.
    pub strips: u16,
    /// Cells with something in them among those strips.
    pub cells: u16,
    /// The packet storage ran out.
    pub overflowed: bool,
}

/// The four corners of cell `band` of strip `strip`, `[top-left, top-right,
/// bottom-left, bottom-right]` as the eye plus `depth` times the ray there,
/// reaching `SEAM_ROWS` texel rows into the neighbouring cells; and the
/// texel rows `(top, last)` of the quad's picture.
pub fn cell_corners(
    eye: [i32; 3],
    strip: usize,
    band: usize,
    depth: i32,
) -> ([(i32, i32, i32); 4], (usize, usize)) {
    let top = (band * BAND_ROWS).saturating_sub(SEAM_ROWS);
    let end = ((band + 1) * BAND_ROWS + SEAM_ROWS).min(ROWS);
    let (sin, cos) = (i32::from(SIN_Q12[strip]), i32::from(COS_Q12[strip]));
    let corner = |u: i32, row: usize| {
        let h = (HALF_HEIGHT_Q12 * (ROWS as i32 / 2 - row as i32)) / (ROWS as i32 / 2);
        let rx = cos - ((u * sin) >> 12);
        let rz = sin + ((u * cos) >> 12);
        (
            eye[0] + ((depth * rx) >> 12),
            eye[1] + ((depth * h) >> 12),
            eye[2] + ((depth * rz) >> 12),
        )
    };
    (
        [
            corner(-HALF_WIDTH_Q12, top),
            corner(HALF_WIDTH_Q12, top),
            corner(-HALF_WIDTH_Q12, end),
            corner(HALF_WIDTH_Q12, end),
        ],
        (top, end - 1),
    )
}

/// One cell the draw will emit: depth byte, strip and band.
#[derive(Copy, Clone, Default)]
struct Cell {
    depth: u8,
    strip: u8,
    band: u8,
}

/// Write the packets of the panels into `out`, nearest cell first: the
/// packets are prepended into the ordering table in memory order, so the
/// farthest cell, written last, runs first. `rooms` names, for each page of
/// the atlas, the room whose panels the page holds (a sky swap changes them
/// one page at a time). The GTE must hold the world view (rotation, and the
/// translation of the world origin).
pub fn draw_panels(
    tables: &PanelTables<'_>,
    rooms: &[u16; PAGES],
    view: &PanelView,
    out: &mut [u32],
) -> PanelDraw {
    let mut draw = PanelDraw::default();
    let mut cells = [Cell::default(); CELLS];
    let mut count = 0usize;
    for strip in 0..STRIPS {
        let Some((depth, _)) = tables.room(usize::from(rooms[strip / STRIPS_PER_PAGE])) else {
            continue;
        };
        let ahead = (view.forward[0] * i32::from(COS_Q12[strip])
            + view.forward[1] * i32::from(SIN_Q12[strip]))
            >> 12;
        if ahead < CULL_COS_Q12 {
            continue;
        }
        draw.strips += 1;
        for band in 0..BANDS {
            let byte = depth[strip * BANDS + band];
            if byte != 0 {
                cells[count] = Cell {
                    depth: byte,
                    strip: strip as u8,
                    band: band as u8,
                };
                count += 1;
            }
        }
    }
    draw.cells = count as u16;
    // Nearest first: insertion sort, a few dozen entries at most.
    for i in 1..count {
        let cell = cells[i];
        let mut j = i;
        while j > 0 && cells[j - 1].depth > cell.depth {
            cells[j] = cells[j - 1];
            j -= 1;
        }
        cells[j] = cell;
    }
    for cell in &cells[..count] {
        if draw.words + PANEL_PACKET_WORDS > out.len() {
            draw.overflowed = true;
            break;
        }
        let (strip, band) = (usize::from(cell.strip), usize::from(cell.band));
        let Some((_, eye)) = tables.room(usize::from(rooms[strip / STRIPS_PER_PAGE])) else {
            continue;
        };
        let eye = eye.map(i32::from);
        let (corners, (v0, v1)) =
            cell_corners(eye, strip, band, i32::from(cell.depth) * DEPTH_UNIT);
        let screen = project_corners(corners, view.near_sz, view.screen);
        if !drawable(&screen) {
            continue;
        }
        let page = strip / STRIPS_PER_PAGE;
        let u0 = ((strip % STRIPS_PER_PAGE) * STRIP_TEXELS) as u8;
        let u1 = u0 + (STRIP_TEXELS - 1) as u8;
        let uv = [
            (u0, v0 as u8),
            (u1, v0 as u8),
            (u0, v1 as u8),
            (u1, v1 as u8),
        ];
        write_quad(
            &screen,
            uv,
            view.clut.wrapping_add(page as u16),
            view.tpage.wrapping_add(page as u16),
            view.slot,
            &mut out[draw.words..draw.words + PANEL_PACKET_WORDS],
        );
        draw.words += PANEL_PACKET_WORDS;
        draw.packets += 1;
    }
    draw
}

/// Write a textured quad of the projected `screen` corners (top-left,
/// top-right, bottom-left, bottom-right) into `out`.
fn write_quad(
    screen: &[GridScreen; 4],
    uv: [(u8, u8); 4],
    clut: u16,
    tpage: u16,
    slot: u16,
    out: &mut [u32],
) {
    let xy = |s: &GridScreen| (s.sxy as u16 as i16, (s.sxy >> 16) as u16 as i16);
    let mut quad = QuadTextured::new(
        [
            xy(&screen[0]),
            xy(&screen[1]),
            xy(&screen[2]),
            xy(&screen[3]),
        ],
        uv,
        clut,
        tpage,
        (0x80, 0x80, 0x80),
    );
    quad.tag = (u32::from(QuadTextured::WORDS) << 24) | u32::from(slot);
    let words: [u32; PANEL_PACKET_WORDS] = [
        quad.tag,
        quad.color_cmd,
        quad.v0,
        quad.uv0_clut,
        quad.v1,
        quad.uv1_tpage,
        quad.v2,
        quad.uv2,
        quad.v3,
        quad.uv3,
    ];
    out[..PANEL_PACKET_WORDS].copy_from_slice(&words);
}

/// The colours of the backdrop behind the panels, `[r, g, b]` each.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Backdrop {
    /// Everything below the horizon.
    pub below: [u8; 3],
    /// The sky at the horizon.
    pub horizon: [u8; 3],
    /// The sky at the top of the screen.
    pub top: [u8; 3],
}

/// Screen `y` (pixels from the top) of the horizon at screen column `x`,
/// for a view with the rotation rows `rotation` (`[0]` right, `[1]` down,
/// `[2]` forward, in world axes with `y` up), projection `focal` and a
/// screen of `screen` pixels. `None` when the view is too steep to say.
pub fn horizon_y(rotation: &[[i16; 3]; 3], focal: i32, screen: [i32; 2], x: i32) -> Option<i32> {
    let down = i32::from(rotation[1][1]);
    if down.abs() < 16 {
        return None;
    }
    let numerator =
        (x - screen[0] / 2) * i32::from(rotation[0][1]) + focal * i32::from(rotation[2][1]);
    Some(screen[1] / 2 - numerator / down)
}

/// Write the backdrop into `out` (at most `2 * BACKDROP_PACKET_WORDS`): a
/// gradient from the horizon colour up to the top colour above the horizon
/// and a flat fill below it. Written after the panels, so it runs before
/// them. Returns `(words, packets)`.
pub fn write_backdrop(
    rotation: &[[i16; 3]; 3],
    focal: i32,
    screen: [i32; 2],
    colours: Backdrop,
    slot: u16,
    out: &mut [u32],
) -> (usize, u32) {
    let (w, h) = (screen[0], screen[1]);
    // A view too steep to place the horizon looks at the sky or the void.
    let looking_up = rotation[2][1] > 0;
    let at = |x: i32| {
        horizon_y(rotation, focal, screen, x)
            .unwrap_or(if looking_up { h } else { 0 })
            .clamp(0, h)
    };
    let (left, right) = (at(0), at(w));
    let mut words = 0usize;
    let mut packets = 0u32;
    let mut put = |corners: [(i32, i32); 4], colour: [[u8; 3]; 4]| {
        if words + BACKDROP_PACKET_WORDS > out.len() {
            return;
        }
        let pack = |c: [u8; 3]| u32::from(c[0]) | u32::from(c[1]) << 8 | u32::from(c[2]) << 16;
        let xy = |(x, y): (i32, i32)| (x as u16 as u32) | ((y as u16 as u32) << 16);
        let o = &mut out[words..words + BACKDROP_PACKET_WORDS];
        o[0] = ((BACKDROP_PACKET_WORDS as u32 - 1) << 24) | u32::from(slot);
        o[1] = 0x3800_0000 | pack(colour[0]);
        o[2] = xy(corners[0]);
        o[3] = pack(colour[1]);
        o[4] = xy(corners[1]);
        o[5] = pack(colour[2]);
        o[6] = xy(corners[2]);
        o[7] = pack(colour[3]);
        o[8] = xy(corners[3]);
        words += BACKDROP_PACKET_WORDS;
        packets += 1;
    };
    if left < h || right < h {
        put([(0, left), (w, right), (0, h), (w, h)], [colours.below; 4]);
    }
    if left > 0 || right > 0 {
        put(
            [(0, 0), (w, 0), (0, left), (w, right)],
            [colours.top, colours.top, colours.horizon, colours.horizon],
        );
    }
    (words, packets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{load_view_rotation, plan_axes, ViewTransform};
    use psx_gte::math::{Mat3I16, Vec3I32};
    use psx_gte::scene;

    /// A view from `eye` looking along heading `yaw` (Q12 turns of 4096 per
    /// circle), with the GTE host model loaded for a 320 by 240 screen.
    fn view_from(eye: [i32; 3], yaw: u16, pitch: u16) -> ViewTransform {
        psx_gte::host::reset();
        scene::set_screen_offset(160 << 16, 120 << 16);
        scene::set_projection_plane(320);
        let rotation = Mat3I16::rotate_xyz(pitch >> 4, yaw >> 4, 0);
        load_view_rotation(
            Vec3I32::new(eye[0] << 12, eye[1] << 12, eye[2] << 12),
            rotation,
        )
    }

    fn panel_view(transform: &ViewTransform) -> PanelView {
        PanelView {
            screen: [320, 240],
            near_sz: 162,
            forward: plan_axes(&transform.rotation.m).1,
            slot: 2047,
            tpage: 0x0010,
            clut: 0x1234,
        }
    }

    #[test]
    fn the_heading_tables_are_unit_vectors_a_quarter_turn_apart() {
        for s in 0..STRIPS {
            let (sin, cos) = (i32::from(SIN_Q12[s]), i32::from(COS_Q12[s]));
            let length = sin * sin + cos * cos;
            assert!(
                (length - 4096 * 4096).abs() < 2 * 4096,
                "strip {s}: {length}"
            );
            // A quarter turn (6 strips) on, sin becomes the old cos.
            assert_eq!(SIN_Q12[(s + 6) % STRIPS], COS_Q12[s]);
            assert_eq!(COS_Q12[(s + 6) % STRIPS], -SIN_Q12[s]);
        }
    }

    #[test]
    fn a_strip_of_the_atlas_keeps_to_its_palette_page() {
        // Four strips fill a 256 texel page exactly, so none straddles two
        // palettes.
        assert_eq!(STRIPS_PER_PAGE * STRIP_TEXELS, 256);
        assert_eq!(STRIPS / STRIPS_PER_PAGE, 6);
        assert_eq!(BANDS * BAND_ROWS, ROWS);
    }

    #[test]
    fn a_cells_corners_lie_on_their_rays_at_their_depth() {
        let eye = [1000, 200, -500];
        // Heading 0 is +X: a cell of depth d is d ahead in x at the middle,
        // half a width to the right (+Z) at its right edge.
        let (corners, rows) = cell_corners(eye, 0, 1, 1600);
        assert_eq!(rows, (BAND_ROWS - SEAM_ROWS, 2 * BAND_ROWS + SEAM_ROWS - 1));
        let width = 1600 * HALF_WIDTH_Q12 / 4096;
        for corner in corners {
            assert!((corner.0 - (eye[0] + 1600)).abs() <= 1);
        }
        assert!((corners[0].2 - (eye[2] - width)).abs() <= 1);
        assert!((corners[1].2 - (eye[2] + width)).abs() <= 1);
        // Row 62 is just above the band, row 130 just below it: the first is
        // higher.
        assert!(corners[0].1 > eye[1] && corners[2].1 < eye[1]);
        // A quarter turn on, the cell is ahead in z instead.
        let (turned, _) = cell_corners(eye, 6, 1, 1600);
        for corner in turned {
            assert!((corner.2 - (eye[2] + 1600)).abs() <= 1);
        }
        assert!((turned[1].0 - (eye[0] - width)).abs() <= 1);
    }

    #[test]
    fn rows_reach_into_the_neighbours_but_not_off_the_strip() {
        let (_, top) = cell_corners([0; 3], 0, 0, 640);
        assert_eq!(top.0, 0);
        let (_, bottom) = cell_corners([0; 3], 0, BANDS - 1, 640);
        assert_eq!(bottom.1, ROWS - 1);
    }

    /// Tables with one cell filled for room 0.
    fn one_cell(strip: usize, band: usize, depth: u8, eye: [i16; 3]) -> (Vec<u8>, Vec<[i16; 3]>) {
        let mut cells = alloc::vec![0u8; CELLS];
        cells[strip * BANDS + band] = depth;
        (cells, alloc::vec![eye])
    }

    use alloc::vec::Vec;

    #[test]
    fn a_cell_ahead_draws_one_quad_and_one_behind_none() {
        let eye = [0, 100, 0];
        let transform = view_from(eye, 0, 0);
        let view = panel_view(&transform);
        let mut out = [0u32; PANEL_DRAW_WORDS];
        // Strip 0 faces +X, where the view looks.
        let (depth, eyes) = one_cell(0, 1, 40, [0, 100, 0]);
        let tables = PanelTables::new(&depth, &eyes);
        let drawn = draw_panels(&tables, &[0; PAGES], &view, &mut out);
        assert_eq!((drawn.packets, drawn.words, drawn.cells), (1, 10, 1));
        assert_eq!(out[0], (9 << 24) | 2047);
        assert_eq!(out[1] >> 24, 0x2C, "an opaque textured quad");
        // The CLUT and the page of strip 0 are the atlas's first.
        assert_eq!(out[3] >> 16, 0x1234);
        assert_eq!(out[5] >> 16, 0x0010);
        // The picture is the strip's first 64 columns, rows 62 to 129.
        assert_eq!(out[3] & 0xFFFF, 62 << 8);
        assert_eq!(out[5] & 0xFFFF, 63 | (62 << 8));
        assert_eq!(out[7] & 0xFFFF, 129 << 8);
        assert_eq!(out[9] & 0xFFFF, 63 | (129 << 8));
        // Strip 12 faces -X, away from the view: not considered. Of the 24
        // strips, the 11 within 75 degrees of the view pass the heading test.
        let (depth, eyes) = one_cell(12, 1, 40, [0, 100, 0]);
        let tables = PanelTables::new(&depth, &eyes);
        let drawn = draw_panels(&tables, &[0; PAGES], &view, &mut out);
        assert_eq!((drawn.packets, drawn.strips, drawn.cells), (0, 11, 0));
    }

    #[test]
    fn the_cell_lands_where_a_ray_to_it_does() {
        // A cell dead ahead, its middle (rows 64 to 127, the band 1 centre
        // is the horizon) on the screen centre: a 1280 deep cell spans
        // 1280 * 0.145 = 186 units each way, 320 / 1280 * 186 = 46 pixels.
        let transform = view_from([0, 100, 0], 0, 0);
        let view = panel_view(&transform);
        let (depth, eyes) = one_cell(0, 1, 40, [0, 100, 0]);
        let tables = PanelTables::new(&depth, &eyes);
        let mut out = [0u32; PANEL_DRAW_WORDS];
        assert_eq!(
            draw_panels(&tables, &[0; PAGES], &view, &mut out).packets,
            1
        );
        let at = |word: u32| (word as u16 as i16, (word >> 16) as u16 as i16);
        let (left, top) = at(out[2]);
        let (right, _) = at(out[4]);
        let (_, bottom) = at(out[6]);
        assert!(left < 160 && right > 160 && top < 120 && bottom > 120);
        assert!((160 - left - (right - 160)).abs() <= 2, "{left} {right}");
        assert!((right - left - 93).abs() <= 3, "{}", right - left);
    }

    #[test]
    fn cells_are_written_nearest_first() {
        let transform = view_from([0, 100, 0], 0, 0);
        let view = panel_view(&transform);
        let mut cells = alloc::vec![0u8; CELLS];
        cells[0 * BANDS + 1] = 90;
        cells[1 * BANDS + 1] = 30;
        cells[23 * BANDS + 1] = 60;
        let eyes = alloc::vec![[0i16, 100, 0]];
        let tables = PanelTables::new(&cells, &eyes);
        let mut out = [0u32; PANEL_DRAW_WORDS];
        let drawn = draw_panels(&tables, &[0; PAGES], &view, &mut out);
        assert_eq!(drawn.packets, 3);
        // Strip 1 (depth 30), strip 23 (60), strip 0 (90): the CLUT/page
        // words are the same page 0; tell them apart by the quads' u range.
        let u = |packet: usize| out[packet * 10 + 3] & 0xFF;
        // Strips 0, 1, 23 are columns 0, 64, 192 of page 0 and page 5.
        assert_eq!(u(0), 64);
        assert_eq!(u(1), 192);
        assert_eq!(u(2), 0);
    }

    #[test]
    fn a_level_without_panels_or_a_room_without_them_draws_nothing() {
        let transform = view_from([0, 100, 0], 0, 0);
        let view = panel_view(&transform);
        let mut out = [0u32; PANEL_DRAW_WORDS];
        let empty = PanelTables::new(&[], &[]);
        assert!(empty.is_empty());
        assert_eq!(
            draw_panels(&empty, &[0; PAGES], &view, &mut out),
            PanelDraw::default()
        );
        let (depth, eyes) = one_cell(0, 1, 40, [0, 100, 0]);
        let tables = PanelTables::new(&depth, &eyes);
        assert_eq!(
            draw_panels(&tables, &[1; PAGES], &view, &mut out),
            PanelDraw::default()
        );
    }

    #[test]
    fn each_page_draws_the_room_it_holds() {
        let transform = view_from([0, 100, 0], 0, 0);
        let view = panel_view(&transform);
        let mut cells = alloc::vec![0u8; 2 * CELLS];
        // Room 0 has strip 0 (page 0), room 1 has strip 23 (page 5); both
        // face the view.
        cells[1] = 40;
        cells[CELLS + 23 * BANDS + 1] = 40;
        let eyes = alloc::vec![[0i16, 100, 0], [0, 100, 0]];
        let tables = PanelTables::new(&cells, &eyes);
        let mut out = [0u32; PANEL_DRAW_WORDS];
        let packets =
            |rooms: [u16; PAGES], out: &mut [u32]| draw_panels(&tables, &rooms, &view, out).packets;
        assert_eq!(packets([0; PAGES], &mut out), 1);
        assert_eq!(packets([1; PAGES], &mut out), 1);
        // A swap part way through: page 5 already holds room 1's.
        assert_eq!(packets([0, 0, 0, 0, 0, 1], &mut out), 2);
        assert_eq!(packets([1, 1, 1, 1, 1, 0], &mut out), 0);
    }

    #[test]
    fn the_horizon_sits_at_the_middle_of_a_level_view_and_moves_with_pitch() {
        let flat = view_from([0, 100, 0], 0, 0);
        for x in [0, 160, 320] {
            let y = horizon_y(&flat.rotation.m, 320, [320, 240], x).unwrap();
            assert!((y - 120).abs() <= 1, "x {x}: {y}");
        }
        // Looking down by a few degrees puts the horizon higher on screen.
        let down = view_from([0, 100, 0], 0, 100);
        let y = horizon_y(&down.rotation.m, 320, [320, 240], 160).unwrap();
        assert!(y < 120, "{y}");
        let up = view_from([0, 100, 0], 0, 4096 - 100);
        let y = horizon_y(&up.rotation.m, 320, [320, 240], 160).unwrap();
        assert!(y > 120, "{y}");
    }

    #[test]
    fn the_backdrop_fills_the_screen_below_and_grades_above_the_horizon() {
        let flat = view_from([0, 100, 0], 0, 0);
        let colours = Backdrop {
            below: [1, 2, 3],
            horizon: [4, 5, 6],
            top: [7, 8, 9],
        };
        let mut out = [0u32; 2 * BACKDROP_PACKET_WORDS];
        let (words, packets) =
            write_backdrop(&flat.rotation.m, 320, [320, 240], colours, 2047, &mut out);
        assert_eq!((words, packets), (18, 2));
        // The flat fill is below the horizon, the gradient above.
        assert_eq!(out[1], 0x3800_0000 | 0x030201);
        assert_eq!(out[9 + 1], 0x3800_0000 | 0x090807);
        // Its lower corners (the third and fourth colours) are the horizon.
        assert_eq!(out[9 + 5], 0x060504);
        assert_eq!(out[9 + 7], 0x060504);
        // Too short a buffer writes what fits.
        let (words, packets) = write_backdrop(
            &flat.rotation.m,
            320,
            [320, 240],
            colours,
            2047,
            &mut out[..10],
        );
        assert_eq!((words, packets), (9, 1));
    }
}
