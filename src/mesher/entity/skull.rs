use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose, SkullType};

/// Skull/head model.
pub(super) fn skull_model(skull_type: SkullType) -> EntityModelDef {
    let texture_path = match skull_type {
        SkullType::Skeleton => "entity/skeleton/skeleton",
        SkullType::WitherSkeleton => "entity/skeleton/wither_skeleton",
        SkullType::Zombie => "entity/zombie/zombie",
        SkullType::Creeper => "entity/creeper/creeper",
        SkullType::Piglin => "entity/piglin/piglin",
        SkullType::Dragon => "entity/enderdragon/dragon",
    };

    let texture_size: [u32; 2] = match skull_type {
        SkullType::Piglin => [64, 64],
        SkullType::Dragon => [256, 256],
        SkullType::Zombie => [64, 64],
        _ => [64, 32],
    };

    // Head: 8x8x8 at texOffs(0,0)
    // Skull uses Y-down entity coords — wrap in root with RotX(PI) to flip Y-up.
    let head = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, -8.0, -4.0],
            dimensions: [8.0, 8.0, 8.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    let mut inner_parts = vec![head];

    // Hat overlay only for types that have hat texture region (zombie, piglin).
    // Skeleton, wither_skeleton, creeper have no hat data — renders as black.
    // Dragon has different UV layout — no standard hat.
    let has_hat = matches!(skull_type, SkullType::Zombie | SkullType::Piglin);
    if has_hat {
        inner_parts.push(EntityPart {
            cubes: vec![EntityCube {
                origin: [-4.0, -8.0, -4.0],
                dimensions: [8.0, 8.0, 8.0],
                tex_offset: [32, 0],
                inflate: 0.25,
                mirror: false,
                skip_faces: vec![],
            }],
            pose: Default::default(),
            children: vec![],
        });
    }

    // Root wrapper: RotX(PI) for Y-down→Y-up, position centers skull on block.
    let root = EntityPart {
        cubes: vec![],
        pose: EntityPartPose {
            position: [8.0, 0.0, 8.0],
            rotation: [std::f32::consts::PI, 0.0, 0.0],
            ..Default::default()
        },
        children: inner_parts,
    };

    EntityModelDef {
        texture_path: texture_path.to_string(),
        texture_size,
        parts: vec![root],
        is_opaque: has_hat,
    }
}
