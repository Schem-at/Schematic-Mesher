//! Lighting test scene.
//!
//!   cargo run --example lighting_scene
//!
//! Exports GLB to artifacts/lighting.glb. Tests block light and sky light.

use schematic_mesher::{
    export_glb, load_resource_pack, Mesher, MesherConfig, TintProvider,
    types::{BlockPosition, BlockSource, BoundingBox, InputBlock},
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

fn build_scene(s: &mut Scene) {
    let w = 12;
    let h = 6;
    let d = 12;

    // Build cutaway room: floor, north wall, west wall, east wall (no south wall, no ceiling)
    // This lets the camera see inside the room
    for x in 0..w {
        for y in 0..h {
            for z in 0..d {
                let is_floor = y == 0;
                let is_north = z == 0;
                let is_west = x == 0;
                let is_east = x == w - 1;
                // Skip south wall (z == d-1) and ceiling (y == h-1) for cutaway
                if is_floor || is_north || is_west || is_east {
                    s.set(x, y, z, "minecraft:stone");
                }
            }
        }
    }

    // Glass windows on the north wall (z=0) for sky light entry
    for x in 3..9 {
        for y in 2..5 {
            s.set(x, y, 0, "minecraft:glass");
        }
    }

    // Interior floor: oak planks
    for x in 1..w - 1 {
        for z in 1..d - 1 {
            s.set(x, 1, z, "minecraft:oak_planks");
        }
    }

    // Light sources at various positions inside the room
    // Torch on the floor
    s.set(2, 2, 5, "minecraft:torch");

    // Glowstone in a corner
    s.set(1, 3, 1, "minecraft:glowstone");

    // Sea lantern mid-room
    s.set(6, 3, 6, "minecraft:sea_lantern");

    // Lit furnace against a wall
    s.set_with(10, 2, 5, "minecraft:furnace", &[("facing", "west"), ("lit", "true")]);

    // Lit redstone lamp
    s.set_with(5, 4, 3, "minecraft:redstone_lamp", &[("lit", "true")]);
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
        enable_block_light: true,
        enable_sky_light: true,
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
    let out = artifacts_dir.join("lighting.glb");
    fs::create_dir_all(artifacts_dir)?;

    let mut scene = Scene::new();
    build_scene(&mut scene);
    println!("{} blocks (lighting scene)", scene.blocks.len());

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
