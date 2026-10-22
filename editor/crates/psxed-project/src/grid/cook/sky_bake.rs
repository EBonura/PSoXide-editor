//! Baked distance: the world beyond a chunk's ring, as a cube sky.
//!
//! A level that streams as a ring of chunks draws only the chunk the player
//! is in and its neighbours. Everything farther is baked into the sky from
//! the chunk's centre: every texel of the six-face cube atlas the runtime
//! already draws as a sky casts a ray through the level's own collision, with
//! the ring left out, and shades what it hits with the level's fog. The seam
//! where the live geometry ends is therefore the jagged outline of the ring,
//! and the fog continues across it.
//!
//! The rays use the runtime's collision, so the bake sees exactly the
//! geometry the game has. A hit takes the mean colour of the material of the
//! surface it lands on ([`WorldColours`]: the floor, ceiling or derived wall
//! of the document leaf there), shaded by a key light, then the fog.

use psx_engine::sky::{CubeFace, CUBE_SKY_FACE_HEIGHT, CUBE_SKY_FACE_WIDTH};
use psx_engine::{CollisionTrace, CollisionTraceProvider, CollisionTraceQuery, RoomPoint};
use psx_grid::collision::GridTraceProvider;
use psx_grid::render::panels::{self, DEPTH_UNIT};
use psx_grid::write::{aligned_copy, words_as_bytes};
use psx_grid::{CollisionView, LevelView, ResidentRooms, RoomView};

use std::collections::HashMap;

use super::CookedLevel;
use crate::grid::leaf::CLICKS_PER_SECTOR;
use crate::grid::sector::GridSector;
use crate::grid::world::GridWorld;
use crate::ResourceId;

/// What a hit is made of: the document's surfaces under a world point,
/// coloured by their materials' mean texel colour.
pub struct WorldColours<'a> {
    world: &'a GridWorld,
    /// Runtime units per sector.
    sector: i32,
    /// World sector of `rooms[0]`.
    origin: [i32; 2],
    width: usize,
    /// Per world sector, the room holding a leaf there (`u16::MAX`: none).
    rooms: Vec<u16>,
    /// Mean texel colour per material, channels 0..=1.
    albedo: HashMap<Option<ResourceId>, [f32; 3]>,
}

impl<'a> WorldColours<'a> {
    /// Colours of `world` cooked at `sector` runtime units a sector, with the
    /// mean colour of each material (a material missing from `albedo` reads
    /// mid grey).
    pub fn new(
        world: &'a GridWorld,
        sector: i32,
        albedo: HashMap<Option<ResourceId>, [f32; 3]>,
    ) -> Self {
        let (mut lo, mut hi) = ([i32::MAX; 2], [i32::MIN; 2]);
        for room in &world.rooms {
            lo = [lo[0].min(room.origin[0]), lo[1].min(room.origin[1])];
            hi = [
                hi[0].max(room.origin[0] + room.width as i32),
                hi[1].max(room.origin[1] + room.depth as i32),
            ];
        }
        let width = (hi[0] - lo[0]).max(0) as usize;
        let depth = (hi[1] - lo[1]).max(0) as usize;
        let mut rooms = vec![u16::MAX; width * depth];
        for (index, room) in world.rooms.iter().enumerate() {
            for (key, _) in room.leaves() {
                let (a, b) = key.sector_range();
                for z in a[1]..=b[1] {
                    for x in a[0]..=b[0] {
                        let cell = (z - lo[1]) as usize * width + (x - lo[0]) as usize;
                        if rooms[cell] == u16::MAX {
                            rooms[cell] = index as u16;
                        }
                    }
                }
            }
        }
        Self {
            world,
            sector,
            origin: lo,
            width,
            rooms,
            albedo,
        }
    }

    /// The materials a world uses, to fill the table [`Self::new`] takes.
    pub fn materials(world: &GridWorld) -> Vec<Option<ResourceId>> {
        let mut out = vec![None];
        let mut add = |m: Option<ResourceId>| {
            if !out.contains(&m) {
                out.push(m);
            }
        };
        for room in &world.rooms {
            add(room.defaults.wall_material);
            for (_, sector) in room.leaves() {
                for surface in [sector.ground_floor(), sector.ground_ceiling()]
                    .into_iter()
                    .flatten()
                {
                    add(surface.tri_a.material);
                    add(surface.tri_b.and_then(|t| t.material));
                }
                add(sector.ground_wall_material());
            }
        }
        out
    }

    /// The leaf under world runtime point `(x, z)` and its room.
    fn leaf(&self, x: i32, z: i32) -> Option<(usize, &'a GridSector)> {
        let click = [
            (x * CLICKS_PER_SECTOR).div_euclid(self.sector),
            (z * CLICKS_PER_SECTOR).div_euclid(self.sector),
        ];
        let cell = [
            click[0].div_euclid(CLICKS_PER_SECTOR) - self.origin[0],
            click[1].div_euclid(CLICKS_PER_SECTOR) - self.origin[1],
        ];
        if cell[0] < 0 || cell[1] < 0 || cell[0] as usize >= self.width {
            return None;
        }
        let room = usize::from(
            *self
                .rooms
                .get(cell[1] as usize * self.width + cell[0] as usize)?,
        );
        let (_, sector) = self.world.rooms.get(room)?.leaf_at(click)?;
        Some((room, sector))
    }

