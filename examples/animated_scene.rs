//! Animated texture test scene.
//!
//!   cargo run --example animated_scene
//!
//! Exports GLB to artifacts/animated.glb. Tests mcmeta loading and animated textures.

use schematic_mesher::{
    export_glb, load_resource_pack, Mesher, MesherConfig, TintProvider,
    types::{BlockPosition, BlockSource, BoundingBox, InputBlock},
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

fn build_scene(s: &mut Scene) {
    // Floor
    for x in 0..12 {
        for z in 0..8 {
            s.set(x, 0, z, "minecraft:stone");
        }
    }

    // Row z=1: Water and lava source blocks (animated textures)
    s.set_with(0, 1, 1, "minecraft:water", &[("level", "0")]);
    s.set_with(2, 1, 1, "minecraft:water", &[("level", "0")]);
    s.set_with(4, 1, 1, "minecraft:lava", &[("level", "0")]);
    s.set_with(6, 1, 1, "minecraft:lava", &[("level", "0")]);

    // Row z=3: Blocks known to have animated textures in vanilla
    s.set(0, 1, 3, "minecraft:sea_lantern");
    s.set(2, 1, 3, "minecraft:magma_block");
    s.set(4, 1, 3, "minecraft:prismarine");

    // Row z=5: Non-animated comparison blocks
    s.set(0, 1, 5, "minecraft:glowstone");
    s.set(2, 1, 5, "minecraft:stone");
    s.set(4, 1, 5, "minecraft:diamond_block");

    // Row z=7: Fire on netherrack
    s.set(0, 1, 7, "minecraft:netherrack");
    s.set(0, 2, 7, "minecraft:fire");
    s.set(2, 1, 7, "minecraft:soul_sand");
    s.set(2, 2, 7, "minecraft:soul_fire");
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
    let out = artifacts_dir.join("animated.glb");
    fs::create_dir_all(artifacts_dir)?;

    let mut scene = Scene::new();
    build_scene(&mut scene);
    println!("{} blocks (animated scene)", scene.blocks.len());

    let pack = load_resource_pack("pack.zip")?;

    // Print mcmeta info for animated textures
    let animated_textures = ["block/water_still", "block/water_flow", "block/lava_still",
        "block/lava_flow", "block/sea_lantern", "block/magma", "block/prismarine_anim",
        "block/fire_0", "block/fire_1", "block/soul_fire_0", "block/soul_fire_1"];
    println!("\nAnimation metadata:");
    for tex_path in &animated_textures {
        if let Some(tex) = pack.get_texture(tex_path) {
            println!("  {}: {}x{}, animated={}, frames={}{}",
                tex_path, tex.width, tex.height, tex.is_animated, tex.frame_count,
                if let Some(ref meta) = tex.animation {
                    format!(", frametime={}, interpolate={}", meta.frametime, meta.interpolate)
                } else {
                    String::new()
                });
        }
    }
    println!();

    let mesher = Mesher::with_config(pack, config());
    let output = mesher.mesh(&scene)?;

    println!(
        "{} tris, {} verts, atlas {}x{}, {} greedy mats, {} animated textures",
        output.total_triangles(),
        output.total_vertices(),
        output.atlas.width,
        output.atlas.height,
        output.greedy_materials.len(),
        output.animated_textures.len(),
    );
    for at in &output.animated_textures {
        println!("  animated: {}x{} @ atlas({},{}) frames={} frametime={}",
            at.frame_width, at.frame_height, at.atlas_x, at.atlas_y,
            at.frame_count, at.frametime);
    }

    let glb = export_glb(&output)?;
    fs::write(&out, &glb)?;
    println!("wrote {}", out.display());

    Ok(())
}
