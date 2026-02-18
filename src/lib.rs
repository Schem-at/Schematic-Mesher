//! # Schematic Mesher
//!
//! A Rust library for converting Minecraft block schematics into triangle meshes
//! with texture atlases. Supports GLB, OBJ, USDZ, and raw vertex buffer exports.
//!
//! ## Overview
//!
//! The library provides two output APIs:
//!
//! - **[`MesherOutput`]** — The internal output type with per-mesh [`Mesh`]/[`Vertex`]
//!   structs. Use this with the standalone export functions ([`export_glb`], [`export_obj`],
//!   [`export_usdz`], [`export_raw`]) or when you need access to greedy material meshes.
//!
//! - **[`MeshOutput`]** / **[`MeshLayer`]** — The canonical public API with
//!   structure-of-arrays vertex data and zero-copy `_bytes()` accessors for direct GPU
//!   upload. Use this when integrating with a game engine or custom renderer.
//!
//! Both APIs produce the same geometry. `MeshOutput` converts from `MesherOutput` via
//! [`From`], merging greedy material meshes into the appropriate transparency layers.
//!
//! ## Quick Start
//!
//! ```ignore
//! use schematic_mesher::{load_resource_pack, Mesher, export_glb};
//!
//! let pack = load_resource_pack("path/to/pack.zip")?;
//! let mesher = Mesher::new(pack);
//! let output = mesher.mesh(&my_block_source)?;
//! let glb_bytes = export_glb(&output)?;
//! ```
//!
//! ## Export Formats
//!
//! ### GLB (Binary glTF)
//!
//! The recommended format for web viewers and game engines. Produces a single self-contained
//! binary with embedded textures, quantized vertices ([`KHR_mesh_quantization`]), and
//! separate primitives per transparency layer (opaque, cutout, transparent).
//!
//! ```ignore
//! // Standalone function (from MesherOutput)
//! let glb = export_glb(&mesher_output)?;
//!
//! // Method on MeshOutput
//! let mesh_output = MeshOutput::from(&mesher_output);
//! let glb = mesh_output.to_glb()?;
//!
//! // Trait-based exporter
//! use schematic_mesher::export::{MeshExporter, GlbExporter};
//! let exporter = GlbExporter { merge_layers: false };
//! let glb = exporter.export(&mesh_output)?;
//! ```
//!
//! ### OBJ (Wavefront)
//!
//! A widely-supported text format. Produces separate OBJ text, MTL text, and PNG textures.
//! Greedy materials get individual MTL entries with per-texture files.
//!
//! ```ignore
//! // Standalone function (returns (obj_string, mtl_string))
//! let (obj, mtl) = export_obj(&mesher_output, "scene")?;
//!
//! // Method on MeshOutput (returns ObjExport with all files)
//! let obj_export = mesh_output.to_obj("scene")?;
//! ```
//!
//! ### USDZ (Universal Scene Description)
//!
//! Apple's AR format. Produces a zero-compression ZIP archive with USDA text and
//! 64-byte-aligned texture assets. Suitable for AR Quick Look on iOS/macOS.
//!
//! ```ignore
//! let usdz = export_usdz(&mesher_output)?;
//! // or
//! let usdz = mesh_output.to_usdz()?;
//! ```
//!
//! ### Raw Vertex Data
//!
//! For custom renderers. Returns structure-of-arrays vertex attributes and atlas pixels.
//! Combines all transparency layers into a single buffer.
//!
//! ```ignore
//! let raw = export_raw(&mesher_output);
//! // raw.positions, raw.normals, raw.uvs, raw.colors, raw.indices
//! // raw.texture_rgba, raw.texture_width, raw.texture_height
//! ```
//!
//! ### MeshOutput (Zero-Copy GPU Buffers)
//!
//! For direct GPU upload in game engines. Each [`MeshLayer`] provides `_bytes()` accessors
//! that return `&[u8]` views over the underlying `Vec<[f32; N]>` data without allocation.
//!
//! ```ignore
//! let output = MeshOutput::from(&mesher_output);
//!
//! // Upload to GPU (e.g., wgpu)
//! let vbo = device.create_buffer_init(&BufferInitDescriptor {
//!     contents: output.opaque.positions_bytes(),
//!     usage: BufferUsages::VERTEX,
//! });
//! ```
//!
//! ## Transparency Layers
//!
//! Geometry is separated into three layers for correct rendering:
//!
//! | Layer | Alpha Mode | Depth Write | Examples |
//! |-------|-----------|-------------|----------|
//! | **Opaque** | `OPAQUE` | Yes | Stone, dirt, wood |
//! | **Cutout** | `MASK` (alpha test) | Yes | Leaves, flowers, fire |
//! | **Transparent** | `BLEND` | No | Water, stained glass, ice |
//!
//! Render in order: opaque first, then cutout, then transparent.
//!
//! ## Chunk Iteration
//!
//! For large worlds, use [`ChunkIter`] to mesh one chunk at a time without
//! loading the entire world into memory:
//!
//! ```ignore
//! let iter = mesher.mesh_chunks(&block_source, 16);
//! for result in iter {
//!     let chunk_mesh: MeshOutput = result?;
//!     let (cx, cy, cz) = chunk_mesh.chunk_coord.unwrap();
//!     // Upload chunk_mesh to GPU...
//! }
//! ```
//!
//! ## Library Integration
//!
//! Implement [`BlockSource`] for your block storage to use `mesher.mesh()` and
//! `mesher.mesh_chunks()`. Or use `mesh_blocks()` with an iterator:
//!
//! ```ignore
//! use schematic_mesher::{Mesher, InputBlock, BlockPosition, BoundingBox};
//!
//! let blocks = vec![
//!     (BlockPosition::new(0, 0, 0), InputBlock::new("minecraft:stone")),
//!     (BlockPosition::new(1, 0, 0), InputBlock::new("minecraft:grass_block")
//!         .with_property("snowy", "false")),
//! ];
//! let bounds = BoundingBox::new([0.0, 0.0, 0.0], [2.0, 1.0, 1.0]);
//!
//! let output = mesher.mesh_blocks(
//!     blocks.iter().map(|(pos, block)| (*pos, block)),
//!     bounds,
//! )?;
//! ```

