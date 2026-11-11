//! Minecraft 1.21.11 multi-axis element rotations, meshed end to end
//! (Schem-at/Nucleation#64).

use std::collections::HashMap;

use schematic_mesher::{
    resource_pack::{BlockModel, BlockstateDefinition, ResourcePack, TextureData},
    types::{BlockPosition, BlockSource, BoundingBox, InputBlock},
    Mesher,
};

struct One(HashMap<BlockPosition, InputBlock>);

impl BlockSource for One {
    fn get_block(&self, p: BlockPosition) -> Option<&InputBlock> {
        self.0.get(&p)
    }
    fn iter_blocks(&self) -> Box<dyn Iterator<Item = (BlockPosition, &InputBlock)> + '_> {
        Box::new(self.0.iter().map(|(p, b)| (*p, b)))
    }
    fn bounds(&self) -> BoundingBox {
        BoundingBox::new([0.0; 3], [1.0; 3])
    }
}

fn mesh_model(rotation: &str) -> Vec<([f32; 3], [f32; 3])> {
    let mut pack = ResourcePack::new();
    pack.add_texture("minecraft", "block/test", TextureData::new(16, 16, vec![200; 16 * 16 * 4]));
    let model = format!(
        r##"{{"textures": {{"all": "minecraft:block/test"}},
            "elements": [{{"from": [0, 0, 0], "to": [16, 8, 16], "rotation": {rotation},
              "faces": {{"up": {{"texture": "#all"}}, "down": {{"texture": "#all"}},
                         "north": {{"texture": "#all"}}, "south": {{"texture": "#all"}},
                         "east": {{"texture": "#all"}}, "west": {{"texture": "#all"}}}}}}]}}"##
    );
    let model: BlockModel = serde_json::from_str(&model).expect("model parses");
    pack.add_model("minecraft", "block/test", model);
    let bs: BlockstateDefinition =
        serde_json::from_str(r#"{"variants": {"": {"model": "minecraft:block/test"}}}"#).unwrap();
    pack.add_blockstate("minecraft", "test", bs);

    let source = One(HashMap::from([(BlockPosition::new(0, 0, 0), InputBlock::new("minecraft:test"))]));
    let out = Mesher::new(pack).mesh(&source).expect("mesh");
    let mut v = Vec::new();
    for layer in [&out.opaque_mesh, &out.cutout_mesh, &out.transparent_mesh] {
        for (p, n) in layer.positions.iter().zip(&layer.normals) {
            v.push((*p, *n));
        }
    }
    assert!(!v.is_empty(), "the rotated element must not be dropped");
    v
}

fn y_range(v: &[([f32; 3], [f32; 3])]) -> (f32, f32) {
    v.iter().fold((f32::MAX, f32::MIN), |(lo, hi), (p, _)| (lo.min(p[1]), hi.max(p[1])))
}

#[test]
fn euler_x_180_turns_a_bottom_slab_into_a_top_slab() {
    // Output is block-centred: the block spans -0.5..0.5.
    let (lo, hi) = y_range(&mesh_model(r#"{"origin": [8, 8, 8], "axis": "x", "angle": 0}"#));
    assert!((lo + 0.5).abs() < 1e-4 && hi.abs() < 1e-4, "unrotated slab: {lo}..{hi}");

    let flipped = mesh_model(r#"{"origin": [8, 8, 8], "x": 180}"#);
    let (lo, hi) = y_range(&flipped);
    assert!(lo.abs() < 1e-4 && (hi - 0.5).abs() < 1e-4, "flipped slab: {lo}..{hi}");

    // The old top face now sits at the bottom of the slab and faces down.
    let bottom: Vec<_> = flipped.iter().filter(|(p, _)| p[1].abs() < 1e-4).collect();
    assert!(!bottom.is_empty());
    assert!(
        bottom.iter().any(|(_, n)| n[1] < -0.99),
        "a face at y=0 should face down after the flip"
    );
}

#[test]
fn euler_rotation_matches_the_equivalent_single_axis_one() {
    let single = mesh_model(r#"{"origin": [8, 8, 8], "axis": "y", "angle": 22.5}"#);
    let euler = mesh_model(r#"{"origin": [8, 8, 8], "y": 22.5}"#);
    assert_eq!(single.len(), euler.len());
    for ((a, _), (b, _)) in single.iter().zip(&euler) {
        for i in 0..3 {
            assert!((a[i] - b[i]).abs() < 1e-5, "{a:?} vs {b:?}");
        }
    }
}
