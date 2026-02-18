//! Canonical mesh output types.
//!
//! [`MeshOutput`] and [`MeshLayer`] are the primary public types for renderer-agnostic
//! mesh data. Each layer contains interleaved vertex attributes (positions, normals, UVs,
//! colors) and triangle indices, with zero-copy byte accessors for GPU upload.

use crate::atlas::TextureAtlas;
use crate::error::Result;
use crate::export::obj::ObjExport;
use crate::mesher::AnimatedTextureExport;
use crate::types::BoundingBox;
use std::mem;

/// A single mesh layer with vertex attributes and indices.
///
/// Each layer represents geometry for one transparency class (opaque, cutout, or transparent).
/// Vertex data is stored in structure-of-arrays layout for efficient GPU upload.
#[derive(Debug, Clone, Default)]
pub struct MeshLayer {
    /// Vertex positions in world space.
    pub positions: Vec<[f32; 3]>,
    /// Vertex normals (unit length).
    pub normals: Vec<[f32; 3]>,
    /// Texture coordinates into the atlas.
    pub uvs: Vec<[f32; 2]>,
    /// Vertex tint colors (biome coloring, AO, lighting baked in). RGBA, premultiplied.
    pub colors: Vec<[f32; 4]>,
    /// Triangle indices (three per triangle).
    pub indices: Vec<u32>,
}

impl MeshLayer {
    /// Create a new empty mesh layer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` if this layer contains no vertices.
    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    /// Number of vertices in this layer.
    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    /// Number of triangles in this layer.
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Raw bytes of the positions array. Zero-allocation view.
    pub fn positions_bytes(&self) -> &[u8] {
        cast_slice(&self.positions)
    }

    /// Raw bytes of the normals array. Zero-allocation view.
    pub fn normals_bytes(&self) -> &[u8] {
        cast_slice(&self.normals)
    }

    /// Raw bytes of the UVs array. Zero-allocation view.
    pub fn uvs_bytes(&self) -> &[u8] {
        cast_slice(&self.uvs)
    }

    /// Raw bytes of the colors array. Zero-allocation view.
    pub fn colors_bytes(&self) -> &[u8] {
        cast_slice(&self.colors)
    }

    /// Raw bytes of the indices array. Zero-allocation view.
    pub fn indices_bytes(&self) -> &[u8] {
        cast_slice(&self.indices)
    }

    /// Merge another layer into this one, offsetting indices appropriately.
    pub fn merge(&mut self, other: &MeshLayer) {
        let offset = self.positions.len() as u32;
        self.positions.extend_from_slice(&other.positions);
        self.normals.extend_from_slice(&other.normals);
        self.uvs.extend_from_slice(&other.uvs);
        self.colors.extend_from_slice(&other.colors);
        self.indices.extend(other.indices.iter().map(|&i| i + offset));
    }
}

/// Cast a slice of `T` to a byte slice without allocation.
fn cast_slice<T: Copy>(slice: &[T]) -> &[u8] {
    let ptr = slice.as_ptr() as *const u8;
    let len = slice.len() * mem::size_of::<T>();
    // SAFETY: [f32; N], [u32] are all Pod-like types with no padding.
    unsafe { std::slice::from_raw_parts(ptr, len) }
}

/// The canonical mesh output from the mesher.
///
/// Contains three transparency layers (opaque, cutout, transparent), the texture atlas,
/// animation metadata, and spatial information. Provides convenience methods for export
/// and layer inspection.
#[derive(Debug, Clone)]
pub struct MeshOutput {
    /// Opaque geometry (rendered first, backface culled, depth write).
    pub opaque: MeshLayer,
    /// Alpha-test geometry (leaves, grass, fire — rendered second, depth write).
    pub cutout: MeshLayer,
    /// Alpha-blend geometry (glass, water — rendered last, no depth write).
    pub transparent: MeshLayer,
    /// The texture atlas shared by all layers.
    pub atlas: TextureAtlas,
    /// Animated texture metadata for viewer-side frame cycling.
    pub animated_textures: Vec<AnimatedTextureExport>,
    /// Axis-aligned bounding box of the meshed region.
    pub bounds: BoundingBox,
    /// Chunk coordinate, set when meshed via [`ChunkIter`]. `None` for single-shot meshing.
    pub chunk_coord: Option<(i32, i32, i32)>,
    /// LOD level. 0 = full detail, higher = coarser. No logic yet — hook for future LOD.
    pub lod_level: u8,
}

impl MeshOutput {
    /// Returns `true` if all layers are empty.
    pub fn is_empty(&self) -> bool {
        self.opaque.is_empty() && self.cutout.is_empty() && self.transparent.is_empty()
    }

    /// Total vertex count across all layers.
    pub fn total_vertices(&self) -> usize {
        self.opaque.vertex_count() + self.cutout.vertex_count() + self.transparent.vertex_count()
    }

