//! Convert model elements to mesh geometry.

use crate::atlas::{AtlasBuilder, TextureAtlas};
use crate::error::{MesherError, Result};
use crate::mesher::face_culler::FaceCuller;
use crate::mesher::geometry::{Mesh, Vertex};
use crate::mesher::MesherConfig;
use crate::resolver::{resolve_block, ModelResolver, ResolvedModel};
use crate::resource_pack::{BlockModel, ModelElement, ModelFace, ResourcePack};
use crate::types::{BlockPosition, BlockTransform, Direction, InputBlock};
use glam::{Mat3, Vec3};
use std::collections::HashSet;

/// Tracks texture mapping for a face (4 vertices).
struct FaceTextureMapping {
    /// Starting vertex index.
    vertex_start: u32,
    /// Texture path for this face.
    texture_path: String,
    /// Whether this face uses a transparent texture.
    is_transparent: bool,
}

/// Builds a mesh from multiple blocks.
pub struct MeshBuilder<'a> {
    resource_pack: &'a ResourcePack,
    config: &'a MesherConfig,
    mesh: Mesh,
    texture_refs: HashSet<String>,
    model_resolver: ModelResolver<'a>,
    /// Track which texture each face uses for UV remapping.
    face_textures: Vec<FaceTextureMapping>,
}

impl<'a> MeshBuilder<'a> {
    pub fn new(resource_pack: &'a ResourcePack, config: &'a MesherConfig) -> Self {
        Self {
            resource_pack,
            config,
            mesh: Mesh::new(),
            texture_refs: HashSet::new(),
            model_resolver: ModelResolver::new(resource_pack),
            face_textures: Vec::new(),
        }
    }

    /// Add a block to the mesh.
    pub fn add_block(
        &mut self,
        pos: BlockPosition,
        block: &InputBlock,
        culler: Option<&FaceCuller>,
    ) -> Result<()> {
        // Resolve the block to models
        let resolved_models = match resolve_block(self.resource_pack, block) {
            Ok(models) => models,
            Err(e) => {
                // Log warning but continue
                eprintln!("Warning: Failed to resolve block {}: {}", block.name, e);
                return Ok(());
            }
        };

        // Generate geometry for each model
        for resolved in resolved_models {
            self.add_model(pos, block, &resolved, culler)?;
        }

        Ok(())
    }

    /// Add a resolved model to the mesh.
    fn add_model(
        &mut self,
        pos: BlockPosition,
        block: &InputBlock,
        resolved: &ResolvedModel,
        culler: Option<&FaceCuller>,
    ) -> Result<()> {
        let model = &resolved.model;
        let transform = &resolved.transform;

        // Resolve textures for this model
        let resolved_textures = self.model_resolver.resolve_textures(model);

        // Process each element
        for element in &model.elements {
            self.add_element(pos, block, element, transform, &resolved_textures, culler)?;
        }

        Ok(())
    }

    /// Add an element to the mesh.
    fn add_element(
        &mut self,
        pos: BlockPosition,
        block: &InputBlock,
        element: &ModelElement,
        transform: &BlockTransform,
        resolved_textures: &std::collections::HashMap<String, String>,
        culler: Option<&FaceCuller>,
    ) -> Result<()> {
        // Process each face
        for (direction, face) in &element.faces {
            // Transform the face direction by block rotation (for AO and cullface)
            let world_direction = direction.rotate_by_transform(transform.x, transform.y);

            // Check if face should be culled
            if let Some(cullface) = &face.cullface {
                if let Some(culler) = culler {
                    // Transform cullface direction to world space
                    let world_cullface = cullface.rotate_by_transform(transform.x, transform.y);
                    if culler.should_cull(pos, world_cullface) {
                        continue;
                    }
                }
            }

            // Resolve the texture reference
            let texture_path = self.resolve_face_texture(&face.texture, resolved_textures);
            self.texture_refs.insert(texture_path.clone());

            // Check if texture has transparency
            let is_transparent = self.resource_pack
                .get_texture(&texture_path)
                .map(|t| t.has_transparency())
                .unwrap_or(false);

            // Track texture mapping for UV remapping
            let vertex_start = self.mesh.vertex_count() as u32;
            self.face_textures.push(FaceTextureMapping {
                vertex_start,
                texture_path,
                is_transparent,
            });

            // Calculate AO if enabled (use world direction for neighbor checks)
            let ao_values = if self.config.ambient_occlusion {
                culler.map(|c| c.calculate_ao(pos, world_direction))
            } else {
                None
            };

            // Generate face geometry
            self.add_face(pos, block, element, *direction, face, transform, ao_values)?;
        }

        Ok(())
    }

