use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose};

/// Bat model — texture `entity/bat`, 64x64.
/// From BatModel.java (MC 1.21.4).
/// Head, body, 2 wings (each with wing_tip child).
pub(super) fn bat_model() -> EntityModelDef {
    let head = EntityPart {
        cubes: vec![EntityCube {
            origin: [-3.0, -3.0, -3.0],
            dimensions: [6.0, 6.0, 6.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let body = EntityPart {
        cubes: vec![EntityCube {
            origin: [-3.0, 4.0, -3.0],
            dimensions: [6.0, 12.0, 6.0],
            tex_offset: [0, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    // Right wing tip (child of right wing)
    let right_wing_tip = EntityPart {
        cubes: vec![EntityCube {
            origin: [-8.0, 1.0, 1.5],
            dimensions: [8.0, 12.0, 1.0],
            tex_offset: [24, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    // Right wing
    let right_wing = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 1.0, 1.5],
            dimensions: [10.0, 16.0, 1.0],
            tex_offset: [42, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 1.0, 1.5],
            ..Default::default()
        },
        children: vec![right_wing_tip],
    };

    // Left wing tip (child of left wing)
    let left_wing_tip = EntityPart {
        cubes: vec![EntityCube {
            origin: [0.0, 1.0, 1.5],
            dimensions: [8.0, 12.0, 1.0],
            tex_offset: [24, 16],
            inflate: 0.0,
            mirror: true,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    // Left wing
    let left_wing = EntityPart {
        cubes: vec![EntityCube {
            origin: [-8.0, 1.0, 1.5],
            dimensions: [10.0, 16.0, 1.0],
            tex_offset: [42, 0],
            inflate: 0.0,
            mirror: true,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 1.0, 1.5],
            ..Default::default()
        },
        children: vec![left_wing_tip],
    };

    // Y-down → Y-up root wrapper
    // Bat is small, roughly 1 block tall
    let root = EntityPart {
        cubes: vec![],
        pose: EntityPartPose {
            position: [8.0, 24.0, 8.0],
            rotation: [std::f32::consts::PI, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![head, body, right_wing, left_wing],
    };

    EntityModelDef {
        texture_path: "entity/bat".to_string(),
        texture_size: [64, 64],
        parts: vec![root],
        is_opaque: true,
    }
}
