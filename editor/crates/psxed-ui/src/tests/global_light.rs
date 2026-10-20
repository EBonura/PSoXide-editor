//! The World panel's global light section (lane editor-light, 2026-10-20),
//! driven through the whole editor window at 1456 x 819.

use super::grid_live::WindowRig;
use super::*;

fn frames(rig: &mut WindowRig, n: usize) {
    for _ in 0..n {
        rig.frame(vec![]);
    }
}

fn world_panel() -> WindowRig {
    let mut rig = WindowRig::starter(Vec2::new(1456.0, 819.0));
    // The rig's starter is an older one with the flat ambient: switch the
    // gather on the way a new project has it.
    rig.ws
        .project
        .active_scene_mut()
        .world
        .grid_mut()
        .unwrap()
        .global_light = psxed_project::grid::GlobalLight::NEW_WORLD;
    rig.ws.select_world();
    frames(&mut rig, 4);
    rig
}

#[test]
fn the_world_panel_shows_the_global_light_of_a_new_world_with_its_knobs() {
    assert!(
        psxed_project::grid::starter::starter_world(None, None, None)
            .global_light
            .enabled,
        "a new world gathers"
    );
    let rig = world_panel();
    assert!(rig.find("Global light").is_some(), "the section");
    assert!(rig.find("Gather light").is_some(), "the switch");
    for knob in [
        "Occlusion",
        "Reach",
        "Bounces",
        "Bounce gain",
        "Rays",
        "Sky",
    ] {
        assert!(rig.find(knob).is_some(), "the {knob} knob");
    }
}

#[test]
fn the_switch_turns_the_gather_off_and_on_and_the_knobs_follow() {
    let mut rig = world_panel();
    let switch = rig.at("Gather light");
    rig.click(switch);
    frames(&mut rig, 3);
    assert!(!rig.world().global_light.enabled, "ticked off");
    assert!(rig.find("Occlusion").is_none(), "the knobs are hidden");
    // The other settings are kept for when it comes back on.
    assert_eq!(rig.world().global_light.bounces, 2);
    let switch = rig.at("Gather light");
    rig.click(switch);
    frames(&mut rig, 3);
    assert!(rig.world().global_light.enabled, "ticked on");
    assert!(rig.find("Occlusion").is_some());
}

/// The panel in both states, written as PNGs.
/// `PSXED_LIGHT_CAPTURE=<dir>`.
#[test]
fn global_light_panel_captures() {
    let Some(dir) = std::env::var_os("PSXED_LIGHT_CAPTURE") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let mut rig = world_panel();
    rig.capture(&dir.join("world-panel-global-light-on.png"));
    let switch = rig.at("Gather light");
    rig.click(switch);
    frames(&mut rig, 3);
    rig.capture(&dir.join("world-panel-global-light-off.png"));
}