    /// Base colour (0..=255 a channel) of the surface hit at `at` with
    /// normal `n`: a floor's or ceiling's material, or a wall's, chosen the
    /// way the cook chooses a derived wall's (the sector behind it, the one
    /// in front, the room's default, then the floor behind it).
    pub fn colour(&self, at: [i32; 3], n: [f64; 3]) -> Option<[f64; 3]> {
        let step = |k: f64| {
            (
                at[0] + (n[0] * k).round() as i32,
                at[2] + (n[2] * k).round() as i32,
            )
        };
        let material = if n[1] > 0.5 {
            let (x, z) = step(-1.0);
            self.leaf(x, z)?.1.ground_floor()?.tri_a.material
        } else if n[1] < -0.5 {
            let (x, z) = step(-1.0);
            self.leaf(x, z)?.1.ground_ceiling()?.tri_a.material
        } else {
            let (bx, bz) = step(-2.0);
            let (fx, fz) = step(2.0);
            let behind = self.leaf(bx, bz);
            let front = self.leaf(fx, fz);
            behind
                .and_then(|(_, s)| s.ground_wall_material())
                .or_else(|| front.and_then(|(_, s)| s.ground_wall_material()))
                .or_else(|| {
                    behind
                        .or(front)
                        .and_then(|(room, _)| self.world.rooms[room].defaults.wall_material)
                })
                .or_else(|| behind.and_then(|(_, s)| s.ground_floor()?.tri_a.material))
        };
        let albedo = self.albedo.get(&material).copied().unwrap_or([0.5; 3]);
        Some(albedo.map(|c| f64::from(c) * 255.0))
    }
}

/// Faces of the atlas in its left-to-right order.
const FACES: [CubeFace; 6] = [
    CubeFace::PositiveX,
    CubeFace::NegativeX,
    CubeFace::PositiveY,
    CubeFace::NegativeY,
    CubeFace::PositiveZ,
    CubeFace::NegativeZ,
];

/// Eye height over the floor at the chunk's centre, runtime units: about a
/// standing body's eyes.
const EYE_HEIGHT: i32 = 60;

/// How far a ray looks, runtime units. Points stay well inside the
/// collision coordinate limit.
const REACH: f64 = 9000.0;

/// Horizon colour of a chunk without fog.
const NO_FOG_HORIZON: [f64; 3] = [96.0, 104.0, 120.0];

/// A baked chunk sky.
pub struct ChunkSky {
    /// The chunk baked from.
    pub chunk: usize,
    /// The chunk and its neighbours, left out of the bake.
    pub ring: Vec<usize>,
    /// Where the rays start, world runtime units.
    pub eye: [i32; 3],
    /// The horizon colour the bake used: the room's fog, or the neutral
    /// horizon of a room without any.
    pub fog: [f64; 3],
    /// Colour of every texel, face-major (`face * 256 * 256 + y * 256 + x`).
    pub rgb: Vec<[u8; 3]>,
    /// Rays that hit geometry.
    pub hits: usize,
}

impl ChunkSky {
    /// The atlas as one 1536x256 row-major image, faces side by side.
    pub fn atlas_rgb(&self) -> Vec<[u8; 3]> {
        let (w, h) = (
            usize::from(CUBE_SKY_FACE_WIDTH),
            usize::from(CUBE_SKY_FACE_HEIGHT),
        );
        let mut out = vec![[0; 3]; 6 * w * h];
        for face in 0..6 {
            for y in 0..h {
                for x in 0..w {
                    out[y * 6 * w + face * w + x] = self.rgb[(face * h + y) * w + x];
                }
            }
        }
        out
    }

    /// The atlas as a 4bpp PSXT texture with one 16-colour palette per face,
    /// the shape the cube sky takes.
    pub fn to_psxt(&self) -> Result<Vec<u8>, String> {
        self.to_psxt_with(&shared_palettes(core::slice::from_ref(self))?)
    }

    /// [`Self::to_psxt`] over the given six face palettes (see
    /// [`shared_palettes`]): every texel takes the nearest colour of its
    /// face's palette.
    pub fn to_psxt_with(&self, palettes: &[Vec<[u8; 3]>; 6]) -> Result<Vec<u8>, String> {
        psxed_tex::encode_indexed_psxt_with_clut_rows(
            CUBE_SKY_FACE_WIDTH * 6,
            CUBE_SKY_FACE_HEIGHT,
            psxed_format::texture::Depth::Bit4,
            &self.indices(palettes),
            palettes.as_slice(),
            false,
        )
        .map_err(|e| format!("encode the sky: {e}"))
    }

    /// The palette index of every texel over `palettes`, as one row-major
    /// 1536x256 image (faces side by side).
    pub fn indices(&self, palettes: &[Vec<[u8; 3]>; 6]) -> Vec<u8> {
        let (w, h) = (
            usize::from(CUBE_SKY_FACE_WIDTH),
            usize::from(CUBE_SKY_FACE_HEIGHT),
        );
        let mut indices = vec![0u8; 6 * w * h];
        for (face, palette) in palettes.iter().enumerate() {
            for y in 0..h {
                for x in 0..w {
                    let texel = self.rgb[(face * h + y) * w + x];
                    indices[y * 6 * w + face * w + x] = nearest(palette, texel);
                }
            }
        }
        indices
    }

    /// The sky as a sparse entry ([`psx_level::sky_tiles`]): the 8x8 tiles
    /// whose indices over `palettes` differ from `background`'s, nothing else.
    pub fn to_sparse(
        &self,
        palettes: &[Vec<[u8; 3]>; 6],
        background: &SkyBackground,
    ) -> Result<Vec<u8>, String> {
        Ok(sparse_entry(&self.indices(palettes), background))
    }
}

/// The sparse entry ([`psx_level::sky_tiles`]) of an atlas of palette
/// `indices` (1536 by 256, row-major): the 8x8 tiles that differ from
/// `background`, nothing else.
fn sparse_entry(indices: &[u8], background: &SkyBackground) -> Vec<u8> {
    use psx_level::sky_tiles as st;
    let view = background.view();
    let width = st::FACES * st::FACE_TEXELS;
    let mut mask = [0u8; st::MASK_BYTES];
    let mut tiles = Vec::new();
    let mut count = 0usize;
    for index in 0..st::TILE_COUNT {
        let (face, tile_y, tile_x) = st::tile_position(index);
        let words: [u32; st::TILE_WORDS] = std::array::from_fn(|r| {
            let y = tile_y * st::TILE_TEXELS + r;
            let at = y * width + face * st::FACE_TEXELS + tile_x * st::TILE_TEXELS;
            (0..st::TILE_TEXELS).fold(0u32, |word, t| word | u32::from(indices[at + t]) << (4 * t))
        });
        if words == view.tile_words(face, tile_y, tile_x) {
            continue;
        }
        mask[index / 8] |= 1 << (index % 8);
        count += 1;
        for word in words {
            tiles.extend_from_slice(&word.to_le_bytes());
        }
    }
    let mut entry = Vec::with_capacity(st::entry_bytes(count));
    entry.extend_from_slice(&st::VERSION.to_le_bytes());
    entry.extend_from_slice(&(count as u16).to_le_bytes());
    entry.extend_from_slice(&mask);
    entry.extend_from_slice(&tiles);
    entry
}

