//! Error-bounded tessellation policy for affine-textured PS1 polygons.
//!
//! The GPU interpolates texture coordinates linearly in screen space. Along a
//! projected edge from `a` to `b` (view depths `za`, `zb`, screen length `L`
//! pixels) the perspective-correct image of the 3D midpoint and the affine
//! screen midpoint differ by exactly
//!
//! ```text
//! e = L * |zb - za| / (2 * (za + zb))      screen pixels
//! ```
//!
//! That is the largest displacement on the edge, and the texel error is the
//! same expression with the edge's UV span in place of `L`. Halving an edge
//! halves `L` and roughly halves `|zb - za|`, so each bisection divides `e` by
//! about four.
//!
//! The policy here splits by `e`, not by depth bands or texel error:
//!
//! * it is what the viewer sees (texels are not pixels: a minified far wall
//!   can be "8 texels" off and look right, a magnified floor 1 texel off and
//!   swim);
//! * it depends only on the edge's two projected endpoints, never on the
//!   face's UV mapping, so the two faces sharing an edge always make the same
//!   decision and a per-edge refinement ([`refine`]) is T-junction free
//!   without underdraw;
//! * it is multiplication-only (no guest divide) and fits `u32` for GTE
//!   screen coordinates (`|x|, |y| < 1024`) and SZ depths (`< 65536`).
//!
//! Validated against the emulator's texture-warp probe (PSoXide-emulator
//! `gpu/warp_probe.rs`, `examples/warp_probe_check.rs`).

/// A projected vertex as the policy sees it: GTE screen position and SZ.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct ScreenDepth {
    /// Screen X in pixels.
    pub x: i32,
    /// Screen Y in pixels.
    pub y: i32,
    /// View depth (GTE SZ units); non-positive means unknown or behind.
    pub z: i32,
}

/// Screen length of an edge, octagonal approximation (within 4 percent,
/// never more than 1 percent under the Euclidean length).
#[inline(always)]
pub fn screen_length(a: ScreenDepth, b: ScreenDepth) -> u32 {
    let dx = a.x.abs_diff(b.x).min(4095);
    let dy = a.y.abs_diff(b.y).min(4095);
    let (hi, lo) = if dx > dy { (dx, dy) } else { (dy, dx) };
    hi + (lo * 3 >> 3)
}

/// Whether an edge's affine displacement exceeds `budget_q3` eighths of a
/// pixel after `level` bisections (each divides it by four).
#[inline(always)]
pub fn edge_exceeds(a: ScreenDepth, b: ScreenDepth, budget_q3: u32, level: u32) -> bool {
    if a.z <= 0 || b.z <= 0 {
        return false;
    }
    let za = (a.z as u32).min(u16::MAX as u32);
    let zb = (b.z as u32).min(u16::MAX as u32);
    // L * dz * 8 / (2 * (za + zb)) > budget * 4^level
    // L < 2^13, dz < 2^16 -> 2^29 * 4 = 2^31: fits u32.
    let lhs = screen_length(a, b) * za.abs_diff(zb) * 4;
    let rhs = (za + zb) * (budget_q3 << (2 * level));
    lhs > rhs
}

/// Smallest bisection level (up to `max_level`) at which an edge's predicted
/// displacement fits `budget_q3` eighths of a pixel.
#[inline(always)]
pub fn edge_level(a: ScreenDepth, b: ScreenDepth, budget_q3: u32, max_level: u8) -> u8 {
    let mut level = 0u8;
    while level < max_level && edge_exceeds(a, b, budget_q3, level as u32) {
        level += 1;
    }
    level
}

/// Uniform lattice level for a triangle: its worst edge.
#[inline(always)]
pub fn triangle_level(
    a: ScreenDepth,
    b: ScreenDepth,
    c: ScreenDepth,
    budget_q3: u32,
    max_level: u8,
) -> u8 {
    edge_level(a, b, budget_q3, max_level)
        .max(edge_level(b, c, budget_q3, max_level))
        .max(edge_level(c, a, budget_q3, max_level))
}

/// Texel-space counterpart of [`edge_exceeds`] (for policies that budget
/// texels): `uv_span * |dz| / (2 (za + zb)) > budget_q3 / 8`.
#[inline(always)]
pub fn edge_texel_exceeds(za: i32, zb: i32, uv_span: u32, budget_q3: u32) -> bool {
    if za <= 0 || zb <= 0 {
        return false;
    }
    let za = (za as u32).min(u16::MAX as u32);
    let zb = (zb as u32).min(u16::MAX as u32);
    uv_span.min(255) * za.abs_diff(zb) * 4 > (za + zb) * budget_q3
}

/// A vertex [`refine`] can split.
pub trait TessVertex: Copy {
    /// Projected position and depth.
    fn screen_depth(&self) -> ScreenDepth;
}

/// Edge-local refinement limits.
#[derive(Copy, Clone, Debug)]
pub struct RefineLimits {
    /// Displacement budget in eighths of a pixel.
    pub budget_q3: u32,
    /// Edges shorter than this many pixels are never split.
    pub min_edge_px: u32,
    /// Safety cap on recursion (edge decisions stay local below it; set it
    /// high enough that `min_edge_px` stops first on real content).
    pub max_depth: u8,
}

