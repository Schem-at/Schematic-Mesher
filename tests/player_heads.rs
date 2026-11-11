//! Player heads: placement parity with mob heads, and skin resolution.

use std::collections::HashMap;

use schematic_mesher::{
    resource_pack::{ResourcePack, TextureData},
    types::{BlockPosition, BlockSource, BoundingBox, InputBlock},
    Mesher, MesherOutput,
};

struct Scene {
    blocks: HashMap<BlockPosition, InputBlock>,
}

impl Scene {
    fn new() -> Self {
        Self { blocks: HashMap::new() }
    }
    fn set(&mut self, x: i32, y: i32, z: i32, id: &str, props: &[(&str, &str)]) {
        let mut b = InputBlock::new(id);
        for &(k, v) in props {
            b.properties.insert(k.to_string(), v.to_string());
        }
        self.blocks.insert(BlockPosition::new(x, y, z), b);
    }
}

impl BlockSource for Scene {
    fn get_block(&self, p: BlockPosition) -> Option<&InputBlock> {
        self.blocks.get(&p)
    }
    fn iter_blocks(&self) -> Box<dyn Iterator<Item = (BlockPosition, &InputBlock)> + '_> {
        Box::new(self.blocks.iter().map(|(p, b)| (*p, b)))
    }
    fn bounds(&self) -> BoundingBox {
        BoundingBox::new([0.0, 0.0, 0.0], [16.0, 2.0, 16.0])
    }
}

fn solid(rgba: [u8; 4]) -> TextureData {
    TextureData::new(64, 64, rgba.iter().copied().cycle().take(64 * 64 * 4).collect())
}

fn pack() -> ResourcePack {
    let mut pack = ResourcePack::new();
    pack.add_texture("minecraft", "entity/zombie/zombie", solid([0, 255, 0, 255]));
    pack.add_texture("minecraft", "entity/player/wide/steve", solid([0, 0, 255, 255]));
    pack.add_texture("minecraft", "entity/player/slim/alex", solid([255, 255, 0, 255]));
    pack.add_texture("nucleation", "skins/abc", solid([255, 0, 255, 255]));
    pack
}

fn mesh(scene: &Scene) -> MesherOutput {
    Mesher::new(pack()).mesh(scene).expect("mesh")
}

/// All vertex positions near the block column at `x` (output is block-centred:
/// block `x` spans `x - 0.5 .. x + 0.5`).
fn positions_at_x(out: &MesherOutput, x: i32) -> Vec<[f32; 3]> {
    [&out.opaque_mesh, &out.cutout_mesh, &out.transparent_mesh]
        .iter()
        .flat_map(|layer| layer.positions.iter().copied())
        .filter(|p| (p[0] - x as f32).abs() <= 1.0)
        .collect()
}

fn bounds(points: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for p in points {
        for i in 0..3 {
            min[i] = min[i].min(p[i]);
            max[i] = max[i].max(p[i]);
        }
    }
    (min, max)
}

fn assert_same_relative_bounds(out: &MesherOutput, a_x: i32, b_x: i32) {
    let (a_min, a_max) = bounds(&positions_at_x(out, a_x));
    let (b_min, b_max) = bounds(&positions_at_x(out, b_x));
    let shift = (b_x - a_x) as f32;
    for i in 0..3 {
        let s = if i == 0 { shift } else { 0.0 };
        assert!((a_min[i] + s - b_min[i]).abs() < 1e-4, "min[{i}]: {a_min:?} vs {b_min:?}");
        assert!((a_max[i] + s - b_max[i]).abs() < 1e-4, "max[{i}]: {a_max:?} vs {b_max:?}");
    }
}

/// A player wall head used to float in the middle of its block, at floor
/// height; vanilla holds it against the wall, 4 px up, like every other head.
#[test]
fn player_wall_head_sits_where_a_zombie_wall_head_does() {
    for facing in ["north", "south", "east", "west"] {
        let mut scene = Scene::new();
        scene.set(0, 0, 0, "minecraft:zombie_wall_head", &[("facing", facing)]);
        scene.set(4, 0, 0, "minecraft:player_wall_head", &[("facing", facing)]);
        let out = mesh(&scene);
        assert_same_relative_bounds(&out, 0, 4);
    }
}

/// A 64x64 head texture whose face (u 8..16, v 8..16) is red and everything
/// else `rest`; the hat layer is fully transparent.
fn face_marked(rest: [u8; 4]) -> TextureData {
    let mut px = Vec::with_capacity(64 * 64 * 4);
    for y in 0..64u32 {
        for x in 0..64u32 {
            let c = if (8..16).contains(&x) && (8..16).contains(&y) {
                [255, 0, 0, 255]
            } else if (32..64).contains(&x) && y < 16 {
                [0, 0, 0, 0]
            } else {
                rest
            };
            px.extend_from_slice(&c);
        }
    }
    TextureData::new(64, 64, px)
}

