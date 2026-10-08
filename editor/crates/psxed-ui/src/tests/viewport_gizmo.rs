use super::*;

/// Diagnostic (not a strict assertion): print an ASCII map of what a
/// click resolves to across the gizmo region, at several zoom levels.
/// Run with `cargo test gizmo_pick_map -- --nocapture` to eyeball
/// where the tile (`#`) leaks in among the handles.
#[test]
fn gizmo_pick_map_diagnostic() {
    // Big room so the floor fills the view behind the gizmo at every
    // zoom -- gaps in the handle pick then show as '#' (tile), the way
    // they do in a real level, not '.' (ray missed the small floor).
    let mut harness = ViewportHarness::floored("gizmo-pick-map", 24);
    let light = harness.add_centre_light(0.25);
    harness.select(light);
    let target = harness.world_center();

    // Sweep camera elevation (degrees above the horizon) at a fixed
    // moderate distance: the flat XZ ground plane foreshortens as the
    // angle gets shallow, the regime the user hits when orbiting to a
    // near-horizontal view.
    println!("##### ELEVATION SWEEP (dist 12000) #####");
    for &deg in &[60.0_f32, 40.0, 25.0, 15.0, 8.0, 4.0] {
        harness.frame_at_elevation(target, 12_000.0, deg.to_radians());
        let xz_area = harness
            .workspace
            .node_gizmo_screen_planes(harness.viewport)
            .into_iter()
            .find(|p| p.plane == NodeGizmoPlane::XZ)
            .map(|p| polygon_area_2d(&p.corners).abs());
        // Where the XZ plane centre projects, whether or not it is
        // culled (a point lerped along +X/+Z from the pivot).
        let pivot = harness
            .workspace
            .node_gizmo_bounds_3d(&[harness.workspace.selection.selected_node])
            .map(|(p, _)| p)
            .unwrap_or([0.0, 0.0, 0.0]);
        let probe_world = [pivot[0] + 300.0, pivot[1], pivot[2] + 300.0];
        let at_center = project_world_to_viewport_screen(
            harness.workspace.viewport_3d_camera(),
            harness.viewport,
            probe_world,
        )
        .map(|p| harness.classify(p));
        println!(
            "elev {deg:>4}deg: XZ area = {:>8}  click-at-XZ-region = {:?}",
            xz_area
                .map(|a| format!("{a:.1}"))
                .unwrap_or_else(|| "CULLED".to_string()),
            at_center,
        );
    }

    for &distance in &[4_000.0, 8_000.0, 16_000.0, 32_000.0, 60_000.0] {
        harness.frame(target, distance);
        // Bounding box over all axis + plane handles, with margin.
        let mut pts: Vec<Pos2> = harness
            .workspace
            .node_gizmo_screen_axes(harness.viewport)
            .into_iter()
            .flat_map(|a| [a.start, a.end])
            .collect();
        for plane in NodeGizmoPlane::ALL {
            if let Some(q) = harness.plane_quad(plane) {
                pts.extend(q);
            }
        }
        if pts.is_empty() {
            println!("dist {distance}: no gizmo projected");
            continue;
        }
        let pad = 16.0;
        let min_x = pts.iter().map(|p| p.x).fold(f32::INFINITY, f32::min) - pad;
        let max_x = pts.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max) + pad;
        let min_y = pts.iter().map(|p| p.y).fold(f32::INFINITY, f32::min) - pad;
        let max_y = pts.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max) + pad;
        let cols = 70usize;
        let rows = 30usize;
        println!("=== dist {distance} | x[{min_x:.0}..{max_x:.0}] y[{min_y:.0}..{max_y:.0}] ===",);
        for r in 0..rows {
            let mut line = String::with_capacity(cols);
            for c in 0..cols {
                let probe = Pos2::new(
                    min_x + (max_x - min_x) * c as f32 / (cols - 1) as f32,
                    min_y + (max_y - min_y) * r as f32 / (rows - 1) as f32,
                );
                line.push(harness.classify(probe));
            }
            println!("{line}");
        }
    }
}

