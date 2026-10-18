//! Preview timing harness (ignored by default).
//!
//! `cargo test -p frontend --features editor --release editor_preview::perf -- --ignored --nocapture --test-threads=1`
//!
//! Prints `PERF` lines: the texture refresh the shell runs every frame, the
//! full frame build from nothing, the steady-state rebuild that reuses the
//! previous frame's buffers, and (when a wgpu adapter exists) the headless
//! render of the resulting command log.

use super::*;
use std::path::PathBuf;
use std::sync::Mutex;

const DEFAULT_LEVELS: [&str; 4] = [
    "grid-cathedral",
    "grid-tower",
    "grid-dungeon",
    "grid-level008",
];

fn levels() -> Vec<String> {
    match std::env::var("PERF_LEVELS") {
        Ok(list) => list.split(',').map(str::to_string).collect(),
        Err(_) => DEFAULT_LEVELS.iter().map(|s| s.to_string()).collect(),
    }
}

fn project_dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../editor/projects")
        .join(name)
}

fn ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

fn report(what: &str, level: &str, v: &mut [f64]) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let at = |q: f64| v[((v.len() as f64 - 1.0) * q).round() as usize];
    println!(
        "PERF {what:<30} {level:<15} n={:<3} median {:8.3} ms  p95 {:8.3}  max {:8.3}",
        v.len(),
        at(0.5),
        at(0.95),
        v[v.len() - 1]
    );
}

fn cameras(project: &ProjectDocument) -> Vec<(&'static str, ViewportCameraState)> {
    let bounds = project
        .active_scene()
        .world
        .grid()
        .and_then(|w| w.editor_bounds())
        .expect("grid level with geometry");
    let mid = [
        (bounds.min[0] + bounds.max[0]) / 2,
        (bounds.min[1] + bounds.max[1]) / 2,
        (bounds.min[2] + bounds.max[2]) / 2,
    ];
    let span = (bounds.max[0] - bounds.min[0]).max(bounds.max[2] - bounds.min[2]);
    let cam = |yaw: u16, pitch: u16, radius: i32| ViewportCameraState {
        mode: psxed_ui::ViewportCameraMode::Orbit,
        yaw_q12: yaw,
        pitch_q12: pitch,
        radius,
        target: mid,
        position: [0; 3],
    };
    vec![
        ("overview", cam(512, 4096 - 700, span)),
        ("interior", cam(1200, 4096 - 250, 3000)),
    ]
}