/// Outward normal of the head face that shows the red face texture, for the
/// head in the column at `x`.
fn face_direction(out: &MesherOutput, x: i32) -> Vec<[i32; 3]> {
    let atlas = &out.atlas;
    let sample = |uv: [f32; 2]| -> [u8; 4] {
        let px = ((uv[0] * atlas.width as f32) as u32).min(atlas.width - 1);
        let py = ((uv[1] * atlas.height as f32) as u32).min(atlas.height - 1);
        let i = ((py * atlas.width + px) * 4) as usize;
        [atlas.pixels[i], atlas.pixels[i + 1], atlas.pixels[i + 2], atlas.pixels[i + 3]]
    };
    let mut found = Vec::new();
    for layer in [&out.opaque_mesh, &out.cutout_mesh, &out.transparent_mesh] {
        for tri in layer.indices.chunks(3) {
            let [a, b, c] = [tri[0] as usize, tri[1] as usize, tri[2] as usize];
            let centre_x = (layer.positions[a][0] + layer.positions[b][0] + layer.positions[c][0]) / 3.0;
            if (centre_x - x as f32).abs() > 0.6 {
                continue;
            }
            let uv = [
                (layer.uvs[a][0] + layer.uvs[b][0] + layer.uvs[c][0]) / 3.0,
                (layer.uvs[a][1] + layer.uvs[b][1] + layer.uvs[c][1]) / 3.0,
            ];
            if sample(uv) == [255, 0, 0, 255] {
                let n = layer.normals[a];
                found.push([n[0].round() as i32, n[1].round() as i32, n[2].round() as i32]);
            }
        }
    }
    found.sort();
    found.dedup();
    found
}

#[test]
fn player_head_faces_the_same_way_as_a_zombie_head() {
    let mut pack = ResourcePack::new();
    pack.add_texture("minecraft", "entity/zombie/zombie", face_marked([0, 255, 0, 255]));
    pack.add_texture("minecraft", "entity/player/wide/steve", face_marked([0, 0, 255, 255]));
    let cases: Vec<(&str, &str, &str)> = ["0", "4", "8", "12"]
        .iter()
        .map(|r| ("rotation", *r, "head"))
        .chain(["north", "south", "east", "west"].iter().map(|f| ("facing", *f, "wall_head")))
        .collect();
    for (key, value, kind) in cases {
        let mut scene = Scene::new();
        scene.set(0, 0, 0, &format!("minecraft:zombie_{kind}"), &[(key, value)]);
        scene.set(4, 0, 0, &format!("minecraft:player_{kind}"), &[(key, value)]);
        let out = Mesher::new(pack.clone()).mesh(&scene).expect("mesh");
        let (player, zombie) = (face_direction(&out, 4), face_direction(&out, 0));
        assert_eq!(zombie.len(), 1, "zombie {kind} {key}={value}: {zombie:?}");
        assert_eq!(player, zombie, "{kind} {key}={value}");
    }
}

fn skin_regions(out: &MesherOutput) -> Vec<&String> {
    let mut keys: Vec<&String> = out.atlas.regions.keys().filter(|k| k.starts_with("_skin/")).collect();
    keys.sort();
    keys
}

#[test]
fn heads_sharing_a_skin_share_one_atlas_entry() {
    let mut scene = Scene::new();
    for x in 0..5 {
        scene.set(x * 2, 0, 0, "minecraft:player_head", &[("skin_texture", "nucleation:skins/abc")]);
    }
    scene.set(0, 0, 4, "minecraft:player_head", &[]);
    let out = mesh(&scene);
    assert_eq!(
        skin_regions(&out),
        vec!["_skin/pack/entity/player/wide/steve", "_skin/pack/nucleation:skins/abc"]
    );
}

#[test]
fn skin_texture_property_selects_a_pack_texture() {
    let mut scene = Scene::new();
    scene.set(0, 0, 0, "minecraft:player_head", &[("skin_texture", "nucleation:skins/abc")]);
    let out = mesh(&scene);
    let magenta = out.atlas.pixels.chunks(4).any(|p| p == [255, 0, 255, 255]);
    assert!(magenta, "registered skin should be packed into the atlas");
}

#[test]
fn missing_skin_texture_falls_back_to_the_default_skin() {
    let mut scene = Scene::new();
    scene.set(0, 0, 0, "minecraft:player_head", &[("skin_texture", "nucleation:skins/missing")]);
    let out = mesh(&scene);
    assert!(out.atlas.pixels.chunks(4).any(|p| p == [0, 0, 255, 255]), "Steve");
    // Keyed by the texture actually used, so a shared atlas can look it up.
    assert_eq!(skin_regions(&out), vec!["_skin/pack/entity/player/wide/steve"]);

    // With a UUID, the fallback is that UUID's default skin.
    let mut scene = Scene::new();
    scene.set(0, 0, 0, "minecraft:player_head", &[
        ("skin_texture", "nucleation:skins/missing"),
        ("uuid", "069a79f4-44e9-4726-a5be-fca90e38aaf5"),
    ]);
    assert_eq!(skin_regions(&mesh(&scene)), vec!["_skin/pack/entity/player/slim/alex"]);
}

#[test]
fn uuid_selects_vanillas_default_skin() {
    // floorMod(UUID.hashCode(), 18) == 0 → slim Alex (checked against the JDK).
    let mut scene = Scene::new();
    scene.set(0, 0, 0, "minecraft:player_head", &[("uuid", "069a79f4-44e9-4726-a5be-fca90e38aaf5")]);
    let out = mesh(&scene);
    assert_eq!(skin_regions(&out), vec!["_skin/pack/entity/player/slim/alex"]);
    assert!(out.atlas.pixels.chunks(4).any(|p| p == [255, 255, 0, 255]), "Alex");
}

