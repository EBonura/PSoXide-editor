//! A small CPU rasteriser for egui's tessellated output, so a screenshot of
//! the editor needs no GPU and comes out the same on every machine. It draws
//! the triangles the way egui's own renderer does (premultiplied vertex
//! colour times texture, blended in gamma space, clipped to each primitive's
//! rectangle); it has no anti-aliasing of its own because egui feathers its
//! edges in the mesh.

use egui::epaint::{ImageData, Primitive, Vertex};
use egui::{Color32, TextureFilter, TextureId, TexturesDelta};
use std::collections::HashMap;

struct Texture {
    size: [usize; 2],
    pixels: Vec<Color32>,
    linear: bool,
}

/// Every texture egui has uploaded so far (apply each frame's delta).
#[derive(Default)]
pub(crate) struct Textures {
    map: HashMap<TextureId, Texture>,
}

impl Textures {
    pub(crate) fn apply(&mut self, delta: &TexturesDelta) {
        for (id, set) in &delta.set {
            let (size, pixels): ([usize; 2], Vec<Color32>) = match &set.image {
                ImageData::Color(image) => (image.size, image.pixels.clone()),
                ImageData::Font(font) => (font.size, font.srgba_pixels(None).collect()),
            };
            let linear = set.options.magnification == TextureFilter::Linear;
            match set.pos {
                None => {
                    self.map.insert(
                        *id,
                        Texture {
                            size,
                            pixels,
                            linear,
                        },
                    );
                }
                Some([x0, y0]) => {
                    let Some(texture) = self.map.get_mut(id) else {
                        continue;
                    };
                    for row in 0..size[1] {
                        let dst = (y0 + row) * texture.size[0] + x0;
                        let src = row * size[0];
                        texture.pixels[dst..dst + size[0]]
                            .copy_from_slice(&pixels[src..src + size[0]]);
                    }
                }
            }
        }
    }
}

/// A rendered frame: RGBA8, row by row from the top.
pub(crate) struct Frame {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) rgba: Vec<u8>,
}

impl Frame {
    fn new(width: usize, height: usize) -> Self {
        let mut rgba = vec![0u8; width * height * 4];
        for px in rgba.chunks_exact_mut(4) {
            px[3] = 255;
        }
        Self {
            width,
            height,
            rgba,
        }
    }
}

fn sample(texture: &Texture, u: f32, v: f32) -> [f32; 4] {
    let [w, h] = texture.size;
    let at = |x: i64, y: i64| {
        let x = x.clamp(0, w as i64 - 1) as usize;
        let y = y.clamp(0, h as i64 - 1) as usize;
        let c = texture.pixels[y * w + x];
        [
            f32::from(c.r()),
            f32::from(c.g()),
            f32::from(c.b()),
            f32::from(c.a()),
        ]
    };
    if !texture.linear {
        return at((u * w as f32).floor() as i64, (v * h as f32).floor() as i64);
    }
    let (fx, fy) = (u * w as f32 - 0.5, v * h as f32 - 0.5);
    let (x0, y0) = (fx.floor(), fy.floor());
    let (tx, ty) = (fx - x0, fy - y0);
    let (x0, y0) = (x0 as i64, y0 as i64);
    let (a, b, c, d) = (
        at(x0, y0),
        at(x0 + 1, y0),
        at(x0, y0 + 1),
        at(x0 + 1, y0 + 1),
    );
    let mut out = [0.0; 4];
    for i in 0..4 {
        let top = a[i] + (b[i] - a[i]) * tx;
        let bottom = c[i] + (d[i] - c[i]) * tx;
        out[i] = top + (bottom - top) * ty;
    }
    out
}

fn edge(a: egui::Pos2, b: egui::Pos2, p: egui::Pos2) -> f32 {
    (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)
}

fn triangle(frame: &mut Frame, clip: [usize; 4], texture: &Texture, v: [&Vertex; 3]) {
    let (a, b, c) = (v[0].pos, v[1].pos, v[2].pos);
    let area = edge(a, b, c);
    if area == 0.0 || !area.is_finite() {
        return;
    }
    let min_x = (a.x.min(b.x).min(c.x).floor().max(clip[0] as f32)) as usize;
    let min_y = (a.y.min(b.y).min(c.y).floor().max(clip[1] as f32)) as usize;
    let max_x = (a.x.max(b.x).max(c.x).ceil().min(clip[2] as f32)).max(0.0) as usize;
    let max_y = (a.y.max(b.y).max(c.y).ceil().min(clip[3] as f32)).max(0.0) as usize;
    let col = |v: &Vertex| {
        [
            f32::from(v.color.r()),
            f32::from(v.color.g()),
            f32::from(v.color.b()),
            f32::from(v.color.a()),
        ]
    };
    let (ca, cb, cc) = (col(v[0]), col(v[1]), col(v[2]));
    let inv = 1.0 / area;
    for y in min_y..max_y {
        for x in min_x..max_x {
            let p = egui::pos2(x as f32 + 0.5, y as f32 + 0.5);
            let w0 = edge(b, c, p) * inv;
            let w1 = edge(c, a, p) * inv;
            let w2 = edge(a, b, p) * inv;
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }
            let u = v[0].uv.x * w0 + v[1].uv.x * w1 + v[2].uv.x * w2;
            let vv = v[0].uv.y * w0 + v[1].uv.y * w1 + v[2].uv.y * w2;
            let t = sample(texture, u, vv);
            let mut src = [0.0f32; 4];
            for i in 0..4 {
                let vertex = ca[i] * w0 + cb[i] * w1 + cc[i] * w2;
                src[i] = vertex * t[i] / 255.0;
            }
            let k = 1.0 - src[3] / 255.0;
            let o = (y * frame.width + x) * 4;
            for (channel, value) in src.iter().take(3).enumerate() {
                let dst = f32::from(frame.rgba[o + channel]);
                frame.rgba[o + channel] = (value + dst * k).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
}

/// Draw the primitives of one frame onto a black `width` x `height` image.
pub(crate) fn render(
    primitives: &[egui::ClippedPrimitive],
    textures: &Textures,
    width: usize,
    height: usize,
) -> Frame {
    let mut frame = Frame::new(width, height);
    for primitive in primitives {
        let Primitive::Mesh(mesh) = &primitive.primitive else {
            continue;
        };
        let Some(texture) = textures.map.get(&mesh.texture_id) else {
            continue;
        };
        let r = primitive.clip_rect;
        let clip = [
            r.min.x.round().clamp(0.0, width as f32) as usize,
            r.min.y.round().clamp(0.0, height as f32) as usize,
            r.max.x.round().clamp(0.0, width as f32) as usize,
            r.max.y.round().clamp(0.0, height as f32) as usize,
        ];
        for tri in mesh.indices.chunks_exact(3) {
            let v = [
                &mesh.vertices[tri[0] as usize],
                &mesh.vertices[tri[1] as usize],
                &mesh.vertices[tri[2] as usize],
            ];
            triangle(&mut frame, clip, texture, v);
        }
    }
    frame
}
