//! Rectangle clipping of one Gouraud triangle in integer pixels.

use crate::draw::ClipRect;

/// A clip-space vertex: pixel position and 8-bit colour channels.
#[derive(Clone, Copy)]
pub(crate) struct ClipVertex {
    pub x: i32,
    pub y: i32,
    pub r: i32,
    pub g: i32,
    pub b: i32,
}

impl ClipVertex {
    /// Unpack a position word (`x` low half, `y` high half) and a colour word.
    #[inline(always)]
    pub fn from_words(pos: u32, color: u32) -> Self {
        Self {
            x: pos as u16 as i16 as i32,
            y: (pos >> 16) as u16 as i16 as i32,
            r: (color & 0xFF) as i32,
            g: ((color >> 8) & 0xFF) as i32,
            b: ((color >> 16) & 0xFF) as i32,
        }
    }

    #[inline(always)]
    pub fn pos_word(self) -> u32 {
        (self.x as u16 as u32) | ((self.y as u16 as u32) << 16)
    }

    #[inline(always)]
    pub fn color_word(self) -> u32 {
        (self.r as u32 & 0xFF) | ((self.g as u32 & 0xFF) << 8) | ((self.b as u32 & 0xFF) << 16)
    }
}

/// The most vertices a triangle clipped against four edges can have.
pub(crate) const MAX_CLIPPED: usize = 7;

/// Which side of a rectangle edge a coordinate is on.
#[derive(Clone, Copy)]
enum Edge {
    Left(i32),
    Right(i32),
    Top(i32),
    Bottom(i32),
}

impl Edge {
    #[inline]
    fn inside(self, v: ClipVertex) -> bool {
        match self {
            Edge::Left(e) => v.x >= e,
            Edge::Right(e) => v.x <= e,
            Edge::Top(e) => v.y >= e,
            Edge::Bottom(e) => v.y <= e,
        }
    }

    /// The point where segment `a`-`b` crosses this edge. Only called when the
    /// endpoints are on opposite sides, so the denominator is not zero. One
    /// division gives the crossing as a Q12 fraction; every channel then
    /// interpolates with a multiply, since a divide costs far more on the R3000.
    fn cross(self, a: ClipVertex, b: ClipVertex) -> ClipVertex {
        let (num, den) = match self {
            Edge::Left(e) | Edge::Right(e) => (e - a.x, b.x - a.x),
            Edge::Top(e) | Edge::Bottom(e) => (e - a.y, b.y - a.y),
        };
        let t = (num << 12) / den;
        let lerp = |p: i32, q: i32| p + (((q - p) * t + 2048) >> 12);
        let mut out = ClipVertex {
            x: lerp(a.x, b.x),
            y: lerp(a.y, b.y),
            r: lerp(a.r, b.r),
            g: lerp(a.g, b.g),
            b: lerp(a.b, b.b),
        };
        match self {
            Edge::Left(e) | Edge::Right(e) => out.x = e,
            Edge::Top(e) | Edge::Bottom(e) => out.y = e,
        }
        out
    }
}

/// Clip a triangle to `rect` (half-open: pixels `x0 <= x < x1`, `y0 <= y < y1`)
/// with Sutherland-Hodgman, colours interpolated linearly. Returns the polygon
/// vertices and their count; a count below 3 means nothing is left.
///
/// Only the edges the triangle's bounding box actually crosses are processed,
/// which for a typical straddling triangle is one or two of the four.
#[inline(never)]
pub(crate) fn clip_triangle(
    tri: [ClipVertex; 3],
    rect: ClipRect,
) -> ([ClipVertex; MAX_CLIPPED], usize) {
    let min_x = tri[0].x.min(tri[1].x).min(tri[2].x);
    let max_x = tri[0].x.max(tri[1].x).max(tri[2].x);
    let min_y = tri[0].y.min(tri[1].y).min(tri[2].y);
    let max_y = tri[0].y.max(tri[1].y).max(tri[2].y);
    // A vertex on the right/bottom boundary x1 still rasterises no pixel at
    // x1, so clipping to x1 itself (inclusive here) keeps the half-open rule.
    let edges = [
        (min_x < rect.x0, Edge::Left(rect.x0)),
        (max_x > rect.x1, Edge::Right(rect.x1)),
        (min_y < rect.y0, Edge::Top(rect.y0)),
        (max_y > rect.y1, Edge::Bottom(rect.y1)),
    ];
    let zero = tri[0];
    let mut poly = [zero; MAX_CLIPPED];
    poly[..3].copy_from_slice(&tri);
    let mut len = 3usize;
    for (needed, edge) in edges {
        if !needed {
            continue;
        }
        let mut next = [zero; MAX_CLIPPED];
        let mut next_len = 0usize;
        let mut a = poly[len - 1];
        let mut a_in = edge.inside(a);
        for &b in &poly[..len] {
            let b_in = edge.inside(b);
            if a_in != b_in {
                next[next_len] = edge.cross(a, b);
                next_len += 1;
            }
            if b_in {
                next[next_len] = b;
                next_len += 1;
            }
            a = b;
            a_in = b_in;
        }
        poly = next;
        len = next_len;
        if len < 3 {
            return (poly, 0);
        }
    }
    (poly, len)
}
