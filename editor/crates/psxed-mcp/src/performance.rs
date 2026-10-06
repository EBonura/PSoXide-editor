//! Per-area BSP costs with optional explicit project authoring budgets.
//! PVS costs are conservative candidate sets, not submitted faces or predicted FPS.
use psxed_project::{
    playtest::{
        analyze_pxbsp_draw_cost, build_package, pxbsp_leaves_at_authored_points,
        PxbspDrawCostReport, PxbspLeafDrawCost,
    },
    ProjectDocument,
};
use serde::{Deserialize, Serialize};
use std::{fmt::Write as _, path::Path};

/// Version of the static measurement contract, not a calibrated FPS profile.
pub const PROFILE: &str = "pxbsp-area-costs-v2";

/// Optional project-specific authoring limits. No default frame-rate cap is inferred.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct DesignBudget {
    /// Authored ceiling for PVS candidate faces. Does not predict submitted geometry or FPS.
    pub pvs_faces: usize,
    /// Authored ceiling for base world triangles before clipping/subdivision.
    pub base_world_triangles: usize,
    /// Optional caller-chosen enemy allowance; mesh/animation costs still need measurement.
    pub enemies: Option<usize>,
}

/// Named authoring area with explicit camera samples and planned actor load.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AreaSamples {
    /// Area name, e.g. arrival court or bridge exit. Reuse one name for its camera samples.
    pub name: String,
    /// Camera positions in AUTHORED units, including eye height. Sample entrances,
    /// exits, combat positions and camera orbit extremes. Points inside solids are errors.
    pub samples: Vec<[i32; 3]>,
    /// Maximum concurrently visible/active Custodian enemies. Omit when unknown. This is a planning assumption, not an inferred engine limit.
    pub enemies: Option<usize>,
}

#[derive(Debug, Serialize)]
struct Cost {
    leaf: usize,
    status: &'static str,
    pvs_faces: usize,
    sky_aperture_faces: usize,
    base_triangles: usize,
    base_packet_slots: usize,
    faces_remaining: Option<i64>,
    triangles_remaining: Option<i64>,
    approximate_surface_anchor: Option<[i32; 3]>,
}
fn status(
    faces: usize,
    triangles: usize,
    enemies: Option<usize>,
    budget: Option<&DesignBudget>,
) -> &'static str {
    let Some(budget) = budget else {
        return "runtime_measurement_required";
    };
    if faces > budget.pvs_faces
        || triangles > budget.base_world_triangles
        || enemies
            .zip(budget.enemies)
            .is_some_and(|(actual, limit)| actual > limit)
    {
        "over_authored_budget"
    } else if budget.enemies.is_some() && enemies.is_none() {
        "actor_load_unspecified"
    } else {
        "within_authored_budget_runtime_unverified"
    }
}
fn cost(leaf: &PxbspLeafDrawCost, enemies: Option<usize>, budget: Option<&DesignBudget>) -> Cost {
    Cost {
        leaf: leaf.leaf_index,
        status: status(
            leaf.visible_face_count,
            leaf.base_triangle_count,
            enemies,
            budget,
        ),
        pvs_faces: leaf.visible_face_count,
        sky_aperture_faces: leaf.visible_sky_aperture_face_count,
        base_triangles: leaf.base_triangle_count,
        base_packet_slots: leaf.base_packet_slots,
        faces_remaining: budget.map(|b| b.pvs_faces as i64 - leaf.visible_face_count as i64),
        triangles_remaining: budget
            .map(|b| b.base_world_triangles as i64 - leaf.base_triangle_count as i64),
        approximate_surface_anchor: leaf.authored_surface_anchor,
    }
}

/// Included in full audit without a second cook.
pub fn audit_summary(report: &PxbspDrawCostReport) -> String {
    let mut text = String::from("\nFrame-rate budget: UNCALIBRATED. The former 120-face / 240-triangle / one-enemy default has been withdrawn. It extrapolated instrumented, camera-specific fixtures into a general Play limit.\n");
    if let Some(worst) = report.worst_leaf() {
        let _ = writeln!(text, "Heaviest PVS: {} candidate faces, {} base triangles. These are not actual submitted geometry or a prediction of FPS.", worst.visible_face_count, worst.base_triangle_count);
    }
    text.push_str("Use area_budget for named samples and optional explicit project budgets. Establish performance with a normal Play build and host display-cadence logs; use instrumented builds only for diagnosis and measure their overhead separately.\n");
    text
}