pub mod error;
pub mod types;
pub mod resource_pack;
pub mod resolver;
pub mod mesher;
pub mod atlas;
pub mod export;
pub mod mesh_output;

// --- Core types ---
pub use error::{MesherError, Result};
pub use types::{Direction, Axis, BlockPosition, BoundingBox, InputBlock, BlockSource};
pub use resource_pack::{ResourcePack, BlockModel, ModelElement, BlockstateDefinition};
pub use atlas::TextureAtlas;

// --- Mesher ---
pub use mesher::{Mesher, MesherConfig, MesherOutput, Mesh, Vertex, TintColors, TintProvider};
pub use mesher::ChunkIter;

// --- Canonical output types ---
pub use mesh_output::{MeshOutput, MeshLayer};

// --- Export: standalone functions ---
pub use export::gltf::export_glb;
pub use export::obj::{export_obj, ObjExport};
pub use export::raw::{export_raw, RawMeshData};
pub use export::usd::{export_usda, export_usdz, UsdaExport, UsdTexture};

// --- Export: trait-based API ---
pub use export::{MeshExporter, GlbExporter, ObjExporter, UsdzExporter};

/// Load a resource pack from a file path (ZIP or directory).
pub fn load_resource_pack<P: AsRef<std::path::Path>>(path: P) -> Result<ResourcePack> {
    resource_pack::loader::load_from_path(path)
}

/// Load a resource pack from bytes (for WASM compatibility).
pub fn load_resource_pack_from_bytes(data: &[u8]) -> Result<ResourcePack> {
    resource_pack::loader::load_from_bytes(data)
}

#[cfg(feature = "wasm")]
pub mod wasm;
