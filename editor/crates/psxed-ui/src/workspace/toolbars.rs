use super::*;
use crate::grid_tools::GridViewMode;

impl EditorWorkspace {
    pub(crate) fn draw_viewport(
        &mut self,
        ctx: &egui::Context,
        viewport_3d: EditorViewport3dPresentation,
        playtest_status: EditorPlaytestStatus,
    ) {
        egui::CentralPanel::default()
            .frame(viewport_frame())
            .show(ctx, |ui| {
                tool_panel_frame().show(ui, |ui| {
                    ui.expand_to_include_rect(ui.max_rect());
                    let workspace_undo_candidate = if matches!(
                        self.active_workspace,
                        WorkspaceView::Material | WorkspaceView::Animation
                    ) {
                        self.prepare_inspector_undo_frame(ctx);
                        inspector_has_edit_input(ctx)
                            .then(|| (self.project.clone(), self.history.epoch()))
                    } else {
                        None
                    };
                    self.draw_viewport_header_toolbar(ui);
                    tool_panel_body(ui, |ui| {
                        if viewport_3d.mode == EditorViewport3dMode::Play {
                            self.draw_viewport_3d_play_body(ui, viewport_3d, playtest_status);
                            return;
                        }

                        if self.active_workspace == WorkspaceView::Material {
                            self.draw_material_lab(ui);
                            if let Some((project_before, history_epoch_before)) =
                                workspace_undo_candidate
                            {
                                self.finish_inspector_undo_frame(
                                    project_before,
                                    history_epoch_before,
                                    ctx,
                                );
                            }
                            return;
                        }

                        if self.active_workspace == WorkspaceView::Animation {
                            let changed = model_animation_viewer::draw_model_animation_viewer(
                                ui,
                                &mut self.project,
                                &self.project_dir,
                                &mut self.animation_viewer,
                                &mut self.animation_viewer_preview_texture,
                            );
                            if changed {
                                self.mark_dirty();
                            }
                            if let Some((project_before, history_epoch_before)) =
                                workspace_undo_candidate
                            {
                                self.finish_inspector_undo_frame(
                                    project_before,
                                    history_epoch_before,
                                    ctx,
                                );
                            }
                            return;
                        }

                        if self.active_workspace == WorkspaceView::Ui {
                            self.draw_ui_workspace_body(ui);
                            return;
                        }

                        if self.is_grid_world()
                            && self.grid.split
                            && self.active_workspace == WorkspaceView::Room
                        {
                            self.draw_viewport_split_body(ui, viewport_3d);
                            return;
                        }

                        if !self.view_2d {
                            self.draw_viewport_3d_body(ui, viewport_3d);
                            return;
                        }

                        self.draw_viewport_2d_body(ui);
                    });
                });
            });
    }

    /// The plan: the orthographic view of the world from above, with its
    /// tools, overlays and picking. Drawn alone, or beside the 3D view in
    /// the split layout.
    pub(crate) fn draw_viewport_2d_body(&mut self, ui: &mut egui::Ui) {
        let size = ui.available_size();
        let size = Vec2::new(size.x.max(320.0), size.y.max(240.0));
        let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        surrender_stale_focus_on_viewport_pointer(ui.ctx(), &response);
        self.last_viewport_size = rect.size();
        if self.grid.fit_pending && self.is_grid_world() {
            // The open-time framing guessed the pane size; fit
            // the world to the real one, once.
            self.grid.fit_pending = false;
            self.frame_viewport();
        }
        #[cfg(test)]
        {
            self.last_orthographic_viewport_rect = rect;
            self.last_orthographic_response = Some((
                response.id,
                response.hovered(),
                response.drag_started_by(egui::PointerButton::Primary),
                response.dragged_by(egui::PointerButton::Primary),
            ));
        }
        let orthographic_view = self.orthographic_view;
        let top_view = orthographic_view == OrthographicView::Top;
        let dnd_active = egui::DragAndDrop::has_any_payload(ui.ctx());
        let resource_drop_hovered =
            top_view && response.dnd_hover_payload::<ResourceId>().is_some();

        if !dnd_active
            && (response.dragged_by(egui::PointerButton::Middle)
                || response.dragged_by(egui::PointerButton::Secondary))
        {
            let delta = ui.input(|input| input.pointer.delta());
            let [horizontal, vertical] = orthographic_view.plane_axes();
            let world = ViewportTransform::from_focus(
                rect,
                orthographic_view,
                [0.0, 0.0],
                self.viewport_zoom,
            )
            .screen_delta_to_world(delta);
            self.orthographic_focus[horizontal] -= world[0];
            self.orthographic_focus[vertical] -= world[1];
        }

        if !dnd_active && response.hovered() {
            let scroll = ui.input(|input| input.raw_scroll_delta.y);
            if scroll.abs() > f32::EPSILON {
                let pointer = ui
                    .input(|input| input.pointer.hover_pos())
                    .unwrap_or_else(|| rect.center());
                let before = ViewportTransform::from_focus(
                    rect,
                    orthographic_view,
                    orthographic_view.project_f32(self.orthographic_focus),
                    self.viewport_zoom,
                )
                .screen_to_world(pointer);
                let zoom_factor = (1.0 + scroll * 0.0015).clamp(0.75, 1.25);
                self.viewport_zoom =
                    (self.viewport_zoom * zoom_factor).clamp(MIN_VIEWPORT_ZOOM, MAX_VIEWPORT_ZOOM);
                let after = ViewportTransform::from_focus(
                    rect,
                    orthographic_view,
                    orthographic_view.project_f32(self.orthographic_focus),
                    self.viewport_zoom,
                )
                .screen_to_world(pointer);
                let projected_focus = orthographic_view.project_f32(self.orthographic_focus);
                self.orthographic_focus = orthographic_view.with_projected_focus(
                    self.orthographic_focus,
                    [
                        projected_focus[0] + before[0] - after[0],
                        projected_focus[1] + before[1] - after[1],
                    ],
                );
            }
        }

        let transform = ViewportTransform::from_focus(
            rect,
            orthographic_view,
            orthographic_view.project_f32(self.orthographic_focus),
            self.viewport_zoom,
        );
        if top_view {
            let dropped_resource = resource_drop_hovered
                .then(|| response.dnd_release_payload::<ResourceId>())
                .flatten()
                .map(|payload| *payload);
            if let Some(pointer) = response.interact_pointer_pos().or(response.hover_pos()) {
                let world = transform.screen_to_world(pointer);
                if let Some(resource_id) = dropped_resource {
                    self.drop_resource_2d(resource_id, world);
                }
            }
        }
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, STUDIO_VIEWPORT);
        if self.show_grid && !self.is_grid_world() {
            draw_world_grid(&painter, transform, self.snap_units.max(1) as f32);
        }

