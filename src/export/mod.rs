//! Mesh export formats.
//!
//! This module provides exporters for various 3D formats.
//!
//! ## Trait-based API
//!
//! Use [`MeshExporter`] with one of the built-in exporters ([`GlbExporter`], [`ObjExporter`],
//! [`UsdzExporter`]) for a uniform export interface:
//!
//! ```ignore
//! use schematic_mesher::export::{MeshExporter, GlbExporter};
//!
//! let exporter = GlbExporter { merge_layers: false };
//! let glb_bytes = exporter.export(&mesh_output)?;
//! ```

pub mod gltf;
pub mod obj;
pub mod raw;
pub mod usd;

pub use gltf::export_glb;
pub use obj::{export_obj, ObjExport};
pub use raw::{export_raw, RawMeshData};
pub use usd::{export_usda, export_usdz, UsdaExport, UsdTexture};

use crate::mesh_output::MeshOutput;

/// Trait for exporting a [`MeshOutput`] to a specific format.
///
/// Implementations receive a reference to the canonical mesh output and produce
/// a format-specific result.
pub trait MeshExporter {
    /// The output type produced by this exporter.
    type Output;

    /// Export the given mesh output.
    fn export(&self, mesh: &MeshOutput) -> crate::Result<Self::Output>;
}

/// GLB (binary glTF) exporter.
///
/// Produces a `Vec<u8>` containing a valid GLB file with embedded textures.
pub struct GlbExporter {
    /// If `true`, merge all layers into a single primitive. If `false` (default),
    /// emit separate primitives per transparency layer for correct rendering.
    pub merge_layers: bool,
}

impl MeshExporter for GlbExporter {
    type Output = Vec<u8>;

    fn export(&self, mesh: &MeshOutput) -> crate::Result<Self::Output> {
        if self.merge_layers {
            // Flatten to single layer, build a temporary MesherOutput with merged mesh
            let flat = mesh.flatten();
            let internal = crate::mesher::MesherOutput {
                opaque_mesh: crate::mesh_output::layer_to_internal_mesh(&flat),
                cutout_mesh: crate::mesher::geometry::Mesh::new(),
                transparent_mesh: crate::mesher::geometry::Mesh::new(),
                atlas: mesh.atlas.clone(),
                bounds: mesh.bounds,
                greedy_materials: Vec::new(),
                animated_textures: mesh.animated_textures.clone(),
            };
            gltf::export_glb(&internal)
        } else {
            mesh.to_glb()
        }
    }
}

/// Wavefront OBJ exporter.
///
/// Produces an [`ObjExport`] containing the OBJ text, MTL text, and texture PNGs.
pub struct ObjExporter {
    /// Object name used in the OBJ/MTL output.
    pub name: String,
}

impl MeshExporter for ObjExporter {
    type Output = ObjExport;

    fn export(&self, mesh: &MeshOutput) -> crate::Result<Self::Output> {
        mesh.to_obj(&self.name)
    }
}

/// USDZ exporter.
///
/// Produces a `Vec<u8>` containing a valid USDZ archive.
pub struct UsdzExporter;

impl MeshExporter for UsdzExporter {
    type Output = Vec<u8>;

    fn export(&self, mesh: &MeshOutput) -> crate::Result<Self::Output> {
        mesh.to_usdz()
    }
}
