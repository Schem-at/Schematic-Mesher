//! Quick debug scene — edit, run, view.
//!
//!   cargo run --example debug_scene                              # manual scene
//!   cargo run --example debug_scene -- /path/to/file.schem       # load schematic
//!
//! Exports GLB to artifacts/debug.glb, auto-reloads in browser viewer.

use schematic_mesher::{
    export_glb, load_resource_pack, Mesher, MesherConfig, TintProvider,
    types::{BlockPosition, BlockSource, BoundingBox, InputBlock},
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

// ============================================================
// === EDIT YOUR SCENE HERE (used when no file argument) ===
// ============================================================

fn build_scene(s: &mut Scene) {
    // Floor
    for x in 0..12 {
        for z in 0..8 {
            s.set(x, 0, z, "minecraft:stone");
        }
    }

    // === Row z=1: Isolated pistons (retracted) in each facing — all 5 exposed faces should show ===
    s.set_with(0, 1, 1, "minecraft:piston", &[("extended", "false"), ("facing", "north")]);
    s.set_with(2, 1, 1, "minecraft:piston", &[("extended", "false"), ("facing", "south")]);
    s.set_with(4, 1, 1, "minecraft:piston", &[("extended", "false"), ("facing", "east")]);
    s.set_with(6, 1, 1, "minecraft:piston", &[("extended", "false"), ("facing", "west")]);
    s.set_with(8, 1, 1, "minecraft:piston", &[("extended", "false"), ("facing", "up")]);
    s.set_with(10, 1, 1, "minecraft:piston", &[("extended", "false"), ("facing", "down")]);

    // === Row z=3: Isolated observers in each facing ===
    s.set_with(0, 1, 3, "minecraft:observer", &[("facing", "north"), ("powered", "false")]);
    s.set_with(2, 1, 3, "minecraft:observer", &[("facing", "south"), ("powered", "false")]);
    s.set_with(4, 1, 3, "minecraft:observer", &[("facing", "east"), ("powered", "false")]);
    s.set_with(6, 1, 3, "minecraft:observer", &[("facing", "west"), ("powered", "false")]);
    s.set_with(8, 1, 3, "minecraft:observer", &[("facing", "up"), ("powered", "false")]);
    s.set_with(10, 1, 3, "minecraft:observer", &[("facing", "down"), ("powered", "false")]);

    // === Row z=5: Piston facing stone — face between should be culled, others visible ===
    s.set_with(0, 1, 5, "minecraft:piston", &[("extended", "false"), ("facing", "east")]);
    s.set(1, 1, 5, "minecraft:stone");

    s.set(4, 1, 5, "minecraft:stone");
    s.set_with(5, 1, 5, "minecraft:observer", &[("facing", "west"), ("powered", "false")]);

    // === Row z=7: Furnaces in each facing (another directional block) ===
    s.set_with(0, 1, 7, "minecraft:furnace", &[("facing", "north"), ("lit", "false")]);
    s.set_with(2, 1, 7, "minecraft:furnace", &[("facing", "south"), ("lit", "false")]);
    s.set_with(4, 1, 7, "minecraft:furnace", &[("facing", "east"), ("lit", "false")]);
    s.set_with(6, 1, 7, "minecraft:furnace", &[("facing", "west"), ("lit", "false")]);
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
// === INFRASTRUCTURE (no need to edit below) ===
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

    /// Load blocks from a nucleation schematic file (.schem, .litematic, .schematic, .mcstructure)
    fn load_schematic(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let data = fs::read(path)?;
        let schematic = if path.ends_with(".litematic") {
            nucleation::formats::litematic::from_litematic(&data)?
        } else if path.ends_with(".mcstructure") {
            nucleation::formats::mcstructure::from_mcstructure(&data)?
        } else {
            nucleation::UniversalSchematic::from_schematic(&data)?
        };

        let mut scene = Scene::new();

        // Collect blocks from all regions
        let regions = schematic.get_all_regions();
        for (_name, region) in regions {
            for index in 0..region.volume() {
                let (x, y, z) = region.index_to_coords(index);
                if let Some(block_state) = region.get_block(x, y, z) {
                    if block_state.name != "minecraft:air" {
                        let mut block = InputBlock::new(&block_state.name);
                        for (k, v) in &block_state.properties {
                            block.properties.insert(k.clone(), v.clone());
                        }
                        scene.blocks.insert(BlockPosition::new(x, y, z), block);
                        scene.grow(x, y, z);
                    }
                }
            }
        }

        Ok(scene)
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
    let out = artifacts_dir.join("debug.glb");
    fs::create_dir_all(artifacts_dir)?;

    // Build scene — from file arg or manual
    let args: Vec<String> = std::env::args().collect();
    let scene = if args.len() > 1 {
        let path = &args[1];
        println!("loading {}", path);
        let s = Scene::load_schematic(path)?;
        println!("{} blocks from schematic", s.blocks.len());
        s
    } else {
        let mut s = Scene::new();
        build_scene(&mut s);
        println!("{} blocks (manual scene)", s.blocks.len());
        s
    };

    // Mesh
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

    // Export
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
