//! The Objects panel's model: what the Scene Graph lists, as the author
//! thinks of it.
//!
//! The document keeps an Entity with child nodes for its parts (Model
//! Renderer, Animator, Character Controller, ...). The author sees one
//! object, an *actor*, and its parts as sections of the Inspector. This
//! module is the view layer between the two: it hides the part nodes, names
//! each object by what it is, and sorts objects into groups. Nothing here
//! writes to the document.

use super::*;

/// The groups of the Objects panel, in the order they are listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ObjectGroup {
    Actors,
    Lights,
    Props,
    Interactions,
    Logic,
    Other,
}

impl ObjectGroup {
    pub(crate) const ALL: [Self; 6] = [
        Self::Actors,
        Self::Lights,
        Self::Props,
        Self::Interactions,
        Self::Logic,
        Self::Other,
    ];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Actors => "Actors",
            Self::Lights => "Lights",
            Self::Props => "Props",
            Self::Interactions => "Interactions",
            Self::Logic => "Logic",
            Self::Other => "Other",
        }
    }
}

/// What an object is. One icon each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ObjectKind {
    /// An actor with a Character Controller: the player or an enemy.
    Character,
    /// An actor with a model and nothing that moves it.
    ModelActor,
    /// An actor with no parts yet.
    EmptyActor,
    /// An actor with an Interaction (Interactable or Point of Interest).
    Interaction,
    Light,
    Billboard,
    BoxProp,
    CylinderProp,
    ArchProp,
    MeshInstance,
    Trigger,
    Zone,
    Breakable,
    HookPoint,
    Emitter,
    SpawnPoint,
    Folder,
    /// A part node that is not under an actor (a stray component).
    Part,
}

impl ObjectKind {
    #[cfg(test)]
    pub(crate) const ALL: [Self; 18] = [
        Self::Character,
        Self::ModelActor,
        Self::EmptyActor,
        Self::Interaction,
        Self::Light,
        Self::Billboard,
        Self::BoxProp,
        Self::CylinderProp,
        Self::ArchProp,
        Self::MeshInstance,
        Self::Trigger,
        Self::Zone,
        Self::Breakable,
        Self::HookPoint,
        Self::Emitter,
        Self::SpawnPoint,
        Self::Folder,
        Self::Part,
    ];

    pub(crate) const fn group(self) -> ObjectGroup {
        match self {
            Self::Character | Self::EmptyActor => ObjectGroup::Actors,
            Self::Light => ObjectGroup::Lights,
            Self::ModelActor
            | Self::Billboard
            | Self::BoxProp
            | Self::CylinderProp
            | Self::ArchProp
            | Self::MeshInstance => ObjectGroup::Props,
            Self::Interaction => ObjectGroup::Interactions,
            Self::Trigger | Self::Zone | Self::Breakable => ObjectGroup::Logic,
            Self::HookPoint | Self::Emitter | Self::SpawnPoint | Self::Folder | Self::Part => {
                ObjectGroup::Other
            }
        }
    }

    /// The word for what it is.
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Character => "Actor",
            Self::ModelActor => "Prop",
            Self::EmptyActor => "Actor",
            Self::Interaction => "Interaction",
            Self::Light => "Light",
            Self::Billboard => "Billboard",
            Self::BoxProp => "Box",
            Self::CylinderProp => "Cylinder",
            Self::ArchProp => "Arch",
            Self::MeshInstance => "Mesh",
            Self::Trigger => "Trigger",
            Self::Zone => "Zone",
            Self::Breakable => "Breakable",
            Self::HookPoint => "Hook point",
            Self::Emitter => "Emitter",
            Self::SpawnPoint => "Spawn point",
            Self::Folder => "Folder",
            Self::Part => "Part",
        }
    }

    /// Its icon. No two kinds share one and none is the generic dot.
    pub(crate) const fn icon(self) -> char {
        match self {
            Self::Character => icons::PERSON_STANDING,
            Self::ModelActor => icons::PACKAGE,
            Self::EmptyActor => icons::USER,
            Self::Interaction => icons::MESSAGE_SQUARE,
            Self::Light => icons::LIGHTBULB,
            Self::Billboard => icons::IMAGE,
            Self::BoxProp => icons::BOX,
            Self::CylinderProp => icons::CYLINDER,
            Self::ArchProp => icons::TRIANGLE,
            Self::MeshInstance => icons::CUBOID,
            Self::Trigger => icons::ZAP,
            Self::Zone => icons::TARGET,
            Self::Breakable => icons::SHIELD,
            Self::HookPoint => icons::ANCHOR,
            Self::Emitter => icons::SPARKLES,
            Self::SpawnPoint => icons::FLAG,
            Self::Folder => icons::FOLDER,
            Self::Part => icons::LAYERS,
        }
    }

    /// Its colour in the list.
    pub(crate) const fn color(self) -> Color32 {
        match self {
            Self::Character | Self::EmptyActor => Color32::from_rgb(104, 194, 142),
            Self::ModelActor | Self::BoxProp | Self::CylinderProp | Self::ArchProp => {
                Color32::from_rgb(135, 180, 220)
            }
            Self::MeshInstance => Color32::from_rgb(156, 174, 190),
            Self::Billboard => Color32::from_rgb(210, 170, 120),
            Self::Interaction => Color32::from_rgb(224, 112, 96),
            Self::Light => Color32::from_rgb(238, 203, 116),
            Self::Trigger | Self::Zone | Self::Breakable => Color32::from_rgb(186, 150, 224),
            Self::Emitter => Color32::from_rgb(152, 214, 230),
            Self::HookPoint | Self::SpawnPoint => Color32::from_rgb(236, 188, 104),
            Self::Folder | Self::Part => Color32::from_rgb(141, 160, 180),
        }
    }
}

