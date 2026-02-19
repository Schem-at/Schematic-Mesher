//! Liquid block test scene.
//!
//!   cargo run --example liquid_scene
//!
//! Exports GLB to artifacts/liquid.glb. Tests water/lava geometry, heights, transparency.

use schematic_mesher::{
    export_glb, load_resource_pack, Mesher, MesherConfig, TintProvider,
    types::{BlockPosition, BlockSource, BoundingBox, InputBlock},
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

fn build_scene(s: &mut Scene) {
    // Stone floor 16x16
    for x in 0..16 {
        for z in 0..16 {
            s.set(x, 0, z, "minecraft:stone");
        }
    }

    // === Section 1 (z=1): Water levels 0-7 showing height progression ===
    for level in 0..8u8 {
        let x = level as i32;
        s.set_with(x, 1, 1, "minecraft:water", &[("level", &level.to_string())]);
    }

    // === Section 2 (z=3): Source blocks in a row for uniform height ===
    for x in 0..4 {
        s.set_with(x, 1, 3, "minecraft:water", &[("level", "0")]);
    }

    // === Section 3 (z=5): Lava pool (source blocks) ===
    for x in 0..4 {
        for z in 5..7 {
            s.set_with(x, 1, z, "minecraft:lava", &[("level", "0")]);
        }
    }

    // === Section 4 (z=8): Water flowing down stone steps ===
    for step in 0..4 {
        let x = step * 2;
        let y = 4 - step;
        s.set(x, y, 8, "minecraft:stone");
        s.set(x + 1, y, 8, "minecraft:stone");
        let level = (step * 2).min(7) as u8;
        s.set_with(x, y + 1, 8, "minecraft:water", &[("level", &level.to_string())]);
        s.set_with(x + 1, y + 1, 8, "minecraft:water", &[("level", &(level + 1).min(7).to_string())]);
    }

    // === Section 5 (z=10): Water adjacent to glass (transparency interaction) ===
    s.set(0, 1, 10, "minecraft:glass");
    s.set_with(1, 1, 10, "minecraft:water", &[("level", "0")]);
    s.set_with(2, 1, 10, "minecraft:water", &[("level", "0")]);
    s.set(3, 1, 10, "minecraft:glass");

    // === Section 6 (z=12): Source block surrounded by flowing water ===
    s.set_with(6, 1, 12, "minecraft:water", &[("level", "0")]); // source center
    s.set_with(5, 1, 12, "minecraft:water", &[("level", "2")]);
    s.set_with(7, 1, 12, "minecraft:water", &[("level", "2")]);
    s.set_with(6, 1, 11, "minecraft:water", &[("level", "2")]);
    s.set_with(6, 1, 13, "minecraft:water", &[("level", "2")]);

    // === Section 7 (z=14): Stacked water (full height when same fluid above) ===
    s.set_with(10, 1, 14, "minecraft:water", &[("level", "0")]);
    s.set_with(10, 2, 14, "minecraft:water", &[("level", "0")]);
    s.set_with(10, 3, 14, "minecraft:water", &[("level", "0")]);
    // Comparison: single source next to it
    s.set_with(12, 1, 14, "minecraft:water", &[("level", "0")]);
}

fn config() -> MesherConfig {
    MesherConfig {
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
        pre_built_atlas: None,
    }
}

struct Scene {
    blocks: HashMap<BlockPosition, InputBlock>,
    bounds: BoundingBox,
}

impl Scene {
    fn new() -> Self {
        Self {
            blocks: HashMap::new(),
            bounds: BoundingBox::new([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
        }
    }

    fn set(&mut self, x: i32, y: i32, z: i32, name: &str) {
        self.blocks.insert(BlockPosition::new(x, y, z), InputBlock::new(name));
        self.grow(x, y, z);
    }

    fn set_with(&mut self, x: i32, y: i32, z: i32, name: &str, props: &[(&str, &str)]) {
        let mut block = InputBlock::new(name);
        for &(k, v) in props {
            block.properties.insert(k.to_string(), v.to_string());
        }
        self.blocks.insert(BlockPosition::new(x, y, z), block);
        self.grow(x, y, z);
    }

    fn grow(&mut self, x: i32, y: i32, z: i32) {
        self.bounds.min[0] = self.bounds.min[0].min(x as f32);
        self.bounds.min[1] = self.bounds.min[1].min(y as f32);
        self.bounds.min[2] = self.bounds.min[2].min(z as f32);
        self.bounds.max[0] = self.bounds.max[0].max(x as f32 + 1.0);
        self.bounds.max[1] = self.bounds.max[1].max(y as f32 + 1.0);
        self.bounds.max[2] = self.bounds.max[2].max(z as f32 + 1.0);
    }
}

impl BlockSource for Scene {
    fn get_block(&self, pos: BlockPosition) -> Option<&InputBlock> { self.blocks.get(&pos) }
    fn iter_blocks(&self) -> Box<dyn Iterator<Item = (BlockPosition, &InputBlock)> + '_> {
        Box::new(self.blocks.iter().map(|(p, b)| (*p, b)))
    }
    fn bounds(&self) -> BoundingBox { self.bounds }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let artifacts_dir = Path::new("artifacts");
    let out = artifacts_dir.join("liquid.glb");
    fs::create_dir_all(artifacts_dir)?;

    let mut scene = Scene::new();
    build_scene(&mut scene);
    println!("{} blocks (liquid scene)", scene.blocks.len());

    let pack = load_resource_pack("pack.zip")?;
    let mesher = Mesher::with_config(pack, config());
    let output = mesher.mesh(&scene)?;

    println!(
        "{} tris, {} verts, atlas {}x{}, {} greedy mats",
        output.total_triangles(),
        output.total_vertices(),
        output.atlas.width,
        output.atlas.height,
        output.greedy_materials.len(),
    );

    let glb = export_glb(&output)?;
    fs::write(&out, &glb)?;
    println!("wrote {}", out.display());

    Ok(())
}
