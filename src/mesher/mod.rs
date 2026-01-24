//! Mesh generation from block models.
//!
//! This module converts resolved block models into triangle meshes.

pub mod geometry;
pub mod element;
pub mod face_culler;
pub mod tint;

pub use geometry::{Mesh, Vertex};
pub use tint::{TintColors, TintProvider};

use crate::atlas::TextureAtlas;
use crate::error::Result;
use crate::resource_pack::ResourcePack;
use crate::types::{BlockPosition, BlockSource, BoundingBox, InputBlock};

/// Main mesher configuration.
#[derive(Debug, Clone)]
pub struct MesherConfig {
    /// Enable face culling between adjacent blocks.
    pub cull_hidden_faces: bool,
    /// Maximum texture atlas dimension.
    pub atlas_max_size: u32,
    /// Padding between textures in the atlas.
    pub atlas_padding: u32,
    /// Include air blocks in output.
    pub include_air: bool,
    /// Tint provider for block coloring (grass, foliage, water, redstone, etc.)
    pub tint_provider: TintProvider,
    /// Enable ambient occlusion.
    pub ambient_occlusion: bool,
    /// AO intensity (0.0 = no darkening, 1.0 = full darkening).
    pub ao_intensity: f32,
}

impl Default for MesherConfig {
    fn default() -> Self {
        Self {
            cull_hidden_faces: true,
            atlas_max_size: 4096,
            atlas_padding: 1,
            include_air: false,
            tint_provider: TintProvider::new(),
            ambient_occlusion: true,
            ao_intensity: 0.4,
        }
    }
}

impl MesherConfig {
    /// Create config with a specific biome for tinting.
    pub fn with_biome(mut self, biome: &str) -> Self {
        self.tint_provider = TintProvider::for_biome(biome);
        self
    }

    /// Create config with custom tint colors.
    pub fn with_tint_colors(mut self, colors: TintColors) -> Self {
        self.tint_provider = TintProvider::with_colors(colors);
        self
    }
}

/// Output from the mesher.
#[derive(Debug)]
pub struct MesherOutput {
    /// The opaque geometry mesh (rendered first).
    pub opaque_mesh: Mesh,
    /// The transparent geometry mesh (rendered second).
    pub transparent_mesh: Mesh,
    /// The texture atlas.
    pub atlas: TextureAtlas,
    /// Bounding box of the mesh.
    pub bounds: BoundingBox,
}

impl MesherOutput {
    /// Get a combined mesh (for backwards compatibility).
    /// Note: For correct transparency, use opaque_mesh and transparent_mesh separately.
    pub fn mesh(&self) -> Mesh {
        let mut combined = self.opaque_mesh.clone();
        combined.merge(&self.transparent_mesh);
        combined
    }

    /// Check if the output has any transparent geometry.
    pub fn has_transparency(&self) -> bool {
        !self.transparent_mesh.is_empty()
    }

    /// Get total vertex count across both meshes.
    pub fn total_vertices(&self) -> usize {
        self.opaque_mesh.vertex_count() + self.transparent_mesh.vertex_count()
    }

    /// Get total triangle count across both meshes.
    pub fn total_triangles(&self) -> usize {
        self.opaque_mesh.triangle_count() + self.transparent_mesh.triangle_count()
    }
}

/// The main mesher struct.
pub struct Mesher {
    resource_pack: ResourcePack,
    config: MesherConfig,
}

impl Mesher {
    /// Create a new mesher with default configuration.
    pub fn new(resource_pack: ResourcePack) -> Self {
        Self {
            resource_pack,
            config: MesherConfig::default(),
        }
    }

    /// Create a new mesher with custom configuration.
    pub fn with_config(resource_pack: ResourcePack, config: MesherConfig) -> Self {
        Self {
            resource_pack,
            config,
        }
    }

    /// Get a reference to the resource pack.
    pub fn resource_pack(&self) -> &ResourcePack {
        &self.resource_pack
    }

    /// Get a reference to the configuration.
    pub fn config(&self) -> &MesherConfig {
        &self.config
    }

    /// Generate a mesh from a block source.
    pub fn mesh<S: BlockSource>(&self, source: &S) -> Result<MesherOutput> {
        let bounds = source.bounds();
        let blocks: Vec<_> = source.iter_blocks().collect();

        self.mesh_blocks_internal(blocks.into_iter(), bounds)
    }

    /// Generate a mesh from an iterator of blocks.
    pub fn mesh_blocks<'a>(
        &self,
        blocks: impl Iterator<Item = (BlockPosition, &'a InputBlock)>,
        bounds: BoundingBox,
    ) -> Result<MesherOutput> {
        self.mesh_blocks_internal(blocks, bounds)
    }

    fn mesh_blocks_internal<'a>(
        &self,
        blocks: impl Iterator<Item = (BlockPosition, &'a InputBlock)>,
        bounds: BoundingBox,
    ) -> Result<MesherOutput> {
        let mut mesh_builder = element::MeshBuilder::new(&self.resource_pack, &self.config);

        // Collect blocks for face culling
        let blocks: Vec<_> = blocks.collect();

        // Build occupancy map for face culling if enabled
        // Uses model data to determine which blocks are full opaque cubes
        let culler = if self.config.cull_hidden_faces {
            Some(face_culler::FaceCuller::new(&self.resource_pack, &blocks))
        } else {
            None
        };

        // Process each block
        for (pos, block) in &blocks {
            if !self.config.include_air && block.is_air() {
                continue;
            }

            mesh_builder.add_block(*pos, block, culler.as_ref())?;
        }

        // Build the final meshes and atlas
        let (opaque_mesh, transparent_mesh, atlas) = mesh_builder.build()?;

        Ok(MesherOutput {
            opaque_mesh,
            transparent_mesh,
            atlas,
            bounds,
        })
    }
}
