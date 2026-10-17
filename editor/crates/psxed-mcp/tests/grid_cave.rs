//! The proof of the natural terrain block: a small cave built in a fresh New
//! Grid Project using only MCP tools, over the stdio protocol.
//!
//! An irregular entrance hall with a ramp up to a ledge and an octagonal
//! pillar, a corridor and a stair up into a second chamber; every outline is
//! a polygon whose staircase corners are cut into 45 degree lines, then the
//! whole is broken (seeded), roughened, painted by height and lit. The
//! geometry stages always run. Screenshots (`PSXED_MCP_VERIFY=shots`) and the
//! playtest (`PSXED_MCP_VERIFY=1`, with `PSOXIDE_GUEST_STAGE_ROOT` set to a
//! private directory) need the renderer binary; `PSXED_MCP_PNG_DIR` names where
//! the pictures go.

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

fn keep(png: &[u8], name: &str) {
    let Some(dir) = std::env::var_os("PSXED_MCP_PNG_DIR") else {
        return;
    };
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir).expect("create the picture directory");
    let temp = dir.join(format!(".{name}.tmp"));
    std::fs::write(&temp, png).expect("write the picture");
    std::fs::rename(&temp, dir.join(format!("{name}.png"))).expect("move the picture");
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
    let project = projects.join("cave");
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

const GROUND: &str = "Graybox / Ground 64";
const CLIFF: &str = "Graybox / Cliff 64";
const HALL: [[f64; 2]; 8] = [
    [2.0, 10.0],
    [8.0, 6.0],
    [15.0, 5.0],
    [20.0, 9.0],
    [21.0, 15.0],
    [16.0, 19.0],
    [9.0, 19.0],
    [3.0, 15.0],
];