#[test]
#[ignore]
fn perf_preview_frame() {
    let gpu = headless_device();
    for name in levels() {
        let root = project_dir(&name);
        let project = ProjectDocument::load_from_path(root.join("project.ron")).expect("load");
        let mut textures = EditorTextures::new();
        let mut assets = crate::editor_assets::EditorAssets::new();
        // First refresh uploads everything; the per-frame one is the no-op check.
        let t = Instant::now();
        textures.refresh(&project, &root);
        textures.refresh_models(&project, &root);
        assets.refresh(&project, &root);
        println!(
            "PERF textures first refresh        {name:<15} {:8.3} ms",
            ms(t)
        );
        let mut refresh = Vec::new();
        for _ in 0..60 {
            let t = Instant::now();
            textures.refresh(&project, &root);
            textures.refresh_models(&project, &root);
            assets.refresh(&project, &root);
            refresh.push(ms(t));
        }
        report("texture refresh (per frame)", &name, &mut refresh);

        let hidden = HashSet::new();
        for (view, camera) in cameras(&project) {
            let label = |s: &str| format!("{s} {view}");
            let mut fresh = Vec::new();
            for _ in 0..8 {
                let t = Instant::now();
                let frame = build_phase1_frame(
                    &project,
                    camera,
                    true,
                    true,
                    &hidden,
                    NodeId::ROOT,
                    None,
                    None,
                    &[],
                    None,
                    &textures,
                    &assets,
                );
                fresh.push(ms(t));
                std::hint::black_box(&frame);
            }
            report(&label("build_frame fresh"), &name, &mut fresh);

            let mut reuse = Vec::new();
            let mut frame = EditorPreviewFrame::default();
            for _ in 0..40 {
                let t = Instant::now();
                frame = build_phase1_frame_reusing(
                    &project,
                    camera,
                    true,
                    true,
                    &hidden,
                    NodeId::ROOT,
                    None,
                    None,
                    &[],
                    None,
                    &textures,
                    &assets,
                    frame,
                );
                reuse.push(ms(t));
            }
            report(&label("build_frame reusing"), &name, &mut reuse);
            println!(
                "PERF commands {view:<10} {name:<15} {} cmd_log entries, {} overlay lines, hash {:016x}",
                frame.cmd_log.len(),
                frame.overlay_lines.len(),
                frame_hash(&frame)
            );

            // Camera orbit: the geometry is kept, every frame is a new view.
            let mut orbit = Vec::new();
            let mut frame = EditorPreviewFrame::default();
            let mut moving = camera;
            for _ in 0..40 {
                moving.yaw_q12 = moving.yaw_q12.wrapping_add(24);
                let t = Instant::now();
                frame = build_phase1_frame_reusing(
                    &project,
                    moving,
                    true,
                    true,
                    &hidden,
                    NodeId::ROOT,
                    None,
                    None,
                    &[],
                    None,
                    &textures,
                    &assets,
                    frame,
                );
                orbit.push(ms(t));
            }
            report(&label("orbit frame"), &name, &mut orbit);

            // Editing: a sector's floor goes up and down, the frame is rebuilt.
            let mut edited = project.clone();
            let mut edit = Vec::new();
            let target = edited.active_scene().world.grid().and_then(|world| {
                world.rooms.iter().enumerate().find_map(|(index, room)| {
                    room.iter()
                        .find(|(_, _, sector)| sector.ground_floor().is_some())
                        .map(|(x, z, _)| (index, x, z))
                })
            });
            if let Some((room_index, x, z)) = target {
                for step in 0..20 {
                    edited.active_scene_mut().world.grid_mut().unwrap().rooms[room_index]
                        .sector_mut(x, z)
                        .and_then(|sector| sector.ground_floor_mut())
                        .unwrap()
                        .heights = [256 * (step % 3); 4];
                    let t = Instant::now();
                    frame = build_phase1_frame_reusing(
                        &edited,
                        camera,
                        true,
                        true,
                        &hidden,
                        NodeId::ROOT,
                        None,
                        None,
                        &[],
                        None,
                        &textures,
                        &assets,
                        frame,
                    );
                    edit.push(ms(t));
                }
                report(&label("edit + rebuild"), &name, &mut edit);
            }

            // A camera that does not move: does the frame come back as it was?
            let still = build_phase1_frame_reusing(
                &project,
                camera,
                true,
                true,
                &hidden,
                NodeId::ROOT,
                None,
                None,
                &[],
                None,
                &textures,
                &assets,
                EditorPreviewFrame::default(),
            );
            let serial = still.serial;
            let t = Instant::now();
            let again = build_phase1_frame_reusing(
                &project,
                camera,
                true,
                true,
                &hidden,
                NodeId::ROOT,
                None,
                None,
                &[],
                None,
                &textures,
                &assets,
                still,
            );
            println!(
                "PERF still frame {view:<10} {name:<15} handed back: {} ({:.3} ms)",
                again.serial == serial,
                ms(t)
            );

            if let Some((device, queue)) = gpu.as_ref() {
                let mut hw =
                    psx_gpu_render::HwRenderer::new_headless(device.clone(), queue.clone());
                let _ = hw.set_internal_scale(2, None);
                let stub = emulator_core::Gpu::new();
                let mut render = Vec::new();
                for _ in 0..20 {
                    let t = Instant::now();
                    hw.render_frame(&stub, &frame.cmd_log, textures.vram_words());
                    render.push(ms(t));
                }
                report(&label("hw render_frame (submit)"), &name, &mut render);
            }
        }
    }
}

/// FNV-1a over the Debug text of every command and overlay line: a golden for
/// "the preview draws exactly what it drew before".
pub(super) fn frame_hash(frame: &EditorPreviewFrame) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut feed = |text: String| {
        for byte in text.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    for entry in &frame.cmd_log {
        feed(format!("{entry:?}"));
    }
    for line in &frame.overlay_lines {
        feed(format!("{line:?}"));
    }
    hash
}

fn headless_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..Default::default()
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))?;
    pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("perf-device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::Performance,
        },
        None,
    ))
    .ok()
}

/// Every grid reference level, from two cameras, drawn with the clock pinned.
/// `PREVIEW_GOLDEN=print` prints the table to paste below.
#[test]
fn reference_levels_draw_what_they_always_drew() {
    use std::sync::atomic::Ordering;
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    state::reset_for_tests();
    TEST_TICK.store(77, Ordering::Relaxed);
    let mut table = Vec::new();
    for name in GOLDEN_LEVELS {
        let root = project_dir(name);
        let project = ProjectDocument::load_from_path(root.join("project.ron")).expect("load");
        let mut textures = EditorTextures::new();
        let mut assets = crate::editor_assets::EditorAssets::new();
        textures.refresh(&project, &root);
        textures.refresh_models(&project, &root);
        assets.refresh(&project, &root);
        let hidden = HashSet::new();
        for (view, camera) in cameras(&project) {
            let frame = build_phase1_frame(
                &project,
                camera,
                true,
                true,
                &hidden,
                NodeId::ROOT,
                None,
                None,
                &[],
                None,
                &textures,
                &assets,
            );
            table.push((*name, view, frame_hash(&frame)));
        }
    }
    TEST_TICK.store(-1, Ordering::Relaxed);
    if std::env::var("PREVIEW_GOLDEN").as_deref() == Ok("print") {
        for (name, view, hash) in &table {
            println!("    (\"{name}\", \"{view}\", 0x{hash:016x}),");
        }
    }
    assert_eq!(table.len(), GOLDEN.len());
    for ((name, view, hash), (gname, gview, ghash)) in table.iter().zip(GOLDEN) {
        assert_eq!((*name, *view), (*gname, *gview));
        assert_eq!(*hash, *ghash, "{name} {view}: the preview changed");
    }
}

