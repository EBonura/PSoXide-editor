use super::*;

impl EditorWorkspace {
    pub(crate) fn replace_node_selection(&mut self, id: NodeId) {
        self.selection.selected_room = None;
        self.selection.selected_node = id;
        self.selection.selected_nodes.clear();
        self.selection.selected_nodes.insert(id);
        self.selection.node_selection_anchor = Some(id);
    }

    pub(crate) fn clear_node_selection_state(&mut self) {
        self.selection.selected_node = NodeId::ROOT;
        self.selection.selected_nodes.clear();
        self.selection.node_selection_anchor = None;
    }

    pub(crate) fn replace_resource_selection(&mut self, id: ResourceId) {
        self.selection.selected_resource = Some(id);
        self.selection.selected_resources.clear();
        self.selection.selected_resources.insert(id);
        self.selection.resource_selection_anchor = Some(id);
        self.resource_delete_confirm = None;
    }

    /// A click on empty space clears every selection domain.
    pub(crate) fn clear_all_selections(&mut self) {
        self.selection.selected_room = None;
        self.clear_resource_selection_state();
        self.clear_node_selection_state();
        self.status = "Cleared selection".to_string();
    }

    pub(crate) fn clear_resource_selection_state(&mut self) {
        self.selection.selected_resource = None;
        self.selection.selected_resources.clear();
        self.selection.resource_selection_anchor = None;
        self.resource_delete_confirm = None;
    }

    pub(crate) fn select_all_current_scope(&mut self) {
        if self.selection.selected_resource.is_some()
            || !self.selection.selected_resources.is_empty()
        {
            self.select_all_resources();
            return;
        }

        self.select_all_scene_nodes();
    }

    pub(crate) fn select_all_scene_nodes(&mut self) {
        let ids: Vec<NodeId> = self
            .scene_node_order()
            .into_iter()
            .filter(|id| *id != NodeId::ROOT)
            .collect();
        if ids.is_empty() {
            self.status = "No scene nodes to select".to_string();
            return;
        }

        self.selection.selected_nodes = ids.iter().copied().collect();
        self.selection.selected_node = ids[0];
        self.selection.node_selection_anchor = Some(ids[0]);
        self.clear_resource_selection_state();
        self.status = if ids.len() == 1 {
            "Selected 1 node".to_string()
        } else {
            format!("Selected {} nodes", ids.len())
        };
    }

    pub(crate) fn select_all_resources(&mut self) {
        let ids: Vec<ResourceId> = self
            .project
            .resources
            .iter()
            .map(|resource| resource.id)
            .collect();
        if ids.is_empty() {
            self.status = "No resources to select".to_string();
            return;
        }

        self.selection.selected_resources = ids.iter().copied().collect();
        self.selection.selected_resource = Some(ids[0]);
        self.selection.resource_selection_anchor = Some(ids[0]);
        self.resource_delete_confirm = None;
        self.clear_node_selection_state();
        self.status = if ids.len() == 1 {
            "Selected 1 resource".to_string()
        } else {
            format!("Selected {} resources", ids.len())
        };
    }

    pub(crate) fn node_is_selected(&self, id: NodeId) -> bool {
        self.selection.selected_nodes.contains(&id)
            || (self.selection.selected_nodes.is_empty() && self.selection.selected_node == id)
    }

    pub(crate) fn resource_is_selected(&self, id: ResourceId) -> bool {
        self.selection.selected_resources.contains(&id)
            || (self.selection.selected_resources.is_empty()
                && self.selection.selected_resource == Some(id))
    }

    /// Select `node` the way a viewport click does: plain replaces the
    /// selection, Shift extends a range, Ctrl/Cmd toggles.
    pub(crate) fn select_node_with_modifiers(&mut self, node: NodeId, modifiers: egui::Modifiers) {
        let order = self.scene_node_order();
        self.apply_node_selection_modifiers(node, modifiers, &order);
    }