    /// Resolve a texture reference to a path.
    fn resolve_face_texture(
        &self,
        reference: &str,
        resolved_textures: &std::collections::HashMap<String, String>,
    ) -> String {
        if reference.starts_with('#') {
            let key = &reference[1..];
            resolved_textures
                .get(key)
                .cloned()
                .unwrap_or_else(|| "block/missing".to_string())
        } else {
            reference.to_string()
        }
    }

    /// Add a face to the mesh.
    fn add_face(
        &mut self,
        pos: BlockPosition,
        block: &InputBlock,
        element: &ModelElement,
        direction: Direction,
        face: &ModelFace,
        transform: &BlockTransform,
        ao_values: Option<[u8; 4]>,
    ) -> Result<()> {
        let normal = direction.normal();
        let uv = face.normalized_uv();

        // Get element bounds in normalized space
        let from = element.normalized_from();
        let to = element.normalized_to();

        // Generate the 4 vertices for this face
        let (positions, uvs) = self.generate_face_vertices(direction, from, to, uv, face.rotation);

        // Apply element rotation if present
        let positions = if let Some(rot) = &element.rotation {
            self.apply_element_rotation(&positions, rot)
        } else {
            positions
        };

        // Apply block transform rotation
        let positions = self.apply_block_transform(&positions, transform);
        let normal = self.rotate_normal(normal, transform);

        // Translate to world position
        let offset = [pos.x as f32, pos.y as f32, pos.z as f32];

        // Get tint color from the tint provider based on block type and tint index
        let base_color = self.config.tint_provider.get_tint(block, face.tintindex);

        // Calculate per-vertex colors with AO
        let colors = if let Some(ao) = ao_values {
            let intensity = self.config.ao_intensity;
            [
                self.apply_ao_to_color(base_color, ao[0], intensity),
                self.apply_ao_to_color(base_color, ao[1], intensity),
                self.apply_ao_to_color(base_color, ao[2], intensity),
                self.apply_ao_to_color(base_color, ao[3], intensity),
            ]
        } else {
            [base_color; 4]
        };

        let v0 = self.mesh.add_vertex(
            Vertex::new(
                [
                    positions[0][0] + offset[0],
                    positions[0][1] + offset[1],
                    positions[0][2] + offset[2],
                ],
                normal,
                uvs[0],
            )
            .with_color(colors[0]),
        );
        let v1 = self.mesh.add_vertex(
            Vertex::new(
                [
                    positions[1][0] + offset[0],
                    positions[1][1] + offset[1],
                    positions[1][2] + offset[2],
                ],
                normal,
                uvs[1],
            )
            .with_color(colors[1]),
        );
        let v2 = self.mesh.add_vertex(
            Vertex::new(
                [
                    positions[2][0] + offset[0],
                    positions[2][1] + offset[1],
                    positions[2][2] + offset[2],
                ],
                normal,
                uvs[2],
            )
            .with_color(colors[2]),
        );
        let v3 = self.mesh.add_vertex(
            Vertex::new(
                [
                    positions[3][0] + offset[0],
                    positions[3][1] + offset[1],
                    positions[3][2] + offset[2],
                ],
                normal,
                uvs[3],
            )
            .with_color(colors[3]),
        );

        // Use AO-aware quad triangulation to fix anisotropy
        if let Some(ao) = ao_values {
            self.mesh.add_quad_ao(v0, v1, v2, v3, ao);
        } else {
            self.mesh.add_quad(v0, v1, v2, v3);
        }

        Ok(())
    }

    /// Apply ambient occlusion to a color.
    /// ao_level: 0-3 (0=darkest, 3=brightest)
    /// intensity: 0.0-1.0 (how much to darken)
    fn apply_ao_to_color(&self, color: [f32; 4], ao_level: u8, intensity: f32) -> [f32; 4] {
        // Map AO level 0-3 to brightness factor
        // Level 3 = full brightness (1.0)
        // Level 0 = minimum brightness (1.0 - intensity)
        let brightness = 1.0 - intensity * (1.0 - ao_level as f32 / 3.0);
        [
            color[0] * brightness,
            color[1] * brightness,
            color[2] * brightness,
            color[3], // Alpha unchanged
        ]
    }

