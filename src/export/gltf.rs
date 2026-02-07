//! glTF/GLB export.

use crate::error::{MesherError, Result};
use crate::mesher::geometry::Mesh;
use crate::mesher::MesherOutput;
use gltf_json as json;
use json::validation::Checked::Valid;
use json::validation::USize64;
use std::mem;

/// Export a mesh to GLB format (binary glTF) with embedded texture.
/// Separates opaque and transparent geometry into different primitives for correct rendering.
/// Greedy-merged materials get their own textures with REPEAT wrapping for proper tiling.
pub fn export_glb(output: &MesherOutput) -> Result<Vec<u8>> {
    let opaque_mesh = &output.opaque_mesh;
    let transparent_mesh = &output.transparent_mesh;
    let atlas = &output.atlas;
    // Check if all meshes are empty
    let has_greedy = output.greedy_materials.iter().any(|gm| {
        !gm.opaque_mesh.is_empty() || !gm.transparent_mesh.is_empty()
    });
    if opaque_mesh.is_empty() && transparent_mesh.is_empty() && !has_greedy {
        return Err(MesherError::Export("Cannot export empty mesh".to_string()));
    }

    // Get texture PNG data
    let texture_png = atlas.to_png()?;

    // Build the binary buffer incrementally
    let mut buffer_data: Vec<u8> = Vec::new();

    // Helper: append mesh data, return (pos_offset, norm_offset, uv_offset, color_offset, idx_offset)
    fn append_mesh_data(buffer: &mut Vec<u8>, mesh: &Mesh) -> (usize, usize, usize, usize, usize) {
        let positions = mesh.positions_flat();
        let normals = mesh.normals_flat();
        let uvs = mesh.uvs_flat();
        let colors = mesh.colors_flat();

        let pos_offset = buffer.len();
        buffer.extend_from_slice(bytemuck_cast_slice(&positions));
        let norm_offset = buffer.len();
        buffer.extend_from_slice(bytemuck_cast_slice(&normals));
        let uv_offset = buffer.len();
        buffer.extend_from_slice(bytemuck_cast_slice(&uvs));
        let color_offset = buffer.len();
        buffer.extend_from_slice(bytemuck_cast_slice(&colors));
        let idx_offset = buffer.len();
        buffer.extend_from_slice(bytemuck_cast_slice(&mesh.indices));

        (pos_offset, norm_offset, uv_offset, color_offset, idx_offset)
    }

    // Track mesh data offsets
    struct MeshOffsets {
        pos_offset: usize,
        pos_bytes: usize,
        norm_offset: usize,
        norm_bytes: usize,
        uv_offset: usize,
        uv_bytes: usize,
        color_offset: usize,
        color_bytes: usize,
        idx_offset: usize,
        idx_bytes: usize,
        vertex_count: usize,
        index_count: usize,
    }

    fn write_mesh(buffer: &mut Vec<u8>, mesh: &Mesh) -> Option<MeshOffsets> {
        if mesh.is_empty() {
            return None;
        }
        let (pos_offset, norm_offset, uv_offset, color_offset, idx_offset) =
            append_mesh_data(buffer, mesh);
        let end = buffer.len();
        Some(MeshOffsets {
            pos_offset,
            pos_bytes: norm_offset - pos_offset,
            norm_offset,
            norm_bytes: uv_offset - norm_offset,
            uv_offset,
            uv_bytes: color_offset - uv_offset,
            color_offset,
            color_bytes: idx_offset - color_offset,
            idx_offset,
            idx_bytes: end - idx_offset,
            vertex_count: mesh.vertex_count(),
            index_count: mesh.indices.len(),
        })
    }

    let opaque_offsets = write_mesh(&mut buffer_data, opaque_mesh);
    let transparent_offsets = write_mesh(&mut buffer_data, transparent_mesh);

    // Write greedy material mesh data
    let mut greedy_mesh_offsets: Vec<(Option<MeshOffsets>, Option<MeshOffsets>)> = Vec::new();
    for gm in &output.greedy_materials {
        let opaque = write_mesh(&mut buffer_data, &gm.opaque_mesh);
        let transparent = write_mesh(&mut buffer_data, &gm.transparent_mesh);
        greedy_mesh_offsets.push((opaque, transparent));
    }

    // Append atlas texture PNG (aligned to 4 bytes)
    let texture_padding = (4 - (buffer_data.len() % 4)) % 4;
    buffer_data.extend(std::iter::repeat(0u8).take(texture_padding));
    let atlas_texture_offset = buffer_data.len();
    buffer_data.extend_from_slice(&texture_png);

    // Append greedy texture PNGs (aligned to 4 bytes)
    let mut greedy_texture_offsets: Vec<(usize, usize)> = Vec::new();
    for gm in &output.greedy_materials {
        if gm.texture_png.is_empty() {
            greedy_texture_offsets.push((0, 0));
            continue;
        }
        let padding = (4 - (buffer_data.len() % 4)) % 4;
        buffer_data.extend(std::iter::repeat(0u8).take(padding));
        let offset = buffer_data.len();
        buffer_data.extend_from_slice(&gm.texture_png);
        greedy_texture_offsets.push((offset, gm.texture_png.len()));
    }

    let total_buffer_size = buffer_data.len();

    // Calculate combined bounding box (include greedy meshes)
    let (min, max) = calculate_bounds_all(output);

    // Build glTF arrays
    let mut accessors = Vec::new();
    let mut buffer_views = Vec::new();
    let mut primitives = Vec::new();
    let mut images = Vec::new();
    let mut textures = Vec::new();
    let mut materials = Vec::new();

    let mut buffer_view_idx = 0u32;

    // Helper: add buffer views, accessors, and primitive for a mesh
    fn add_mesh_primitive(
        offsets: &MeshOffsets,
        material_idx: u32,
        bounds_min: [f32; 3],
        bounds_max: [f32; 3],
        buffer_views: &mut Vec<json::buffer::View>,
        accessors: &mut Vec<json::Accessor>,
        primitives: &mut Vec<json::mesh::Primitive>,
        buffer_view_idx: &mut u32,
    ) {
        let accessor_start = accessors.len() as u32;

        // 5 buffer views: positions, normals, uvs, colors, indices
        buffer_views.push(create_buffer_view(offsets.pos_offset, offsets.pos_bytes, Some(json::buffer::Target::ArrayBuffer)));
        let pos_view = *buffer_view_idx; *buffer_view_idx += 1;

        buffer_views.push(create_buffer_view(offsets.norm_offset, offsets.norm_bytes, Some(json::buffer::Target::ArrayBuffer)));
        let norm_view = *buffer_view_idx; *buffer_view_idx += 1;

        buffer_views.push(create_buffer_view(offsets.uv_offset, offsets.uv_bytes, Some(json::buffer::Target::ArrayBuffer)));
        let uv_view = *buffer_view_idx; *buffer_view_idx += 1;

        buffer_views.push(create_buffer_view(offsets.color_offset, offsets.color_bytes, Some(json::buffer::Target::ArrayBuffer)));
        let color_view = *buffer_view_idx; *buffer_view_idx += 1;

        buffer_views.push(create_buffer_view(offsets.idx_offset, offsets.idx_bytes, Some(json::buffer::Target::ElementArrayBuffer)));
        let idx_view = *buffer_view_idx; *buffer_view_idx += 1;

        // 5 accessors
        accessors.push(create_accessor(pos_view, offsets.vertex_count, json::accessor::Type::Vec3, json::accessor::ComponentType::F32, Some(bounds_min), Some(bounds_max)));
        accessors.push(create_accessor(norm_view, offsets.vertex_count, json::accessor::Type::Vec3, json::accessor::ComponentType::F32, None, None));
        accessors.push(create_accessor(uv_view, offsets.vertex_count, json::accessor::Type::Vec2, json::accessor::ComponentType::F32, None, None));
        accessors.push(create_accessor(color_view, offsets.vertex_count, json::accessor::Type::Vec4, json::accessor::ComponentType::F32, None, None));
        accessors.push(create_accessor(idx_view, offsets.index_count, json::accessor::Type::Scalar, json::accessor::ComponentType::U32, None, None));

        primitives.push(create_primitive(accessor_start, accessor_start + 4, material_idx));
    }

    // Material 0: Atlas opaque
    materials.push(create_material_with_texture(json::material::AlphaMode::Opaque, 0));
    // Material 1: Atlas transparent
    materials.push(create_material_with_texture(json::material::AlphaMode::Blend, 0));

    // Atlas texture image and glTF texture
    buffer_views.push(json::buffer::View {
        buffer: json::Index::new(0),
        byte_length: USize64(texture_png.len() as u64),
        byte_offset: Some(USize64(atlas_texture_offset as u64)),
        byte_stride: None,
        extensions: Default::default(),
        extras: Default::default(),
        target: None,
    });
    let atlas_image_view = buffer_view_idx;
    buffer_view_idx += 1;

    images.push(json::Image {
        buffer_view: Some(json::Index::new(atlas_image_view)),
        mime_type: Some(json::image::MimeType("image/png".to_string())),
        uri: None,
        extensions: Default::default(),
        extras: Default::default(),
    });
    textures.push(json::Texture {
        sampler: Some(json::Index::new(0)),
        source: json::Index::new(0),
        extensions: Default::default(),
        extras: Default::default(),
    });

    // Add atlas-based primitives
    if let Some(ref offsets) = opaque_offsets {
        add_mesh_primitive(offsets, 0, min, max, &mut buffer_views, &mut accessors, &mut primitives, &mut buffer_view_idx);
    }
    if let Some(ref offsets) = transparent_offsets {
        add_mesh_primitive(offsets, 1, min, max, &mut buffer_views, &mut accessors, &mut primitives, &mut buffer_view_idx);
    }

    // Add greedy material images, textures, materials, and primitives
    for (i, gm) in output.greedy_materials.iter().enumerate() {
        let (tex_offset, tex_len) = greedy_texture_offsets[i];
        if tex_len == 0 {
            continue;
        }

        // Add image buffer view
        buffer_views.push(json::buffer::View {
            buffer: json::Index::new(0),
            byte_length: USize64(tex_len as u64),
            byte_offset: Some(USize64(tex_offset as u64)),
            byte_stride: None,
            extensions: Default::default(),
            extras: Default::default(),
            target: None,
        });
        let img_view = buffer_view_idx;
        buffer_view_idx += 1;

        let image_idx = images.len() as u32;
        images.push(json::Image {
            buffer_view: Some(json::Index::new(img_view)),
            mime_type: Some(json::image::MimeType("image/png".to_string())),
            uri: None,
            extensions: Default::default(),
            extras: Default::default(),
        });

        let texture_idx = textures.len() as u32;
        textures.push(json::Texture {
            sampler: Some(json::Index::new(0)), // Reuse sampler 0 (REPEAT)
            source: json::Index::new(image_idx),
            extensions: Default::default(),
            extras: Default::default(),
        });

        // Opaque material for this greedy texture
        let opaque_mat_idx = materials.len() as u32;
        materials.push(create_material_with_texture(json::material::AlphaMode::Opaque, texture_idx));

        // Transparent material for this greedy texture
        let transparent_mat_idx = materials.len() as u32;
        materials.push(create_material_with_texture(json::material::AlphaMode::Blend, texture_idx));

        let (ref opaque_off, ref transparent_off) = greedy_mesh_offsets[i];

        if let Some(ref offsets) = opaque_off {
            add_mesh_primitive(offsets, opaque_mat_idx, min, max, &mut buffer_views, &mut accessors, &mut primitives, &mut buffer_view_idx);
        }
        if let Some(ref offsets) = transparent_off {
            add_mesh_primitive(offsets, transparent_mat_idx, min, max, &mut buffer_views, &mut accessors, &mut primitives, &mut buffer_view_idx);
        }
    }

    // Build glTF JSON
    let root = json::Root {
        accessors,
        buffers: vec![json::Buffer {
            byte_length: USize64(total_buffer_size as u64),
            extensions: Default::default(),
            extras: Default::default(),
            uri: None,
        }],
        buffer_views,
        images,
        samplers: vec![json::texture::Sampler {
            mag_filter: Some(Valid(json::texture::MagFilter::Nearest)),
            min_filter: Some(Valid(json::texture::MinFilter::Nearest)),
            wrap_s: Valid(json::texture::WrappingMode::Repeat),
            wrap_t: Valid(json::texture::WrappingMode::Repeat),
            extensions: Default::default(),
            extras: Default::default(),
        }],
        textures,
        materials,
        meshes: vec![json::Mesh {
            extensions: Default::default(),
            extras: Default::default(),
            primitives,
            weights: None,
        }],
        nodes: vec![json::Node {
            camera: None,
            children: None,
            extensions: Default::default(),
            extras: Default::default(),
            matrix: None,
            mesh: Some(json::Index::new(0)),
            rotation: None,
            scale: None,
            translation: None,
            skin: None,
            weights: None,
        }],
        scenes: vec![json::Scene {
            extensions: Default::default(),
            extras: Default::default(),
            nodes: vec![json::Index::new(0)],
        }],
        scene: Some(json::Index::new(0)),
        ..Default::default()
    };

    // Serialize JSON
    let json_string = json::serialize::to_string(&root)
        .map_err(|e| MesherError::Export(format!("Failed to serialize glTF JSON: {}", e)))?;
    let json_bytes = json_string.as_bytes();

    // Pad JSON to 4-byte alignment
    let json_padding = (4 - (json_bytes.len() % 4)) % 4;
    let padded_json_len = json_bytes.len() + json_padding;

    // Pad buffer to 4-byte alignment
    let buffer_padding = (4 - (buffer_data.len() % 4)) % 4;
    let padded_buffer_len = buffer_data.len() + buffer_padding;

    // Calculate total size
    let total_size = 12 + // GLB header
        8 + padded_json_len + // JSON chunk
        8 + padded_buffer_len; // BIN chunk

    let mut glb = Vec::with_capacity(total_size);

    // GLB Header
    glb.extend_from_slice(b"glTF"); // magic
    glb.extend_from_slice(&2u32.to_le_bytes()); // version
    glb.extend_from_slice(&(total_size as u32).to_le_bytes()); // length

    // JSON Chunk
    glb.extend_from_slice(&(padded_json_len as u32).to_le_bytes()); // chunk length
    glb.extend_from_slice(&0x4E4F534Au32.to_le_bytes()); // chunk type: JSON
    glb.extend_from_slice(json_bytes);
    glb.extend_from_slice(&vec![0x20u8; json_padding]); // padding (spaces)

    // BIN Chunk
    glb.extend_from_slice(&(padded_buffer_len as u32).to_le_bytes()); // chunk length
    glb.extend_from_slice(&0x004E4942u32.to_le_bytes()); // chunk type: BIN
    glb.extend_from_slice(&buffer_data);
    glb.extend_from_slice(&vec![0u8; buffer_padding]); // padding (zeros)

    Ok(glb)
}

