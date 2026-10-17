//! The natural-terrain tools over the stdio protocol, on a fresh New Grid
//! Project: cracks, voids, wall heights, polygon outlines and chamfers.
//! The whole cave scenario (the proof of the block) lives in `grid_cave.rs`.

mod common;

use std::path::{Path, PathBuf};

use common::Client;
use serde_json::json;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("repo root")
}

fn new_project(tmp: &Path) -> PathBuf {
    let projects = tmp.join("projects");
    std::fs::create_dir_all(&projects).expect("create projects");
    for name in ["default", "graybox-reach"] {
        std::os::unix::fs::symlink(
            repo_root().join("editor/projects").join(name),
            projects.join(name),
        )
        .expect("link a base project");
    }
    let project = projects.join("natural");
    let made = std::process::Command::new(env!("CARGO_BIN_EXE_psxed-mcp"))
        .args(["--new", project.to_str().unwrap()])
        .env("PSOXIDE_PROJECTS_DIR", &projects)
        .output()
        .expect("run --new");
    assert!(
        made.status.success(),
        "{}",
        String::from_utf8_lossy(&made.stderr)
    );
    project
}

#[test]
fn cracks_voids_and_wall_heights_change_what_the_read_tools_report() {
    let tmp = std::env::temp_dir().join(format!("psxed-mcp-natural-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let project = new_project(&tmp);
    let mut c = Client::start(&["--project", project.to_str().unwrap()], &[], &repo_root());

    // ---- a crack: one triangle of a cell on its own level ----------------
    let before = c
        .ok("grid_walls", json!({"from": [2, 2], "to": [2, 2]}))
        .text;
    assert!(!before.contains("diagonal"), "{before}");
    let cracked = c
        .ok(
            "grid_crack",
            json!({"from": [2, 2], "triangle": "b", "delta": 768}),
        )
        .text;
    assert!(cracked.contains("1 derived diagonal wall"), "{cracked}");
    let sector = c.ok("grid_sector", json!({"cell": [2, 2]})).text;
    assert!(sector.contains("second triangle at"), "{sector}");
    assert!(
        sector.contains("derived wall diagonal NW-SE riser"),
        "{sector}"
    );
    let walls = c
        .ok("grid_walls", json!({"from": [2, 2], "to": [2, 2]}))
        .text;
    assert!(
        walls.contains("one-sided cliff wall"),
        "a three click step (60 runtime units, the character steps 40) is a cliff: {walls}"
    );
    // A step of one click is climbed, so it needs no wall record.
    c.ok(
        "grid_crack",
        json!({"from": [3, 2], "triangle": "a", "delta": 256}),
    );
    let walls = c
        .ok("grid_walls", json!({"from": [3, 2], "to": [3, 2]}))
        .text;
    assert!(walls.contains("diagonal NW-SE riser"), "{walls}");
    assert!(walls.contains("no wall record (step rule)"), "{walls}");
    // The cook accepts it.
    let cook = c.ok("cook_check", json!({})).text;
    assert!(!cook.to_lowercase().contains("error"), "{cook}");
    // Healing merges the halves back and the wall goes.
    c.ok("grid_crack", json!({"from": [2, 2], "heal": true}));
    let walls = c
        .ok("grid_walls", json!({"from": [2, 2], "to": [2, 2]}))
        .text;
    assert!(!walls.contains("diagonal"), "{walls}");

    // ---- a void: half a cell with nothing in it --------------------------
    let voided = c
        .ok("grid_void", json!({"from": [4, 4], "triangle": "a"}))
        .text;
    assert!(voided.contains("1 diagonal closure"), "{voided}");
    let sector = c.ok("grid_sector", json!({"cell": [4, 4]})).text;
    assert!(sector.contains("VOID triangle"), "{sector}");
    assert!(
        sector.contains("derived wall diagonal NW-SE closure"),
        "{sector}"
    );
    // The neighbour north of the void triangle's cardinal edge closes itself.
    let north = c.ok("grid_sector", json!({"cell": [4, 3]})).text;
    assert!(north.contains("derived wall south closure"), "{north}");
    c.ok("grid_void", json!({"from": [4, 4], "triangle": "none"}));
    let sector = c.ok("grid_sector", json!({"cell": [4, 4]})).text;
    assert!(!sector.contains("VOID"), "{sector}");

    // ---- wall heights ------------------------------------------------------
    c.ok(
        "grid_place_wall",
        json!({"from": [1, 4], "to": [1, 4], "edge": "north", "material": "Graybox / Cliff 64"}),
    );
    let set = c
        .ok(
            "grid_set_wall",
            json!({"cell": [1, 4], "edge": "north", "index": 0, "top_delta": [0, -512], "snap": "fine"}),
        )
        .text;
    assert!(set.contains("heights"), "{set}");
    let sector = c.ok("grid_sector", json!({"cell": [1, 4]})).text;
    assert!(sector.contains("authored wall north 0"), "{sector}");
    assert!(
        c.call(
            "grid_set_wall",
            json!({"cell": [1, 4], "edge": "north", "index": 0})
        )
        .is_err(),
        "nothing to set"
    );
}

#[test]
fn a_polygon_outline_gets_forty_five_degree_walls() {
    let tmp = std::env::temp_dir().join(format!("psxed-mcp-natural-poly-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let project = new_project(&tmp);
    let mut c = Client::start(&["--project", project.to_str().unwrap()], &[], &repo_root());
    let region = c
        .ok(
            "grid_polygon",
            json!({"kind": "region", "points": [[10.0, 0.0], [16.0, 0.0], [16.0, 6.0], [10.0, 6.0]]}),
        )
        .text;
    assert!(region.contains("36 sector"), "{region}");
    // A diamond: every edge a 45 degree line.
    let made = c
        .ok(
            "grid_polygon",
            json!({"points": [[20.0, 0.0], [26.0, 6.0], [20.0, 12.0], [14.0, 6.0]],
                   "floor_height": 0, "ceiling_height": 2048}),
        )
        .text;
    assert!(made.contains("convex corner(s) cut"), "{made}");
    let walls = c
        .ok("grid_walls", json!({"from": [14, 0], "to": [26, 12]}))
        .text;
    assert!(
        walls.contains("diagonal NW-SE closure") || walls.contains("diagonal NE-SW closure"),
        "{walls}"
    );
    let cook = c.ok("cook_check", json!({})).text;
    assert!(!cook.to_lowercase().contains("error"), "{cook}");
    // Corridors, and chamfer on its own.
    let corridor = c
        .ok(
            "grid_polygon",
            json!({"kind": "corridor", "points": [[3.0, 8.0], [9.0, 8.0], [9.0, 14.0]], "width": 2.0,
                   "floor_height": 256}),
        )
        .text;
    assert!(corridor.contains("sector(s) at 256"), "{corridor}");
    let restored = c
        .ok(
            "grid_chamfer",
            json!({"from": [14, 0], "to": [26, 12], "mode": "restore"}),
        )
        .text;
    assert!(restored.contains("brought back"), "{restored}");
}

#[test]
fn break_rough_and_paint_rules_work_through_the_tools_and_reroll_is_deterministic() {
    let tmp = std::env::temp_dir().join(format!("psxed-mcp-natural-break-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let project = new_project(&tmp);
    let mut c = Client::start(&["--project", project.to_str().unwrap()], &[], &repo_root());
    // A plateau two clicks up inside a bigger floor.
    c.ok(
        "grid_add_surface",
        json!({"kind": "floor", "from": [0, 0], "to": [13, 13], "height": 0}),
    );
    c.ok(
        "grid_add_surface",
        json!({"kind": "ceiling", "from": [0, 0], "to": [13, 13], "height": 3072}),
    );
    c.ok(
        "grid_raise",
        json!({"from": [4, 4], "to": [9, 9], "amount": 512, "follow_ceiling": true}),
    );
    let heights = |c: &mut Client| {
        c.ok("grid_heights", json!({"from": [2, 2], "to": [11, 11]}))
            .text
    };
    let flat = heights(&mut c);

    // A dry run reports and changes nothing.
    let dry = c
        .ok(
            "grid_break",
            json!({"from": [4, 4], "to": [9, 9], "intensity": 90, "seed": 3, "dry_run": true,
                   "targets": ["walls", "floor", "ceiling"]}),
        )
        .text;
    assert!(
        dry.contains("break seed 3") && dry.contains("dry run"),
        "{dry}"
    );
    assert_eq!(heights(&mut c), flat);

    let broken = c
        .ok(
            "grid_break",
            json!({"from": [4, 4], "to": [9, 9], "intensity": 90, "seed": 3,
                   "targets": ["walls", "floor", "ceiling"]}),
        )
        .text;
    assert!(broken.contains("break seed 3 intensity 90"), "{broken}");
    assert!(broken.contains("walkability:"), "{broken}");
    assert!(broken.contains("collision"), "{broken}");
    let first = heights(&mut c);
    assert_ne!(first, flat, "the break changed the heights");

    // The same seed again gives the same document; another seed another.
    c.ok(
        "grid_break",
        json!({"from": [4, 4], "to": [9, 9], "intensity": 90, "seed": 3, "reroll": true,
               "targets": ["walls", "floor", "ceiling"]}),
    );
    assert_eq!(heights(&mut c), first, "same seed, same result");
    let next = c
        .ok(
            "grid_break",
            json!({"from": [4, 4], "to": [9, 9], "intensity": 90, "reroll": true,
                   "targets": ["walls", "floor", "ceiling"]}),
        )
        .text;
    assert!(
        next.contains("break seed 4") && next.contains("re-rolled"),
        "{next}"
    );
    let after = c.ok("grid_walkability", json!({})).text;
    assert!(
        after.contains("0 too steep") && after.contains("0 step(s) taller"),
        "keep_walkable held the mask: {after}"
    );
    // Restoring the checkpoint puts the plateau back exactly.
    c.ok("restore", json!({"name": "before-break"}));
    assert_eq!(heights(&mut c), flat);

    // Rough: levels over a region and the room settings.
    let rough = c
        .ok(
            "grid_rough",
            json!({"brush": {"centre": [6, 6], "radius": 2}, "level": 2, "seed": 5, "tint": 20}),
        )
        .text;
    assert!(rough.contains("rough level 2"), "{rough}");
    assert!(rough.contains("tint 20%"), "{rough}");
    let budget = c.ok("grid_budget", json!({})).text;
    assert!(
        !budget.contains("rough jitter"),
        "default amplitudes are within the cap: {budget}"
    );
    let big = c.ok("grid_rough", json!({"amp": [20, 40, 80]})).text;
    assert!(big.contains("[20, 40, 80]"), "{big}");
    let budget = c.ok("grid_budget", json!({})).text;
    assert!(
        budget.contains("rough jitter"),
        "an over-large jitter warns: {budget}"
    );
    c.ok("grid_rough", json!({"amp": [13, 26, 51]}));
    let cook = c.ok("cook_check", json!({})).text;
    assert!(!cook.to_lowercase().contains("error"), "{cook}");

    // Paint rules by height, reapplied to the plateau.
    let rules = c
        .ok(
            "grid_paint_rules",
            json!({"floor_by_height": {"at": [256], "materials": ["Graybox / Cliff 64", "Graybox / Original 64"]},
                   "from": [2, 2], "to": [11, 11]}),
        )
        .text;
    assert!(rules.contains("floor(s) painted"), "{rules}");
    let low = c.ok("grid_sector", json!({"cell": [3, 3]})).text;
    let high = c.ok("grid_sector", json!({"cell": [6, 6]})).text;
    assert!(low.contains("Cliff 64"), "{low}");
    assert!(high.contains("Original 64"), "{high}");
}