    pub(crate) fn apply_node_selection_modifiers(
        &mut self,
        id: NodeId,
        modifiers: egui::Modifiers,
        visible_order: &[NodeId],
    ) {
        let toggle = modifiers.command || modifiers.ctrl;
        self.selection
            .apply_node_modifiers(id, modifiers.shift, toggle, visible_order);
        let count = self.selection.selected_nodes.len();
        let scene = self.project.active_scene();
        if count > 1 {
            self.status = format!("Selected {count} nodes");
        } else if let Some(n) = scene.node(self.selection.selected_node) {
            let what = match (&n.kind, classify(scene, n)) {
                (NodeKind::Entity, ObjectKind::ModelActor) => "Prop",
                (NodeKind::Entity, ObjectKind::Interaction) => "Interaction",
                (NodeKind::Entity, _) => "Actor",
                (NodeKind::PointLight { .. }, _) => "Light",
                (kind, _) => kind.label(),
            };
            self.status = format!("Selected {what} '{}'", n.name);
        } else {
            self.status = "Cleared node selection".to_string();
        }
    }

    pub(crate) fn apply_resource_selection_modifiers(
        &mut self,
        id: ResourceId,
        modifiers: egui::Modifiers,
        visible_order: &[ResourceId],
    ) {
        let toggle = modifiers.command || modifiers.ctrl;
        self.selection
            .apply_resource_modifiers(id, modifiers.shift, toggle, visible_order);
        self.resource_delete_confirm = None;

        let count = self.selection.selected_resources.len();
        if count > 1 {
            self.status = format!("Selected {count} resources");
        } else if let Some(id) = self.selection.selected_resource {
            if let Some(name) = self.project.resource_name(id) {
                self.status = format!("Selected {name}");
            }
        } else {
            self.status = "Cleared resource selection".to_string();
        }
    }

    pub(crate) fn selected_resource_ids_in_project_order(&self) -> Vec<ResourceId> {
        let mut selected = self.selection.selected_resources.clone();
        if selected.is_empty() {
            if let Some(id) = self.selection.selected_resource {
                selected.insert(id);
            }
        }
        self.project
            .resources
            .iter()
            .map(|resource| resource.id)
            .filter(|id| selected.contains(id))
            .collect()
    }

    pub(crate) fn resource_delete_targets(&self, fallback: ResourceId) -> Vec<ResourceId> {
        let selected = self.selected_resource_ids_in_project_order();
        if selected.is_empty() && self.project.resource(fallback).is_some() {
            vec![fallback]
        } else {
            selected
        }
    }

    pub(crate) fn begin_resource_delete_confirmation(&mut self) {
        let targets = self
            .selection
            .selected_resource
            .map(|id| self.resource_delete_targets(id))
            .unwrap_or_else(|| self.selected_resource_ids_in_project_order());
        if targets.is_empty() {
            self.status = "No resource selected".to_string();
            self.resource_delete_confirm = None;
            return;
        }
        self.status = if targets.len() == 1 {
            "Confirm resource deletion in the inspector".to_string()
        } else {
            format!(
                "Confirm deletion of {} resources in the inspector",
                targets.len()
            )
        };
        self.resource_delete_confirm = Some(targets);
    }