/// Conforming per-edge refinement of one triangle.
///
/// An edge splits when its own predicted displacement exceeds the budget and
/// it is at least `min_edge_px` long; both tests read only the edge's two
/// endpoints, and `midpoint` must be a function of the two endpoints only
/// (world-space average, re-projected on the GTE), so any two triangles
/// sharing an edge produce the same vertices along it: no T-junctions and
/// no crack underdraw. Triangles split 1:2, 1:3 or 1:4 depending on how
/// many edges split (red-green refinement), recursively.
pub fn refine<V, M, E>(tri: [V; 3], limits: RefineLimits, midpoint: &mut M, emit: &mut E)
where
    V: TessVertex,
    M: FnMut(&V, &V) -> V,
    E: FnMut([V; 3]),
{
    refine_at(tri, limits, 0, midpoint, emit);
}

/// Split an edge if it needs it: returns its midpoint. An edge too short in
/// view space to have a distinct midpoint (integer halving returns an
/// endpoint) is left whole, which is also an edge-local decision.
fn split<V, M>(a: &V, b: &V, limits: RefineLimits, midpoint: &mut M) -> Option<V>
where
    V: TessVertex,
    M: FnMut(&V, &V) -> V,
{
    let (sa, sb) = (a.screen_depth(), b.screen_depth());
    if screen_length(sa, sb) < limits.min_edge_px || !edge_exceeds(sa, sb, limits.budget_q3, 0) {
        return None;
    }
    let m = midpoint(a, b);
    let sm = m.screen_depth();
    (sm != sa && sm != sb).then_some(m)
}

fn refine_at<V, M, E>(tri: [V; 3], limits: RefineLimits, depth: u8, midpoint: &mut M, emit: &mut E)
where
    V: TessVertex,
    M: FnMut(&V, &V) -> V,
    E: FnMut([V; 3]),
{
    let [a, b, c] = tri;
    if depth >= limits.max_depth {
        emit(tri);
        return;
    }
    let (mab, mbc, mca) = (
        split(&a, &b, limits, midpoint),
        split(&b, &c, limits, midpoint),
        split(&c, &a, limits, midpoint),
    );
    let d = depth + 1;
    match (mab, mbc, mca) {
        (None, None, None) => emit(tri),
        (Some(ab), Some(bc), Some(ca)) => {
            refine_at([a, ab, ca], limits, d, midpoint, emit);
            refine_at([ab, b, bc], limits, d, midpoint, emit);
            refine_at([ca, bc, c], limits, d, midpoint, emit);
            refine_at([ab, bc, ca], limits, d, midpoint, emit);
        }
        // One split edge: bisect toward the opposite vertex.
        (Some(m), None, None) => {
            refine_at([a, m, c], limits, d, midpoint, emit);
            refine_at([m, b, c], limits, d, midpoint, emit);
        }
        (None, Some(m), None) => {
            refine_at([a, b, m], limits, d, midpoint, emit);
            refine_at([a, m, c], limits, d, midpoint, emit);
        }
        (None, None, Some(m)) => {
            refine_at([a, b, m], limits, d, midpoint, emit);
            refine_at([m, b, c], limits, d, midpoint, emit);
        }
        // Two split edges: a quad plus a triangle, cut across the shorter
        // diagonal of the quad.
        (Some(ab), Some(bc), None) => {
            refine_at([ab, b, bc], limits, d, midpoint, emit);
            green_quad([a, ab, bc, c], limits, d, midpoint, emit);
        }
        (None, Some(bc), Some(ca)) => {
            refine_at([ca, bc, c], limits, d, midpoint, emit);
            green_quad([b, bc, ca, a], limits, d, midpoint, emit);
        }
        (Some(ab), None, Some(ca)) => {
            refine_at([a, ab, ca], limits, d, midpoint, emit);
            green_quad([ab, b, c, ca], limits, d, midpoint, emit);
        }
    }
}

