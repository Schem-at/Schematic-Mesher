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
pub fn export_glb(output: &MesherOutput) -> Result<Vec<u8>> {
    let opaque_mesh = &output.opaque_mesh;
    let transparent_mesh = &output.transparent_mesh;
    let atlas = &output.atlas;

    // Check if both meshes are empty
    if opaque_mesh.is_empty() && transparent_mesh.is_empty() {
        return Err(MesherError::Export("Cannot export empty mesh".to_string()));
    }

    // Get texture PNG data
    let texture_png = atlas.to_png()?;

    // Calculate buffer data for both meshes
    let opaque_positions = opaque_mesh.positions_flat();
    let opaque_normals = opaque_mesh.normals_flat();
    let opaque_uvs = opaque_mesh.uvs_flat();
    let opaque_colors = opaque_mesh.colors_flat();
    let opaque_indices = &opaque_mesh.indices;

    let transparent_positions = transparent_mesh.positions_flat();
    let transparent_normals = transparent_mesh.normals_flat();
    let transparent_uvs = transparent_mesh.uvs_flat();
    let transparent_colors = transparent_mesh.colors_flat();
    let transparent_indices = &transparent_mesh.indices;

    // Calculate byte sizes for opaque mesh
    let opaque_positions_bytes = opaque_positions.len() * mem::size_of::<f32>();
    let opaque_normals_bytes = opaque_normals.len() * mem::size_of::<f32>();
    let opaque_uvs_bytes = opaque_uvs.len() * mem::size_of::<f32>();
    let opaque_colors_bytes = opaque_colors.len() * mem::size_of::<f32>();
    let opaque_indices_bytes = opaque_indices.len() * mem::size_of::<u32>();

    // Calculate byte sizes for transparent mesh
    let transparent_positions_bytes = transparent_positions.len() * mem::size_of::<f32>();
    let transparent_normals_bytes = transparent_normals.len() * mem::size_of::<f32>();
    let transparent_uvs_bytes = transparent_uvs.len() * mem::size_of::<f32>();
    let transparent_colors_bytes = transparent_colors.len() * mem::size_of::<f32>();
    let transparent_indices_bytes = transparent_indices.len() * mem::size_of::<u32>();

    // Calculate buffer offsets
    // Opaque mesh data
    let opaque_positions_offset = 0;
    let opaque_normals_offset = opaque_positions_offset + opaque_positions_bytes;
    let opaque_uvs_offset = opaque_normals_offset + opaque_normals_bytes;
    let opaque_colors_offset = opaque_uvs_offset + opaque_uvs_bytes;
    let opaque_indices_offset = opaque_colors_offset + opaque_colors_bytes;

    // Transparent mesh data (after opaque)
    let transparent_positions_offset = opaque_indices_offset + opaque_indices_bytes;
    let transparent_normals_offset = transparent_positions_offset + transparent_positions_bytes;
    let transparent_uvs_offset = transparent_normals_offset + transparent_normals_bytes;
    let transparent_colors_offset = transparent_uvs_offset + transparent_uvs_bytes;
    let transparent_indices_offset = transparent_colors_offset + transparent_colors_bytes;

    // Texture (after all mesh data)
    let texture_offset = transparent_indices_offset + transparent_indices_bytes;
    let texture_padding_before = (4 - (texture_offset % 4)) % 4;
    let aligned_texture_offset = texture_offset + texture_padding_before;

    let total_buffer_size = aligned_texture_offset + texture_png.len();

    // Create the binary buffer
    let mut buffer_data = vec![0u8; total_buffer_size];

    // Copy opaque mesh data
    if !opaque_mesh.is_empty() {
        buffer_data[opaque_positions_offset..opaque_positions_offset + opaque_positions_bytes]
            .copy_from_slice(bytemuck_cast_slice(&opaque_positions));
        buffer_data[opaque_normals_offset..opaque_normals_offset + opaque_normals_bytes]
            .copy_from_slice(bytemuck_cast_slice(&opaque_normals));
        buffer_data[opaque_uvs_offset..opaque_uvs_offset + opaque_uvs_bytes]
            .copy_from_slice(bytemuck_cast_slice(&opaque_uvs));
        buffer_data[opaque_colors_offset..opaque_colors_offset + opaque_colors_bytes]
            .copy_from_slice(bytemuck_cast_slice(&opaque_colors));
        buffer_data[opaque_indices_offset..opaque_indices_offset + opaque_indices_bytes]
            .copy_from_slice(bytemuck_cast_slice(opaque_indices));
    }

    // Copy transparent mesh data
    if !transparent_mesh.is_empty() {
        buffer_data
            [transparent_positions_offset..transparent_positions_offset + transparent_positions_bytes]
            .copy_from_slice(bytemuck_cast_slice(&transparent_positions));
        buffer_data
            [transparent_normals_offset..transparent_normals_offset + transparent_normals_bytes]
            .copy_from_slice(bytemuck_cast_slice(&transparent_normals));
        buffer_data[transparent_uvs_offset..transparent_uvs_offset + transparent_uvs_bytes]
            .copy_from_slice(bytemuck_cast_slice(&transparent_uvs));
        buffer_data[transparent_colors_offset..transparent_colors_offset + transparent_colors_bytes]
            .copy_from_slice(bytemuck_cast_slice(&transparent_colors));
        buffer_data
            [transparent_indices_offset..transparent_indices_offset + transparent_indices_bytes]
            .copy_from_slice(bytemuck_cast_slice(transparent_indices));
    }

    // Copy texture data
    buffer_data[aligned_texture_offset..aligned_texture_offset + texture_png.len()]
        .copy_from_slice(&texture_png);

    // Calculate combined bounding box
    let (min, max) = calculate_bounds(opaque_mesh, transparent_mesh);

    // Build accessors, buffer views, and primitives
    let mut accessors = Vec::new();
    let mut buffer_views = Vec::new();
    let mut primitives = Vec::new();

    // Buffer view indices
    let mut buffer_view_idx = 0u32;

    // Add opaque mesh data if not empty
    if !opaque_mesh.is_empty() {
        let opaque_accessor_start = accessors.len() as u32;

        // Buffer views for opaque mesh
        buffer_views.push(create_buffer_view(
            opaque_positions_offset,
            opaque_positions_bytes,
            Some(json::buffer::Target::ArrayBuffer),
        ));
        let opaque_positions_view = buffer_view_idx;
        buffer_view_idx += 1;

        buffer_views.push(create_buffer_view(
            opaque_normals_offset,
            opaque_normals_bytes,
            Some(json::buffer::Target::ArrayBuffer),
        ));
        let opaque_normals_view = buffer_view_idx;
        buffer_view_idx += 1;

        buffer_views.push(create_buffer_view(
            opaque_uvs_offset,
            opaque_uvs_bytes,
            Some(json::buffer::Target::ArrayBuffer),
        ));
        let opaque_uvs_view = buffer_view_idx;
        buffer_view_idx += 1;

        buffer_views.push(create_buffer_view(
            opaque_colors_offset,
            opaque_colors_bytes,
            Some(json::buffer::Target::ArrayBuffer),
        ));
        let opaque_colors_view = buffer_view_idx;
        buffer_view_idx += 1;

        buffer_views.push(create_buffer_view(
            opaque_indices_offset,
            opaque_indices_bytes,
            Some(json::buffer::Target::ElementArrayBuffer),
        ));
        let opaque_indices_view = buffer_view_idx;
        buffer_view_idx += 1;

        // Accessors for opaque mesh
        accessors.push(create_accessor(
            opaque_positions_view,
            opaque_mesh.vertex_count(),
            json::accessor::Type::Vec3,
            json::accessor::ComponentType::F32,
            Some(min),
            Some(max),
        ));
        accessors.push(create_accessor(
            opaque_normals_view,
            opaque_mesh.vertex_count(),
            json::accessor::Type::Vec3,
            json::accessor::ComponentType::F32,
            None,
            None,
        ));
        accessors.push(create_accessor(
            opaque_uvs_view,
            opaque_mesh.vertex_count(),
            json::accessor::Type::Vec2,
            json::accessor::ComponentType::F32,
            None,
            None,
        ));
        accessors.push(create_accessor(
            opaque_colors_view,
            opaque_mesh.vertex_count(),
            json::accessor::Type::Vec4,
            json::accessor::ComponentType::F32,
            None,
            None,
        ));
        accessors.push(create_accessor(
            opaque_indices_view,
            opaque_indices.len(),
            json::accessor::Type::Scalar,
            json::accessor::ComponentType::U32,
            None,
            None,
        ));

        // Primitive for opaque mesh (material 0 = opaque)
        primitives.push(create_primitive(
            opaque_accessor_start,
            opaque_accessor_start + 4,
            0, // Opaque material
        ));
    }

    // Add transparent mesh data if not empty
    if !transparent_mesh.is_empty() {
        let transparent_accessor_start = accessors.len() as u32;

        // Buffer views for transparent mesh
        buffer_views.push(create_buffer_view(
            transparent_positions_offset,
            transparent_positions_bytes,
            Some(json::buffer::Target::ArrayBuffer),
        ));
        let transparent_positions_view = buffer_view_idx;
        buffer_view_idx += 1;

        buffer_views.push(create_buffer_view(
            transparent_normals_offset,
            transparent_normals_bytes,
            Some(json::buffer::Target::ArrayBuffer),
        ));
        let transparent_normals_view = buffer_view_idx;
        buffer_view_idx += 1;

        buffer_views.push(create_buffer_view(
            transparent_uvs_offset,
            transparent_uvs_bytes,
            Some(json::buffer::Target::ArrayBuffer),
        ));
        let transparent_uvs_view = buffer_view_idx;
        buffer_view_idx += 1;

        buffer_views.push(create_buffer_view(
            transparent_colors_offset,
            transparent_colors_bytes,
            Some(json::buffer::Target::ArrayBuffer),
        ));
        let transparent_colors_view = buffer_view_idx;
        buffer_view_idx += 1;

        buffer_views.push(create_buffer_view(
            transparent_indices_offset,
            transparent_indices_bytes,
            Some(json::buffer::Target::ElementArrayBuffer),
        ));
        let transparent_indices_view = buffer_view_idx;
        buffer_view_idx += 1;

        // Accessors for transparent mesh
        accessors.push(create_accessor(
            transparent_positions_view,
            transparent_mesh.vertex_count(),
            json::accessor::Type::Vec3,
            json::accessor::ComponentType::F32,
            Some(min),
            Some(max),
        ));
        accessors.push(create_accessor(
            transparent_normals_view,
            transparent_mesh.vertex_count(),
            json::accessor::Type::Vec3,
            json::accessor::ComponentType::F32,
            None,
            None,
        ));
        accessors.push(create_accessor(
            transparent_uvs_view,
            transparent_mesh.vertex_count(),
            json::accessor::Type::Vec2,
            json::accessor::ComponentType::F32,
            None,
            None,
        ));
        accessors.push(create_accessor(
            transparent_colors_view,
            transparent_mesh.vertex_count(),
            json::accessor::Type::Vec4,
            json::accessor::ComponentType::F32,
            None,
            None,
        ));
        accessors.push(create_accessor(
            transparent_indices_view,
            transparent_indices.len(),
            json::accessor::Type::Scalar,
            json::accessor::ComponentType::U32,
            None,
            None,
        ));

        // Primitive for transparent mesh (material 1 = blend)
        primitives.push(create_primitive(
            transparent_accessor_start,
            transparent_accessor_start + 4,
            1, // Transparent material
        ));
    }

    // Buffer view for texture
    buffer_views.push(json::buffer::View {
        buffer: json::Index::new(0),
        byte_length: USize64(texture_png.len() as u64),
        byte_offset: Some(USize64(aligned_texture_offset as u64)),
        byte_stride: None,
        extensions: Default::default(),
        extras: Default::default(),
        target: None,
    });
    let texture_view = buffer_view_idx;

    // Build materials
    let materials = vec![
        // Material 0: Opaque
        create_material(json::material::AlphaMode::Opaque),
        // Material 1: Transparent (Blend)
        create_material(json::material::AlphaMode::Blend),
    ];

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
        images: vec![json::Image {
            buffer_view: Some(json::Index::new(texture_view)),
            mime_type: Some(json::image::MimeType("image/png".to_string())),
            uri: None,
            extensions: Default::default(),
            extras: Default::default(),
        }],
        samplers: vec![json::texture::Sampler {
            mag_filter: Some(Valid(json::texture::MagFilter::Nearest)),
            min_filter: Some(Valid(json::texture::MinFilter::Nearest)),
            wrap_s: Valid(json::texture::WrappingMode::Repeat),
            wrap_t: Valid(json::texture::WrappingMode::Repeat),
            extensions: Default::default(),
            extras: Default::default(),
        }],
        textures: vec![json::Texture {
            sampler: Some(json::Index::new(0)),
            source: json::Index::new(0),
            extensions: Default::default(),
            extras: Default::default(),
        }],
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

/// Calculate bounding box from both meshes.
fn calculate_bounds(opaque: &Mesh, transparent: &Mesh) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];

    for vertex in opaque.vertices.iter().chain(transparent.vertices.iter()) {
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

/// Create a material with the specified alpha mode.
fn create_material(alpha_mode: json::material::AlphaMode) -> json::Material {
    json::Material {
        pbr_metallic_roughness: json::material::PbrMetallicRoughness {
            base_color_texture: Some(json::texture::Info {
                index: json::Index::new(0),
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
        };

        let glb = export_glb(&output).unwrap();
        assert_eq!(&glb[0..4], b"glTF");
    }
}
