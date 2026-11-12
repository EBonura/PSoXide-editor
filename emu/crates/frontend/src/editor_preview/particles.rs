//! Particle-emitter preview submission for editor scenes.

use super::*;

/// What one emitter's particles are drawn with: its own sprite (a Material's
/// image, resolved by the texture cache as the cook resolves it) or the
/// built-in disc.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PreviewParticleSprite {
    pub(crate) slot: MaterialSlot,
    pub(crate) uv_min: (u8, u8),
    pub(crate) uv_max: (u8, u8),
    /// Whether the emitter's own sprite is in use (false: the disc); tests
    /// read it.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) textured: bool,
}

/// The sprite for an emitter whose `texture` is given. The runtime samples
/// the whole sprite, UV 0 to size-1 inside its texture window; the disc keeps
/// the shared page's fixed rectangle. A sprite the cache cannot place (no
/// image, a missing resource) falls back to the disc, as the cook does.
pub(crate) fn particle_sprite(
    textures: &crate::editor_textures::EditorTextures,
    texture: Option<psxed_project::ResourceId>,
) -> PreviewParticleSprite {
    if let Some(slot) = texture.and_then(|id| textures.slot(id)) {
        return PreviewParticleSprite {
            slot,
            uv_min: (0, 0),
            uv_max: (
                slot.texture_width.saturating_sub(1),
                slot.texture_height.saturating_sub(1),
            ),
            textured: true,
        };
    }
    let slot = textures.particle_slot();
    PreviewParticleSprite {
        slot,
        uv_min: (PREVIEW_PARTICLE_TEXEL_U, PREVIEW_PARTICLE_TEXEL_V),
        uv_max: (
            PREVIEW_PARTICLE_TEXEL_U + slot.texture_width.saturating_sub(1),
            PREVIEW_PARTICLE_TEXEL_V + slot.texture_height.saturating_sub(1),
        ),
        textured: false,
    }
}

/// The blend the runtime draws an emitter with: its cooked two-bit code, where
/// Opaque and Average share code 0.
fn particle_blend(mode: psxed_project::PsxBlendMode) -> BlendMode {
    match psx_blend_mode(mode) {
        BlendMode::Opaque => BlendMode::Average,
        other => other,
    }
}

pub(crate) fn walk_particle_emitters(
    project: &ProjectDocument,
    hidden_scene_nodes: &HashSet<NodeId>,
    textures: &crate::editor_textures::EditorTextures,
    preview_tick: u32,
    scratch: &mut PreviewScratch,
) {
    let scene = project.active_scene();
    for node in scene.nodes() {
        if scene_node_hidden(scene, hidden_scene_nodes, node.id) {
            continue;
        }
        let NodeKind::ParticleEmitter { settings } = &node.kind else {
            continue;
        };
        let [x, y, z] = node.transform.translation;
        push_particle_emitter_preview(
            settings,
            psx_engine::WorldVertex::new(x.round() as i32, y.round() as i32, z.round() as i32),
            particle_sprite(textures, settings.texture),
            preview_tick,
            scratch,
        );
    }
}

pub(crate) fn push_particle_emitter_preview(
    settings: &psxed_project::ParticleEmitterSettings,
    origin: psx_engine::WorldVertex,
    sprite: PreviewParticleSprite,
    preview_tick: u32,
    scratch: &mut PreviewScratch,
) {
    if !settings.enabled
        || settings.max_particles == 0
        || settings.lifetime_frames == 0
        || settings.spawn_rate_q8 == 0
    {
        return;
    }
    scratch.clock_used = true;
    let lifetime = settings.lifetime_frames as u32;
    let steady_count = ((settings.spawn_rate_q8 as u32)
        .saturating_mul(lifetime)
        .saturating_add(60 * 256 - 1))
        / (60 * 256);
    let count = (settings.max_particles as u32)
        .min(PREVIEW_PARTICLE_DRAW_CAP as u32)
        .min(steady_count.max(1));
    for i in 0..count {
        let seed = preview_particle_seed(origin.x as u32, origin.z as u32, i);
        let age = (preview_tick + (i * lifetime / count)) % lifetime;
        push_particle_preview_sample(
            settings,
            origin,
            sprite,
            seed,
            age as i32,
            lifetime as i32,
            scratch,
        );
    }
}

