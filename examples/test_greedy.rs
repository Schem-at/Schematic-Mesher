//! Test greedy meshing and occlusion culling with visual output.
//!
//! Generates GLB files for comparison:
//!   artifacts/greedy_off.glb  — baseline (no greedy, no occlusion culling)
//!   artifacts/greedy_on.glb   — greedy meshing + occlusion culling enabled
//!
//! Run: cargo run --example test_greedy

use schematic_mesher::{
    export_glb, export_usdz, load_resource_pack, Mesher, MesherConfig, TintProvider,
    types::{BlockPosition, BlockSource, BoundingBox, InputBlock},
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

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

    fn add_block(&mut self, x: i32, y: i32, z: i32, name: &str) {
        self.blocks
            .insert(BlockPosition::new(x, y, z), InputBlock::new(name));
        self.bounds.min[0] = self.bounds.min[0].min(x as f32);
        self.bounds.min[1] = self.bounds.min[1].min(y as f32);
        self.bounds.min[2] = self.bounds.min[2].min(z as f32);
        self.bounds.max[0] = self.bounds.max[0].max(x as f32 + 1.0);
        self.bounds.max[1] = self.bounds.max[1].max(y as f32 + 1.0);
        self.bounds.max[2] = self.bounds.max[2].max(z as f32 + 1.0);
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

fn build_solid_cube(source: &mut TestBlockSource, size: i32, block: &str) {
    for x in 0..size {
        for y in 0..size {
            for z in 0..size {
                source.add_block(x, y, z, block);
            }
        }
    }
}

fn build_mixed_scene(source: &mut TestBlockSource) {
    // Stone floor 8x1x8
    for x in 0..8 {
        for z in 0..8 {
            source.add_block(x, 0, z, "minecraft:stone");
        }
    }
    // Dirt pillar 2x4x2
    for x in 3..5 {
        for y in 1..5 {
            for z in 3..5 {
                source.add_block(x, y, z, "minecraft:dirt");
            }
        }
    }
    // Oak log wall
    for x in 0..8 {
        for y in 1..4 {
            source.add_block(x, y, 0, "minecraft:oak_log");
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let artifacts_dir = Path::new("artifacts");
    fs::create_dir_all(artifacts_dir)?;

    println!("=== Greedy Meshing & Occlusion Culling Test ===\n");

    println!("Loading resource pack...");
    let pack = load_resource_pack("pack.zip")?;

    // --- Test 1: Solid stone cube ---
    println!("\n--- Test 1: 6x6x6 Stone Cube ---");
    {
        let mut source = TestBlockSource::new();
        build_solid_cube(&mut source, 6, "minecraft:stone");
        let total_blocks = source.blocks.len();

        // Baseline: no greedy, no occlusion culling
        let pack1 = load_resource_pack("pack.zip")?;
        let config_off = MesherConfig {
            cull_hidden_faces: true,
            cull_occluded_blocks: false,
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
            enable_particles: false,
            pre_built_atlas: None,
        };
        let mesher_off = Mesher::with_config(pack1, config_off);
        let output_off = mesher_off.mesh(&source)?;

        println!(
            "  Baseline:   {} blocks -> {} triangles, {} vertices",
            total_blocks,
            output_off.total_triangles(),
            output_off.total_vertices()
        );

        let glb_off = export_glb(&output_off)?;
        fs::write(artifacts_dir.join("cube_baseline.glb"), &glb_off)?;
        let usdz_off = export_usdz(&output_off)?;
        fs::write(artifacts_dir.join("cube_baseline.usdz"), &usdz_off)?;

        // Greedy + occlusion
        let pack2 = load_resource_pack("pack.zip")?;
        let config_on = MesherConfig {
            cull_hidden_faces: true,
            cull_occluded_blocks: true,
            greedy_meshing: true,
            atlas_max_size: 4096,
            atlas_padding: 1,
            include_air: false,
            tint_provider: TintProvider::new(),
            ambient_occlusion: true,
            ao_intensity: 0.4,
            enable_block_light: false,
            enable_sky_light: false,
            sky_light_level: 15,
            enable_particles: false,
            pre_built_atlas: None,
        };
        let mesher_on = Mesher::with_config(pack2, config_on);
        let output_on = mesher_on.mesh(&source)?;

        println!(
            "  Optimized:  {} blocks -> {} triangles, {} vertices",
            total_blocks,
            output_on.total_triangles(),
            output_on.total_vertices()
        );
        println!(
            "  Greedy materials: {} (separate tiled textures)",
            output_on.greedy_materials.len()
        );
        for gm in &output_on.greedy_materials {
            let verts = gm.opaque_mesh.vertex_count() + gm.transparent_mesh.vertex_count();
            let tris = gm.opaque_mesh.triangle_count() + gm.transparent_mesh.triangle_count();
            println!("    {} -> {} verts, {} tris", gm.texture_path, verts, tris);
        }

        let reduction = if output_off.total_triangles() > 0 {
            (1.0 - output_on.total_triangles() as f64 / output_off.total_triangles() as f64) * 100.0
        } else {
            0.0
        };
        println!("  Reduction:  {:.1}% fewer triangles", reduction);

        let glb_on = export_glb(&output_on)?;
        fs::write(artifacts_dir.join("cube_optimized.glb"), &glb_on)?;
        let usdz_on = export_usdz(&output_on)?;
        fs::write(artifacts_dir.join("cube_optimized.usdz"), &usdz_on)?;
    }

    // --- Test 2: Mixed scene ---
    println!("\n--- Test 2: Mixed Scene (floor + pillar + wall) ---");
    {
        let mut source = TestBlockSource::new();
        build_mixed_scene(&mut source);
        let total_blocks = source.blocks.len();

        let pack1 = load_resource_pack("pack.zip")?;
        let config_off = MesherConfig {
            cull_hidden_faces: true,
            cull_occluded_blocks: false,
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
            enable_particles: false,
            pre_built_atlas: None,
        };
        let mesher_off = Mesher::with_config(pack1, config_off);
        let output_off = mesher_off.mesh(&source)?;

        println!(
            "  Baseline:   {} blocks -> {} triangles, {} vertices",
            total_blocks,
            output_off.total_triangles(),
            output_off.total_vertices()
        );

        let glb_off = export_glb(&output_off)?;
        fs::write(artifacts_dir.join("mixed_baseline.glb"), &glb_off)?;
        let usdz_off = export_usdz(&output_off)?;
        fs::write(artifacts_dir.join("mixed_baseline.usdz"), &usdz_off)?;

        let pack2 = load_resource_pack("pack.zip")?;
        let config_on = MesherConfig {
            cull_hidden_faces: true,
            cull_occluded_blocks: true,
            greedy_meshing: true,
            atlas_max_size: 4096,
            atlas_padding: 1,
            include_air: false,
            tint_provider: TintProvider::new(),
            ambient_occlusion: true,
            ao_intensity: 0.4,
            enable_block_light: false,
            enable_sky_light: false,
            sky_light_level: 15,
            enable_particles: false,
            pre_built_atlas: None,
        };
        let mesher_on = Mesher::with_config(pack2, config_on);
        let output_on = mesher_on.mesh(&source)?;

        println!(
            "  Optimized:  {} blocks -> {} triangles, {} vertices",
            total_blocks,
            output_on.total_triangles(),
            output_on.total_vertices()
        );
        println!(
            "  Greedy materials: {} (separate tiled textures)",
            output_on.greedy_materials.len()
        );
        for gm in &output_on.greedy_materials {
            let verts = gm.opaque_mesh.vertex_count() + gm.transparent_mesh.vertex_count();
            let tris = gm.opaque_mesh.triangle_count() + gm.transparent_mesh.triangle_count();
            println!("    {} -> {} verts, {} tris", gm.texture_path, verts, tris);
        }

        let reduction = if output_off.total_triangles() > 0 {
            (1.0 - output_on.total_triangles() as f64 / output_off.total_triangles() as f64) * 100.0
        } else {
            0.0
        };
        println!("  Reduction:  {:.1}% fewer triangles", reduction);

        let glb_on = export_glb(&output_on)?;
        fs::write(artifacts_dir.join("mixed_optimized.glb"), &glb_on)?;
        let usdz_on = export_usdz(&output_on)?;
        fs::write(artifacts_dir.join("mixed_optimized.usdz"), &usdz_on)?;
    }

    println!("\n--- Output files ---");
    println!("  artifacts/cube_baseline.glb     (no optimization)");
    println!("  artifacts/cube_baseline.usdz    (no optimization)");
    println!("  artifacts/cube_optimized.glb    (greedy + occlusion)");
    println!("  artifacts/cube_optimized.usdz   (greedy + occlusion)");
    println!("  artifacts/mixed_baseline.glb    (no optimization)");
    println!("  artifacts/mixed_baseline.usdz   (no optimization)");
    println!("  artifacts/mixed_optimized.glb   (greedy + occlusion)");
    println!("  artifacts/mixed_optimized.usdz  (greedy + occlusion)");
    println!("\nAO is baked directly into tile textures (not vertex colors).");
    println!("Open in any glTF viewer or Blender to compare!");

    Ok(())
}
