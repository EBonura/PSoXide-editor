use super::*;

fn add_model_renderer_node(
    scene: &mut Scene,
    entity: NodeId,
    model_id: ResourceId,
    visual_scale_q8: u16,
    default_visual_yaw_q12: i16,
) -> NodeId {
    let renderer = scene.add_node(
        entity,
        "Model Renderer",
        NodeKind::ModelRenderer {
            model: Some(model_id),
            material: None,
            visual_offset: [0; 3],
            visual_scale_q8,
        },
    );
    if let Some(node) = scene.node_mut(renderer) {
        node.transform.rotation_degrees[1] = q12_turns_to_degrees(default_visual_yaw_q12 as i32);
    }
    renderer
}

fn default_model_visual_defaults(project: &ProjectDocument, model_id: ResourceId) -> (u16, i16) {
    project
        .resource(model_id)
        .and_then(|resource| match &resource.data {
            ResourceData::Model(model) => {
                Some((model.scale_q8[1].max(1), model.default_visual_yaw_q12))
            }
            _ => None,
        })
        .unwrap_or((psxed_project::MODEL_SCALE_ONE_Q8, 0))
}

impl EditorWorkspace {
    /// Character explicitly authored for the player role when there is one
    /// unambiguous choice. Enemy profiles never participate in this fallback,
    /// so adding the light/heavy catalogue cannot make a new player spawn
    /// ambiguous.
    pub(crate) fn default_player_character_resource(&self) -> Option<ResourceId> {
        let mut players = self.project.resources.iter().filter_map(|resource| {
            matches!(
                &resource.data,
                ResourceData::Character(character)
                    if character.spawn_role == psxed_project::CharacterSpawnRole::Player
            )
            .then_some(resource.id)
        });
        let player = players.next()?;
        players.next().is_none().then_some(player)
    }

    /// The scene root is the parent of authored point entities: the cook
    /// consumes their transforms directly in world units.
    pub(crate) fn authoring_root(&self) -> NodeId {
        self.project.active_scene().root
    }

    /// Vertical lift applied to a placement anchor.
    ///
    /// Floor-anchored actors get one unit so they never start inside the solid
    /// boundary, and point lights get a useful room-height default. Logic
    /// nodes get NOTHING: a Trigger Volume's AABB grows upward from its anchor
    /// while the character motor stands its feet exactly on the floor plane,
    /// so a lifted anchor produces a volume that starts one unit above the
    /// player and can never fire.
    pub(crate) fn place_height_offset(&self) -> f32 {
        match self.place_kind {
            PlaceKind::PointLightMarker => 256.0,
            PlaceKind::Logic => 0.0,
            _ => 1.0,
        }
    }

    pub(crate) fn place_at_world_hit(&mut self, hit: [f32; 3]) -> bool {
        let root = self.authoring_root();
        let before = self.project.active_scene().nodes().len();
        self.place_node_at_world_hit(root, hit);
        let placed = self.project.active_scene().nodes().len() > before;
        if placed {
            self.status = if matches!(self.place_kind, PlaceKind::PointOfInterest) {
                format!(
                    "Placed Point of Interest at {:.0},{:.0},{:.0} — edit Page 1 to replace the default message",
                    hit[0], hit[1], hit[2]
                )
            } else {
                format!(
                    "Placed {} at {:.0},{:.0},{:.0}",
                    self.place_kind.label(),
                    hit[0],
                    hit[1],
                    hit[2]
                )
            };
        }
        placed
    }

    /// Height of the floor under an editor-space X/Z, or the shared
    /// orthographic Y focus when the scene has no grid world or no floor
    /// there.
    pub(crate) fn floor_y_at(&self, world: [f32; 2]) -> f32 {
        self.project
            .active_scene()
            .world
            .grid()
            .and_then(|grid| {
                grid.floor_height_at(
                    world[0].round() as i32,
                    world[1].round() as i32,
                    i32::MAX / 2,
                )
            })
            .map_or(self.orthographic_focus[1], |y| y as f32)
    }

    /// Top-view placement of the active `PlaceKind` on the floor under the
    /// clicked X/Z.
    pub(crate) fn place_node_from_top(&mut self, world: [f32; 2]) -> bool {
        self.place_at_world_hit([
            world[0],
            self.floor_y_at(world) + self.place_height_offset(),
            world[1],
        ])
    }

