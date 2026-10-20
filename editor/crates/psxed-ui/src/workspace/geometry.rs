use super::*;

fn project_center_half_2d(
    view: OrthographicView,
    center: [f32; 3],
    half: [f32; 3],
) -> ([f32; 2], [f32; 2]) {
    (view.project_f32(center), view.project_f32(half))
}

impl EditorWorkspace {
    /// Whether the active scene has a world to frame: a grid with a sector.
    pub(crate) fn has_world_geometry(&self) -> bool {
        self.project
            .active_scene()
            .world
            .grid()
            .is_some_and(|g| g.sector_count() > 0)
    }

    /// Give projects created before camera authoring metadata a usable first
    /// 3D view. The exact default-state checks keep authored cameras and any
    /// in-session camera movement untouched.
    pub(crate) fn frame_camera_if_uninitialized(&mut self) -> bool {
        let default_camera = EditorCameraState::default();
        if !self.has_world_geometry() || self.current_editor_camera_state() != default_camera {
            return false;
        }
        let Some((center, half)) = self.all_world_frame_bounds_3d() else {
            return false;
        };

        // A wrapped negative pitch places the orbit camera above the level.
        self.camera_rig.pitch = 3840;
        self.frame_3d_bounds(center, half);
        true
    }

    pub(crate) fn frame_viewport_if_uninitialized(&mut self) -> bool {
        let untouched_focus = self
            .orthographic_focus
            .into_iter()
            .all(|value| value.abs() <= f32::EPSILON);
        let untouched_zoom = (self.viewport_zoom - DEFAULT_VIEWPORT_ZOOM).abs() <= f32::EPSILON;
        if self.has_world_geometry() && untouched_focus && untouched_zoom {
            self.frame_viewport();
            return true;
        }
        false
    }

    /// Stop a transient character action/movement preview when its Animator is
    /// edited. The transient preview carries its own clip override, so leaving
    /// it alive would mask a newly selected `Editor Clip` until the project was
    /// reopened (which happened to clear the preview state).
    pub(crate) fn reconcile_character_preview_after_node_kind_edit(
        &mut self,
        edited_node: NodeId,
        before: &NodeKind,
    ) {
        if !matches!(before, NodeKind::Animator { .. }) {
            return;
        }
        let scene = self.project.active_scene();
        let Some(node) = scene.node(edited_node) else {
            return;
        };
        if node.kind == *before || !matches!(node.kind, NodeKind::Animator { .. }) {
            return;
        }
        let Some(entity) = node.parent else {
            return;
        };
        if self
            .character_motion_preview
            .is_some_and(|preview| preview.entity == entity)
        {
            self.character_motion_preview = None;
        }
    }

    pub(crate) fn preview_character_action(
        &mut self,
        selected: NodeId,
        action: psxed_project::CharacterAnimationAction,
    ) -> bool {
        let (host, animator) = {
            let scene = self.project.active_scene();
            let Some(selected_node) = scene.node(selected) else {
                return false;
            };
            let host = if matches!(selected_node.kind, NodeKind::Entity) {
                selected
            } else if matches!(selected_node.kind, NodeKind::CharacterController { .. }) {
                let Some(parent) = selected_node.parent else {
                    self.status = "Character Controller has no owning Entity".to_string();
                    return false;
                };
                parent
            } else {
                return false;
            };
            let Some(host_node) = scene.node(host) else {
                return false;
            };
            let animator = host_node.children.iter().find_map(|child| {
                scene
                    .node(*child)
                    .filter(|node| matches!(node.kind, NodeKind::Animator { .. }))
                    .map(|node| node.id)
            });
            (host, animator)
        };

        let Some(animator) = animator else {
            self.status = "Add an Animator component to preview character actions".to_string();
            return false;
        };

        let local_clip =
            self.project
                .active_scene()
                .node(animator)
                .and_then(|node| match &node.kind {
                    NodeKind::Animator { action_clips, .. } => action_clips
                        .iter()
                        .find(|binding| binding.action == action)
                        .map(|binding| binding.clip),
                    _ => None,
                });
        let context = selected_animator_clip_context(&self.project, animator, &self.project_dir);
        let clip = local_clip.or_else(|| {
            context
                .as_ref()
                .and_then(|ctx| ctx.profile_action_clips[action.to_index()])
        });
        let Some(clip) = clip else {
            self.status = format!(
                "{} has no effective animation clip; bind one on Animator",
                action.label()
            );
            return false;
        };
        let clip_name = context
            .as_ref()
            .and_then(|ctx| ctx.clips.get(clip as usize))
            .cloned()
            .unwrap_or_else(|| format!("Clip {clip}"));

        self.character_motion_preview = Some(CharacterMotionPreviewState {
            entity: host,
            action,
            clip,
            started_at: Instant::now(),
        });
        self.status = format!("Previewing {} · {clip_name}", action.label());
        false
    }