    pub(crate) fn draw_resource_delete_controls(
        &mut self,
        ui: &mut egui::Ui,
        fallback: ResourceId,
    ) -> bool {
        let targets = self.resource_delete_targets(fallback);
        if targets.is_empty() {
            return false;
        }

        let label = self.resource_delete_label(&targets);
        let reference_count: usize = targets
            .iter()
            .map(|id| self.project.resource_reference_count(*id))
            .sum();
        let confirming = self
            .resource_delete_confirm
            .as_ref()
            .is_some_and(|pending| pending.as_slice() == targets.as_slice());

        ui.separator();
        if confirming {
            let mut confirmed = false;
            let mut cancelled = false;
            section_frame().show(ui, |ui| {
                ui.label(
                    RichText::new(format!("Delete {label}?"))
                        .strong()
                        .color(Color32::from_rgb(255, 190, 150)),
                );
                ui.label(
                    RichText::new(
                        "This removes the resource and deletes its project-owned backing files.",
                    )
                    .color(STUDIO_TEXT_WEAK)
                    .small(),
                );
                if reference_count > 0 {
                    ui.label(
                        RichText::new(format!(
                            "{reference_count} project reference(s) will be cleared."
                        ))
                        .color(Color32::from_rgb(255, 218, 150))
                        .small(),
                    );
                }
                ui.horizontal(|ui| {
                    if ui
                        .button(icons::label(icons::TRASH, "Confirm Delete"))
                        .clicked()
                    {
                        confirmed = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancelled = true;
                    }
                });
            });
            if confirmed {
                self.delete_resources(&targets);
                return true;
            }
            if cancelled {
                self.resource_delete_confirm = None;
                self.status = "Resource deletion cancelled".to_string();
            }
        } else if ui
            .button(icons::label(
                icons::TRASH,
                if targets.len() == 1 {
                    "Delete Resource"
                } else {
                    "Delete Resources"
                },
            ))
            .clicked()
        {
            self.resource_delete_confirm = Some(targets);
            self.status = format!("Confirm deletion of {label}");
        }
        false
    }

    pub(crate) fn resource_delete_label(&self, ids: &[ResourceId]) -> String {
        match ids {
            [id] => self
                .project
                .resource_name(*id)
                .map(|name| format!("'{name}'"))
                .unwrap_or_else(|| format!("resource #{}", id.raw())),
            _ => format!("{} resources", ids.len()),
        }
    }