    /// Generate the 4 vertices for a face.
    /// Returns (positions, uvs) in CCW order.
    fn generate_face_vertices(
        &self,
        direction: Direction,
        from: [f32; 3],
        to: [f32; 3],
        uv: [f32; 4],
        rotation: i32,
    ) -> ([[f32; 3]; 4], [[f32; 2]; 4]) {
        let (u1, v1, u2, v2) = (uv[0], uv[1], uv[2], uv[3]);

        // Base UVs (before rotation)
        // glTF uses v=0 at top (same as Minecraft), so no V flip needed
        // UV order: top-left, top-right, bottom-right, bottom-left (CCW)
        let base_uvs = [[u1, v1], [u2, v1], [u2, v2], [u1, v2]];

        // Rotate UVs
        let uvs = self.rotate_uvs(base_uvs, rotation);

        // Generate positions based on direction
        let positions = match direction {
            Direction::Down => [
                [from[0], from[1], to[2]],
                [to[0], from[1], to[2]],
                [to[0], from[1], from[2]],
                [from[0], from[1], from[2]],
            ],
            Direction::Up => [
                [from[0], to[1], from[2]],
                [to[0], to[1], from[2]],
                [to[0], to[1], to[2]],
                [from[0], to[1], to[2]],
            ],
            Direction::North => [
                [to[0], to[1], from[2]],
                [from[0], to[1], from[2]],
                [from[0], from[1], from[2]],
                [to[0], from[1], from[2]],
            ],
            Direction::South => [
                [from[0], to[1], to[2]],
                [to[0], to[1], to[2]],
                [to[0], from[1], to[2]],
                [from[0], from[1], to[2]],
            ],
            Direction::West => [
                [from[0], to[1], from[2]],
                [from[0], to[1], to[2]],
                [from[0], from[1], to[2]],
                [from[0], from[1], from[2]],
            ],
            Direction::East => [
                [to[0], to[1], to[2]],
                [to[0], to[1], from[2]],
                [to[0], from[1], from[2]],
                [to[0], from[1], to[2]],
            ],
        };

        (positions, uvs)
    }

    /// Rotate UV coordinates.
    fn rotate_uvs(&self, uvs: [[f32; 2]; 4], rotation: i32) -> [[f32; 2]; 4] {
        let steps = ((rotation / 90) % 4 + 4) % 4;
        let mut result = uvs;
        for _ in 0..steps {
            result = [result[3], result[0], result[1], result[2]];
        }
        result
    }

    /// Apply element rotation to positions.
    fn apply_element_rotation(
        &self,
        positions: &[[f32; 3]; 4],
        rotation: &crate::types::ElementRotation,
    ) -> [[f32; 3]; 4] {
        let origin = rotation.normalized_origin();
        let angle = rotation.angle_radians();
        let rescale = rotation.rescale_factor();

        let rotation_matrix = match rotation.axis {
            crate::types::Axis::X => Mat3::from_rotation_x(angle),
            crate::types::Axis::Y => Mat3::from_rotation_y(angle),
            crate::types::Axis::Z => Mat3::from_rotation_z(angle),
        };

        let mut result = [[0.0; 3]; 4];
        for (i, pos) in positions.iter().enumerate() {
            // Translate to origin
            let p = Vec3::new(pos[0] - origin[0], pos[1] - origin[1], pos[2] - origin[2]);

            // Rotate
            let rotated = rotation_matrix * p;

            // Rescale if needed
            let scaled = if rescale != 1.0 {
                match rotation.axis {
                    crate::types::Axis::X => {
                        Vec3::new(rotated.x, rotated.y * rescale, rotated.z * rescale)
                    }
                    crate::types::Axis::Y => {
                        Vec3::new(rotated.x * rescale, rotated.y, rotated.z * rescale)
                    }
                    crate::types::Axis::Z => {
                        Vec3::new(rotated.x * rescale, rotated.y * rescale, rotated.z)
                    }
                }
            } else {
                rotated
            };

            // Translate back
            result[i] = [
                scaled.x + origin[0],
                scaled.y + origin[1],
                scaled.z + origin[2],
            ];
        }
        result
    }

    /// Apply block-level transform to positions.
    fn apply_block_transform(
        &self,
        positions: &[[f32; 3]; 4],
        transform: &BlockTransform,
    ) -> [[f32; 3]; 4] {
        if transform.is_identity() {
            return *positions;
        }

        let x_rot = Mat3::from_rotation_x((transform.x as f32).to_radians());
        let y_rot = Mat3::from_rotation_y((transform.y as f32).to_radians());
        let rotation_matrix = y_rot * x_rot;

        let mut result = [[0.0; 3]; 4];
        for (i, pos) in positions.iter().enumerate() {
            let p = Vec3::new(pos[0], pos[1], pos[2]);
            let rotated = rotation_matrix * p;
            result[i] = [rotated.x, rotated.y, rotated.z];
        }
        result
    }

    /// Rotate a normal by the block transform.
    fn rotate_normal(&self, normal: [f32; 3], transform: &BlockTransform) -> [f32; 3] {
        if transform.is_identity() {
            return normal;
        }

        let x_rot = Mat3::from_rotation_x((transform.x as f32).to_radians());
        let y_rot = Mat3::from_rotation_y((transform.y as f32).to_radians());
        let rotation_matrix = y_rot * x_rot;

        let n = Vec3::new(normal[0], normal[1], normal[2]);
        let rotated = rotation_matrix * n;
        [rotated.x, rotated.y, rotated.z]
    }

