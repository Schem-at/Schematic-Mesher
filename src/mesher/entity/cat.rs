use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose};

/// Cat (ocelot) model — texture `entity/cat/tabby`, 64x32.
/// From OcelotModel.java / CatModel.java (MC 1.21.4).
pub(super) fn cat_model() -> EntityModelDef {
    let head = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.5, -2.0, -3.0],
            dimensions: [5.0, 4.0, 5.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 15.0, -9.0],
            ..Default::default()
        },
        children: vec![],
    };

    let body = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, -3.0, -8.0],
            dimensions: [4.0, 16.0, 6.0],
            tex_offset: [20, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 12.0, -10.0],
            rotation: [std::f32::consts::FRAC_PI_2, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let tail1 = EntityPart {
        cubes: vec![EntityCube {
            origin: [-0.5, 0.0, 0.0],
            dimensions: [1.0, 8.0, 1.0],
            tex_offset: [0, 15],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 15.0, 8.0],
            rotation: [0.9, 0.0, 0.0], // ~51 degrees natural tail pose
            ..Default::default()
        },
        children: vec![],
    };

    let tail2 = EntityPart {
        cubes: vec![EntityCube {
            origin: [-0.5, 0.0, 0.0],
            dimensions: [1.0, 8.0, 1.0],
            tex_offset: [4, 15],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 20.0, 14.0],
            rotation: [1.7278, 0.0, 0.0], // ~99 degrees
            ..Default::default()
        },
        children: vec![],
    };

    let right_hind_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, 0.0, 1.0],
            dimensions: [2.0, 6.0, 2.0],
            tex_offset: [8, 13],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-1.1, 18.0, 5.0],
            ..Default::default()
        },
        children: vec![],
    };

    let left_hind_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, 0.0, 1.0],
            dimensions: [2.0, 6.0, 2.0],
            tex_offset: [8, 13],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [1.1, 18.0, 5.0],
            ..Default::default()
        },
        children: vec![],
    };

    let right_front_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, 0.0, 0.0],
            dimensions: [2.0, 10.0, 2.0],
            tex_offset: [40, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-1.2, 14.1, -5.0],
            ..Default::default()
        },
        children: vec![],
    };

    let left_front_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, 0.0, 0.0],
            dimensions: [2.0, 10.0, 2.0],
            tex_offset: [40, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [1.2, 14.1, -5.0],
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
            head, body, tail1, tail2,
            right_hind_leg, left_hind_leg, right_front_leg, left_front_leg,
        ],
    };

    EntityModelDef {
        texture_path: "entity/cat/tabby".to_string(),
        texture_size: [64, 32],
        parts: vec![root],
        is_opaque: true,
    }
}