    /// Drop a resource into the scene from the 3D view: models, characters
    /// and weapons land on the grid face under the pointer, one unit above it.
    pub(crate) fn drop_resource_3d(
        &mut self,
        resource_id: ResourceId,
        rect: egui::Rect,
        pointer: egui::Pos2,
    ) {
        let Some(resource) = self.project.resource(resource_id) else {
            self.status = format!("Resource #{} no longer exists", resource_id.raw());
            return;
        };
        if matches!(resource.data, ResourceData::Material(_)) {
            self.status =
                "Materials are applied from the palette: select faces, then click a swatch"
                    .to_string();
            return;
        }
        let Some(hit) = self.pick_face_3d(rect, pointer) else {
            self.status = "Drop onto a floor face".to_string();
            return;
        };
        let mut point = hit.point;
        // The same one-unit floor clearance the Place tool applies.
        point[1] += 1.0;
        let root = self.authoring_root();
        self.drop_resource_at_room_hit(resource_id, root, point);
    }

    pub(crate) fn drop_resource_2d(&mut self, resource_id: ResourceId, editor_world: [f32; 2]) {
        let Some(resource) = self.project.resource(resource_id) else {
            self.status = format!("Resource #{} no longer exists", resource_id.raw());
            return;
        };
        if matches!(resource.data, ResourceData::Material(_)) {
            self.status =
                "Materials are applied from the palette: select faces, then click a swatch"
                    .to_string();
            return;
        }
        let root = self.authoring_root();
        self.drop_resource_at_room_hit(
            resource_id,
            root,
            [
                editor_world[0],
                self.floor_y_at(editor_world) + 1.0,
                editor_world[1],
            ],
        );
    }

    pub(crate) fn drop_resource_at_room_hit(
        &mut self,
        resource_id: ResourceId,
        room_id: NodeId,
        hit_world: [f32; 3],
    ) {
        let Some(resource) = self.project.resource(resource_id).cloned() else {
            self.status = format!("Resource #{} no longer exists", resource_id.raw());
            return;
        };

        match resource.data {
            ResourceData::Model(_) => {
                let translation = self.placement_translation_for_room_hit(room_id, hit_world);
                if let Some(existing) =
                    self.find_duplicate_model_entity(room_id, resource_id, translation)
                {
                    self.reject_duplicate_placement(existing, "Prop");
                    return;
                }
                self.push_undo();
                let node = self.create_model_entity_at_room_hit(
                    room_id,
                    resource_id,
                    &resource.name,
                    hit_world,
                );
                self.replace_node_selection(node);
                self.clear_resource_selection_state();
                self.status = format!("Created Prop Entity from model {}", resource.name);
                self.mark_dirty();
            }
            ResourceData::Character(character) => {
                let translation = self.placement_translation_for_room_hit(room_id, hit_world);
                if let Some(existing) =
                    self.find_duplicate_character_entity(room_id, resource_id, translation)
                {
                    self.reject_duplicate_placement(existing, "Character");
                    return;
                }
                self.push_undo();
                let player = match character.spawn_role {
                    psxed_project::CharacterSpawnRole::Auto => !self.has_player_source(),
                    psxed_project::CharacterSpawnRole::Player => true,
                    psxed_project::CharacterSpawnRole::Enemy => false,
                };
                if player && character.spawn_role == psxed_project::CharacterSpawnRole::Player {
                    self.demote_player_sources_except(None);
                }
                let idle_clip = self.resolve_character_idle_preview_clip(&character);
                let settings = CharacterControllerSettings::from_character(&character);
                let camera_settings = character.camera_settings();
                let node = self.create_character_entity_at_room_hit(
                    room_id,
                    resource_id,
                    &resource.name,
                    character.model,
                    character.material,
                    idle_clip,
                    settings,
                    player,
                    camera_settings,
                    hit_world,
                );
                self.replace_node_selection(node);
                self.clear_resource_selection_state();
                self.status = if player {
                    format!(
                        "Created Player Character Entity from profile {}",
                        resource.name
                    )
                } else {
                    format!("Created Character Entity from profile {}", resource.name)
                };
                self.mark_dirty();
            }
            ResourceData::Weapon(weapon) => {
                let translation = self.placement_translation_for_room_hit(room_id, hit_world);
                if let Some(existing) =
                    self.find_duplicate_weapon_entity(room_id, resource_id, translation)
                {
                    self.reject_duplicate_placement(existing, "Weapon");
                    return;
                }
                self.push_undo();
                let node = self.create_weapon_entity_at_room_hit(
                    room_id,
                    resource_id,
                    &resource.name,
                    weapon.model,
                    weapon.default_character_socket.as_str(),
                    weapon.grip.name.as_str(),
                    hit_world,
                );
                self.replace_node_selection(node);
                self.clear_resource_selection_state();
                self.status = format!("Created Weapon Entity from resource {}", resource.name);
                self.mark_dirty();
            }
            ResourceData::Material(_) => {
                self.status = "Drop Material onto an existing face".to_string();
            }
            _ => {
                self.status = format!(
                    "Drag Model, Character Profile, or Weapon resources into the scene; {} is not placeable",
                    resource.data.label()
                );
            }
        }
    }

