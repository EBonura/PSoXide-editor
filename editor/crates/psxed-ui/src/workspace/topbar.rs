//! The top bar: one row. The project (its name with an unsaved dot, then the
//! menus) on the left, the workspaces as a segmented control in the middle,
//! Undo, Redo, Build and Play on the right. Play is the only filled button.

use super::*;

/// Width of one segment of the workspace control, so the control has a known
/// width and can sit in the exact middle of the bar.
const WORKSPACE_SEGMENT_WIDTH: f32 = 88.0;
/// Room the project name may take before it is cut.
const PROJECT_NAME_MAX_WIDTH: f32 = 170.0;
/// Height of a segmented control.
pub(crate) const SEGMENT_HEIGHT: f32 = 26.0;

/// A segmented control: one rounded frame, one segment per label, the
/// selected one lit with an accent underline. Returns the index clicked.
/// Every segment is `width` points wide, so the control's size is known.
pub(crate) fn segmented_control(
    ui: &mut egui::Ui,
    id: &str,
    labels: &[&str],
    hovers: &[String],
    selected: Option<usize>,
    width: f32,
) -> Option<usize> {
    let total = Vec2::new(width * labels.len() as f32 + 4.0, SEGMENT_HEIGHT);
    let (outer, _) = ui.allocate_exact_size(total, Sense::hover());
    ui.painter().rect(
        outer,
        egui::CornerRadius::same(CONTROL_RADIUS + 1),
        STUDIO_INPUT,
        Stroke::new(1.0, STUDIO_BORDER),
        egui::StrokeKind::Inside,
    );
    let font = egui::TextStyle::Button.resolve(ui.style());
    let mut clicked = None;
    for (index, label) in labels.iter().enumerate() {
        let rect = Rect::from_min_size(
            outer.min + Vec2::new(2.0 + width * index as f32, 2.0),
            Vec2::new(width, SEGMENT_HEIGHT - 4.0),
        );
        let response = ui
            .interact(rect, ui.id().with((id, index)), Sense::click())
            .on_hover_text(hovers.get(index).map_or("", String::as_str));
        let on = selected == Some(index);
        if on {
            ui.painter().rect_filled(
                rect,
                egui::CornerRadius::same(CONTROL_RADIUS),
                STUDIO_SELECTION,
            );
            let underline = Rect::from_min_max(
                Pos2::new(rect.min.x + 8.0, rect.max.y - 2.0),
                Pos2::new(rect.max.x - 8.0, rect.max.y),
            );
            ui.painter()
                .rect_filled(underline, egui::CornerRadius::same(1), STUDIO_ACCENT);
        } else if response.hovered() {
            ui.painter()
                .rect_filled(rect, egui::CornerRadius::same(CONTROL_RADIUS), STUDIO_HOVER);
        }
        let colour = if on || response.hovered() {
            STUDIO_TEXT
        } else {
            STUDIO_TEXT_WEAK
        };
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            *label,
            font.clone(),
            colour,
        );
        if response.clicked() {
            clicked = Some(index);
        }
    }
    clicked
}

impl EditorWorkspace {
    pub(crate) fn draw_action_bar(
        &mut self,
        ctx: &egui::Context,
        playtest_status: EditorPlaytestStatus,
        play_metrics: Option<EditorPlaytestMetrics>,
    ) {
        let action_bar_height = action_bar_height_for_status(&self.status);
        egui::TopBottomPanel::top("psxed_action_bar")
            .exact_height(action_bar_height)
            .frame(top_bar_frame())
            .show(ctx, |ui| {
                let whole = ui.max_rect();
                let middle = Rect::from_center_size(
                    whole.center(),
                    Vec2::new(WORKSPACE_SEGMENT_WIDTH * 4.0 + 4.0, whole.height()),
                );
                let gap = 10.0;
                let left =
                    Rect::from_min_max(whole.min, Pos2::new(middle.min.x - gap, whole.max.y));
                let right =
                    Rect::from_min_max(Pos2::new(middle.max.x + gap, whole.min.y), whole.max);
                ui.allocate_new_ui(
                    egui::UiBuilder::new()
                        .max_rect(left)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    |ui| {
                        ui.set_clip_rect(left.expand2(Vec2::new(0.0, 4.0)));
                        self.draw_project_identity(ui);
                        ui.add_space(6.0);
                        self.draw_main_menus(ctx, ui, playtest_status);
                    },
                );
                ui.allocate_new_ui(
                    egui::UiBuilder::new()
                        .max_rect(middle)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    |ui| self.draw_workspace_tabs(ui),
                );
                ui.allocate_new_ui(
                    egui::UiBuilder::new()
                        .max_rect(right)
                        .layout(egui::Layout::right_to_left(egui::Align::Center)),
                    |ui| {
                        self.draw_build_play_controls(ui, playtest_status, play_metrics);
                    },
                );
            });
    }

