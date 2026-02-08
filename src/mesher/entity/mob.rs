use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose, MobType};
use super::armor_stand;
use super::minecart;

pub(super) fn build_mob_model(mob_type: MobType) -> EntityModelDef {
    match mob_type {
        MobType::Zombie => zombie_model(),
        MobType::Skeleton => skeleton_model(),
        MobType::Creeper => creeper_model(),
        MobType::Pig => pig_model(),
        MobType::ArmorStand => armor_stand::armor_stand_model(),
        MobType::Minecart => minecart::minecart_model(),
        MobType::ItemFrame | MobType::GlowItemFrame | MobType::DroppedItem => {
            unreachable!("Item frames and dropped items handled in generate_mob_geometry")
        }
    }
}

/// Wrap mob body parts in a root part that converts Java Y-down to Y-up.
/// RotX(PI) flips Y and Z. Position [8, 24, 8] centers X/Z and translates up
/// so feet land at ground level (24/16 = 1.5 blocks up).
fn mob_root(children: Vec<EntityPart>) -> EntityPart {
    EntityPart {
        cubes: vec![],
        pose: EntityPartPose {
            position: [8.0, 24.0, 8.0],
            rotation: [std::f32::consts::PI, 0.0, 0.0],
            ..Default::default()
        },
        children,
    }
}

/// Zombie model — texture `entity/zombie/zombie`, 64x64.
fn zombie_model() -> EntityModelDef {
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

    let hat = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, -8.0, -4.0],
            dimensions: [8.0, 8.0, 8.0],
            tex_offset: [32, 0],
            inflate: 0.5,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    let body = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, 0.0, -2.0],
            dimensions: [8.0, 12.0, 4.0],
            tex_offset: [16, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    let right_arm = EntityPart {
        cubes: vec![EntityCube {
            origin: [-3.0, -2.0, -2.0],
            dimensions: [4.0, 12.0, 4.0],
            tex_offset: [40, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-5.0, 2.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let left_arm = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, -2.0, -2.0],
            dimensions: [4.0, 12.0, 4.0],
            tex_offset: [40, 16],
            inflate: 0.0,
            mirror: true,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [5.0, 2.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let right_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -2.0],
            dimensions: [4.0, 12.0, 4.0],
            tex_offset: [0, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-1.9, 12.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let left_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -2.0],
            dimensions: [4.0, 12.0, 4.0],
            tex_offset: [0, 16],
            inflate: 0.0,
            mirror: true,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [1.9, 12.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let root = mob_root(vec![head, hat, body, right_arm, left_arm, right_leg, left_leg]);

    EntityModelDef {
        texture_path: "entity/zombie/zombie".to_string(),
        texture_size: [64, 64],
        parts: vec![root],
        is_opaque: false, // Hat overlay has transparent pixels
    }
}

/// Skeleton model — texture `entity/skeleton/skeleton`, 64x32.
/// Same structure as zombie but 2-wide arms/legs.
fn skeleton_model() -> EntityModelDef {
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

    let body = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, 0.0, -2.0],
            dimensions: [8.0, 12.0, 4.0],
            tex_offset: [16, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    let right_arm = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, -2.0, -1.0],
            dimensions: [2.0, 12.0, 2.0],
            tex_offset: [40, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-5.0, 2.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let left_arm = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, -2.0, -1.0],
            dimensions: [2.0, 12.0, 2.0],
            tex_offset: [40, 16],
            inflate: 0.0,
            mirror: true,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [5.0, 2.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let right_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, 0.0, -1.0],
            dimensions: [2.0, 12.0, 2.0],
            tex_offset: [0, 16],
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
            dimensions: [2.0, 12.0, 2.0],
            tex_offset: [0, 16],
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

    let root = mob_root(vec![head, body, right_arm, left_arm, right_leg, left_leg]);

    EntityModelDef {
        texture_path: "entity/skeleton/skeleton".to_string(),
        texture_size: [64, 32],
        parts: vec![root],
        is_opaque: false, // Ribcage has transparent pixels
    }
}

/// Creeper model — texture `entity/creeper/creeper`, 64x32.
/// Quadruped with 4 identical short legs.
fn creeper_model() -> EntityModelDef {
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
            position: [0.0, 6.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let body = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, 0.0, -2.0],
            dimensions: [8.0, 12.0, 4.0],
            tex_offset: [16, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 6.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let right_hind_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -2.0],
            dimensions: [4.0, 6.0, 4.0],
            tex_offset: [0, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-2.0, 18.0, 4.0],
            ..Default::default()
        },
        children: vec![],
    };

    let left_hind_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -2.0],
            dimensions: [4.0, 6.0, 4.0],
            tex_offset: [0, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [2.0, 18.0, 4.0],
            ..Default::default()
        },
        children: vec![],
    };

    let right_front_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -2.0],
            dimensions: [4.0, 6.0, 4.0],
            tex_offset: [0, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-2.0, 18.0, -4.0],
            ..Default::default()
        },
        children: vec![],
    };

    let left_front_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -2.0],
            dimensions: [4.0, 6.0, 4.0],
            tex_offset: [0, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [2.0, 18.0, -4.0],
            ..Default::default()
        },
        children: vec![],
    };

    let root = mob_root(vec![head, body, right_hind_leg, left_hind_leg, right_front_leg, left_front_leg]);

    EntityModelDef {
        texture_path: "entity/creeper/creeper".to_string(),
        texture_size: [64, 32],
        parts: vec![root],
        is_opaque: true,
    }
}

/// Pig model — texture `entity/pig/temperate_pig`, 64x64.
/// Snout is a child of head. Body has RotX(PI/2).
fn pig_model() -> EntityModelDef {
    let snout = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -9.0],
            dimensions: [4.0, 3.0, 1.0],
            tex_offset: [16, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    let head = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, -4.0, -8.0],
            dimensions: [8.0, 8.0, 8.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 12.0, -6.0],
            ..Default::default()
        },
        children: vec![snout],
    };

    let body = EntityPart {
        cubes: vec![EntityCube {
            origin: [-5.0, -10.0, -7.0],
            dimensions: [10.0, 16.0, 8.0],
            tex_offset: [28, 8],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 11.0, 2.0],
            rotation: [std::f32::consts::FRAC_PI_2, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let right_hind_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -2.0],
            dimensions: [4.0, 6.0, 4.0],
            tex_offset: [0, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-3.0, 18.0, 7.0],
            ..Default::default()
        },
        children: vec![],
    };

    let left_hind_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -2.0],
            dimensions: [4.0, 6.0, 4.0],
            tex_offset: [0, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [3.0, 18.0, 7.0],
            ..Default::default()
        },
        children: vec![],
    };

    let right_front_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -2.0],
            dimensions: [4.0, 6.0, 4.0],
            tex_offset: [0, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [-3.0, 18.0, -5.0],
            ..Default::default()
        },
        children: vec![],
    };

    let left_front_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -2.0],
            dimensions: [4.0, 6.0, 4.0],
            tex_offset: [0, 16],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [3.0, 18.0, -5.0],
            ..Default::default()
        },
        children: vec![],
    };

    let root = mob_root(vec![head, body, right_hind_leg, left_hind_leg, right_front_leg, left_front_leg]);

    EntityModelDef {
        texture_path: "entity/pig/temperate_pig".to_string(),
        texture_size: [64, 64],
        parts: vec![root],
        is_opaque: true,
    }
}
