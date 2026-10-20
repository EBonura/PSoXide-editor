//! The Inspector of an actor: its name and placement, then its parts (Character,
//! Model, Animation, Camera, Equipment, Weight, Interaction) as collapsible
//! sections. The document keeps the parts as child nodes of the Entity; here
//! each is the full editor of that node, so nothing is a row and nothing is
//! lost.

use super::*;

/// The resource lists the part editors offer, owned so the scene can be
/// borrowed mutably while a section draws.
struct PartLookups {
    material_options: Vec<(ResourceId, String)>,
    material_texture_dimensions: Vec<(ResourceId, [u16; 2])>,
    texture_options: Vec<(ResourceId, String)>,
    destructible_options: Vec<(NodeId, String)>,
    model_options: Vec<(ResourceId, String, Vec<String>)>,
    character_options: Vec<(ResourceId, String)>,
    character_defaults: Vec<(ResourceId, CharacterControllerSettings)>,
    character_loadouts: Vec<(ResourceId, Vec<String>)>,
    weapon_options: Vec<(ResourceId, String)>,
    boost_module_options: Vec<(ResourceId, String)>,
}

fn named_resources(
    project: &ProjectDocument,
    keep: impl Fn(&ResourceData) -> bool,
) -> Vec<(ResourceId, String)> {
    project
        .resources
        .iter()
        .filter(|resource| keep(&resource.data))
        .map(|resource| (resource.id, resource.name.clone()))
        .collect()
}

impl PartLookups {
    fn collect(ws: &EditorWorkspace) -> Self {
        let project = &ws.project;
        Self {
            material_options: project.material_options(),
            material_texture_dimensions: ws
                .texture_thumbs
                .iter()
                .map(|(id, entry)| (*id, [entry.stats.width, entry.stats.height]))
                .collect(),
            texture_options: named_resources(project, |d| {
                matches!(d, ResourceData::Texture { .. })
            }),
            destructible_options: collect_destructible_options(project),
            model_options: collect_model_options(project),
            character_options: collect_character_options(project),
            character_defaults: project
                .resources
                .iter()
                .filter_map(|resource| match &resource.data {
                    ResourceData::Character(character) => Some((
                        resource.id,
                        CharacterControllerSettings::from_character(character),
                    )),
                    _ => None,
                })
                .collect(),
            character_loadouts: project
                .resources
                .iter()
                .filter_map(|resource| match &resource.data {
                    ResourceData::Character(character) => Some((
                        resource.id,
                        character
                            .loadouts
                            .iter()
                            .map(|loadout| loadout.name.clone())
                            .collect(),
                    )),
                    _ => None,
                })
                .collect(),
            weapon_options: collect_weapon_options(project),
            boost_module_options: named_resources(project, |d| {
                matches!(d, ResourceData::BoostModule(_))
            }),
        }
    }
}

/// How a part is titled in the Inspector, its icon, and whether its section
/// starts open. They start folded so an actor reads as its list of parts; an
/// Interaction is the point of a beacon, so it starts open.
fn part_section(kind: &NodeKind) -> (&'static str, char, bool) {
    match kind {
        NodeKind::CharacterController { .. } => ("Character", icons::PERSON_STANDING, false),
        NodeKind::ModelRenderer { .. } => ("Model", icons::BOX, false),
        NodeKind::Animator { .. } => ("Animation", icons::PLAY, false),
        NodeKind::Camera { .. } => ("Camera", icons::CAMERA, false),
        NodeKind::Equipment { .. } => ("Equipment", icons::HAMMER, false),
        NodeKind::PhysicsBody { .. } => ("Weight", icons::SCALE_3D, false),
        NodeKind::Interactable { .. } | NodeKind::PointOfInterest { .. } => {
            ("Interaction", icons::MESSAGE_SQUARE, true)
        }
        NodeKind::Collider { .. } => ("Collider", icons::SCALE_3D, false),
        _ => ("Part", icons::LAYERS, false),
    }
}

/// The order the sections come in.
fn part_rank(kind: &NodeKind) -> u8 {
    match kind {
        NodeKind::CharacterController { .. } => 0,
        NodeKind::ModelRenderer { .. } => 1,
        NodeKind::Animator { .. } => 2,
        NodeKind::Camera { .. } => 3,
        NodeKind::Equipment { .. } => 4,
        NodeKind::PhysicsBody { .. } => 5,
        NodeKind::Interactable { .. } | NodeKind::PointOfInterest { .. } => 6,
        _ => 7,
    }
}

