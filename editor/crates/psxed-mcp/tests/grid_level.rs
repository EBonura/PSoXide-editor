//! Builds a small grid level end to end through the MCP, over the stdio
//! protocol, and plays it.
//!
//! Everything here goes through `tools/call` on a real `psxed-mcp` process, the
//! way an agent's host does: the test never touches a `ProjectDocument`
//! except to read the saved file back and cook it at the end.
//!
//! The level: an entry hall with a pit crossed by a plane bridge, a second
//! room one storey-ish higher joined by a ghosted wall opening and a plane
//! ramp, a banded wall in three materials, a FIT 2x2 floor patch, two lights
//! and the player spawn.
//!
//! The geometry stages always run. Screenshots and the playtest need the
//! renderer binary and a guest toolchain, so they are opt in:
//! `PSXED_MCP_VERIFY=shots` renders the three screenshots, `PSXED_MCP_VERIFY=1`
//! also builds a disc and plays the route (set `PSOXIDE_GUEST_STAGE_ROOT` to a
//! private directory). `PSXED_MCP_PNG_DIR` names where the pictures go.

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

/// Write `png` as `<dir>/mcp-<nn>-<name>.png` through a temp name, when a
/// picture directory was asked for.
fn keep(png: &[u8], nn: u32, name: &str) {
    let Some(dir) = std::env::var_os("PSXED_MCP_PNG_DIR") else {
        return;
    };
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir).expect("create the picture directory");
    let temp = dir.join(format!(".mcp-{nn:02}-{name}.tmp"));
    std::fs::write(&temp, png).expect("write the picture");
    std::fs::rename(&temp, dir.join(format!("mcp-{nn:02}-{name}.png"))).expect("move the picture");
}

fn is_png(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x89, b'P', b'N', b'G'])
}

/// A projects directory beside which a New Grid Project resolves its
/// `../default/assets` style paths: the repo's two base projects, linked.
fn projects_dir(tmp: &Path) -> PathBuf {
    let projects = tmp.join("projects");
    std::fs::create_dir_all(&projects).expect("create projects");
    for name in ["default", "graybox-reach"] {
        std::os::unix::fs::symlink(
            repo_root().join("editor/projects").join(name),
            projects.join(name),
        )
        .expect("link a base project");
    }
    projects
}