    pub(crate) fn delete_resources(&mut self, ids: &[ResourceId]) {
        let targets: Vec<ResourceId> = ids
            .iter()
            .copied()
            .filter(|id| self.project.resource(*id).is_some())
            .collect();
        if targets.is_empty() {
            self.resource_delete_confirm = None;
            self.status = "No matching resources to delete".to_string();
            return;
        }

        let before = self.project.clone();
        let mut removed = 0usize;
        let mut cleared_references = 0usize;
        let mut deleted_files = 0usize;
        let mut skipped_files = 0usize;
        let mut removed_names = Vec::new();
        let mut failed = None;
        for id in targets {
            self.remove_texture_thumb(id);
            match self
                .project
                .delete_resource_with_files(id, &self.project_dir)
            {
                Ok(report) => {
                    removed += 1;
                    cleared_references += report.cleared_references;
                    deleted_files += report.deleted_files.len();
                    skipped_files += report.skipped_files.len();
                    removed_names.push(report.removed.name);
                }
                Err(error) => {
                    failed = Some(error.to_string());
                    break;
                }
            }
        }

        self.clear_resource_selection_state();
        self.resource_renaming = None;
        self.reconcile_selection_after_document_change();
        if removed > 0 {
            if deleted_files > 0 {
                self.history.clear();
            } else {
                self.history.record(before);
            }
            self.mark_dirty();
            let mut status = if removed == 1 {
                let name = removed_names
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "resource".to_string());
                if cleared_references > 0 {
                    format!("Deleted {name}; cleared {cleared_references} reference(s)")
                } else {
                    format!("Deleted {name}")
                }
            } else if cleared_references > 0 {
                format!("Deleted {removed} resources; cleared {cleared_references} reference(s)")
            } else {
                format!("Deleted {removed} resources")
            };
            if deleted_files > 0 {
                status.push_str(&format!("; deleted {deleted_files} file(s)"));
            }
            if skipped_files > 0 {
                status.push_str(&format!("; skipped {skipped_files} file path(s)"));
            }
            if let Some(error) = failed {
                status.push_str(&format!("; stopped: {error}"));
            }
            self.status = status;
        } else if let Some(error) = failed {
            self.status = format!("Delete failed: {error}");
        }
    }

    pub(crate) fn scene_node_order(&self) -> Vec<NodeId> {
        self.project
            .active_scene()
            .hierarchy_rows()
            .into_iter()
            .map(|row| row.id)
            .collect()
    }

    pub(crate) fn scene_node_effectively_hidden(&self, id: NodeId) -> bool {
        scene_node_hidden(self.project.active_scene(), &self.hidden_scene_nodes, id)
    }

    pub(crate) fn selected_node_ids_in_hierarchy(&self) -> Vec<NodeId> {
        let mut selected = self.selection.selected_nodes.clone();
        if self.selection.selected_node != NodeId::ROOT {
            selected.insert(self.selection.selected_node);
        }
        self.project
            .active_scene()
            .hierarchy_rows()
            .into_iter()
            .map(|row| row.id)
            .filter(|id| *id != NodeId::ROOT && selected.contains(id))
            .collect()
    }

    pub(crate) fn scene_tree_drag_sources(&self, source: NodeId) -> Vec<NodeId> {
        if !self.selection.selected_nodes.contains(&source) {
            return vec![source];
        }

        let scene = self.project.active_scene();
        let mut selected = self.selection.selected_nodes.clone();
        if self.selection.selected_node != NodeId::ROOT {
            selected.insert(self.selection.selected_node);
        }
        let ordered: Vec<NodeId> = scene
            .hierarchy_rows()
            .into_iter()
            .map(|row| row.id)
            .filter(|id| *id != NodeId::ROOT && selected.contains(id))
            .collect();

        let roots: Vec<NodeId> = ordered
            .iter()
            .copied()
            .filter(|id| {
                !ordered
                    .iter()
                    .any(|other| *other != *id && scene.is_descendant_of(*id, *other))
            })
            .collect();
        if roots.is_empty() {
            vec![source]
        } else {
            roots
        }
    }

    pub(crate) fn scene_tree_reparent_is_valid(
        &self,
        sources: &[NodeId],
        target_parent: NodeId,
    ) -> bool {
        let scene = self.project.active_scene();
        if scene.node(target_parent).is_none() {
            return false;
        }
        sources.iter().all(|source| {
            *source != NodeId::ROOT
                && scene.node(*source).is_some()
                && *source != target_parent
                && !scene.is_descendant_of(target_parent, *source)
        })
    }

    /// Union of the entity bounds of everything under a Group node.
    fn group_bounds_3d(&self, group: NodeId) -> Option<([f32; 3], [f32; 3])> {
        let scene = self.project.active_scene();
        let mut bounds = None;
        for bound in self.collect_entity_bounds() {
            if scene.is_descendant_of(bound.node, group) {
                merge_bounds_3d(&mut bounds, bound.center, bound.half_extents);
            }
        }
        bounds.map(bounds_3d_to_center_half)
    }

    pub(crate) fn node_frame_bounds_3d(&self, id: NodeId) -> Option<([f32; 3], [f32; 3])> {
        if self.scene_node_effectively_hidden(id) {
            return None;
        }
        let scene = self.project.active_scene();
        let node = scene.node(id)?;
        if matches!(node.kind, NodeKind::Group) {
            return self.group_bounds_3d(id);
        }

        let entity_bounds = self.collect_entity_bounds();
        let mut current = Some(node.id);
        while let Some(id) = current {
            if let Some(bounds) = entity_bounds.iter().find(|b| b.node == id) {
                return Some((bounds.center, bounds.half_extents));
            }
            current = scene.node(id).and_then(|n| n.parent);
        }
        None
    }

    pub(crate) fn node_frame_bounds_2d(&self, id: NodeId) -> Option<([f32; 2], [f32; 2])> {
        if self.scene_node_effectively_hidden(id) {
            return None;
        }
        let scene = self.project.active_scene();
        let node = scene.node(id)?;
        if matches!(node.kind, NodeKind::Group) {
            return self
                .group_bounds_3d(id)
                .map(|(center, half)| ([center[0], center[2]], [half[0], half[2]]));
        }
        Some((node_world(node), [0.75, 0.75]))
    }

    pub(crate) fn retain_hidden_ui_nodes_for_project(&mut self) {
        let valid_ui_nodes: HashSet<(UiSceneId, UiNodeId)> = self
            .project
            .ui_scenes
            .iter()
            .flat_map(|scene| {
                let scene_id = scene.id;
                scene.nodes().iter().map(move |node| (scene_id, node.id))
            })
            .collect();
        self.hidden_ui_nodes
            .retain(|key| valid_ui_nodes.contains(key));
    }

    pub(crate) fn reconcile_selection_after_document_change(&mut self) {
        if let Some(world) = self.project.active_scene().world.grid() {
            self.grid.selection.retain_valid(world);
            self.grid.plane_sel.retain(|id| id.exists(world));
        } else {
            self.grid.selection.clear();
            self.grid.plane_sel.clear();
        }
        let valid_nodes: HashSet<NodeId> = self
            .project
            .active_scene()
            .nodes()
            .iter()
            .map(|node| node.id)
            .collect();
        self.selection
            .selected_nodes
            .retain(|id| valid_nodes.contains(id));
        self.collapsed_scene_nodes
            .retain(|id| valid_nodes.contains(id));
        self.hidden_scene_nodes
            .retain(|id| valid_nodes.contains(id));
        if self
            .selection
            .node_selection_anchor
            .is_some_and(|id| !valid_nodes.contains(&id))
        {
            self.selection.node_selection_anchor = None;
        }
        if self.selection.selected_node != NodeId::ROOT
            && !valid_nodes.contains(&self.selection.selected_node)
        {
            self.selection.selected_node =
                first_in_order(&self.scene_node_order(), &self.selection.selected_nodes)
                    .unwrap_or(NodeId::ROOT);
        }

        let valid_resources: HashSet<ResourceId> = self
            .project
            .resources
            .iter()
            .map(|resource| resource.id)
            .collect();
        self.selection
            .selected_resources
            .retain(|id| valid_resources.contains(id));
        if self
            .selection
            .resource_selection_anchor
            .is_some_and(|id| !valid_resources.contains(&id))
        {
            self.selection.resource_selection_anchor = None;
        }
        if self
            .selection
            .selected_resource
            .is_some_and(|id| !valid_resources.contains(&id))
        {
            self.selection.selected_resource = self
                .project
                .resources
                .iter()
                .map(|resource| resource.id)
                .find(|id| self.selection.selected_resources.contains(id));
        }
        if let Some(ids) = &mut self.resource_delete_confirm {
            ids.retain(|id| valid_resources.contains(id));
            if ids.is_empty() {
                self.resource_delete_confirm = None;
            }
        }

        // The UI-scene list can shrink across undo/redo (a created or
        // duplicated scene is rolled back). Clamp the active index and
        // drop any pending scene-strip rename / delete-confirm so they
        // never point past the end.
        let ui_scene_count = self.project.ui_scenes.len();
        if ui_scene_count == 0 {
            self.active_ui_scene_index = 0;
        } else if self.active_ui_scene_index >= ui_scene_count {
            self.active_ui_scene_index = ui_scene_count - 1;
        }
        if self
            .ui_scene_renaming
            .as_ref()
            .is_some_and(|(index, _)| *index >= ui_scene_count)
        {
            self.ui_scene_renaming = None;
            self.ui_scene_rename_focus_pending = false;
        }
        if self
            .ui_scene_delete_confirm
            .is_some_and(|index| index >= ui_scene_count)
        {
            self.ui_scene_delete_confirm = None;
        }
        let scene_state_count = self.project.scene_states.len();
        if scene_state_count == 0 {
            self.active_scene_state_index = 0;
        } else if self.active_scene_state_index >= scene_state_count {
            self.active_scene_state_index = scene_state_count - 1;
        }
        self.retain_hidden_ui_nodes_for_project();
        // The selected UI node belongs to whatever scene is now active;
        // a stale id from a rolled-back scene snaps back to the canvas.
        let ui_root = self
            .current_ui_scene()
            .map(|scene| scene.root)
            .unwrap_or(UiNodeId::ROOT);
        if self
            .current_ui_scene()
            .is_none_or(|scene| scene.node(self.selection.selected_ui_node).is_none())
        {
            self.selection.selected_ui_node = ui_root;
        }
    }
}
