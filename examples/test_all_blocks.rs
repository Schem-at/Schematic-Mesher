//! Test ALL blocks from the resource pack to identify rendering issues.
//! Automatically extracts block names from the pack and tests each one.

use schematic_mesher::{
    export_glb, load_resource_pack, Mesher, MesherConfig, TintProvider,
    types::{BlockPosition, BlockSource, BoundingBox, InputBlock},
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// A simple block source implementation for testing
struct TestBlockSource {
    blocks: HashMap<BlockPosition, InputBlock>,
    bounds: BoundingBox,
}

impl TestBlockSource {
    fn new() -> Self {
        Self {
            blocks: HashMap::new(),
            bounds: BoundingBox::new([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
        }
    }

    fn add_block(&mut self, x: i32, y: i32, z: i32, block: InputBlock) {
        self.blocks.insert(BlockPosition::new(x, y, z), block);
        self.bounds.min[0] = self.bounds.min[0].min(x as f32);
        self.bounds.min[1] = self.bounds.min[1].min(y as f32);
        self.bounds.min[2] = self.bounds.min[2].min(z as f32);
        self.bounds.max[0] = self.bounds.max[0].max(x as f32 + 1.0);
        self.bounds.max[1] = self.bounds.max[1].max(y as f32 + 1.0);
        self.bounds.max[2] = self.bounds.max[2].max(z as f32 + 1.0);
    }

    fn clear(&mut self) {
        self.blocks.clear();
        self.bounds = BoundingBox::new([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
    }
}

impl BlockSource for TestBlockSource {
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

#[derive(Debug)]
struct TestResult {
    block_name: String,
    success: bool,
    error: Option<String>,
    vertices: usize,
    triangles: usize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let artifacts_dir = Path::new("artifacts");
    fs::create_dir_all(artifacts_dir)?;

    println!("=== Comprehensive Block Test Suite ===\n");

    // Load resource pack
    println!("1. Loading resource pack...");
    let pack = load_resource_pack("pack.zip")?;

    // Extract all block names from the pack
    let mut all_blocks: Vec<String> = Vec::new();
    for (namespace, blocks) in &pack.blockstates {
        for block_id in blocks.keys() {
            all_blocks.push(format!("{}:{}", namespace, block_id));
        }
    }
    all_blocks.sort();

    println!("   Found {} blockstates in resource pack\n", all_blocks.len());

    // Test each block individually
    println!("2. Testing each block...");

    let config = MesherConfig {
        cull_hidden_faces: true,
        cull_occluded_blocks: true,
        greedy_meshing: false,
        atlas_max_size: 256,
        atlas_padding: 1,
        include_air: false,
        tint_provider: TintProvider::new(),
        ambient_occlusion: false, // Disable for individual tests
        ao_intensity: 0.5,
        enable_block_light: false,
        enable_sky_light: false,
        sky_light_level: 15,
    };

    let mesher = Mesher::with_config(pack, config);
    let mut source = TestBlockSource::new();

    let mut results: Vec<TestResult> = Vec::new();
    let mut successful_blocks: Vec<(String, InputBlock)> = Vec::new();

    for (i, block_name) in all_blocks.iter().enumerate() {
        // Progress indicator every 100 blocks
        if i % 100 == 0 {
            println!("   Testing block {}/{}...", i, all_blocks.len());
        }

        source.clear();
        source.add_block(0, 0, 0, InputBlock::new(block_name));

        let result = mesher.mesh(&source);

        match result {
            Ok(output) => {
                let verts = output.total_vertices();
                let tris = output.total_triangles();

                results.push(TestResult {
                    block_name: block_name.clone(),
                    success: true,
                    error: None,
                    vertices: verts,
                    triangles: tris,
                });

                // Track successful blocks for combined mesh
                successful_blocks.push((block_name.clone(), InputBlock::new(block_name)));
            }
            Err(e) => {
                results.push(TestResult {
                    block_name: block_name.clone(),
                    success: false,
                    error: Some(format!("{}", e)),
                    vertices: 0,
                    triangles: 0,
                });
            }
        }
    }

    // Categorize results
    let successful: Vec<_> = results.iter().filter(|r| r.success).collect();
    let failed: Vec<_> = results.iter().filter(|r| !r.success).collect();
    let empty_mesh: Vec<_> = successful.iter().filter(|r| r.vertices == 0).collect();
    let has_geometry: Vec<_> = successful.iter().filter(|r| r.vertices > 0).collect();

    println!("\n3. Results Summary:");
    println!("   Total blocks: {}", results.len());
    println!("   Successful: {} ({} with geometry, {} empty)",
             successful.len(), has_geometry.len(), empty_mesh.len());
    println!("   Failed: {}", failed.len());

    // Generate detailed report
    let mut report = String::new();
    report.push_str("Comprehensive Block Test Report\n");
    report.push_str("================================\n\n");
    report.push_str(&format!("Total blockstates tested: {}\n", results.len()));
    report.push_str(&format!("Successful: {} ({} with geometry, {} empty/air-like)\n",
                             successful.len(), has_geometry.len(), empty_mesh.len()));
    report.push_str(&format!("Failed: {}\n\n", failed.len()));

    // List failures by error type
    report.push_str("=== FAILURES ===\n\n");

    // Group failures by error message
    let mut error_groups: HashMap<String, Vec<&TestResult>> = HashMap::new();
    for result in &failed {
        let error = result.error.as_ref().unwrap();
        // Extract error type (first part before details)
        let error_type = if error.contains(':') {
            error.split(':').next().unwrap_or(error)
        } else {
            error.as_str()
        };
        error_groups.entry(error_type.to_string()).or_default().push(result);
    }

    for (error_type, blocks) in &error_groups {
        report.push_str(&format!("\n--- {} ({} blocks) ---\n", error_type, blocks.len()));
        for result in blocks {
            report.push_str(&format!("  {}\n", result.block_name));
            if let Some(err) = &result.error {
                report.push_str(&format!("    Error: {}\n", err));
            }
        }
    }

    // List blocks with geometry
    report.push_str("\n\n=== SUCCESSFUL BLOCKS WITH GEOMETRY ===\n\n");
    for result in &has_geometry {
        report.push_str(&format!("{}: {} vertices, {} triangles\n",
                                 result.block_name, result.vertices, result.triangles));
    }

    // List empty blocks (might be air-like or need properties)
    report.push_str("\n\n=== BLOCKS WITH NO GEOMETRY (may need properties) ===\n\n");
    for result in &empty_mesh {
        report.push_str(&format!("{}\n", result.block_name));
    }

    fs::write(artifacts_dir.join("block_test_report.txt"), &report)?;
    println!("\n4. Report written to artifacts/block_test_report.txt");

    // Now create a combined mesh with all successful blocks that have geometry
    println!("\n5. Creating combined mesh of successful blocks...");

    // Reload pack for fresh mesher
    let pack2 = load_resource_pack("pack.zip")?;
    let config2 = MesherConfig {
        cull_hidden_faces: true,
        cull_occluded_blocks: true,
        greedy_meshing: false,
        atlas_max_size: 4096,
        atlas_padding: 1,
        include_air: false,
        tint_provider: TintProvider::new(),
        ambient_occlusion: true,
        ao_intensity: 0.5,
        enable_block_light: false,
        enable_sky_light: false,
        sky_light_level: 15,
    };
    let mesher2 = Mesher::with_config(pack2, config2);

    // Only include blocks that had geometry
    let blocks_with_geometry: Vec<_> = successful_blocks.iter()
        .filter(|(name, _)| has_geometry.iter().any(|r| &r.block_name == name))
        .collect();

    let mut combined_source = TestBlockSource::new();
    let grid_width = 25;
    let spacing = 2;

    // Add floor
    let floor_size = ((blocks_with_geometry.len() / grid_width) + 2) * spacing + 2;
    for x in 0..floor_size as i32 {
        for z in 0..floor_size as i32 {
            combined_source.add_block(x, 0, z, InputBlock::new("minecraft:stone"));
        }
    }

    // Place blocks
    for (i, (name, block)) in blocks_with_geometry.iter().enumerate() {
        let x = ((i % grid_width) * spacing + 1) as i32;
        let z = ((i / grid_width) * spacing + 1) as i32;
        combined_source.add_block(x, 1, z, (*block).clone());
    }

    let output = mesher2.mesh(&combined_source)?;

    println!("   Generated mesh: {} vertices, {} triangles",
             output.total_vertices(), output.total_triangles());
    println!("   Atlas: {}x{} with {} regions",
             output.atlas.width, output.atlas.height, output.atlas.regions.len());

    // Export combined mesh
    let glb = export_glb(&output)?;
    fs::write(artifacts_dir.join("all_blocks.glb"), &glb)?;
    println!("   Exported to artifacts/all_blocks.glb ({} bytes)", glb.len());

    // Write block position map
    let mut position_map = String::new();
    position_map.push_str("Block Position Map\n");
    position_map.push_str("==================\n\n");
    position_map.push_str(&format!("Grid: {} blocks wide, {} spacing\n\n", grid_width, spacing));

    for (i, (name, _)) in blocks_with_geometry.iter().enumerate() {
        let x = (i % grid_width) * spacing + 1;
        let z = (i / grid_width) * spacing + 1;
        position_map.push_str(&format!("({:2},{:2}): {}\n", x, z, name));
    }

    fs::write(artifacts_dir.join("block_positions.txt"), &position_map)?;

    println!("\n=== Done! ===");
    println!("Check artifacts/block_test_report.txt for full results");
    println!("View artifacts/all_blocks.glb to inspect all working blocks\n");

    Ok(())
}
