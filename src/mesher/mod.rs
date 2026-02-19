//! Mesh generation from Minecraft block models.
//!
//! The [`Mesher`] is the main entry point. It takes a [`ResourcePack`](crate::ResourcePack)
//! and converts blocks into triangle meshes via:
//!
//! - [`mesh()`](Mesher::mesh) — Mesh an entire [`BlockSource`](crate::BlockSource)
//! - [`mesh_blocks()`](Mesher::mesh_blocks) — Mesh an iterator of `(BlockPosition, &InputBlock)`
//! - [`mesh_chunks()`](Mesher::mesh_chunks) — Lazy per-chunk iteration via [`ChunkIter`]
//!
//! All methods return geometry separated into opaque, cutout, and transparent layers
//! with a shared texture atlas.

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
    /// If set, skip per-chunk atlas building and use this pre-built atlas instead.
    /// UVs will be remapped to this atlas's regions. Used for global atlas workflows.
    pub pre_built_atlas: Option<TextureAtlas>,
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
            pre_built_atlas: None,
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
        let (opaque_mesh, cutout_mesh, transparent_mesh, atlas, greedy_materials, dynamic_animated) = mesh_builder.build(self.config.pre_built_atlas.clone())?;

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

    /// Create a lazy chunk iterator that meshes one cubic chunk at a time.
    ///
    /// Pre-scans the source to discover all unique chunk coordinates (using
    /// `div_euclid` for correct negative coordinate handling), then yields one
    /// [`MeshOutput`](crate::mesh_output::MeshOutput) per chunk with `chunk_coord`
    /// set to `Some((cx, cy, cz))`.
    ///
    /// Each chunk is meshed independently with its own atlas. If your [`BlockSource`]
    /// implements [`blocks_in_region()`](BlockSource::blocks_in_region) efficiently
    /// (e.g., by only reading relevant chunks from disk), this avoids loading the
    /// entire world into memory.
    ///
    /// `chunk_size` is the side length of each cubic chunk in blocks (typically 16).
    pub fn mesh_chunks<'s, S: BlockSource>(
        &'s self,
        source: &'s S,
        chunk_size: i32,
    ) -> ChunkIter<'s, S> {
        // Pre-scan to collect all unique chunk coords
        let mut chunk_coords: Vec<(i32, i32, i32)> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for (pos, _) in source.iter_blocks() {
            let cx = pos.x.div_euclid(chunk_size);
            let cy = pos.y.div_euclid(chunk_size);
            let cz = pos.z.div_euclid(chunk_size);
            if seen.insert((cx, cy, cz)) {
                chunk_coords.push((cx, cy, cz));
            }
        }

        ChunkIter {
            mesher: self,
            source,
            chunk_size,
            chunk_coords,
            index: 0,
        }
    }

    /// Discover all texture paths needed to mesh a block source, without building geometry.
    ///
    /// Runs the full face processing pipeline to collect texture references, but the
    /// resulting mesh is discarded. Use this to pre-scan blocks for a global atlas.
    pub fn discover_textures<S: BlockSource>(&self, source: &S) -> std::collections::HashSet<String> {
        let blocks: Vec<_> = source.iter_blocks().collect();
        let block_map: std::collections::HashMap<BlockPosition, &InputBlock> =
            blocks.iter().map(|(pos, block)| (*pos, *block)).collect();
        let culler = if self.config.cull_hidden_faces {
            Some(face_culler::FaceCuller::new(&self.resource_pack, &blocks))
        } else {
            None
        };
        let light_map: Option<lighting::LightMap> = None; // Skip lighting for discovery

        let mut mesh_builder = element::MeshBuilder::new(
            &self.resource_pack,
            &self.config,
            culler.as_ref(),
            Some(&block_map),
            light_map.as_ref(),
        );

        for (pos, block) in &blocks {
            if !self.config.include_air && block.is_air() {
                continue;
            }
            if self.config.cull_occluded_blocks {
                if let Some(ref culler) = culler {
                    if culler.is_fully_occluded(*pos) {
                        continue;
                    }
                }
            }
            let _ = mesh_builder.add_block(*pos, block);
        }

        mesh_builder.texture_refs().clone()
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

/// A lazy iterator that yields one [`MeshOutput`](crate::mesh_output::MeshOutput) per chunk.
///
/// Created by [`Mesher::mesh_chunks`]. Each yielded `MeshOutput` has its
/// [`chunk_coord`](crate::mesh_output::MeshOutput::chunk_coord) field set to
/// `Some((cx, cy, cz))`.
///
/// Use [`chunk_count()`](ChunkIter::chunk_count) and
/// [`chunk_coords()`](ChunkIter::chunk_coords) to inspect the work ahead without
/// consuming the iterator (useful for progress bars).
pub struct ChunkIter<'s, S: BlockSource> {
    mesher: &'s Mesher,
    source: &'s S,
    chunk_size: i32,
    chunk_coords: Vec<(i32, i32, i32)>,
    index: usize,
}

impl<'s, S: BlockSource> ChunkIter<'s, S> {
    /// Total number of chunks discovered during pre-scan.
    pub fn chunk_count(&self) -> usize {
        self.chunk_coords.len()
    }

    /// The chunk coordinates that will be iterated, in discovery order.
    pub fn chunk_coords(&self) -> &[(i32, i32, i32)] {
        &self.chunk_coords
    }
}

impl<'s, S: BlockSource> Iterator for ChunkIter<'s, S> {
    type Item = Result<crate::mesh_output::MeshOutput>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.chunk_coords.len() {
            return None;
        }