    /// Build the final meshes (opaque and transparent) and atlas.
    /// Returns (opaque_mesh, transparent_mesh, atlas).
    /// Opaque geometry should be rendered first, then transparent.
    pub fn build(mut self) -> Result<(Mesh, Mesh, TextureAtlas)> {
        // Build texture atlas
        let mut atlas_builder = AtlasBuilder::new(
            self.config.atlas_max_size,
            self.config.atlas_padding,
        );

        for texture_ref in &self.texture_refs {
            if let Some(texture) = self.resource_pack.get_texture(texture_ref) {
                atlas_builder.add_texture(texture_ref.clone(), texture.first_frame());
            }
        }

        let atlas = atlas_builder.build()?;

        // Remap UVs to atlas coordinates
        for face_mapping in &self.face_textures {
            if let Some(region) = atlas.get_region(&face_mapping.texture_path) {
                // Each face has 4 vertices
                for i in 0..4 {
                    let vertex_idx = face_mapping.vertex_start as usize + i;
                    if vertex_idx < self.mesh.vertices.len() {
                        let vertex = &mut self.mesh.vertices[vertex_idx];
                        // Transform UV from [0,1] local space to atlas region
                        vertex.uv = region.transform_uv(vertex.uv[0], vertex.uv[1]);
                    }
                }
            }
        }

        // Separate into opaque and transparent meshes
        let (opaque_mesh, transparent_mesh) = self.separate_by_transparency();

        Ok((opaque_mesh, transparent_mesh, atlas))
    }

    /// Separate the mesh into opaque and transparent parts based on texture transparency.
    fn separate_by_transparency(&self) -> (Mesh, Mesh) {
        let mut opaque_mesh = Mesh::new();
        let mut transparent_mesh = Mesh::new();

        // Process each face (4 vertices + 2 triangles per face)
        for face_mapping in &self.face_textures {
            let start = face_mapping.vertex_start as usize;

            // Get the 4 vertices for this face
            let vertices: Vec<_> = (0..4)
                .filter_map(|i| self.mesh.vertices.get(start + i).copied())
                .collect();

            if vertices.len() != 4 {
                continue;
            }

            // Find the triangles that use these vertices
            // In our mesh, faces are added as quads which become 2 triangles
            // We need to find the triangle indices that reference these vertices

            // Add to appropriate mesh
            let target_mesh = if face_mapping.is_transparent {
                &mut transparent_mesh
            } else {
                &mut opaque_mesh
            };

            // Add the 4 vertices and get their new indices
            let v0 = target_mesh.add_vertex(vertices[0]);
            let v1 = target_mesh.add_vertex(vertices[1]);
            let v2 = target_mesh.add_vertex(vertices[2]);
            let v3 = target_mesh.add_vertex(vertices[3]);

            // Find the triangle winding from original mesh
            // Search for triangles that use the original vertex indices
            let orig_v0 = face_mapping.vertex_start;
            let orig_v1 = orig_v0 + 1;
            let orig_v2 = orig_v0 + 2;
            let orig_v3 = orig_v0 + 3;

            // Find triangles in original mesh that use these vertices
            for tri_idx in (0..self.mesh.indices.len()).step_by(3) {
                let i0 = self.mesh.indices[tri_idx];
                let i1 = self.mesh.indices[tri_idx + 1];
                let i2 = self.mesh.indices[tri_idx + 2];

                // Check if this triangle uses our face's vertices
                if i0 >= orig_v0 && i0 <= orig_v3 &&
                   i1 >= orig_v0 && i1 <= orig_v3 &&
                   i2 >= orig_v0 && i2 <= orig_v3 {
                    // Map old indices to new
                    let new_i0 = match i0 - orig_v0 {
                        0 => v0, 1 => v1, 2 => v2, _ => v3
                    };
                    let new_i1 = match i1 - orig_v0 {
                        0 => v0, 1 => v1, 2 => v2, _ => v3
                    };
                    let new_i2 = match i2 - orig_v0 {
                        0 => v0, 1 => v1, 2 => v2, _ => v3
                    };
                    target_mesh.add_triangle(new_i0, new_i1, new_i2);
                }
            }
        }

        (opaque_mesh, transparent_mesh)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rotate_uvs() {
        let pack = ResourcePack::new();
        let config = MesherConfig::default();
        let builder = MeshBuilder::new(&pack, &config);

        let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];

        // 0 degrees - no change
        assert_eq!(builder.rotate_uvs(uvs, 0), uvs);

        // 90 degrees
        let rotated_90 = builder.rotate_uvs(uvs, 90);
        assert_eq!(rotated_90[0], uvs[3]);
        assert_eq!(rotated_90[1], uvs[0]);

        // 180 degrees
        let rotated_180 = builder.rotate_uvs(uvs, 180);
        assert_eq!(rotated_180[0], uvs[2]);
        assert_eq!(rotated_180[2], uvs[0]);
    }
}
