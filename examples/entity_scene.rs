//! Block entity test scene.
//!
//!   cargo run --example entity_scene
//!
//! Exports GLB to artifacts/entity.glb. Tests chests, beds, bells, signs, skulls.

use schematic_mesher::{
    export_glb, load_resource_pack, Mesher, MesherConfig, TintProvider,
    types::{BlockPosition, BlockSource, BoundingBox, InputBlock},
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

fn build_scene(s: &mut Scene) {
    // Stone floor 24x24
    for x in 0..24 {
        for z in 0..24 {
            s.set(x, 0, z, "minecraft:stone");
        }
    }

    // === Section 1 (z=1-2): Chests in 4 facings ===
    for (i, facing) in ["north", "south", "east", "west"].iter().enumerate() {
        let x = i as i32 * 3 + 1;
        s.set_with(x, 1, 1, "minecraft:chest", &[("facing", facing)]);
    }

    // Trapped chest
    s.set_with(13, 1, 1, "minecraft:trapped_chest", &[("facing", "north")]);

    // Ender chest
    s.set_with(16, 1, 1, "minecraft:ender_chest", &[("facing", "north")]);

    // Double chest (left + right blocks side by side)
    s.set_with(19, 1, 1, "minecraft:chest", &[("facing", "north"), ("type", "left")]);
    s.set_with(20, 1, 1, "minecraft:chest", &[("facing", "north"), ("type", "right")]);

    // === Section 2 (z=4-5): Beds ===
    let colors = ["red", "blue", "green", "white"];
    for (i, color) in colors.iter().enumerate() {
        let x = i as i32 * 4 + 1;
        let name = format!("minecraft:{}_bed", color);
        // Head at z=4, foot at z=5 (bed faces north toward z=4)
        s.set_with(x, 1, 4, &name, &[("part", "head"), ("facing", "south")]);
        s.set_with(x, 1, 5, &name, &[("part", "foot"), ("facing", "south")]);
    }

    // === Section 3 (z=8): Bell ===
    s.set_with(1, 1, 8, "minecraft:bell", &[("facing", "north"), ("attachment", "floor")]);

    // === Section 4 (z=10-11): Signs ===
    // Standing signs
    s.set_with(1, 1, 10, "minecraft:oak_sign", &[("rotation", "0")]);
    s.set_with(4, 1, 10, "minecraft:birch_sign", &[("rotation", "4")]);
    s.set_with(7, 1, 10, "minecraft:spruce_sign", &[("rotation", "8")]);

    // Wall signs (on stone wall)
    s.set(1, 2, 12, "minecraft:stone");
    s.set_with(1, 2, 11, "minecraft:wall_oak_sign", &[("facing", "south")]);
    s.set(5, 2, 12, "minecraft:stone");
    s.set_with(5, 2, 11, "minecraft:wall_birch_sign", &[("facing", "south")]);

    // === Section 5 (z=14): Skulls ===
    s.set_with(1, 1, 14, "minecraft:skeleton_skull", &[("rotation", "0")]);
    s.set_with(4, 1, 14, "minecraft:creeper_head", &[("rotation", "8")]);
    s.set_with(7, 1, 14, "minecraft:zombie_head", &[("rotation", "4")]);

    // Wall skulls (on stone wall)
    s.set(1, 2, 16, "minecraft:stone");
    s.set_with(1, 2, 15, "minecraft:skeleton_wall_skull", &[("facing", "south")]);
    s.set(4, 2, 16, "minecraft:stone");
    s.set_with(4, 2, 15, "minecraft:creeper_wall_head", &[("facing", "south")]);

    // === Section 6 (z=18): Mobs ===
    s.set_with(1, 1, 18, "entity:zombie", &[("facing", "south")]);
    s.set_with(4, 1, 18, "entity:skeleton", &[("facing", "south")]);
    s.set_with(7, 1, 18, "entity:creeper", &[("facing", "south")]);
    s.set_with(10, 1, 18, "entity:pig", &[("facing", "south")]);

    // === Reference blocks nearby ===
    s.set(22, 1, 1, "minecraft:oak_planks");
    s.set(22, 1, 4, "minecraft:diamond_block");
    s.set(22, 1, 8, "minecraft:iron_block");
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
    let out = artifacts_dir.join("entity.glb");
    fs::create_dir_all(artifacts_dir)?;

    let mut scene = Scene::new();
    build_scene(&mut scene);
    println!("{} blocks (entity scene)", scene.blocks.len());

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