/// One row of the Objects panel.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ObjectRow {
    /// The node the row stands for (an Entity for an actor).
    pub(crate) id: NodeId,
    pub(crate) parent: Option<NodeId>,
    /// Depth in the tree view, 0 for a child of the World root.
    pub(crate) depth: usize,
    pub(crate) sibling_index: usize,
    pub(crate) name: String,
    pub(crate) kind: ObjectKind,
    /// One short word or number after the name: "Player", "r 6.0".
    pub(crate) detail: Option<String>,
    /// A colour of its own that replaces the kind's: a light's.
    pub(crate) tint: Option<Color32>,
    /// Children that are rows themselves (a folder's contents; not parts).
    pub(crate) child_count: usize,
}

impl ObjectRow {
    pub(crate) fn group(&self) -> ObjectGroup {
        self.kind.group()
    }

    pub(crate) fn icon_color(&self) -> Color32 {
        self.tint.unwrap_or_else(|| self.kind.color())
    }
}

/// Whether `kind` is a part of an actor when it sits under an Entity.
fn is_part_under_entity(scene: &Scene, node: &SceneNode) -> bool {
    node.kind.is_component()
        && node
            .parent
            .and_then(|parent| scene.node(parent))
            .is_some_and(|parent| matches!(parent.kind, NodeKind::Entity))
}

/// The part nodes of the actor `entity`, in storage order.
pub(crate) fn actor_parts(scene: &Scene, entity: NodeId) -> Vec<NodeId> {
    scene
        .node(entity)
        .into_iter()
        .flat_map(|node| node.children.iter().copied())
        .filter(|id| scene.node(*id).is_some_and(|c| c.kind.is_component()))
        .collect()
}

/// The object that owns `id` in the Objects panel: the actor, for one of an
/// actor's parts; the node itself for everything else.
pub(crate) fn owner_object(scene: &Scene, id: NodeId) -> NodeId {
    match scene.node(id) {
        Some(node) if is_part_under_entity(scene, node) => node.parent.unwrap_or(id),
        _ => id,
    }
}

/// What a node is, by its kind and, for an Entity, by its parts.
pub(crate) fn classify(scene: &Scene, node: &SceneNode) -> ObjectKind {
    match &node.kind {
        NodeKind::Entity => {
            let parts: Vec<&NodeKind> = actor_parts(scene, node.id)
                .into_iter()
                .filter_map(|id| scene.node(id))
                .map(|part| &part.kind)
                .collect();
            let has = |f: fn(&NodeKind) -> bool| parts.iter().any(|k| f(k));
            if has(|k| matches!(k, NodeKind::CharacterController { .. })) {
                ObjectKind::Character
            } else if has(|k| {
                matches!(
                    k,
                    NodeKind::PointOfInterest { .. } | NodeKind::Interactable { .. }
                )
            }) {
                ObjectKind::Interaction
            } else if has(|k| matches!(k, NodeKind::ModelRenderer { .. })) {
                ObjectKind::ModelActor
            } else {
                ObjectKind::EmptyActor
            }
        }
        NodeKind::PointLight { .. } => ObjectKind::Light,
        NodeKind::ImageProp { .. } => ObjectKind::Billboard,
        NodeKind::BoxProp { .. } => ObjectKind::BoxProp,
        NodeKind::CylinderProp { .. } => ObjectKind::CylinderProp,
        NodeKind::ArchProp { .. } => ObjectKind::ArchProp,
        NodeKind::MeshInstance { .. } => ObjectKind::MeshInstance,
        NodeKind::Logic { .. } => ObjectKind::Trigger,
        NodeKind::VitalityCircle { .. } => ObjectKind::Zone,
        NodeKind::Destructible { .. } => ObjectKind::Breakable,
        NodeKind::HookPoint => ObjectKind::HookPoint,
        NodeKind::ParticleEmitter { .. } => ObjectKind::Emitter,
        NodeKind::SpawnPoint { .. } => ObjectKind::SpawnPoint,
        NodeKind::Group | NodeKind::Node | NodeKind::Node3D | NodeKind::World { .. } => {
            ObjectKind::Folder
        }
        _ => ObjectKind::Part,
    }
}

