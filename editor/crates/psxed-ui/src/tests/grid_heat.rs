//! The performance heatmap overlay through the editor and on every reference
//! level.

use super::grid_derived::derived_project_for_tiling;
use super::grid_rig::*;
use super::grid_ui::{draw_editor, full_context};
use super::*;

fn text_of(output: &egui::FullOutput) -> String {
    output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) => Some(t.galley.text().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn alt_p_toggles_the_heatmap_and_it_draws_a_labelled_model() {
    let mut rig = GridRig::top("heat-key", derived_project_for_tiling(6, 6));
    rig.key_with(egui::Key::P, egui::Modifiers::ALT);
    assert!(rig.ws.grid.overlays.perf, "{}", rig.ws.status);
    rig.key_with(egui::Key::P, egui::Modifiers::ALT);
    assert!(!rig.ws.grid.overlays.perf);
    // A plain P is still the Paint tool.
    rig.key(egui::Key::P);
    assert_eq!(rig.ws.grid.tool, crate::grid_tools::GridTool::Paint);

    let ctx = full_context();
    let mut ws =
        EditorWorkspace::with_project(test_temp_dir("heat-draw"), derived_project_for_tiling(6, 6));
    ws.last_viewport_size = Vec2::new(1200.0, 700.0);
    ws.apply_grid_pose("overlay=perf").unwrap();
    for view_2d in [true, false] {
        let text = text_of(&draw_editor(&mut ws, &ctx, view_2d));
        assert!(text.contains("Performance heatmap"), "{text}");
        assert!(text.contains("MODEL"), "{text}");
        assert!(text.contains("worst spot"), "{text}");
        assert!(text.contains("under 60% of the world budget"), "{text}");
    }
}

#[test]
fn the_heatmap_renders_on_every_reference_level() {
    let ctx = full_context();
    for name in [
        "grid-cathedral",
        "grid-level008",
        "grid-dungeon",
        "grid-cave",
        "grid-sewers",
        "grid-west",
    ] {
        let path = psxed_project::projects_dir().join(name).join("project.ron");
        let project = ProjectDocument::load_from_path(path).expect("the level loads");
        let mut ws = EditorWorkspace::with_project(test_temp_dir(&format!("heat-{name}")), project);
        ws.last_viewport_size = Vec2::new(1200.0, 700.0);
        ws.apply_grid_pose("overlay=perf").unwrap();
        let text = text_of(&draw_editor(&mut ws, &ctx, true));
        assert!(text.contains("worst spot"), "{name}: {text}");
        let run = ws.heat_run().expect("a result");
        assert!(run.fresh && !run.map.samples.is_empty(), "{name}");
    }
}

/// The model on every level, printed, not asserted. Run with
/// `cargo test -p psxed-ui --release --lib measure_heatmap -- --ignored --nocapture`.
#[test]
#[ignore = "a measurement, not a check"]
fn measure_heatmap_on_the_reference_levels() {
    use psxed_project::grid::heat::{heatmap, HeatOptions};
    use psxed_project::grid::walk::{analyse, player_spawns, WalkLimits};
    for name in [
        "grid-cathedral",
        "grid-level008",
        "grid-dungeon",
        "grid-cave",
        "grid-sewers",
        "grid-west",
    ] {
        let path = psxed_project::projects_dir().join(name).join("project.ron");
        let project = ProjectDocument::load_from_path(path).expect("the level loads");
        let world = project.active_scene().world.grid().unwrap();
        let spawns = player_spawns(&project, world);
        let t = std::time::Instant::now();
        let walk = analyse(world, &spawns, &WalkLimits::DEFAULT).unwrap();
        let walk_ms = t.elapsed().as_secs_f64() * 1e3;
        let m = heatmap(world, HeatOptions::default()).unwrap();
        let w = m.worst().unwrap();
        eprintln!(
            "{name}: {} sectors, walkability {walk_ms:.2} ms (counts {:?}); heatmap {} spots, {} quads, cook {:.0} ms, sampling {:.0} ms; quads p50 {} p90 {} max {}; cycles p50 {} p90 {} max {} ({:.0}% of {})",
            world.sector_count(),
            walk.counts(),
            m.samples.len(),
            m.quads_total,
            m.cook_ms,
            m.sample_ms,
            m.quads_percentile(50),
            m.quads_percentile(90),
            m.quads_percentile(100),
            m.percentile(50),
            m.percentile(90),
            w.cycles,
            m.heat(w) * 100.0,
            m.options.world_budget()
        );
        if let Some(sp) = spawns.first() {
            let views = psxed_project::grid::heat::spot_views(
                world,
                sp.room,
                sp.cell,
                HeatOptions::default(),
            )
            .unwrap();
            eprintln!(
                "  spawn room {} sector {:?}: (quads, near) per yaw {views:?}",
                sp.room, sp.cell
            );
        }
    }
}