/// The index every texel that no ray hit reads, level-wide: what the sparse
/// skies are sparse against ([`psx_level::sky_tiles::Background`]).
///
/// A miss paints the same colour whatever the chunk (a function of the
/// texel's elevation, and the level's fog), and the palettes are shared, so
/// the index it quantises to is a function of the texel's place on its face.
/// The map is exact, texel by texel: the side faces' gradient contours bend
/// with the column, and the zenith face's are circles, and both are kept as
/// they quantise, as runs of equal index along each row.
pub struct SkyBackground {
    /// First run of every row of every face, then the end (1,537 entries).
    pub rows: Vec<u16>,
    /// Runs as `(last column, index)` byte pairs.
    pub runs: Vec<u8>,
}

impl SkyBackground {
    /// The background of a level whose rooms fog to `fog`, over `palettes`.
    pub fn new(fog: [f64; 3], palettes: &[Vec<[u8; 3]>; 6]) -> Self {
        let (w, h) = (
            usize::from(CUBE_SKY_FACE_WIDTH),
            usize::from(CUBE_SKY_FACE_HEIGHT),
        );
        let (mut rows, mut runs) = (Vec::new(), Vec::new());
        for (face, palette) in palettes.iter().enumerate() {
            for y in 0..h {
                rows.push((runs.len() / 2) as u16);
                let mut previous: Option<u8> = None;
                for x in 0..w {
                    let d = texel_direction(FACES[face], x, y);
                    let index = nearest(palette, background_colour(d[1], fog));
                    match previous {
                        Some(p) if p == index => {
                            let last = runs.len() - 2;
                            runs[last] = x as u8;
                        }
                        _ => runs.extend([x as u8, index]),
                    }
                    previous = Some(index);
                }
            }
        }
        rows.push((runs.len() / 2) as u16);
        Self { rows, runs }
    }

    /// The tables as the runtime reads them.
    pub fn view(&self) -> psx_level::sky_tiles::Background<'_> {
        psx_level::sky_tiles::Background::new(&self.rows, &self.runs)
    }
}

/// One 16-colour palette per cube face for a whole set of chunk skies. With
/// every chunk's sky on the same palettes, the runtime can stream a new sky's
/// texels over the shown one row by row: there is no palette to swap, so no
/// frame shows texels under the wrong colours, and no second copy of the sky
/// is needed in VRAM.
pub fn shared_palettes(skies: &[ChunkSky]) -> Result<[Vec<[u8; 3]>; 6], String> {
    let (w, h) = (
        usize::from(CUBE_SKY_FACE_WIDTH),
        usize::from(CUBE_SKY_FACE_HEIGHT),
    );
    // A sample of every sky's face, kept to about a quarter million texels.
    let stride = (skies.len() * w * h / 262_144).max(1);
    let mut palettes: [Vec<[u8; 3]>; 6] = Default::default();
    for (face, palette) in palettes.iter_mut().enumerate() {
        let sample: Vec<[u8; 3]> = skies
            .iter()
            .flat_map(|sky| sky.rgb[face * w * h..(face + 1) * w * h].iter().copied())
            .step_by(stride)
            .collect();
        *palette = psxed_tex::quantize_rgb(&sample, 16)
            .map_err(|e| format!("face {face}: {e}"))?
            .0;
    }
    Ok(palettes)
}

/// Index of the colour of `palette` nearest `c`.
fn nearest(palette: &[[u8; 3]], c: [u8; 3]) -> u8 {
    let distance = |p: &[u8; 3]| {
        (0..3)
            .map(|i| (i32::from(p[i]) - i32::from(c[i])).pow(2))
            .sum::<i32>()
    };
    palette
        .iter()
        .enumerate()
        .min_by_key(|(_, p)| distance(p))
        .map_or(0, |(i, _)| i as u8)
}

/// The unit direction through the centre of texel `(x, y)` of `face`.
fn texel_direction(face: CubeFace, x: usize, y: usize) -> [f64; 3] {
    let (w, h) = (
        usize::from(CUBE_SKY_FACE_WIDTH),
        usize::from(CUBE_SKY_FACE_HEIGHT),
    );
    let u = (x as f64 + 0.5) / w as f64 * 2.0 - 1.0;
    let v = (y as f64 + 0.5) / h as f64 * 2.0 - 1.0;
    let d = cube_direction(face, u, v);
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    d.map(|c| c / len)
}

/// What a ray that hits nothing paints, looking along a unit direction with
/// vertical component `dy`: the void under the level, darker than the fog,
/// below the horizon; the sky's gradient from the fog up to a deep blue above.
fn background_colour(dy: f64, fog: [f64; 3]) -> [u8; 3] {
    let colour = if dy < 0.0 {
        fog.map(|c| c * 0.6)
    } else {
        let k = dy.clamp(0.0, 1.0).powf(0.6);
        let top = [fog[0] * 0.35, fog[1] * 0.45, fog[2] * 0.7];
        [0, 1, 2].map(|i| fog[i] + (top[i] - fog[i]) * k)
    };
    colour.map(|c| c.clamp(0.0, 255.0) as u8)
}

