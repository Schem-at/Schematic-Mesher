//! # Schematic Mesher
//!
//! A Rust library for generating 3D meshes from Minecraft schematics.
//!
//! ## Overview
//!
//! This library takes a Minecraft schematic and resource pack as input,
//! and produces a 3D mesh (GLB/glTF) with a texture atlas as output.
//!
//! ## Quick Start
//!
//! ```ignore
//! use schematic_mesher::{load_resource_pack, Mesher, export_glb};
//!
//! // Load a resource pack
//! let pack = load_resource_pack("path/to/pack.zip")?;
//!
//! // Create a mesher
//! let mesher = Mesher::new(pack);
//!
//! // Generate mesh from blocks (using BlockSource trait)
//! let output = mesher.mesh(&my_block_source)?;
//!
//! // Export to GLB
//! let glb_bytes = export_glb(&output)?;
//! ```
//!
//! ## Library Integration
//!
//! For integrating with existing block storage (like Nucleation), implement the
//! `BlockSource` trait or use `mesh_blocks()` with an iterator of blocks:
//!
//! ```ignore
//! use schematic_mesher::{Mesher, InputBlock, BlockPosition, BoundingBox};
//!
//! // Create blocks from your schematic
//! let blocks: Vec<(BlockPosition, InputBlock)> = /* ... */;
//! let bounds = BoundingBox::new([0, 0, 0], [16, 16, 16]);
//!
//! // Mesh with references
//! let output = mesher.mesh_blocks(
//!     blocks.iter().map(|(pos, block)| (*pos, block)),
//!     bounds
//! )?;
//! ```

pub mod error;
pub mod types;
pub mod resource_pack;
pub mod resolver;
pub mod mesher;
pub mod atlas;
pub mod export;

// Re-export main types for convenience
pub use error::{MesherError, Result};
pub use types::{Direction, Axis, BlockPosition, BoundingBox, InputBlock, BlockSource};
pub use resource_pack::{ResourcePack, BlockModel, ModelElement, BlockstateDefinition};
pub use mesher::{Mesher, MesherConfig, MesherOutput, Mesh, Vertex, TintColors, TintProvider};
pub use atlas::TextureAtlas;
pub use export::gltf::export_glb;
pub use export::obj::{export_obj, ObjExport};
pub use export::raw::{export_raw, RawMeshData};

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
