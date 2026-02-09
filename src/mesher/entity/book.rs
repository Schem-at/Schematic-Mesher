use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose};

/// Book model for lecterns and enchanting tables (64x32 texture).
/// Based on Minecraft's BookModel.java — 7 parts forming an open book.
///
/// Lids are open at ~70° (rotY = ±1.2 radians). Flip pages follow their
/// respective side. Pages have a small depth (0.005) to avoid z-fighting.
pub(super) fn book_model() -> EntityModelDef {
    // Left lid: texOffs(0,0), origin [-6,-5,0], dims [6,10,0.005], posed at (0,0,-1), rotY=+1.2
    let left_lid = EntityPart {
        cubes: vec![EntityCube {
            origin: [-6.0, -5.0, 0.0],
            dimensions: [6.0, 10.0, 0.005],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 0.0, -1.0],
            rotation: [0.0, 1.2, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Right lid: texOffs(16,0), origin [0,-5,0], dims [6,10,0.005], posed at (0,0,1), rotY=-1.2
    let right_lid = EntityPart {
        cubes: vec![EntityCube {
            origin: [0.0, -5.0, 0.0],
            dimensions: [6.0, 10.0, 0.005],
            tex_offset: [16, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 0.0, 1.0],
            rotation: [0.0, -1.2, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Seam (spine): texOffs(12,0), origin [-1,-5,0], dims [2,10,0.005], at origin
    let seam = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, -5.0, 0.0],
            dimensions: [2.0, 10.0, 0.005],
            tex_offset: [12, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    // Left pages: texOffs(0,10), origin [0,-4,-1], dims [5,8,1], posed at (0,0,-1), rotY=+1.2
    let left_pages = EntityPart {
        cubes: vec![EntityCube {
            origin: [0.0, -4.0, -1.0],
            dimensions: [5.0, 8.0, 1.0],
            tex_offset: [0, 10],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 0.0, -1.0],
            rotation: [0.0, 1.2, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Right pages: texOffs(12,10), origin [0,-4,0], dims [5,8,1], posed at (0,0,1), rotY=-1.2
    let right_pages = EntityPart {
        cubes: vec![EntityCube {
            origin: [0.0, -4.0, 0.0],
            dimensions: [5.0, 8.0, 1.0],
            tex_offset: [12, 10],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 0.0, 1.0],
            rotation: [0.0, -1.2, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Flip page 1: texOffs(24,10), origin [0,-4,0], dims [5,8,0.005], posed at (0,0,-1), rotY=+1.2
    let flip_page1 = EntityPart {
        cubes: vec![EntityCube {
            origin: [0.0, -4.0, 0.0],
            dimensions: [5.0, 8.0, 0.005],
            tex_offset: [24, 10],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 0.0, -1.0],
            rotation: [0.0, 1.2, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Flip page 2: texOffs(24,10), origin [0,-4,0], dims [5,8,0.005], posed at (0,0,1), rotY=-1.2
    let flip_page2 = EntityPart {
        cubes: vec![EntityCube {
            origin: [0.0, -4.0, 0.0],
            dimensions: [5.0, 8.0, 0.005],
            tex_offset: [24, 10],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 0.0, 1.0],
            rotation: [0.0, -1.2, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    EntityModelDef {
        texture_path: "entity/enchanting_table_book".to_string(),
        texture_size: [64, 32],
        parts: vec![left_lid, right_lid, seam, left_pages, right_pages, flip_page1, flip_page2],
        is_opaque: false,
    }
}
