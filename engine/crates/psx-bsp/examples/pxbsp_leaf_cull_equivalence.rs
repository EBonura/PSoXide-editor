//! Render one cooked world from many cameras with and without its v7 leaf
//! bounds and require identical packets and statistics.
//!
//! ```sh
//! cargo run --release -p psx-bsp --example pxbsp_leaf_cull_equivalence -- LEGACY_MAP BOUNDED_MAP [POSES]
//! ```
//!
//! Both files must be cooks of the same world, one as PXBSP v6 (no leaf
//! bounds, so the renderer takes its previous path) and one as v7.

use psx_bsp::collision::{Trace, TraceScratch};
use psx_bsp::pxbsp_resident::PxbspResidentMap;
use psx_bsp::render::{
    configure_projection, load_pxbsp_view, Camera, PxbspTextureBinding, Renderer,
    DEFAULT_PACKET_WORDS,
};
use psx_bsp::Vec3I32;

fn load(path: &str) -> PxbspResidentMap {
    let bytes = std::fs::read(path).unwrap_or_else(|error| panic!("read {path}: {error}"));
    let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
    // `from_static` wants a four-byte-aligned base; `Box<[u8]>` of this size
    // is allocator-aligned far beyond that on the host.
    PxbspResidentMap::from_static(1, bytes).unwrap_or_else(|error| panic!("load {path}: {error:?}"))
}

fn main() {
    let mut args = std::env::args().skip(1);
    let legacy_path = args.next().expect("LEGACY_MAP path");
    let bounded_path = args.next().expect("BOUNDED_MAP path");
    let poses: usize = args
        .next()
        .map_or(2000, |value| value.parse().expect("POSES"));
    let legacy = load(&legacy_path);
    let bounded = load(&bounded_path);
    assert!(legacy.leaf_bounds().is_none(), "first map must be pre-v7");
    let bounds = bounded.leaf_bounds().expect("second map must be v7");
    println!(
        "faces {} leaves {} leaf-bounds {} ({} full-range)",
        bounded.faces().len(),
        bounded.leaves().len(),
        bounds.len(),
        bounds
            .iter()
            .filter(|b| **b == psx_bsp::LeafBounds::FULL)
            .count()
    );

    let world = bounded.brush_models().get(0).expect("world model");
    let (mins, maxs) = (world.mins, world.maxs);
    let hull = bounded
        .model_collision_hull(0, 0)
        .expect("world point hull");

    configure_projection();
    let bindings = vec![Some(PxbspTextureBinding::default()); legacy.materials().len()];
    let mut renderer_a = Renderer::new_pxbsp_with_nodes(legacy.faces().len(), legacy.nodes().len());
    let mut renderer_b =
        Renderer::new_pxbsp_with_nodes(bounded.faces().len(), bounded.nodes().len());
    let mut packets_a = vec![0u32; DEFAULT_PACKET_WORDS];
    let mut packets_b = vec![0u32; DEFAULT_PACKET_WORDS];

    let mut state = 0x9e37_79b9u32;
    let mut next = |range: i32| -> i32 {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        ((state >> 8) as i32).rem_euclid(range.max(1))
    };
    let (mut compared, mut skipped, mut mismatches) = (0usize, 0usize, 0usize);
    let (mut faces_total, mut words_total) = (0u64, 0u64);
    for _ in 0..poses {
        let mut span = |lo: i16, hi: i16| i32::from(lo) + next(i32::from(hi) - i32::from(lo) + 1);
        let origin = Vec3I32 {
            x: span(mins.x, maxs.x) << 12,
            y: span(mins.y, maxs.y) << 12,
            z: span(mins.z, maxs.z) << 12,
        };
        // Only poses someone could stand in: an empty leaf with floor below.
        if hull.point_contents(origin) != Some(-1) || bounded.point_leaf_index(origin).is_none() {
            skipped += 1;
            continue;
        }
        let mut floor = Trace::default();
        let floor_end = Vec3I32 {
            y: origin.y.saturating_sub(256 << 12),
            ..origin
        };
        assert!(hull.trace_into(&origin, &floor_end, &mut TraceScratch::new(), &mut floor));
        let camera = Camera {
            origin,
            angles: [(next(801) - 400) as i16, next(4096) as i16, 0],
        };
        let view = load_pxbsp_view(camera);
        let a = renderer_a.draw_pxbsp_world(&legacy, camera, view, &bindings, 0, &mut packets_a);
        let b = renderer_b.draw_pxbsp_world(&bounded, camera, view, &bindings, 0, &mut packets_b);
        compared += 1;
        faces_total += u64::from(b.stats.visible_faces);
        words_total += b.packet_words as u64;
        let same = a.packet_words == b.packet_words
            && packets_a[..a.packet_words] == packets_b[..b.packet_words]
            && a.stats.visible_faces == b.stats.visible_faces
            && a.stats.packets == b.stats.packets
            && a.stats.hardware_triangles == b.stats.hardware_triangles
            && a.stats.surface_batches == b.stats.surface_batches
            && a.stats.visible_sky_apertures == b.stats.visible_sky_apertures
            && a.stats.unresolved_material_faces == b.stats.unresolved_material_faces;
        if !same {
            mismatches += 1;
            println!(
                "MISMATCH at {:?} angles {:?}: words {} vs {}, faces {} vs {}",
                camera.origin,
                camera.angles,
                a.packet_words,
                b.packet_words,
                a.stats.visible_faces,
                b.stats.visible_faces
            );
        }
    }
    println!(
        "poses compared {compared} (skipped {skipped}), mismatches {mismatches}, \
         mean faces {} mean packet words {}",
        faces_total / compared.max(1) as u64,
        words_total / compared.max(1) as u64
    );
    assert!(compared > 0 && mismatches == 0);
}