/// Direction through face-local `(u, v)` in `-1..=1` of `face`: the inverse
/// of [`psx_engine::sky::cube_face_uv_q12`].
pub fn cube_direction(face: CubeFace, u: f64, v: f64) -> [f64; 3] {
    match face {
        CubeFace::PositiveX => [1.0, -v, -u],
        CubeFace::NegativeX => [-1.0, -v, u],
        CubeFace::PositiveY => [u, 1.0, v],
        CubeFace::NegativeY => [u, -1.0, -v],
        CubeFace::PositiveZ => [u, -v, 1.0],
        CubeFace::NegativeZ => [-u, -v, -1.0],
    }
}

fn point_trace(
    provider: &mut GridTraceProvider<'_, '_>,
    a: [i32; 3],
    b: [i32; 3],
) -> CollisionTrace {
    let mut out = CollisionTrace::unobstructed(RoomPoint::from_array(b));
    provider.trace_into(
        CollisionTraceQuery::point(RoomPoint::from_array(a), RoomPoint::from_array(b)),
        &mut out,
    );
    out
}

/// Bake the sky of room `chunk` of `cooked`: the chunk and every room one
/// portal away are left out, the rest is traced from the chunk's centre.
pub fn bake_chunk_sky(cooked: &CookedLevel, chunk: usize) -> Result<ChunkSky, String> {
    bake_chunk_sky_with(cooked, chunk, None)
}

