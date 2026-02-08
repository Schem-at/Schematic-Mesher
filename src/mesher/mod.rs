//! Mesh generation from block models.
//!
//! This module converts resolved block models into triangle meshes.

pub mod geometry;
pub mod element;
pub mod entity;
pub mod face_culler;
pub mod greedy;
pub mod lighting;
pub mod liquid;
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
    /// Skip blocks that are fully hidden by opaque neighbors on all 6 sides.
    pub cull_occluded_blocks: bool,
    /// Merge adjacent coplanar faces into larger quads (reduces triangle count).
    /// Merged faces use separate materials with REPEAT wrapping for proper tiling.
    pub greedy_meshing: bool,
    /// Enable ambient occlusion.
    pub ambient_occlusion: bool,
    /// AO intensity (0.0 = no darkening, 1.0 = full darkening).
    pub ao_intensity: f32,
    /// Enable block light (torches, glowstone, etc.).
    pub enable_block_light: bool,
    /// Enable sky light (sunlight from above).
    pub enable_sky_light: bool,
    /// Sky light level (0-15, default 15 for daytime).
    pub sky_light_level: u8,
    /// Enable static particle marker quads (torches, campfires, candles, etc.).
    pub enable_particles: bool,
}

impl Default for MesherConfig {
    fn default() -> Self {
        Self {
            cull_hidden_faces: true,
            cull_occluded_blocks: true,
            greedy_meshing: false,
            atlas_max_size: 4096,
            atlas_padding: 1,
            include_air: false,
            tint_provider: TintProvider::new(),
            ambient_occlusion: true,
            ao_intensity: 0.4,
            enable_block_light: false,
            enable_sky_light: false,
            sky_light_level: 15,
            enable_particles: true,
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

/// Animation metadata for a texture in the atlas, exported for viewer-side frame cycling.
#[derive(Debug, Clone)]
pub struct AnimatedTextureExport {
    /// The sprite sheet PNG (all frames stacked vertically).
    pub sprite_sheet_png: Vec<u8>,
    /// Number of animation frames.
    pub frame_count: u32,
    /// Ticks per frame (Minecraft default: 1 tick = 50ms).
    pub frametime: u32,
    /// Whether to interpolate between frames.
    pub interpolate: bool,
    /// Explicit frame order (indices into sprite sheet). None = sequential.
    pub frames: Option<Vec<u32>>,
    /// Frame width in pixels.
    pub frame_width: u32,
    /// Frame height in pixels.
    pub frame_height: u32,
    /// Atlas region: pixel X offset.
    pub atlas_x: u32,
    /// Atlas region: pixel Y offset.
    pub atlas_y: u32,
}

/// Output from the mesher.
#[derive(Debug)]
pub struct MesherOutput {
    /// The opaque geometry mesh (rendered first, backface culled).
    pub opaque_mesh: Mesh,
    /// Binary-alpha geometry mesh (rendered second, alpha-tested, writes depth).
    /// Used for fire, flowers, leaves, etc. where pixels are fully opaque or fully transparent.
    pub cutout_mesh: Mesh,
    /// The semi-transparent geometry mesh (rendered last, alpha-blended, no depth write).
    /// Used for water, ice, stained glass, etc.
    pub transparent_mesh: Mesh,
    /// The texture atlas.
    pub atlas: TextureAtlas,
    /// Bounding box of the mesh.
    pub bounds: BoundingBox,
    /// Per-texture materials for greedy-merged faces (bypass atlas, use REPEAT wrapping).
    pub greedy_materials: Vec<element::GreedyMaterial>,
    /// Animated texture metadata for viewer-side frame cycling.
    pub animated_textures: Vec<AnimatedTextureExport>,
}

impl MesherOutput {
    /// Get a combined mesh (for backwards compatibility).
    /// Note: For correct transparency, use opaque_mesh, cutout_mesh, and transparent_mesh separately.
    /// Includes greedy material meshes.
    pub fn mesh(&self) -> Mesh {
        let mut combined = self.opaque_mesh.clone();
        combined.merge(&self.cutout_mesh);
        combined.merge(&self.transparent_mesh);
        for gm in &self.greedy_materials {
            combined.merge(&gm.opaque_mesh);
            combined.merge(&gm.transparent_mesh);
        }
        combined
    }

    /// Check if the output has any transparent or cutout geometry.
    pub fn has_transparency(&self) -> bool {
        !self.cutout_mesh.is_empty()
            || !self.transparent_mesh.is_empty()
            || self.greedy_materials.iter().any(|gm| !gm.transparent_mesh.is_empty())
    }

    /// Get total vertex count across all meshes.
    pub fn total_vertices(&self) -> usize {
        self.opaque_mesh.vertex_count()
            + self.cutout_mesh.vertex_count()
            + self.transparent_mesh.vertex_count()
            + self.greedy_materials.iter().map(|gm| {
                gm.opaque_mesh.vertex_count() + gm.transparent_mesh.vertex_count()
            }).sum::<usize>()
    }

    /// Get total triangle count across all meshes.
    pub fn total_triangles(&self) -> usize {
        self.opaque_mesh.triangle_count()
            + self.cutout_mesh.triangle_count()
            + self.transparent_mesh.triangle_count()
            + self.greedy_materials.iter().map(|gm| {
                gm.opaque_mesh.triangle_count() + gm.transparent_mesh.triangle_count()
            }).sum::<usize>()
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
        // Collect blocks for face culling
        let blocks: Vec<_> = blocks.collect();

        // Build block map for neighbor lookups (used by liquid geometry)
        let block_map: std::collections::HashMap<BlockPosition, &InputBlock> =
            blocks.iter().map(|(pos, block)| (*pos, *block)).collect();

        // Build occupancy map for face culling if enabled
        // Uses model data to determine which blocks are full opaque cubes
        let culler = if self.config.cull_hidden_faces {
            Some(face_culler::FaceCuller::new(&self.resource_pack, &blocks))
        } else {
            None
        };

        // Build light map if lighting is enabled
        let lighting_config = lighting::LightingConfig {
            enable_block_light: self.config.enable_block_light,
            enable_sky_light: self.config.enable_sky_light,
            sky_light_level: self.config.sky_light_level,
            ambient_light: 0.05,
        };
        let light_map = if lighting_config.is_enabled() {
            Some(lighting::LightMap::compute(&blocks, &lighting_config))
        } else {
            None
        };

        let mut mesh_builder = element::MeshBuilder::new(
            &self.resource_pack,
            &self.config,
            culler.as_ref(),
            Some(&block_map),
            light_map.as_ref(),
        );

        // Process each block
        for (pos, block) in &blocks {
            if !self.config.include_air && block.is_air() {
                continue;
            }

            // Skip blocks that are fully occluded by opaque neighbors
            if self.config.cull_occluded_blocks {
                if let Some(ref culler) = culler {
                    if culler.is_fully_occluded(*pos) {
                        continue;
                    }
                }
            }

            mesh_builder.add_block(*pos, block)?;
        }

        // Build the final meshes and atlas
        let (opaque_mesh, cutout_mesh, transparent_mesh, atlas, greedy_materials, dynamic_animated) = mesh_builder.build()?;

        // Collect animated texture metadata for viewer-side frame cycling
        let mut animated_textures = Self::collect_animated_textures(&self.resource_pack, &atlas);
        animated_textures.extend(dynamic_animated);

        Ok(MesherOutput {
            opaque_mesh,
            cutout_mesh,
            transparent_mesh,
            atlas,
            bounds,
            greedy_materials,
            animated_textures,
        })
    }

    /// Collect animation metadata for textures that are animated and present in the atlas.
    fn collect_animated_textures(
        resource_pack: &ResourcePack,
        atlas: &TextureAtlas,
    ) -> Vec<AnimatedTextureExport> {
        let mut result = Vec::new();

        for (texture_path, region) in &atlas.regions {
            let texture = match resource_pack.get_texture(texture_path) {
                Some(t) if t.is_animated && t.frame_count > 1 => t,
                _ => continue,
            };

            // Encode the full sprite sheet as PNG
            let sprite_sheet_png = match texture.to_png() {
                Ok(png) => png,
                Err(_) => continue,
            };

            let anim = texture.animation.as_ref();
            let frame_width = anim.and_then(|a| a.frame_width).unwrap_or(texture.width);
            let frame_height = anim.and_then(|a| a.frame_height).unwrap_or(frame_width);
            let frametime = anim.map(|a| a.frametime).unwrap_or(1);
            let interpolate = anim.map(|a| a.interpolate).unwrap_or(false);
            let frames = anim.and_then(|a| a.frames.as_ref()).map(|fs| {
                fs.iter().map(|f| f.index).collect()
            });

            // Convert atlas region from UV (0-1) to pixel coordinates
            let atlas_x = (region.u_min * atlas.width as f32).round() as u32;
            let atlas_y = (region.v_min * atlas.height as f32).round() as u32;

            result.push(AnimatedTextureExport {
                sprite_sheet_png,
                frame_count: texture.frame_count,
                frametime,
                interpolate,
                frames,
                frame_width,
                frame_height,
                atlas_x,
                atlas_y,
            });
        }

        result
    }
}
