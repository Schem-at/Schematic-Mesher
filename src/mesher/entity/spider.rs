use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose};

/// Spider model — texture `entity/spider/spider`, 64x32.
/// From SpiderModel.java (MC 1.21.4).
pub(super) fn spider_model() -> EntityModelDef {
    let head = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, -4.0, -8.0],
            dimensions: [8.0, 8.0, 8.0],
            tex_offset: [32, 4],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 15.0, -3.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Neck (body0 in MC code)
    let neck = EntityPart {
        cubes: vec![EntityCube {
            origin: [-3.0, -3.0, -3.0],
            dimensions: [6.0, 6.0, 6.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 15.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Abdomen (body1 in MC code)
    let abdomen = EntityPart {
        cubes: vec![EntityCube {
            origin: [-5.0, -4.0, -6.0],
            dimensions: [10.0, 8.0, 12.0],
            tex_offset: [0, 12],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 15.0, 9.0],
            ..Default::default()
        },
        children: vec![],
    };

    // 8 legs — 4 pairs, each pair mirrored
    // Spider legs in MC use Y+Z rotation for the angled poses
    let leg_tex = [18, 0]; // All legs use same texture offset

    // Right legs (negative X positions)
    let right_leg_1 = spider_leg(leg_tex, [-4.0, 15.0, 2.0], 0.576, 0.1920);
    let right_leg_2 = spider_leg(leg_tex, [-4.0, 15.0, 1.0], 0.2618, 0.1920);
    let right_leg_3 = spider_leg(leg_tex, [-4.0, 15.0, 0.0], -0.2618, 0.1920);
    let right_leg_4 = spider_leg(leg_tex, [-4.0, 15.0, -1.0], -0.576, 0.1920);

    // Left legs (positive X positions)
    let left_leg_1 = spider_leg(leg_tex, [4.0, 15.0, 2.0], -0.576, -0.1920);
    let left_leg_2 = spider_leg(leg_tex, [4.0, 15.0, 1.0], -0.2618, -0.1920);
    let left_leg_3 = spider_leg(leg_tex, [4.0, 15.0, 0.0], 0.2618, -0.1920);
    let left_leg_4 = spider_leg(leg_tex, [4.0, 15.0, -1.0], 0.576, -0.1920);

    // Y-down → Y-up root wrapper
    let root = EntityPart {
        cubes: vec![],
        pose: EntityPartPose {
            position: [8.0, 24.0, 8.0],
            rotation: [std::f32::consts::PI, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![
            head, neck, abdomen,
            right_leg_1, right_leg_2, right_leg_3, right_leg_4,
            left_leg_1, left_leg_2, left_leg_3, left_leg_4,
        ],
    };

    EntityModelDef {
        texture_path: "entity/spider/spider".to_string(),
        texture_size: [64, 32],
        parts: vec![root],
        is_opaque: true,
    }
}

fn spider_leg(tex_offset: [u32; 2], position: [f32; 3], rot_y: f32, rot_z: f32) -> EntityPart {
    EntityPart {
        cubes: vec![EntityCube {
            origin: [-15.0, -1.0, -1.0],
            dimensions: [16.0, 2.0, 2.0],
            tex_offset,
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position,
            rotation: [0.0, rot_y, rot_z],
            ..Default::default()
        },
        children: vec![],
    }
}
