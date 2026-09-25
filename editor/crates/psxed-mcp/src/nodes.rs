//! Scene-node (entity) tools.
//!
//! There are 28 `NodeKind` variants carrying everything from enemies to
//! trigger volumes, and hand-writing a schema for each would be a lot of code
//! that goes stale the first time a field is added. Instead these tools hand
//! the agent the RON itself: `NodeKind` round-trips through serde, so reading
//! a node, editing its text and writing it back covers every variant and
//! cannot drift from the format.
//!
//! This is the TrenchBroom MCP's `fgd_class`/`fgd_classes` idea, adapted: we
//! have no FGD, but we do have a project full of authored examples, and a real
//! enemy as it actually ships is better documentation than a field list.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use psxed_project::{NodeId, NodeKind, ProjectDocument, Scene};

use crate::resolve_scene;

/// Pretty-print settings matching how the project file stores nodes.
fn ron_config() -> ron::ser::PrettyConfig {
    ron::ser::PrettyConfig::new()
        .depth_limit(6)
        .struct_names(true)
}

/// Serialize one node's kind payload.
pub fn node_kind_to_ron(kind: &NodeKind) -> Result<String, String> {
    ron::ser::to_string_pretty(kind, ron_config())
        .map_err(|error| format!("serialize node kind: {error}"))
}

/// Parse a node kind from RON text.
pub fn node_kind_from_ron(text: &str) -> Result<NodeKind, String> {
    ron::from_str(text).map_err(|error| {
        format!("parse node kind: {error}. The text must be one NodeKind variant, e.g. PointLight(color: (255, 236, 208), intensity: 1.0, radius: 2.5)")
    })
}

/// Find a node by exact name, then by unique case-insensitive substring, then
/// by numeric id. Ambiguity lists the candidates rather than guessing.
pub fn find_node(scene: &Scene, needle: &str) -> Result<NodeId, String> {
    if let Some(node) = scene.nodes().iter().find(|node| node.name == needle) {
        return Ok(node.id);
    }
    if let Ok(raw) = needle.parse::<u64>() {
        if let Some(node) = scene.nodes().iter().find(|node| node.id.raw() == raw) {
            return Ok(node.id);
        }
    }
    let lowered = needle.to_ascii_lowercase();
    let hits: Vec<_> = scene
        .nodes()
        .iter()
        .filter(|node| node.name.to_ascii_lowercase().contains(&lowered))
        .collect();
    match hits.as_slice() {
        [only] => Ok(only.id),
        [] => Err(format!(
            "no node matches {needle:?}. Call entity_types to see what the scene holds."
        )),
        many => Err(format!(
            "{needle:?} is ambiguous between {} nodes: {}",
            many.len(),
            many.iter()
                .take(12)
                .map(|node| format!("{:?} (id {})", node.name, node.id.raw()))
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// Every node kind present in the scene, with counts and example names.
///
/// Deliberately reports what the project actually contains rather than every
/// variant the format allows: an example you can clone is more useful than a
/// name you would have to construct from scratch.
pub fn entity_types(project: &ProjectDocument, scene_index: Option<usize>) -> Result<String, String> {
    let index = resolve_scene(project, scene_index)?;
    let scene = &project.scenes[index];
    let mut kinds: BTreeMap<&'static str, (usize, Vec<String>)> = BTreeMap::new();
    for node in scene.nodes() {
        let entry = kinds.entry(node.kind.label()).or_default();
        entry.0 += 1;
        if entry.1.len() < 3 {
            entry.1.push(node.name.clone());
        }
    }
    let mut out = format!("# Node kinds in scene {index}\n\n");
    let _ = writeln!(out, "{} nodes across {} kinds.\n", scene.nodes().len(), kinds.len());
    for (label, (count, names)) in &kinds {
        let _ = writeln!(
            out,
            "- {label}: {count}  e.g. {}",
            names
                .iter()
                .map(|name| format!("{name:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    out.push_str(
        "\nCall get_node with one of those names to see its full RON, then place_node to \
         clone it somewhere, or set_node to edit it. Cloning a working example beats \
         constructing a variant from scratch.\n",
    );
    Ok(out)
}

/// One node's full detail: transform, parent, children, and its kind as RON.
pub fn get_node(
    project: &ProjectDocument,
    scene_index: Option<usize>,
    needle: &str,
) -> Result<String, String> {
    let index = resolve_scene(project, scene_index)?;
    let scene = &project.scenes[index];
    let id = find_node(scene, needle)?;
    let node = scene
        .node(id)
        .ok_or_else(|| format!("node {needle:?} vanished"))?;
    let mut out = format!("# {:?} (id {})\n\n", node.name, node.id.raw());
    let _ = writeln!(out, "kind: {}", node.kind.label());
    let translation = node.transform.translation;
    let _ = writeln!(
        out,
        "position: [{:.0}, {:.0}, {:.0}]",
        translation[0], translation[1], translation[2]
    );
    let rotation = node.transform.rotation_degrees;
    let _ = writeln!(
        out,
        "rotation degrees: [{:.1}, {:.1}, {:.1}]",
        rotation[0], rotation[1], rotation[2]
    );
    if let Some(parent) = node.parent.and_then(|id| scene.node(id)) {
        let _ = writeln!(out, "parent: {:?} (id {})", parent.name, parent.id.raw());
    }
    if !node.children.is_empty() {
        // Ids, not just names: a cloned subtree keeps its children's names,
        // so several nodes share one name and only the id addresses them.
        let names: Vec<_> = node
            .children
            .iter()
            .filter_map(|id| scene.node(*id))
            .map(|child| format!("{:?} (id {})", child.name, child.id.raw()))
            .collect();
        let _ = writeln!(out, "children: {}", names.join(", "));
    }
    let _ = writeln!(out, "\n## kind payload (RON)\n\n```\n{}\n```", node_kind_to_ron(&node.kind)?);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The round trip these tools rest on: a node kind survives RON in both
    /// directions. If that ever stops holding, every entity tool is unsound.
    #[test]
    fn node_kinds_round_trip_through_ron() {
        let project = ProjectDocument::default();
        let scene = &project.scenes[0];
        let mut checked = 0usize;
        for node in scene.nodes() {
            let text = node_kind_to_ron(&node.kind).expect("serialize");
            let parsed = node_kind_from_ron(&text).expect("parse back");
            assert_eq!(parsed, node.kind, "{:?} did not survive RON", node.name);
            checked += 1;
        }
        assert!(checked > 20, "the starter scene should exercise many kinds");

        // Lookup: exact name, numeric id, and a refusal that lists candidates.
        let world = find_node(scene, "World").expect("the root is named World");
        assert_eq!(world, NodeId::ROOT);
        assert!(find_node(scene, "no such node at all").is_err());
        let by_id = find_node(scene, &world.raw().to_string()).expect("numeric id resolves");
        assert_eq!(by_id, world);

        // A garbled payload is an error carrying a usable example.
        let error = node_kind_from_ron("PointLight(nonsense").expect_err("must not parse");
        assert!(error.contains("NodeKind variant"), "{error}");
    }
}
