//! The MCP pain points of the first agent-built test level, fixed and proven
//! over the stdio protocol on a fresh New Grid Project:
//!
//! 1. `grid_tiling` `fit_across` fits N x M repeats across a whole rectangle;
//! 2. `screenshot` poses are exact and can hide the player and the overlays;
//! 3. plane angles have a documented convention and `facing`/`lean_deg`/`normal`;
//! 4. a plane at floor height warns;
//! 5. (lights) preview and cook bake the same colours: see the cook's
//!    `light_tests`; here the lights are shown on blue-grey and on warm stone;
//! 6. the new project's palette holds six materials that read apart;
//! 7. the `materials` table speaks the grid mapping.
//!
//! The geometry always runs. Screenshots need the renderer binary and are
//! opt in: `PSXED_MCP_VERIFY=shots`. `PSXED_MCP_PNG_DIR` names where the
//! pictures go (written through a temp name, then moved).

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

fn keep(png: &[u8], nn: u32, name: &str) {
    let Some(dir) = std::env::var_os("PSXED_MCP_PNG_DIR") else {
        return;
    };
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir).expect("create the picture directory");
    let temp = dir.join(format!(".mcpfix-{nn:02}-{name}.tmp"));
    std::fs::write(&temp, png).expect("write the picture");
    std::fs::rename(&temp, dir.join(format!("mcpfix-{nn:02}-{name}.png"))).expect("move it");
}

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

