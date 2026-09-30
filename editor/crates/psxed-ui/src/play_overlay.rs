use super::*;

pub(crate) fn human_bytes(n: u32) -> String {
    human_bytes_u64(n as u64)
}

pub(crate) fn human_bytes_u64(n: u64) -> String {
    if n < 1024 {
        format!("{} B", n)
    } else if n < 1024 * 1024 {
        format!("{:.1} KB", (n as f64) / 1024.0)
    } else {
        format!("{:.1} MB", (n as f64) / (1024.0 * 1024.0))
    }
}

pub(crate) fn draw_play_overlay_icon_button(
    ui: &mut egui::Ui,
    rect: Rect,
    id_source: &'static str,
    icon: char,
    tooltip: &'static str,
    active: bool,
    enabled: bool,
    active_fill: Option<Color32>,
) -> bool {
    let response = ui
        .interact(
            rect,
            ui.id().with(("play_overlay_icon_button", id_source)),
            if enabled {
                Sense::click()
            } else {
                Sense::hover()
            },
        )
        .on_hover_text(tooltip);
    let hovered = response.hovered();
    let fill = if active {
        active_fill.unwrap_or(STUDIO_ACCENT_DIM)
    } else if hovered && enabled {
        Color32::from_rgba_unmultiplied(34, 48, 58, 232)
    } else if enabled {
        Color32::from_black_alpha(176)
    } else {
        Color32::from_rgba_unmultiplied(0, 0, 0, 112)
    };
    let stroke = if active {
        Stroke::new(1.0, STUDIO_ACCENT)
    } else if hovered && enabled {
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(210, 220, 235, 128))
    } else {
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(210, 220, 235, 84))
    };
    let icon_color = if !enabled {
        Color32::from_rgba_unmultiplied(142, 154, 168, 108)
    } else if hovered || active {
        Color32::WHITE
    } else {
        STUDIO_TEXT
    };
    let painter = ui.painter();
    painter.rect_filled(rect, 4.0, fill);
    painter.rect_stroke(rect, 4.0, stroke, StrokeKind::Inside);
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        icon.to_string(),
        icons::font(14.0),
        icon_color,
    );
    enabled && response.clicked()
}

pub(crate) fn q12_degrees(angle: u16) -> f32 {
    angle as f32 * 360.0 / 4096.0
}