    /// Where a placed Entity lands for a pick: the hit, with X and Z on the
    /// node snap step (`node_snap_step`) like every other move path.
    pub(crate) fn placement_translation_for_room_hit(
        &self,
        room_id: NodeId,
        hit_world: [f32; 3],
    ) -> [f32; 3] {
        self.snap_placed_translation(
            &NodeKind::Entity,
            self.placement_hit_translation(room_id, hit_world),
        )
    }

    /// Land a placed node's X and Z on its snap step. Y keeps the picked
    /// surface height.
    pub(crate) fn snap_placed_translation(
        &self,
        kind: &NodeKind,
        translation: [f32; 3],
    ) -> [f32; 3] {
        let grid = i32::from(self.snap_units.max(1));
        [
            snap_node_component(kind, translation[0], grid),
            translation[1],
            snap_node_component(kind, translation[2], grid),
        ]
    }

    /// The picked point as a node translation, before any snap.
    pub(crate) fn placement_hit_translation(
        &self,
        _room_id: NodeId,
        hit_world: [f32; 3],
    ) -> [f32; 3] {
        // Preserve the picked surface height instead of pinning placed
        // content to the floor. Node translations are world units, so the
        // hit is the translation.
        hit_world
    }

    pub(crate) fn create_model_entity_at_room_hit(
        &mut self,
        room_id: NodeId,
        model_id: ResourceId,
        name: &str,
        hit_world: [f32; 3],
    ) -> NodeId {
        let translation = self.placement_translation_for_room_hit(room_id, hit_world);
        let (visual_scale_q8, default_visual_yaw_q12) =
            default_model_visual_defaults(&self.project, model_id);
        let scene = self.project.active_scene_mut();
        let entity = scene.add_node(room_id, name.to_string(), NodeKind::Entity);
        if let Some(node) = scene.node_mut(entity) {
            node.transform.translation = translation;
        }
        add_model_renderer_node(
            scene,
            entity,
            model_id,
            visual_scale_q8,
            default_visual_yaw_q12,
        );
        scene.add_node(
            entity,
            "Animator",
            NodeKind::Animator {
                clip: None,
                action_clips: Vec::new(),
                autoplay: true,
                pose_frame: 0,
            },
        );
        entity
    }