#[test]
fn node_gizmo_no_tile_inside_plane_footprint() {
    // Regression for "clicking near the plane grabs the tile when
    // zoomed out". A move plane is drawn as a small inset square in
    // the corner between its two axes, but the user reads the whole
    // corner as the handle. The pick covered only the small square (+
    // a few px) and the axis tubes, leaving wedges of bare tile
    // between them. This sweeps each plane's footprint triangle
    // (pivot + the two axis endpoints) and requires every interior
    // point to resolve to a gizmo handle, never a floor Surface.
    // Big room so the floor is always behind the gizmo.
    let mut harness = ViewportHarness::floored("gizmo-footprint", 24);
    let light = harness.add_centre_light(0.25);
    harness.select(light);
    let target = harness.world_center();

    let mut checked = 0;
    for &distance in &[6_000.0, 10_000.0, 16_000.0, 24_000.0] {
        harness.frame(target, distance);
        for plane in NodeGizmoPlane::ALL {
            let Some(tri) = harness.plane_footprint_triangle(plane) else {
                continue;
            };
            // Barycentric sweep of the triangle interior.
            let steps = 12;
            for i in 0..=steps {
                for j in 0..=(steps - i) {
                    let a = i as f32 / steps as f32;
                    let b = j as f32 / steps as f32;
                    let c = 1.0 - a - b;
                    // Stay strictly interior so we test the wedge, not
                    // the axis edges themselves.
                    if a < 0.08 || b < 0.08 || c < 0.08 {
                        continue;
                    }
                    let probe = Pos2::new(
                        tri[0].x * c + tri[1].x * a + tri[2].x * b,
                        tri[0].y * c + tri[1].y * a + tri[2].y * b,
                    );
                    checked += 1;
                    let resolved = harness.resolve(probe);
                    assert!(
                        !matches!(resolved, Some(Viewport3dPointerTarget::Brush { .. })),
                        "dist {distance}, {plane:?} footprint: click at {probe:?} \
                             grabbed the tile ({resolved:?}) instead of a gizmo handle",
                    );
                }
            }
        }
    }
    assert!(
        checked >= 50,
        "footprint sweep covered too few points ({checked})"
    );
}

#[test]
fn node_gizmo_xz_plane_grabbable_across_zoom_out() {
    let mut harness = ViewportHarness::floored("gizmo-zoom-grab", 4);
    let light = harness.add_centre_light(0.25);
    harness.select(light);
    let target = harness.world_center();

    // Regression: the green XZ ground plane must stay grabbable as the
    // camera pulls back. The bug was a click inside the plane quad
    // grabbing an axis (which, on a foreshortened quad, read as the
    // floor tile behind it) because axes were picked first and
    // short-circuited the plane test.
    //
    // The three move-plane quads overlap on screen once foreshortened,
    // so in the overlap zone which plane wins is genuinely ambiguous
    // and not worth asserting. We instead sample a dense grid over the
    // XZ quad's bounding box, keep only points that are inside XZ and
    // outside XY and YZ -- the region unambiguously "on the XZ handle"
    // -- and require every one of those to grab XZ. The centre is
    // always required to be such a point and to grab XZ.
    for &distance in &[3_000.0, 6_000.0, 12_000.0, 20_000.0] {
        harness.frame(target, distance);
        let xz = harness.plane_quad(NodeGizmoPlane::XZ).unwrap_or_else(|| {
            panic!("XZ plane should still project when framed from {distance} units")
        });
        let others: Vec<[Pos2; 4]> = [NodeGizmoPlane::XY, NodeGizmoPlane::YZ]
            .into_iter()
            .filter_map(|plane| harness.plane_quad(plane))
            .collect();
        let exclusive_to_xz = |p: Pos2| {
            point_in_polygon_2d(p, &xz) && others.iter().all(|quad| !point_in_polygon_2d(p, quad))
        };

        let center = {
            let sum = xz.iter().fold(Vec2::ZERO, |acc, c| acc + c.to_vec2());
            Pos2::new(sum.x / 4.0, sum.y / 4.0)
        };
        assert!(
            exclusive_to_xz(center),
            "framed from {distance} units, the XZ quad centre {center:?} is not \
                 exclusively on XZ -- the quads overlap their own centre, revisit the test",
        );

        let min_x = xz.iter().map(|c| c.x).fold(f32::INFINITY, f32::min);
        let max_x = xz.iter().map(|c| c.x).fold(f32::NEG_INFINITY, f32::max);
        let min_y = xz.iter().map(|c| c.y).fold(f32::INFINITY, f32::min);
        let max_y = xz.iter().map(|c| c.y).fold(f32::NEG_INFINITY, f32::max);
        let mut checked = 0;
        for ix in 0..=10 {
            for iy in 0..=10 {
                let probe = Pos2::new(
                    min_x + (max_x - min_x) * ix as f32 / 10.0,
                    min_y + (max_y - min_y) * iy as f32 / 10.0,
                );
                if !exclusive_to_xz(probe) {
                    continue;
                }
                checked += 1;
                assert_eq!(
                    harness.gizmo_handle_at(probe),
                    Some(NodeGizmoHandle::Plane(NodeGizmoPlane::XZ)),
                    "framed from {distance} units, a click at {probe:?} \
                         exclusively on the XZ quad did not grab the XZ plane",
                );
            }
        }
        assert!(
            checked >= 4,
            "framed from {distance} units, only {checked} probes were exclusively \
                 on XZ -- too few to be a meaningful regression check",
        );
    }
}

