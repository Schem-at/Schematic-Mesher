use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose};

/// Horse model — texture `entity/horse/horse_brown`, 64x64.
/// From HorseModel.java / AbstractHorseModel.java (MC 1.21.4).
pub(super) fn horse_model() -> EntityModelDef {
    // Mane child of head_parts
    let mane = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, -11.0, 5.01],
            dimensions: [2.0, 16.0, 2.0],
            tex_offset: [56, 36],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    // Mouth (upper_mouth child of head_parts)
    let mouth = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, -11.0, -7.0],
            dimensions: [4.0, 5.0, 5.0],
            tex_offset: [0, 25],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    // Head parts (head + mane + mouth) — analogous to MC's head_parts
    let head_parts = EntityPart {
        cubes: vec![EntityCube {
            origin: [-3.0, -11.0, -2.0],
            dimensions: [6.0, 5.0, 7.0],
            tex_offset: [0, 35],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 4.0, -12.0],
            rotation: [std::f32::consts::FRAC_PI_6, 0.0, 0.0], // ~30 deg head tilt
            ..Default::default()
        },
        children: vec![mane, mouth],
    };

    let body = EntityPart {
        cubes: vec![EntityCube {
            origin: [-5.0, -8.0, -17.0],
            dimensions: [10.0, 10.0, 22.0],
            tex_offset: [0, 32],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 11.0, 5.0],
            rotation: [std::f32::consts::FRAC_PI_2, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let tail = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.5, 0.0, -2.0],
            dimensions: [3.0, 14.0, 4.0],
            tex_offset: [42, 36],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 4.0, 14.0],
            rotation: [-std::f32::consts::FRAC_PI_6, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // 4 legs — horse has 4×12 legs
    let right_hind_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -2.0],
            dimensions: [4.0, 12.0, 4.0],
            tex_offset: [48, 21],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-4.0, 12.0, 7.0],
            ..Default::default()
        },
        children: vec![],
    };

    let left_hind_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -2.0],
            dimensions: [4.0, 12.0, 4.0],
            tex_offset: [48, 21],
            inflate: 0.0,
            mirror: true,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [4.0, 12.0, 7.0],
            ..Default::default()
        },
        children: vec![],
    };

    let right_front_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -1.0],
            dimensions: [4.0, 12.0, 4.0],
            tex_offset: [48, 21],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-4.0, 12.0, -10.0],
            ..Default::default()
        },
        children: vec![],
    };

    let left_front_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -1.0],
            dimensions: [4.0, 12.0, 4.0],
            tex_offset: [48, 21],
            inflate: 0.0,
            mirror: true,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [4.0, 12.0, -10.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Y-down → Y-up root wrapper
    let root = EntityPart {
        cubes: vec![],
        pose: EntityPartPose {
            position: [8.0, 24.0, 8.0],
            rotation: [std::f32::consts::PI, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![
            head_parts, body, tail,
            right_hind_leg, left_hind_leg, right_front_leg, left_front_leg,
        ],
    };

    EntityModelDef {
        texture_path: "entity/horse/horse_brown".to_string(),
        texture_size: [64, 64],
        parts: vec![root],
        is_opaque: true,
    }
}
