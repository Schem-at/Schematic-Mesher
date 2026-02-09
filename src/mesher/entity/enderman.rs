use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose};

/// Enderman model — texture `entity/enderman/enderman`, 64x32.
/// From EndermanModel.java (MC 1.21.4).
/// Humanoid but with very long arms (30u) and legs (30u).
pub(super) fn enderman_model() -> EntityModelDef {
    let head = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, -8.0, -4.0],
            dimensions: [8.0, 8.0, 8.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, -13.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Hat overlay (same head shape, inflated)
    let hat = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, -8.0, -4.0],
            dimensions: [8.0, 8.0, 8.0],
            tex_offset: [0, 16],
            inflate: -0.5,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, -13.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let body = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, 0.0, -2.0],
            dimensions: [8.0, 12.0, 4.0],
            tex_offset: [32, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    // Long arms (30 units tall instead of 12)
    let right_arm = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, -2.0, -1.0],
            dimensions: [2.0, 30.0, 2.0],
            tex_offset: [56, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-5.0, -12.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let left_arm = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, -2.0, -1.0],
            dimensions: [2.0, 30.0, 2.0],
            tex_offset: [56, 0],
            inflate: 0.0,
            mirror: true,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [5.0, -12.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Long legs (30 units tall instead of 12)
    let right_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, 0.0, -1.0],
            dimensions: [2.0, 30.0, 2.0],
            tex_offset: [56, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-2.0, 12.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let left_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, 0.0, -1.0],
            dimensions: [2.0, 30.0, 2.0],
            tex_offset: [56, 0],
            inflate: 0.0,
            mirror: true,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [2.0, 12.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Y-down → Y-up root wrapper
    // Enderman is ~3 blocks tall, needs higher root position
    let root = EntityPart {
        cubes: vec![],
        pose: EntityPartPose {
            position: [8.0, 44.0, 8.0],
            rotation: [std::f32::consts::PI, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![head, hat, body, right_arm, left_arm, right_leg, left_leg],
    };

    EntityModelDef {
        texture_path: "entity/enderman/enderman".to_string(),
        texture_size: [64, 32],
        parts: vec![root],
        is_opaque: false, // Has transparent hat layer
    }
}
