//! The tool row: one line under the top bar, never wrapping.
//!
//! Select and Height first (no label), then CREATE (Floor, Wall, Ceiling and
//! a menu with Plane and Mesh), SHAPE (a split button: its face is the last
//! shape tool used, its arrow lists all nine), SURFACE (Paint, Bands) and
//! OBJECTS (a Place split button whose menu lists Light, Prop and
//! Actor). Every tool shows its key beside its name.

use super::*;

/// Height of one button of the row.
const BUTTON_HEIGHT: f32 = 26.0;
/// Width of a split button's arrow.
const ARROW_WIDTH: f32 = 18.0;

/// The kinds of the OBJECTS menu: the word on the menu, the node it places.
const OBJECT_KINDS: [(&str, PlaceKind); 3] = [
    ("Light", PlaceKind::PointLightMarker),
    ("Prop", PlaceKind::ModelInstance),
    ("Actor", PlaceKind::Character),
];

/// Which corners of a painted button are round: a split button's face is
/// round on the left and its arrow on the right, so they read as one.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Corners {
    All,
    Left,
}

fn radius(corners: Corners) -> egui::CornerRadius {
    match corners {
        Corners::All => egui::CornerRadius::same(CONTROL_RADIUS),
        Corners::Left => egui::CornerRadius {
            nw: CONTROL_RADIUS,
            sw: CONTROL_RADIUS,
            ne: 0,
            se: 0,
        },
    }
}

/// A button with the tool's name and, in weak small type beside it, its key.
/// `min_width` keeps a button that changes its word (the SHAPE face) from
/// moving what follows it.
fn name_and_key_button(
    ui: &mut egui::Ui,
    name: &str,
    key: &str,
    selected: bool,
    corners: Corners,
    min_width: f32,
) -> egui::Response {
    let (name_galley, key_galley) = name_and_key_galleys(ui, name, key);
    let width = (9.0 + name_galley.size().x + 6.0 + key_galley.size().x + 9.0).max(min_width);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, BUTTON_HEIGHT), Sense::click());
    if selected {
        ui.painter()
            .rect_filled(rect, radius(corners), STUDIO_SELECTION);
        ui.painter().rect_stroke(
            rect,
            radius(corners),
            Stroke::new(1.0, STUDIO_ACCENT),
            egui::StrokeKind::Inside,
        );
    } else {
        // A split button's face is a boxed control, so its arrow reads as
        // part of it; a plain tool is flat until hovered.
        if corners == Corners::Left {
            ui.painter()
                .rect_filled(rect, radius(corners), STUDIO_INPUT);
            ui.painter().rect_stroke(
                rect,
                radius(corners),
                Stroke::new(1.0, STUDIO_BORDER),
                egui::StrokeKind::Inside,
            );
        }
        if response.hovered() {
            ui.painter()
                .rect_filled(rect, radius(corners), STUDIO_HOVER);
        }
    }
    let name_pos = Pos2::new(
        rect.min.x + 9.0,
        rect.center().y - name_galley.size().y / 2.0,
    );
    let key_pos = Pos2::new(
        name_pos.x + name_galley.size().x + 6.0,
        rect.center().y - key_galley.size().y / 2.0 + 1.0,
    );
    ui.painter().galley(name_pos, name_galley, STUDIO_TEXT);
    ui.painter().galley(key_pos, key_galley, STUDIO_TEXT_WEAK);
    response
}

fn name_and_key_galleys(
    ui: &egui::Ui,
    name: &str,
    key: &str,
) -> (std::sync::Arc<egui::Galley>, std::sync::Arc<egui::Galley>) {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let key_font = egui::FontId::proportional(10.5);
    ui.fonts(|f| {
        (
            f.layout_no_wrap(name.to_string(), font, STUDIO_TEXT),
            f.layout_no_wrap(key.to_string(), key_font, STUDIO_TEXT_WEAK),
        )
    })
}

/// The width a button needs for the widest of `tools`.
fn widest(ui: &egui::Ui, tools: &[GridTool]) -> f32 {
    tools
        .iter()
        .map(|t| {
            let (n, k) = name_and_key_galleys(ui, t.label(), t.key_label());
            9.0 + n.size().x + 6.0 + k.size().x + 9.0
        })
        .fold(0.0, f32::max)
}

/// The small word that names a group.
fn group_label(ui: &mut egui::Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(text).color(STUDIO_TEXT_WEAK).size(9.5));
    ui.add_space(2.0);
}

/// A thin line between two groups.
fn group_gap(ui: &mut egui::Ui) {
    ui.add_space(6.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, 16.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, STUDIO_BORDER);
    ui.add_space(6.0);
}

fn tool_hover(tool: GridTool) -> String {
    format!(
        "{} ({})\n{}",
        tool.label(),
        tool.key_label(),
        tool.tooltip()
    )
}

/// One item of a tool menu: the name, its key at the right edge.
fn menu_tool_item(ui: &mut egui::Ui, tool: GridTool, selected: bool) -> bool {
    let clicked = ui
        .add(
            egui::Button::new(tool.label())
                .shortcut_text(RichText::new(tool.key_label()).color(STUDIO_TEXT_WEAK))
                .selected(selected)
                .min_size(Vec2::new(150.0, 22.0)),
        )
        .on_hover_text(tool_hover(tool))
        .clicked();
    if clicked {
        ui.close_menu();
    }
    clicked
}

