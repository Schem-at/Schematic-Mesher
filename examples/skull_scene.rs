//! Mob-head comparison scene: zombie (reference), piglin and dragon heads,
//! standing at rotation 0 and 4 and on walls facing north and east.
//!
//!   cargo run --example skull_scene -- <resource-pack.zip|client.jar> [out.glb]
//!
//! Row per head type (zombie z=2, piglin z=6, dragon z=10); columns:
//! x=1 rotation 0, x=4 rotation 4, x=7 wall facing north, x=10 wall facing east.

use schematic_mesher::{
    export_glb, load_resource_pack, Mesher, MesherConfig,
    types::{BlockPosition, BlockSource, BoundingBox, InputBlock},
};
use std::collections::HashMap;

struct Scene {
    blocks: HashMap<BlockPosition, InputBlock>,
    bounds: BoundingBox,
}

impl Scene {
    fn set(&mut self, x: i32, y: i32, z: i32, name: &str, props: &[(&str, &str)]) {
        let mut block = InputBlock::new(name);
        for &(k, v) in props {
            block.properties.insert(k.to_string(), v.to_string());
        }
        self.blocks.insert(BlockPosition::new(x, y, z), block);
        for (i, c) in [x, y, z].into_iter().enumerate() {
            self.bounds.min[i] = self.bounds.min[i].min(c as f32);
            self.bounds.max[i] = self.bounds.max[i].max(c as f32 + 1.0);
        }
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
    let args: Vec<String> = std::env::args().collect();
    let pack_path = args.get(1).map(String::as_str).unwrap_or("pack.zip");
    let out = args.get(2).map(String::as_str).unwrap_or("artifacts/skulls.glb");

    let mut s = Scene {
        blocks: HashMap::new(),
        bounds: BoundingBox::new([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
    };
    for x in 0..12 {
        for z in 0..13 {
            s.set(x, 0, z, "minecraft:stone", &[]);
        }
    }
    for (kind, z) in [("zombie", 2), ("piglin", 6), ("dragon", 10)] {
        s.set(1, 1, z, &format!("minecraft:{kind}_head"), &[("rotation", "0")]);
        s.set(4, 1, z, &format!("minecraft:{kind}_head"), &[("rotation", "4")]);
        // Facing north: the supporting wall is on the south side.
        s.set(7, 1, z + 1, "minecraft:stone", &[]);
        s.set(7, 1, z, &format!("minecraft:{kind}_wall_head"), &[("facing", "north")]);
        // Facing east: the supporting wall is on the west side.
        s.set(9, 1, z, "minecraft:stone", &[]);
        s.set(10, 1, z, &format!("minecraft:{kind}_wall_head"), &[("facing", "east")]);
    }

    let pack = load_resource_pack(pack_path)?;
    let output = Mesher::with_config(pack, MesherConfig::default()).mesh(&s)?;
    println!("{} tris, {} verts", output.total_triangles(), output.total_vertices());
    if let Some(dir) = std::path::Path::new(out).parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(out, export_glb(&output)?)?;
    println!("wrote {out}");
    Ok(())
}
