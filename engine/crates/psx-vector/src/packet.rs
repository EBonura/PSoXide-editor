//! Self-contained triangle packets for ordering tables.
//!
//! Untextured semi-transparency reads its equation from the GPU draw mode, and
//! an ordering table gives no promise about what ran just before a node. So an
//! OT triangle carries its own GP0(E1h) word in the same node, the way the
//! SDK's `QuadGouraudBlended` does for quads.

use psx_gpu::material::{BlendMode, TextureMaterial};
use psx_gpu::prim::GpuPacket;

use crate::asset::{GroupId, VectorAsset};
use crate::draw::{DrawError, Emit, Placement, Scratch, Style};

/// A Gouraud triangle with its draw mode: 7 payload words after the OT tag.
///
/// It also draws immediately: `Gpu::draw(&packet)` sends the payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C, align(4))]
pub struct VectorPacket {
    /// OT linkage; written by the link operation.
    pub tag: u32,
    /// GP0(E1h) draw mode selecting the blend equation.
    pub draw_mode: u32,
    /// Vertex 0 colour with the polygon opcode.
    pub color0_cmd: u32,
    /// Vertex 0 position.
    pub v0: u32,
    /// Vertex 1 colour.
    pub color1: u32,
    /// Vertex 1 position.
    pub v1: u32,
    /// Vertex 2 colour.
    pub color2: u32,
    /// Vertex 2 position.
    pub v2: u32,
}

// SAFETY: `repr(C, align(4))`, a `u32` tag first, then 7 `u32` payload words.
unsafe impl GpuPacket for VectorPacket {
    const WORDS: u8 = 7;
}

impl VectorPacket {
    /// A zeroed packet, for sizing static pools.
    pub const ZERO: Self = Self {
        tag: 0,
        draw_mode: 0,
        color0_cmd: 0,
        v0: 0,
        color1: 0,
        v1: 0,
        color2: 0,
        v2: 0,
    };
}

/// Why [`VectorAsset::emit_packets`] did not fill the output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PacketError {
    /// The draw itself failed, see [`DrawError`].
    Draw(DrawError),
    /// `out` was too small; `needed` packets would have been written.
    OutputTooSmall {
        /// Packets the draw produces.
        needed: usize,
    },
}

impl<'a> VectorAsset<'a> {
    /// Write `group`'s triangles as [`VectorPacket`]s into `out`, in paint
    /// order, and return how many.
    ///
    /// An ordering table walks one slot last-in-first-out, so to keep paint
    /// order within a slot link them back to front:
    /// `out[..n].iter_mut().rev()`. A clip rectangle can split a triangle into
    /// up to five, so size `out` for five times [`GroupInfo::triangles`](crate::GroupInfo)
    /// when clipping, or for exactly that count when not.
    pub fn emit_packets<const N: usize>(
        &self,
        group: GroupId,
        place: &Placement,
        style: &Style,
        scratch: &mut Scratch<N>,
        out: &mut [VectorPacket],
    ) -> Result<usize, PacketError> {
        let mut mode_word = 0u32;
        let mut written = 0usize;
        self.emit_group(group, place, style, scratch, &mut |step| match step {
            Emit::Blend(mode) => mode_word = draw_mode_word(mode),
            Emit::Triangle(t) => {
                if let Some(slot) = out.get_mut(written) {
                    *slot = VectorPacket {
                        tag: 0,
                        draw_mode: mode_word,
                        color0_cmd: t.color0_cmd,
                        v0: t.v0,
                        color1: t.color1,
                        v1: t.v1,
                        color2: t.color2,
                        v2: t.v2,
                    };
                }
                written += 1;
            }
        })
        .map_err(PacketError::Draw)?;
        if written > out.len() {
            Err(PacketError::OutputTooSmall { needed: written })
        } else {
            Ok(written)
        }
    }
}

/// The GP0(E1h) word for `mode`, with dithering off.
pub fn draw_mode_word(mode: BlendMode) -> u32 {
    TextureMaterial::blended(0, 0, (0, 0, 0), mode).draw_mode_word()
}