/// Calculate bounding box from all meshes in the output.
fn calculate_bounds_all(output: &MesherOutput) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];

    let all_vertices = output.opaque_mesh.vertices.iter()
        .chain(output.transparent_mesh.vertices.iter())
        .chain(output.greedy_materials.iter().flat_map(|gm| {
            gm.opaque_mesh.vertices.iter().chain(gm.transparent_mesh.vertices.iter())
        }));

    for vertex in all_vertices {
        for i in 0..3 {
            min[i] = min[i].min(vertex.position[i]);
            max[i] = max[i].max(vertex.position[i]);
        }
    }

    // Handle empty case
    if min[0] == f32::MAX {
        min = [0.0; 3];
        max = [0.0; 3];
    }

    (min, max)
}

/// Create a buffer view.
fn create_buffer_view(
    offset: usize,
    size: usize,
    target: Option<json::buffer::Target>,
) -> json::buffer::View {
    json::buffer::View {
        buffer: json::Index::new(0),
        byte_length: USize64(size as u64),
        byte_offset: Some(USize64(offset as u64)),
        byte_stride: None,
        extensions: Default::default(),
        extras: Default::default(),
        target: target.map(Valid),
    }
}

/// Create an accessor.
fn create_accessor(
    buffer_view: u32,
    count: usize,
    type_: json::accessor::Type,
    component_type: json::accessor::ComponentType,
    min: Option<[f32; 3]>,
    max: Option<[f32; 3]>,
) -> json::Accessor {
    json::Accessor {
        buffer_view: Some(json::Index::new(buffer_view)),
        byte_offset: Some(USize64(0)),
        count: USize64(count as u64),
        component_type: Valid(json::accessor::GenericComponentType(component_type)),
        extensions: Default::default(),
        extras: Default::default(),
        type_: Valid(type_),
        min: min.map(|m| json::Value::from(m.to_vec())),
        max: max.map(|m| json::Value::from(m.to_vec())),
        normalized: false,
        sparse: None,
    }
}