#[test]
fn a_cave_is_built_with_the_natural_terrain_tools_and_played() {
    let repo = repo_root();
    let tmp = std::env::temp_dir().join(format!("psxed-mcp-cave-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let project = new_project(&tmp);
    let frontend = repo.join("target/release/frontend");
    let scratch = tmp.join("scratch");
    let mut args = vec![
        "--project".to_string(),
        project.to_string_lossy().into_owned(),
        "--scratch".to_string(),
        scratch.to_string_lossy().into_owned(),
    ];
    if frontend.is_file() {
        args.push("--frontend".to_string());
        args.push(frontend.to_string_lossy().into_owned());
    }
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut c = Client::start(&arg_refs, &[], &repo);
    assert!(
        c.instructions.contains("NATURAL TERRAIN") && c.instructions.contains("grid_break"),
        "{}",
        c.instructions
    );

    // ---- the starter room goes; outlines come from polygons ---------------
    c.ok(
        "grid_remove_surface",
        json!({"kind": "floor", "from": [0, 0], "to": [5, 5]}),
    );
    c.ok(
        "grid_remove_surface",
        json!({"kind": "ceiling", "from": [0, 0], "to": [5, 5]}),
    );
    c.ok("grid_set_room", json!({"wall_material": CLIFF}));
    let hall = c
        .ok(
            "grid_polygon",
            json!({"points": HALL, "floor_height": 0, "ceiling_height": 2560,
                   "material": GROUND, "ceiling_material": CLIFF}),
        )
        .text;
    assert!(
        hall.contains("198 floor(s) and 198 ceiling(s) made"),
        "{hall}"
    );
    assert!(hall.contains("23 convex corner(s) cut"), "{hall}");
    c.ok(
        "grid_polygon",
        json!({"kind": "corridor", "points": [[20.0, 12.0], [27.0, 12.0], [27.0, 18.0]], "width": 2.0,
               "floor_height": 0, "ceiling_height": 2560, "material": GROUND, "ceiling_material": CLIFF}),
    );
    c.ok(
        "grid_polygon",
        json!({"points": [[24.0, 18.0], [31.0, 17.0], [33.0, 22.0], [29.0, 26.0], [24.0, 24.0]],
               "floor_height": 1024, "ceiling_height": 3584, "material": GROUND, "ceiling_material": CLIFF}),
    );
    // The outline is diagonal walls, not a staircase.
    let walls = c
        .ok("grid_walls", json!({"from": [2, 5], "to": [21, 19]}))
        .text;
    assert!(walls.contains("diagonal NW-SE closure"), "{walls}");
    assert!(walls.contains("diagonal NE-SW closure"), "{walls}");

    // ---- the floor profile: a stair, a ledge with a ramp ----------------------
    let stairs = c
        .ok(
            "grid_stairs",
            json!({"path": [[26, 14], [26, 18]], "width": 2, "rise": 1024, "riser": "click"}),
        )
        .text;
    assert!(stairs.contains("4 step(s)"), "{stairs}");
    c.ok(
        "grid_raise",
        json!({"from": [9, 8], "to": [13, 10], "amount": 768, "follow_ceiling": false}),
    );
    c.ok(
        "grid_ramp",
        json!({"from": [9, 11], "to": [13, 12], "keep": "south", "rise": 768, "walkable": true}),
    );

    // ---- an octagonal pillar: voids and cut corners ---------------------------
    for cell in [[15, 13], [14, 13], [16, 13], [15, 12], [15, 14]] {
        c.ok("grid_void", json!({"from": cell, "triangle": "both"}));
    }
    for (cell, split, tri) in [
        ([14, 12], "ne_sw", "b"),
        ([16, 12], "nw_se", "b"),
        ([14, 14], "nw_se", "a"),
        ([16, 14], "ne_sw", "a"),
    ] {
        c.ok(
            "grid_set_split",
            json!({"kind": "floor", "from": cell, "split": split}),
        );
        c.ok(
            "grid_set_split",
            json!({"kind": "ceiling", "from": cell, "split": split}),
        );
        c.ok("grid_void", json!({"from": cell, "triangle": tri}));
    }
    // The pillar closes itself: each side of the hole has a wall.
    let sector = c.ok("grid_sector", json!({"cell": [14, 12]})).text;
    assert!(sector.contains("VOID triangle"), "{sector}");
    assert!(sector.contains("closure"), "{sector}");

    // ---- the player and the light ------------------------------------------
    c.ok(
        "move_node",
        json!({"node": "Aletha (Player)", "position": [13440, 64, 21120]}),
    );
    c.ok(
        "move_node",
        json!({"node": "Starter Light", "position": [14000, 2000, 15200]}),
    );
    c.ok("add_light", json!({"name": "Chamber lamp", "position": [37000, 2600, 28000], "radius": 9000, "color": [255, 214, 170], "intensity": 1.2}));
    let walk = c.ok("grid_walkability", json!({})).text;
    assert!(walk.contains("0 too steep"), "{walk}");
    assert!(walk.contains("0 unreachable"), "{walk}");
    assert!(walk.contains("Reachability ran"), "{walk}");
    c.ok("checkpoint", json!({"name": "shaped"}));

    // ---- break: seeded detail, held to the walkability mask ------------------
    let region = json!({"polygon": HALL, "targets": ["walls", "floor", "ceiling"]});
    let call = |c: &mut Client, extra: serde_json::Value| {
        let mut args = region.clone();
        for (k, v) in extra.as_object().unwrap() {
            args[k] = v.clone();
        }
        c.ok("grid_break", args).text
    };
    let broken = call(&mut c, json!({"intensity": 40, "seed": 5}));
    assert!(broken.contains("break seed 5 intensity 40"), "{broken}");
    assert!(
        broken.contains("walkability: 280 standable, 0 unreachable"),
        "{broken}"
    );
    let map = c
        .ok("grid_heights", json!({"from": [7, 6], "to": [20, 18]}))
        .text;
    // Re-rolling the same seed repeats the level exactly; the next seed does not.
    call(&mut c, json!({"intensity": 40, "seed": 5, "reroll": true}));
    assert_eq!(
        c.ok("grid_heights", json!({"from": [7, 6], "to": [20, 18]}))
            .text,
        map
    );
    let again = call(&mut c, json!({"intensity": 40, "reroll": true}));
    assert!(
        again.contains("break seed 6") && again.contains("re-rolled"),
        "{again}"
    );
    c.ok("restore", json!({"name": "before-break"}));
    call(&mut c, json!({"intensity": 40, "seed": 5}));
    assert_eq!(
        c.ok("grid_heights", json!({"from": [7, 6], "to": [20, 18]}))
            .text,
        map
    );
    // The mask held: nothing steeper, no step the character cannot climb
    // beyond the ledge's own, no cell cut off.
    let walk = c.ok("grid_walkability", json!({})).text;
    assert!(
        walk.contains("0 too steep") && walk.contains("0 unreachable"),
        "{walk}"
    );

    // ---- rough rock and paint rules --------------------------------------------
    let rough = c
        .ok(
            "grid_rough",
            json!({"polygon": [[1.0, 9.0], [8.0, 5.0], [15.0, 4.0], [21.0, 9.0], [22.0, 15.0], [16.0, 20.0], [9.0, 20.0], [2.0, 16.0]],
                   "level": 2, "seed": 3, "tint": 18}),
        )
        .text;
    assert!(rough.contains("rough level 2"), "{rough}");
    c.ok(
        "grid_paint_rules",
        json!({"floor_by_height": {"at": [512], "materials": [GROUND, CLIFF]},
               "from": [3, 6], "to": [32, 25]}),
    );
    let budget = c.ok("grid_budget", json!({})).text;
    assert!(!budget.to_lowercase().contains("error"), "{budget}");
    let cook = c.ok("cook_check", json!({})).text;
    assert!(cook.starts_with("cook: OK"), "{cook}");
    // Rough costs no primitive: the cook's counts are the unjittered level's.
    assert!(cook.contains("faces"), "{cook}");

    let verify = std::env::var("PSXED_MCP_VERIFY").ok();
    if verify.is_some() {
        assert!(
            frontend.is_file(),
            "PSXED_MCP_VERIFY needs {}",
            frontend.display()
        );
        let shots = c.ok(
            "screenshot",
            json!({"shots": [
                {"label": "the hall from the south west", "position": [9000, 2600, 21500], "look_at": [15000, 0, 15500]},
                {"label": "the ledge and its ramp", "position": [19500, 3300, 17800], "look_at": [14700, 300, 11800]},
                {"label": "the pillar and the wall behind it", "position": [12000, 1500, 19800], "look_at": [19000, 600, 16000]},
                {"label": "the upper chamber", "position": [28500, 4200, 23500], "look_at": [38000, 1000, 28500]}
            ]}),
        );
        assert_eq!(shots.images.len(), 4);
        for (i, name) in ["cave-hall", "cave-ledge", "cave-pillar", "cave-chamber"]
            .iter()
            .enumerate()
        {
            keep(&shots.images[i], name);
        }
    }
    if verify.as_deref() == Some("1") {
        // North from the spawn: up the ramp, onto the ledge.
        let run = c.ok(
            "playtest",
            json!({
                "route": [{"polls": 300, "dir": "forward"}],
                "warmup_polls": 450,
                "frames": [0, 150, 299],
                "track_player": true
            }),
        );
        println!("{}", run.text);
        assert!(!run.text.contains("stalled"), "{}", run.text);
        for (i, png) in run.images.iter().enumerate() {
            keep(png, &format!("cave-play-{i}"));
        }
        let path: Vec<[i32; 3]> = run
            .text
            .lines()
            .find(|l| l.starts_with("path, route poll"))
            .expect("a path line")
            .split_whitespace()
            .filter_map(|tok| tok.split_once(":[").map(|(_, v)| v.trim_end_matches(']')))
            .filter_map(|v| {
                let n: Vec<i32> = v.split(',').filter_map(|x| x.parse().ok()).collect();
                (n.len() == 3).then(|| [n[0], n[1], n[2]])
            })
            .collect();
        assert!(path.len() > 8, "{path:?}");
        assert!(
            path.iter().all(|p| p[1] > -100),
            "the player never fell: {path:?}"
        );
        let top = path.iter().map(|p| p[1]).max().unwrap();
        // The ledge is three clicks (768 grid units, 960 editor units) up.
        assert!(top >= 900, "the player climbed onto the ledge: {path:?}");
        let end = *path.last().unwrap();
        assert!(
            end[2] < 13000,
            "the player walked the length of the hall: {end:?}"
        );
    }

    let saved = c.ok("save", json!({})).text;
    assert!(saved.contains("saved"), "{saved}");
    let doc = psxed_project::ProjectDocument::load_from_path(project.join("project.ron"))
        .expect("the saved project loads");
    let (_, report) = psxed_project::playtest::build_package(&doc, &project);
    assert!(
        report.is_ok(),
        "the saved project cooks: {:?}",
        report.error_messages()
    );
    let _ = std::fs::remove_dir_all(&tmp);
}
