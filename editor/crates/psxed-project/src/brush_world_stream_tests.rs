//! Gates for the streamed cook (M6): the streamed map must show what the
//! same geometry shows as one whole map, installing in any order must never
//! leave a dangling reference, and unresident space must read solid.
//!
//! The *flat reference* below is built independently of the runtime install
//! code: it concatenates the cooked region tables with plain additive bases
//! and the dense, slot-free PVS rows the cooker computed before it rewrote
//! them into the rank layout. Equality against it therefore exercises the
//! rank rewrite, the slot relocation, the link patches and the renderer's
//! streamed PVS decode together.

use std::path::PathBuf;

use psx_bsp::collision::{CONTENTS_EMPTY, CONTENTS_SOLID};
use psx_bsp::pxbsp::{PxbspIndex, PxbspLumpKind, PXBSP_LUMP_COUNT};
use psx_bsp::pxbsp_resident::stream::{RegionLoader, StreamError};
use psx_bsp::pxbsp_resident::PxbspResidentMap;
use psx_bsp::render::{configure_projection, load_pxbsp_view, Camera, Renderer};
use psx_bsp::{SliceReader, Vec3I32};

use super::*;
use crate::brush_region::PartitionParams;
use crate::stream_world::{generate, load_donor, StreamWorldConfig};

fn projects_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../projects")
}

fn cook(project: &ProjectDocument, root: &Path, params: &PartitionParams) -> StreamedBrushWorld {
    match cook_project_streamed(project, root, BrushWorldCookMode::Draft, [0; 3], params)
        .expect("cook")
    {
        CookedWorld::Streamed(world) => *world,
        CookedWorld::Whole(_) => panic!("expected more than one region"),
    }
}

fn graybox() -> (ProjectDocument, PathBuf) {
    let root = projects_dir().join("graybox-reach");
    let project = ProjectDocument::load_from_path(root.join("project.ron")).expect("graybox-reach");
    (project, root)
}

fn lcg(state: &mut u32) -> u32 {
    *state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    *state >> 8
}

// ---- the flat reference ----------------------------------------------------

