//! The preview's "too much on screen" report.
//!
//! The 3D preview submits triangles into fixed pools. When a frame needs
//! more than a pool holds, the frontend reports how many primitives it had
//! to leave out, and the viewport says so in red instead of silently
//! drawing a level with holes in it.

use std::sync::atomic::{AtomicU32, Ordering};

static DROPPED: AtomicU32 = AtomicU32::new(0);

/// Called by the preview builder at the end of every frame with the number
/// of primitives (or whole sectors) it could not draw.
pub fn report_preview_overflow(dropped: u32) {
    DROPPED.store(dropped, Ordering::Relaxed);
}

/// Primitives left out of the last preview frame.
pub(crate) fn preview_overflow() -> u32 {
    DROPPED.load(Ordering::Relaxed)
}

/// The warning to show over the 3D view, if the last frame overflowed.
pub(crate) fn preview_overflow_message(dropped: u32) -> Option<String> {
    (dropped > 0).then(|| {
        format!(
            "PREVIEW FULL: {dropped} primitives not drawn. The picture has holes; \
             hide rooms or look at less of the level."
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_dropped_says_nothing_and_a_drop_says_how_many() {
        assert_eq!(preview_overflow_message(0), None);
        let text = preview_overflow_message(37).unwrap();
        assert!(text.contains("37") && text.contains("PREVIEW FULL"));
    }

    #[test]
    fn the_report_round_trips() {
        report_preview_overflow(12);
        assert_eq!(preview_overflow(), 12);
        report_preview_overflow(0);
        assert_eq!(preview_overflow(), 0);
    }
}