/// Create a primitive.
fn create_primitive(
    positions_accessor: u32,
    indices_accessor: u32,
    material: u32,
) -> json::mesh::Primitive {
    let mut attributes = std::collections::BTreeMap::new();
    attributes.insert(
        Valid(json::mesh::Semantic::Positions),
        json::Index::new(positions_accessor),
    );
    attributes.insert(
        Valid(json::mesh::Semantic::Normals),
        json::Index::new(positions_accessor + 1),
    );
    attributes.insert(
        Valid(json::mesh::Semantic::TexCoords(0)),
        json::Index::new(positions_accessor + 2),
    );
    attributes.insert(
        Valid(json::mesh::Semantic::Colors(0)),
        json::Index::new(positions_accessor + 3),
    );

    json::mesh::Primitive {
        attributes,
        extensions: Default::default(),
        extras: Default::default(),
        indices: Some(json::Index::new(indices_accessor)),
        material: Some(json::Index::new(material)),
        mode: Valid(json::mesh::Mode::Triangles),
        targets: None,
    }
}

/// Create a material with the specified alpha mode and texture index.
fn create_material_with_texture(alpha_mode: json::material::AlphaMode, texture_idx: u32) -> json::Material {
    json::Material {
        pbr_metallic_roughness: json::material::PbrMetallicRoughness {
            base_color_texture: Some(json::texture::Info {
                index: json::Index::new(texture_idx),
                tex_coord: 0,
                extensions: Default::default(),
                extras: Default::default(),
            }),
            base_color_factor: json::material::PbrBaseColorFactor([1.0, 1.0, 1.0, 1.0]),
            metallic_factor: json::material::StrengthFactor(0.0),
            roughness_factor: json::material::StrengthFactor(1.0),
            metallic_roughness_texture: None,
            extensions: Default::default(),
            extras: Default::default(),
        },
        alpha_mode: Valid(alpha_mode),
        alpha_cutoff: None,
        double_sided: true,
        normal_texture: None,
        occlusion_texture: None,
        emissive_texture: None,
        emissive_factor: json::material::EmissiveFactor([0.0, 0.0, 0.0]),
        extensions: Default::default(),
        extras: Default::default(),
    }
}

