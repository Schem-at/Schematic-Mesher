//! Convert model elements to mesh geometry.

use crate::atlas::{AtlasBuilder, TextureAtlas};
use crate::error::{MesherError, Result};
use crate::mesher::face_culler::FaceCuller;
use crate::mesher::geometry::{Mesh, Vertex};
use crate::mesher::greedy::{FaceMergeKey, GreedyMesher, quantize_color};
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

/// Tracks texture mapping for a greedy-merged face (bypasses atlas).
struct GreedyFaceMapping {
    /// Starting vertex index.
    vertex_start: u32,
    /// Texture path for this face.
    texture_path: String,
    /// Whether this face uses a transparent texture.
    is_transparent: bool,
    /// Per-vertex AO values (from FaceMergeKey).
    ao: [u8; 4],
}

/// A material for greedy-merged faces, with its own texture (not atlas-packed).
/// UVs on these meshes exceed [0,1] for tiling via REPEAT wrapping.
#[derive(Debug)]
pub struct GreedyMaterial {
    /// Texture path (e.g., "block/stone").
    pub texture_path: String,
    /// Opaque geometry using this texture.
    pub opaque_mesh: Mesh,
    /// Transparent geometry using this texture.
    pub transparent_mesh: Mesh,
    /// PNG-encoded texture data for this material.
    pub texture_png: Vec<u8>,
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
    /// Track greedy-merged faces separately (bypass atlas, use tiled UVs).
    greedy_face_textures: Vec<GreedyFaceMapping>,
    /// Face culler for visibility and AO calculations.
    culler: Option<&'a FaceCuller<'a>>,
    /// Greedy mesher for merging adjacent coplanar faces.
    greedy: Option<GreedyMesher>,
}

impl<'a> MeshBuilder<'a> {
    pub fn new(
        resource_pack: &'a ResourcePack,
        config: &'a MesherConfig,
        culler: Option<&'a FaceCuller<'a>>,
    ) -> Self {
        let greedy = if config.greedy_meshing {
            Some(GreedyMesher::new())
        } else {
            None
        };
        Self {
            resource_pack,
            config,
            mesh: Mesh::new(),
            texture_refs: HashSet::new(),
            model_resolver: ModelResolver::new(resource_pack),
            face_textures: Vec::new(),
            greedy_face_textures: Vec::new(),
            culler,
            greedy,
        }
    }

    /// Add a block to the mesh.
    pub fn add_block(
        &mut self,
        pos: BlockPosition,
        block: &InputBlock,
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
            self.add_model(pos, block, &resolved)?;
        }