fn start(tag: &str) -> (Client, PathBuf) {
    let repo = repo_root();
    let tmp = std::env::temp_dir().join(format!("psxed-mcp-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let projects = projects_dir(&tmp);
    let project = projects.join("mcpfix");
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
    let frontend = repo.join("target/release/frontend");
    let mut args = vec![
        "--project".to_string(),
        project.to_string_lossy().into_owned(),
        "--scratch".to_string(),
        tmp.join("scratch").to_string_lossy().into_owned(),
    ];
    if frontend.is_file() {
        args.push("--frontend".to_string());
        args.push(frontend.to_string_lossy().into_owned());
    }
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    (Client::start(&refs, &[], &repo), tmp)
}

#[test]
fn tiling_planes_palette_and_materials_speak_the_fixed_conventions() {
    let (mut c, tmp) = start("fixes");

    // ---- 6 and 7: the palette and the grid tile column ---------------------
    let table = c.ok("materials", json!({"palette_only": true})).text;
    for name in [
        "Graybox / Ground 64",
        "Graybox / Original 64",
        "Graybox / Cliff 64",
        "Graybox / Warm Stone 64",
        "Graybox / Dark Trim 64",
        "Graybox / Light Frieze 64",
        "Graybox / Wood 64",
        "Graybox / Metal 64",
        "Graybox / Moss 64",
    ] {
        assert!(table.contains(name), "{name} in the palette: {table}");
    }
    assert!(table.contains("grid tile (default mapping)"), "{table}");
    // A new world spends 64 texels per sector: one tile per sector.
    assert!(
        table.contains("a tile is 1 x 1 sectors = 1280 x 1280 editor units"),
        "{table}"
    );
    assert!(!table.contains("1024x1024"), "no brush era figure: {table}");
    // A material default changes its row.
    c.ok(
        "grid_material_tiling",
        json!({"material": "Graybox / Moss 64", "mode": "world", "density": [32, 32]}),
    );
    let table = c.ok("materials", json!({"palette_only": true})).text;
    let moss = table
        .lines()
        .find(|l| l.contains("Graybox / Moss 64"))
        .unwrap();
    assert!(
        moss.contains("a tile is 2 x 2 sectors = 2560 x 2560"),
        "{moss}"
    );

    // ---- 1: fit across a rectangle -----------------------------------------
    c.ok(
        "grid_paint",
        json!({"room": 0, "from": [0, 4], "to": [3, 5], "faces": ["floor"], "material": "Graybox / Wood 64"}),
    );
    let each = c
        .ok(
            "grid_tiling",
            json!({"room": 0, "from": [0, 4], "to": [3, 5], "faces": ["floor"], "mode": "fit", "repeats": [2.0, 2.0]}),
        )
        .text;
    assert!(each.contains("2 x 2 on this face"), "{each}");
    let across = c
        .ok(
            "grid_tiling",
            json!({"room": 0, "from": [0, 4], "to": [3, 5], "faces": ["floor"], "mode": "fit_across", "repeats": [2.0, 2.0]}),
        )
        .text;
    assert!(
        across.contains(
            "floor patch of 8 face(s), 4 x 2 sectors: 2 x 2 tiles across the whole patch"
        ),
        "{across}"
    );
    assert!(
        across.contains("0.5 x 1 on this face, part of 2 x 2 across its patch"),
        "each cell shows its share: {across}"
    );
    assert!(!across.contains("warning"), "within the window: {across}");
    // Any later tiling call on one face reads its share back.
    let one = c
        .ok(
            "grid_tiling",
            json!({"room": 0, "from": [1, 4], "to": [1, 4], "faces": ["floor"], "offset": [0, 0]}),
        )
        .text;
    assert!(one.contains("part of 2 x 2 across its patch"), "{one}");
    // 16 tiles across 4 cells is 4 a cell: still one primitive; 32 is 8 a
    // cell, so the cook cuts and the reply says so.
    let more = c
        .ok(
            "grid_tiling",
            json!({"room": 0, "from": [0, 4], "to": [3, 5], "faces": ["floor"], "mode": "fit_across", "repeats": [16.0, 8.0]}),
        )
        .text;
    assert!(!more.contains("warning"), "{more}");
    let cut = c
        .ok(
            "grid_tiling",
            json!({"room": 0, "from": [0, 4], "to": [3, 5], "faces": ["floor"], "mode": "fit_across", "repeats": [32.0, 8.0]}),
        )
        .text;
    assert!(
        cut.contains("warning: the cook cuts 8 of these faces"),
        "{cut}"
    );
    // Walls are not a patch; bad repeats are refused; a plain fit goes back.
    assert!(c
        .call(
            "grid_tiling",
            json!({"room": 0, "from": [0, 4], "to": [3, 5], "faces": ["floor"], "mode": "fit_across", "repeats": [0.0, 1.0]}),
        )
        .is_err());
    let walls_only = c.call(
        "grid_tiling",
        json!({"room": 0, "from": [0, 5], "to": [3, 5], "faces": ["walls"], "mode": "fit_across", "repeats": [2.0, 1.0]}),
    );
    assert!(walls_only.is_err(), "walls fit per run");
    let back = c
        .ok(
            "grid_tiling",
            json!({"room": 0, "from": [0, 4], "to": [3, 5], "faces": ["floor"], "mode": "fit", "repeats": [1.0, 1.0]}),
        )
        .text;
    assert!(
        back.contains("1 x 1 on this face") && !back.contains("patch"),
        "{back}"
    );
    assert!(c.ok("cook_check", json!({})).text.contains("cook: OK"));

    // ---- 3: plane angles ---------------------------------------------------
    let wall = c
        .ok(
            "grid_add_plane",
            json!({"room": 0, "center": [2560, 1024, 2560], "size": [1024, 1024], "facing": "east", "lean_deg": 15}),
        )
        .text;
    assert!(
        wall.contains("rotation [yaw, pitch, roll] [90, 15, 0]"),
        "{wall}"
    );
    assert!(
        wall.contains("faces east, top leans 15 degrees away from that side"),
        "{wall}"
    );
    let by_normal = c
        .ok(
            "grid_add_plane",
            json!({"room": 0, "center": [2560, 1024, 3584], "size": [1024, 1024], "normal": [0.966, 0.259, 0.0]}),
        )
        .text;
    assert!(by_normal.contains("[90, 15, 0]"), "{by_normal}");
    let flat = c
        .ok(
            "grid_set_plane",
            json!({"room": 0, "index": 1, "facing": "up"}),
        )
        .text;
    assert!(flat.contains("faces up (a floor)"), "{flat}");
    assert!(c
        .call(
            "grid_set_plane",
            json!({"room": 0, "index": 1, "facing": "east", "rotation": [0, 0, 0]}),
        )
        .is_err());
    assert!(c
        .call(
            "grid_add_plane",
            json!({"room": 0, "center": [2560, 1024, 2560], "lean_deg": 10}),
        )
        .is_err());

    // ---- 4: a plane at floor height z-fights -------------------------------
    let fight = c
        .ok(
            "grid_add_plane",
            json!({"room": 0, "center": [3072, 0, 1536], "size": [1024, 1024], "orientation": "floor", "double_sided": true}),
        )
        .text;
    assert!(
        fight.contains("warning: this plane is within 32 grid units of a floor"),
        "{fight}"
    );
    assert!(fight.contains("z-fight"), "{fight}");
    let lifted = c
        .ok(
            "grid_set_plane",
            json!({"room": 0, "index": 2, "center": [3072, 64, 1536]}),
        )
        .text;
    assert!(!lifted.contains("warning"), "{lifted}");

    // ---- the descriptions say what they must -------------------------------
    let tools = c.tools();
    let describe = |name: &str| -> String {
        tools
            .iter()
            .find(|t| t["name"] == name)
            .and_then(|t| t["description"].as_str())
            .unwrap_or_default()
            .to_string()
    };
    for (tool, words) in [
        (
            "grid_add_plane",
            ["facing", "lean_deg", "normal"].as_slice(),
        ),
        ("screenshot", ["hide_player", "clean", "EXACT"].as_slice()),
    ] {
        for word in words {
            assert!(describe(tool).contains(word), "{tool} mentions {word}");
        }
    }
    let schema = serde_json::to_string(
        tools
            .iter()
            .find(|t| t["name"] == "grid_add_plane")
            .expect("tool"),
    )
    .unwrap();
    assert!(
        schema.contains("top leans away") || schema.contains("AWAY"),
        "{schema}"
    );
    let _ = std::fs::remove_dir_all(tmp);
}

/// The rebuild of the earlier test level through the MCP alone: a hall, a
/// dais, a pit with a bridge, banded walls in distinct materials, a fit
/// across patch and warm lights; then pictures with exact poses and the
/// player and overlays hidden.
#[test]
fn the_test_level_is_rebuilt_with_the_fixed_tools() {
    let (mut c, tmp) = start("level");
    // The hall: the starter 6x6 plus two sectors east, 8 wide, floor and
    // ceiling in the new materials.
    c.ok(
        "grid_add_surface",
        json!({"room": 0, "kind": "floor", "from": [6, 0], "to": [7, 5], "height": 0}),
    );
    c.ok(
        "grid_add_surface",
        json!({"room": 0, "kind": "ceiling", "from": [6, 0], "to": [7, 5], "height": 2048}),
    );
    c.ok(
        "grid_paint",
        json!({"room": 0, "from": [0, 0], "to": [7, 5], "faces": ["floor"], "material": "Graybox / Warm Stone 64"}),
    );
    c.ok(
        "grid_paint",
        json!({"room": 0, "from": [0, 0], "to": [7, 5], "faces": ["ceiling"], "material": "Graybox / Dark Trim 64"}),
    );
    c.ok(
        "grid_set_room",
        json!({"room": 0, "wall_material": "Graybox / Warm Stone 64", "ambient": [104, 100, 96]}),
    );
    // A dais of two by two sectors, half a click shy of a metre, in light
    // frieze stone with dark risers.
    c.ok(
        "grid_set_heights",
        json!({"room": 0, "kind": "floor", "from": [5, 1], "to": [6, 2], "height": 512}),
    );
    c.ok(
        "grid_paint",
        json!({"room": 0, "from": [5, 1], "to": [6, 2], "faces": ["floor"], "material": "Graybox / Light Frieze 64"}),
    );
    let risers = c
        .ok(
            "grid_paint",
            json!({"room": 0, "from": [5, 1], "to": [6, 2], "faces": ["derived"], "material": "Graybox / Dark Trim 64"}),
        )
        .text;
    assert!(risers.contains("wall"), "{risers}");
    // A pit two sectors wide and long, crossed by a walkable plane bridge in
    // wood, lifted a fine step so it does not fight the floor ends.
    c.ok(
        "grid_remove_surface",
        json!({"room": 0, "kind": "floor", "from": [2, 3], "to": [3, 4]}),
    );
    let bridge = c
        .ok(
            "grid_add_plane",
            json!({
                "room": 0, "from": [3072, 64, 2560], "to": [3072, 64, 5632], "width": 1024,
                "material": "Graybox / Wood 64", "collision": "walkable", "double_sided": true
            }),
        )
        .text;
    assert!(bridge.contains("platform"), "{bridge}");
    assert!(!bridge.contains("warning"), "{bridge}");
    // Banded north wall: dark trim, stone, light frieze.
    let north = json!({"room": 0, "cell": [2, 0], "edge": "north"});
    let mut cut = north.clone();
    cut["anchor"] = json!("bottom");
    cut["value"] = json!(384);
    cut["scope"] = json!("room");
    c.ok("grid_band_cut", cut.clone());
    cut["anchor"] = json!("top");
    cut["value"] = json!(384);
    let cut_result = c.ok("grid_band_cut", cut).text;
    assert!(cut_result.contains("3 band"), "{cut_result}");
    for (band, material) in [
        (0, "Graybox / Dark Trim 64"),
        (1, "Graybox / Warm Stone 64"),
        (2, "Graybox / Light Frieze 64"),
    ] {
        let mut req = north.clone();
        req["band"] = json!(band);
        req["material"] = json!(material);
        req["scope"] = json!("room");
        c.ok("grid_band_set", req);
    }
    // A patch fitted 2 x 2 across 4 x 2 cells at the south end, in the grid
    // texture so the repeats can be counted in a picture.
    c.ok(
        "grid_paint",
        json!({"room": 0, "from": [4, 4], "to": [7, 5], "faces": ["floor"], "material": "Graybox / Original 64"}),
    );
    let patch = c
        .ok(
            "grid_tiling",
            json!({"room": 0, "from": [4, 4], "to": [7, 5], "faces": ["floor"], "mode": "fit_across", "repeats": [2.0, 2.0]}),
        )
        .text;
    assert!(
        patch.contains("2 x 2 tiles across the whole patch"),
        "{patch}"
    );
    // Warm lights, one over the dais, two along the hall.
    c.ok("delete_node", json!({"node": "Starter Light"}));
    for (name, x, z) in [
        ("Dais", 7680, 2560),
        ("West", 1280, 2200),
        ("South", 5000, 6400),
    ] {
        c.ok(
            "add_light",
            json!({"name": name, "position": [x, 1500, z], "radius": 4200, "color": [255, 170, 90], "intensity": 1.2}),
        );
    }
    c.ok(
        "move_node",
        json!({"node": "Aletha (Player)", "position": [4480, 64, 7000]}),
    );
    let budget = c.ok("grid_budget", json!({})).text;
    assert!(!budget.contains("error:"), "{budget}");
    assert!(c.ok("cook_check", json!({})).text.contains("cook: OK"));
    let plan = c.ok("grid_plan", json!({"pixels_per_sector": 40}));
    keep(&plan.images[0], 1, "plan");

    if std::env::var("PSXED_MCP_VERIFY").is_err() {
        let _ = std::fs::remove_dir_all(tmp);
        return;
    }
    assert!(
        repo_root().join("target/release/frontend").is_file(),
        "PSXED_MCP_VERIFY needs target/release/frontend"
    );
    // An eye pose is exact: the same view asked for as an orbit from far
    // away is the same picture, pixel for pixel.
    let eye = c.ok(
        "screenshot",
        json!({"position": [4480, 700, 7300], "look_at": [4480, 700, 1000], "clean": true}),
    );
    let orbit = c.ok(
        "screenshot",
        json!({"target": [4480, 700, 1000], "distance": 6300, "yaw": 0, "pitch": 0, "clean": true}),
    );
    assert!(
        eye.text.contains("exact eye [4480, 700, 7300]"),
        "{}",
        eye.text
    );
    assert!(
        eye.text.contains("hidden: actors, overlays"),
        "{}",
        eye.text
    );
    assert_eq!(
        eye.images[0], orbit.images[0],
        "an eye pose is not an orbit"
    );
    keep(&eye.images[0], 2, "hall-from-the-entrance-clean");
    // The player at the entrance is in the way until it is hidden.
    let shown = c.ok(
        "screenshot",
        json!({"position": [4480, 700, 7300], "look_at": [4480, 700, 1000]}),
    );
    assert_ne!(
        shown.images[0], eye.images[0],
        "the player draws unless hidden"
    );
    keep(
        &shown.images[0],
        3,
        "same-pose-with-the-player-and-overlays",
    );
    let hidden_player = c.ok(
        "screenshot",
        json!({"position": [4480, 700, 7300], "look_at": [4480, 700, 1000], "hide_player": true, "hide_overlays": true}),
    );
    assert!(hidden_player.text.contains("hidden: player, overlays"));
    assert_eq!(hidden_player.images[0], eye.images[0]);
    let shots = c.ok(
        "screenshot",
        json!({"clean": true, "shots": [
            {"label": "overview", "target": [5000, 0, 3800], "distance": 9500, "pitch": 3500, "yaw": 300},
            {"label": "dais from the hall", "position": [2400, 900, 3300], "look_at": [7000, 400, 2600]},
            {"label": "pit and bridge", "position": [900, 1200, 7200], "look_at": [3800, 0, 4300]},
            {"label": "fit across patch from above", "position": [7680, 8200, 8000], "look_at": [7680, 0, 6400]},
            {"label": "banded north wall", "position": [3200, 900, 3600], "look_at": [3200, 1000, 0]}
        ]}),
    );
    assert_eq!(shots.images.len(), 5);
    assert_eq!(
        shots.text.matches("hidden: actors, overlays").count(),
        5,
        "the outer clean is every shot's default: {}",
        shots.text
    );
    for (i, name) in [
        "overview",
        "dais",
        "pit-and-bridge",
        "fit-across-patch-2x2-over-4x2",
        "banded-north-wall",
    ]
    .iter()
    .enumerate()
    {
        keep(&shots.images[i], 4 + i as u32, name);
    }

    // The same patch the way `fit` did it: 2 x 2 on each of the eight cells.
    c.ok("checkpoint", json!({"name": "across"}));
    c.ok(
        "grid_tiling",
        json!({"room": 0, "from": [4, 4], "to": [7, 5], "faces": ["floor"], "mode": "fit", "repeats": [2.0, 2.0]}),
    );
    let per_face = c.ok(
        "screenshot",
        json!({"position": [7680, 8200, 8000], "look_at": [7680, 0, 6400], "clean": true}),
    );
    keep(
        &per_face.images[0],
        9,
        "same-patch-fit-per-face-eight-tilings",
    );
    c.ok("restore", json!({"name": "across"}));

    // Lights: the same pose with the room back in the blue-grey base
    // materials (before) and in the new ones (after).
    c.ok("checkpoint", json!({"name": "new-materials"}));
    c.ok(
        "grid_paint",
        json!({"room": 0, "from": [0, 0], "to": [7, 5], "faces": ["floor", "walls"], "material": "Graybox / Ground 64"}),
    );
    c.ok(
        "grid_set_room",
        json!({"room": 0, "wall_material": "Graybox / Ground 64"}),
    );
    let before = c.ok(
        "screenshot",
        json!({"position": [1200, 800, 6800], "look_at": [5200, 300, 1800], "clean": true}),
    );
    keep(&before.images[0], 10, "lights-on-blue-grey-before");
    c.ok("restore", json!({"name": "new-materials"}));
    let after = c.ok(
        "screenshot",
        json!({"position": [1200, 800, 6800], "look_at": [5200, 300, 1800], "clean": true}),
    );
    keep(&after.images[0], 11, "lights-on-the-new-palette-after");
    assert_ne!(before.images[0], after.images[0]);
    let _ = std::fs::remove_dir_all(tmp);
}