fn push_particle_preview_sample(
    settings: &psxed_project::ParticleEmitterSettings,
    origin: psx_engine::WorldVertex,
    sprite: PreviewParticleSprite,
    seed: u32,
    age: i32,
    lifetime: i32,
    scratch: &mut PreviewScratch,
) {
    let spawn_radius = settings.spawn_radius as i32;
    let origin_x = origin
        .x
        .saturating_add(preview_particle_signed_spread(seed, spawn_radius));
    let origin_y = origin.y.saturating_add(preview_particle_signed_spread(
        seed.rotate_left(9),
        spawn_radius / 2,
    ));
    let origin_z = origin.z.saturating_add(preview_particle_signed_spread(
        seed.rotate_left(17),
        spawn_radius,
    ));
    let x = preview_particle_axis_position(
        origin_x,
        settings.base_velocity_q4[0],
        settings.random_velocity_q4[0],
        settings.acceleration_q4[0],
        age,
        seed.rotate_left(3),
    );
    let y = preview_particle_axis_position(
        origin_y,
        settings.base_velocity_q4[1],
        settings.random_velocity_q4[1],
        settings.acceleration_q4[1],
        age,
        seed.rotate_left(11),
    );
    let z = preview_particle_axis_position(
        origin_z,
        settings.base_velocity_q4[2],
        settings.random_velocity_q4[2],
        settings.acceleration_q4[2],
        age,
        seed.rotate_left(21),
    );
    let projected = gte_scene::project_vertex(world_to_view([x, y, z]));
    if projected.sz == 0 {
        return;
    }
    let t_q8 = if lifetime <= 1 {
        255
    } else {
        ((age * 255) / (lifetime - 1)).clamp(0, 255)
    };
    let size = preview_particle_lerp_u16(settings.start_size, settings.end_size, t_q8);
    let half = ((i32::from(size) * PROJ_H) / i32::from(projected.sz.max(1))).clamp(
        i32::from(PREVIEW_PARTICLE_MIN_SCREEN_SIZE),
        i32::from(PREVIEW_PARTICLE_MAX_SCREEN_SIZE),
    ) as i16;
    let tint = preview_particle_lerp_rgb(settings.start_color, settings.end_color, t_q8);
    push_particle_preview_quad(
        projected,
        half,
        sprite,
        tint,
        particle_blend(settings.blend_mode),
        scratch,
    );
}

fn push_particle_preview_quad(
    center: psx_gte::scene::Projected,
    half: i16,
    sprite: PreviewParticleSprite,
    tint: (u8, u8, u8),
    blend_mode: BlendMode,
    scratch: &mut PreviewScratch,
) {
    if scratch.particle_used >= PREVIEW_PARTICLE_DRAW_CAP {
        return;
    }
    let left = (i32::from(center.sx).saturating_sub(i32::from(half)))
        .clamp(i16::MIN as i32, i16::MAX as i32) as i16;
    let right = (i32::from(center.sx).saturating_add(i32::from(half)))
        .clamp(i16::MIN as i32, i16::MAX as i32) as i16;
    let top = (i32::from(center.sy).saturating_sub(i32::from(half)))
        .clamp(i16::MIN as i32, i16::MAX as i32) as i16;
    let bottom = (i32::from(center.sy).saturating_add(i32::from(half)))
        .clamp(i16::MIN as i32, i16::MAX as i32) as i16;
    if left == right || top == bottom {
        return;
    }
    let (u0, v0) = sprite.uv_min;
    let (u1, v1) = sprite.uv_max;
    let material = preview_texture_material(sprite.slot, tint, blend_mode);
    let idx = scratch.particle_used;
    scratch.particle_quads[idx] = QuadTexturedMaterial::with_material(
        [(left, top), (right, top), (left, bottom), (right, bottom)],
        [(u0, v0), (u1, v0), (u0, v1), (u1, v1)],
        material,
    );
    scratch.particle_used = idx + 1;
    let packet_ptr: *mut QuadTexturedMaterial = &mut scratch.particle_quads[idx];
    unsafe {
        scratch.ot.resume_frame().add_raw(
            preview_depth_slot(
                scratch.depth_range,
                u32::from(center.sz) << scratch.gte_depth_shift,
            ),
            packet_ptr.cast::<u32>(),
            QuadTexturedMaterial::WORDS,
        );
    }
}

fn preview_particle_axis_position(
    origin: i32,
    base_velocity_q4: i16,
    random_velocity_q4: u16,
    acceleration_q4: i16,
    age: i32,
    seed: u32,
) -> i32 {
    let random_velocity = preview_particle_signed_spread(seed, random_velocity_q4 as i32);
    let velocity = i32::from(base_velocity_q4).saturating_add(random_velocity);
    let velocity_term = velocity.saturating_mul(age) >> 4;
    let acceleration_term = i32::from(acceleration_q4)
        .saturating_mul(age)
        .saturating_mul(age)
        >> 5;
    origin
        .saturating_add(velocity_term)
        .saturating_add(acceleration_term)
}

