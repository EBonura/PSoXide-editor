//! Verification pass: the step the Blender MCP has no equivalent for.
//!
//! Blender has no shipping budget. PSoXide does, and it is what makes a level
//! unshippable: an open sightline that puts every face of a colonnade in one
//! visibility leaf costs more than the frame has. Catching that at blockout
//! beats catching it after texturing and lighting.
//!
//! Three passes, cheapest first, because the last one is a full cook:
//!
//! 1. Geometry the brush kernel itself can reject: degenerate solids, faces
//!    sharing a plane (the z-fighting case the editor's own audit looks for),
//!    coordinates off the working grid.
//! 2. Sealing, via the brush-world leak diagnostic: does the interior connect
//!    to the void.
//! 3. The cook, for validation errors and per-leaf draw cost.

use std::fmt::Write as _;
use std::path::Path;
use std::time::Instant;

use psxed_project::brush::BRUSH_EDIT_EXTENT_LIMIT;
use psxed_project::brush_overlap::find_brush_face_overlaps;
use psxed_project::brush_world::diagnose_brush_world_leak;
use psxed_project::playtest::{analyze_pxbsp_draw_cost, build_package};
use psxed_project::ProjectDocument;

use crate::resolve_scene;

/// How much of the audit to run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditDepth {
    /// Geometry only: milliseconds, safe to run after every edit.
    Quick,
    /// Geometry plus sealing.
    Sealing,
    /// Everything, including a full cook for draw cost. Seconds.
    Full,
}

impl AuditDepth {
    /// Parse the `depth` tool argument.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.to_ascii_lowercase().as_str() {
            "quick" | "geometry" => Ok(Self::Quick),
            "sealing" | "leak" => Ok(Self::Sealing),
            "full" | "cook" => Ok(Self::Full),
            other => Err(format!(
                "unknown depth {other:?}: use quick, sealing or full"
            )),
        }
    }
}