/// The parts "Add part" offers: the part templates by the word the author
/// sees, minus the ones the actor has. Interactable and Point of Interest are
/// one Interaction.
fn addable_parts(scene: &Scene, actor: NodeId) -> Vec<(&'static str, &'static str, NodeKind)> {
    let existing: Vec<&NodeKind> = actor_parts(scene, actor)
        .into_iter()
        .filter_map(|id| scene.node(id))
        .map(|node| &node.kind)
        .collect();
    component_templates_for_host(&NodeKind::Entity)
        .into_iter()
        .filter_map(|(node_label, kind)| {
            let shown = match node_label {
                "Model Renderer" => "Model",
                "Animator" => "Animation",
                "Character Controller" => "Character",
                "Physics Body" => "Weight",
                "Point of Interest" => "Interaction",
                "Interactable" => return None,
                "Camera" => "Camera",
                "Equipment" => "Equipment",
                _ => return None,
            };
            let free = if shown == "Interaction" {
                !existing.iter().any(|k| {
                    matches!(
                        k,
                        NodeKind::PointOfInterest { .. } | NodeKind::Interactable { .. }
                    )
                })
            } else {
                component_can_be_added(&kind, &existing)
            };
            free.then_some((shown, node_label, kind))
        })
        .collect()
}

impl EditorWorkspace {
    /// The actor the Inspector shows for the selection: an Entity, or the
    /// Entity that owns the selected part node.
    pub(crate) fn actor_for_inspector(&self) -> Option<NodeId> {
        let scene = self.project.active_scene();
        let id = self.selection.selected_node;
        let owner = owner_object(scene, id);
        scene
            .node(owner)
            .is_some_and(|node| matches!(node.kind, NodeKind::Entity))
            .then_some(owner)
    }