fn detail_of(scene: &Scene, node: &SceneNode, kind: ObjectKind) -> Option<String> {
    match (&node.kind, kind) {
        (NodeKind::PointLight { radius, .. }, _) => Some(format!("r {radius:.1}")),
        (NodeKind::Entity, ObjectKind::Character) => {
            let player = actor_parts(scene, node.id)
                .into_iter()
                .filter_map(|id| scene.node(id))
                .any(|part| {
                    matches!(
                        part.kind,
                        NodeKind::CharacterController { player: true, .. }
                    )
                });
            Some(if player { "Player" } else { "Enemy" }.to_string())
        }
        (NodeKind::Entity, ObjectKind::ModelActor) => None,
        (_, ObjectKind::Part) => Some(node.kind.label().to_string()),
        _ => None,
    }
}

/// Every object of the scene, in document order: the nodes under the World
/// root, minus the part nodes of actors.
pub(crate) fn object_rows(scene: &Scene) -> Vec<ObjectRow> {
    let mut rows = Vec::new();
    for row in scene.hierarchy_rows() {
        if row.id == scene.root {
            continue;
        }
        let Some(node) = scene.node(row.id) else {
            continue;
        };
        if is_part_under_entity(scene, node) {
            continue;
        }
        let kind = classify(scene, node);
        let tint = match &node.kind {
            NodeKind::PointLight { color, .. } => {
                Some(Color32::from_rgb(color[0], color[1], color[2]))
            }
            _ => None,
        };
        let child_count = node
            .children
            .iter()
            .filter_map(|id| scene.node(*id))
            .filter(|child| !is_part_under_entity(scene, child))
            .count();
        rows.push(ObjectRow {
            id: row.id,
            parent: row.parent,
            depth: row.depth.saturating_sub(1),
            sibling_index: row.sibling_index,
            name: node.name.clone(),
            kind,
            detail: detail_of(scene, node, kind),
            tint,
            child_count,
        });
    }
    rows
}

/// The rows of one group, in document order.
pub(crate) fn rows_of_group(rows: &[ObjectRow], group: ObjectGroup) -> Vec<&ObjectRow> {
    rows.iter().filter(|row| row.group() == group).collect()
}

pub(crate) fn object_matches_filter(row: &ObjectRow, filter: &str) -> bool {
    filter.is_empty()
        || row.name.to_ascii_lowercase().contains(filter)
        || row.kind.label().to_ascii_lowercase().contains(filter)
        || row.group().label().to_ascii_lowercase().contains(filter)
}

/// The tree view's rows: the document order with a folder's contents hidden
/// while it is collapsed.
pub(crate) fn tree_display_rows<'a>(
    rows: &'a [ObjectRow],
    collapsed: &HashSet<NodeId>,
) -> Vec<&'a ObjectRow> {
    let mut collapsed_depth = None;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        if let Some(depth) = collapsed_depth {
            if row.depth > depth {
                continue;
            }
            collapsed_depth = None;
        }
        out.push(row);
        if row.child_count > 0 && collapsed.contains(&row.id) {
            collapsed_depth = Some(row.depth);
        }
    }
    out
}

/// The name a new light gets: "Light 3" when two lights exist, and the first
/// "Light N" nobody has taken.
pub(crate) fn next_light_name(scene: &Scene) -> String {
    let lights = scene
        .nodes()
        .iter()
        .filter(|node| matches!(node.kind, NodeKind::PointLight { .. }))
        .count();
    let mut n = lights + 1;
    loop {
        let name = format!("Light {n}");
        if !scene.nodes().iter().any(|node| node.name == name) {
            return name;
        }
        n += 1;
    }
}
