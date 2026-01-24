//! Example: Mesh a schematic and export to GLB
//!
//! This example demonstrates the full pipeline:
//! 1. Load a resource pack
//! 2. Create a schematic using Nucleation
//! 3. Generate a mesh
//! 4. Export to GLB and save artifacts

use nucleation::UniversalSchematic;
use schematic_mesher::{
    load_resource_pack, Mesher, MesherConfig, TintProvider,
    export_glb, export_obj,
    types::{BlockPosition, BoundingBox, InputBlock, BlockSource},
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// A simple block source implementation for testing
struct SimpleBlockSource {
    blocks: HashMap<BlockPosition, InputBlock>,
    bounds: BoundingBox,
}

impl SimpleBlockSource {
    fn new() -> Self {
        Self {
            blocks: HashMap::new(),
            bounds: BoundingBox::new([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
        }
    }

    fn add_block(&mut self, x: i32, y: i32, z: i32, name: &str) {
        self.blocks.insert(
            BlockPosition::new(x, y, z),
            InputBlock::new(name),
        );
        // Update bounds
        self.bounds.min[0] = self.bounds.min[0].min(x as f32);
        self.bounds.min[1] = self.bounds.min[1].min(y as f32);
        self.bounds.min[2] = self.bounds.min[2].min(z as f32);
        self.bounds.max[0] = self.bounds.max[0].max(x as f32 + 1.0);
        self.bounds.max[1] = self.bounds.max[1].max(y as f32 + 1.0);
        self.bounds.max[2] = self.bounds.max[2].max(z as f32 + 1.0);
    }

    fn add_block_with_props(&mut self, x: i32, y: i32, z: i32, name: &str, props: Vec<(&str, &str)>) {
        let mut block = InputBlock::new(name);
        for (k, v) in props {
            block.properties.insert(k.to_string(), v.to_string());
        }
        self.blocks.insert(BlockPosition::new(x, y, z), block);
        // Update bounds
        self.bounds.min[0] = self.bounds.min[0].min(x as f32);
        self.bounds.min[1] = self.bounds.min[1].min(y as f32);
        self.bounds.min[2] = self.bounds.min[2].min(z as f32);
        self.bounds.max[0] = self.bounds.max[0].max(x as f32 + 1.0);
        self.bounds.max[1] = self.bounds.max[1].max(y as f32 + 1.0);
        self.bounds.max[2] = self.bounds.max[2].max(z as f32 + 1.0);
    }
}

impl BlockSource for SimpleBlockSource {
    fn get_block(&self, pos: BlockPosition) -> Option<&InputBlock> {
        self.blocks.get(&pos)
    }

    fn iter_blocks(&self) -> Box<dyn Iterator<Item = (BlockPosition, &InputBlock)> + '_> {
        Box::new(self.blocks.iter().map(|(p, b)| (*p, b)))
    }

    fn bounds(&self) -> BoundingBox {
        self.bounds
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let artifacts_dir = Path::new("artifacts");
    fs::create_dir_all(artifacts_dir)?;

    println!("=== Schematic Mesher Example ===\n");

    // Step 1: Load the resource pack
    println!("1. Loading resource pack...");
    let pack_path = "pack.zip";

    let pack = match load_resource_pack(pack_path) {
        Ok(p) => {
            println!("   Loaded resource pack from {}", pack_path);
            println!("   - Blockstates: {} namespaces", p.blockstates.len());
            for (ns, blocks) in &p.blockstates {
                println!("     - {}: {} blockstates", ns, blocks.len());
            }
            println!("   - Models: {} namespaces", p.models.len());
            for (ns, models) in &p.models {
                println!("     - {}: {} models", ns, models.len());
            }
            println!("   - Textures: {} namespaces", p.textures.len());
            for (ns, textures) in &p.textures {
                println!("     - {}: {} textures", ns, textures.len());
            }
            p
        }
        Err(e) => {
            println!("   Failed to load resource pack: {}", e);
            println!("   Creating empty pack for demo...");
            schematic_mesher::resource_pack::ResourcePack::new()
        }
    };

    // Save pack info to artifacts
    let pack_info = format!(
        "Resource Pack Info\n==================\n\nBlockstates:\n{}\n\nModels:\n{}\n\nTextures:\n{}\n",
        pack.blockstates.iter()
            .flat_map(|(ns, blocks)| blocks.keys().take(20).map(move |k| format!("  {}:{}", ns, k)))
            .collect::<Vec<_>>()
            .join("\n"),
        pack.models.iter()
            .flat_map(|(ns, models)| models.keys().take(20).map(move |k| format!("  {}:{}", ns, k)))
            .collect::<Vec<_>>()
            .join("\n"),
        pack.textures.iter()
            .flat_map(|(ns, textures)| textures.keys().take(20).map(move |k| format!("  {}:{}", ns, k)))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    fs::write(artifacts_dir.join("01_pack_info.txt"), pack_info)?;
    println!("   Saved pack info to artifacts/01_pack_info.txt\n");

    // Step 2: Create a test schematic
    println!("2. Creating test schematic...");
    let mut source = SimpleBlockSource::new();

    // Test scene for UV orientation and occlusion fixes
    // 7x7 floor
    for x in 0..7 {
        for z in 0..7 {
            source.add_block(x, 0, z, "minecraft:stone");
        }
    }

    // TNT blocks - test that text is right-side up
    source.add_block(1, 1, 1, "minecraft:tnt");
    source.add_block(2, 1, 1, "minecraft:tnt");
    source.add_block(3, 1, 1, "minecraft:tnt");

    // Grass blocks with flowers on top - test that grass top isn't culled
    source.add_block(1, 1, 3, "minecraft:grass_block");
    source.add_block(1, 2, 3, "minecraft:poppy");  // Flower shouldn't occlude grass top

    source.add_block(2, 1, 3, "minecraft:grass_block");
    source.add_block(2, 2, 3, "minecraft:dandelion");  // Another flower

    source.add_block(3, 1, 3, "minecraft:grass_block");
    source.add_block(3, 2, 3, "minecraft:blue_orchid");

    // Bookshelf - test text orientation
    source.add_block(5, 1, 1, "minecraft:bookshelf");
    source.add_block(5, 2, 1, "minecraft:bookshelf");
    source.add_block(5, 1, 2, "minecraft:bookshelf");

    // Furnace - test that the face shows correctly
    source.add_block_with_props(5, 1, 3, "minecraft:furnace", vec![("facing", "south")]);
    source.add_block_with_props(5, 1, 4, "minecraft:furnace", vec![("facing", "west")]);
    source.add_block_with_props(5, 1, 5, "minecraft:furnace", vec![("facing", "north")]);

    // Log pillar with explicit axis=y (vertical orientation)
    source.add_block_with_props(1, 1, 5, "minecraft:oak_log", vec![("axis", "y")]);
    source.add_block_with_props(1, 2, 5, "minecraft:oak_log", vec![("axis", "y")]);
    source.add_block_with_props(1, 3, 5, "minecraft:oak_log", vec![("axis", "y")]);

    // Torch on wall - test non-solid block
    source.add_block(3, 1, 5, "minecraft:stone");
    source.add_block(3, 2, 5, "minecraft:torch");  // Torch on top of stone

    println!("   Created schematic with {} blocks", source.blocks.len());
    println!("   Bounds: {:?}\n", source.bounds);

    // Save schematic info
    let schematic_info = format!(
        "Test Schematic\n==============\n\nBlocks ({} total):\n{}\n\nBounds: {:?}\n",
        source.blocks.len(),
        source.blocks.iter()
            .map(|(p, b)| format!("  ({}, {}, {}): {}", p.x, p.y, p.z, b.name))
            .collect::<Vec<_>>()
            .join("\n"),
        source.bounds
    );
    fs::write(artifacts_dir.join("02_schematic_info.txt"), schematic_info)?;
    println!("   Saved schematic info to artifacts/02_schematic_info.txt\n");

    // Step 3: Create mesher and generate mesh
    println!("3. Generating mesh...");
    let config = MesherConfig {
        cull_hidden_faces: true,
        atlas_max_size: 4096,
        atlas_padding: 1,
        include_air: false,
        tint_provider: TintProvider::new(),
        ambient_occlusion: true,
        ao_intensity: 0.6, // Increased for visibility
    };

    let mesher = Mesher::with_config(pack, config);

    let output = match mesher.mesh(&source) {
        Ok(o) => {
            println!("   Generated mesh successfully!");
            println!("   - Vertices: {} (opaque: {}, transparent: {})",
                o.total_vertices(), o.opaque_mesh.vertex_count(), o.transparent_mesh.vertex_count());
            println!("   - Triangles: {} (opaque: {}, transparent: {})",
                o.total_triangles(), o.opaque_mesh.triangle_count(), o.transparent_mesh.triangle_count());
            println!("   - Atlas size: {}x{}", o.atlas.width, o.atlas.height);
            println!("   - Atlas regions: {}", o.atlas.regions.len());
            o
        }
        Err(e) => {
            println!("   Mesh generation failed: {}", e);
            println!("   Creating fallback mesh...");

            // Create a simple cube mesh as fallback
            let mut mesh = schematic_mesher::mesher::geometry::Mesh::new();

            // Simple cube vertices
            let positions = [
                // Front face
                [-0.5, -0.5,  0.5], [ 0.5, -0.5,  0.5], [ 0.5,  0.5,  0.5], [-0.5,  0.5,  0.5],
                // Back face
                [ 0.5, -0.5, -0.5], [-0.5, -0.5, -0.5], [-0.5,  0.5, -0.5], [ 0.5,  0.5, -0.5],
                // Top face
                [-0.5,  0.5,  0.5], [ 0.5,  0.5,  0.5], [ 0.5,  0.5, -0.5], [-0.5,  0.5, -0.5],
                // Bottom face
                [-0.5, -0.5, -0.5], [ 0.5, -0.5, -0.5], [ 0.5, -0.5,  0.5], [-0.5, -0.5,  0.5],
                // Right face
                [ 0.5, -0.5,  0.5], [ 0.5, -0.5, -0.5], [ 0.5,  0.5, -0.5], [ 0.5,  0.5,  0.5],
                // Left face
                [-0.5, -0.5, -0.5], [-0.5, -0.5,  0.5], [-0.5,  0.5,  0.5], [-0.5,  0.5, -0.5],
            ];

            let normals = [
                [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
                [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], [0.0, 0.0, -1.0],
                [0.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 1.0, 0.0],
                [0.0, -1.0, 0.0], [0.0, -1.0, 0.0], [0.0, -1.0, 0.0], [0.0, -1.0, 0.0],
                [1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 0.0],
                [-1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [-1.0, 0.0, 0.0],
            ];

            let uvs = [
                [0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0],
                [0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0],
                [0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0],
                [0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0],
                [0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0],
                [0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0],
            ];

            for i in 0..24 {
                mesh.add_vertex(schematic_mesher::mesher::geometry::Vertex::new(
                    positions[i],
                    normals[i],
                    uvs[i],
                ));
            }

            // Add quads (two triangles each)
            for face in 0..6 {
                let base = face * 4;
                mesh.add_quad(base, base + 1, base + 2, base + 3);
            }

            schematic_mesher::mesher::MesherOutput {
                opaque_mesh: mesh,
                transparent_mesh: schematic_mesher::mesher::geometry::Mesh::new(),
                atlas: schematic_mesher::atlas::TextureAtlas::empty(),
                bounds: BoundingBox::new([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5]),
            }
        }
    };
    println!();

    // Save mesh info
    let combined_mesh = output.mesh();
    let mesh_info = format!(
        "Mesh Info\n=========\n\nVertices: {} (opaque: {}, transparent: {})\nTriangles: {} (opaque: {}, transparent: {})\nIndices: {}\n\nAtlas:\n  Size: {}x{}\n  Regions: {}\n\nBounds: {:?}\n\nFirst 20 vertices (showing AO via color):\n{}\n\nVertex color statistics:\n{}\n",
        output.total_vertices(),
        output.opaque_mesh.vertex_count(),
        output.transparent_mesh.vertex_count(),
        output.total_triangles(),
        output.opaque_mesh.triangle_count(),
        output.transparent_mesh.triangle_count(),
        combined_mesh.indices.len(),
        output.atlas.width,
        output.atlas.height,
        output.atlas.regions.len(),
        output.bounds,
        combined_mesh.vertices.iter().take(20)
            .enumerate()
            .map(|(i, v)| format!("  {}: pos={:?} normal={:?} uv={:?} color={:?}", i, v.position, v.normal, v.uv, v.color))
            .collect::<Vec<_>>()
            .join("\n"),
        {
            // Analyze vertex color variations to check AO
            let colors: Vec<_> = combined_mesh.vertices.iter().map(|v| v.color[0]).collect();
            let min_brightness = colors.iter().cloned().fold(f32::MAX, f32::min);
            let max_brightness = colors.iter().cloned().fold(f32::MIN, f32::max);
            let unique_brightness: std::collections::HashSet<_> = colors.iter().map(|c| (c * 100.0) as i32).collect();
            format!("  Min brightness: {:.3}\n  Max brightness: {:.3}\n  Unique levels: {}", min_brightness, max_brightness, unique_brightness.len())
        }
    );
    fs::write(artifacts_dir.join("03_mesh_info.txt"), mesh_info)?;
    println!("   Saved mesh info to artifacts/03_mesh_info.txt\n");

    // Step 4: Export GLB
    println!("4. Exporting to GLB...");
    match export_glb(&output) {
        Ok(glb_data) => {
            let glb_path = artifacts_dir.join("04_output.glb");
            fs::write(&glb_path, &glb_data)?;
            println!("   Exported GLB ({} bytes) to {}", glb_data.len(), glb_path.display());
        }
        Err(e) => {
            println!("   GLB export failed: {}", e);
        }
    }
    println!();

    // Step 5: Export atlas as PNG
    println!("5. Exporting texture atlas...");
    match output.atlas.to_png() {
        Ok(png_data) => {
            let png_path = artifacts_dir.join("05_atlas.png");
            fs::write(&png_path, &png_data)?;
            println!("   Exported atlas PNG ({} bytes) to {}", png_data.len(), png_path.display());
        }
        Err(e) => {
            println!("   Atlas PNG export failed: {}", e);
        }
    }
    println!();

    // Step 6: Export OBJ (alternative format with vertex colors)
    println!("6. Exporting to OBJ...");
    match export_obj(&output, "schematic") {
        Ok((obj_data, mtl_data)) => {
            let obj_path = artifacts_dir.join("06_output.obj");
            let mtl_path = artifacts_dir.join("schematic.mtl");
            let obj_atlas_path = artifacts_dir.join("schematic_atlas.png");

            fs::write(&obj_path, &obj_data)?;
            fs::write(&mtl_path, &mtl_data)?;
            // Copy atlas for OBJ
            if let Ok(png_data) = output.atlas.to_png() {
                fs::write(&obj_atlas_path, &png_data)?;
            }

            println!("   Exported OBJ ({} bytes) to {}", obj_data.len(), obj_path.display());
            println!("   Exported MTL ({} bytes) to {}", mtl_data.len(), mtl_path.display());
            println!("   Note: OBJ includes vertex colors (AO) as RGB values");
        }
        Err(e) => {
            println!("   OBJ export failed: {}", e);
        }
    }
    println!();

    // Create a summary
    let summary = format!(
        r#"Schematic Mesher - Artifact Summary
====================================

Generated: {}

Files:
  01_pack_info.txt       - Resource pack contents
  02_schematic_info.txt  - Test schematic blocks
  03_mesh_info.txt       - Generated mesh statistics
  04_output.glb          - GLB model (view in Blender or online)
  05_atlas.png           - Texture atlas
  06_output.obj          - OBJ model (alternative format)
  schematic.mtl          - OBJ material file
  schematic_atlas.png    - OBJ texture

Mesh Statistics:
  Vertices: {}
  Triangles: {}
  Atlas: {}x{}
  Ambient Occlusion: Enabled (0.6 intensity)

To view the GLB:
  - Open https://gltf-viewer.donmccurdy.com/ and drag 04_output.glb
  - Or open in Blender: File -> Import -> glTF 2.0

To view the OBJ:
  - Open in Blender: File -> Import -> Wavefront (.obj)
  - The OBJ includes vertex colors with AO baked in
  - Many viewers (including Blender) support "v x y z r g b" format
"#,
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        output.total_vertices(),
        output.total_triangles(),
        output.atlas.width,
        output.atlas.height
    );
    fs::write(artifacts_dir.join("README.txt"), summary)?;

    println!("=== Done! ===");
    println!("Check the 'artifacts' folder for output files.");
    println!("View 04_output.glb at https://gltf-viewer.donmccurdy.com/\n");

    Ok(())
}