/// Run the audit and render it as a report.
pub fn audit(
    project: &ProjectDocument,
    project_root: &Path,
    scene_index: Option<usize>,
    depth: AuditDepth,
    grid: i32,
    range: Option<(usize, usize)>,
) -> Result<String, String> {
    let index = resolve_scene(project, scene_index)?;
    let scene = &project.scenes[index];
    let mut out = String::new();
    let _ = writeln!(out, "# Audit of scene {index}: {:?}\n", scene.name);

    // A range keeps an incremental audit about the work just done. Without it
    // an established level's own history drowns the new section: the shipped
    // v0.4 map already carries 141 coplanar overlaps of its own.
    let (from, to) = match range {
        Some((first, count)) => {
            let to = first.saturating_add(count).min(scene.brushes.len());
            if first >= scene.brushes.len() {
                return Err(format!(
                    "brush {first} is out of range: scene {index} has {}",
                    scene.brushes.len()
                ));
            }
            let _ = writeln!(
                out,
                "Scoped to brushes {first}..{to}; the rest of the scene is ignored.\n"
            );
            (first, to)
        }
        None => (0, scene.brushes.len()),
    };
    let in_range = |position: usize| (from..to).contains(&position);

    // --- 1. Geometry ------------------------------------------------------
    let audited = &scene.brushes[from..to];
    let faces: usize = audited.iter().map(|brush| brush.faces.len()).sum();
    let _ = writeln!(out, "## Geometry\n");
    let _ = writeln!(out, "{} brushes, {faces} faces", audited.len());

    let mut degenerate = Vec::new();
    let mut oversized = Vec::new();
    for (position, brush) in audited.iter().enumerate().map(|(i, b)| (i + from, b)) {
        let solved = brush.solve();
        if !solved.is_valid() {
            degenerate.push(position);
        } else if !solved.within_extent(BRUSH_EDIT_EXTENT_LIMIT) {
            oversized.push(position);
        }
    }
    report_indices(&mut out, "degenerate (enclose no volume)", &degenerate);
    report_indices(
        &mut out,
        &format!("beyond the {BRUSH_EDIT_EXTENT_LIMIT:.0}-unit extent limit"),
        &oversized,
    );

    // Run the overlap search over the WHOLE scene even when scoped: a new
    // wall sharing a plane with an existing one is exactly the defect worth
    // catching, so only report pairs that touch the range.
    let overlaps: Vec<_> = find_brush_face_overlaps(&scene.brushes)
        .into_iter()
        .filter(|overlap| in_range(overlap.brush_a) || in_range(overlap.brush_b))
        .collect();
    if overlaps.is_empty() {
        out.push_str("coplanar face overlaps: none\n");
    } else {
        let total: f64 = overlaps.iter().map(|overlap| overlap.area).sum();
        let _ = writeln!(
            out,
            "coplanar face overlaps: {} covering {total:.0} sq units. Two faces on the \
             same plane fight for the same pixels at runtime; delete one or move it off \
             the shared plane.",
            overlaps.len()
        );
        let mut worst = overlaps.clone();
        worst.sort_by(|a, b| b.area.total_cmp(&a.area));
        for overlap in worst.iter().take(8) {
            let _ = writeln!(
                out,
                "  brush {} face {} against brush {} face {}: {:.0} sq units",
                overlap.brush_a, overlap.face_a, overlap.brush_b, overlap.face_b, overlap.area
            );
        }
    }

    let grid = grid.max(1);
    let (off_grid, total_coords) = count_off_grid(audited, grid);
    if off_grid == 0 {
        let _ = writeln!(out, "grid {grid}: every authored coordinate is aligned");
    } else {
        let _ = writeln!(
            out,
            "grid {grid}: {off_grid} of {total_coords} authored coordinates are off it \
             ({:.1}%)",
            100.0 * off_grid as f64 / total_coords.max(1) as f64
        );
    }

    let untextured = audited
        .iter()
        .flat_map(|brush| brush.faces.iter())
        .filter(|face| face.material.is_none())
        .count();
    if untextured > 0 {
        let _ = writeln!(
            out,
            "{untextured} of {faces} faces have no material and will cook untextured"
        );
    }

    out.push_str(
        "\nNote: this finds faces sharing a PLANE, which z-fight. Solids that merely \
         interpenetrate are legal here (BrushContents::precedence resolves them, and \
         the shipped level relies on it), so a pillar passing through a stair is a \
         look-at-it decision, not an error. Check a plan_view for those.\n",
    );

    if depth == AuditDepth::Quick {
        out.push_str("\n(quick audit: pass depth=\"sealing\" or \"full\" for the rest)\n");
        return Ok(out);
    }

    // --- 2. Sealing -------------------------------------------------------
    let _ = writeln!(out, "\n## Sealing\n");
    let started = Instant::now();
    match diagnose_brush_world_leak(project.clone()) {
        Ok(diagnostic) if diagnostic.is_empty() => {
            out.push_str("sealed: no path from the interior to the void\n");
        }
        Ok(diagnostic) => {
            let _ = writeln!(
                out,
                "LEAKS. A {}-point path escapes to the void. The engine cannot compute \
                 visibility through a leak, so the whole map falls back to drawing \
                 everything.",
                diagnostic.path.len()
            );
            if let Some(first) = diagnostic.path.first() {
                let authored = first.map(|value| value * crate::WORLD_UNIT_DIVISOR);
                let _ = writeln!(
                    out,
                    "  path starts at {first:?} engine = {authored:?} authored, which is the \
                     OCCUPANT the flood starts from, normally the player."
                );
                out.push_str(
                    "  Check that point is inside your geometry first. A player left at another \
                     level's coordinates reports every map as leaking, however well sealed it is.\n",
                );
            }
            if let Some(opening) = diagnostic.likely_opening.first() {
                let _ = writeln!(out, "  likely opening near {opening:?} (engine units)");
            }
            out.push_str(
                "  Note these are ENGINE units: multiply by 16 for authored coordinates.\n",
            );
        }
        Err(error) => {
            let _ = writeln!(out, "the leak check could not run: {error}");
        }
    }
    let _ = writeln!(out, "(sealing check took {:.1}s)", started.elapsed().as_secs_f64());

    if depth == AuditDepth::Sealing {
        out.push_str("\n(pass depth=\"full\" to cook and measure draw cost)\n");
        return Ok(out);
    }

    // --- 3. The cook ------------------------------------------------------
    let _ = writeln!(out, "\n## Cook and draw cost\n");
    let started = Instant::now();
    let (package, validation) = build_package(project, project_root);
    for error in &validation.errors {
        let _ = writeln!(out, "ERROR: {}", error.message);
    }
    for warning in validation.warnings.iter().take(20) {
        let _ = writeln!(out, "warning: {warning}");
    }
    if validation.warnings.len() > 20 {
        let _ = writeln!(
            out,
            "... and {} more warnings",
            validation.warnings.len() - 20
        );
    }
    let Some(package) = package else {
        out.push_str("the project did not cook, so there is no draw cost to report\n");
        return Ok(out);
    };

    match analyze_pxbsp_draw_cost(&package) {
        Ok(Some(report)) => {
            let _ = writeln!(
                out,
                "\n{} world faces, {} base triangles, {} all-face packet slots, \
                 {} non-solid leaves",
                report.world_face_count,
                report.world_base_triangle_count,
                report.world_base_packet_slots,
                report.non_solid_leaf_count
            );
            if report.unreadable_pvs_leaf_count > 0 {
                let _ = writeln!(
                    out,
                    "{} leaves had an undecodable PVS row, which usually means the map leaks",
                    report.unreadable_pvs_leaf_count
                );
            }
            out.push_str(
                "\nHeaviest leaves. `base packet slots` is the figure to watch: it is what \
                 one frame has to push from a camera standing in that leaf.\n",
            );
            for leaf in report.heaviest_leaves(8) {
                let anchor = leaf.authored_surface_anchor.map_or_else(
                    || "anchor unavailable".to_string(),
                    |[x, y, z]| format!("near authored ({x}, {y}, {z})"),
                );
                let _ = writeln!(
                    out,
                    "  leaf {}: {} packet slots, {} visible faces across {} visible leaves, {anchor}",
                    leaf.leaf_index,
                    leaf.base_packet_slots,
                    leaf.visible_face_count,
                    leaf.visible_leaf_count
                );
            }
            if let Some(worst) = report.worst_leaf() {
                let _ = writeln!(
                    out,
                    "\nWorst sightline is leaf {} at {} packet slots. If that number climbed \
                     after an edit, the edit opened a sightline; break it with geometry \
                     rather than by deleting detail.",
                    worst.leaf_index, worst.base_packet_slots
                );
            }
        }
        Ok(None) => out.push_str("the package has no PXBSP world geometry to measure\n"),
        Err(error) => {
            let _ = writeln!(out, "draw-cost analysis failed: {error}");
        }
    }
    let _ = writeln!(out, "\n(cook took {:.1}s)", started.elapsed().as_secs_f64());
    Ok(out)
}