    /// Total triangle count across all layers.
    pub fn total_triangles(&self) -> usize {
        self.opaque.triangle_count()
            + self.cutout.triangle_count()
            + self.transparent.triangle_count()
    }

    /// Returns `true` if there is any cutout or transparent geometry.
    pub fn has_transparency(&self) -> bool {
        !self.cutout.is_empty() || !self.transparent.is_empty()
    }

    /// Merge all layers into a single [`MeshLayer`]. Loses transparency ordering.
    pub fn flatten(&self) -> MeshLayer {
        let mut combined = self.opaque.clone();
        combined.merge(&self.cutout);
        combined.merge(&self.transparent);
        combined
    }

    /// Export to GLB (binary glTF) format.
    ///
    /// Converts the [`MeshOutput`] back to the internal [`MesherOutput`] format and
    /// delegates to the existing GLB exporter.
    pub fn to_glb(&self) -> Result<Vec<u8>> {
        let internal = self.to_mesher_output();
        crate::export::gltf::export_glb(&internal)
    }

    /// Export to USDZ format.
    pub fn to_usdz(&self) -> Result<Vec<u8>> {
        let internal = self.to_mesher_output();
        crate::export::usd::export_usdz(&internal)
    }

    /// Export to OBJ format.
    pub fn to_obj(&self, name: &str) -> Result<ObjExport> {
        let internal = self.to_mesher_output();
        ObjExport::from_output(&internal, name)
    }

    /// Convert back to the internal `MesherOutput` for use with existing exporters.
    fn to_mesher_output(&self) -> crate::mesher::MesherOutput {
        crate::mesher::MesherOutput {
            opaque_mesh: layer_to_mesh(&self.opaque),
            cutout_mesh: layer_to_mesh(&self.cutout),
            transparent_mesh: layer_to_mesh(&self.transparent),
            atlas: self.atlas.clone(),
            bounds: self.bounds,
            greedy_materials: Vec::new(),
            animated_textures: self.animated_textures.clone(),
        }
    }
}

/// Convert a [`MeshLayer`] to the internal [`Mesh`](crate::mesher::geometry::Mesh) type.
///
/// This is exposed as `pub(crate)` for use by exporters.
pub(crate) fn layer_to_internal_mesh(layer: &MeshLayer) -> crate::mesher::geometry::Mesh {
    layer_to_mesh(layer)
}

/// Convert a [`MeshLayer`] to the internal [`Mesh`](crate::mesher::geometry::Mesh) type.
fn layer_to_mesh(layer: &MeshLayer) -> crate::mesher::geometry::Mesh {
    use crate::mesher::geometry::{Mesh, Vertex};

    let mut mesh = Mesh::new();
    for i in 0..layer.positions.len() {
        mesh.vertices.push(Vertex {
            position: layer.positions[i],
            normal: layer.normals[i],
            uv: layer.uvs[i],
            color: layer.colors[i],
        });
    }
    mesh.indices = layer.indices.clone();
    mesh
}

/// Convert an internal [`Mesh`](crate::mesher::geometry::Mesh) to a [`MeshLayer`].
pub(crate) fn mesh_to_layer(mesh: &crate::mesher::geometry::Mesh) -> MeshLayer {
    MeshLayer {
        positions: mesh.vertices.iter().map(|v| v.position).collect(),
        normals: mesh.vertices.iter().map(|v| v.normal).collect(),
        uvs: mesh.vertices.iter().map(|v| v.uv).collect(),
        colors: mesh.vertices.iter().map(|v| v.color).collect(),
        indices: mesh.indices.clone(),
    }
}