fn preview_particle_seed(x: u32, z: u32, index: u32) -> u32 {
    let mut value = x
        .rotate_left(7)
        .wrapping_add(z.rotate_left(17))
        .wrapping_add(index.wrapping_mul(0x85EB_CA6B));
    value ^= value >> 16;
    value = value.wrapping_mul(0x7FEB_352D);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846C_A68B);
    value ^ (value >> 16)
}

fn preview_particle_signed_spread(seed: u32, spread: i32) -> i32 {
    if spread <= 0 {
        return 0;
    }
    let span = spread.saturating_mul(2).saturating_add(1) as u32;
    (seed % span) as i32 - spread
}

fn preview_particle_lerp_u16(a: u16, b: u16, t_q8: i32) -> u16 {
    let inv = 255 - t_q8;
    (((i32::from(a) * inv) + (i32::from(b) * t_q8)) / 255).clamp(0, u16::MAX as i32) as u16
}

fn preview_particle_lerp_rgb(a: [u8; 3], b: [u8; 3], t_q8: i32) -> (u8, u8, u8) {
    (
        preview_particle_lerp_u8(a[0], b[0], t_q8),
        preview_particle_lerp_u8(a[1], b[1], t_q8),
        preview_particle_lerp_u8(a[2], b[2], t_q8),
    )
}

fn preview_particle_lerp_u8(a: u8, b: u8, t_q8: i32) -> u8 {
    let inv = 255 - t_q8;
    (((i32::from(a) * inv) + (i32::from(b) * t_q8)) / 255).clamp(0, 255) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor_textures::EditorTextures;
    use psxed_project::{MaterialResource, MaterialTextureMode, ProjectDocument, ResourceData};
    use std::path::Path;

    fn project_with_sprite() -> (ProjectDocument, psxed_project::ResourceId) {
        let mut project = ProjectDocument::new("particle sprite");
        let mut material = MaterialResource::opaque(None);
        material.texture_mode = MaterialTextureMode::Generated;
        let id = project.add_resource("FX / Puff", ResourceData::Material(material));
        let scene = project.active_scene_mut();
        scene.add_node(
            scene.root,
            "Smoke",
            NodeKind::ParticleEmitter {
                settings: psxed_project::ParticleEmitterSettings {
                    texture: Some(id),
                    ..Default::default()
                },
            },
        );
        (project, id)
    }

    #[test]
    fn a_textured_emitter_draws_its_sprite_and_an_untextured_one_the_disc() {
        let (project, id) = project_with_sprite();
        let mut textures = EditorTextures::new();
        textures.refresh(&project, Path::new("."));
        let slot = textures.slot(id).expect("the sprite is in the cache");

        let sprite = particle_sprite(&textures, Some(id));
        assert!(sprite.textured);
        assert_eq!(sprite.slot.clut_word, slot.clut_word);
        assert_eq!(sprite.slot.tpage_word, slot.tpage_word);
        assert_eq!(sprite.uv_min, (0, 0));
        assert_eq!(
            sprite.uv_max,
            (slot.texture_width - 1, slot.texture_height - 1)
        );

        let disc = particle_sprite(&textures, None);
        assert!(!disc.textured);
        assert_eq!(disc.slot.clut_word, textures.particle_slot().clut_word);
        assert_eq!(
            disc.uv_min,
            (PREVIEW_PARTICLE_TEXEL_U, PREVIEW_PARTICLE_TEXEL_V)
        );
        assert_ne!(disc.slot.clut_word, sprite.slot.clut_word);

        // A sprite the cache does not hold (missing resource) is the disc too.
        let mut other = ProjectDocument::new("elsewhere");
        let missing = (0..8)
            .map(|n| {
                other.add_resource(
                    format!("R{n}"),
                    ResourceData::Material(MaterialResource::opaque(None)),
                )
            })
            .last()
            .unwrap();
        assert!(!particle_sprite(&textures, Some(missing)).textured);
    }

    #[test]
    fn emitter_blend_follows_the_cooked_code() {
        use psxed_project::PsxBlendMode as P;
        assert!(matches!(particle_blend(P::Add), BlendMode::Add));
        assert!(matches!(particle_blend(P::Opaque), BlendMode::Average));
        assert!(matches!(particle_blend(P::Average), BlendMode::Average));
    }
}