        if top_view {
            if let Some(world) = self.project.active_scene().world.grid() {
                crate::grid_view::draw_grid_world(
                    &painter,
                    transform,
                    world,
                    true,
                    &crate::grid_view::GridRoomView {
                        hidden: &self.grid_hidden_rooms,
                        active: self.grid_active_room,
                        level: self.grid.level.y,
                        walls: Some(crate::grid_view::WallLineSource::new(
                            &self.top_wall_lines,
                            self.grid.rev,
                        )),
                    },
                );
            }
        }

        let hits = if top_view {
            draw_scene_viewport(
                &painter,
                transform,
                &self.project,
                SceneViewportContext {
                    hidden_scene_nodes: &self.hidden_scene_nodes,
                    selected: self.selection.selected_node,
                    selected_nodes: &self.selection.selected_nodes,
                    show_lights: self.show_lights,
                },
            )
        } else {
            Vec::new()
        };

        let pointer_world = response
            .hover_pos()
            .or_else(|| response.interact_pointer_pos())
            .map(|pos| transform.screen_to_world(pos));
        let tool_frame = tools2d::ToolFrame2d {
            rect,
            transform,
            top_view,
            pointer_world,
            pointer_interact: response.interact_pointer_pos(),
            press_origin: ui.input(|input| input.pointer.press_origin()),
            modifiers: ui.input(|input| input.modifiers),
            hits: &hits,
            hovered: response.hovered(),
            dnd_active,
            drag_started: response.drag_started_by(egui::PointerButton::Primary),
            dragged: response.dragged_by(egui::PointerButton::Primary),
            drag_stopped: response.drag_stopped_by(egui::PointerButton::Primary),
            clicked: response.clicked_by(egui::PointerButton::Primary),
        };
        let tool2d = tools2d::tool_impl_2d(self);
        tool2d.hover(self, ui, &tool_frame);
        if tool_frame.drag_started {
            tool2d.primary_pressed(self, ui, &tool_frame);
        }
        if tool_frame.dragged {
            tool2d.primary_dragged(self, ui, &tool_frame);
        }
        if tool_frame.drag_stopped {
            tool2d.primary_released(self, ui, &tool_frame);
        }
        tool2d.frame_end(self, &tool_frame);
        if tool_frame.clicked {
            tool2d.primary_clicked(self, ui, &tool_frame);
        }
        if top_view && self.is_grid_world() {
            self.draw_grid_overlay_2d(&painter, transform);
        } else if self.is_grid_world() {
            painter.text(
                rect.center_top() + Vec2::new(0.0, 14.0),
                Align2::CENTER_TOP,
                "Grid worlds are edited in the Top and 3D views",
                FontId::proportional(13.0),
                STUDIO_TEXT_WEAK,
            );
        }
        draw_viewport_overlay(
            &painter,
            rect,
            &self.project,
            self.viewport_zoom,
            self.snap_units,
            orthographic_view,
            self.is_grid_world(),
        );
        draw_axes_gizmo(&painter, rect, orthographic_view);
        if resource_drop_hovered {
            painter.rect_stroke(
                rect.shrink(2.0),
                2.0,
                Stroke::new(EDITOR_OUTLINE_STROKE_WIDTH, EDITOR_OUTLINE_ACCENT),
                StrokeKind::Inside,
            );
            painter.text(
                rect.center_top() + Vec2::new(0.0, 16.0),
                Align2::CENTER_TOP,
                "Drop resource into scene",
                FontId::proportional(13.0),
                STUDIO_ACCENT,
            );
        }
    }

    /// The plan on the left and the 3D view on the right, each in its own
    /// half of the centre, both live: a tool works in either, and what one
    /// view hovers the other outlines.
    pub(crate) fn draw_viewport_split_body(
        &mut self,
        ui: &mut egui::Ui,
        viewport_3d: EditorViewport3dPresentation,
    ) {
        let whole = ui.available_rect_before_wrap();
        let gap = 6.0;
        let half = ((whole.width() - gap) / 2.0).max(1.0);
        let left = Rect::from_min_size(whole.min, Vec2::new(half, whole.height()));
        let right = Rect::from_min_size(
            whole.min + Vec2::new(half + gap, 0.0),
            Vec2::new(half, whole.height()),
        );
        // The plan is always the Top orthographic view.
        self.orthographic_view = OrthographicView::Top;
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(left), |ui| {
            ui.set_clip_rect(left);
            self.draw_viewport_2d_body(ui);
        });
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(right), |ui| {
            ui.set_clip_rect(right);
            self.draw_viewport_3d_body(ui, viewport_3d);
        });
        ui.painter().line_segment(
            [
                Pos2::new(whole.min.x + half + gap / 2.0, whole.min.y),
                Pos2::new(whole.min.x + half + gap / 2.0, whole.max.y),
            ],
            Stroke::new(1.0, STUDIO_BORDER),
        );
    }

    pub(crate) fn draw_viewport_header_toolbar(&mut self, ui: &mut egui::Ui) {
        let grid_room = self.is_grid_world() && self.active_workspace == WorkspaceView::Room;
        egui::Frame::new()
            .fill(STUDIO_PANEL_HEADER)
            .inner_margin(egui::Margin::symmetric(8, 3))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(4.0, 3.0);
                ui.horizontal(|ui| {
                    self.draw_workspace_tabs(ui);
                    if grid_room {
                        // The view switch and the overlays sit with the tabs, so
                        // the plan is one click away and the tool row below is
                        // tools and their options only.
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            self.draw_grid_view_controls(ui);
                        });
                    }
                });
                ui.separator();
                if self.active_workspace == WorkspaceView::Material {
                    ui.horizontal_wrapped(|ui| self.draw_material_lab_toolbar(ui));
                } else if self.active_workspace == WorkspaceView::Animation {
                    let action = model_animation_viewer::draw_model_animation_viewer_toolbar(
                        ui,
                        &mut self.project,
                        &self.project_dir,
                        &mut self.animation_viewer,
                        &mut self.animation_viewer_preview_texture,
                    );
                    if let Some(action) = action {
                        self.handle_animation_viewer_action(action);
                    }
                } else if grid_room {
                    self.draw_grid_tool_rows(ui);
                    ui.horizontal_wrapped(|ui| self.draw_grid_context_controls(ui));
                } else if self.is_grid_world() {
                    ui.horizontal_wrapped(|ui| self.draw_viewport_toolbar(ui));
                } else {
                    ui.horizontal(|ui| self.draw_viewport_toolbar(ui));
                }
            });
    }

    /// The right end of the tab row in a grid world, right to left: Keys and
    /// the overlays, the camera, visibility and frame buttons, and the view
    /// switch (3D, Plan, Split).
    pub(crate) fn draw_grid_view_controls(&mut self, ui: &mut egui::Ui) {
        self.retain_shortcut_group_flash();
        if self.shortcut_group_flash.is_some() {
            ui.ctx().request_repaint();
        }
        self.draw_grid_overlay_controls(ui);
        ui.separator();
        toolbar_group_menu_icon_only(
            ui,
            7,
            self.shortcut_group_glow(ShortcutGroup::Visibility),
            self.visibility_group_icon(),
            "Visibility",
            self.visibility_group_label(),
            |ui| self.draw_visibility_menu_contents(ui),
        );
        toolbar_group_menu_icon_only(
            ui,
            8,
            self.shortcut_group_glow(ShortcutGroup::Camera),
            self.camera_rig.mode.icon(),
            "Camera",
            self.viewport_camera_mode_label(),
            |ui| self.draw_camera_group_menu(ui),
        );
        if ui
            .add(egui::Button::new(icons::text(icons::FOCUS, 14.0)).min_size(Vec2::new(28.0, 23.0)))
            .on_hover_text("Frame selection (.)")
            .clicked()
        {
            self.frame_viewport();
        }
        ui.separator();
        // Right to left: the last mode is added first.
        let current = self.grid_view_mode();
        for mode in GridViewMode::ALL.into_iter().rev() {
            let response = ui
                .add(
                    egui::Button::new(icons::label(mode.icon(), mode.label()))
                        .selected(current == mode)
                        .min_size(Vec2::new(28.0, 23.0)),
                )
                .on_hover_text(format!(
                    "{}\nCtrl/Cmd+9 cycles 3D, Plan and Split",
                    mode.tooltip()
                ));
            if response.clicked() {
                self.set_grid_view_mode(mode);
            }
        }
    }

    /// Always-visible top-level workspaces. Their row stays separate from
    /// each workspace's tools so every mode has the same selector -> toolbar
    /// -> content rhythm.
    pub(crate) fn draw_workspace_tabs(&mut self, ui: &mut egui::Ui) {
        for view in [
            WorkspaceView::Room,
            WorkspaceView::Ui,
            WorkspaceView::Animation,
            WorkspaceView::Material,
        ] {
            let selected = self.active_workspace == view;
            let response = ui
                .add(
                    egui::Button::new(icons::label(view.icon(), view.label()))
                        .selected(selected)
                        .min_size(Vec2::new(82.0, 23.0)),
                )
                .on_hover_text(format!("Open the {} workspace", view.label()));
            if response.clicked() && !selected {
                self.active_workspace = view;
                self.status = format!("Workspace: {}", view.label());
                self.mark_shortcut_group_changed(ShortcutGroup::Workspace);
                if view == WorkspaceView::Material {
                    if let Some(material) = self.selected_material_resource() {
                        self.material_lab.focused_material = Some(material);
                    }
                }
            }
        }
    }

    pub(crate) fn mark_shortcut_group_changed(&mut self, group: ShortcutGroup) {
        self.shortcut_group_flash = Some((group, Instant::now()));
    }

    pub(crate) fn retain_shortcut_group_flash(&mut self) {
        if self.shortcut_group_flash.is_some_and(|(_, started)| {
            started.elapsed().as_secs_f32() >= SHORTCUT_GROUP_FLASH_SECONDS
        }) {
            self.shortcut_group_flash = None;
        }
    }

    pub(crate) fn shortcut_group_glow(&self, group: ShortcutGroup) -> f32 {
        let Some((active, started)) = self.shortcut_group_flash else {
            return 0.0;
        };
        if active != group {
            return 0.0;
        }
        let elapsed = started.elapsed().as_secs_f32();
        (1.0 - elapsed / SHORTCUT_GROUP_FLASH_SECONDS).clamp(0.0, 1.0)
    }

    pub(crate) fn draw_viewport_toolbar(&mut self, ui: &mut egui::Ui) {
        self.retain_shortcut_group_flash();
        if self.shortcut_group_flash.is_some() {
            ui.ctx().request_repaint();
        }
        ui.spacing_mut().item_spacing.x = 4.0;
        if self.active_workspace == WorkspaceView::Ui {
            self.draw_ui_viewport_toolbar(ui);
            return;
        }

        if self.is_grid_world() {
            self.draw_grid_tool_strip(ui);
            self.draw_grid_context_controls(ui);
        } else {
            ui.weak("No grid world in this scene");
            self.draw_add_menu(ui);
            ui.separator();
            self.draw_grid_controls(ui);
        }
        ui.separator();
        toolbar_group_menu_icon_only(
            ui,
            7,
            self.shortcut_group_glow(ShortcutGroup::Visibility),
            self.visibility_group_icon(),
            "Visibility",
            self.visibility_group_label(),
            |ui| self.draw_visibility_menu_contents(ui),
        );
        toolbar_group_menu_icon_only(
            ui,
            8,
            self.shortcut_group_glow(ShortcutGroup::Camera),
            self.camera_rig.mode.icon(),
            "Camera",
            self.viewport_camera_mode_label(),
            |ui| self.draw_camera_group_menu(ui),
        );
        if ui
            .add(egui::Button::new(icons::text(icons::FOCUS, 14.0)).min_size(Vec2::new(28.0, 23.0)))
            .on_hover_text("Frame selection (.)")
            .clicked()
        {
            self.frame_viewport();
        }
        toolbar_group_menu_icon_only(
            ui,
            9,
            self.shortcut_group_glow(ShortcutGroup::Viewport),
            self.view_dimension_group_icon(),
            "Viewport",
            self.view_dimension_group_label(),
            |ui| self.draw_view_dimension_group_menu(ui),
        );
    }

    pub(crate) fn draw_add_menu(&mut self, ui: &mut egui::Ui) {
        let button = egui::Button::new(icons::text(icons::PLUS, 15.0))
            .selected(self.active_tool == ViewTool::Place)
            .min_size(Vec2::new(28.0, 23.0));
        let response = egui::menu::menu_custom_button(ui, button, |ui| {
            ui.set_min_width(210.0);
            ui.label(
                RichText::new("ADD TO WORLD")
                    .small()
                    .color(STUDIO_TEXT_WEAK),
            );
            for kind in PlaceKind::ALL {
                let selected = self.active_tool == ViewTool::Place && self.place_kind == kind;
                if toolbar_menu_choice(ui, icons::label(kind.icon(), kind.label()), selected) {
                    self.set_active_tool_cycle_value((ViewTool::Place, Some(kind)));
                }
            }
        })
        .response;
        response.on_hover_text("Add an enemy, spawn, prop, light, particle emitter or logic node");
    }

    fn draw_grid_controls(&mut self, ui: &mut egui::Ui) {
        let master_response = ui
            .add(
                egui::Button::new(icons::text(icons::GRID, 14.0))
                    .selected(self.show_grid)
                    .min_size(Vec2::new(28.0, 23.0)),
            )
            .on_hover_text(if self.show_grid {
                "Grid overlay on; click to hide it"
            } else {
                "Grid overlay off; click to show it"
            });
        if master_response.clicked() {
            self.toggle_grid_overlays();
        }

        let button_text = self.snap_units.max(1).to_string();
        let current = format!(
            "{} units · background {}",
            self.snap_units.max(1),
            if self.show_grid { "shown" } else { "hidden" },
        );
        toolbar_option_menu(
            ui,
            icons::CHEVRON_DOWN,
            &button_text,
            "Grid settings",
            current,
            false,
            |ui| {
                ui.set_min_width(238.0);
                let visibility_before = self.show_grid;
                ui.checkbox(&mut self.show_grid, "Background grid");
                if visibility_before != self.show_grid {
                    self.mark_shortcut_group_changed(ShortcutGroup::Visibility);
                }

                ui.separator();
                ui.label(
                    RichText::new("QUANTISATION")
                        .small()
                        .color(STUDIO_TEXT_WEAK),
                );
                let snap_before = self.snap_units;
                ui.add(
                    egui::DragValue::new(&mut self.snap_units)
                        .range(ENGINE_UNIT..=MAX_GRID_UNITS)
                        .speed(f64::from(ENGINE_UNIT) * 0.25)
                        .prefix("Grid "),
                )
                .on_hover_text(
                    "Multiples of 16: the cook keeps positions in 16-unit engine steps, \
                     so a finer grid would not survive it.",
                );
                self.snap_units = engine_grid_units(self.snap_units);
                ui.horizontal_wrapped(|ui| {
                    for step in [16_u16, 32, 64, 128, 256, 512, 1024, 2048] {
                        ui.selectable_value(&mut self.snap_units, step, step.to_string());
                    }
                });
                if self.snap_units != snap_before {
                    self.status = format!("Grid: {} units", self.snap_units);
                }
                ui.weak("Node placement snaps to this interval.");
            },
        );
    }

    pub(crate) fn toggle_grid_overlays(&mut self) {
        let visible = !self.show_grid;
        self.show_grid = visible;
        self.status = if visible {
            "Grid overlays shown".to_string()
        } else {
            "Grid overlays hidden".to_string()
        };
    }

    pub(crate) fn draw_transform_gizmo_toolbar_controls(&mut self, ui: &mut egui::Ui) {
        toolbar_group_menu(
            ui,
            3,
            self.shortcut_group_glow(ShortcutGroup::Transform),
            self.transform_gizmo_mode.icon(),
            "Transform",
            self.transform_gizmo_mode.label(),
            |ui| self.draw_transform_group_menu(ui),
        );
        let space = self.gizmo_space;
        let space_response = ui
            .add(
                egui::Button::new(icons::label(space.icon(), space.label()))
                    .min_size(Vec2::new(30.0, 23.0)),
            )
            .on_hover_text(
                "Gizmo orientation. Global aligns handles to the world axes; \
                 Local aligns them to the selected node's own rotation.",
            );
        if space_response.clicked() {
            self.gizmo_space = space.toggled();
            self.status = format!("Gizmo orientation: {}", self.gizmo_space.label());
        }
    }

    pub(crate) fn draw_ui_viewport_toolbar(&mut self, ui: &mut egui::Ui) {
        toolbar_group_menu(
            ui,
            2,
            self.shortcut_group_glow(ShortcutGroup::Transform),
            self.ui_transform_mode.icon(),
            "Transform",
            self.ui_transform_mode.label(),
            |ui| {
                ui.set_min_width(160.0);
                for mode in [UiTransformMode::Move, UiTransformMode::Rotate] {
                    if toolbar_menu_choice(
                        ui,
                        icons::label(mode.icon(), mode.label()),
                        self.ui_transform_mode == mode,
                    ) {
                        self.ui_transform_mode = mode;
                        self.status = format!("UI transform: {}", mode.label());
                        self.mark_shortcut_group_changed(ShortcutGroup::Transform);
                    }
                }
            },
        );

        toolbar_option_menu(ui, icons::PLUS, "Add", "Add UI Node", "Add", false, |ui| {
            ui.set_min_width(160.0);
            for (label, kind) in default_addable_ui_kinds() {
                if ui.button(label).clicked() {
                    self.add_ui_child(kind, label);
                    ui.close_menu();
                }
            }
        });

        let center_snap_label = if self.ui_center_snap { "On" } else { "Off" };
        toolbar_option_menu(
            ui,
            icons::FOCUS,
            "Snap",
            "Center Snap",
            center_snap_label,
            self.ui_center_snap,
            |ui| {
                ui.set_min_width(240.0);
                if ui
                    .checkbox(
                        &mut self.ui_center_snap,
                        "Snap moved nodes to canvas centre",
                    )
                    .changed()
                {
                    self.status = if self.ui_center_snap {
                        "UI centre snap enabled".to_string()
                    } else {
                        "UI centre snap disabled".to_string()
                    };
                }
                ui.weak("Shows blue centre guides while dragging.");
            },
        );

        let nav_preview_label = if self.ui_nav_preview { "On" } else { "Off" };
        toolbar_option_menu(
            ui,
            icons::PLAY,
            "Preview",
            "Navigation Preview",
            nav_preview_label,
            self.ui_nav_preview,
            |ui| {
                ui.set_min_width(228.0);
                if ui
                    .checkbox(&mut self.ui_nav_preview, "Preview UI navigation")
                    .changed()
                {
                    self.status = if self.ui_nav_preview {
                        "UI navigation preview enabled".to_string()
                    } else {
                        "UI navigation preview disabled".to_string()
                    };
                }
                if self.ui_nav_preview {
                    ui.weak("Arrow keys move focus; Enter activates.");
                }
            },
        );

        let screen_offset_label = if self.screen_offset_sim_px == 0 {
            "0 px".to_string()
        } else {
            format!("{:+} px", self.screen_offset_sim_px)
        };
        toolbar_option_menu(
            ui,
            icons::MOVE,
            &screen_offset_label,
            "Screen Offset",
            screen_offset_label.clone(),
            self.screen_offset_sim_px != 0,
            |ui| {
                ui.set_min_width(220.0);
                ui.horizontal(|ui| {
                    ui.label("Screen offset");
                    let response = ui.add(
                        egui::DragValue::new(&mut self.screen_offset_sim_px)
                            .range(-48..=48)
                            .suffix(" px"),
                    );
                    if response.changed() {
                        self.status = format!(
                            "UI screen offset preview {:+} px",
                            self.screen_offset_sim_px
                        );
                    }
                });
                if self.screen_offset_sim_px != 0 && ui.button("Centre").clicked() {
                    self.screen_offset_sim_px = 0;
                    self.status = "UI screen offset preview centred".to_string();
                    ui.close_menu();
                }
                ui.weak("TV-position preview; authored positions stay centred.");
            },
        );
    }

    pub(crate) fn draw_transform_group_menu(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(176.0);
        for mode in [
            TransformGizmoMode::Move,
            TransformGizmoMode::Rotate,
            TransformGizmoMode::Scale,
        ] {
            if toolbar_menu_choice(
                ui,
                icons::label(mode.icon(), mode.label()),
                self.transform_gizmo_mode == mode,
            ) {
                self.set_transform_gizmo_mode(mode);
            }
        }
    }

    pub(crate) fn draw_visibility_menu_contents(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(224.0);
        let mut changed = false;
        egui::Grid::new("viewport-visibility-menu-grid")
            .num_columns(2)
            .spacing(Vec2::new(14.0, 5.0))
            .show(ui, |ui| {
                changed |= visibility_menu_row(ui, "lights", "Lights", &mut self.show_lights);
                changed |= visibility_menu_row(ui, "bounds", "Bounds", &mut self.preview_bounds);
            });
        if changed {
            self.mark_shortcut_group_changed(ShortcutGroup::Visibility);
        }
    }

    pub(crate) fn draw_camera_group_menu(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(238.0);
        for mode in [ViewportCameraMode::Orbit, ViewportCameraMode::Free] {
            if toolbar_menu_choice(
                ui,
                icons::label(mode.icon(), viewport_camera_mode_label(mode)),
                self.camera_rig.mode == mode,
            ) {
                self.set_viewport_3d_camera_mode(mode);
                self.status = match mode {
                    ViewportCameraMode::Orbit => "Camera: Orbit (RMB/MMB drag; Shift pans)".into(),
                    ViewportCameraMode::Free => {
                        "Camera: Free (WASD, Shift faster, RMB/MMB look)".into()
                    }
                };
            }
        }
        ui.add_space(3.0);
        ui.label(
            RichText::new("Free: WASD fly · Shift faster\nRMB/MMB drag look · wheel forward/back")
                .small()
                .color(STUDIO_TEXT_WEAK),
        );
        ui.separator();
        ui.label("Zoom speed");
        let mut zoom_speed = self.camera_rig.zoom_speed();
        if ui
            .add(egui::Slider::new(&mut zoom_speed, 0.2..=3.0).fixed_decimals(1))
            .changed()
        {
            self.camera_rig.set_zoom_speed(zoom_speed);
        }
    }

    pub(crate) fn draw_view_dimension_group_menu(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(140.0);
        for view in OrthographicView::ALL {
            if toolbar_menu_choice(
                ui,
                icons::label(icons::SQUARE, view.label()),
                self.view_2d && self.orthographic_view == view,
            ) && (!self.view_2d || self.orthographic_view != view)
            {
                self.set_orthographic_view(view);
            }
        }
        if toolbar_menu_choice(ui, icons::label(icons::BOX, "3D"), !self.view_2d) && self.view_2d {
            self.view_2d = false;
            self.status = "Viewport: 3D".to_string();
            self.mark_shortcut_group_changed(ShortcutGroup::Viewport);
        }
    }

    pub(crate) fn visibility_group_icon(&self) -> char {
        if self.editor_visibility_has_hidden_items() {
            icons::EYE_OFF
        } else {
            icons::EYE
        }
    }

    pub(crate) fn visibility_group_label(&self) -> &'static str {
        if self.editor_visibility_has_hidden_items() {
            "Some Hidden"
        } else {
            "All Shown"
        }
    }

    pub(crate) fn viewport_camera_mode_label(&self) -> &'static str {
        viewport_camera_mode_label(self.camera_rig.mode)
    }

    pub(crate) fn view_dimension_group_icon(&self) -> char {
        if self.view_2d {
            icons::SQUARE
        } else {
            icons::BOX
        }
    }

    pub(crate) fn view_dimension_group_label(&self) -> &'static str {
        if self.view_2d {
            self.orthographic_view.label()
        } else {
            "3D"
        }
    }

    pub(crate) fn editor_visibility_has_hidden_items(&self) -> bool {
        !self.show_lights || !self.preview_bounds
    }

    pub(crate) fn draw_active_place_options(&mut self, ui: &mut egui::Ui) {
        match self.place_kind {
            PlaceKind::ModelInstance => {
                self.draw_place_resource_picker(ui, ResourceFilter::Model, "Model", false)
            }
            PlaceKind::Character => {
                self.draw_place_resource_picker(ui, ResourceFilter::Character, "Profile", false)
            }
            PlaceKind::ImageProp => {
                self.draw_place_resource_picker(ui, ResourceFilter::ImagePropSource, "Image", false)
            }
            PlaceKind::BoxProp | PlaceKind::CylinderProp | PlaceKind::ArchProp => self
                .draw_place_resource_picker(ui, ResourceFilter::ImagePropSource, "Material", true),
            PlaceKind::ParticleEmitter => {
                ui.weak(
                    "Point-projected sprite emitter. Configure texture and budget after placement.",
                );
            }
            _ => {}
        }
    }

    pub(crate) fn draw_place_resource_picker(
        &mut self,
        ui: &mut egui::Ui,
        filter: ResourceFilter,
        label: &str,
        allow_unassigned: bool,
    ) {
        let options: Vec<(ResourceId, String)> = self
            .project
            .resources
            .iter()
            .filter(|resource| filter.matches(&resource.data))
            .map(|resource| (resource.id, resource.name.clone()))
            .collect();
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(icons::text(filter.icon(), 14.0).color(STUDIO_TEXT_WEAK))
                .on_hover_text(label);
            ui.label(label);
        });

        let before = self.place_resource;
        let auto_label = if options.len() == 1 {
            format!("Auto ({})", options[0].1)
        } else if allow_unassigned {
            "None (assign later)".to_string()
        } else {
            format!("Auto {label}")
        };
        let selected_label = self
            .place_resource
            .and_then(|selected| {
                options
                    .iter()
                    .find(|(id, _)| *id == selected)
                    .map(|(_, name)| name.as_str())
            })
            .unwrap_or(&auto_label);
        let search_hint = format!("Search {}…", label.to_ascii_lowercase());
        let picker_changed = searchable_picker(
            ui,
            ui.id().with(("place-resource-picker", label)),
            &mut self.place_resource,
            selected_label,
            &options,
            SearchablePickerConfig::optional(&auto_label)
                .with_width(220.0)
                .with_popup_min_width(360.0)
                .with_search_hint(&search_hint),
        );
        if options.is_empty() {
            ui.weak(format!("No {} resources", label.to_ascii_lowercase()));
        }
        if picker_changed {
            if let Some(id) = self.place_resource {
                self.replace_resource_selection(id);
            }
        }
        if self.place_resource != before {
            let name = self
                .place_resource
                .and_then(|id| {
                    self.project
                        .resource(id)
                        .map(|resource| resource.name.clone())
                })
                .unwrap_or_else(|| "Auto".to_string());
            self.status = format!("Place {label}: {name}");
            self.mark_shortcut_group_changed(ShortcutGroup::Tool);
        }
    }

    pub(crate) fn place_resource_candidates(&self) -> impl Iterator<Item = ResourceId> {
        [self.place_resource, self.selection.selected_resource]
            .into_iter()
            .flatten()
    }

    /// Pick the Model resource a `PlaceKind::ModelInstance` prop click
    /// should bind to. Returns `(id, default_node_name)` on
    /// success or an actionable status message on failure. The
    /// caller renders the failure into `self.status` and skips
    /// the place altogether -- never silently substitutes a
    /// generic marker.
    pub(crate) fn resolve_place_model_resource(&self) -> Result<(ResourceId, String), String> {
        // (a) Chosen Place toolbar resource, or selected resource,
        // is a Model? Use it.
        for id in self.place_resource_candidates() {
            if let Some(resource) = self.project.resource(id) {
                if matches!(resource.data, ResourceData::Model(_)) {
                    return Ok((id, resource.name.clone()));
                }
            }
        }
        // (b) Exactly one Model resource? Auto-pick.
        let models: Vec<&Resource> = self
            .project
            .resources
            .iter()
            .filter(|r| matches!(r.data, ResourceData::Model(_)))
            .collect();
        match models.len() {
            0 => Err("No Model resources exist. Register or import a model first.".to_string()),
            1 => Ok((models[0].id, models[0].name.clone())),
            n => Err(format!(
                "Select a Model resource before placing a prop ({n} available)"
            )),
        }
    }

    pub(crate) fn resolve_place_image_prop_material(
        &mut self,
    ) -> Result<(ResourceId, String), String> {
        for id in self.place_resource_candidates().collect::<Vec<_>>() {
            let Some(resource) = self.project.resource(id) else {
                continue;
            };
            if matches!(&resource.data, ResourceData::Material(_)) {
                return Ok((id, resource.name.clone()));
            }
        }

        let image_sources: Vec<(ResourceId, String)> = self
            .project
            .resources
            .iter()
            .filter_map(|resource| match &resource.data {
                ResourceData::Material(_) => Some((resource.id, resource.name.clone())),
                _ => None,
            })
            .collect();
        match image_sources.len() {
            0 => Err("No Material resources exist. Import a texture first.".to_string()),
            1 => {
                let (id, name) = &image_sources[0];
                Ok((*id, name.clone()))
            }
            n => Err(format!(
                "Select a Material before placing an image prop ({n} available)"
            )),
        }
    }

    /// Resolve an optional material for a new Box Prop. A Box Prop remains
    /// useful editable geometry without a material, so an ambiguous material
    /// choice must not block placement.
    pub(crate) fn resolve_place_box_prop_material(&self) -> Option<(ResourceId, String)> {
        for id in self.place_resource_candidates() {
            let Some(resource) = self.project.resource(id) else {
                continue;
            };
            if matches!(&resource.data, ResourceData::Material(_)) {
                return Some((id, resource.name.clone()));
            }
        }

        let mut materials = self
            .project
            .resources
            .iter()
            .filter(|&resource| matches!(&resource.data, ResourceData::Material(_)))
            .map(|resource| (resource.id, resource.name.clone()));
        let only = materials.next()?;
        materials.next().is_none().then_some(only)
    }

    pub(crate) fn resolve_place_character_resource(
        &self,
    ) -> Result<(ResourceId, String, psxed_project::CharacterResource), String> {
        for id in self.place_resource_candidates() {
            if let Some(resource) = self.project.resource(id) {
                if let ResourceData::Character(character) = &resource.data {
                    return Ok((id, resource.name.clone(), character.clone()));
                }
            }
        }

        let characters: Vec<&Resource> = self
            .project
            .resources
            .iter()
            .filter(|r| matches!(r.data, ResourceData::Character(_)))
            .collect();
        match characters.len() {
            0 => Err("No Character Profile resources exist. Sync starter content or add a profile first.".to_string()),
            1 => {
                let ResourceData::Character(character) = &characters[0].data else {
                    unreachable!("filtered to character resources");
                };
                Ok((characters[0].id, characters[0].name.clone(), character.clone()))
            }
            n => Err(format!(
                "Select a Character Profile resource before placing a character ({n} available)"
            )),
        }
    }

    /// Walk the active scene and collect a selectable AABB for
    /// every entity-kind node -- every node that's neither the
    /// world root nor a structural Node/World.
    pub fn collect_entity_bounds(&self) -> Vec<EntityBounds> {
        let scene = self.project.active_scene();
        let mut out = Vec::new();
        for node in scene.nodes() {
            if node.id == scene.root {
                continue;
            }
            if self.scene_node_effectively_hidden(node.id) {
                continue;
            }
            if matches!(node.kind, NodeKind::PointLight { .. }) && !self.show_lights {
                continue;
            }
            let Some((kind, half_extents)) = entity_bound_kind_and_size(self, node) else {
                continue;
            };
            // Nodes live in raw world space. Use the translation directly,
            // lifted by the half extent so the bound sits on the anchor.
            let p = node.transform.translation;
            let center_world = [p[0], p[1] + half_extents[1], p[2]];
            out.push(EntityBounds {
                node: node.id,
                kind,
                center: center_world,
                half_extents,
                yaw_degrees: node.transform.rotation_degrees[1],
            });
        }
        out
    }

    /// Pick the nearest entity bound under the camera ray.
    /// Returns the `EntityBoundHit` plus its world distance --
    /// the 3D click handler compares this against surface hits to
    /// pick whichever is closer.
    pub fn pick_entity_bound(
        &self,
        rect: egui::Rect,
        pointer: egui::Pos2,
    ) -> Option<EntityBoundHit> {
        let (origin, dir) = self.camera_ray_for_pointer(rect, pointer)?;
        let bounds = self.collect_entity_bounds();
        let mut best: Option<EntityBoundHit> = None;
        for b in &bounds {
            let Some(t) = ray_intersects_aabb(origin, dir, b.center, b.half_extents) else {
                continue;
            };
            if best.as_ref().is_some_and(|h| h.distance <= t) {
                continue;
            }
            best = Some(EntityBoundHit {
                node: b.node,
                distance: t,
                point: [
                    origin[0] + dir[0] * t,
                    origin[1] + dir[1] * t,
                    origin[2] + dir[2] * t,
                ],
                bounds: *b,
            });
        }
        best
    }

    pub(crate) fn selected_material_resource(&self) -> Option<ResourceId> {
        let id = self.selection.selected_resource?;
        matches!(
            self.project.resource(id).map(|resource| &resource.data),
            Some(ResourceData::Material(_))
        )
        .then_some(id)
    }

    pub(crate) fn has_player_source(&self) -> bool {
        self.project
            .active_scene()
            .nodes()
            .iter()
            .any(|node| node_kind_is_player_source(&node.kind))
    }

    pub(crate) fn selected_node_is_player_source(&self) -> bool {
        self.project
            .active_scene()
            .node(self.selection.selected_node)
            .is_some_and(|node| node_kind_is_player_source(&node.kind))
    }

    pub(crate) fn demote_player_sources_except(&mut self, keep: Option<NodeId>) {
        let scene = self.project.active_scene_mut();
        let ids: Vec<NodeId> = scene
            .nodes()
            .iter()
            .filter(|node| Some(node.id) != keep && node_kind_is_player_source(&node.kind))
            .map(|node| node.id)
            .collect();
        for id in ids {
            let Some(node) = scene.node_mut(id) else {
                continue;
            };
            match &mut node.kind {
                NodeKind::SpawnPoint { player, character } => {
                    *player = false;
                    *character = None;
                }
                NodeKind::CharacterController { player, .. } => {
                    *player = false;
                }
                _ => {}
            }
        }
    }
}