#[test]
fn node_gizmo_plane_click_never_falls_through_to_tile() {
    // The user-visible symptom was the click "grabbing the underlying
    // tile". That happened in the thin band *just outside* a
    // foreshortened plane quad: too far for strict polygon containment
    // (so the old plane test missed) and too far from any axis (so the
    // axis test missed too), leaving `pick_node_gizmo_handle` returning
    // None and the click falling through to the floor Surface.
    //
    // We reconstruct exactly that band: scan a grid around the XZ quad
    // and keep points that are outside every plane quad AND outside the
    // axis pick radius but within the plane pick tolerance of XZ. Under
    // the old code each such point resolved to a Surface (tile); with
    // the fix each must resolve to the XZ plane handle. The final
    // assert that the band was non-empty stops this silently becoming a
    // no-op if the projection ever changes.
    let mut harness = ViewportHarness::floored("gizmo-no-tile-fallthrough", 4);
    let light = harness.add_centre_light(0.25);
    harness.select(light);
    let target = harness.world_center();

    let mut total_band_points = 0;
    for &distance in &[6_000.0, 12_000.0, 20_000.0] {
        harness.frame(target, distance);
        let xz = harness
            .plane_quad(NodeGizmoPlane::XZ)
            .unwrap_or_else(|| panic!("XZ plane should project when framed from {distance} units"));
        let other_quads: Vec<[Pos2; 4]> = [NodeGizmoPlane::XY, NodeGizmoPlane::YZ]
            .into_iter()
            .filter_map(|plane| harness.plane_quad(plane))
            .collect();

        let min_x = xz.iter().map(|c| c.x).fold(f32::INFINITY, f32::min) - GIZMO_PLANE_PICK_RADIUS;
        let max_x =
            xz.iter().map(|c| c.x).fold(f32::NEG_INFINITY, f32::max) + GIZMO_PLANE_PICK_RADIUS;
        let min_y = xz.iter().map(|c| c.y).fold(f32::INFINITY, f32::min) - GIZMO_PLANE_PICK_RADIUS;
        let max_y =
            xz.iter().map(|c| c.y).fold(f32::NEG_INFINITY, f32::max) + GIZMO_PLANE_PICK_RADIUS;

        for ix in 0..=24 {
            for iy in 0..=24 {
                let probe = Pos2::new(
                    min_x + (max_x - min_x) * ix as f32 / 24.0,
                    min_y + (max_y - min_y) * iy as f32 / 24.0,
                );
                // The fallthrough band: outside every quad (strict),
                // beyond axis radius, but within plane tolerance of XZ.
                let outside_all_quads = !point_in_polygon_2d(probe, &xz)
                    && other_quads
                        .iter()
                        .all(|quad| !point_in_polygon_2d(probe, quad));
                let beyond_axes = harness.nearest_axis_distance(probe) > GIZMO_AXIS_PICK_RADIUS;
                let within_xz_tolerance =
                    distance_to_polygon_edges_2d(probe, &xz) <= GIZMO_PLANE_PICK_RADIUS;
                if !(outside_all_quads && beyond_axes && within_xz_tolerance) {
                    continue;
                }
                total_band_points += 1;
                // The symptom was grabbing the tile, so the invariant
                // is "not a Surface", not a specific plane: a band
                // point can sit within tolerance of XZ and a
                // neighbouring plane at once, and either gizmo handle is
                // a correct, non-fallthrough result.
                let resolved = harness.resolve(probe);
                assert!(
                    matches!(resolved, Some(Viewport3dPointerTarget::NodeGizmo(_))),
                    "framed from {distance} units, a click at {probe:?} in the XZ \
                         tolerance band fell through to {resolved:?} instead of a gizmo handle",
                );
            }
        }
    }
    assert!(
        total_band_points >= 3,
        "expected the XZ fallthrough band to contain probe points across zoom levels, \
             found {total_band_points} -- the test is no longer exercising the bug",
    );
}