impl From<&crate::mesher::MesherOutput> for MeshOutput {
    /// Convert from the internal [`MesherOutput`] to the canonical [`MeshOutput`].
    ///
    /// Greedy material meshes are merged into the appropriate layers (opaque/transparent).
    fn from(output: &crate::mesher::MesherOutput) -> Self {
        let mut opaque = mesh_to_layer(&output.opaque_mesh);
        let cutout = mesh_to_layer(&output.cutout_mesh);
        let mut transparent = mesh_to_layer(&output.transparent_mesh);

        // Merge greedy material meshes into the appropriate layers
        for gm in &output.greedy_materials {
            if !gm.opaque_mesh.is_empty() {
                opaque.merge(&mesh_to_layer(&gm.opaque_mesh));
            }
            if !gm.transparent_mesh.is_empty() {
                transparent.merge(&mesh_to_layer(&gm.transparent_mesh));
            }
        }

        MeshOutput {
            opaque,
            cutout,
            transparent,
            atlas: output.atlas.clone(),
            animated_textures: output.animated_textures.clone(),
            bounds: output.bounds,
            chunk_coord: None,
            lod_level: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mesh_layer_merge() {
        let mut a = MeshLayer {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            colors: vec![[1.0, 1.0, 1.0, 1.0]; 3],
            indices: vec![0, 1, 2],
        };
        let b = MeshLayer {
            positions: vec![[2.0, 0.0, 0.0], [3.0, 0.0, 0.0], [2.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            colors: vec![[1.0, 1.0, 1.0, 1.0]; 3],
            indices: vec![0, 1, 2],
        };
        a.merge(&b);

        assert_eq!(a.vertex_count(), 6);
        assert_eq!(a.triangle_count(), 2);
        // Second triangle's indices should be offset by 3
        assert_eq!(a.indices, vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(a.positions[3], [2.0, 0.0, 0.0]);
    }

    #[test]
    fn test_mesh_layer_bytes_zero_alloc() {
        let layer = MeshLayer {
            positions: vec![[1.0, 2.0, 3.0]],
            normals: vec![[0.0, 1.0, 0.0]],
            uvs: vec![[0.5, 0.5]],
            colors: vec![[1.0, 0.0, 0.0, 1.0]],
            indices: vec![0],
        };

        assert_eq!(layer.positions_bytes().len(), 12); // 3 * 4 bytes
        assert_eq!(layer.normals_bytes().len(), 12);
        assert_eq!(layer.uvs_bytes().len(), 8); // 2 * 4 bytes
        assert_eq!(layer.colors_bytes().len(), 16); // 4 * 4 bytes
        assert_eq!(layer.indices_bytes().len(), 4); // 1 * 4 bytes
    }

    #[test]
    fn test_mesh_output_flatten() {
        let opaque = MeshLayer {
            positions: vec![[0.0, 0.0, 0.0]],
            normals: vec![[0.0, 1.0, 0.0]],
            uvs: vec![[0.0, 0.0]],
            colors: vec![[1.0, 1.0, 1.0, 1.0]],
            indices: vec![0],
        };
        let cutout = MeshLayer {
            positions: vec![[1.0, 0.0, 0.0]],
            normals: vec![[0.0, 1.0, 0.0]],
            uvs: vec![[1.0, 0.0]],
            colors: vec![[1.0, 1.0, 1.0, 1.0]],
            indices: vec![0],
        };
        let transparent = MeshLayer {
            positions: vec![[2.0, 0.0, 0.0]],
            normals: vec![[0.0, 1.0, 0.0]],
            uvs: vec![[0.0, 1.0]],
            colors: vec![[1.0, 1.0, 1.0, 0.5]],
            indices: vec![0],
        };

        let output = MeshOutput {
            opaque,
            cutout,
            transparent,
            atlas: TextureAtlas::empty(),
            animated_textures: Vec::new(),
            bounds: BoundingBox::new([0.0, 0.0, 0.0], [3.0, 1.0, 1.0]),
            chunk_coord: None,
            lod_level: 0,
        };

        assert_eq!(output.total_vertices(), 3);
        assert_eq!(output.total_triangles(), 0); // each layer has 1 index, not enough for a full triangle
        assert!(output.has_transparency());

        let flat = output.flatten();
        assert_eq!(flat.vertex_count(), 3);
        // indices: [0] merged with [0] (offset 1) merged with [0] (offset 2) = [0, 1, 2]
        assert_eq!(flat.indices, vec![0, 1, 2]);
    }

    #[test]
    fn test_mesh_output_is_empty() {
        let output = MeshOutput {
            opaque: MeshLayer::new(),
            cutout: MeshLayer::new(),
            transparent: MeshLayer::new(),
            atlas: TextureAtlas::empty(),
            animated_textures: Vec::new(),
            bounds: BoundingBox::new([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
            chunk_coord: None,
            lod_level: 0,
        };
        assert!(output.is_empty());
        assert!(!output.has_transparency());
    }

    #[test]
    fn test_mesh_output_to_glb() {
        use crate::mesher::geometry::{Mesh, Vertex};

        // Build a MeshOutput with a triangle in the opaque layer
        let mut mesh = Mesh::new();
        let v0 = mesh.add_vertex(Vertex::new([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0]));
        let v1 = mesh.add_vertex(Vertex::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0]));
        let v2 = mesh.add_vertex(Vertex::new([0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [0.0, 1.0]));
        mesh.add_triangle(v0, v1, v2);

        let mesher_output = crate::mesher::MesherOutput {
            opaque_mesh: mesh,
            cutout_mesh: Mesh::new(),
            transparent_mesh: Mesh::new(),
            atlas: TextureAtlas::empty(),
            bounds: BoundingBox::new([0.0, 0.0, 0.0], [1.0, 0.0, 1.0]),
            greedy_materials: Vec::new(),
            animated_textures: Vec::new(),
        };

        let output = MeshOutput::from(&mesher_output);
        let glb = output.to_glb().unwrap();
        assert!(!glb.is_empty());
        assert_eq!(&glb[0..4], b"glTF");
    }
}
