//! Box, Cylinder and Image props in the 3D preview, drawn from the surfaces
//! the cook generates (`psxed_project::prop_surfaces`) with the runtime's
//! visibility rules: Box and Cylinder faces draw when they face the camera,
//! Image cards draw both sides, and a face without a material is skipped
//! like the cook skips it.

use super::*;
use psxed_project::prop_surfaces::{
    bake_prop_uv, box_prop_world_surfaces, cylinder_prop_world_surfaces, image_prop_world_quad,
    prop_uv_corners, PropSurface, PROP_FACE_UV_Q8,
};
use psxed_project::GridUvTransform;

pub(super) fn walk_props(
    project: &ProjectDocument,
    textures: &EditorTextures,
    camera: psx_engine::WorldCamera,
    hidden_scene_nodes: &HashSet<NodeId>,
    scratch: &mut PreviewScratch,
) {
    let scene = project.active_scene();
    for node in scene.nodes() {
        if scratch.geometry_full() {
            break;
        }
        if scene_node_hidden(scene, hidden_scene_nodes, node.id) {
            continue;
        }
        let pos = node.transform.translation.map(|value| value.round() as i32);
        let [pitch, yaw, roll] = node
            .transform
            .rotation_degrees
            .map(|degrees| psxed_project::spatial::euler_degrees_to_q12(degrees) as i16);
        match &node.kind {
            NodeKind::BoxProp {
                materials,
                uvs,
                vertices,
                erosion,
                ..
            } => {
                for surface in box_prop_world_surfaces(pos, pitch, yaw, roll, *vertices, *erosion) {
                    let slot = usize::from(surface.slot);
                    emit_prop_surface(
                        project,
                        textures,
                        camera,
                        scratch,
                        &surface,
                        materials.get(slot).copied().flatten(),
                        uvs.get(slot).copied().unwrap_or_default(),
                        true,
                    );
                }
            }
            NodeKind::CylinderProp {
                materials,
                uvs,
                geometry,
                ..
            } => {
                for surface in cylinder_prop_world_surfaces(pos, pitch, yaw, roll, *geometry) {
                    let slot = usize::from(surface.slot);
                    emit_prop_surface(
                        project,
                        textures,
                        camera,
                        scratch,
                        &surface,
                        materials.get(slot).copied().flatten(),
                        uvs.get(slot).copied().unwrap_or_default(),
                        true,
                    );
                }
            }
            NodeKind::ImageProp {
                material,
                width,
                height,
                cylindrical_billboard,
                ..
            } => {
                let billboard =
                    cylindrical_billboard.then(|| (camera.sin_yaw.raw(), camera.cos_yaw.raw()));
                let surface = PropSurface {
                    vertices: image_prop_world_quad(
                        pos, *width, *height, pitch, yaw, roll, billboard,
                    ),
                    vertex_count: 4,
                    uv_q8: PROP_FACE_UV_Q8,
                    slot: 0,
                };
                // The card maps its whole texture; it has no UV transform.
                emit_prop_surface(
                    project,
                    textures,
                    camera,
                    scratch,
                    &surface,
                    *material,
                    GridUvTransform::default(),
                    false,
                );
            }
            _ => {}
        }
    }
}

/// True when the surface's front (the side its winding faces) looks at the
/// camera, the runtime's per-face test for Box and Cylinder props.
fn prop_surface_faces_camera(camera: psx_engine::WorldCamera, surface: &PropSurface) -> bool {
    let v = surface.vertices.map(|vertex| vertex.map(i64::from));
    let ab = [v[1][0] - v[0][0], v[1][1] - v[0][1], v[1][2] - v[0][2]];
    let ac = [v[2][0] - v[0][0], v[2][1] - v[0][1], v[2][2] - v[0][2]];
    let normal = [
        ab[1] * ac[2] - ab[2] * ac[1],
        ab[2] * ac[0] - ab[0] * ac[2],
        ab[0] * ac[1] - ab[1] * ac[0],
    ];
    let count = i64::from(surface.vertex_count.clamp(3, 4));
    let used = &v[..count as usize];
    let center: [i64; 3] =
        std::array::from_fn(|axis| used.iter().map(|vertex| vertex[axis]).sum::<i64>() / count);
    let to_camera = [
        i64::from(camera.position.x) - center[0],
        i64::from(camera.position.y) - center[1],
        i64::from(camera.position.z) - center[2],
    ];
    normal[0] * to_camera[0] + normal[1] * to_camera[1] + normal[2] * to_camera[2] > 0
}

#[allow(clippy::too_many_arguments)]
fn emit_prop_surface(
    project: &ProjectDocument,
    textures: &EditorTextures,
    camera: psx_engine::WorldCamera,
    scratch: &mut PreviewScratch,
    surface: &PropSurface,
    material: Option<ResourceId>,
    uv: GridUvTransform,
    front_facing_only: bool,
) {
    let Some(material) = material else {
        return;
    };
    if front_facing_only && !prop_surface_faces_camera(camera, surface) {
        return;
    }
    // Culling is decided above (or not at all for image cards), so the
    // triangles themselves draw from either side.
    let shade = face_shade(project, Some(material), (0x80, 0x80, 0x80), textures)
        .with_sidedness(psxed_project::MaterialFaceSidedness::Both);
    let count = usize::from(surface.vertex_count.clamp(3, 4));
    let corners = match shade {
        FaceShade::Textured { slot, .. } => prop_uv_corners(
            uv,
            u16::from(slot.texture_width),
            u16::from(slot.texture_height),
        ),
        FaceShade::Flat { .. } => [(0, 0); 4],
    };
    let mut verts = [[0.0; 3]; 4];
    let mut uvs = [[0.0; 2]; 4];
    for index in 0..count {
        verts[index] = surface.vertices[index].map(f64::from);
        let [u, v] = bake_prop_uv(corners, surface.uv_q8[index]);
        uvs[index] = [f64::from(u), f64::from(v)];
    }
    emit_uv_polygon(scratch, camera, shade, &verts[..count], &uvs[..count], None);
}
