//! Redstone test scene — verifies rendering of redstone components.
//!
//!   cargo run --example redstone_scene
//!
//! Tests repeater/comparator glow effects, redstone torches, wire,
//! and blocks with auto-UV (composters, cauldrons, hoppers).
//! Exports GLB to artifacts/redstone.glb.

use schematic_mesher::{
    export_glb, load_resource_pack, Mesher, MesherConfig, TintProvider,
    types::{BlockPosition, BlockSource, BoundingBox, InputBlock},
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

fn build_scene(s: &mut Scene) {
    // Stone floor 20x16
    for x in 0..20 {
        for z in 0..16 {
            s.set(x, 0, z, "minecraft:stone");
        }
    }

    // =====================================================
    // Row z=1: Repeaters — all 4 facings, powered on/off
    // =====================================================
    // Off repeaters (delay 1-4)
    for delay in 1..=4i32 {
        let x = (delay - 1) * 2;
        s.set_with(x, 1, 1, "minecraft:repeater", &[
            ("facing", "north"), ("delay", &delay.to_string()),
            ("locked", "false"), ("powered", "false"),
        ]);
    }
    // On repeaters (delay 1-4)
    for delay in 1..=4i32 {
        let x = 8 + (delay - 1) * 2;
        s.set_with(x, 1, 1, "minecraft:repeater", &[
            ("facing", "north"), ("delay", &delay.to_string()),
            ("locked", "false"), ("powered", "true"),
        ]);
    }
    // Repeaters in each facing (powered)
    s.set_with(16, 1, 1, "minecraft:repeater", &[
        ("facing", "south"), ("delay", "1"), ("locked", "false"), ("powered", "true"),
    ]);
    s.set_with(18, 1, 1, "minecraft:repeater", &[
        ("facing", "east"), ("delay", "1"), ("locked", "false"), ("powered", "true"),
    ]);

    // =====================================================
    // Row z=3: Comparators — compare/subtract, on/off
    // =====================================================
    s.set_with(0, 1, 3, "minecraft:comparator", &[
        ("facing", "north"), ("mode", "compare"), ("powered", "false"),
    ]);
    s.set_with(2, 1, 3, "minecraft:comparator", &[
        ("facing", "north"), ("mode", "compare"), ("powered", "true"),
    ]);
    s.set_with(4, 1, 3, "minecraft:comparator", &[
        ("facing", "north"), ("mode", "subtract"), ("powered", "false"),
    ]);
    s.set_with(6, 1, 3, "minecraft:comparator", &[
        ("facing", "north"), ("mode", "subtract"), ("powered", "true"),
    ]);
    // Comparators in all facings
    s.set_with(8, 1, 3, "minecraft:comparator", &[
        ("facing", "south"), ("mode", "compare"), ("powered", "true"),
    ]);
    s.set_with(10, 1, 3, "minecraft:comparator", &[
        ("facing", "east"), ("mode", "compare"), ("powered", "true"),
    ]);
    s.set_with(12, 1, 3, "minecraft:comparator", &[
        ("facing", "west"), ("mode", "compare"), ("powered", "true"),
    ]);

    // =====================================================
    // Row z=5: Redstone torches — standing and wall
    // =====================================================
    s.set_with(0, 1, 5, "minecraft:redstone_torch", &[("lit", "true")]);
    s.set_with(2, 1, 5, "minecraft:redstone_torch", &[("lit", "false")]);
    // Wall torches: facing = direction torch protrudes, block is on opposite side
    s.set(4, 1, 4, "minecraft:stone"); // block to the north
    s.set_with(4, 1, 5, "minecraft:redstone_wall_torch", &[("facing", "south"), ("lit", "true")]);
    s.set(6, 1, 4, "minecraft:stone"); // block to the north
    s.set_with(6, 1, 5, "minecraft:redstone_wall_torch", &[("facing", "south"), ("lit", "false")]);
    // Wall torches in other facings
    s.set(8, 1, 6, "minecraft:stone"); // block to the south
    s.set_with(8, 1, 5, "minecraft:redstone_wall_torch", &[("facing", "north"), ("lit", "true")]);
    s.set(10, 1, 5, "minecraft:stone"); // block to the west
    s.set_with(11, 1, 5, "minecraft:redstone_wall_torch", &[("facing", "east"), ("lit", "true")]);

    // =====================================================
    // Row z=7: Redstone wire with various power levels
    // =====================================================
    for power in 0..=15 {
        s.set_with(power, 1, 7, "minecraft:redstone_wire", &[
            ("power", &power.to_string()),
            ("north", "side"), ("south", "side"),
            ("east", "none"), ("west", "none"),
        ]);
    }
    // Cross connection
    s.set_with(17, 1, 7, "minecraft:redstone_wire", &[
        ("power", "15"),
        ("north", "side"), ("south", "side"),
        ("east", "side"), ("west", "side"),
    ]);

    // =====================================================
    // Row z=9: Redstone lamps, blocks, and mechanisms
    // =====================================================
    s.set_with(0, 1, 9, "minecraft:redstone_lamp", &[("lit", "false")]);
    s.set_with(2, 1, 9, "minecraft:redstone_lamp", &[("lit", "true")]);
    s.set(4, 1, 9, "minecraft:redstone_block");
    s.set(6, 1, 9, "minecraft:target");
    // Lever on floor, on wall
    s.set_with(8, 1, 9, "minecraft:lever", &[
        ("face", "floor"), ("facing", "north"), ("powered", "false"),
    ]);
    s.set_with(10, 1, 9, "minecraft:lever", &[
        ("face", "floor"), ("facing", "north"), ("powered", "true"),
    ]);
    // Button on stone (facing = direction button protrudes, block on opposite side)
    s.set(12, 1, 8, "minecraft:stone"); // block to the north
    s.set_with(12, 1, 9, "minecraft:stone_button", &[
        ("face", "wall"), ("facing", "south"), ("powered", "false"),
    ]);
    s.set(14, 1, 8, "minecraft:stone"); // block to the north
    s.set_with(14, 1, 9, "minecraft:stone_button", &[
        ("face", "wall"), ("facing", "south"), ("powered", "true"),
    ]);

    // =====================================================
    // Row z=11: Pistons and sticky pistons
    // =====================================================
    s.set_with(0, 1, 11, "minecraft:piston", &[("extended", "false"), ("facing", "north")]);
    s.set_with(2, 1, 11, "minecraft:piston", &[("extended", "false"), ("facing", "south")]);
    s.set_with(4, 1, 11, "minecraft:piston", &[("extended", "false"), ("facing", "east")]);
    s.set_with(6, 1, 11, "minecraft:piston", &[("extended", "false"), ("facing", "up")]);
    s.set_with(8, 1, 11, "minecraft:sticky_piston", &[("extended", "false"), ("facing", "north")]);
    s.set_with(10, 1, 11, "minecraft:sticky_piston", &[("extended", "false"), ("facing", "up")]);
    // Observer
    s.set_with(12, 1, 11, "minecraft:observer", &[("facing", "north"), ("powered", "false")]);
    s.set_with(14, 1, 11, "minecraft:observer", &[("facing", "north"), ("powered", "true")]);
    // Dropper / Dispenser
    s.set_with(16, 1, 11, "minecraft:dropper", &[("facing", "north"), ("triggered", "false")]);
    s.set_with(18, 1, 11, "minecraft:dispenser", &[("facing", "north"), ("triggered", "false")]);

    // =====================================================
    // Row z=13: Auto-UV test blocks (composters, cauldrons, hoppers)
    // =====================================================
    s.set_with(0, 1, 13, "minecraft:composter", &[("level", "0")]);
    s.set_with(2, 1, 13, "minecraft:composter", &[("level", "4")]);
    s.set_with(4, 1, 13, "minecraft:composter", &[("level", "7")]);
    s.set_with(6, 1, 13, "minecraft:composter", &[("level", "8")]);
    s.set(8, 1, 13, "minecraft:cauldron");
    s.set_with(10, 1, 13, "minecraft:water_cauldron", &[("level", "1")]);
    s.set_with(12, 1, 13, "minecraft:water_cauldron", &[("level", "3")]);
    s.set(14, 1, 13, "minecraft:lava_cauldron");
    s.set_with(16, 1, 13, "minecraft:hopper", &[("enabled", "true"), ("facing", "down")]);
    s.set_with(18, 1, 13, "minecraft:hopper", &[("enabled", "true"), ("facing", "south")]);

    // =====================================================
    // Row z=15: Slabs (top/bottom) and stairs — variant test
    // =====================================================
    s.set_with(0, 1, 15, "minecraft:stone_slab", &[("type", "bottom"), ("waterlogged", "false")]);
    s.set_with(2, 1, 15, "minecraft:stone_slab", &[("type", "top"), ("waterlogged", "false")]);
    s.set_with(4, 1, 15, "minecraft:stone_slab", &[("type", "double"), ("waterlogged", "false")]);
    s.set_with(6, 1, 15, "minecraft:oak_slab", &[("type", "bottom"), ("waterlogged", "false")]);
    s.set_with(8, 1, 15, "minecraft:oak_slab", &[("type", "top"), ("waterlogged", "false")]);
    // Stairs
    s.set_with(10, 1, 15, "minecraft:stone_stairs", &[
        ("facing", "north"), ("half", "bottom"), ("shape", "straight"), ("waterlogged", "false"),
    ]);
    s.set_with(12, 1, 15, "minecraft:stone_stairs", &[
        ("facing", "north"), ("half", "top"), ("shape", "straight"), ("waterlogged", "false"),
    ]);
    s.set_with(14, 1, 15, "minecraft:oak_stairs", &[
        ("facing", "east"), ("half", "bottom"), ("shape", "straight"), ("waterlogged", "false"),
    ]);
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
        enable_particles: true,
    }
}

// ============================================================
// === INFRASTRUCTURE ===
// ============================================================

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
        self.blocks
            .insert(BlockPosition::new(x, y, z), InputBlock::new(name));
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
    let out = artifacts_dir.join("redstone.glb");
    fs::create_dir_all(artifacts_dir)?;

    let mut scene = Scene::new();
    build_scene(&mut scene);
    println!("{} blocks", scene.blocks.len());

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

    // Copy viewer HTML into artifacts if not present
    let viewer_src = Path::new("tools/viewer.html");
    let viewer_dst = artifacts_dir.join("viewer.html");
    if viewer_src.exists() && !viewer_dst.exists() {
        let _ = fs::copy(viewer_src, &viewer_dst);
    }

    Ok(())
}