/// Cook staged content once and report per-leaf and sampled-area budget margins.
pub fn area_budget(
    project: &ProjectDocument,
    root: &Path,
    areas: &[AreaSamples],
    budget: Option<&DesignBudget>,
) -> Result<String, String> {
    if areas.len() > 128 || areas.iter().map(|a| a.samples.len()).sum::<usize>() > 4096 {
        return Err("at most 128 areas / 4096 camera samples per call".into());
    }
    for area in areas {
        if area.name.trim().is_empty() || area.samples.is_empty() {
            return Err("every area needs a nonempty name and at least one camera sample".into());
        }
    }
    if budget.is_some_and(|b| b.pvs_faces == 0 || b.base_world_triangles == 0) {
        return Err("explicit geometry budgets must be positive".into());
    }
    let sealing = psxed_project::brush_world::diagnose_brush_world_leak(project.clone())
        .map_err(|e| format!("could not verify sealing: {e}"))?;
    let sealed = sealing.is_empty();
    let sky_modes: Vec<_> = project
        .scenes
        .iter()
        .flat_map(|s| s.nodes().iter())
        .filter_map(|n| {
            if let psxed_project::NodeKind::World { sky, .. } = &n.kind {
                Some(format!("{:?}", sky.mode))
            } else {
                None
            }
        })
        .collect();
    let (package, validation) = build_package(project, root);
    if !validation.errors.is_empty() {
        return Err(validation
            .errors
            .iter()
            .map(|e| e.message.as_str())
            .collect::<Vec<_>>()
            .join("\n"));
    }
    let package = package.ok_or("project did not cook")?;
    let report = analyze_pxbsp_draw_cost(&package)?.ok_or("no PXBSP world")?;
    let points: Vec<_> = areas
        .iter()
        .flat_map(|a| a.samples.iter().copied())
        .collect();
    let leaves = pxbsp_leaves_at_authored_points(&package, &points)?;
    let mut offset = 0;
    let mut area_reports = Vec::new();
    for area in areas {
        let enemies = area.enemies;
        let mut sample_reports = Vec::new();
        let mut max_faces = 0;
        let mut max_triangles = 0;
        let mut invalid = false;
        for point in &area.samples {
            let leaf = leaves[offset];
            offset += 1;
            let found = leaf.and_then(|index| report.leaves.iter().find(|l| l.leaf_index == index));
            if let Some(found) = found {
                max_faces = max_faces.max(found.visible_face_count);
                max_triangles = max_triangles.max(found.base_triangle_count);
                sample_reports.push(
                    serde_json::json!({"position": point, "cost": cost(found, enemies, budget)}),
                );
            } else {
                invalid = true;
                sample_reports.push(serde_json::json!({"position": point, "leaf": leaf,
                    "status": "invalid_sample", "reason": "solid/exterior leaf or unreadable PVS; move the camera sample into playable space"}));
            }
        }
        let status = if !sealed {
            "invalid_sealing"
        } else if invalid {
            "invalid_samples"
        } else {
            status(max_faces, max_triangles, enemies, budget)
        };
        area_reports.push(
            serde_json::json!({"name": area.name, "assumed_enemies": enemies,
            "status": status, "max_pvs_faces": max_faces, "max_base_triangles": max_triangles,
            "faces_remaining": budget.map(|b| b.pvs_faces as i64 - max_faces as i64),
            "triangles_remaining": budget.map(|b| b.base_world_triangles as i64 - max_triangles as i64),
            "samples": sample_reports}),
        );
    }
    let over = budget.map(|b| {
        report
            .leaves
            .iter()
            .filter(|l| {
                l.visible_face_count > b.pvs_faces || l.base_triangle_count > b.base_world_triangles
            })
            .count()
    });
    serde_json::to_string_pretty(&serde_json::json!({
        "schema_version": 2, "profile": PROFILE,
        "profile_status": "no_universal_fps_ceiling_calibrated", "limits": budget,
        "withdrawn_default": {"pvs_faces":120,"base_world_triangles":240,"enemies":1,
            "reason":"Not validated for normal Play. Derived from instrumented fixtures and PVS upper bounds rather than actual per-view submitted work; do not restrict levels or enemy counts using it."},
        "performance_evidence": "benchmarks/engine-stress/valley-recheck.md",
        "meaning": "PVS includes potential geometry in adjacent areas before frustum/backface culling. Base triangles exclude sky and runtime subdivision. Null remaining budgets mean no explicit project budget was supplied, not zero headroom. Supplied budgets are authoring constraints, not calibrated FPS limits.",
        "runtime_acceptance": "Use the normal Play guest, without emulator-telemetry, and host route/display logs on representative traversal and combat. Record build flags, DMA mode and camera/input tape. Profile a separate instrumented build to diagnose costs and quantify its overhead; missing guest counters remain unknown.",
        "sealing": {"sealed": sealed, "leak_path": sealing.path},
        "sky_modes": sky_modes, "cook_mode": format!("{:?}",project.bsp_cook_mode),
        "staged_geometry": true,
        "world": {"total_faces":report.world_face_count,"non_solid_leaves":report.non_solid_leaf_count,
            "above_authored_geometry_budget_leaves":over,"unreadable_pvs_leaves":report.unreadable_pvs_leaf_count,
            "status": if !sealed {"invalid_sealing"} else if report.unreadable_pvs_leaf_count>0 {"invalid_pvs"} else {"runtime_measurement_required"}},
        "heaviest_leaves":report.heaviest_leaves(8).iter().map(|l|cost(l,None,budget)).collect::<Vec<_>>(),
        "areas":area_reports,"cook_warnings":validation.warnings,
        "next_action":"Measure representative views in normal Play. Do not shrink the level or remove actors solely because PVS counts exceed the withdrawn reference fixture."
    })).map_err(|e|e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn candidate_counts_do_not_invent_a_frame_rate_or_actor_limit() {
        assert_eq!(
            status(421, 792, Some(2), None),
            "runtime_measurement_required"
        );
        assert_eq!(status(40, 80, None, None), "runtime_measurement_required");
    }
    #[test]
    fn explicit_authoring_budgets_remain_distinct_from_runtime_acceptance() {
        let b = DesignBudget {
            pvs_faces: 400,
            base_world_triangles: 800,
            enemies: Some(2),
        };
        assert_eq!(status(421, 792, Some(2), Some(&b)), "over_authored_budget");
        assert_eq!(
            status(400, 800, Some(2), Some(&b)),
            "within_authored_budget_runtime_unverified"
        );
        assert_eq!(status(100, 200, None, Some(&b)), "actor_load_unspecified");
        assert_eq!(status(100, 200, Some(3), Some(&b)), "over_authored_budget");
    }
    #[test]
    fn empty_area_is_not_a_pass() {
        assert!(area_budget(
            &ProjectDocument::default(),
            Path::new("."),
            &[AreaSamples {
                name: "court".into(),
                samples: vec![],
                enemies: None
            }],
            None
        )
        .is_err());
    }
}