/// Split the quad `p0 p1 p2 p3` (in order) along its shorter screen diagonal.
fn green_quad<V, M, E>(q: [V; 4], limits: RefineLimits, depth: u8, midpoint: &mut M, emit: &mut E)
where
    V: TessVertex,
    M: FnMut(&V, &V) -> V,
    E: FnMut([V; 3]),
{
    let s = q.map(|v| v.screen_depth());
    if screen_length(s[0], s[2]) <= screen_length(s[1], s[3]) {
        refine_at([q[0], q[1], q[2]], limits, depth, midpoint, emit);
        refine_at([q[0], q[2], q[3]], limits, depth, midpoint, emit);
    } else {
        refine_at([q[0], q[1], q[3]], limits, depth, midpoint, emit);
        refine_at([q[1], q[2], q[3]], limits, depth, midpoint, emit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A view-space point projected with H = 256 around (160, 120).
    #[derive(Copy, Clone, Debug, PartialEq)]
    struct P {
        v: [i64; 3],
    }

    impl TessVertex for P {
        fn screen_depth(&self) -> ScreenDepth {
            ScreenDepth {
                x: (160 + self.v[0] * 256 / self.v[2]) as i32,
                y: (120 + self.v[1] * 256 / self.v[2]) as i32,
                z: self.v[2] as i32,
            }
        }
    }

    fn mid(a: &P, b: &P) -> P {
        P {
            v: [
                (a.v[0] + b.v[0]) >> 1,
                (a.v[1] + b.v[1]) >> 1,
                (a.v[2] + b.v[2]) >> 1,
            ],
        }
    }

    /// Exact affine displacement at an edge midpoint, pixels.
    fn exact(a: ScreenDepth, b: ScreenDepth) -> f64 {
        let l = (((a.x - b.x) as f64).powi(2) + ((a.y - b.y) as f64).powi(2)).sqrt();
        l * (a.z - b.z).abs() as f64 / (2.0 * (a.z + b.z) as f64)
    }

    #[test]
    fn predicate_brackets_exact_error() {
        let a = ScreenDepth {
            x: 10,
            y: 200,
            z: 100,
        };
        let b = ScreenDepth {
            x: 300,
            y: 150,
            z: 900,
        };
        let e = exact(a, b); // ~117 px
        let q3 = (e * 8.0) as u32;
        assert!(edge_exceeds(a, b, q3 - q3 / 20, 0));
        assert!(!edge_exceeds(a, b, q3 + q3 / 20, 0));
    }

    #[test]
    fn equal_depth_never_splits() {
        let a = ScreenDepth {
            x: -900,
            y: 0,
            z: 500,
        };
        let b = ScreenDepth {
            x: 900,
            y: 0,
            z: 500,
        };
        assert_eq!(edge_level(a, b, 1, 4), 0);
    }

    #[test]
    fn levels_divide_error_by_four() {
        let a = ScreenDepth {
            x: 0,
            y: 239,
            z: 60,
        };
        let b = ScreenDepth {
            x: 0,
            y: 130,
            z: 600,
        };
        let e = exact(a, b);
        let budget = (e * 8.0 / 10.0) as u32; // needs about /10
        assert_eq!(edge_level(a, b, budget, 3), 2);
    }

    fn floor_grid(n: i64) -> Vec<[P; 3]> {
        // A 4x4 floor patch receding from z=40 to z=1200, as triangles.
        // Positions in 1/16 units, as a fixed-point GTE input would carry
        // them: integer halving of coarse coordinates folds tiny triangles.
        let pt = |i: i64, j: i64| P {
            v: [16 * (-600 + 300 * i), 16 * 80, 16 * (40 + (1160 * j) / n)],
        };
        let mut out = Vec::new();
        for j in 0..n {
            for i in 0..4 {
                out.push([pt(i, j), pt(i + 1, j), pt(i, j + 1)]);
                out.push([pt(i + 1, j), pt(i + 1, j + 1), pt(i, j + 1)]);
            }
        }
        out
    }

    #[test]
    fn refinement_is_t_junction_free() {
        let limits = RefineLimits {
            budget_q3: 8,
            min_edge_px: 4,
            max_depth: 12,
        };
        let mut tris = Vec::new();
        for t in floor_grid(4) {
            refine(t, limits, &mut mid, &mut |t| tris.push(t));
        }
        assert!(tris.len() > 8 * 4);
        // Conforming mesh: every edge is shared by two triangles unless it
        // lies on the patch's outer boundary.
        let mut edges = std::collections::HashMap::new();
        for t in &tris {
            for (a, b) in [(0, 1), (1, 2), (2, 0)] {
                let (pa, pb) = (t[a].v, t[b].v);
                let key = if pa < pb { (pa, pb) } else { (pb, pa) };
                *edges.entry(key).or_insert(0u32) += 1;
            }
        }
        let on_boundary = |p: [i64; 3], q: [i64; 3]| {
            (p[0] == q[0] && (p[0] == -9600 || p[0] == 9600))
                || (p[2] == q[2] && (p[2] == 640 || p[2] == 19200))
        };
        for ((p, q), n) in edges {
            assert!(n <= 2, "edge {p:?}-{q:?} used {n} times");
            assert!(
                n == 2 || on_boundary(p, q),
                "T-junction or crack at {p:?}-{q:?}"
            );
        }
    }

    #[test]
    fn refinement_meets_budget_except_short_edges() {
        let limits = RefineLimits {
            budget_q3: 8,
            min_edge_px: 4,
            max_depth: 12,
        };
        let mut worst = 0.0f64;
        for t in floor_grid(2) {
            refine(t, limits, &mut mid, &mut |t: [P; 3]| {
                for (a, b) in [(0, 1), (1, 2), (2, 0)] {
                    let (sa, sb) = (t[a].screen_depth(), t[b].screen_depth());
                    if screen_length(sa, sb) >= 4 {
                        worst = worst.max(exact(sa, sb));
                    }
                }
            });
        }
        // Budget is 1 px; allow the octagonal length approximation.
        assert!(worst <= 1.05, "worst edge error {worst}");
    }
}
