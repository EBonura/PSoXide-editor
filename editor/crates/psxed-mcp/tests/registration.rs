//! The repository's `.mcp.json` still starts a working server.
//!
//! The registration runs `cargo run -p psxed-mcp -- --project <dir>`; this
//! test reads it, takes the server arguments after `--`, and starts the built
//! binary with them from the repository root, which is where a client would.

mod common;

use std::path::PathBuf;

use common::Client;
use serde_json::{json, Value};

#[test]
fn the_registered_server_starts_and_answers() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("repo root");
    let text = std::fs::read_to_string(repo.join(".mcp.json")).expect("read .mcp.json");
    let config: Value = serde_json::from_str(&text).expect(".mcp.json parses");
    let entry = &config["mcpServers"]["psoxide-editor"];
    assert_eq!(entry["command"], "cargo", "{entry}");
    let args: Vec<String> = entry["args"]
        .as_array()
        .expect("args")
        .iter()
        .filter_map(|a| a.as_str().map(str::to_string))
        .collect();
    assert!(args.iter().any(|a| a == "psxed-mcp"), "{args:?}");
    let server_args: Vec<&str> = args
        .iter()
        .skip_while(|a| *a != "--")
        .skip(1)
        .map(String::as_str)
        .collect();
    assert_eq!(server_args.first(), Some(&"--project"), "{args:?}");

    let mut client = Client::start(&server_args, &[], &repo);
    let status = client.ok("status", json!({})).text;
    assert!(status.contains("project:"), "{status}");
    assert!(client.ok("metrics", json!({})).text.contains("Grid world"));
    // The tool list carries the grid tools next to the older ones.
    let names: Vec<String> = client
        .tools()
        .iter()
        .filter_map(|t| t["name"].as_str().map(str::to_string))
        .collect();
    for expected in ["grid_rooms", "place_node", "playtest", "save"] {
        assert!(
            names.iter().any(|n| n == expected),
            "{expected} in {names:?}"
        );
    }
}