        Ok(())
    }

    /// Add a resolved model to the mesh.
    fn add_model(
        &mut self,
        pos: BlockPosition,
        block: &InputBlock,
        resolved: &ResolvedModel,
    ) -> Result<()> {
        let model = &resolved.model;
        let transform = &resolved.transform;

        // Resolve textures for this model
        let resolved_textures = self.model_resolver.resolve_textures(model);

        // Process each element
        for element in &model.elements {
            self.add_element(pos, block, element, transform, &resolved_textures)?;
        }

        Ok(())
    }

    /// Check if an element/face is eligible for greedy merging.
    fn is_greedy_eligible(
        &self,
        element: &ModelElement,
        face: &ModelFace,
        transform: &BlockTransform,
    ) -> bool {
        // Must be a full cube element: from=[0,0,0], to=[16,16,16]
        const EPSILON: f32 = 0.001;
        if element.from[0].abs() > EPSILON
            || element.from[1].abs() > EPSILON
            || element.from[2].abs() > EPSILON
        {
            return false;
        }
        if (element.to[0] - 16.0).abs() > EPSILON
            || (element.to[1] - 16.0).abs() > EPSILON
            || (element.to[2] - 16.0).abs() > EPSILON
        {
            return false;
        }

        // No element rotation
        if element.rotation.is_some() {
            return false;
        }

        // Block transform must be identity
        if !transform.is_identity() {
            return false;
        }

        // UV must cover full texture (default [0,0,16,16])
        let uv = face.uv_or_default();
        if uv[0].abs() > EPSILON
            || uv[1].abs() > EPSILON
            || (uv[2] - 16.0).abs() > EPSILON
            || (uv[3] - 16.0).abs() > EPSILON
        {
            return false;
        }

        // UV rotation must be 0
        if face.rotation != 0 {
            return false;
        }

        true
    }

    /// Add an element to the mesh.
    fn add_element(
        &mut self,
        pos: BlockPosition,
        block: &InputBlock,
        element: &ModelElement,
        transform: &BlockTransform,
        resolved_textures: &std::collections::HashMap<String, String>,
    ) -> Result<()> {
        // Process each face
        for (direction, face) in &element.faces {
            // Transform the face direction by block rotation (for AO and cullface)
            let world_direction = direction.rotate_by_transform(transform.x, transform.y);

            // Check if face should be culled
            if let Some(cullface) = &face.cullface {
                if let Some(culler) = self.culler {
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

            // Route to greedy mesher if eligible
            if self.greedy.is_some() && self.is_greedy_eligible(element, face, transform) {
                let base_color = self.config.tint_provider.get_tint(block, face.tintindex);
                // Compute per-vertex AO for this face so it's included in the merge key
                let ao = if self.config.ambient_occlusion {
                    self.culler
                        .map(|c| c.calculate_ao(pos, world_direction))
                        .unwrap_or([3, 3, 3, 3])
                } else {
                    [3, 3, 3, 3]
                };
                let key = FaceMergeKey {
                    texture: texture_path,
                    tint: quantize_color(base_color),
                    ao,
                };
                self.greedy.as_mut().unwrap().add_face(
                    pos,
                    world_direction,
                    key,
                    is_transparent,
                );
                continue;
            }

            // Track texture mapping for UV remapping
            let vertex_start = self.mesh.vertex_count() as u32;
            self.face_textures.push(FaceTextureMapping {
                vertex_start,
                texture_path,
                is_transparent,
            });

            // Calculate AO if enabled (use world direction for neighbor checks)
            let ao_values = if self.config.ambient_occlusion {
                self.culler.map(|c| c.calculate_ao(pos, world_direction))
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

    /// Emit merged quads from the greedy mesher into the mesh.
    /// Greedy faces use tiled UVs [0, width] x [0, height] and bypass the atlas.
    /// AO is baked into the tile texture (not vertex colors) for universal viewer support.
    fn emit_greedy_quads(&mut self) {
        let greedy = match self.greedy.take() {
            Some(g) => g,
            None => return,
        };

        let merged_quads = greedy.merge();

        for quad in &merged_quads {
            let positions = quad.world_positions();
            let normal = quad.direction.normal();

            // Convert quantized tint back to f32 color (no AO — it's baked into the texture)
            let base_color = [
                quad.tint[0] as f32 / 255.0,
                quad.tint[1] as f32 / 255.0,
                quad.tint[2] as f32 / 255.0,
                quad.tint[3] as f32 / 255.0,
            ];

            // Tiled UVs: [0, width] x [0, height] so texture repeats per block
            let w = quad.width as f32;
            let h = quad.height as f32;
            let uvs = [[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]];

            // Track greedy face separately (bypasses atlas UV remapping)
            let vertex_start = self.mesh.vertex_count() as u32;
            self.greedy_face_textures.push(GreedyFaceMapping {
                vertex_start,
                texture_path: quad.texture.clone(),
                is_transparent: quad.is_transparent,
                ao: quad.ao,
            });

            let v0 = self.mesh.add_vertex(
                Vertex::new(positions[0], normal, uvs[0]).with_color(base_color),
            );
            let v1 = self.mesh.add_vertex(
                Vertex::new(positions[1], normal, uvs[1]).with_color(base_color),
            );
            let v2 = self.mesh.add_vertex(
                Vertex::new(positions[2], normal, uvs[2]).with_color(base_color),
            );
            let v3 = self.mesh.add_vertex(
                Vertex::new(positions[3], normal, uvs[3]).with_color(base_color),
            );

            // Use AO-aware triangulation even though colors are uniform,
            // to keep consistent winding with the AO baked into the texture
            if self.config.ambient_occlusion && quad.ao != [3, 3, 3, 3] {
                self.mesh.add_quad_ao(v0, v1, v2, v3, quad.ao);
            } else {
                self.mesh.add_quad(v0, v1, v2, v3);
            }
        }
    }

    /// Build the final meshes (opaque and transparent) and atlas.
    /// Returns (opaque_mesh, transparent_mesh, atlas, greedy_materials).
    /// Opaque geometry should be rendered first, then transparent.
    /// Greedy materials have their own textures with REPEAT wrapping for tiling.
    pub fn build(mut self) -> Result<(Mesh, Mesh, TextureAtlas, Vec<GreedyMaterial>)> {
        // Emit greedy-merged quads into the mesh before atlas building
        self.emit_greedy_quads();

        // Build texture atlas (only for non-greedy faces)
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

        // Remap UVs to atlas coordinates (only non-greedy faces)
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

        // Separate atlas-based faces into opaque and transparent meshes
        let (opaque_mesh, transparent_mesh) = self.separate_by_transparency();

        // Build greedy materials: group greedy faces by texture path
        let greedy_materials = self.build_greedy_materials();

        Ok((opaque_mesh, transparent_mesh, atlas, greedy_materials))
    }

    /// Build per-texture GreedyMaterial meshes from greedy faces.
    /// Groups by (texture_path, ao_pattern) so each AO variant gets its own
    /// baked texture with AO darkening applied at the pixel level.
    fn build_greedy_materials(&self) -> Vec<GreedyMaterial> {
        use std::collections::HashMap;

        // Group greedy faces by (texture_path, ao_pattern)
        // This ensures faces with different AO get different baked textures.
        type MaterialKey = (String, [u8; 4]);
        let mut material_map: HashMap<MaterialKey, (Mesh, Mesh)> = HashMap::new();

        for face_mapping in &self.greedy_face_textures {
            let start = face_mapping.vertex_start as usize;

            let vertices: Vec<_> = (0..4)
                .filter_map(|i| self.mesh.vertices.get(start + i).copied())
                .collect();

            if vertices.len() != 4 {
                continue;
            }

            let key = (face_mapping.texture_path.clone(), face_mapping.ao);
            let (opaque_mesh, transparent_mesh) = material_map
                .entry(key)
                .or_insert_with(|| (Mesh::new(), Mesh::new()));

            let target_mesh = if face_mapping.is_transparent {
                transparent_mesh
            } else {
                opaque_mesh
            };

            let v0 = target_mesh.add_vertex(vertices[0]);
            let v1 = target_mesh.add_vertex(vertices[1]);
            let v2 = target_mesh.add_vertex(vertices[2]);
            let v3 = target_mesh.add_vertex(vertices[3]);

            let orig_v0 = face_mapping.vertex_start;
            let orig_v3 = orig_v0 + 3;

            for tri_idx in (0..self.mesh.indices.len()).step_by(3) {
                let i0 = self.mesh.indices[tri_idx];
                let i1 = self.mesh.indices[tri_idx + 1];
                let i2 = self.mesh.indices[tri_idx + 2];

                if i0 >= orig_v0 && i0 <= orig_v3 &&
                   i1 >= orig_v0 && i1 <= orig_v3 &&
                   i2 >= orig_v0 && i2 <= orig_v3 {
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

        let ao_intensity = self.config.ao_intensity;

        material_map
            .into_iter()
            .map(|((texture_path, ao), (opaque_mesh, transparent_mesh))| {
                let texture_png = if ao == [3, 3, 3, 3] {
                    // No AO darkening — use original tile texture
                    self.resource_pack
                        .get_texture(&texture_path)
                        .and_then(|t| t.first_frame().to_png().ok())
                        .unwrap_or_default()
                } else {
                    // Bake AO gradient into tile texture pixels
                    self.resource_pack
                        .get_texture(&texture_path)
                        .map(|t| {
                            let frame = t.first_frame();
                            bake_ao_into_tile(&frame.pixels, frame.width, frame.height, ao, ao_intensity)
                        })
                        .unwrap_or_default()
                };
                GreedyMaterial {
                    texture_path,
                    opaque_mesh,
                    transparent_mesh,
                    texture_png,
                }
            })
            .collect()
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

/// Bake ambient occlusion into a tile texture's pixels.
///
/// Creates a new PNG where each pixel is darkened according to a bilinear
/// interpolation of the 4 corner AO values. The tile repeats with REPEAT
/// wrapping, so each block in a merged quad independently shows the AO gradient.
///
/// AO corner mapping (image coordinates, row 0 = top):
///   (0,0)=AO[0]  (w,0)=AO[1]
///   (0,h)=AO[3]  (w,h)=AO[2]
///
/// In glTF, UV (0,0) = top-left of image. So emit_greedy_quads maps:
///   vertex 0 UV(0,0) → image top-left    → AO[0]
///   vertex 1 UV(w,0) → image top-right   → AO[1]
///   vertex 2 UV(w,h) → image bottom-right→ AO[2]
///   vertex 3 UV(0,h) → image bottom-left → AO[3]
fn bake_ao_into_tile(
    pixels: &[u8],
    width: u32,
    height: u32,
    ao: [u8; 4],
    intensity: f32,
) -> Vec<u8> {
    use image::{ImageBuffer, Rgba};

    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(width, height, pixels.to_vec())
            .expect("Failed to create image buffer for AO baking");

    let w = width.max(1) as f32;
    let h = height.max(1) as f32;

    // Precompute brightness for each AO level
    let brightness: [f32; 4] = std::array::from_fn(|i| {
        1.0 - intensity * (1.0 - ao[i] as f32 / 3.0)
    });

    for row in 0..height {
        for col in 0..width {
            // Normalized position in image space (row 0 = top)
            let cx = col as f32 / (w - 1.0).max(1.0); // 0=left, 1=right
            let cy = row as f32 / (h - 1.0).max(1.0); // 0=top, 1=bottom

            // Bilinear interpolation of corner brightness values
            // Image corners: top-left=AO[0], top-right=AO[1], bottom-left=AO[3], bottom-right=AO[2]
            let b = (1.0 - cx) * (1.0 - cy) * brightness[0]
                  + cx * (1.0 - cy) * brightness[1]
                  + (1.0 - cx) * cy * brightness[3]
                  + cx * cy * brightness[2];

            let pixel = img.get_pixel_mut(col, row);
            pixel[0] = (pixel[0] as f32 * b).round().min(255.0) as u8;
            pixel[1] = (pixel[1] as f32 * b).round().min(255.0) as u8;
            pixel[2] = (pixel[2] as f32 * b).round().min(255.0) as u8;
            // Alpha unchanged
        }
    }

    let mut bytes = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut bytes);
    img.write_to(&mut cursor, image::ImageFormat::Png)
        .expect("Failed to encode AO-baked tile as PNG");
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_rotate_uvs() {
        let pack = ResourcePack::new();
        let config = MesherConfig::default();
        let builder = MeshBuilder::new(&pack, &config, None);

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

    fn full_cube_element() -> ModelElement {
        ModelElement {
            from: [0.0, 0.0, 0.0],
            to: [16.0, 16.0, 16.0],
            rotation: None,
            shade: true,
            faces: HashMap::new(),
        }
    }

    fn full_face() -> ModelFace {
        ModelFace {
            uv: None, // defaults to [0,0,16,16]
            texture: "#all".to_string(),
            cullface: Some(Direction::Up),
            rotation: 0,
            tintindex: -1,
        }
    }

    #[test]
    fn test_greedy_eligible_full_cube() {
        let pack = ResourcePack::new();
        let config = MesherConfig::default();
        let builder = MeshBuilder::new(&pack, &config, None);

        let element = full_cube_element();
        let face = full_face();
        let identity = BlockTransform::default();

        assert!(builder.is_greedy_eligible(&element, &face, &identity));
    }

    #[test]
    fn test_greedy_ineligible_partial_element() {
        let pack = ResourcePack::new();
        let config = MesherConfig::default();
        let builder = MeshBuilder::new(&pack, &config, None);

        // Slab-like element (half height)
        let element = ModelElement {
            from: [0.0, 0.0, 0.0],
            to: [16.0, 8.0, 16.0],
            rotation: None,
            shade: true,
            faces: HashMap::new(),
        };
        let face = full_face();
        let identity = BlockTransform::default();

        assert!(!builder.is_greedy_eligible(&element, &face, &identity));
    }

    #[test]
    fn test_greedy_ineligible_rotated_block() {
        let pack = ResourcePack::new();
        let config = MesherConfig::default();
        let builder = MeshBuilder::new(&pack, &config, None);

        let element = full_cube_element();
        let face = full_face();
        let rotated = BlockTransform::new(0, 90, false);

        assert!(!builder.is_greedy_eligible(&element, &face, &rotated));
    }

    #[test]
    fn test_greedy_ineligible_custom_uv() {
        let pack = ResourcePack::new();
        let config = MesherConfig::default();
        let builder = MeshBuilder::new(&pack, &config, None);

        let element = full_cube_element();
        let face = ModelFace {
            uv: Some([0.0, 0.0, 8.0, 8.0]), // Half-size UV
            texture: "#all".to_string(),
            cullface: None,
            rotation: 0,
            tintindex: -1,
        };
        let identity = BlockTransform::default();

        assert!(!builder.is_greedy_eligible(&element, &face, &identity));
    }

    #[test]
    fn test_greedy_ineligible_rotated_uv() {
        let pack = ResourcePack::new();
        let config = MesherConfig::default();
        let builder = MeshBuilder::new(&pack, &config, None);

        let element = full_cube_element();
        let face = ModelFace {
            uv: None,
            texture: "#all".to_string(),
            cullface: None,
            rotation: 90, // Rotated UV
            tintindex: -1,
        };
        let identity = BlockTransform::default();

        assert!(!builder.is_greedy_eligible(&element, &face, &identity));
    }

    #[test]
    fn test_greedy_ineligible_element_rotation() {
        let pack = ResourcePack::new();
        let config = MesherConfig::default();
        let builder = MeshBuilder::new(&pack, &config, None);

        let element = ModelElement {
            from: [0.0, 0.0, 0.0],
            to: [16.0, 16.0, 16.0],
            rotation: Some(crate::types::ElementRotation {
                origin: [8.0, 8.0, 8.0],
                axis: crate::types::Axis::Y,
                angle: 22.5,
                rescale: false,
            }),
            shade: true,
            faces: HashMap::new(),
        };
        let face = full_face();
        let identity = BlockTransform::default();

        assert!(!builder.is_greedy_eligible(&element, &face, &identity));
    }
}