/// Tests that pin the clock or compare against a cold cache run one at a time.
static LOCK: Mutex<()> = Mutex::new(());

const GOLDEN_LEVELS: &[&str] = &[
    "grid-cathedral",
    "grid-tower",
    "grid-dungeon",
    "grid-level008",
    "grid-cave",
    "grid-sewers",
    "grid-west",
    "grid-starter",
    "grid-gate",
];

/// Recorded from the preview as it was before the grid geometry cache.
const GOLDEN: &[(&str, &str, u64)] = &[
    ("grid-cathedral", "overview", 0xe6ce8be0ebb8aa5b),
    ("grid-cathedral", "interior", 0xe6a8c25330e3392b),
    ("grid-tower", "overview", 0xae46076dd968443a),
    ("grid-tower", "interior", 0xb565922ff97e67aa),
    ("grid-dungeon", "overview", 0x7a02b2c41b244600),
    ("grid-dungeon", "interior", 0x29e7e1ebef8aea94),
    ("grid-level008", "overview", 0xf5a1bf1dc54eeb02),
    ("grid-level008", "interior", 0xea1af84c6d2e021a),
    ("grid-cave", "overview", 0xa4a187427f436d18),
    ("grid-cave", "interior", 0x82b971b868486736),
    ("grid-sewers", "overview", 0x953368311817b238),
    ("grid-sewers", "interior", 0xaf41848564fb9b7c),
    ("grid-west", "overview", 0xce8478ee8c3813d2),
    ("grid-west", "interior", 0x164c585580b2cb54),
    ("grid-starter", "overview", 0x8d6c056af966c8b1),
    ("grid-starter", "interior", 0x7feeadbdfdc729ea),
    ("grid-gate", "overview", 0x5caa40040f527299),
    ("grid-gate", "interior", 0x5bf11821b7d6da3a),
];

fn load_level(
    name: &str,
) -> (
    PathBuf,
    ProjectDocument,
    EditorTextures,
    crate::editor_assets::EditorAssets,
) {
    let root = project_dir(name);
    let project = ProjectDocument::load_from_path(root.join("project.ron")).expect("load");
    let mut textures = EditorTextures::new();
    let mut assets = crate::editor_assets::EditorAssets::new();
    textures.refresh(&project, &root);
    textures.refresh_models(&project, &root);
    assets.refresh(&project, &root);
    (root, project, textures, assets)
}

fn build(
    project: &ProjectDocument,
    camera: ViewportCameraState,
    textures: &EditorTextures,
    assets: &crate::editor_assets::EditorAssets,
    frame: EditorPreviewFrame,
) -> EditorPreviewFrame {
    build_phase1_frame_reusing(
        project,
        camera,
        true,
        true,
        &HashSet::new(),
        NodeId::ROOT,
        None,
        None,
        &[],
        None,
        textures,
        assets,
        frame,
    )
}

/// The frame a cold cache builds for `project`: the oracle an incremental
/// build must equal.
fn cold_hash(
    project: &ProjectDocument,
    camera: ViewportCameraState,
    textures: &EditorTextures,
    assets: &crate::editor_assets::EditorAssets,
) -> u64 {
    state::reset_for_tests();
    frame_hash(&build(
        project,
        camera,
        textures,
        assets,
        EditorPreviewFrame::default(),
    ))
}

