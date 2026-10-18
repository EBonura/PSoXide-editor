//! Editor timing harness (ignored by default).
//!
//! Run with
//! `cargo test -p psxed-ui --release perf_ -- --ignored --nocapture --test-threads=1`
//! and read the `PERF` lines. `PERF_LEVELS=grid-tower,grid-dungeon` narrows the
//! levels. The 3D preview rebuild lives in the frontend crate and has its own
//! harness there (`editor_preview::perf`).
//!
//! Every figure is wall clock on the machine that runs it; the point is the
//! ratio between two builds on one machine, not the absolute numbers.

use super::grid_rig::GridRig;
use super::*;
use std::time::Instant;

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
        .join("../../projects")
        .join(name)
}

fn load(name: &str) -> ProjectDocument {
    ProjectDocument::load_from_path(project_dir(name).join("project.ron"))
        .unwrap_or_else(|e| panic!("load {name}: {e}"))
}

fn rss_kb() -> u64 {
    let out = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .expect("ps");
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse()
        .unwrap_or(0)
}

fn ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

/// median, p95, max of `v` in the unit it was recorded in.
fn stats(v: &mut [f64]) -> (f64, f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let at = |q: f64| v[((v.len() as f64 - 1.0) * q).round() as usize];
    (at(0.5), at(0.95), *v.last().unwrap())
}

fn report(what: &str, level: &str, v: &mut [f64]) {
    let (med, p95, max) = stats(v);
    println!(
        "PERF {what:<26} {level:<15} n={:<3} median {med:8.3} ms  p95 {p95:8.3}  max {max:8.3}",
        v.len()
    );
}

