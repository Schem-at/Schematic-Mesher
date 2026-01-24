//! Wavefront OBJ export.
//!
//! OBJ is a simple, widely-supported text-based 3D format.
//! This exports the mesh with UVs and vertex colors (as comments).

use crate::error::Result;
use crate::mesher::MesherOutput;
use std::fmt::Write;

/// Export a mesh to OBJ format.
/// Returns (obj_content, mtl_content) as strings.
/// Note: OBJ doesn't support separate transparency handling, so opaque and transparent
/// geometry are combined.
pub fn export_obj(output: &MesherOutput, name: &str) -> Result<(String, String)> {
    // Combine opaque and transparent meshes for OBJ export
    let mesh = output.mesh();
    let mut obj = String::new();
    let mut mtl = String::new();

    // OBJ header
    writeln!(obj, "# Schematic Mesher OBJ Export").unwrap();
    writeln!(obj, "# Vertices: {}", mesh.vertex_count()).unwrap();
    writeln!(obj, "# Triangles: {}", mesh.triangle_count()).unwrap();
    writeln!(obj).unwrap();

    // Reference material file
    writeln!(obj, "mtllib {}.mtl", name).unwrap();
    writeln!(obj).unwrap();

    // Object name
    writeln!(obj, "o {}", name).unwrap();
    writeln!(obj).unwrap();

    // Vertices (v x y z)
    // OBJ also supports vertex colors as: v x y z r g b
    for vertex in &mesh.vertices {
        writeln!(
            obj,
            "v {} {} {} {} {} {}",
            vertex.position[0],
            vertex.position[1],
            vertex.position[2],
            vertex.color[0],
            vertex.color[1],
            vertex.color[2]
        )
        .unwrap();
    }
    writeln!(obj).unwrap();

    // Texture coordinates (vt u v)
    for vertex in &mesh.vertices {
        writeln!(obj, "vt {} {}", vertex.uv[0], vertex.uv[1]).unwrap();
    }
    writeln!(obj).unwrap();

    // Normals (vn x y z)
    for vertex in &mesh.vertices {
        writeln!(
            obj,
            "vn {} {} {}",
            vertex.normal[0], vertex.normal[1], vertex.normal[2]
        )
        .unwrap();
    }
    writeln!(obj).unwrap();

    // Use material
    writeln!(obj, "usemtl {}_material", name).unwrap();
    writeln!(obj).unwrap();

    // Faces (f v/vt/vn v/vt/vn v/vt/vn)
    // OBJ indices are 1-based
    for i in (0..mesh.indices.len()).step_by(3) {
        let i0 = mesh.indices[i] as usize + 1;
        let i1 = mesh.indices[i + 1] as usize + 1;
        let i2 = mesh.indices[i + 2] as usize + 1;
        writeln!(
            obj,
            "f {}/{}/{} {}/{}/{} {}/{}/{}",
            i0, i0, i0, i1, i1, i1, i2, i2, i2
        )
        .unwrap();
    }

    // MTL file
    writeln!(mtl, "# Schematic Mesher Material").unwrap();
    writeln!(mtl).unwrap();
    writeln!(mtl, "newmtl {}_material", name).unwrap();
    writeln!(mtl, "Ka 1.0 1.0 1.0").unwrap(); // Ambient
    writeln!(mtl, "Kd 1.0 1.0 1.0").unwrap(); // Diffuse
    writeln!(mtl, "Ks 0.0 0.0 0.0").unwrap(); // Specular
    writeln!(mtl, "Ns 10.0").unwrap(); // Specular exponent
    writeln!(mtl, "d 1.0").unwrap(); // Opacity
    writeln!(mtl, "illum 1").unwrap(); // Illumination model (1 = diffuse only)
    writeln!(mtl, "map_Kd {}_atlas.png", name).unwrap(); // Diffuse texture

    Ok((obj, mtl))
}

/// Export mesh and atlas to OBJ format bytes for writing to files.
pub struct ObjExport {
    pub obj: String,
    pub mtl: String,
    pub texture_png: Vec<u8>,
}

impl ObjExport {
    pub fn from_output(output: &MesherOutput, name: &str) -> Result<Self> {
        let (obj, mtl) = export_obj(output, name)?;
        let texture_png = output.atlas.to_png()?;
        Ok(Self {
            obj,
            mtl,
            texture_png,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atlas::TextureAtlas;
    use crate::mesher::geometry::{Mesh, Vertex};
    use crate::types::BoundingBox;

    #[test]
    fn test_export_simple_obj() {
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

        let (obj, mtl) = export_obj(&output, "test").unwrap();

        assert!(obj.contains("v 0 0 0"));
        assert!(obj.contains("vt 0 0"));
        assert!(obj.contains("vn 0 1 0"));
        assert!(obj.contains("f 1/1/1 2/2/2 3/3/3"));
        assert!(mtl.contains("newmtl test_material"));
    }
}