    /// Single dispatch point for primary-button clicks on the viewport.
    pub(crate) fn handle_viewport_click(
        &mut self,
        world: [f32; 2],
        hits: &[ViewportHit],
        modifiers: egui::Modifiers,
    ) {
        match self.active_tool {
            ViewTool::Select => {
                if let Some(hit) = hits.iter().rev().find(|hit| hit.contains(world)) {
                    self.select_node_with_modifiers(hit.id, modifiers);
                } else {
                    self.clear_resource_selection_state();
                }
            }
            ViewTool::Place => {
                self.place_node_from_top(world);
            }
        }
    }

    pub(crate) fn duplicate_current_selection(&mut self) {
        self.duplicate_selected();
    }

    pub(crate) fn draw_clipboard_notice(&mut self, ctx: &egui::Context) {
        const NOTICE_SECONDS: f32 = 3.0;
        let Some(notice) = self.clipboard_notice.clone() else {
            return;
        };
        let elapsed = notice.shown_at.elapsed().as_secs_f32();
        if elapsed >= NOTICE_SECONDS {
            self.clipboard_notice = None;
            return;
        }
        let accent = if notice.success {
            STUDIO_SUCCESS
        } else {
            STUDIO_ERROR
        };
        let fill = if notice.success {
            STUDIO_SUCCESS_DIM
        } else {
            STUDIO_ERROR_DIM
        };
        egui::Area::new(egui::Id::new("geometry_clipboard_notice"))
            .anchor(egui::Align2::CENTER_TOP, Vec2::new(0.0, 72.0))
            .order(egui::Order::Foreground)
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(fill)
                    .stroke(egui::Stroke::new(2.0, accent))
                    .corner_radius(egui::CornerRadius::same(8))
                    .inner_margin(egui::Margin::symmetric(18, 10))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(notice.message)
                                .strong()
                                .size(16.0)
                                .color(egui::Color32::WHITE),
                        );
                    });
            });
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
    }

    pub(crate) fn add_child(&mut self, kind: NodeKind, name: &str) {
        let parent = self.selection.selected_node;
        if kind.is_component() {
            let scene = self.project.active_scene();
            let Some(host) = scene.node(parent) else {
                self.status = format!("Cannot add {name}: no selected host node");
                return;
            };
            if !component_can_be_added_to_host(&host.kind, &kind, scene, parent) {
                self.status = format!("Cannot add {name} to {}", host.name);
                return;
            }
        }
        self.push_undo();
        let id = self
            .project
            .active_scene_mut()
            .add_node(parent, name.to_string(), kind);
        self.replace_node_selection(id);
        self.clear_resource_selection_state();
        self.status = format!("Added {name}");
        self.mark_dirty();
    }

    pub(crate) fn add_ui_child(&mut self, kind: UiNodeKind, name: &str) {
        self.push_undo();
        let parent = self.selection.selected_ui_node;
        let Some(scene) = self.current_ui_scene_mut() else {
            self.status = "No UI scene available".to_string();
            return;
        };
        let id = scene.add_node(parent, name.to_string(), kind);
        self.selection.selected_ui_node = id;
        self.clear_resource_selection_state();
        self.status = format!("Added UI {name}");
        self.mark_dirty();
    }

    pub(crate) fn duplicate_selected(&mut self) {
        let selected = {
            let scene = self.project.active_scene();
            let mut roots = Vec::new();
            for id in self.selected_node_ids_in_hierarchy() {
                // A POI component has no transform or beacon of its own: its
                // Entity parent is the authored object. Placement initially
                // selects the component, so duplicating that row must copy the
                // complete host instead of adding a second component to the
                // original host.
                let root = scene
                    .node(id)
                    .and_then(|node| {
                        matches!(node.kind, NodeKind::PointOfInterest { .. })
                            .then_some(node.parent)
                            .flatten()
                    })
                    .unwrap_or(id);
                if roots
                    .iter()
                    .any(|ancestor| scene.is_descendant_of(root, *ancestor))
                {
                    continue;
                }
                roots.push(root);
            }
            roots
        };
        if selected.is_empty() {
            return;
        }
        self.push_undo();
        let mut duplicated = Vec::new();
        for selected in selected {
            let Some(source) = self.project.active_scene().node(selected).cloned() else {
                continue;
            };
            if matches!(source.kind, NodeKind::Group) || !source.children.is_empty() {
                let scene = self.project.active_scene();
                let subtree: Vec<_> = scene
                    .hierarchy_rows()
                    .into_iter()
                    .filter(|row| row.id == selected || scene.is_descendant_of(row.id, selected))
                    .filter_map(|row| scene.node(row.id).cloned())
                    .collect();
                let mut remap = HashMap::new();
                for node in subtree {
                    let parent = if node.id == selected {
                        source.parent.unwrap_or(NodeId::ROOT)
                    } else {
                        node.parent
                            .and_then(|parent| remap.get(&parent).copied())
                            .unwrap_or(source.parent.unwrap_or(NodeId::ROOT))
                    };
                    let name = if node.id == selected {
                        format!("{} Copy", node.name)
                    } else {
                        node.name.clone()
                    };
                    let mut kind = node.kind;
                    if let NodeKind::PointOfInterest { persistence_id, .. } = &mut kind {
                        // Save identity is not copyable state. An empty id is
                        // cooked from the new host's stable NodeId, avoiding
                        // duplicate persistence keys after duplication.
                        persistence_id.clear();
                    }
                    let id = self.project.active_scene_mut().add_node(parent, name, kind);
                    if let Some(copy) = self.project.active_scene_mut().node_mut(id) {
                        copy.transform = node.transform;
                    }
                    remap.insert(node.id, id);
                }
                if let Some(root) = remap.get(&selected).copied() {
                    duplicated.push(root);
                }
                continue;
            }
            let parent = source.parent.unwrap_or(NodeId::ROOT);
            // A light is "Light 4", not "Point Light Copy Copy".
            let name = if matches!(source.kind, NodeKind::PointLight { .. }) {
                next_light_name(self.project.active_scene())
            } else {
                format!("{} Copy", source.name)
            };
            let id = self
                .project
                .active_scene_mut()
                .add_node(parent, name, source.kind);
            if let Some(node) = self.project.active_scene_mut().node_mut(id) {
                node.transform = source.transform;
            }
            duplicated.push(id);
        }
        if duplicated.is_empty() {
            return;
        }
        self.selection.selected_nodes = duplicated.iter().copied().collect();
        self.selection.selected_node = duplicated[0];
        self.selection.node_selection_anchor = duplicated.last().copied();
        self.clear_resource_selection_state();
        self.status = if duplicated.len() == 1 {
            "Duplicated node".to_string()
        } else {
            format!("Duplicated {} nodes", duplicated.len())
        };
        self.mark_dirty();
    }

    pub(crate) fn delete_selected(&mut self) {
        let selected = self.selected_node_ids_in_hierarchy();
        if selected.is_empty() {
            return;
        }
        self.push_undo();
        let mut removed = 0usize;
        for id in selected.iter().rev() {
            if self.project.active_scene_mut().remove_node(*id) {
                removed += 1;
            }
        }
        if removed > 0 {
            self.clear_node_selection_state();
            self.clear_resource_selection_state();
            self.status = if removed == 1 {
                "Deleted node".to_string()
            } else {
                format!("Deleted {removed} nodes")
            };
            self.mark_dirty();
        }
    }

    pub(crate) fn draw_component_authoring_panel(
        &mut self,
        ui: &mut egui::Ui,
        selected: NodeId,
        character_options: &[(ResourceId, String)],
        nav_target: &mut Option<ResourceId>,
        preview_action: &mut Option<psxed_project::CharacterAnimationAction>,
    ) -> bool {
        let scene = self.project.active_scene();
        let Some(node) = scene.node(selected) else {
            return false;
        };

        let is_host = matches!(node.kind, NodeKind::Entity);
        let is_component = node.kind.is_component();
        if !is_host && !is_component {
            return false;
        }

        if is_component {
            let parent = node
                .parent
                .and_then(|parent| scene.node(parent).map(|node| (node.id, node.name.clone())));
            egui::CollapsingHeader::new(icons::label(icons::LAYERS, "Relationship"))
                .default_open(false)
                .show(ui, |ui| {
                    if let Some((parent_id, parent_name)) = &parent {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Host").color(STUDIO_TEXT_WEAK));
                            if ui.button(parent_name).clicked() {
                                self.replace_node_selection(*parent_id);
                                self.clear_resource_selection_state();
                            }
                        });
                    } else {
                        ui.weak("Component has no host parent.");
                    }
                });
            return false;
        }

        let host_kind = node.kind.clone();
        let components: Vec<(NodeId, String, &'static str)> = node
            .children
            .iter()
            .filter_map(|id| scene.node(*id))
            .filter(|child| child.kind.is_component())
            .map(|child| (child.id, child.name.clone(), child.kind.label()))
            .collect();
        let existing: Vec<&NodeKind> = node
            .children
            .iter()
            .filter_map(|id| scene.node(*id))
            .filter(|child| child.kind.is_component())
            .map(|child| &child.kind)
            .collect();
        let addable = addable_component_templates(&host_kind, &existing);

        let mut add_component = None;
        let mut select_component = None;
        let mut promoted_controller = None;
        let mut changed = false;
        egui::CollapsingHeader::new(icons::label(icons::LAYERS, "Components"))
            .default_open(true)
            .show(ui, |ui| {
                if components.is_empty() {
                    ui.weak("No components attached.");
                } else {
                    for (id, name, kind) in &components {
                        let is_character_controller = *kind == "Character Controller";
                        let title = if name == kind {
                            name.clone()
                        } else {
                            format!("{name} · {kind}")
                        };
                        inspector_section(
                            ui,
                            ("entity-component", selected.raw(), id.raw()),
                            node_lucide_icon(kind, false),
                            &title,
                            is_character_controller,
                            |ui| {
                                if is_character_controller {
                                    // Resolve the type's tuning before taking
                                    // the mutable scene borrow.
                                    let inherited = self
                                        .project
                                        .active_scene()
                                        .node(*id)
                                        .and_then(|node| match &node.kind {
                                            NodeKind::CharacterController { character, .. } => {
                                                *character
                                            }
                                            _ => None,
                                        })
                                        .and_then(|cid| self.project.resource(cid))
                                        .and_then(|resource| match &resource.data {
                                            psxed_project::ResourceData::Character(character) => {
                                                Some(CharacterControllerSettings::from_character(
                                                    character,
                                                ))
                                            }
                                            _ => None,
                                        })
                                        .unwrap_or_default();
                                    let (component_changed, became_player) = {
                                        let Some(node) =
                                            self.project.active_scene_mut().node_mut(*id)
                                        else {
                                            ui.colored_label(
                                                Color32::from_rgb(220, 120, 100),
                                                "Component no longer exists.",
                                            );
                                            return;
                                        };
                                        let NodeKind::CharacterController {
                                            loadout: _,
                                            character,
                                            settings,
                                            player,
                                        } = &mut node.kind
                                        else {
                                            return;
                                        };
                                        let was_player = *player;
                                        let edited = ui
                                            .push_id(
                                                ("inline-character-controller", id.raw()),
                                                |ui| {
                                                    draw_character_controller_editor(
                                                        ui,
                                                        character,
                                                        settings.get_or_insert(inherited),
                                                        player,
                                                        character_options,
                                                        nav_target,
                                                        preview_action,
                                                    )
                                                },
                                            )
                                            .inner;
                                        (edited, !was_player && *player)
                                    };
                                    changed |= component_changed;
                                    if became_player {
                                        promoted_controller = Some(*id);
                                    }
                                } else {
                                    ui.label(
                                        RichText::new(
                                            "This component keeps its dedicated settings editor.",
                                        )
                                        .color(STUDIO_TEXT_WEAK)
                                        .small(),
                                    );
                                }
                                if ui
                                    .button(icons::label(icons::POINTER, "Open full settings"))
                                    .on_hover_text("Select this component in the Scene Graph")
                                    .clicked()
                                {
                                    select_component = Some(*id);
                                }
                            },
                        );
                    }
                }

                ui.separator();
                ui.menu_button(icons::label(icons::PLUS, "Add Component"), |ui| {
                    if addable.is_empty() {
                        ui.weak("All singleton components are already present.");
                    }
                    for (label, kind) in &addable {
                        if ui.button(*label).clicked() {
                            add_component = Some((*label, kind.clone()));
                            ui.close_menu();
                        }
                    }
                });
            });

        if let Some(id) = select_component {
            self.replace_node_selection(id);
            self.clear_resource_selection_state();
        }
        if let Some((label, kind)) = add_component {
            self.add_component_to_host(selected, label, kind);
        }
        if let Some(controller) = promoted_controller {
            self.demote_player_sources_except(Some(controller));
        }
        changed
    }

    #[cfg(test)]
    pub(crate) fn set_character_controller_player_controlled(
        &mut self,
        controller: NodeId,
        player: bool,
    ) {
        let Some(current) =
            self.project
                .active_scene()
                .node(controller)
                .and_then(|node| match &node.kind {
                    NodeKind::CharacterController { player, .. } => Some(*player),
                    _ => None,
                })
        else {
            self.status = "Selected component is not a Character Controller".to_string();
            return;
        };
        if current == player {
            return;
        }

        self.push_undo();
        if player {
            self.demote_player_sources_except(Some(controller));
        }
        let Some(node) = self.project.active_scene_mut().node_mut(controller) else {
            self.status = "Character Controller no longer exists".to_string();
            return;
        };
        let NodeKind::CharacterController {
            player: current, ..
        } = &mut node.kind
        else {
            self.status = "Selected component is not a Character Controller".to_string();
            return;
        };
        *current = player;
        self.status = if player {
            "Marked Character Controller as player controlled".to_string()
        } else {
            "Cleared player control from Character Controller".to_string()
        };
        self.mark_dirty();
    }

    pub(crate) fn add_component_to_host(
        &mut self,
        host: NodeId,
        label: &'static str,
        kind: NodeKind,
    ) -> Option<NodeId> {
        if !kind.is_component() {
            self.status = "Only component nodes can be added as components".to_string();
            return None;
        }
        let scene = self.project.active_scene();
        let Some(host_node) = scene.node(host) else {
            self.status = "Component host no longer exists".to_string();
            return None;
        };
        if !matches!(host_node.kind, NodeKind::Entity) {
            self.status = "Components can only be added to Entity nodes".to_string();
            return None;
        }
        if !component_can_be_added_to_host(&host_node.kind, &kind, scene, host) {
            self.status = format!("{label} is already present or invalid for this host");
            return None;
        }

        self.push_undo();
        let id = self
            .project
            .active_scene_mut()
            .add_node(host, label.to_string(), kind);
        self.replace_node_selection(id);
        self.clear_resource_selection_state();
        self.status = format!("Added {label} component");
        self.mark_dirty();
        Some(id)
    }

    pub(crate) fn open_new_project_dialog(&mut self) {
        self.modal = Modal::NewProject {
            name: String::new(),
            error: None,
        };
    }

    pub(crate) fn open_texture_import_dialog(&mut self) {
        self.texture_import_dialog.open = true;
        self.texture_import_dialog.status = None;
        self.retire_texture_import_preview();
    }

    pub(crate) fn open_model_import_dialog(&mut self) {
        // The imported model lands in the Assets drawer.
        self.resources_open = true;
        self.model_import_dialog.open = true;
        self.model_import_dialog.status = None;
        self.retire_model_import_preview();
        self.model_import_dialog.selected_clip = 0;
    }

    pub(crate) fn catalogue_animation_source_folder(&mut self) {
        let mut dialog = rfd::FileDialog::new().set_title("Choose animation source folder");
        if self.project_dir.is_dir() {
            dialog = dialog.set_directory(&self.project_dir);
        }
        let Some(path) = dialog.pick_folder() else {
            return;
        };
        self.catalogue_animation_source_path(&path);
    }

    pub(crate) fn catalogue_animation_source_zip(&mut self) {
        let mut dialog = rfd::FileDialog::new()
            .set_title("Choose animation source zip")
            .add_filter("Animation source zip", &["zip"]);
        if self.project_dir.is_dir() {
            dialog = dialog.set_directory(&self.project_dir);
        }
        let Some(path) = dialog.pick_file() else {
            return;
        };
        self.catalogue_animation_source_path(&path);
    }

    pub(crate) fn catalogue_animation_source_path(&mut self, path: &Path) {
        match catalogue_animation_sources_from_path(&mut self.project, &self.project_dir, path) {
            Ok(report) => {
                self.status = format!(
                    "Catalogued animation sources: {} found, {} added, {} updated",
                    report.source_candidates, report.sources_added, report.sources_updated
                );
                if report.changed() {
                    self.mark_dirty();
                }
            }
            Err(error) => {
                self.status = format!("Animation source catalogue failed: {error}");
            }
        }
    }

    pub(crate) fn handle_animation_viewer_action(
        &mut self,
        action: model_animation_viewer::AnimationViewerAction,
    ) {
        match action {
            model_animation_viewer::AnimationViewerAction::BakeSourceForModel {
                model_id,
                source_id,
            } => {
                let Some((model_source, world_height)) =
                    self.project.resource(model_id).and_then(|resource| {
                        let ResourceData::Model(model) = &resource.data else {
                            return None;
                        };
                        Some((model.source_path.clone(), model.world_height))
                    })
                else {
                    self.status = format!("Model #{} is not available", model_id.raw());
                    return;
                };
                let Some(model_source) = model_source.filter(|path| !path.trim().is_empty()) else {
                    self.status = format!(
                        "Model #{} has no source path. Set it in the Model inspector or reimport the model.",
                        model_id.raw()
                    );
                    return;
                };
                let Some(animation_source) =
                    self.project.resource(source_id).and_then(|resource| {
                        let ResourceData::AnimationSource(source) = &resource.data else {
                            return None;
                        };
                        Some(source.source_path.clone())
                    })
                else {
                    self.status = format!("Animation source #{} is not available", source_id.raw());
                    return;
                };

                let temp_dir = match make_animation_bake_temp_dir() {
                    Ok(path) => path,
                    Err(error) => {
                        self.status = format!("Animation bake failed: {error}");
                        return;
                    }
                };
                let result = (|| {
                    let model_source_path = materialize_authoring_source_path(
                        &model_source,
                        &self.project_dir,
                        &temp_dir,
                    )?;
                    let animation_source_path = materialize_authoring_source_path(
                        &animation_source,
                        &self.project_dir,
                        &temp_dir,
                    )?;
                    let config = psxed_project::model_import::RigidModelConfig {
                        world_height,
                        extra_animations_affect_bounds: false,
                        ..Default::default()
                    };
                    psxed_project::model_import::bake_animation_source_for_model(
                        &mut self.project,
                        model_id,
                        source_id,
                        &model_source_path,
                        &animation_source_path,
                        &self.project_dir,
                        config,
                    )
                    .map_err(|error| error.to_string())
                })();
                let _ = std::fs::remove_dir_all(&temp_dir);
                match result {
                    Ok(clip_id) => {
                        self.animation_viewer.focus_resource(&self.project, clip_id);
                        self.animation_viewer_preview_texture = None;
                        // Baking writes cooked files as well as mutating the
                        // document. A document-only undo could not restore the
                        // matching filesystem state, so start a fresh history
                        // epoch instead of recording a misleading transaction.
                        self.history.clear();
                        self.inspector_undo_transaction = None;
                        self.mark_dirty();
                        self.status = format!("Baked animation clip #{}", clip_id.raw());
                    }
                    Err(error) => {
                        self.status = format!("Animation bake failed: {error}");
                    }
                }
            }
            model_animation_viewer::AnimationViewerAction::ProjectChanged => {
                self.mark_dirty();
            }
        }
    }

    pub(crate) fn mark_dirty(&mut self) {
        self.dirty = true;
        // Grid caches (picking, budget) follow the document revision.
        self.grid.rev = self.grid.rev.wrapping_add(1);
        // Any authoring change makes the exact last-cook numbers stale. The
        // Play menu immediately falls back to a fresh authored estimate until
        // Build/Play replaces it with another exact package report.
        self.last_playtest_budget = None;
        self.clear_validation_issues();
    }

    pub(crate) fn commit_resource_rename(&mut self, id: ResourceId, name: String) {
        let Some(current_name) = self.project.resource_name(id).map(str::to_string) else {
            self.resource_renaming = None;
            self.status = format!("Resource #{} no longer exists", id.raw());
            return;
        };

        let final_name = name.trim();
        if final_name.is_empty() {
            self.resource_renaming = Some((id, current_name));
            self.status = "Resource name cannot be empty".to_string();
            return;
        }
        if final_name == current_name {
            self.resource_renaming = Some((id, current_name));
            return;
        }

        let before = self.project.clone();
        match self
            .project
            .rename_resource_with_files(id, final_name, &self.project_dir)
        {
            Ok(report) => {
                if report.renamed_files.is_empty() {
                    self.history.record(before);
                } else {
                    self.history.clear();
                }
                self.resource_renaming = Some((id, final_name.to_string()));
                self.mark_dirty();

                let moved = report.renamed_files.len();
                let skipped = report.skipped_files.len();
                self.status = match (moved, skipped) {
                    (0, 0) => format!("Renamed {final_name}"),
                    (m, 0) => format!("Renamed {final_name}; moved {m} file(s)"),
                    (0, s) => format!("Renamed {final_name}; skipped {s} file path(s)"),
                    (m, s) => {
                        format!("Renamed {final_name}; moved {m} file(s), skipped {s} path(s)")
                    }
                };
            }
            Err(error) => {
                self.resource_renaming = Some((id, current_name));
                self.status = format!("Rename failed: {error}");
            }
        }
    }

    /// Snapshot the current project before a discrete mutation.
    /// Call once per user action -- paint click, place, add/delete
    /// node, etc -- so each undo step matches one author intent.
    pub(crate) fn push_undo(&mut self) {
        self.history.record(self.project.clone());
    }

    /// Drop an inspector coalescing token once its pointer drag or focused
    /// keyboard edit has ended. A new control then starts a fresh undo step.
    pub(crate) fn prepare_inspector_undo_frame(&mut self, ctx: &egui::Context) {
        self.prepare_inspector_undo(InspectorUndoInput::from_context(ctx));
    }

    pub(crate) fn prepare_inspector_undo(&mut self, input: InspectorUndoInput) {
        let Some(transaction) = self.inspector_undo_transaction.as_mut() else {
            return;
        };
        if input.pointer_down {
            if input.focused_widget.is_some() {
                transaction.focused_widget = input.focused_widget;
            }
            return;
        }
        if !input.wants_keyboard || input.focused_widget != transaction.focused_widget {
            self.inspector_undo_transaction = None;
        }
    }

    /// Record the pre-draw document when the Inspector mutated project data.
    /// Nested actions that explicitly touched history win: this preserves the
    /// non-undoable contract for filesystem-backed resource operations.
    pub(crate) fn finish_inspector_undo_frame(
        &mut self,
        before: ProjectDocument,
        history_epoch_before: u64,
        ctx: &egui::Context,
    ) {
        self.finish_inspector_undo(
            before,
            history_epoch_before,
            InspectorUndoInput::from_context(ctx),
        );
    }

    pub(crate) fn finish_inspector_undo(
        &mut self,
        before: ProjectDocument,
        history_epoch_before: u64,
        input: InspectorUndoInput,
    ) {
        if self.project == before {
            return;
        }
        if self.history.epoch() != history_epoch_before {
            self.inspector_undo_transaction = None;
            return;
        }

        // Focus may have moved while the Inspector was drawn, so close a stale
        // text transaction before deciding whether this edit needs a snapshot.
        self.prepare_inspector_undo(input);
        if self.inspector_undo_transaction.is_none() {
            self.history.record(before);
        }

        self.inspector_undo_transaction = if input.pointer_down || input.wants_keyboard {
            Some(InspectorUndoTransaction {
                focused_widget: input.focused_widget,
            })
        } else {
            None
        };
    }

    /// Pop the most recent snapshot back into `project`.
    /// Undo and redo replace the whole document. A drag still in flight
    /// holds indices and base copies from the OLD document, and its
    /// next preview or commit would write them over the restored one (or
    /// index past its end), so every live gesture ends here first.
    fn abandon_gestures_for_document_swap(&mut self) {
        self.interaction = Interaction::Idle;
        self.node_drag_2d = None;
        self.grid.abandon();
    }

    pub(crate) fn do_undo(&mut self) {
        self.abandon_gestures_for_document_swap();
        self.inspector_undo_transaction = None;
        if let Some(prev) = self.history.undo(self.project.clone()) {
            self.project = prev;
            self.clear_resource_selection_state();
            self.resource_renaming = None;
            self.reconcile_selection_after_document_change();
            self.status = "Undo".to_string();
            self.mark_dirty();
        } else {
            self.status = "Nothing to undo".to_string();
        }
    }

    pub(crate) fn do_redo(&mut self) {
        self.abandon_gestures_for_document_swap();
        self.inspector_undo_transaction = None;
        if let Some(next) = self.history.redo(self.project.clone()) {
            self.project = next;
            self.clear_resource_selection_state();
            self.resource_renaming = None;
            self.reconcile_selection_after_document_change();
            self.status = "Redo".to_string();
            self.mark_dirty();
        } else {
            self.status = "Nothing to redo".to_string();
        }
    }

    pub(crate) fn frame_viewport(&mut self) {
        if !self.view_2d {
            if let Some((center, half)) = self.current_frame_bounds_3d() {
                self.frame_3d_bounds(center, half);
                self.status = "Framed selection".to_string();
            } else {
                self.status = "Nothing to frame".to_string();
            }
            // The split layout shows the plan beside the 3D view: frame both.
            if self.is_grid_world() && self.grid.split {
                self.frame_plan();
            }
            return;
        }
        self.frame_plan();
    }

    /// Fit the plan (the orthographic view) to what is selected, or to the
    /// whole world.
    fn frame_plan(&mut self) {
        let Some((center, half)) = self.current_frame_bounds_2d() else {
            self.orthographic_focus = [0.0; 3];
            self.viewport_zoom = DEFAULT_VIEWPORT_ZOOM;
            self.status = "Reset viewport frame".to_string();
            return;
        };
        let content = [(half[0] * 2.0).max(1.0), (half[1] * 2.0).max(1.0)];
        let viewport = [
            self.last_viewport_size.x.max(320.0),
            self.last_viewport_size.y.max(240.0),
        ];
        let zoom_x = viewport[0] * 0.72 / content[0];
        let zoom_y = viewport[1] * 0.72 / content[1];
        self.viewport_zoom = zoom_x
            .min(zoom_y)
            .clamp(MIN_VIEWPORT_ZOOM, MAX_VIEWPORT_ZOOM);
        self.orthographic_focus = self
            .orthographic_view
            .with_projected_focus(self.orthographic_focus, center);
        self.status = "Framed selection".to_string();
    }

    pub(crate) fn frame_3d_bounds(&mut self, center: [f32; 3], half: [f32; 3]) {
        let target = center.map(round_to_i32);
        let radius = frame_radius_for_3d_bounds(half);
        self.camera_rig.target = target;
        self.camera_rig.radius = radius;

        if self.camera_rig.mode == ViewportCameraMode::Free {
            let forward =
                camera_forward_from_angles(self.camera_rig.free_yaw, self.camera_rig.free_pitch);
            self.camera_rig.free_position = [
                round_to_i32(target[0] as f32 - forward[0] * radius as f32),
                round_to_i32(target[1] as f32 - forward[1] * radius as f32),
                round_to_i32(target[2] as f32 - forward[2] * radius as f32),
            ];
            self.camera_rig.free_initialized = true;
        }
    }

    pub(crate) fn current_frame_bounds_3d(&self) -> Option<([f32; 3], [f32; 3])> {
        self.selected_frame_bounds_3d()
            .or_else(|| self.all_world_frame_bounds_3d())
    }

    pub(crate) fn selected_frame_bounds_3d(&self) -> Option<([f32; 3], [f32; 3])> {
        if let Some(bounds) = self.grid_selection_bounds() {
            return Some(bounds);
        }
        let selected_nodes = self.selected_node_ids_in_hierarchy();
        if selected_nodes.len() > 1 {
            let mut bounds = None;
            for id in selected_nodes {
                if let Some((center, half)) = self.node_frame_bounds_3d(id) {
                    merge_bounds_3d(&mut bounds, center, half);
                }
            }
            if let Some(bounds) = bounds {
                return Some(bounds_3d_to_center_half(bounds));
            }
        }

        if let Some(bounds) = self.node_frame_bounds_3d(self.selection.selected_node) {
            return Some(bounds);
        }
        None
    }

    fn all_world_frame_bounds_3d(&self) -> Option<([f32; 3], [f32; 3])> {
        let mut bounds = None;
        if let Some(grid) = self
            .project
            .active_scene()
            .world
            .grid()
            .and_then(|g| g.editor_bounds())
        {
            let center = core::array::from_fn(|i| (grid.min[i] as f32 + grid.max[i] as f32) * 0.5);
            let half = core::array::from_fn(|i| (grid.max[i] as f32 - grid.min[i] as f32) * 0.5);
            merge_bounds_3d(&mut bounds, center, half);
        }
        bounds.map(bounds_3d_to_center_half)
    }

    pub(crate) fn current_frame_bounds_2d(&self) -> Option<([f32; 2], [f32; 2])> {
        if let Some((center, half)) = self.grid_selection_bounds() {
            return Some(project_center_half_2d(self.orthographic_view, center, half));
        }
        let selected_nodes = self.selected_node_ids_in_hierarchy();
        if selected_nodes.len() > 1 {
            let mut bounds = None;
            for id in selected_nodes {
                if let Some((center, half)) = self.node_frame_bounds_2d(id) {
                    merge_bounds(&mut bounds, center, half);
                }
            }
            if let Some(bounds) = bounds {
                return Some(bounds_to_center_half(bounds));
            }
        }

        let selected_node_bounds = self.node_frame_bounds_2d(self.selection.selected_node);
        if self.selection.selected_node != self.project.active_scene().root {
            return selected_node_bounds;
        }

        // The World root has a point transform but no useful geometry bounds.
        // Frame the whole grid world instead; retain the root point as the
        // final fallback for a truly empty scene.
        self.all_world_frame_bounds_3d()
            .map(|(center, half)| project_center_half_2d(self.orthographic_view, center, half))
            .or(selected_node_bounds)
    }

    /// End a Top-view node drag: the next drag starts a new gesture (and a
    /// new undo step).
    pub(crate) fn end_node_drag_2d(&mut self) {
        self.node_drag_2d = None;
    }

    /// Top-view node drag. The pointer delta accumulates over the whole
    /// gesture and every frame snaps `base + accumulated`, so a slow drag
    /// (under half a grid step per frame) still reaches the next step, and
    /// the gesture records exactly one undo step on its first real change.
    pub(crate) fn drag_selected_node(&mut self, screen_delta: Vec2) {
        let selected = self.selected_node_ids_in_hierarchy();
        if selected.is_empty() || screen_delta == Vec2::ZERO {
            return;
        }

        let world_delta = ViewportTransform::from_focus(
            Rect::NOTHING,
            OrthographicView::Top,
            [0.0, 0.0],
            self.viewport_zoom,
        )
        .screen_delta_to_world(screen_delta);
        let same_gesture = self.node_drag_2d.as_ref().is_some_and(|drag| {
            drag.base
                .iter()
                .map(|(id, _)| *id)
                .eq(selected.iter().copied())
        });
        if !same_gesture {
            let scene = self.project.active_scene();
            let base = selected
                .iter()
                .filter_map(|id| {
                    scene
                        .node(*id)
                        .map(|node| (*id, node.transform.translation))
                })
                .collect();
            self.node_drag_2d = Some(NodeDrag2d {
                base,
                accumulated: [0.0, 0.0],
                undo_recorded: false,
            });
        }
        let Some(drag) = self.node_drag_2d.as_mut() else {
            return;
        };
        drag.accumulated[0] += world_delta[0];
        drag.accumulated[1] += world_delta[1];
        let accumulated = drag.accumulated;
        let undo_recorded = drag.undo_recorded;
        let targets = drag
            .base
            .iter()
            .map(|(id, base)| (*id, *base))
            .collect::<Vec<_>>();
        let before = (!undo_recorded).then(|| self.project.clone());
        let snap_step = i32::from(self.snap_units.max(1));
        let arch_tile_size = self
            .project
            .world_sector_size_for_node(self.project.active_scene().root);
        let mut moved = Vec::new();
        for (id, base) in targets {
            if let Some(node) = self.project.active_scene_mut().node_mut(id) {
                let previous = node.transform.translation;
                node.transform.translation[0] =
                    snap_node_component(&node.kind, base[0] + accumulated[0], snap_step);
                node.transform.translation[2] =
                    snap_node_component(&node.kind, base[2] + accumulated[1], snap_step);
                if let NodeKind::ArchProp { geometry, .. } = &node.kind {
                    let geometry = *geometry;
                    snap_arch_prop_transform(&mut node.transform, geometry, arch_tile_size);
                }
                if node.transform.translation != previous {
                    moved.push(node.name.clone());
                }
            }
        }
        if moved.is_empty() {
            return;
        }
        if let Some(before) = before {
            self.history.record(before);
            if let Some(drag) = self.node_drag_2d.as_mut() {
                drag.undo_recorded = true;
            }
        }

        match moved.as_slice() {
            [] => {}
            [name] => {
                self.status = format!("Moved {name}");
                self.mark_dirty();
            }
            _ => {
                self.status = format!("Moved {} nodes", moved.len());
                self.mark_dirty();
            }
        }
    }
}