#[test]
fn an_agent_builds_a_grid_level_through_the_mcp_and_plays_it() {
    let repo = repo_root();
    let tmp = std::env::temp_dir().join(format!("psxed-mcp-level-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let projects = projects_dir(&tmp);
    let project = projects.join("mcp-proof");

    // An empty New Grid Project, made the way File > New Grid Project makes it.
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
    let original = std::fs::read_to_string(project.join("project.ron")).unwrap();

    let frontend = repo.join("target/release/frontend");
    let frontend_arg = frontend.to_string_lossy().into_owned();
    let scratch = tmp.join("scratch");
    let mut args = vec![
        "--project".to_string(),
        project.to_string_lossy().into_owned(),
        "--scratch".to_string(),
        scratch.to_string_lossy().into_owned(),
    ];
    if frontend.is_file() {
        args.push("--frontend".to_string());
        args.push(frontend_arg);
    }
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut c = Client::start(&arg_refs, &[], &repo);

    // ---- the server describes itself ------------------------------------
    assert!(
        c.instructions.contains("READ, ACT, VERIFY") && c.instructions.contains("1024 grid units"),
        "{}",
        c.instructions
    );
    let tools: Vec<String> = c
        .tools()
        .iter()
        .filter_map(|t| t["name"].as_str().map(str::to_string))
        .collect();
    for expected in [
        "grid_rooms",
        "grid_heights",
        "grid_walls",
        "grid_budget",
        "grid_plan",
        "grid_add_room",
        "grid_add_surface",
        "grid_set_heights",
        "grid_place_wall",
        "grid_set_split",
        "grid_paint",
        "grid_tiling",
        "grid_material_tiling",
        "grid_band_cut",
        "grid_band_set",
        "grid_add_plane",
        "grid_wall_override",
        "checkpoint",
        "restore",
        "screenshot",
        "playtest",
        "save",
        "cook_check",
        "place_node",
        "add_light",
    ] {
        assert!(
            tools.iter().any(|t| t == expected),
            "missing tool {expected}: {tools:?}"
        );
    }

    // ---- READ -----------------------------------------------------------
    let rooms = c.ok("grid_rooms", json!({})).text;
    assert!(
        rooms.contains("room 0 \"Starter room\": 6x6 sectors"),
        "{rooms}"
    );
    assert!(
        rooms.contains("sector = 1024 grid units = 1280 editor units"),
        "{rooms}"
    );
    let materials = c.ok("materials", json!({"palette_only": true})).text;
    assert!(materials.contains("Graybox / Cliff 64"), "{materials}");
    c.ok("checkpoint", json!({"name": "empty"}));

    // ---- ACT: the entry hall, a pit and a bridge over it ----------------
    // The starter hall is 6x6 sectors, floor at 0, ceiling at 2048. Cut a pit
    // 3 sectors wide and 2 deep across the middle, with the ceiling left on.
    let pit = c
        .ok(
            "grid_remove_surface",
            json!({"room": 0, "kind": "floor", "from": [2, 1], "to": [4, 2]}),
        )
        .text;
    assert!(pit.contains("removed 6 floor"), "{pit}");
    let map = c.ok("grid_heights", json!({})).text;
    let row1 = map
        .lines()
        .find(|l| l.trim_start().starts_with("1 "))
        .expect("row z=1");
    // Columns x 0..5: floor, floor, pit, pit, pit, floor.
    let cells: Vec<&str> = row1.split_whitespace().skip(1).collect();
    assert_eq!(
        &cells[..6],
        ["0", "0", ".", ".", ".", "0"],
        "the pit shows as dots: {map}"
    );

    // A plane bridge, flat at floor height, running north-south over the pit.
    let bridge = c
        .ok(
            "grid_add_plane",
            json!({
                "room": 0, "from": [3712, 0, 3072], "to": [3712, 0, 1024], "width": 1536,
                "material": "DP City / Bright Green Signal", "collision": "walkable", "double_sided": true
            }),
        )
        .text;
    assert!(
        bridge.contains("platform"),
        "a walkable flat plane cooks to a platform: {bridge}"
    );

    // ---- ACT: a second room at another height, joined by an opening ------
    let gallery = c
        .ok(
            "grid_add_room",
            json!({"name": "Gallery", "origin": [6, 0], "y_offset": 512}),
        )
        .text;
    assert!(gallery.contains("added room 1"), "{gallery}");
    c.ok("grid_add_surface", json!({"room": 1, "kind": "floor", "from": [6, 0], "to": [9, 5], "height": 0, "material": "Graybox / Ground 64"}));
    c.ok("grid_add_surface", json!({"room": 1, "kind": "ceiling", "from": [6, 0], "to": [9, 5], "height": 2048, "material": "Graybox / Cliff 64"}));
    c.ok("grid_set_room", json!({"room": 1, "wall_material": "DP City / Bright Blue Signal", "ambient": [112, 112, 124]}));
    let heights = c.ok("grid_heights", json!({})).text;
    assert!(
        heights.contains("0.50") || heights.contains(" 2"),
        "the gallery floor sits two clicks up: {heights}"
    );

    // The wall between the rooms is two closure walls. Make a one-sector
    // opening in it: hide both faces and make them passable.
    for (room, cell, edge) in [(0, [5, 0], "east"), (1, [6, 0], "west")] {
        let r = c
            .ok("grid_wall_override", json!({"room": room, "cell": cell, "edge": edge, "kind": "closure", "hidden": true, "solid": false}))
            .text;
        assert!(r.contains("hidden") && r.contains("passable"), "{r}");
    }
    let walls = c
        .ok(
            "grid_walls",
            json!({"room": 1, "from": [6, 0], "to": [6, 0], "edge": "west"}),
        )
        .text;
    assert!(
        walls.contains("hidden") && walls.contains("ghost"),
        "{walls}"
    );

    // A plane ramp from the hall floor up to the gallery floor, through it.
    let ramp = c
        .ok(
            "grid_add_plane",
            json!({
                "room": 0, "from": [4096, 0, 512], "to": [6144, 512, 512], "width": 1024,
                "material": "DP City / Dense Teal Light Bay", "collision": "walkable", "double_sided": true
            }),
        )
        .text;
    assert!(ramp.contains("platform"), "{ramp}");
    let planes = c.ok("grid_planes", json!({})).text;
    assert!(
        planes.contains("plane 0") && planes.contains("plane 1"),
        "{planes}"
    );

    // ---- ACT: a banded wall in three materials ---------------------------
    // The hall's north wall is one run of six closure faces. Two cuts make
    // three bands; each band gets a material of its own.
    let north = json!({"room": 0, "cell": [2, 0], "edge": "north"});
    let mut cut = north.clone();
    cut["anchor"] = json!("bottom");
    cut["value"] = json!(512);
    cut["scope"] = json!("room");
    c.ok("grid_band_cut", cut.clone());
    cut["anchor"] = json!("top");
    let cut_result = c.ok("grid_band_cut", cut).text;
    assert!(
        cut_result.contains("3 band(s)") || cut_result.contains("3 bands"),
        "{cut_result}"
    );
    for (band, material) in [
        (0, "DP City / Structural Beam"),
        (1, "Graybox / Original 64"),
        (2, "DP City / Dense Teal Light Bay"),
    ] {
        let mut req = north.clone();
        req["band"] = json!(band);
        req["material"] = json!(material);
        req["scope"] = json!("room");
        c.ok("grid_band_set", req);
    }
    let bands = c.ok("grid_bands", north.clone()).text;
    for material in ["Structural Beam", "Original 64", "Dense Teal Light Bay"] {
        assert!(bands.contains(material), "{bands}");
    }

    // ---- ACT: a FIT 2x2 floor patch --------------------------------------
    c.ok("grid_paint", json!({"room": 0, "from": [0, 4], "to": [1, 5], "faces": ["floor"], "material": "Graybox / Cliff 64"}));
    let fit = c
        .ok("grid_tiling", json!({"room": 0, "from": [0, 4], "to": [1, 5], "faces": ["floor"], "mode": "fit", "repeats": [2.0, 2.0]}))
        .text;
    assert!(fit.contains("2 x 2 on this face"), "{fit}");

    // ---- ACT: lights and the player ---------------------------------------
    c.ok("delete_node", json!({"node": "Starter Light"}));
    c.ok("add_light", json!({"name": "Hall lamp", "position": [4480, 1900, 4480], "radius": 6400, "color": [255, 226, 190], "intensity": 1.3}));
    c.ok("add_light", json!({"name": "Gallery lamp", "position": [9600, 2300, 1100], "radius": 6400, "color": [190, 214, 255], "intensity": 1.2}));
    let lights = c.ok("lights", json!({})).text;
    assert!(lights.contains("2 point light(s)"), "{lights}");
    // The player starts south of the pit, on the bridge's line.
    let floor = c.ok("grid_height_at", json!({"point": [4480, 5760]})).text;
    assert!(floor.contains("floor 0 editor units"), "{floor}");
    c.ok(
        "move_node",
        json!({"node": "Aletha (Player)", "position": [4480, 64, 5760]}),
    );

    // ---- a mistake, and a checkpoint takes it back ------------------------
    c.ok("checkpoint", json!({"name": "built"}));
    let before = c.ok("grid_heights", json!({})).text;
    c.ok(
        "grid_set_heights",
        json!({"room": 0, "kind": "floor", "from": [0, 3], "to": [5, 5], "height": 1024}),
    );
    let wrong = c.ok("grid_heights", json!({})).text;
    assert_ne!(before, wrong);
    c.ok("restore", json!({"name": "built"}));
    assert_eq!(
        c.ok("grid_heights", json!({})).text,
        before,
        "restore brought the world back"
    );
    assert!(c.call("restore", json!({"name": "no such"})).is_err());

    // ---- VALIDATE ----------------------------------------------------------
    let budget = c.ok("grid_budget", json!({})).text;
    assert!(budget.contains("drawable faces"), "{budget}");
    assert!(!budget.contains("error:"), "no validation errors: {budget}");
    let cook = c.ok("cook_check", json!({})).text;
    assert!(
        cook.contains("cook: OK"),
        "the staged project cooks: {cook}"
    );

    let plan = c.ok("grid_plan", json!({"pixels_per_sector": 40}));
    assert!(is_png(&plan.images[0]));
    assert!(
        plan.text.contains("room outlines are numbered"),
        "{}",
        plan.text
    );
    assert!(plan.text.contains("player"), "{}", plan.text);
    keep(&plan.images[0], 1, "plan-of-the-level");

    // Nothing is on disk yet: the staged edits live in the server.
    assert_eq!(
        std::fs::read_to_string(project.join("project.ron")).unwrap(),
        original
    );

    let verify = std::env::var("PSXED_MCP_VERIFY").ok();
    if verify.is_some() {
        assert!(
            frontend.is_file(),
            "PSXED_MCP_VERIFY needs {}",
            frontend.display()
        );
        // ---- VERIFY: three poses, in editor units (a sector is 1280) -------
        let shots = c.ok(
            "screenshot",
            json!({"shots": [
                {"label": "overview from above the south west", "position": [-1500, 6800, 10400], "look_at": [6600, 0, 3000]},
                {"label": "the hall from the south west: pit, bridge and banded wall", "position": [900, 1500, 7300], "look_at": [4500, 100, 1900]},
                {"label": "the ramp and the opening into the gallery", "position": [2600, 1400, 3400], "look_at": [7600, 500, 700]}
            ]}),
        );
        assert_eq!(shots.images.len(), 3);
        for (i, name) in ["overview", "hall-pit-bridge-bands", "ramp-and-opening"]
            .iter()
            .enumerate()
        {
            assert!(is_png(&shots.images[i]));
            keep(&shots.images[i], 2 + i as u32, name);
        }
        assert!(
            shots.text.contains("staged edits, not saved"),
            "{}",
            shots.text
        );

        // The older single-shot arguments still work, and an eye pose can be
        // given as a heading instead of a look-at point.
        let orbit = c.ok(
            "screenshot",
            json!({"target": [4480, 0, 3840], "distance": 6000, "pitch": 3400}),
        );
        assert_eq!(orbit.images.len(), 1);
        let heading = c.ok(
            "screenshot",
            json!({"position": [4480, 2100, 7200], "yaw_deg": 0, "pitch_deg": -25, "label": "heading"}),
        );
        assert!(
            heading.text.contains("heading: eye [4480, 2100, 7200]"),
            "{}",
            heading.text
        );
        assert!(
            c.call("screenshot", json!({"look_at": [0, 0, 0]})).is_err(),
            "look_at needs an eye"
        );
    }
    if verify.as_deref() == Some("1") {
        // ---- VERIFY: walk north over the bridge, then east up the ramp ------
        // The default camera looks north, so forward is north and right is
        // east. 600 polls reach the far wall; 750 more climb into the gallery.
        let run = c.ok(
            "playtest",
            json!({
                "route": [
                    {"polls": 600, "dir": "forward"},
                    {"polls": 750, "dir": "right"}
                ],
                "warmup_polls": 450,
                "frames": [0, 75, 180, 630, 700, 1100],
                "track_player": true
            }),
        );
        println!("{}", run.text);
        assert!(run.text.contains("port1-polls"), "{}", run.text);
        assert!(!run.text.contains("stalled"), "{}", run.text);
        // Telemetry is the default for a route: cycles per frame, not just cadence.
        assert!(
            run.text.contains("frame cost from guest telemetry"),
            "{}",
            run.text
        );
        assert!(
            run.text.contains("frames over 1,128,960 cycles"),
            "{}",
            run.text
        );
        assert!(
            run.text.contains("world (room draw) cycles"),
            "{}",
            run.text
        );
        assert!(
            run.images.len() >= 5,
            "frames came back: {}",
            run.images.len()
        );
        let names = [
            "playtest-1-spawn-south-of-the-pit",
            "playtest-2-on-the-bridge",
            "playtest-3-across-at-the-north-wall",
            "playtest-4-climbing-the-ramp",
            "playtest-5-in-the-gallery",
            "playtest-6-at-the-gallery-east-wall",
            "playtest-7-final-frame",
        ];
        assert_eq!(
            run.images.len(),
            names.len(),
            "six frames and the final one"
        );
        for (i, png) in run.images.iter().enumerate() {
            assert!(is_png(png));
            keep(png, 5 + i as u32, names[i]);
        }

        // The path says it: the player stood south of the pit, passed over it
        // on the bridge without falling, and ended in the gallery.
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
        let start = path[0];
        let end = *path.last().unwrap();
        assert!(
            (start[0] - 4480).abs() < 200 && (start[2] - 5760).abs() < 300,
            "starts at the spawn: {start:?}"
        );
        // Over the pit: x within the pit's columns, z inside its rows (1280..3840).
        // Over the pit there is no floor, so standing at floor height there
        // is the bridge holding the player up.
        let over_pit: Vec<&[i32; 3]> = path
            .iter()
            .filter(|p| (2600..6300).contains(&p[0]) && (1400..3700).contains(&p[2]))
            .collect();
        assert!(!over_pit.is_empty(), "the path crosses the pit: {path:?}");
        assert!(
            over_pit.iter().all(|p| p[1] >= 0),
            "held up over the pit: {over_pit:?}"
        );
        assert!(
            path.iter().all(|p| p[1] > -300),
            "the player never fell: {path:?}"
        );
        // The gallery floor is 512 grid units up (640 editor), east of x 7680.
        assert!(end[0] > 8000, "ends in the gallery: {end:?}");
        assert!(
            (end[1] - 640).abs() < 200,
            "stands on the gallery floor: {end:?}"
        );
    }

    // ---- SAVE ---------------------------------------------------------------
    let saved = c.ok("save", json!({})).text;
    assert!(saved.contains("saved"), "{saved}");
    assert_ne!(
        std::fs::read_to_string(project.join("project.ron")).unwrap(),
        original
    );

    // The saved file is a project that loads and cooks, with the level in it.
    let doc = psxed_project::ProjectDocument::load_from_path(project.join("project.ron"))
        .expect("the saved project loads");
    let world = doc.active_scene().world.grid().expect("a grid world");
    assert_eq!(world.rooms.len(), 2);
    assert_eq!(world.rooms[0].planes.len(), 2);
    assert_eq!(world.rooms[1].y_offset, 512);
    let (_, report) = psxed_project::playtest::build_package(&doc, &project);
    assert!(
        report.is_ok(),
        "the saved project cooks: {:?}",
        report.error_messages()
    );

    let _ = std::fs::remove_dir_all(&tmp);
}