/// Cast a slice of T to a slice of bytes.
fn bytemuck_cast_slice<T: Copy>(slice: &[T]) -> &[u8] {
    let ptr = slice.as_ptr() as *const u8;
    let len = slice.len() * mem::size_of::<T>();
    unsafe { std::slice::from_raw_parts(ptr, len) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atlas::TextureAtlas;
    use crate::mesher::geometry::{Mesh, Vertex};
    use crate::types::BoundingBox;

    #[test]
    fn test_export_simple_mesh() {
        let mut mesh = Mesh::new();

        // Create a simple triangle
        let v0 = mesh.add_vertex(Vertex::new([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0]));
        let v1 = mesh.add_vertex(Vertex::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0]));
        let v2 = mesh.add_vertex(Vertex::new([0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [0.0, 1.0]));
        mesh.add_triangle(v0, v1, v2);

        let output = MesherOutput {
            opaque_mesh: mesh,
            transparent_mesh: Mesh::new(),
            atlas: TextureAtlas::empty(),
            bounds: BoundingBox::new([0.0, 0.0, 0.0], [1.0, 0.0, 1.0]),
            greedy_materials: Vec::new(),
        };

        let glb = export_glb(&output).unwrap();

        // Check GLB header
        assert_eq!(&glb[0..4], b"glTF");
        assert_eq!(u32::from_le_bytes([glb[4], glb[5], glb[6], glb[7]]), 2); // version
    }

    #[test]
    fn test_export_empty_mesh_fails() {
        let output = MesherOutput {
            opaque_mesh: Mesh::new(),
            transparent_mesh: Mesh::new(),
            atlas: TextureAtlas::empty(),
            bounds: BoundingBox::new([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
            greedy_materials: Vec::new(),
        };

        let result = export_glb(&output);
        assert!(result.is_err());
    }

    #[test]
    fn test_export_transparent_only() {
        let mut mesh = Mesh::new();

        let v0 = mesh.add_vertex(Vertex::new([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0]));
        let v1 = mesh.add_vertex(Vertex::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0]));
        let v2 = mesh.add_vertex(Vertex::new([0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [0.0, 1.0]));
        mesh.add_triangle(v0, v1, v2);

        let output = MesherOutput {
            opaque_mesh: Mesh::new(),
            transparent_mesh: mesh,
            atlas: TextureAtlas::empty(),
            bounds: BoundingBox::new([0.0, 0.0, 0.0], [1.0, 0.0, 1.0]),
            greedy_materials: Vec::new(),
        };

        let glb = export_glb(&output).unwrap();
        assert_eq!(&glb[0..4], b"glTF");
    }
}