#[test]
fn an_incremental_build_equals_a_cold_one_after_every_kind_of_edit() {
    use std::sync::atomic::Ordering;
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    TEST_TICK.store(5, Ordering::Relaxed);
    for name in ["grid-cathedral", "grid-tower", "grid-dungeon"] {
        let (_root, mut project, textures, assets) = load_level(name);
        let cameras = cameras(&project);
        state::reset_for_tests();
        // Warm the cache from both cameras, then edit and compare each time.
        let mut frame = EditorPreviewFrame::default();
        for (_, camera) in &cameras {
            frame = build(&project, *camera, &textures, &assets, frame);
        }
        let mut edits: Vec<(&str, Box<dyn Fn(&mut ProjectDocument)>)> = Vec::new();
        edits.push((
            "raise a floor",
            Box::new(|p| {
                let world = p.active_scene_mut().world.grid_mut().expect("grid");
                for room in &mut world.rooms {
                    let found = room
                        .iter()
                        .find(|(_, _, sector)| sector.ground_floor().is_some())
                        .map(|(x, z, _)| (x, z));
                    if let Some((x, z)) = found {
                        let floor = room
                            .sector_mut(x, z)
                            .and_then(|s| s.ground_floor_mut())
                            .expect("floor");
                        floor.heights = floor.heights.map(|h| h + 256);
                        break;
                    }
                }
            }),
        ));
        edits.push((
            "recolour a material",
            Box::new(|p| {
                for resource in &mut p.resources {
                    if let ResourceData::Material(material) = &mut resource.data {
                        material.tint = [200, 90, 40];
                        break;
                    }
                }
            }),
        ));
        edits.push((
            "darken a room",
            Box::new(|p| {
                let world = p.active_scene_mut().world.grid_mut().expect("grid");
                world.rooms[0].ambient = [10, 12, 14];
            }),
        ));
        edits.push((
            "move a light",
            Box::new(|p| {
                let scene = p.active_scene_mut();
                let light = scene
                    .nodes()
                    .iter()
                    .find(|node| matches!(node.kind, NodeKind::PointLight { .. }))
                    .map(|node| node.id);
                if let Some(node) = light.and_then(|id| scene.node_mut(id)) {
                    node.transform.translation[0] += 700.0;
                }
            }),
        ));
        for (what, edit) in &edits {
            edit(&mut project);
            for (view, camera) in &cameras {
                let incremental = build(&project, *camera, &textures, &assets, frame);
                let hash = frame_hash(&incremental);
                frame = incremental;
                // Build the same frame from nothing, then put the warm
                // state back for the next edit.
                let cold = cold_hash(&project, *camera, &textures, &assets);
                assert_eq!(hash, cold, "{name} {view}: {what}");
                frame = build(
                    &project,
                    *camera,
                    &textures,
                    &assets,
                    EditorPreviewFrame::default(),
                );
            }
        }
    }
    TEST_TICK.store(-1, Ordering::Relaxed);
}

#[test]
fn a_still_scene_is_handed_back_and_any_change_rebuilds_it() {
    use psxed_project::grid::{GridRoom, GridSurface, GridWorld, SceneWorld};
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    state::reset_for_tests();
    // No model, particle, beacon or layered sky: nothing moves with the clock.
    let mut room = GridRoom::new("hall", 3, 3);
    for x in 0..3 {
        for z in 0..3 {
            let sector = room.sector_or_insert(x, z).unwrap();
            sector.set_ground_floor(Some(GridSurface::flat(0, None)));
        }
    }
    let mut project = ProjectDocument::new("still");
    project.active_scene_mut().world = SceneWorld::Grid(GridWorld {
        rooms: vec![room],
        ..GridWorld::new()
    });
    let textures = EditorTextures::new();
    let assets = crate::editor_assets::EditorAssets::new();
    let camera = ViewportCameraState {
        mode: psxed_ui::ViewportCameraMode::Orbit,
        yaw_q12: 320,
        pitch_q12: 4096 - 400,
        radius: 4096,
        target: [1536, 0, 1536],
        position: [0; 3],
    };
    let first = build(
        &project,
        camera,
        &textures,
        &assets,
        EditorPreviewFrame::default(),
    );
    assert!(!first.cmd_log.is_empty());
    let (serial, ptr, len) = (first.serial, first.cmd_log.as_ptr(), first.cmd_log.len());
    let second = build(&project, camera, &textures, &assets, first);
    assert_eq!(second.serial, serial, "an unchanged scene is not rebuilt");
    assert_eq!((second.cmd_log.as_ptr(), second.cmd_log.len()), (ptr, len));

    // A moved camera is a different frame.
    let mut moved = camera;
    moved.yaw_q12 = moved.yaw_q12.wrapping_add(64);
    let third = build(&project, moved, &textures, &assets, second);
    assert_ne!(third.serial, serial);

    // So is an edit.
    let mut edited = project.clone();
    edited.active_scene_mut().world.grid_mut().unwrap().rooms[0]
        .sector_mut(1, 1)
        .and_then(|s| s.ground_floor_mut())
        .unwrap()
        .heights = [256; 4];
    let serial = third.serial;
    let fourth = build(&edited, moved, &textures, &assets, third);
    assert_ne!(fourth.serial, serial);
    // And a frame that was not the memo's is never mistaken for it.
    let stranger = build(
        &edited,
        moved,
        &textures,
        &assets,
        EditorPreviewFrame::default(),
    );
    assert_ne!(stranger.serial, 0);
}