/// The arrow of a split button, or a menu button alone: flat until hovered,
/// lit while one of its tools is the active one.
fn arrow_menu(
    ui: &mut egui::Ui,
    selected: bool,
    hover: &str,
    corners: Corners,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    ui.scope(|ui| {
        // Boxed, so a lone arrow (CREATE) is found on the row.
        ui.visuals_mut().widgets.inactive.weak_bg_fill = STUDIO_INPUT;
        ui.visuals_mut().widgets.inactive.bg_stroke = Stroke::new(1.0, STUDIO_BORDER);
        let corner = match corners {
            Corners::All => egui::CornerRadius::same(CONTROL_RADIUS),
            Corners::Left => egui::CornerRadius {
                nw: 0,
                sw: 0,
                ne: CONTROL_RADIUS,
                se: CONTROL_RADIUS,
            },
        };
        let button = egui::Button::new(icons::text(icons::CHEVRON_DOWN, 12.0).color(STUDIO_TEXT))
            .selected(selected)
            .corner_radius(corner)
            .min_size(Vec2::new(ARROW_WIDTH, BUTTON_HEIGHT));
        ui.set_min_width(ARROW_WIDTH);
        egui::menu::menu_custom_button(ui, button, |ui| {
            ui.set_min_width(150.0);
            add_contents(ui);
        })
        .response
        .on_hover_text(hover);
    });
}

impl EditorWorkspace {
    /// The row itself, a panel under the top bar across the whole window.
    pub(crate) fn draw_grid_tool_row(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("psxed_tool_row")
            .exact_height(TOOL_ROW_HEIGHT)
            .frame(
                egui::Frame::new()
                    .fill(STUDIO_PANEL_HEADER)
                    .inner_margin(egui::Margin::symmetric(10, 5)),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);
                ui.horizontal_centered(|ui| {
                    ui.set_clip_rect(ui.max_rect().expand2(Vec2::new(0.0, 4.0)));
                    self.draw_tool_row_items(ui);
                });
            });
    }

    fn draw_tool_row_items(&mut self, ui: &mut egui::Ui) {
        // Select and Height open the row, with no label.
        self.row_tool(ui, GridTool::Select);
        self.row_tool(ui, GridTool::Height);
        group_gap(ui);

        group_label(ui, ToolGroup::Create.label());
        for tool in [GridTool::Floor, GridTool::Wall, GridTool::Ceiling] {
            self.row_tool(ui, tool);
        }
        let more_active = matches!(self.grid.tool, GridTool::Plane | GridTool::Mesh);
        let mut chosen = None;
        arrow_menu(
            ui,
            more_active,
            "More: Plane and Mesh",
            Corners::All,
            |ui| {
                for tool in [GridTool::Plane, GridTool::Mesh] {
                    if menu_tool_item(ui, tool, self.grid.tool == tool) {
                        chosen = Some(tool);
                    }
                }
            },
        );
        if let Some(tool) = chosen {
            self.set_grid_tool(tool);
        }
        group_gap(ui);

        group_label(ui, ToolGroup::Shape.label());
        self.row_shape_button(ui);
        group_gap(ui);

        group_label(ui, ToolGroup::Surface.label());
        for tool in [GridTool::Paint, GridTool::Bands] {
            self.row_tool(ui, tool);
        }
        group_gap(ui);

        group_label(ui, ToolGroup::Objects.label());
        self.row_place_button(ui);
    }

    /// A plain tool button: its name, its key, lit while it is the tool.
    fn row_tool(&mut self, ui: &mut egui::Ui, tool: GridTool) {
        let response = name_and_key_button(
            ui,
            tool.label(),
            tool.key_label(),
            self.grid.tool == tool,
            Corners::All,
            0.0,
        )
        .on_hover_text(tool_hover(tool));
        if response.clicked() {
            self.set_grid_tool(tool);
        }
    }

    /// SHAPE: the face picks the last shape tool used, the arrow lists all.
    fn row_shape_button(&mut self, ui: &mut egui::Ui) {
        let last = self.grid.last_shape;
        let active = self.grid.tool.group() == ToolGroup::Shape;
        let width = widest(ui, &GridTool::SHAPES);
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let face = name_and_key_button(
                ui,
                last.label(),
                last.key_label(),
                active,
                Corners::Left,
                width,
            )
            .on_hover_text(tool_hover(last));
            let mut chosen = face.clicked().then_some(last);
            arrow_menu(
                ui,
                active,
                "Shape tools: Extrude, Ramp, Stairs, Spiral, Break, Rough, Cells, Upper floor, Diagonal",
                Corners::Left,
                |ui| {
                    for tool in GridTool::SHAPES {
                        if menu_tool_item(ui, tool, self.grid.tool == tool) {
                            chosen = Some(tool);
                        }
                    }
                },
            );
            if let Some(tool) = chosen {
                self.set_grid_tool(tool);
            }
        });
    }

    /// OBJECTS: the face picks Place with the kind it has, the arrow lists
    /// the kinds.
    fn row_place_button(&mut self, ui: &mut egui::Ui) {
        let active = self.grid.tool == GridTool::Place;
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let face = name_and_key_button(
                ui,
                GridTool::Place.label(),
                GridTool::Place.key_label(),
                active,
                Corners::Left,
                0.0,
            )
            .on_hover_text(tool_hover(GridTool::Place));
            if face.clicked() {
                self.set_grid_tool(GridTool::Place);
            }
            let mut chosen = None;
            arrow_menu(
                ui,
                active,
                "What Place puts down: Light, Spawn, Prop, Entity",
                Corners::Left,
                |ui| {
                    for (label, kind) in OBJECT_KINDS {
                        let selected = active && self.place_kind == kind;
                        if toolbar_menu_choice(ui, label, selected) {
                            chosen = Some(kind);
                        }
                    }
                },
            );
            if let Some(kind) = chosen {
                self.place_kind = kind;
                self.set_grid_tool(GridTool::Place);
                self.status = format!("Tool: Place, {}", kind.label());
            }
        });
    }
}