    pub(crate) fn create_weapon_entity_at_room_hit(
        &mut self,
        room_id: NodeId,
        weapon_id: ResourceId,
        name: &str,
        model_id: Option<ResourceId>,
        character_socket: &str,
        weapon_grip: &str,
        hit_world: [f32; 3],
    ) -> NodeId {
        let translation = self.placement_translation_for_room_hit(room_id, hit_world);
        let model_visual_defaults =
            model_id.map(|model_id| default_model_visual_defaults(&self.project, model_id));
        let scene = self.project.active_scene_mut();
        let entity = scene.add_node(room_id, name.to_string(), NodeKind::Entity);
        if let Some(node) = scene.node_mut(entity) {
            node.transform.translation = translation;
        }
        if let Some(model_id) = model_id {
            let (visual_scale_q8, default_visual_yaw_q12) =
                model_visual_defaults.unwrap_or((psxed_project::MODEL_SCALE_ONE_Q8, 0));
            add_model_renderer_node(
                scene,
                entity,
                model_id,
                visual_scale_q8,
                default_visual_yaw_q12,
            );
        }
        scene.add_node(
            entity,
            "Equipment",
            NodeKind::Equipment {
                weapon: Some(weapon_id),
                character_socket: character_socket.to_string(),
                weapon_grip: weapon_grip.to_string(),
            },
        );
        entity
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn create_character_entity_at_room_hit(
        &mut self,
        room_id: NodeId,
        character_id: ResourceId,
        name: &str,
        model_id: Option<ResourceId>,
        material_id: Option<ResourceId>,
        idle_clip: Option<u16>,
        _settings: CharacterControllerSettings,
        player: bool,
        camera_settings: WorldCameraSettings,
        hit_world: [f32; 3],
    ) -> NodeId {
        let translation = self.placement_translation_for_room_hit(room_id, hit_world);
        let model_visual_defaults =
            model_id.map(|model_id| default_model_visual_defaults(&self.project, model_id));
        let scene = self.project.active_scene_mut();
        let entity = scene.add_node(room_id, name.to_string(), NodeKind::Entity);
        if let Some(node) = scene.node_mut(entity) {
            node.transform.translation = translation;
        }
        if let Some(model_id) = model_id {
            let (visual_scale_q8, default_visual_yaw_q12) =
                model_visual_defaults.unwrap_or((psxed_project::MODEL_SCALE_ONE_Q8, 0));
            let renderer = add_model_renderer_node(
                scene,
                entity,
                model_id,
                visual_scale_q8,
                default_visual_yaw_q12,
            );
            if let Some(node) = scene.node_mut(renderer) {
                if let NodeKind::ModelRenderer { material, .. } = &mut node.kind {
                    *material = material_id;
                }
            }
            scene.add_node(
                entity,
                "Animator",
                NodeKind::Animator {
                    clip: idle_clip,
                    action_clips: Vec::new(),
                    autoplay: true,
                    pose_frame: 0,
                },
            );
        }
        scene.add_node(
            entity,
            "Character Controller",
            NodeKind::CharacterController {
                loadout: None,
                character: Some(character_id),
                // No override: the placement inherits the Character, so later
                // edits to the type reach it. `settings` is materialised only
                // when this placement is actually tuned away from its type.
                settings: None,
                player,
            },
        );
        if player {
            scene.add_node(
                entity,
                "Camera",
                NodeKind::Camera {
                    settings: camera_settings,
                },
            );
        }
        entity
    }

    pub(crate) fn find_duplicate_model_entity(
        &self,
        room_id: NodeId,
        model_id: ResourceId,
        translation: [f32; 3],
    ) -> Option<NodeId> {
        self.find_duplicate_room_child(room_id, translation, |scene, node| {
            matches!(node.kind, NodeKind::Entity)
                && entity_model_resource_id(scene, node) == Some(model_id)
                && entity_character_component_resource_id(scene, node).is_none()
                && entity_weapon_resource_id(scene, node).is_none()
        })
    }

    pub(crate) fn find_duplicate_character_entity(
        &self,
        room_id: NodeId,
        character_id: ResourceId,
        translation: [f32; 3],
    ) -> Option<NodeId> {
        self.find_duplicate_room_child(room_id, translation, |scene, node| {
            matches!(node.kind, NodeKind::Entity)
                && entity_character_component_resource_id(scene, node) == Some(character_id)
        })
    }

    pub(crate) fn find_duplicate_weapon_entity(
        &self,
        room_id: NodeId,
        weapon_id: ResourceId,
        translation: [f32; 3],
    ) -> Option<NodeId> {
        self.find_duplicate_room_child(room_id, translation, |scene, node| {
            matches!(node.kind, NodeKind::Entity)
                && entity_weapon_resource_id(scene, node) == Some(weapon_id)
        })
    }

    pub(crate) fn find_duplicate_image_prop(
        &self,
        room_id: NodeId,
        material_id: ResourceId,
        translation: [f32; 3],
    ) -> Option<NodeId> {
        self.find_duplicate_room_child(room_id, translation, |_, node| {
            matches!(
                node.kind,
                NodeKind::ImageProp {
                    material: Some(id),
                    ..
                } if id == material_id
            )
        })
    }

    pub(crate) fn find_duplicate_box_prop(
        &self,
        room_id: NodeId,
        material_id: Option<ResourceId>,
        translation: [f32; 3],
    ) -> Option<NodeId> {
        self.find_duplicate_room_child(room_id, translation, |_, node| {
            matches!(
                node.kind,
                NodeKind::BoxProp { materials, .. }
                    if material_id.map_or_else(
                        || materials.iter().all(Option::is_none),
                        |material| materials.contains(&Some(material)),
                    )
            )
        })
    }

    pub(crate) fn find_duplicate_cylinder_prop(
        &self,
        room_id: NodeId,
        material_id: Option<ResourceId>,
        translation: [f32; 3],
    ) -> Option<NodeId> {
        self.find_duplicate_room_child(room_id, translation, |_, node| {
            matches!(
                node.kind,
                NodeKind::CylinderProp { materials, .. }
                    if material_id.map_or_else(
                        || materials.iter().all(Option::is_none),
                        |material| materials.contains(&Some(material)),
                    )
            )
        })
    }

    pub(crate) fn find_duplicate_spawn_marker(
        &self,
        room_id: NodeId,
        player: bool,
        translation: [f32; 3],
    ) -> Option<NodeId> {
        self.find_duplicate_room_child(room_id, translation, |_, node| {
            matches!(
                node.kind,
                NodeKind::SpawnPoint {
                    player: node_player,
                    character: None,
                } if node_player == player
            )
        })
    }

    pub(crate) fn find_duplicate_point_light(
        &self,
        room_id: NodeId,
        translation: [f32; 3],
    ) -> Option<NodeId> {
        self.find_duplicate_room_child(room_id, translation, |_, node| {
            matches!(node.kind, NodeKind::PointLight { .. })
        })
    }

    pub(crate) fn find_duplicate_particle_emitter(
        &self,
        room_id: NodeId,
        translation: [f32; 3],
    ) -> Option<NodeId> {
        self.find_duplicate_room_child(room_id, translation, |_, node| {
            matches!(node.kind, NodeKind::ParticleEmitter { .. })
        })
    }

    pub(crate) fn find_duplicate_room_child<F>(
        &self,
        room_id: NodeId,
        translation: [f32; 3],
        mut matches_placement: F,
    ) -> Option<NodeId>
    where
        F: FnMut(&Scene, &SceneNode) -> bool,
    {
        let scene = self.project.active_scene();
        let room = scene.node(room_id)?;
        room.children.iter().copied().find(|child_id| {
            scene.node(*child_id).is_some_and(|node| {
                translations_match(node.transform.translation, translation)
                    && matches_placement(scene, node)
            })
        })
    }

    pub(crate) fn reject_duplicate_placement(&mut self, existing: NodeId, label: &str) {
        self.replace_node_selection(existing);
        self.clear_resource_selection_state();
        self.status = format!("{label} already exists at this position");
    }

    pub(crate) fn resolve_character_idle_preview_clip(
        &self,
        character: &psxed_project::CharacterResource,
    ) -> Option<u16> {
        character.model.and_then(|model_id| {
            self.project.resource(model_id).and_then(|resource| {
                let ResourceData::Model(model) = &resource.data else {
                    return None;
                };
                psxed_project::resolve::resolve_character_idle_preview_clip_for_model(
                    &self.project,
                    character,
                    model_id,
                    model,
                )
            })
        })
    }

    /// Place the active entity kind at a world-space hit.
    pub(crate) fn place_node_at_world_hit(&mut self, room_id: NodeId, hit_world: [f32; 3]) {
        let sector_size_i = DEFAULT_WORLD_SECTOR_SIZE;
        let arch_tile_size = self.project.world_sector_size_for_node(room_id);
        let translation = self.placement_hit_translation(room_id, hit_world);
        let kind = self.place_kind;
        if matches!(kind, PlaceKind::PlayerSpawn) && self.has_player_source() {
            self.status =
                    "Only one player source is allowed per world. Delete or demote the existing player first."
                        .to_string();
            return;
        }
        if matches!(kind, PlaceKind::PointOfInterest) {
            let translation = self.snap_placed_translation(&NodeKind::Entity, translation);
            self.push_undo();
            let scene = self.project.active_scene_mut();
            let entity = scene.add_node(room_id, "Point of Interest", NodeKind::Entity);
            if let Some(node) = scene.node_mut(entity) {
                node.transform.translation = translation;
            }
            let component = scene.add_node(
                entity,
                "Point of Interest",
                NodeKind::PointOfInterest {
                    pages: psxed_project::default_point_of_interest_pages(),
                    pages_it: Vec::new(),
                    prompt: psxed_project::default_point_of_interest_prompt(),
                    radius: psxed_project::default_point_of_interest_radius(),
                    marker_height: psxed_project::default_point_of_interest_marker_height(),
                    repeatable: true,
                    persistence_id: String::new(),
                    reward: None,
                    enabled: true,
                },
            );
            // The authored payload is the useful first inspector. Viewport
            // picking subsequently selects the Entity host for movement.
            self.replace_node_selection(component);
            self.clear_resource_selection_state();
            self.status =
                "Placed Point of Interest — edit Page 1 to replace the default message".to_string();
            self.mark_dirty();
            self.return_to_select_after_place();
            return;
        }
        let (default_name, node_kind): (String, NodeKind) = match kind {
            PlaceKind::PlayerSpawn => (
                "Player Spawn".to_string(),
                NodeKind::SpawnPoint {
                    player: true,
                    character: self.default_player_character_resource(),
                },
            ),
            PlaceKind::SpawnMarker => {
                if let Some(existing) =
                    self.find_duplicate_spawn_marker(room_id, false, translation)
                {
                    self.reject_duplicate_placement(existing, "Spawn");
                    return;
                }
                (
                    "Spawn".to_string(),
                    NodeKind::SpawnPoint {
                        player: false,
                        character: None,
                    },
                )
            }
            PlaceKind::ModelInstance => {
                // Resolve which Model resource to bind. Order:
                // (a) user has a Model selected in the resource
                //     panel -- use it; (b) exactly one Model
                //     resource exists project-wide -- auto-pick;
                //     (c) refuse with an actionable status.
                match self.resolve_place_model_resource() {
                    Ok((model_id, name)) => {
                        if let Some(existing) =
                            self.find_duplicate_model_entity(room_id, model_id, translation)
                        {
                            self.reject_duplicate_placement(existing, "Prop");
                            return;
                        }
                        self.push_undo();
                        let id = self
                            .create_model_entity_at_room_hit(room_id, model_id, &name, hit_world);
                        self.replace_node_selection(id);
                        self.clear_resource_selection_state();
                        self.status = "Placed Prop".to_string();
                        self.mark_dirty();
                        self.return_to_select_after_place();
                        return;
                    }
                    Err(message) => {
                        self.status = message;
                        return;
                    }
                }
            }
            PlaceKind::Character => match self.resolve_place_character_resource() {
                Ok((character_id, name, character)) => {
                    if let Some(existing) =
                        self.find_duplicate_character_entity(room_id, character_id, translation)
                    {
                        self.reject_duplicate_placement(existing, "Character");
                        return;
                    }
                    let player = match character.spawn_role {
                        psxed_project::CharacterSpawnRole::Auto => !self.has_player_source(),
                        psxed_project::CharacterSpawnRole::Player => true,
                        psxed_project::CharacterSpawnRole::Enemy => false,
                    };
                    let idle_clip = self.resolve_character_idle_preview_clip(&character);
                    let settings = CharacterControllerSettings::from_character(&character);
                    let camera_settings = character.camera_settings();
                    self.push_undo();
                    if player && character.spawn_role == psxed_project::CharacterSpawnRole::Player {
                        self.demote_player_sources_except(None);
                    }
                    let id = self.create_character_entity_at_room_hit(
                        room_id,
                        character_id,
                        &name,
                        character.model,
                        character.material,
                        idle_clip,
                        settings,
                        player,
                        camera_settings,
                        hit_world,
                    );
                    self.replace_node_selection(id);
                    self.clear_resource_selection_state();
                    self.status = if player {
                        "Placed Player Character".to_string()
                    } else {
                        "Placed Character".to_string()
                    };
                    self.mark_dirty();
                    self.return_to_select_after_place();
                    return;
                }
                Err(message) => {
                    self.status = message;
                    return;
                }
            },
            PlaceKind::ImageProp => match self.resolve_place_image_prop_material() {
                Ok((material_id, name)) => {
                    let size = image_prop_default_size_for_sector(sector_size_i);
                    if let Some(existing) =
                        self.find_duplicate_image_prop(room_id, material_id, translation)
                    {
                        self.reject_duplicate_placement(existing, "Image Prop");
                        return;
                    }
                    (
                        format!("{name} Image"),
                        NodeKind::ImageProp {
                            material: Some(material_id),
                            width: size,
                            height: size,
                            cylindrical_billboard: false,
                            collision_enabled: false,
                            collision_size: [size, size, size],
                            destructible: None,
                        },
                    )
                }
                Err(message) => {
                    self.status = message;
                    return;
                }
            },
            PlaceKind::BoxProp => {
                let material = self.resolve_place_box_prop_material();
                let material_id = material.as_ref().map(|(id, _)| *id);
                let size = image_prop_default_size_for_sector(sector_size_i);
                if let Some(existing) =
                    self.find_duplicate_box_prop(room_id, material_id, translation)
                {
                    self.reject_duplicate_placement(existing, "Box Prop");
                    return;
                }
                let name = material
                    .as_ref()
                    .map(|(_, name)| format!("{name} Box"))
                    .unwrap_or_else(|| "Box Prop".to_string());
                (
                    name,
                    NodeKind::BoxProp {
                        materials: [material_id; psxed_project::BOX_PROP_FACE_COUNT],
                        uvs: [UvTransform::IDENTITY; psxed_project::BOX_PROP_FACE_COUNT],
                        vertices: psxed_project::box_prop_vertices_for_size(size),
                        collision_enabled: true,
                        break_flags: 0,
                        erosion: psxed_project::BoxPropErosion::default(),
                    },
                )
            }
            PlaceKind::CylinderProp => {
                let material = self.resolve_place_box_prop_material();
                let material_id = material.as_ref().map(|(id, _)| *id);
                let size = image_prop_default_size_for_sector(sector_size_i);
                if let Some(existing) =
                    self.find_duplicate_cylinder_prop(room_id, material_id, translation)
                {
                    self.reject_duplicate_placement(existing, "Cylinder Prop");
                    return;
                }
                let name = material
                    .as_ref()
                    .map(|(_, name)| format!("{name} Column"))
                    .unwrap_or_else(|| "Cylinder Prop".to_string());
                let geometry = psxed_project::CylinderPropGeometry {
                    radius: [size / 2, size / 2],
                    height: size.saturating_mul(2),
                    ..Default::default()
                };
                (
                    name,
                    NodeKind::CylinderProp {
                        materials: [material_id; psxed_project::CYLINDER_PROP_MATERIAL_COUNT],
                        uvs: [UvTransform::IDENTITY; psxed_project::CYLINDER_PROP_MATERIAL_COUNT],
                        geometry,
                        collision_enabled: true,
                    },
                )
            }
            PlaceKind::ArchProp => {
                let material = self.resolve_place_box_prop_material();
                let material_id = material.as_ref().map(|(id, _)| *id);
                let name = material
                    .as_ref()
                    .map(|(_, name)| format!("{name} Arch"))
                    .unwrap_or_else(|| "Arch Prop".to_string());
                (
                    name,
                    NodeKind::ArchProp {
                        materials: [material_id; psxed_project::ARCH_PROP_MATERIAL_COUNT],
                        uvs: [UvTransform::IDENTITY; psxed_project::ARCH_PROP_MATERIAL_COUNT],
                        geometry: psxed_project::ArchPropGeometry::default(),
                        collision_enabled: false,
                    },
                )
            }
            PlaceKind::PointLightMarker => {
                if let Some(existing) = self.find_duplicate_point_light(room_id, translation) {
                    self.reject_duplicate_placement(existing, "Point Light");
                    return;
                }
                (
                    "Point Light".to_string(),
                    NodeKind::PointLight {
                        color: [255, 240, 200],
                        intensity: 1.0,
                        // Sectors. Matches the Add Child default
                        // and covers a typical 4×4 sector room.
                        radius: 4.0,
                    },
                )
            }
            PlaceKind::ParticleEmitter => {
                if let Some(existing) = self.find_duplicate_particle_emitter(room_id, translation) {
                    self.reject_duplicate_placement(existing, "Particle Emitter");
                    return;
                }
                (
                    "Particle Emitter".to_string(),
                    NodeKind::ParticleEmitter {
                        settings: ParticleEmitterSettings::default(),
                    },
                )
            }
            PlaceKind::Logic => (
                // Default to a trigger volume; the inspector's
                // kind picker switches it to relay/multisource/
                // door. Rename the node to give the record its
                // targetname.
                "Trigger Volume".to_string(),
                NodeKind::Logic {
                    kind: psxed_project::LogicNodeKind::default(),
                    target: String::new(),
                    killtarget: String::new(),
                    master: String::new(),
                    delay_ticks: 0,
                    // Fire once, then retire (hl's `wait -1`). A wait-0
                    // volume re-activates on EVERY tick the player stands
                    // inside it, which soft-locks any overlay it opens.
                    // Authored projects keep whatever they already store.
                    wait_ticks: -1,
                    enabled: true,
                },
            ),
            PlaceKind::Destructible => (
                "Destructible".to_string(),
                NodeKind::Destructible {
                    max_health: psxed_project::default_destructible_max_health(),
                    damage_affinity: psxed_project::DestructibleDamageAffinity::Both,
                    enabled: true,
                },
            ),
            PlaceKind::HookPoint => ("Hook Point".to_string(), NodeKind::HookPoint),
            PlaceKind::VitalityCircle => (
                "Horizon Vitality Circle".to_string(),
                NodeKind::VitalityCircle {
                    axis: psxed_project::VitalityCircleAxis::Horizon,
                    radius: psxed_project::default_vitality_circle_radius(),
                    refill_per_second: psxed_project::default_vitality_circle_refill_rate(),
                    drain_per_second: psxed_project::default_vitality_circle_drain_rate(),
                    enabled: true,
                },
            ),
            PlaceKind::PointOfInterest => unreachable!("handled above"),
        };
        let translation = self.snap_placed_translation(&node_kind, translation);
        self.push_undo();
        let id = self
            .project
            .active_scene_mut()
            .add_node(room_id, default_name, node_kind);
        if let Some(node) = self.project.active_scene_mut().node_mut(id) {
            node.transform.translation = translation;
            let arch_geometry = match &node.kind {
                NodeKind::ArchProp { geometry, .. } => Some(*geometry),
                _ => None,
            };
            if let Some(geometry) = arch_geometry {
                crate::inspector_transform_node::snap_arch_prop_transform(
                    &mut node.transform,
                    geometry,
                    arch_tile_size,
                );
            }
        }
        self.replace_node_selection(id);
        self.clear_resource_selection_state();
        self.status = format!("Placed {}", kind.label());
        self.mark_dirty();
        self.return_to_select_after_place();
    }

    pub(crate) fn apply_selected_box_prop_resource_click(&mut self, click: ResourceClick) -> bool {
        let plain_click =
            !click.modifiers.shift && !click.modifiers.ctrl && !click.modifiers.command;
        if !plain_click {
            return false;
        }
        let Some(assignment) = self.assign_selected_box_props_resource(click.id) else {
            return false;
        };
        self.status = match (assignment.targets, assignment.updated) {
            (1, 0) => "Material already assigned to selected Prop".to_string(),
            (1, _) => "Assigned material to Prop".to_string(),
            (_, 0) => "Material already assigned to selected Props".to_string(),
            (total, updated) if total == updated => {
                format!("Assigned material to {updated} selected Props")
            }
            (total, updated) => {
                format!("Assigned material to {updated}/{total} selected Props")
            }
        };
        self.clear_resource_selection_state();
        true
    }

    pub(crate) fn assign_selected_box_props_resource(
        &mut self,
        resource_id: ResourceId,
    ) -> Option<BoxPropMaterialAssignment> {
        let targets = self.selected_box_prop_nodes();
        let target_count = targets.len();
        if target_count == 0 {
            return None;
        }
        let material = match self.project.resource(resource_id).map(|r| &r.data) {
            Some(ResourceData::Material(_)) => resource_id,
            _ => return None,
        };
        let needs_update = targets
            .iter()
            .any(|id| self.box_prop_materials_differ(*id, material));
        if !needs_update {
            return Some(BoxPropMaterialAssignment {
                material,
                targets: target_count,
                updated: 0,
            });
        }

        self.push_undo();
        let updated = self.assign_box_prop_nodes_material_no_undo(&targets, material);
        if updated > 0 {
            self.mark_dirty();
        }
        Some(BoxPropMaterialAssignment {
            material,
            targets: target_count,
            updated,
        })
    }

    pub(crate) fn selected_box_prop_nodes(&self) -> Vec<NodeId> {
        self.selected_node_ids_in_hierarchy()
            .into_iter()
            .filter(|id| {
                self.project.active_scene().node(*id).is_some_and(|node| {
                    matches!(
                        node.kind,
                        NodeKind::BoxProp { .. } | NodeKind::CylinderProp { .. }
                    )
                })
            })
            .collect()
    }

    pub(crate) fn box_prop_materials_differ(&self, id: NodeId, material: ResourceId) -> bool {
        self.project
            .active_scene()
            .node(id)
            .is_some_and(|node| match &node.kind {
                NodeKind::BoxProp { materials, .. } => {
                    materials.iter().any(|slot| *slot != Some(material))
                }
                NodeKind::CylinderProp { materials, .. } => {
                    materials.iter().any(|slot| *slot != Some(material))
                }
                _ => false,
            })
    }

    pub(crate) fn assign_box_prop_nodes_material_no_undo(
        &mut self,
        targets: &[NodeId],
        material: ResourceId,
    ) -> usize {
        let scene = self.project.active_scene_mut();
        let mut updated = 0usize;
        for id in targets {
            let Some(node) = scene.node_mut(*id) else {
                continue;
            };
            match &mut node.kind {
                NodeKind::BoxProp { materials, .. } => {
                    if materials.iter().all(|slot| *slot == Some(material)) {
                        continue;
                    }
                    *materials = [Some(material); psxed_project::BOX_PROP_FACE_COUNT];
                }
                NodeKind::CylinderProp { materials, .. } => {
                    if materials.iter().all(|slot| *slot == Some(material)) {
                        continue;
                    }
                    *materials = [Some(material); psxed_project::CYLINDER_PROP_MATERIAL_COUNT];
                }
                _ => continue,
            }
            updated += 1;
        }
        updated
    }
}