    /// Draw the Inspector of `actor`. A selected part node of it is the one
    /// whose section opens.
    pub(crate) fn draw_actor_inspector(
        &mut self,
        ui: &mut egui::Ui,
        actor: NodeId,
        camera_preview: Option<EditorCameraPreviewPresentation>,
    ) {
        let lookups = PartLookups::collect(self);
        let selected = self.selection.selected_node;
        let focus = (selected != actor).then_some(selected);
        let focus_new = focus.is_some() && self.inspector_focused_part != focus;
        self.inspector_focused_part = focus;
        let world_sector_size = self.project.world_sector_size_for_node(actor);
        let mut changed = false;
        let mut nav_target: Option<ResourceId> = None;
        let mut snap_to_floor = false;
        let mut preview_action = None;
        let mut remove_part: Option<NodeId> = None;
        let mut open_material_lab: Option<ResourceId> = None;
        let mut became_player: Option<NodeId> = None;

        // Identity, name and placement.
        {
            let scene = self.project.active_scene();
            let Some(node) = scene.node(actor) else {
                ui.weak("No actor selected");
                return;
            };
            let kind = classify(scene, node);
            let title = match kind {
                ObjectKind::ModelActor => "Prop".to_string(),
                ObjectKind::Interaction => interaction_kind_of(scene, actor)
                    .map_or("Interaction".to_string(), |k| {
                        format!("Interaction · {}", k.label())
                    }),
                _ => "Actor".to_string(),
            };
            inspector_identity_header(
                ui,
                kind.icon(),
                kind.color(),
                &node.name,
                &title,
                actor.raw(),
            );
        }
        {
            let scene = self.project.active_scene_mut();
            let Some(node) = scene.node_mut(actor) else {
                return;
            };
            changed |= inspector_property_row(ui, "Name", |ui| {
                ui.add(egui::TextEdit::singleline(&mut node.name).desired_width(f32::INFINITY))
                    .changed()
            });
            inspector_section(
                ui,
                ("node-transform", actor.raw()),
                icons::MOVE,
                "Transform",
                true,
                |ui| {
                    changed |= draw_transform_policy_editor(
                        ui,
                        node,
                        world_sector_size,
                        &lookups.texture_options,
                        &mut nav_target,
                        &mut snap_to_floor,
                    );
                },
            );
        }

        // One section per part.
        let mut parts: Vec<(NodeId, NodeKind)> = actor_parts(self.project.active_scene(), actor)
            .into_iter()
            .filter_map(|id| Some((id, self.project.active_scene().node(id)?.kind.clone())))
            .collect();
        parts.sort_by_key(|(_, kind)| part_rank(kind));
        for (part, kind) in parts {
            let (title, icon, default_open) = part_section(&kind);
            let animator_clip_context =
                selected_animator_clip_context(&self.project, part, &self.project_dir);
            let was_player = matches!(kind, NodeKind::CharacterController { player: true, .. });
            let before = kind.clone();
            let project = &mut self.project;
            let mut section = egui::CollapsingHeader::new(icons::label(icon, title))
                .id_salt(("actor-part", actor.raw(), part.raw()))
                .default_open(default_open);
            if focus == Some(part) && focus_new {
                section = section.open(Some(true));
            }
            let response = section.show(ui, |ui| {
                let mut part_changed = false;
                {
                    let Some(node) = project.active_scene_mut().node_mut(part) else {
                        return false;
                    };
                    part_changed |= draw_node_kind_editor(
                        ui,
                        &mut node.kind,
                        NodeKindEditorContext {
                            character_defaults: &lookups.character_defaults,
                            character_loadouts: &lookups.character_loadouts,
                            material_options: &lookups.material_options,
                            material_texture_dimensions: &lookups.material_texture_dimensions,
                            texture_options: &lookups.texture_options,
                            destructible_options: &lookups.destructible_options,
                            model_options: &lookups.model_options,
                            character_options: &lookups.character_options,
                            weapon_options: &lookups.weapon_options,
                            boost_module_options: &lookups.boost_module_options,
                            animator_clip_context: animator_clip_context.as_ref(),
                            world_sector_size,
                            nav_target: &mut nav_target,
                            character_preview_action: &mut preview_action,
                            camera_preview,
                        },
                    );
                }
                // A model's material override edits the Material resource.
                if let NodeKind::ModelRenderer {
                    material: Some(material_id),
                    ..
                } = project
                    .active_scene()
                    .node(part)
                    .map(|node| node.kind.clone())
                    .unwrap_or(NodeKind::Node)
                {
                    egui::CollapsingHeader::new(icons::label(icons::BLEND, "Material Override"))
                        .id_salt(("actor-material", part.raw()))
                        .default_open(true)
                        .show(ui, |ui| {
                            if ui
                                .button(icons::label(icons::PALETTE, "Open full Material Lab"))
                                .clicked()
                            {
                                open_material_lab = Some(material_id);
                            }
                            let Some(resource) = project.resource_mut(material_id) else {
                                ui.colored_label(
                                    Color32::from_rgb(220, 120, 100),
                                    "Material resource is missing.",
                                );
                                return;
                            };
                            let ResourceData::Material(material) = &mut resource.data else {
                                ui.colored_label(
                                    Color32::from_rgb(220, 120, 100),
                                    "Selected resource is not a Material.",
                                );
                                return;
                            };
                            part_changed |= draw_model_material_override_editor(
                                ui,
                                material,
                                &lookups.material_options,
                                material_id,
                            );
                        });
                }
                ui.add_space(2.0);
                if ui
                    .small_button(icons::label(icons::TRASH, "Remove part"))
                    .on_hover_text("Delete this part from the actor")
                    .clicked()
                {
                    remove_part = Some(part);
                }
                part_changed
            });
            if focus == Some(part) && focus_new {
                response.header_response.scroll_to_me(None);
            }
            if response.body_returned == Some(true) {
                changed = true;
                let now = self
                    .project
                    .active_scene()
                    .node(part)
                    .map(|node| node.kind.clone());
                if let Some(now) = now {
                    self.reconcile_character_preview_after_node_kind_edit(part, &before);
                    if !was_player
                        && matches!(now, NodeKind::CharacterController { player: true, .. })
                    {
                        became_player = Some(part);
                    }
                }
            }
        }

        // The parts the actor could still have.
        let addable = addable_parts(self.project.active_scene(), actor);
        let mut add_part = None;
        ui.add_space(4.0);
        ui.menu_button(icons::label(icons::PLUS, "Add part"), |ui| {
            if addable.is_empty() {
                ui.weak("This actor has every part.");
            }
            for (shown, node_label, kind) in &addable {
                if ui.button(*shown).clicked() {
                    add_part = Some((*node_label, kind.clone()));
                    ui.close_menu();
                }
            }
        });

        if snap_to_floor {
            self.snap_selected_entities_to_floor();
        }
        if let Some(part) = became_player {
            self.demote_player_sources_except(Some(part));
        }
        if let Some(action) = preview_action {
            self.preview_character_action(actor, action);
        }
        if let Some(part) = remove_part {
            self.remove_actor_part(actor, part);
        }
        if let Some((label, kind)) = add_part {
            self.add_component_to_host(actor, label, kind);
        }
        if let Some(material) = open_material_lab {
            self.material_lab.focused_material = Some(material);
            self.active_workspace = WorkspaceView::Material;
            self.status = "Opened Material Lab".to_string();
        }
        if changed {
            self.mark_dirty();
            ui.ctx().request_repaint();
        }
        if let Some(target) = nav_target {
            self.replace_resource_selection(target);
            self.clear_node_selection_state();
        }
    }

    /// Delete one part of an actor, as one undo step. The actor stays
    /// selected.
    pub(crate) fn remove_actor_part(&mut self, actor: NodeId, part: NodeId) {
        let is_part = self
            .project
            .active_scene()
            .node(part)
            .is_some_and(|node| node.parent == Some(actor) && node.kind.is_component());
        if !is_part {
            return;
        }
        self.push_undo();
        let label = self
            .project
            .active_scene()
            .node(part)
            .map(|node| part_section(&node.kind).0)
            .unwrap_or("part");
        if self.project.active_scene_mut().remove_node(part) {
            self.replace_node_selection(actor);
            self.status = format!("Removed {label}");
            self.mark_dirty();
        }
    }
}
