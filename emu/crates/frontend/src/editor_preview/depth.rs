//! Frame-local depth range for the editor's unrestricted overview camera.
use super::*;

const DEFAULT_DEPTH_RANGE: psx_engine::DepthRange = psx_engine::DepthRange::new(
    (PREVIEW_GEOMETRY_SLOT_MIN as i32) << 2,
    (PREVIEW_GEOMETRY_SLOT_MAX as i32) << 2,
);

/// Expand only as needed, retaining the original four-unit buckets in close
/// scenes. Both brushes and the engine model pass must use this same range.
fn range_for_farthest_depth(farthest: i32) -> psx_engine::DepthRange {
    psx_engine::DepthRange::new(
        DEFAULT_DEPTH_RANGE.near(),
        farthest.max(DEFAULT_DEPTH_RANGE.far()),
    )
}

pub(super) fn preview_scene_depth_range(
    project: &ProjectDocument,
    camera: psx_engine::WorldCamera,
    hidden: &HashSet<NodeId>,
    entities: &[psxed_ui::EntityBounds],
) -> psx_engine::DepthRange {
    let mut farthest = DEFAULT_DEPTH_RANGE.far();
    let mut include_bounds = |min: [f64; 3], max: [f64; 3]| {
        for corner in 0..8 {
            let p = core::array::from_fn::<_, 3, _>(|axis| {
                if corner & (1 << axis) == 0 {
                    min[axis]
                } else {
                    max[axis]
                }
            });
            let depth = camera
                .view_vertex(psx_engine::WorldVertex::new(
                    p[0].round() as i32,
                    p[1].round() as i32,
                    p[2].round() as i32,
                ))
                .z;
            farthest = farthest.max(depth.saturating_add(1024));
        }
    };
    with_cached_csg_surfaces(project, hidden, |surfaces| {
        for surface in surfaces {
            include_bounds(surface.bounds.min, surface.bounds.max);
        }
    });
    for entity in entities {
        if !hidden.contains(&entity.node) {
            include_bounds(
                core::array::from_fn(|i| f64::from(entity.center[i] - entity.half_extents[i])),
                core::array::from_fn(|i| f64::from(entity.center[i] + entity.half_extents[i])),
            );
        }
    }
    range_for_farthest_depth(farthest)
}

pub(super) fn preview_depth_slot(range: psx_engine::DepthRange, depth: u32) -> usize {
    // Host-side wide arithmetic also handles extremely distant editor cameras.
    let near = range.near() as u64;
    let span = (range.far() - range.near()).max(1) as u64;
    let offset = u64::from(depth).saturating_sub(near).min(span);
    PREVIEW_GEOMETRY_SLOT_MIN
        + (offset * (PREVIEW_GEOMETRY_SLOT_MAX - PREVIEW_GEOMETRY_SLOT_MIN) as u64 / span) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overview_depths_remain_distinct_beyond_old_ot_and_u16_limits() {
        let range = range_for_farthest_depth(120_000);
        let depths = [20_000, 40_000, 65_535, 80_000, 100_000];
        let slots = depths.map(|depth| {
            let vertices = [PreviewClipVertex::new(
                psx_engine::ViewVertex::new(0, 0, depth),
                [0.0; 2],
                (128, 128, 128),
            ); 3];
            clipped_surface_depth_slot(&vertices, range)
        });
        assert!(slots.windows(2).all(|pair| pair[0] < pair[1]), "{slots:?}");
        assert!(slots[4] < PREVIEW_GEOMETRY_SLOT_MAX);
    }

    #[test]
    fn close_view_preserves_four_unit_depth_buckets_and_reserved_layers() {
        let range = range_for_farthest_depth(8192);
        for depth in [0, 4, 8, 128, 4096, 8192, 16368] {
            assert_eq!(
                preview_depth_slot(range, depth),
                ((depth as usize) >> 2).clamp(1, PREVIEW_GEOMETRY_SLOT_MAX)
            );
        }
        assert!(preview_depth_slot(range, u32::MAX) < PREVIEW_FAR_VISTA_SLOT);
    }

    #[test]
    fn brushes_and_models_share_overview_depth_mapping() {
        let range = range_for_farthest_depth(120_000);
        let options = preview_model_surface_options(
            TextureMaterial::opaque(0, 0, (128, 128, 128)),
            psxed_project::MaterialFaceSidedness::Front,
            64,
            range,
        );
        assert_eq!(options.depth_range, range);
        let band = options.depth_band;
        for depth in [100, 20_000, 40_000, 80_000, 119_000] {
            assert_eq!(
                preview_depth_slot(range, depth as u32),
                band.slot::<OT_DEPTH>(range, depth).index()
            );
        }
    }
    #[test]
    fn distant_overlapping_faces_draw_back_to_front_in_either_submission_order() {
        for depths in [[40_000, 80_000], [80_000, 40_000]] {
            let mut scratch = new_preview_scratch();
            scratch.ot.clear();
            scratch.depth_range = range_for_farthest_depth(100_000);
            for depth in depths {
                let color = if depth == 40_000 {
                    (255, 0, 0)
                } else {
                    (0, 255, 0)
                };
                let vertices = [PreviewClipVertex::new(
                    psx_engine::ViewVertex::new(0, 0, depth),
                    [0.0; 2],
                    color,
                ); 3];
                let slot = clipped_surface_depth_slot(&vertices, scratch.depth_range);
                let projected = [(100, 100), (200, 100), (150, 200)].map(|(sx, sy)| {
                    psx_gte::scene::Projected {
                        sx,
                        sy,
                        sz: depth.min(65535) as u16,
                    }
                });
                assert!(push_tri_colors_at_slot(
                    &mut scratch,
                    projected,
                    [color; 3],
                    slot
                ));
            }
            let mut commands = Vec::new();
            // The aligned scratch owns every live packet while the OT is read.
            unsafe { psx_gpu_render::build_cmd_log_into(&scratch.ot, &mut commands) };
            let colors: Vec<_> = commands
                .iter()
                .filter(|c| c.opcode == 0x30)
                .map(|c| c.fifo[0] & 0x00ff_ffff)
                .collect();
            assert_eq!(
                colors,
                [0x00ff00, 0x0000ff],
                "the nearer red face must cover green"
            );
        }
    }
}