fn rd16(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn wr16(bytes: &mut [u8], at: usize, value: u16) {
    bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

struct Flat {
    bytes: Vec<u8>,
    /// Region and local face of every flat face.
    face_of: Vec<(usize, usize)>,
    /// Region and local leaf (1-based) of every flat leaf (index 0 unused).
    leaf_of: Vec<(usize, usize)>,
}

fn lump(container: &[u8], index: &PxbspIndex, kind: PxbspLumpKind) -> Vec<u8> {
    let r = index.lump(kind);
    container[r.offset as usize..r.end() as usize].to_vec()
}

fn flatten(world: &StreamedBrushWorld) -> Flat {
    let debug = &world.debug;
    let regions = debug.regions.len();
    let top = &debug.top;
    let index = PxbspIndex::read(&mut SliceReader::new(&world.container)).unwrap();
    let mut lumps: [Vec<u8>; PXBSP_LUMP_COUNT] = core::array::from_fn(|_| Vec::new());
    lumps[PxbspLumpKind::Materials as usize] =
        lump(&world.container, &index, PxbspLumpKind::Materials);
    lumps[PxbspLumpKind::Entities as usize] =
        lump(&world.container, &index, PxbspLumpKind::Entities);

    // Additive bases, region by region.
    let mut base = vec![[0usize; 8]; regions]; // planes, vertices, faces, marks, nodes, clip, leaf, vis
    let top_planes = top.planes.len() / 12;
    let top_nodes = top.nodes.len() / 16;
    let top_clip = top.clip_nodes.len() / 6;
    let mut at = [top_planes, 0, 0, 0, top_nodes, top_clip, 1, 0];
    for (r, b) in debug.regions.iter().enumerate() {
        base[r] = at;
        at[0] += b.planes.len() / 12;
        at[1] += b.vertices.len() / 12;
        at[2] += b.faces.len() / 10;
        at[3] += b.marks.len() / 2;
        at[4] += b.nodes.len() / 16;
        at[5] += b.clip_nodes.len() / 6;
        at[6] += b.leaves.len() / 14;
    }

    // Region child -> flat child.
    let render_child = |r: usize, c: i16| -> i16 {
        if c >= 0 {
            (c as usize + base[r][4]) as i16
        } else {
            let l = (-1 - c) as usize;
            if l == 0 {
                c
            } else {
                (-1 - (base[r][6] + l - 1) as i32) as i16
            }
        }
    };
    let clip_child = |r: usize, c: i16| -> i16 {
        if c >= 0 {
            (c as usize + base[r][5]) as i16
        } else {
            c
        }
    };

    let mut planes = top.planes.clone();
    let mut vertices = Vec::new();
    let mut faces = Vec::new();
    let mut marks = Vec::new();
    let mut nodes = top.nodes.clone();
    let mut clips = top.clip_nodes.clone();
    let mut leaves = Vec::new();
    let mut face_of = Vec::new();
    let mut leaf_of = vec![(0, 0)];
    // Sentinel leaf.
    {
        let mut sentinel = vec![0u8; 14];
        sentinel[0] = CONTENTS_SOLID as i8 as u8;
        sentinel[4..8].copy_from_slice(&(-1i32).to_le_bytes());
        sentinel[13] = 64;
        leaves.extend(sentinel);
    }
    // Dense visibility, interned.
    let mut vis = Vec::new();
    let mut interned = std::collections::BTreeMap::<Vec<u8>, i32>::new();
    for (r, b) in debug.regions.iter().enumerate() {
        planes.extend_from_slice(&b.planes);
        vertices.extend_from_slice(&b.vertices);
        for (local, face) in b.faces.chunks_exact(10).enumerate() {
            let mut f = face.to_vec();
            wr16(&mut f, 0, rd16(face, 0) + base[r][0] as u16);
            wr16(&mut f, 2, rd16(face, 2) + base[r][1] as u16);
            faces.extend(f);
            face_of.push((r, local));
        }
        for mark in b.marks.chunks_exact(2) {
            marks.extend_from_slice(&(rd16(mark, 0) + base[r][2] as u16).to_le_bytes());
        }
        for node in b.nodes.chunks_exact(16) {
            let mut n = node.to_vec();
            wr16(&mut n, 0, rd16(node, 0) + base[r][0] as u16);
            for side in 0..2 {
                wr16(
                    &mut n,
                    2 + side * 2,
                    render_child(r, rd16(node, 2 + side * 2) as i16) as u16,
                );
            }
            nodes.extend(n);
        }
        for node in b.clip_nodes.chunks_exact(6) {
            let mut n = node.to_vec();
            wr16(&mut n, 0, rd16(node, 0) + base[r][0] as u16);
            for side in 0..2 {
                wr16(
                    &mut n,
                    2 + side * 2,
                    clip_child(r, rd16(node, 2 + side * 2) as i16) as u16,
                );
            }
            clips.extend(n);
        }
        for (k, leaf) in b.leaves.chunks_exact(14).enumerate() {
            let mut l = leaf.to_vec();
            wr16(&mut l, 8, rd16(leaf, 8) + base[r][3] as u16);
            let row = &debug.dense_rows[r][k];
            let compressed = crate::brush_pack::compress_visibility(row);
            let offset = *interned.entry(compressed.clone()).or_insert_with(|| {
                let at = vis.len() as i32;
                vis.extend_from_slice(&compressed);
                at
            });
            l[4..8].copy_from_slice(&offset.to_le_bytes());
            leaves.extend(l);
            leaf_of.push((r, k + 1));
        }
    }
    // Top nodes: a stub child becomes the region's root.
    for t in 0..top_nodes {
        for side in 0..2 {
            let at = t * 16 + 2 + side * 2;
            let child = rd16(&nodes, at) as i16;
            if child < 0 {
                let r = (-child - 2) as usize;
                let root = render_child(r, debug.regions[r].render_root);
                wr16(&mut nodes, at, root as u16);
            }
        }
    }
    // Top clip nodes: patch each region's two links.
    for (r, entry) in world.index.regions.iter().enumerate() {
        for hull in 0..2 {
            let parent = entry.parents[1 + hull] as usize;
            let side = entry.side(1 + hull);
            let root = clip_child(r, debug.regions[r].clip_roots[hull]);
            wr16(&mut clips, parent * 6 + 2 + side * 2, root as u16);
        }
    }
    let n = top_nodes;
    let mut model = Vec::new();
    for v in debug
        .world_bounds
        .0
        .into_iter()
        .chain(debug.world_bounds.1)
        .chain([0; 3])
    {
        model.extend_from_slice(&v.to_le_bytes());
    }
    for head in [0i16, 0, 1, 1 + n as i16] {
        model.extend_from_slice(&head.to_le_bytes());
    }
    model.extend_from_slice(&(debug.dense_total as i16).to_le_bytes());
    model.extend_from_slice(&0u16.to_le_bytes());
    model.extend_from_slice(&((faces.len() / 10) as u16).to_le_bytes());
    lumps[PxbspLumpKind::Vertices as usize] = vertices;
    lumps[PxbspLumpKind::Planes as usize] = planes;
    lumps[PxbspLumpKind::Faces as usize] = faces;
    lumps[PxbspLumpKind::MarkSurfaces as usize] = marks;
    lumps[PxbspLumpKind::Visibility as usize] = vis;
    lumps[PxbspLumpKind::Leaves as usize] = leaves;
    lumps[PxbspLumpKind::Nodes as usize] = nodes;
    lumps[PxbspLumpKind::ClipNodes as usize] = clips;
    lumps[PxbspLumpKind::Models as usize] = model;
    Flat {
        bytes: write_pxbsp(&lumps).expect("flat container"),
        face_of,
        leaf_of,
    }
}

fn load_flat(flat: &Flat) -> PxbspResidentMap {
    let mut map = PxbspResidentMap::with_capacity(flat.bytes.len() * 2);
    map.load(1, &mut SliceReader::new(&flat.bytes))
        .expect("the flat reference is a valid legacy map");
    map
}

fn load_streamed(world: &StreamedBrushWorld, slots: u16) -> PxbspResidentMap {
    let mut map = PxbspResidentMap::with_capacity(world.container.len() * 8 + (1 << 22));
    map.load_streamed(1, &mut SliceReader::new(&world.container), slots)
        .expect("streamed container");
    map
}

fn install_all(world: &StreamedBrushWorld, map: &mut PxbspResidentMap) {
    let mut loader = RegionLoader::new(SliceReader::new(&world.region_pack), 0);
    let mut buf = vec![0u8; world.region_pack.len().max(4096)];
    for r in 0..world.index.regions.len() as u16 {
        let n = loader.load(&world.index, r, &mut buf).expect("region read");
        let slot = map.streaming().unwrap().free_slot().expect("free slot");
        map.install_region(r, slot, &buf[..n]).expect("install");
    }
}

// ---- gate (a): streamed == whole ------------------------------------------

struct Sampled {
    views: usize,
    skipped: usize,
    pvs_faces_total: usize,
    frame_faces_total: usize,
    source_ids_total: usize,
}

fn compare_views(world: &StreamedBrushWorld, views: usize, seed: u32) -> Sampled {
    configure_projection();
    let flat = flatten(world);
    let flat_map = load_flat(&flat);
    let regions = world.index.regions.len() as u16;
    let mut streamed = load_streamed(world, regions);
    install_all(world, &mut streamed);
    streamed.check_integrity().expect("integrity");

    let mut flat_renderer =
        Renderer::new_pxbsp_with_nodes(flat_map.faces().len(), flat_map.nodes().len());
    let mut streamed_renderer =
        Renderer::new_pxbsp_with_nodes(streamed.faces().len(), streamed.nodes().len());
    let (mins, maxs) = world.debug.world_bounds;
    let mut state = seed;
    let mut out = Sampled {
        views: 0,
        skipped: 0,
        pvs_faces_total: 0,
        frame_faces_total: 0,
        source_ids_total: 0,
    };
    let state_ref = streamed.streaming().unwrap();
    let caps = state_ref.index().caps;
    let mut attempts = 0;
    while out.views < views {
        attempts += 1;
        assert!(
            attempts < views * 400,
            "could not find {views} open camera positions"
        );
        let mut origin = [0i32; 3];
        for axis in 0..3 {
            let span = i32::from(maxs[axis]) - i32::from(mins[axis]);
            let value = i32::from(mins[axis]) + (lcg(&mut state) as i32).rem_euclid(span.max(1));
            origin[axis] = value << 12;
        }
        let camera = Camera {
            origin: Vec3I32 {
                x: origin[0],
                y: origin[1],
                z: origin[2],
            },
            angles: [
                (lcg(&mut state) % 4096) as i16 - 2048 + 1024 * 0,
                (lcg(&mut state) % 4096) as i16,
                0,
            ],
        };
        let flat_leaf = flat_map.point_leaf_index(camera.origin).unwrap();
        let streamed_leaf = streamed.point_leaf_index(camera.origin).unwrap();
        let flat_open =
            flat_leaf != 0 && flat_map.leaves().get(flat_leaf).unwrap().contents != CONTENTS_SOLID;
        let streamed_open = streamed_leaf != 0
            && streamed.leaves().get(streamed_leaf).unwrap().contents != CONTENTS_SOLID;
        assert_eq!(
            flat_open, streamed_open,
            "the two maps disagree on whether {origin:?} is solid"
        );
        if !flat_open {
            out.skipped += 1;
            continue;
        }
        // Same leaf in both maps.
        let (region, local) = flat.leaf_of[flat_leaf];
        let slot_state = streamed.streaming().unwrap();
        let slot = slot_state.slot_of(region as u16).expect("installed") as usize;
        let top_leaves = slot_state.index().top.leaves as usize;
        assert_eq!(
            streamed_leaf,
            top_leaves + slot * caps.leaves as usize + (local - 1)
        );

        let view = load_pxbsp_view(camera);
        let (pvs_flat, frame_flat) = flat_renderer
            .debug_world_selection(&flat_map, camera, view)
            .expect("flat selection");
        let (pvs_streamed, frame_streamed) = streamed_renderer
            .debug_world_selection(&streamed, camera, view)
            .expect("streamed selection");
        let key_flat = |f: &u16| flat.face_of[*f as usize];
        let key_streamed = |f: &u16| {
            let slot = *f as usize / caps.faces as usize;
            let region = streamed
                .streaming()
                .unwrap()
                .region_in_slot(slot as u16)
                .unwrap() as usize;
            (region, *f as usize % caps.faces as usize)
        };
        let mut a: Vec<_> = pvs_flat.iter().map(key_flat).collect();
        let mut b: Vec<_> = pvs_streamed.iter().map(key_streamed).collect();
        a.sort_unstable();
        b.sort_unstable();
        assert_eq!(
            a, b,
            "PVS face sets differ at {origin:?} angles {:?}",
            camera.angles
        );
        let mut c: Vec<_> = frame_flat.iter().map(key_flat).collect();
        let mut d: Vec<_> = frame_streamed.iter().map(key_streamed).collect();
        c.sort_unstable();
        d.sort_unstable();
        assert_eq!(
            c, d,
            "frame face sets differ at {origin:?} angles {:?}",
            camera.angles
        );
        // The same sets by stable source id.
        let ids = |keys: &[(usize, usize)]| {
            let mut v: Vec<u32> = keys
                .iter()
                .map(|&(r, f)| world.debug.face_source[r][f])
                .collect();
            v.sort_unstable();
            v.dedup();
            v
        };
        assert_eq!(ids(&c), ids(&d));
        out.source_ids_total += ids(&c).len();
        out.pvs_faces_total += a.len();
        out.frame_faces_total += c.len();
        out.views += 1;
    }
    assert_eq!(streamed.streaming().unwrap().pvs_missing(), 0);
    out
}

#[test]
fn graybox_reach_streamed_equals_whole_over_sampled_views() {
    let (project, root) = graybox();
    let world = cook(&project, &root, &PartitionParams::default());
    assert!(world.index.regions.len() >= 2);
    let sampled = compare_views(&world, 400, 0xC0FFEE);
    println!(
        "graybox-reach: {} regions, {} views ({} closed samples skipped), {} pvs faces, {} frame faces, {} source ids",
        world.index.regions.len(),
        sampled.views,
        sampled.skipped,
        sampled.pvs_faces_total,
        sampled.frame_faces_total,
        sampled.source_ids_total
    );
    assert!(sampled.frame_faces_total > 0);
}

fn small_world() -> (ProjectDocument, PathBuf) {
    let donor = load_donor(&projects_dir()).expect("donor");
    let generated = generate(&StreamWorldConfig::small([4, 4]), &donor).expect("world");
    (generated.project, projects_dir().join("graybox-reach"))
}

#[test]
fn stress_world_small_variant_streamed_equals_whole() {
    let (project, root) = small_world();
    let world = cook(&project, &root, &PartitionParams::default());
    let sampled = compare_views(&world, 400, 0xBADC0DE);
    println!(
        "stress-small: {} regions, {} views ({} skipped), {} frame faces",
        world.index.regions.len(),
        sampled.views,
        sampled.skipped,
        sampled.frame_faces_total
    );
    assert!(sampled.frame_faces_total > 0);
}

// ---- gate (b): collision --------------------------------------------------

#[test]
fn collision_matches_the_whole_map_and_absent_regions_are_walls() {
    let (project, root) = graybox();
    let world = cook(&project, &root, &PartitionParams::default());
    let flat = flatten(&world);
    let flat_map = load_flat(&flat);
    let regions = world.index.regions.len() as u16;
    let mut map = load_streamed(&world, regions);
    let (mins, maxs) = world.debug.world_bounds;
    let mut state = 77u32;
    let sample = |state: &mut u32| {
        let mut p = [0i32; 3];
        for axis in 0..3 {
            let span = i32::from(maxs[axis]) - i32::from(mins[axis]);
            p[axis] = (i32::from(mins[axis]) + (lcg(state) as i32).rem_euclid(span.max(1))) << 12;
        }
        Vec3I32 {
            x: p[0],
            y: p[1],
            z: p[2],
        }
    };
    // Nothing installed: every hull reads solid everywhere.
    for _ in 0..500 {
        let p = sample(&mut state);
        for hull in 0..3 {
            assert_eq!(
                map.model_collision_hull(0, hull).unwrap().point_contents(p),
                Some(CONTENTS_SOLID),
                "hull {hull} with nothing installed"
            );
        }
    }
    install_all(&world, &mut map);
    let mut open = 0;
    for _ in 0..4000 {
        let p = sample(&mut state);
        for hull in 0..3 {
            let a = flat_map
                .model_collision_hull(0, hull)
                .unwrap()
                .point_contents(p);
            let b = map.model_collision_hull(0, hull).unwrap().point_contents(p);
            assert_eq!(a, b, "hull {hull} contents at {p:?}");
            open += usize::from(hull == 1 && a == Some(CONTENTS_EMPTY));
        }
        // And a short trace.
        let q = Vec3I32 {
            x: p.x + (lcg(&mut state) as i32 % 64 - 32) * 4096,
            y: p.y + (lcg(&mut state) as i32 % 64 - 32) * 4096,
            z: p.z + (lcg(&mut state) as i32 % 64 - 32) * 4096,
        };
        for hull in 1..3 {
            let mut scratch_a = psx_bsp::collision::TraceScratch::new();
            let mut scratch_b = psx_bsp::collision::TraceScratch::new();
            let mut ta = psx_bsp::collision::Trace::default();
            let mut tb = psx_bsp::collision::Trace::default();
            let ra = flat_map.model_collision_hull(0, hull).unwrap().trace_into(
                &p,
                &q,
                &mut scratch_a,
                &mut ta,
            );
            let rb = map.model_collision_hull(0, hull).unwrap().trace_into(
                &p,
                &q,
                &mut scratch_b,
                &mut tb,
            );
            assert_eq!(ra, rb);
            assert_eq!(
                (ta.fraction, ta.end),
                (tb.fraction, tb.end),
                "hull {hull} trace {p:?}->{q:?}"
            );
        }
    }
    assert!(open > 100, "the sample should hit open space ({open})");
    // Evicting a region turns exactly its cell into a wall.
    for victim in 0..regions {
        map.uninstall_region(victim).unwrap();
        map.check_integrity().unwrap();
        for _ in 0..800 {
            let p = sample(&mut state);
            let at_victim = map.unresident_region_at(p) == Some(victim);
            let c = map.model_collision_hull(0, 1).unwrap().point_contents(p);
            if at_victim {
                assert_eq!(c, Some(CONTENTS_SOLID));
                let leaf = map.point_leaf_index(p).unwrap();
                assert_eq!(map.leaves().get(leaf).unwrap().contents, CONTENTS_SOLID);
            }
        }
        let mut loader = RegionLoader::new(SliceReader::new(&world.region_pack), 0);
        let mut buf = vec![0u8; world.region_pack.len().max(4096)];
        let n = loader.load(&world.index, victim, &mut buf).unwrap();
        map.install_region(
            victim,
            map.streaming().unwrap().free_slot().unwrap(),
            &buf[..n],
        )
        .unwrap();
    }
    assert_eq!(
        map.install_region(0, 0, &world.payloads[0]),
        Err(StreamError::AlreadyInstalled)
    );
}

#[test]
fn random_install_orders_on_a_cooked_world_never_dangle() {
    let (project, root) = graybox();
    let world = cook(&project, &root, &PartitionParams::default());
    let regions = world.index.regions.len() as u16;
    let slots = (regions / 2).max(2);
    let mut state = 4242u32;
    let mut ops = 0;
    for _ in 0..30 {
        let mut map = load_streamed(&world, slots);
        for _ in 0..150 {
            let region = (lcg(&mut state) % u32::from(regions)) as u16;
            if map.streaming().unwrap().is_resident(region) {
                if lcg(&mut state) % 2 == 0 {
                    map.uninstall_region(region).unwrap();
                    ops += 1;
                }
            } else if let Some(slot) = map.streaming().unwrap().free_slot() {
                map.install_region(region, slot, &world.payloads[region as usize])
                    .unwrap();
                ops += 1;
            }
            map.check_integrity().unwrap_or_else(|e| panic!("{e:?}"));
        }
    }
    println!("random install/uninstall: {ops} operations, integrity checked after each");
}

// ---- format and cook properties -------------------------------------------

#[test]
fn a_loader_without_streaming_support_refuses_the_container() {
    let (project, root) = graybox();
    let world = cook(&project, &root, &PartitionParams::default());
    let mut map = PxbspResidentMap::with_capacity(world.container.len() * 2);
    // The streaming loader accepts it only through load_streamed.
    assert_eq!(
        map.load(1, &mut SliceReader::new(&world.container)),
        Err(psx_bsp::pxbsp_resident::PxbspMapLoadError::StreamedWorldUnsupported)
    );
    // A loader that predates the feature validates leaf contents -6..=-1 and
    // would see the -7 stub leaves: build that view by erasing the index.
    let index = PxbspIndex::read(&mut SliceReader::new(&world.container)).unwrap();
    let leaves = index.lump(PxbspLumpKind::Leaves);
    let record = &world.container[leaves.offset as usize + 14..][..14];
    assert_eq!(record[0] as i8, -7, "stub leaf wire contents");
}

#[test]
fn cook_is_deterministic_and_regions_fit_their_declared_shape() {
    let (project, root) = graybox();
    let a = cook(&project, &root, &PartitionParams::default());
    let b = cook(&project, &root, &PartitionParams::default());
    assert_eq!(a.container, b.container);
    assert_eq!(a.region_pack, b.region_pack);
    for (r, entry) in a.index.regions.iter().enumerate() {
        assert_eq!(
            entry.fnv,
            psx_bsp::pxbsp_resident::stream::fnv1a32(&a.payloads[r])
        );
        assert!(entry.vis_count as usize * a.index.caps.leaves as usize / 8 <= 1024);
        assert_eq!(a.index.vis_list(r)[0] as usize, r);
        // Every face of the region has a stable source id.
        assert_eq!(a.debug.face_source[r].len(), a.stats.regions[r].faces);
    }
    println!("{:#?}", a.stats.regions);
}

#[test]
fn under_budget_projects_cook_byte_identical_to_the_whole_map_path() {
    let (project, root) = graybox();
    // A pool and region size no partition needs to cut for.
    let mut params = PartitionParams::default();
    params.region_target_bytes = 64 * 1024 * 1024;
    params.region_hard_cap_bytes = 64 * 1024 * 1024;
    params.pool_bytes = u32::MAX;
    params.caps = crate::brush_region::SlotCaps {
        faces: u32::MAX,
        vertices: u32::MAX,
        nodes: u32::MAX,
        leaves: u32::MAX,
        mark_surfaces: u32::MAX,
        clip_nodes: u32::MAX,
    };
    let cooked = cook_project_streamed(&project, &root, BrushWorldCookMode::Draft, [0; 3], &params)
        .expect("cook");
    let CookedWorld::Whole(whole) = cooked else {
        panic!("one region must take the whole-map path");
    };
    let mut scaled = project.clone();
    crate::units::scale_project_to_engine_units(&mut scaled);
    let reference = compile_brush_world(
        &scaled,
        BrushWorldCookOptions {
            project_root: &root,
            mode: BrushWorldCookMode::Draft,
            ambient: [0; 3],
            texture_asset_base: 0,
        },
    )
    .expect("reference cook");
    assert_eq!(whole.pxbsp.bytes, reference.pxbsp.bytes);
    // And an empty StreamingIndex lump, as before.
    let index = PxbspIndex::read(&mut SliceReader::new(&whole.pxbsp.bytes)).unwrap();
    assert_eq!(index.lump(PxbspLumpKind::StreamingIndex).len, 0);
}