        let (cx, cy, cz) = self.chunk_coords[self.index];
        self.index += 1;

        let cs = self.chunk_size;
        let chunk_bounds = BoundingBox::new(
            [(cx * cs) as f32, (cy * cs) as f32, (cz * cs) as f32],
            [((cx + 1) * cs) as f32, ((cy + 1) * cs) as f32, ((cz + 1) * cs) as f32],
        );

        // Get blocks in this chunk region
        let blocks: Vec<_> = self.source.blocks_in_region(chunk_bounds).collect();
        if blocks.is_empty() {
            // Return empty MeshOutput for this chunk
            return Some(Ok(crate::mesh_output::MeshOutput {
                opaque: crate::mesh_output::MeshLayer::new(),
                cutout: crate::mesh_output::MeshLayer::new(),
                transparent: crate::mesh_output::MeshLayer::new(),
                atlas: crate::atlas::TextureAtlas::empty(),
                animated_textures: Vec::new(),
                bounds: chunk_bounds,
                chunk_coord: Some((cx, cy, cz)),
                lod_level: 0,
            }));
        }

        // Mesh this chunk
        let result = self.mesher.mesh_blocks(
            blocks.into_iter(),
            chunk_bounds,
        );

        Some(result.map(|mesher_output| {
            let mut mesh_output = crate::mesh_output::MeshOutput::from(&mesher_output);
            mesh_output.chunk_coord = Some((cx, cy, cz));
            mesh_output
        }))
    }
}

#[cfg(test)]
mod chunk_tests {
    use super::*;

    /// Simple in-memory block source for testing.
    struct TestBlockSource {
        blocks: Vec<(BlockPosition, InputBlock)>,
        bounds: BoundingBox,
    }

    impl BlockSource for TestBlockSource {
        fn get_block(&self, pos: BlockPosition) -> Option<&InputBlock> {
            self.blocks.iter().find(|(p, _)| *p == pos).map(|(_, b)| b)
        }

        fn iter_blocks(&self) -> Box<dyn Iterator<Item = (BlockPosition, &InputBlock)> + '_> {
            Box::new(self.blocks.iter().map(|(p, b)| (*p, b)))
        }

        fn bounds(&self) -> BoundingBox {
            self.bounds
        }
    }

    #[test]
    fn test_chunk_iter_yields_correct_coords() {
        // Place blocks in 3 different chunks (chunk_size=4):
        // block at (0,0,0) => chunk (0,0,0)
        // block at (4,0,0) => chunk (1,0,0)
        // block at (0,4,0) => chunk (0,1,0)
        let blocks = vec![
            (BlockPosition::new(0, 0, 0), InputBlock::new("minecraft:stone")),
            (BlockPosition::new(4, 0, 0), InputBlock::new("minecraft:stone")),
            (BlockPosition::new(0, 4, 0), InputBlock::new("minecraft:stone")),
        ];
        let source = TestBlockSource {
            bounds: BoundingBox::new([0.0, 0.0, 0.0], [5.0, 5.0, 1.0]),
            blocks,
        };

        // We need a resource pack to create a Mesher, but we can test ChunkIter
        // construction and coordinate discovery without actually meshing.
        // Use mesh_chunks to create the iterator and check chunk_count/coords.
        let pack = crate::ResourcePack::new();
        let mesher = Mesher::new(pack);
        let iter = mesher.mesh_chunks(&source, 4);

        assert_eq!(iter.chunk_count(), 3);

        let mut coords: Vec<_> = iter.chunk_coords().to_vec();
        coords.sort();
        assert_eq!(coords, vec![(0, 0, 0), (0, 1, 0), (1, 0, 0)]);
    }

    #[test]
    fn test_chunk_iter_single_chunk() {
        // All blocks in same chunk
        let blocks = vec![
            (BlockPosition::new(0, 0, 0), InputBlock::new("minecraft:stone")),
            (BlockPosition::new(1, 0, 0), InputBlock::new("minecraft:stone")),
            (BlockPosition::new(0, 1, 0), InputBlock::new("minecraft:stone")),
        ];
        let source = TestBlockSource {
            bounds: BoundingBox::new([0.0, 0.0, 0.0], [2.0, 2.0, 1.0]),
            blocks,
        };

        let pack = crate::ResourcePack::new();
        let mesher = Mesher::new(pack);
        let iter = mesher.mesh_chunks(&source, 16);

        assert_eq!(iter.chunk_count(), 1);
        assert_eq!(iter.chunk_coords(), &[(0, 0, 0)]);
    }

    #[test]
    fn test_chunk_iter_negative_coords() {
        // Blocks in negative coordinate space
        let blocks = vec![
            (BlockPosition::new(-1, 0, 0), InputBlock::new("minecraft:stone")),
            (BlockPosition::new(0, 0, 0), InputBlock::new("minecraft:stone")),
        ];
        let source = TestBlockSource {
            bounds: BoundingBox::new([-1.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
            blocks,
        };

        let pack = crate::ResourcePack::new();
        let mesher = Mesher::new(pack);
        let iter = mesher.mesh_chunks(&source, 4);

        assert_eq!(iter.chunk_count(), 2);
        let mut coords: Vec<_> = iter.chunk_coords().to_vec();
        coords.sort();
        // -1 div_euclid 4 = -1, 0 div_euclid 4 = 0
        assert_eq!(coords, vec![(-1, 0, 0), (0, 0, 0)]);
    }
}