/// [`bake_chunk_sky`] with hits coloured by `colours` (flat graybox blues
/// without).
pub fn bake_chunk_sky_with(
    cooked: &CookedLevel,
    chunk: usize,
    colours: Option<&WorldColours<'_>>,
) -> Result<ChunkSky, String> {
    let rooms = cooked.level.rooms.len();
    if chunk >= rooms {
        return Err(format!("room {chunk} does not exist ({rooms} rooms)"));
    }
    let chunk_words: Vec<Vec<u32>> = cooked.chunks.iter().map(|c| aligned_copy(c)).collect();
    let views = chunk_words
        .iter()
        .zip(&cooked.chunks)
        .map(|(words, bytes)| {
            RoomView::parse(words_as_bytes(words, bytes.len()))
                .map(|view| Some(view.col))
                .map_err(|e| format!("room chunk does not parse: {e:?}"))
        })
        .collect::<Result<Vec<Option<CollisionView<'_>>>, String>>()?;
    let core_words = aligned_copy(&cooked.core);
    let level = LevelView::parse(words_as_bytes(&core_words, cooked.core.len()))
        .map_err(|e| format!("level core does not parse: {e:?}"))?;
    let mut ring = vec![chunk];
    for portal in level.room_portals(chunk) {
        let dest = usize::from(portal.dest);
        if !ring.contains(&dest) {
            ring.push(dest);
        }
    }
    let s = i32::from(cooked.level.sector_units);
    let record = cooked.level.rooms[chunk];
    let (cx, cz) = (
        record.origin[0] + i32::from(record.width) * s / 2,
        record.origin[2] + i32::from(record.depth) * s / 2,
    );
    // The eye stands on whatever is under the chunk's centre in the whole
    // level.
    let eye_y = {
        let set = ResidentRooms::from_collisions(level, &views)
            .ok_or("the level's rooms do not fit a resident set")?;
        let world = set.collision();
        let mut provider = GridTraceProvider::new(&world);
        let top = record.origin[1] + i32::from(record.max_y) + EYE_HEIGHT;
        let bottom = record.origin[1] + i32::from(record.min_y) - EYE_HEIGHT;
        let down = point_trace(&mut provider, [cx, top, cz], [cx, bottom, cz]);
        if down.hit() && !down.start_solid {
            down.end.y + EYE_HEIGHT
        } else {
            record.origin[1] + EYE_HEIGHT
        }
    };
    let eye = [cx, eye_y, cz];
    // The level without the ring: its rooms stay resident (their chunks are
    // what they are) but their footprints move far outside the level, where
    // no ray reaches. Leaving them out of the resident set instead would make
    // them stand in as solid boxes around the eye.
    let mut core = cooked.level.clone();
    for &room in &ring {
        core.rooms[room].origin = [40_000, 30_000, 40_000];
    }
    let cut_bytes = core.try_to_bytes()?;
    let cut_words = aligned_copy(&cut_bytes);
    let cut = LevelView::parse(words_as_bytes(&cut_words, cut_bytes.len()))
        .map_err(|e| format!("cut level core does not parse: {e:?}"))?;
    let set = ResidentRooms::from_collisions(cut, &views)
        .ok_or("the level's rooms do not fit a resident set")?;
    let world = set.collision();
    let mut provider = GridTraceProvider::new(&world);

    // A room without fog bakes with no haze over a neutral horizon colour.
    let fogged = record.flags & psx_grid::ROOM_FOG != 0;
    let fog = if fogged {
        record.fog_rgb.map(f64::from)
    } else {
        NO_FOG_HORIZON
    };
    let (fog_near, fog_far) = if fogged {
        (f64::from(record.fog_near), f64::from(record.fog_far))
    } else {
        (f64::MAX, f64::MAX)
    };
    let light = {
        let l = [0.4f64, 0.8, 0.3];
        let n = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt();
        [l[0] / n, l[1] / n, l[2] / n]
    };
    let (w, h) = (
        usize::from(CUBE_SKY_FACE_WIDTH),
        usize::from(CUBE_SKY_FACE_HEIGHT),
    );
    let mut rgb = vec![[0u8; 3]; 6 * w * h];
    let mut hits = 0;
    for (index, face) in FACES.into_iter().enumerate() {
        for y in 0..h {
            for x in 0..w {
                let d = texel_direction(face, x, y);
                let end = [
                    eye[0] + (d[0] * REACH) as i32,
                    eye[1] + (d[1] * REACH) as i32,
                    eye[2] + (d[2] * REACH) as i32,
                ];
                let t = point_trace(&mut provider, eye, end);
                rgb[(index * h + y) * w + x] = if !t.hit() || t.start_solid {
                    background_colour(d[1], fog)
                } else {
                    hits += 1;
                    let distance = REACH * f64::from(t.fraction_q12) / 4096.0;
                    let n = t.normal_q12.map(|c| f64::from(c) / 4096.0);
                    let lambert = (n[0] * light[0] + n[1] * light[1] + n[2] * light[2]).max(0.0);
                    let at = [t.end.x, t.end.y, t.end.z];
                    let base = colours.and_then(|c| c.colour(at, n)).unwrap_or(
                        // Tops a little lighter than sides, as the graybox reads.
                        if n[1] > 0.7 {
                            [70.0, 92.0, 150.0]
                        } else {
                            [44.0, 58.0, 110.0]
                        },
                    );
                    let lit = base.map(|c| c * (0.45 + 0.55 * lambert));
                    let k = if fogged {
                        ((distance - fog_near) / (fog_far - fog_near).max(1.0)).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    [0, 1, 2]
                        .map(|i| lit[i] + (fog[i] - lit[i]) * k)
                        .map(|c| c.clamp(0.0, 255.0) as u8)
                };
            }
        }
    }
    Ok(ChunkSky {
        chunk,
        ring,
        eye,
        fog,
        rgb,
        hits,
    })
}

/// The far panels of one room ([`psx_grid::render::panels`]): the picture an
/// eye at the room's centre sees beyond the rooms near it, as 24 strips in
/// the sky atlas's layout, and the depth of each cell of each strip.
pub struct ChunkPanels {
    /// The room baked from. `sky.rgb` is the atlas, face-major like a cube
    /// sky's (strip `s` is at columns `s % 4 * 64` of face `s / 4`); a texel no
    /// ray hit is black, which is index 0, transparent.
    pub sky: ChunkSky,
    /// Depth of each cell in [`DEPTH_UNIT`]s, strip-major; 0 for a cell no
    /// ray hit.
    pub depth: [u8; panels::CELLS],
}

impl ChunkPanels {
    /// The palette index of every texel over `palettes` (index 0 for a miss,
    /// the nearest of the others otherwise), as one row-major 1536x256 image.
    pub fn indices(&self, palettes: &[Vec<[u8; 3]>; 6]) -> Vec<u8> {
        let (w, h) = (
            usize::from(CUBE_SKY_FACE_WIDTH),
            usize::from(CUBE_SKY_FACE_HEIGHT),
        );
        let mut indices = vec![0u8; 6 * w * h];
        for (face, palette) in palettes.iter().enumerate() {
            for y in 0..h {
                for x in 0..w {
                    let texel = self.sky.rgb[(face * h + y) * w + x];
                    if texel != [0, 0, 0] {
                        indices[y * 6 * w + face * w + x] = nearest(&palette[1..], texel) + 1;
                    }
                }
            }
        }
        indices
    }

    /// The atlas as a 4bpp PSXT over `palettes`, index 0 transparent.
    pub fn to_psxt_with(&self, palettes: &[Vec<[u8; 3]>; 6]) -> Result<Vec<u8>, String> {
        psxed_tex::encode_indexed_psxt_with_clut_rows(
            CUBE_SKY_FACE_WIDTH * 6,
            CUBE_SKY_FACE_HEIGHT,
            psxed_format::texture::Depth::Bit4,
            &self.indices(palettes),
            palettes.as_slice(),
            true,
        )
        .map_err(|e| format!("encode the panels: {e}"))
    }

    /// The panels as a sparse entry against [`panel_background`].
    pub fn to_sparse(&self, palettes: &[Vec<[u8; 3]>; 6], background: &SkyBackground) -> Vec<u8> {
        sparse_entry(&self.indices(palettes), background)
    }
}

/// The background of a panel atlas: index 0 (transparent) everywhere.
pub fn panel_background() -> SkyBackground {
    let rows = 6 * usize::from(CUBE_SKY_FACE_HEIGHT);
    SkyBackground {
        rows: (0..=rows as u16).collect(),
        runs: [255u8, 0].repeat(rows),
    }
}

/// One 16-colour palette per atlas page for a whole set of panels, index 0
/// black (transparent) and the other fifteen quantised from every texel a ray
/// hit.
pub fn panel_palettes(panels: &[ChunkPanels]) -> Result<[Vec<[u8; 3]>; 6], String> {
    let (w, h) = (
        usize::from(CUBE_SKY_FACE_WIDTH),
        usize::from(CUBE_SKY_FACE_HEIGHT),
    );
    let mut palettes: [Vec<[u8; 3]>; 6] = Default::default();
    for (face, palette) in palettes.iter_mut().enumerate() {
        let hits: Vec<[u8; 3]> = panels
            .iter()
            .flat_map(|p| p.sky.rgb[face * w * h..(face + 1) * w * h].iter().copied())
            .filter(|texel| *texel != [0, 0, 0])
            .collect();
        let stride = (hits.len() / 262_144).max(1);
        let sample: Vec<[u8; 3]> = hits.into_iter().step_by(stride).collect();
        *palette = vec![[0, 0, 0]];
        if sample.is_empty() {
            continue;
        }
        palette.extend(
            psxed_tex::quantize_rgb(&sample, 15)
                .map_err(|e| format!("panel page {face}: {e}"))?
                .0,
        );
    }
    Ok(palettes)
}

/// The backdrop colours a level of fog `fog` shows through its panels, the
/// colours the cube sky paints a ray that hits nothing: the void under the
/// level, the fog at the horizon, and the sky a little way up.
pub fn panel_backdrop(fog: [f64; 3]) -> panels::Backdrop {
    let q = |c: [f64; 3]| c.map(|v| v.clamp(0.0, 255.0) as u8);
    let top = [fog[0] * 0.35, fog[1] * 0.45, fog[2] * 0.7];
    panels::Backdrop {
        below: q(fog.map(|c| c * 0.6)),
        horizon: q(fog),
        // The top of the screen is about half way to the zenith.
        top: q([0, 1, 2].map(|i| fog[i] + (top[i] - fog[i]) * 0.5)),
    }
}

/// Bake the panels of room `chunk` of `cooked`: the rooms within
/// `near_sectors` sectors of the room's centre are left out, the rest is
/// traced from the centre through each texel of each strip.
pub fn bake_chunk_panels_with(
    cooked: &CookedLevel,
    chunk: usize,
    colours: Option<&WorldColours<'_>>,
    near_sectors: i32,
) -> Result<ChunkPanels, String> {
    let rooms = cooked.level.rooms.len();
    if chunk >= rooms {
        return Err(format!("room {chunk} does not exist ({rooms} rooms)"));
    }
    let chunk_words: Vec<Vec<u32>> = cooked.chunks.iter().map(|c| aligned_copy(c)).collect();
    let views = chunk_words
        .iter()
        .zip(&cooked.chunks)
        .map(|(words, bytes)| {
            RoomView::parse(words_as_bytes(words, bytes.len()))
                .map(|view| Some(view.col))
                .map_err(|e| format!("room chunk does not parse: {e:?}"))
        })
        .collect::<Result<Vec<Option<CollisionView<'_>>>, String>>()?;
    let core_words = aligned_copy(&cooked.core);
    let level = LevelView::parse(words_as_bytes(&core_words, cooked.core.len()))
        .map_err(|e| format!("level core does not parse: {e:?}"))?;
    let s = i32::from(cooked.level.sector_units);
    let record = cooked.level.rooms[chunk];
    let (cx, cz) = (
        record.origin[0] + i32::from(record.width) * s / 2,
        record.origin[2] + i32::from(record.depth) * s / 2,
    );
    // The eye stands on whatever is under the room's centre in the whole
    // level.
    let eye_y = {
        let set = ResidentRooms::from_collisions(level, &views)
            .ok_or("the level's rooms do not fit a resident set")?;
        let world = set.collision();
        let mut provider = GridTraceProvider::new(&world);
        let top = record.origin[1] + i32::from(record.max_y) + EYE_HEIGHT;
        let bottom = record.origin[1] + i32::from(record.min_y) - EYE_HEIGHT;
        let down = point_trace(&mut provider, [cx, top, cz], [cx, bottom, cz]);
        if down.hit() && !down.start_solid {
            down.end.y + EYE_HEIGHT
        } else {
            record.origin[1] + EYE_HEIGHT
        }
    };
    let eye = [cx, eye_y, cz];
    // The rooms the runtime draws whole from anywhere in this room are left
    // out: those within `near_sectors` of the eye. They move far outside the
    // level, as the cube sky's ring does, so no ray starts inside one.
    let near = i64::from(near_sectors.max(0)) * i64::from(s);
    let gap = |axis: usize, lo: i32, size: i32| {
        i64::from((lo - eye[axis]).max(eye[axis] - (lo + size)).max(0))
    };
    let excluded: Vec<usize> = (0..rooms)
        .filter(|&room| {
            let r = cooked.level.rooms[room];
            let (gx, gz) = (
                gap(0, r.origin[0], i32::from(r.width) * s),
                gap(2, r.origin[2], i32::from(r.depth) * s),
            );
            room == chunk || gx * gx + gz * gz <= near * near
        })
        .collect();
    let mut core = cooked.level.clone();
    for &room in &excluded {
        core.rooms[room].origin = [40_000, 30_000, 40_000];
    }
    let cut_bytes = core.try_to_bytes()?;
    let cut_words = aligned_copy(&cut_bytes);
    let cut = LevelView::parse(words_as_bytes(&cut_words, cut_bytes.len()))
        .map_err(|e| format!("cut level core does not parse: {e:?}"))?;
    let set = ResidentRooms::from_collisions(cut, &views)
        .ok_or("the level's rooms do not fit a resident set")?;
    let world = set.collision();
    let mut provider = GridTraceProvider::new(&world);

    let fogged = record.flags & psx_grid::ROOM_FOG != 0;
    let fog = if fogged {
        record.fog_rgb.map(f64::from)
    } else {
        NO_FOG_HORIZON
    };
    let (fog_near, fog_far) = if fogged {
        (f64::from(record.fog_near), f64::from(record.fog_far))
    } else {
        (f64::MAX, f64::MAX)
    };
    let light = {
        let l = [0.4f64, 0.8, 0.3];
        let n = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt();
        [l[0] / n, l[1] / n, l[2] / n]
    };
    let (face_w, face_h) = (
        usize::from(CUBE_SKY_FACE_WIDTH),
        usize::from(CUBE_SKY_FACE_HEIGHT),
    );
    let half_width = f64::from(panels::HALF_WIDTH_Q12) / 4096.0;
    let half_height = f64::from(panels::HALF_HEIGHT_Q12) / 4096.0;
    let mut rgb = vec![[0u8; 3]; 6 * face_w * face_h];
    let mut cells: Vec<Vec<u16>> = vec![Vec::new(); panels::CELLS];
    let mut hits = 0;
    for strip in 0..panels::STRIPS {
        let theta = (strip as f64 * 15.0).to_radians();
        let n = [theta.cos(), 0.0, theta.sin()];
        let t = [-theta.sin(), 0.0, theta.cos()];
        let (face, column) = (
            strip / panels::STRIPS_PER_PAGE,
            strip % panels::STRIPS_PER_PAGE * panels::STRIP_TEXELS,
        );
        for y in 0..panels::ROWS {
            let h = half_height * (1.0 - (y as f64 + 0.5) / (panels::ROWS as f64 / 2.0));
            for x in 0..panels::STRIP_TEXELS {
                let u = half_width * ((x as f64 + 0.5) / (panels::STRIP_TEXELS as f64 / 2.0) - 1.0);
                // The ray's depth along the strip's heading is 1 per unit of
                // its length, so a ray `REACH` long reaches depth `REACH`.
                let d = [n[0] + t[0] * u, h, n[2] + t[2] * u];
                let end = [
                    eye[0] + (d[0] * REACH) as i32,
                    eye[1] + (d[1] * REACH) as i32,
                    eye[2] + (d[2] * REACH) as i32,
                ];
                let hit = point_trace(&mut provider, eye, end);
                if !hit.hit() || hit.start_solid {
                    continue;
                }
                hits += 1;
                let depth = REACH * f64::from(hit.fraction_q12) / 4096.0;
                let distance = depth * (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                let normal = hit.normal_q12.map(|c| f64::from(c) / 4096.0);
                let lambert =
                    (normal[0] * light[0] + normal[1] * light[1] + normal[2] * light[2]).max(0.0);
                let at = [hit.end.x, hit.end.y, hit.end.z];
                let base =
                    colours
                        .and_then(|c| c.colour(at, normal))
                        .unwrap_or(if normal[1] > 0.7 {
                            [70.0, 92.0, 150.0]
                        } else {
                            [44.0, 58.0, 110.0]
                        });
                let lit = base.map(|c| c * (0.45 + 0.55 * lambert));
                let k = if fogged {
                    ((distance - fog_near) / (fog_far - fog_near).max(1.0)).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                // Never pure black: that is a miss.
                rgb[(face * face_h + y) * face_w + column + x] = [0, 1, 2]
                    .map(|i| lit[i] + (fog[i] - lit[i]) * k)
                    .map(|c| c.clamp(8.0, 255.0) as u8);
                let steps = (depth / f64::from(DEPTH_UNIT)).round().clamp(1.0, 255.0);
                cells[strip * panels::BANDS + y / panels::BAND_ROWS].push(steps as u16);
            }
        }
    }
    let mut depth = [0u8; panels::CELLS];
    for (byte, found) in depth.iter_mut().zip(&mut cells) {
        if !found.is_empty() {
            found.sort_unstable();
            *byte = found[found.len() / 2] as u8;
        }
    }
    Ok(ChunkPanels {
        sky: ChunkSky {
            chunk,
            ring: excluded,
            eye,
            fog,
            rgb,
            hits,
        },
        depth,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::cook::tests::cook;
    use crate::grid::room::GridRoom;
    use crate::grid::surface::GridSurface;
    use crate::grid::world::GridWorld;
    use psx_engine::sky::{cube_face, cube_face_uv_q12};

    #[test]
    fn cube_directions_land_back_on_their_face_and_texel() {
        for face in FACES {
            for (u, v) in [(0.0, 0.0), (0.5, -0.25), (-0.9, 0.9)] {
                let d = cube_direction(face, u, v);
                let q = d.map(|c| (c * 4096.0) as i32);
                assert_eq!(cube_face(q), face);
                let uv = cube_face_uv_q12(q, face);
                assert!((f64::from(uv[0]) / 4096.0 - u).abs() < 0.01, "{face:?} u");
                assert!((f64::from(uv[1]) / 4096.0 - v).abs() < 0.01, "{face:?} v");
            }
        }
    }

    /// A floored room at `origin` (world sectors), its cells raised to
    /// `height`.
    fn block(origin: [i32; 2], w: usize, d: usize, height: i32) -> GridRoom {
        let mut room = GridRoom::new_derived("block", w, d);
        room.origin = origin;
        for z in 0..d {
            for x in 0..w {
                room.sector_or_insert(x, z)
                    .unwrap()
                    .set_ground_floor(Some(GridSurface::flat(height, None)));
            }
        }
        room
    }

    #[test]
    fn the_bake_sees_far_rooms_and_not_its_own_ring() {
        // A floor to stand on, a raised block beside it (its neighbour, so
        // in the ring), and a walled tower far to the east (not in the ring).
        let mut tower = block([40, 0], 2, 2, 0);
        // A roofless room closes its edges only with an open wall height.
        tower.defaults.open_wall = 8192;
        let world = GridWorld {
            rooms: vec![block([0, 0], 2, 2, 0), block([2, 0], 2, 2, 8192), tower],
            ..GridWorld::default()
        };
        let cooked = cook(&world).unwrap();
        let sky = bake_chunk_sky(&cooked, 0).unwrap();
        assert!(
            sky.ring.contains(&1) && !sky.ring.contains(&2),
            "{:?}",
            sky.ring
        );
        let (w, h) = (256, 256);
        let texel = |face: usize, x: usize, y: usize| sky.rgb[(face * h + y) * w + x];
        // Looking east (+X, face 0) just above the horizon meets the far
        // block; looking west (-X, face 1) there is only sky. The near block
        // sits east too, but it is in the ring.
        let east = texel(0, 128, 120);
        let west = texel(1, 128, 120);
        assert_ne!(east, west);
        assert!(sky.hits > 0);
        let psxt = sky.to_psxt().unwrap();
        let texture = psx_asset::Texture::from_bytes(&psxt).unwrap();
        assert_eq!([texture.width(), texture.height()], [1536, 256]);
        assert_eq!(texture.clut_entries(), 96);
    }

    /// The test world: a floor to stand on, a block in the ring, a far tower.
    fn far_tower_world() -> GridWorld {
        let mut tower = block([40, 0], 2, 2, 0);
        tower.defaults.open_wall = 8192;
        GridWorld {
            rooms: vec![block([0, 0], 2, 2, 0), block([2, 0], 2, 2, 8192), tower],
            ..GridWorld::default()
        }
    }

    #[test]
    fn a_sparse_sky_decodes_to_the_dense_indices_exactly() {
        use psx_level::sky_tiles as st;
        let cooked = cook(&far_tower_world()).unwrap();
        let sky = bake_chunk_sky(&cooked, 0).unwrap();
        let palettes = shared_palettes(core::slice::from_ref(&sky)).unwrap();
        let background = SkyBackground::new(sky.fog, &palettes);
        let entry = sky.to_sparse(&palettes, &background).unwrap();
        let count = st::header_tile_count(&entry).unwrap();
        assert_eq!(entry.len(), st::entry_bytes(count));
        // The mask holds exactly the tiles the entry stores.
        let stored: usize = entry[st::HEADER_BYTES..st::TILES_OFFSET]
            .iter()
            .map(|b| b.count_ones() as usize)
            .sum();
        assert_eq!(stored, count);
        // The tower is in it, and most of the sky is not.
        assert!(count > 0 && count < st::TILE_COUNT / 4, "{count} tiles");
        let mut decoded = vec![0u8; 6 * 256 * 256];
        st::decode_indices(&entry, &background.view(), &mut decoded).unwrap();
        assert_eq!(decoded, sky.indices(&palettes));
    }

    #[test]
    fn the_background_is_the_index_of_every_texel_no_ray_hit() {
        let cooked = cook(&far_tower_world()).unwrap();
        let sky = bake_chunk_sky(&cooked, 0).unwrap();
        let palettes = shared_palettes(core::slice::from_ref(&sky)).unwrap();
        let background = SkyBackground::new(sky.fog, &palettes);
        let view = background.view();
        let (w, h) = (256, 256);
        let mut missed = 0;
        for face in 0..6 {
            for y in 0..h {
                for x in 0..w {
                    let d = texel_direction(FACES[face], x, y);
                    let rgb = sky.rgb[(face * h + y) * w + x];
                    if rgb == background_colour(d[1], sky.fog) {
                        missed += 1;
                        assert_eq!(
                            usize::from(view.index_at(face, x, y)),
                            usize::from(nearest(&palettes[face], rgb)),
                            "face {face} ({x}, {y})"
                        );
                    }
                }
            }
        }
        // Nearly every ray of this world misses.
        assert!(missed > 6 * w * h * 9 / 10);
        // Every row ends its last run on column 255.
        for row in 0..6 * h {
            let end = usize::from(background.rows[row + 1]) * 2;
            assert_eq!(background.runs[end - 2], 255);
        }
        assert_eq!(background.rows.len(), 6 * h + 1);
    }

    /// The depth byte of cell `band` of heading `strip`, in sectors.
    fn depth_sectors(panels: &ChunkPanels, strip: usize, band: usize) -> f64 {
        f64::from(panels.depth[strip * panels::BANDS + band]) * f64::from(DEPTH_UNIT) / 80.0
    }

    #[test]
    fn panels_show_the_far_tower_at_its_depth_and_nothing_else() {
        // Room 0 is the eye's, room 1 (a raised block beside it) is within
        // three sectors and left out, the tower's wall stands 38 sectors east.
        let cooked = cook(&far_tower_world()).unwrap();
        let s = f64::from(cooked.level.sector_units);
        assert_eq!(s, 80.0);
        let panels = bake_chunk_panels_with(&cooked, 0, None, 3).unwrap();
        assert!(panels.sky.ring.contains(&0) && panels.sky.ring.contains(&1));
        assert!(!panels.sky.ring.contains(&2));
        // Strip 0 looks along +X at the tower: some cell of it sits near the
        // tower's 39 sectors, in the bands around the horizon.
        let east: Vec<f64> = (0..panels::BANDS)
            .map(|b| depth_sectors(&panels, 0, b))
            .filter(|&d| d > 0.0)
            .collect();
        assert!(!east.is_empty(), "{:?}", panels.depth);
        for d in &east {
            assert!((d - 38.0).abs() < 3.0, "tower at {d} sectors");
        }
        // Every strip that faces away (the quarter turn past 90 degrees to
        // 270) has nothing in it.
        for strip in 7..=17 {
            for band in 0..panels::BANDS {
                assert_eq!(
                    panels.depth[strip * panels::BANDS + band],
                    0,
                    "strip {strip}"
                );
            }
        }
        assert!(panels.sky.hits > 0);
    }

    #[test]
    fn a_larger_near_radius_leaves_out_more_rooms() {
        let cooked = cook(&far_tower_world()).unwrap();
        // The block beside the eye is two sectors off: in at 3, out at 0.
        let wide = bake_chunk_panels_with(&cooked, 0, None, 3).unwrap();
        let narrow = bake_chunk_panels_with(&cooked, 0, None, 0).unwrap();
        assert!(wide.sky.ring.len() > narrow.sky.ring.len());
        // The room itself is always left out.
        assert_eq!(narrow.sky.ring, vec![0]);
    }

    #[test]
    fn a_panel_atlas_is_sparse_against_a_transparent_background_and_decodes_exactly() {
        use psx_level::sky_tiles as st;
        let cooked = cook(&far_tower_world()).unwrap();
        let panels = vec![bake_chunk_panels_with(&cooked, 0, None, 3).unwrap()];
        let palettes = panel_palettes(&panels).unwrap();
        // Index 0 is black in every palette, the rest never are.
        for palette in &palettes {
            assert_eq!(palette[0], [0, 0, 0]);
        }
        let background = panel_background();
        let entry = panels[0].to_sparse(&palettes, &background);
        let count = st::header_tile_count(&entry).unwrap();
        // The tower is a small part of 6 pages of mostly empty strips.
        assert!(count > 0 && count < st::TILE_COUNT / 8, "{count} tiles");
        let mut decoded = vec![0u8; 6 * 256 * 256];
        st::decode_indices(&entry, &background.view(), &mut decoded).unwrap();
        let indices = panels[0].indices(&palettes);
        assert_eq!(decoded, indices);
        // Texels that hit something are never index 0.
        let hits = indices.iter().filter(|&&i| i != 0).count();
        assert_eq!(hits, panels[0].sky.hits);
        // The PSXT keeps index 0 transparent.
        let psxt = panels[0].to_psxt_with(&palettes).unwrap();
        let texture = psx_asset::Texture::from_bytes(&psxt).unwrap();
        assert!(texture.is_index_zero_transparent());
        assert_eq!([texture.width(), texture.height()], [1536, 256]);
        assert_eq!(texture.clut_entries(), 96);
    }

    #[test]
    fn the_backdrop_is_the_cube_skys_miss_colours() {
        let fog = [34.0, 38.0, 40.0];
        let backdrop = panel_backdrop(fog);
        assert_eq!(backdrop.below, background_colour(-0.5, fog));
        assert_eq!(backdrop.horizon, background_colour(0.0, fog));
        let top = background_colour(1.0, fog);
        for i in 0..3 {
            assert!(backdrop.top[i] <= backdrop.horizon[i].max(top[i]));
            assert!(backdrop.top[i] >= top[i].min(backdrop.horizon[i]));
        }
    }
}