    /// The project's name, an unsaved dot after it, and the way to rename it
    /// (a click on the name). The dot's room is always reserved, so the menus
    /// do not shift when the project becomes dirty.
    pub(crate) fn draw_project_identity(&mut self, ui: &mut egui::Ui) {
        let logo_texture = self.psoxide_logo_texture_id(ui.ctx());
        if let Some(texture_id) = logo_texture {
            ui.add(egui::Image::new((texture_id, Vec2::splat(28.0))))
                .on_hover_text("PSoXide");
        } else {
            ui.label(icons::text(icons::BOX, 18.0).color(STUDIO_ACCENT))
                .on_hover_text("PSoXide");
        }
        ui.add_space(2.0);

        if self.project_name_editing {
            let edit_id = ui.id().with("project_name_inline_edit");
            let response = ui.add_sized(
                Vec2::new(154.0, 23.0),
                egui::TextEdit::singleline(&mut self.project.name)
                    .hint_text("Project name")
                    .font(egui::TextStyle::Button)
                    .id(edit_id),
            );
            if self.project_name_focus_pending {
                response.request_focus();
                self.project_name_focus_pending = false;
            }
            if response.changed() {
                self.mark_dirty();
            }
            let finish_edit = response.lost_focus()
                || (response.has_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)));
            if finish_edit {
                self.project_name_editing = false;
            }
        } else {
            let display_name = if self.project.name.trim().is_empty() {
                "Untitled"
            } else {
                self.project.name.trim()
            };
            let name = ui
                .scope(|ui| {
                    ui.set_max_width(PROJECT_NAME_MAX_WIDTH);
                    ui.add(
                        egui::Label::new(RichText::new(display_name).strong().size(15.0))
                            .truncate()
                            .sense(Sense::click()),
                    )
                })
                .inner
                .on_hover_text("Rename the project")
                .on_hover_cursor(egui::CursorIcon::Text);
            if name.clicked() {
                self.project_name_editing = true;
                self.project_name_focus_pending = true;
            }
        }
        let (dot, response) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
        if self.dirty {
            ui.painter()
                .circle_filled(dot.center(), 4.0, STUDIO_WARNING);
            response.on_hover_text(format!(
                "Unsaved changes ({} saves)",
                command_shortcut_text("S")
            ));
        }
    }

    /// The workspaces, as one segmented control.
    pub(crate) fn draw_workspace_tabs(&mut self, ui: &mut egui::Ui) {
        const VIEWS: [WorkspaceView; 4] = [
            WorkspaceView::Room,
            WorkspaceView::Ui,
            WorkspaceView::Animation,
            WorkspaceView::Material,
        ];
        let labels = VIEWS.map(WorkspaceView::label);
        let hovers = VIEWS.map(|v| format!("Open the {} workspace", v.label()));
        let selected = VIEWS.iter().position(|v| *v == self.active_workspace);
        let clicked = segmented_control(
            ui,
            "workspace_control",
            &labels,
            &hovers,
            selected,
            WORKSPACE_SEGMENT_WIDTH,
        );
        let Some(view) = clicked.map(|i| VIEWS[i]) else {
            return;
        };
        if view == self.active_workspace {
            return;
        }
        self.active_workspace = view;
        self.status = format!("Workspace: {}", view.label());
        self.mark_shortcut_group_changed(ShortcutGroup::Workspace);
        if view == WorkspaceView::Material {
            if let Some(material) = self.selected_material_resource() {
                self.material_lab.focused_material = Some(material);
            }
        }
    }

    /// The right end of the bar, drawn right to left: Play (a split button,
    /// the one filled control), Stop while a playtest runs, Build, Redo, Undo
    /// and, only while a cook, build or playtest is going on, its state.
    pub(crate) fn draw_build_play_controls(
        &mut self,
        ui: &mut egui::Ui,
        playtest_status: EditorPlaytestStatus,
        play_metrics: Option<EditorPlaytestMetrics>,
    ) {
        let playtest_active = playtest_status.is_active();
        let play_label = if playtest_active {
            "Rebuild & Play"
        } else {
            "Play"
        };
        let mut set_boot: Option<BootTarget> = None;
        let mut focus_budget_target = None;
        let current_boot = self.project.boot;
        let boot_label = self.boot_target_label();
        // Play is a split button: the face runs the project; the arrow opens
        // the boot settings and the latest PSX envelope without starting the
        // game. Right to left, so the arrow comes first; no spacing between
        // the two makes them read as one control.
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.visuals_mut().widgets.hovered.weak_bg_fill = STUDIO_PLAY_FILL_HOVER;
            let arrow = egui::Button::new(
                icons::text(icons::CHEVRON_DOWN, 12.0).color(Color32::WHITE),
            )
            .fill(STUDIO_PLAY_FILL)
            .corner_radius(egui::CornerRadius {
                nw: 0,
                sw: 0,
                ne: CONTROL_RADIUS,
                se: CONTROL_RADIUS,
            })
            .min_size(Vec2::new(24.0, 24.0));
            egui::menu::menu_custom_button(ui, arrow, |ui| {
                self.draw_boot_menu(ui, current_boot, &mut set_boot, &mut focus_budget_target);
            })
            .response
            .on_hover_text(
                "Choose the boot target; inspect pre/post-cook PSX envelopes without starting Play.",
            );
            let face = egui::Button::new(
                RichText::new(icons::label(icons::PLAY, play_label))
                    .strong()
                    .color(Color32::WHITE),
            )
            .fill(STUDIO_PLAY_FILL)
            .corner_radius(egui::CornerRadius {
                nw: CONTROL_RADIUS,
                sw: CONTROL_RADIUS,
                ne: 0,
                se: 0,
            })
            .min_size(Vec2::new(60.0, 24.0));
            if ui
                .add(face)
                .on_hover_text(format!(
                    "Cook, build, and run inside the 3D viewport. Boots at: {}. Shortcut: {}.",
                    boot_label,
                    command_shortcut_text("Enter")
                ))
                .clicked()
            {
                self.request_play_or_rebuild(playtest_status);
            }
        });
        if let Some(target) = set_boot {
            self.set_boot_target(target);
        }
        if let Some(target) = focus_budget_target {
            let _ = self.focus_playtest_validation_target(target);
        }

        if playtest_active
            && ui
                .button(icons::label(icons::TRASH, "Stop"))
                .on_hover_text("Stop embedded play mode and return the viewport to editing.")
                .clicked()
        {
            self.pending_playtest_request = Some(EditorPlaytestRequest::Stop);
        }

        if ui
            .button(icons::label(icons::BOX, "Build"))
            .on_hover_text(format!(
                "Cook assets, build the runtime EXE, and export it into the launcher Projects list. Shortcut: {}.",
                command_shortcut_text("B")
            ))
            .clicked()
        {
            self.request_project_build();
        }

        self.draw_undo_redo_buttons(ui);

        if let Some(state) = self.action_bar_status(playtest_status) {
            ui.label(
                RichText::new(icons::label(state.icon, state.badge))
                    .small()
                    .strong()
                    .color(state.accent),
            )
            .on_hover_text(state.message);
        }

        // The Debug snapshot is in the Tools menu; the menu has no play
        // metrics, so it leaves the request here.
        if std::mem::take(&mut self.debug_snapshot_requested) {
            self.capture_debug_snapshot(play_metrics);
        }
    }

    /// Undo and Redo: the keys are the only other way to reach them in a
    /// tool that is all pointer work. Right to left, so Redo is drawn first.
    pub(crate) fn draw_undo_redo_buttons(&mut self, ui: &mut egui::Ui) {
        let can_undo = self.history.can_undo();
        let can_redo = self.history.can_redo();
        if ui
            .add_enabled(
                can_redo,
                egui::Button::new(icons::label(icons::REDO, "Redo")),
            )
            .on_hover_text(format!("Redo ({})", command_shift_shortcut_text("Z")))
            .clicked()
        {
            self.do_redo();
        }
        if ui
            .add_enabled(
                can_undo,
                egui::Button::new(icons::label(icons::UNDO, "Undo")),
            )
            .on_hover_text(format!("Undo ({})", command_shortcut_text("Z")))
            .clicked()
        {
            self.do_undo();
        }
    }

    /// The Play arrow's menu: the boot target and the PSX envelopes.
    fn draw_boot_menu(
        &self,
        ui: &mut egui::Ui,
        current_boot: BootTarget,
        set_boot: &mut Option<BootTarget>,
        focus_budget_target: &mut Option<psxed_project::playtest::PlaytestValidationTarget>,
    ) {
        // Only an open menu pays for the estimate.
        let budget = self.playtest_budget_for_menu();
        ui.label(RichText::new("Boot at").color(STUDIO_TEXT_WEAK).small());
        if ui
            .selectable_label(current_boot == BootTarget::Gameplay, "Gameplay")
            .clicked()
        {
            *set_boot = Some(BootTarget::Gameplay);
            ui.close_menu();
        }
        ui.separator();
        ui.label(
            RichText::new("Screen States")
                .color(STUDIO_TEXT_WEAK)
                .small(),
        );
        for state in &self.project.scene_states {
            let target = BootTarget::SceneState(state.id);
            if ui
                .selectable_label(current_boot == target, &state.name)
                .clicked()
            {
                *set_boot = Some(target);
                ui.close_menu();
            }
        }
        ui.separator();
        ui.label(RichText::new("UI Scenes").color(STUDIO_TEXT_WEAK).small());
        for scene in &self.project.ui_scenes {
            let target = BootTarget::UiScene(scene.id);
            if ui
                .selectable_label(current_boot == target, &scene.name)
                .clicked()
            {
                *set_boot = Some(target);
                ui.close_menu();
            }
        }
        ui.separator();
        let stage = match budget.stage {
            psxed_project::playtest::PlaytestBudgetStage::AuthoredEstimate => {
                "PSX budget estimate (before cook)"
            }
            psxed_project::playtest::PlaytestBudgetStage::Cooked => "PSX budget (last cook)",
        };
        ui.label(RichText::new(stage).color(STUDIO_TEXT_WEAK).small());
        ui.monospace(format!(
            "World     {} faces · {}",
            budget.world_faces,
            human_bytes_u64(budget.world_bytes as u64)
        ));
        ui.monospace(format!(
            "Lighting  {}",
            human_bytes_u64(budget.light_bytes as u64)
        ));
        ui.monospace(format!(
            "Textures  {}",
            human_bytes_u64(budget.texture_bytes as u64)
        ));
        ui.monospace(format!(
            "RAM       {}/{} · {}/{} slots",
            human_bytes_u64(budget.ram_bytes as u64),
            human_bytes_u64(psxed_project::playtest::PLAYTEST_RAM_PHYSICAL_BYTES as u64),
            budget.ram_asset_slots,
            psxed_project::playtest::PLAYTEST_RAM_ASSET_SLOT_LIMIT,
        ));
        ui.monospace(format!(
            "VRAM      {}/{} · {}/{} slots",
            human_bytes_u64(budget.vram_bytes as u64),
            human_bytes_u64(psxed_project::playtest::PLAYTEST_VRAM_PHYSICAL_BYTES as u64),
            budget.vram_asset_slots,
            psxed_project::playtest::PLAYTEST_VRAM_ASSET_SLOT_LIMIT,
        ));
        ui.monospace(format!(
            "Packets   {}/{}",
            budget.packet_count, budget.packet_limit,
        ));
        if !self.last_cook_errors.is_empty() {
            ui.separator();
            ui.label(
                RichText::new(format!(
                    "Last cook failed with {} error{}",
                    self.last_cook_errors.len(),
                    if self.last_cook_errors.len() == 1 {
                        ""
                    } else {
                        "s"
                    }
                ))
                .color(STUDIO_ERROR),
            );
            // Every row is focusable, not just the first: the author
            // reading error four wants error four's node.
            for error in &self.last_cook_errors {
                ui.horizontal_wrapped(|ui| {
                    ui.add(
                        egui::Label::new(RichText::new(&error.message).color(STUDIO_ERROR).small())
                            .selectable(true),
                    );
                });
                if let Some(target) = error.target {
                    if ui.small_button("Focus").clicked() {
                        *focus_budget_target = Some(target);
                        ui.close_menu();
                    }
                }
            }
        }
        if let Some(issue) = budget.first_actionable_issue() {
            ui.separator();
            ui.label(RichText::new(issue.message()).color(STUDIO_ERROR));
            if let Some(target) = issue.target {
                if ui.button("Focus budget offender").clicked() {
                    *focus_budget_target = Some(target);
                    ui.close_menu();
                }
            }
        } else {
            ui.weak("No host envelope exceeded; MIPS link/replay remain final gates.");
        }
    }
}
