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
    /// endpoints are on opposite sides, so the denominator is not zero.
    fn cross(self, a: ClipVertex, b: ClipVertex) -> ClipVertex {
        let (num, den) = match self {
            Edge::Left(e) | Edge::Right(e) => (e - a.x, b.x - a.x),
            Edge::Top(e) | Edge::Bottom(e) => (e - a.y, b.y - a.y),
        };
        let lerp = |p: i32, q: i32| p + (q - p) * num / den;
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
pub(crate) fn clip_triangle(
    tri: [ClipVertex; 3],
    rect: ClipRect,
) -> ([ClipVertex; MAX_CLIPPED], usize) {
    // A vertex on the right/bottom boundary x1 still rasterises no pixel at
    // x1, so clipping to x1 itself (inclusive here) keeps the half-open rule.
    let edges = [
        Edge::Left(rect.x0),
        Edge::Right(rect.x1),
        Edge::Top(rect.y0),
        Edge::Bottom(rect.y1),
    ];
    let zero = tri[0];
    let mut poly = [zero; MAX_CLIPPED];
    poly[..3].copy_from_slice(&tri);
    let mut len = 3usize;
    for edge in edges {
        let mut next = [zero; MAX_CLIPPED];
        let mut next_len = 0usize;
        for i in 0..len {
            let a = poly[i];
            let b = poly[(i + 1) % len];
            let (a_in, b_in) = (edge.inside(a), edge.inside(b));
            if a_in {
                next[next_len] = a;
                next_len += 1;
            }
            if a_in != b_in {
                next[next_len] = edge.cross(a, b);
                next_len += 1;
            }
        }
        poly = next;
        len = next_len;
        if len < 3 {
            return (poly, 0);
        }
    }
    (poly, len)
}