#[test]
#[ignore]
fn perf_load_save_memory() {
    for name in levels() {
        let rss0 = rss_kb();
        let mut load_ms = Vec::new();
        let mut project = load(&name);
        for _ in 0..5 {
            let t = Instant::now();
            project = load(&name);
            load_ms.push(ms(t));
        }
        let rss1 = rss_kb();
        report("load project.ron", &name, &mut load_ms);

        let mut open_ms = Vec::new();
        for _ in 0..3 {
            let t = Instant::now();
            let ws = EditorWorkspace::open_directory(project_dir(&name)).expect("open");
            open_ms.push(ms(t));
            drop(ws);
        }
        report("open_directory", &name, &mut open_ms);

        let dir = test_temp_dir("perf-save");
        std::fs::create_dir_all(&dir).unwrap();
        let mut ws = EditorWorkspace::with_project(dir.clone(), project.clone());
        let mut save_ms = Vec::new();
        for _ in 0..5 {
            let t = Instant::now();
            ws.save().expect("save");
            save_ms.push(ms(t));
        }
        report("save", &name, &mut save_ms);

        let mut clone_ms = Vec::new();
        for _ in 0..20 {
            let t = Instant::now();
            let c = project.clone();
            clone_ms.push(ms(t));
            std::hint::black_box(&c);
        }
        report("document clone (undo snap)", &name, &mut clone_ms);
        let snap = project.clone();
        let mut eq_ms = Vec::new();
        for _ in 0..20 {
            let t = Instant::now();
            let same = snap == project;
            eq_ms.push(ms(t));
            std::hint::black_box(same);
        }
        report("document == (undo dedupe)", &name, &mut eq_ms);

        // Memory: one document, then 64 undo snapshots of it.
        let rss2 = rss_kb();
        let snapshots: Vec<ProjectDocument> = (0..64).map(|_| project.clone()).collect();
        let rss3 = rss_kb();
        println!(
            "PERF memory {name:<15} document ~{} MiB (5 loads kept {} MiB net), 64 snapshots +{} MiB",
            (rss1.saturating_sub(rss0)) / 1024 / 6,
            rss1.saturating_sub(rss0) / 1024,
            rss3.saturating_sub(rss2) / 1024
        );
        drop(snapshots);
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// Full `workspace.draw` frames with nothing happening, Top and 3D views.
#[test]
#[ignore]
fn perf_idle_frame() {
    for name in levels() {
        for (label, top) in [("idle frame top", true), ("idle frame 3d", false)] {
            let project = load(&name);
            let mut rig = if top {
                GridRig::top("perf-idle", project)
            } else {
                GridRig::persp("perf-idle", project)
            };
            for _ in 0..10 {
                rig.pump(vec![], egui::Modifiers::NONE);
            }
            let mut frames = Vec::new();
            for _ in 0..120 {
                let t = Instant::now();
                rig.pump(vec![], egui::Modifiers::NONE);
                frames.push(ms(t));
            }
            report(label, &name, &mut frames);
        }
    }
}

/// Pointer sweeps and edits through the real tool path in the Top view.
#[test]
#[ignore]
fn perf_edit_latency_top() {
    for name in levels() {
        let project = load(&name);
        let bounds = project
            .active_scene()
            .world
            .grid()
            .and_then(|w| w.editor_bounds())
            .expect("level has geometry");
        let mut rig = GridRig::top("perf-top", project);
        let (min, max) = (bounds.min, bounds.max);
        let span = (max[0] - min[0]).max(max[2] - min[2]).max(1) as f32;
        rig.ws.orthographic_focus = [
            (min[0] + max[0]) as f32 * 0.5,
            0.0,
            (min[2] + max[2]) as f32 * 0.5,
        ];
        rig.ws.viewport_zoom = (600.0 / span).max(0.0005);
        for _ in 0..5 {
            rig.pump(vec![], egui::Modifiers::NONE);
        }
        // World sector size in editor units for this level.
        let sector = rig
            .world()
            .scale
            .to_editor(psxed_project::grid::SECTOR_UNITS) as f32;
        let centre = Pos2::new(
            rig.top_at(rig.ws.orthographic_focus[0], rig.ws.orthographic_focus[2])
                .x,
            rig.top_at(rig.ws.orthographic_focus[0], rig.ws.orthographic_focus[2])
                .y,
        );
        let cx = (rig.ws.orthographic_focus[0] / sector).floor() as i32;
        let cz = (rig.ws.orthographic_focus[2] / sector).floor() as i32;
        let _ = centre;

        // Hover sweep: the pointer moves with the Select tool.
        let mut hover = Vec::new();
        for i in 0..40 {
            let p = rig.cell_centre(cx - 6 + (i % 12), cz - 3 + (i / 12));
            let t = Instant::now();
            rig.move_to(p, egui::Modifiers::NONE);
            hover.push(ms(t));
        }
        report("hover top (select)", &name, &mut hover);

        // Floor drag: a 4 x 3 rectangle, one pump per pointer step.
        rig.tool(crate::grid_tools::GridTool::Floor);
        let a = rig.cell_centre(cx - 2, cz - 1);
        let b = rig.cell_centre(cx + 1, cz + 1);
        let mut floor = Vec::new();
        let t = Instant::now();
        rig.move_to(a, egui::Modifiers::NONE);
        floor.push(ms(t));
        let t = Instant::now();
        rig.button(a, true, egui::Modifiers::NONE);
        floor.push(ms(t));
        for step in 1..=8 {
            let p = a + (b - a) * (step as f32 / 8.0);
            let t = Instant::now();
            rig.move_to(p, egui::Modifiers::NONE);
            floor.push(ms(t));
        }
        let t = Instant::now();
        rig.release(b, egui::Modifiers::NONE);
        let release = ms(t);
        report("floor drag step", &name, &mut floor);
        println!("PERF floor release             {name:<15} {release:8.3} ms");

        // Undo and redo of that edit.
        let mut undo = Vec::new();
        for _ in 0..3 {
            let t = Instant::now();
            rig.key_with(egui::Key::Z, egui::Modifiers::COMMAND);
            undo.push(ms(t) / 2.0);
            let t = Instant::now();
            rig.key_with(
                egui::Key::Z,
                egui::Modifiers {
                    shift: true,
                    ..egui::Modifiers::COMMAND
                },
            );
            undo.push(ms(t) / 2.0);
        }
        report("undo/redo (per frame)", &name, &mut undo);

        // Paint drag over existing floor.
        rig.tool(crate::grid_tools::GridTool::Paint);
        let a = rig.cell_centre(cx - 4, cz);
        let b = rig.cell_centre(cx + 4, cz);
        let mut paint = Vec::new();
        rig.move_to(a, egui::Modifiers::NONE);
        let t = Instant::now();
        rig.button(a, true, egui::Modifiers::NONE);
        paint.push(ms(t));
        for step in 1..=16 {
            let p = a + (b - a) * (step as f32 / 16.0);
            let t = Instant::now();
            rig.move_to(p, egui::Modifiers::NONE);
            paint.push(ms(t));
        }
        let t = Instant::now();
        rig.release(b, egui::Modifiers::NONE);
        paint.push(ms(t));
        report("paint drag step", &name, &mut paint);

        // Raise verb through the pose path: region then drag.
        let spec = format!(
            "tool=raise;region={},{}:{},{};verb-drag=8",
            cx - 2,
            cz - 1,
            cx + 1,
            cz + 1
        );
        let t = Instant::now();
        let r = rig.ws.apply_grid_pose(&spec);
        let raise = ms(t);
        println!("PERF raise pose (region+drag)  {name:<15} {raise:8.3} ms ({r:?})");
    }
}

/// Pointer movement over the 3D view (hover pick, gizmo hit tests).
#[test]
#[ignore]
fn perf_hover_3d() {
    for name in levels() {
        let project = load(&name);
        let bounds = project
            .active_scene()
            .world
            .grid()
            .and_then(|w| w.editor_bounds())
            .expect("geometry");
        let mut rig = GridRig::persp("perf-3d", project);
        let mid = [
            (bounds.min[0] + bounds.max[0]) / 2,
            bounds.min[1],
            (bounds.min[2] + bounds.max[2]) / 2,
        ];
        let span = (bounds.max[0] - bounds.min[0]).max(bounds.max[2] - bounds.min[2]);
        rig.ws.camera_rig.free_position = [mid[0], mid[1] + span / 2, mid[2] + span / 2];
        let (yaw, pitch) =
            camera_angles_to_look_at(rig.ws.camera_rig.free_position, mid).expect("angles");
        rig.ws.camera_rig.free_yaw = yaw;
        rig.ws.camera_rig.free_pitch = pitch;
        for _ in 0..5 {
            rig.pump(vec![], egui::Modifiers::NONE);
        }
        let rect = rig.persp_rect();
        let mut hover = Vec::new();
        for i in 0..60 {
            let f = i as f32 / 60.0;
            let p = Pos2::new(
                rect.left() + rect.width() * (0.2 + 0.6 * f),
                rect.top() + rect.height() * (0.3 + 0.4 * ((f * 7.0).sin() * 0.5 + 0.5)),
            );
            let t = Instant::now();
            rig.move_to(p, egui::Modifiers::NONE);
            hover.push(ms(t));
        }
        report("hover 3d", &name, &mut hover);
    }
}

/// The whole `workspace.draw` (docks, tree, inspector, strip) in the 3D view,
/// idle, then with the pointer moving over the viewport.
#[test]
#[ignore]
fn perf_full_ui_frame() {
    use super::egui_driver::{real_egui_workspace_ctx, real_egui_workspace_frame};
    for name in levels() {
        let project = load(&name);
        let (ctx, viewport) = real_egui_workspace_ctx("perf-full");
        let mut ws = EditorWorkspace::with_project(test_temp_dir("perf-full"), project);
        ws.active_workspace = WorkspaceView::Room;
        ws.view_2d = false;
        let mut time = 0.0;
        for _ in 0..10 {
            time += 1.0 / 60.0;
            real_egui_workspace_frame(&ctx, &mut ws, &viewport, time, vec![]);
        }
        let mut idle = Vec::new();
        for _ in 0..90 * frames_mul() {
            time += 1.0 / 60.0;
            let t = Instant::now();
            real_egui_workspace_frame(&ctx, &mut ws, &viewport, time, vec![]);
            idle.push(ms(t));
        }
        report("full ui frame idle 3d", &name, &mut idle);
        let mut moving = Vec::new();
        for i in 0..60 {
            time += 1.0 / 60.0;
            let p = Pos2::new(500.0 + (i as f32) * 8.0, 300.0 + (i as f32) * 4.0);
            let t = Instant::now();
            real_egui_workspace_frame(
                &ctx,
                &mut ws,
                &viewport,
                time,
                vec![egui::Event::PointerMoved(p)],
            );
            moving.push(ms(t));
        }
        report("full ui frame hover 3d", &name, &mut moving);
        ws.view_2d = true;
        for _ in 0..10 {
            time += 1.0 / 60.0;
            real_egui_workspace_frame(&ctx, &mut ws, &viewport, time, vec![]);
        }
        let mut top = Vec::new();
        for _ in 0..90 * frames_mul() {
            time += 1.0 / 60.0;
            let t = Instant::now();
            real_egui_workspace_frame(&ctx, &mut ws, &viewport, time, vec![]);
            top.push(ms(t));
        }
        report("full ui frame idle top", &name, &mut top);
    }
}

/// `PERF_FRAMES_MUL=10` stretches the frame loops so a sampling profiler can
/// attach to the test process.
fn frames_mul() -> usize {
    std::env::var("PERF_FRAMES_MUL")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
}