fn report_indices(out: &mut String, label: &str, indices: &[usize]) {
    if indices.is_empty() {
        let _ = writeln!(out, "{label}: none");
        return;
    }
    let shown: Vec<String> = indices.iter().take(12).map(usize::to_string).collect();
    let _ = writeln!(
        out,
        "{label}: {} (brushes {}{})",
        indices.len(),
        shown.join(", "),
        if indices.len() > shown.len() {
            ", ..."
        } else {
            ""
        }
    );
}

/// Authored plane-point coordinates that do not sit on the working grid.
fn count_off_grid(brushes: &[psxed_project::brush::Brush], grid: i32) -> (usize, usize) {
    let mut off = 0usize;
    let mut total = 0usize;
    for point in brushes
        .iter()
        .flat_map(|brush| brush.faces.iter())
        .flat_map(|face| face.points.iter())
        .flat_map(|point| point.iter())
    {
        total += 1;
        if point % grid != 0 {
            off += 1;
        }
    }
    (off, total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use psxed_project::brush::Brush;

    /// The quick pass has to name the two defects an agent authors by
    /// accident: a face sharing a plane with another, and a coordinate that
    /// missed the grid. Both used to be invisible until the cook or the eye.
    #[test]
    fn quick_audit_names_coplanar_overlaps_and_off_grid_points() {
        let mut project = ProjectDocument::default();
        // `ProjectDocument::default()` is the embedded starter level.
        project.scenes[0].brushes.clear();
        let scene = &mut project.scenes[0];
        // Two slabs sharing their top plane over the same footprint.
        scene.brushes.push(Brush::cuboid([0, 0, 0], [1024, 256, 1024]));
        scene.brushes.push(Brush::cuboid([0, 0, 0], [1024, 256, 1024]));
        // One brush deliberately off the 64 grid.
        scene.brushes.push(Brush::cuboid([7, 0, 7], [1031, 256, 1031]));

        let report = audit(&project, Path::new("."), None, AuditDepth::Quick, 64, None)
            .expect("the quick audit runs");
        assert!(report.contains("coplanar face overlaps: "), "{report}");
        assert!(!report.contains("coplanar face overlaps: none"), "{report}");
        assert!(report.contains("are off it"), "{report}");
        assert!(report.contains("degenerate (enclose no volume): none"), "{report}");
        // The quick pass must not cook; it says so instead.
        assert!(report.contains("quick audit"), "{report}");

        // A clean scene reports clean.
        project.scenes[0].brushes.truncate(1);
        let clean = audit(&project, Path::new("."), None, AuditDepth::Quick, 64, None)
            .expect("the quick audit runs");
        assert!(clean.contains("coplanar face overlaps: none"), "{clean}");
        assert!(clean.contains("every authored coordinate is aligned"), "{clean}");

        // Scoping hides the rest of the scene: brush 0 alone is clean even
        // while its coplanar twin is still present.
        project.scenes[0].brushes.push(Brush::cuboid([0, 0, 0], [1024, 256, 1024]));
        let scoped = audit(&project, Path::new("."), None, AuditDepth::Quick, 64, Some((0, 1)))
            .expect("a scoped audit runs");
        assert!(scoped.contains("Scoped to brushes 0..1"), "{scoped}");
        // The pair still touches brush 0, so it is still reported: a scoped
        // audit hides unrelated history, not collisions with it.
        assert!(!scoped.contains("coplanar face overlaps: none"), "{scoped}");
        assert!(scoped.contains("1 brushes, 6 faces"), "{scoped}");

        // An out-of-range scope is an error, not an empty report.
        assert!(audit(&project, Path::new("."), None, AuditDepth::Quick, 64, Some((99, 1))).is_err());
    }
}
