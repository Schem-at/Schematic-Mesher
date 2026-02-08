//! Block entity model geometry generation.
//!
//! Hardcodes entity model geometry for block entities like chests, beds, bells,
//! signs, and skulls. These blocks have minimal/empty JSON block models — their
//! actual visual geometry is defined in Java code.
//!
//! Follows the liquid module's integration pattern: detect entity type, generate
//! vertices/indices/face textures, then integrate in MeshBuilder::add_block().

use crate::mesher::geometry::Vertex;
use crate::types::{Direction, InputBlock};
use glam::{Mat4, Vec3, Vec4};

// ── Data Structures ─────────────────────────────────────────────────────────

/// A cube within an entity model part.
#[derive(Debug, Clone)]
pub struct EntityCube {
    /// Origin in 1/16th block units.
    pub origin: [f32; 3],
    /// Dimensions (W, H, D) in 1/16th block units.
    pub dimensions: [f32; 3],
    /// UV offset (u0, v0) in pixels on the texture sheet.
    pub tex_offset: [u32; 2],
    /// Expansion from cube center.
    pub inflate: f32,
    /// Mirror UVs horizontally.
    pub mirror: bool,
}

/// Pose/transform for an entity model part.
#[derive(Debug, Clone)]
pub struct EntityPartPose {
    /// Translation in 1/16th block units.
    pub position: [f32; 3],
    /// Rotation (x, y, z) in radians.
    pub rotation: [f32; 3],
    /// Scale factors.
    pub scale: [f32; 3],
}

impl Default for EntityPartPose {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
        }
    }
}

/// A part in the entity model hierarchy.
#[derive(Debug, Clone)]
pub struct EntityPart {
    pub cubes: Vec<EntityCube>,
    pub pose: EntityPartPose,
    pub children: Vec<EntityPart>,
}

/// Complete entity model definition.
#[derive(Debug, Clone)]
pub struct EntityModelDef {
    /// Texture path (e.g., "entity/chest/normal").
    pub texture_path: String,
    /// Texture sheet dimensions in pixels.
    pub texture_size: [u32; 2],
    /// Top-level parts.
    pub parts: Vec<EntityPart>,
    /// Whether this entity's geometry is opaque.
    pub is_opaque: bool,
}

/// Chest variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChestVariant {
    Normal,
    Trapped,
    Ender,
    Christmas,
}

/// Double chest side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoubleChestSide {
    Left,
    Right,
}

/// Sign wood types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignWood {
    Oak, Spruce, Birch, Jungle, Acacia, DarkOak,
    Crimson, Warped, Mangrove, Cherry, Bamboo,
}

/// Skull/head types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkullType {
    Skeleton, WitherSkeleton, Zombie, Creeper, Piglin, Dragon,
}

/// Block entity type detected from block ID.
#[derive(Debug, Clone)]
pub enum BlockEntityType {
    Chest(ChestVariant),
    DoubleChest { variant: ChestVariant, side: DoubleChestSide },
    Bed { color: String, is_head: bool },
    Bell,
    Sign { wood: SignWood, is_wall: bool },
    Skull(SkullType),
}

/// Mob entity types (rendered as static models).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MobType {
    Zombie,
    Skeleton,
    Creeper,
    Pig,
}

/// Face texture info for a generated entity face.
pub struct EntityFaceTexture {
    pub texture: String,
    pub is_transparent: bool,
}

// ── Detection ───────────────────────────────────────────────────────────────

/// Detect if a block is a block entity that needs hardcoded geometry.
pub fn detect_block_entity(block: &InputBlock) -> Option<BlockEntityType> {
    let block_id = block.block_id();

    match block_id {
        // Chests
        "chest" => {
            if let Some(side) = detect_double_chest(block) {
                Some(BlockEntityType::DoubleChest { variant: ChestVariant::Normal, side })
            } else {
                Some(BlockEntityType::Chest(ChestVariant::Normal))
            }
        }
        "trapped_chest" => {
            if let Some(side) = detect_double_chest(block) {
                Some(BlockEntityType::DoubleChest { variant: ChestVariant::Trapped, side })
            } else {
                Some(BlockEntityType::Chest(ChestVariant::Trapped))
            }
        }
        "ender_chest" => Some(BlockEntityType::Chest(ChestVariant::Ender)),

        // Beds
        id if id.ends_with("_bed") => {
            let color = id.strip_suffix("_bed").unwrap_or("red").to_string();
            let is_head = block.properties.get("part")
                .map(|p| p == "head")
                .unwrap_or(false);
            Some(BlockEntityType::Bed { color, is_head })
        }

        // Bell
        "bell" => Some(BlockEntityType::Bell),

        // Signs
        id if id.ends_with("_sign") || id.ends_with("_hanging_sign") => {
            // Skip hanging signs for now
            if id.contains("hanging") {
                return None;
            }
            let is_wall = id.starts_with("wall_") || id.contains("_wall_");
            let wood_str = id.strip_suffix("_sign")
                .and_then(|s| s.strip_prefix("wall_").or(Some(s)))
                .unwrap_or("oak");
            let wood = match wood_str {
                "spruce" => SignWood::Spruce,
                "birch" => SignWood::Birch,
                "jungle" => SignWood::Jungle,
                "acacia" => SignWood::Acacia,
                "dark_oak" => SignWood::DarkOak,
                "crimson" => SignWood::Crimson,
                "warped" => SignWood::Warped,
                "mangrove" => SignWood::Mangrove,
                "cherry" => SignWood::Cherry,
                "bamboo" => SignWood::Bamboo,
                _ => SignWood::Oak,
            };
            Some(BlockEntityType::Sign { wood, is_wall })
        }

        // Skulls / Heads
        "skeleton_skull" | "skeleton_wall_skull" =>
            Some(BlockEntityType::Skull(SkullType::Skeleton)),
        "wither_skeleton_skull" | "wither_skeleton_wall_skull" =>
            Some(BlockEntityType::Skull(SkullType::WitherSkeleton)),
        "zombie_head" | "zombie_wall_head" =>
            Some(BlockEntityType::Skull(SkullType::Zombie)),
        "creeper_head" | "creeper_wall_head" =>
            Some(BlockEntityType::Skull(SkullType::Creeper)),
        "piglin_head" | "piglin_wall_head" =>
            Some(BlockEntityType::Skull(SkullType::Piglin)),
        "dragon_head" | "dragon_wall_head" =>
            Some(BlockEntityType::Skull(SkullType::Dragon)),
        "player_head" | "player_wall_head" =>
            Some(BlockEntityType::Skull(SkullType::Skeleton)), // fallback texture

        _ => None,
    }
}

/// Detect if a block is a mob entity (custom `entity:` namespace convention).
pub fn detect_mob(block: &InputBlock) -> Option<MobType> {
    let block_id = block.block_id();
    match block_id {
        "zombie" => Some(MobType::Zombie),
        "skeleton" => Some(MobType::Skeleton),
        "creeper" => Some(MobType::Creeper),
        "pig" => Some(MobType::Pig),
        _ => None,
    }
}

fn detect_double_chest(block: &InputBlock) -> Option<DoubleChestSide> {
    match block.properties.get("type").map(|s| s.as_str()) {
        Some("left") => Some(DoubleChestSide::Left),
        Some("right") => Some(DoubleChestSide::Right),
        _ => None,
    }
}

// ── Facing Helpers ──────────────────────────────────────────────────────────

/// Get facing direction from block properties. Defaults to north.
fn get_facing(block: &InputBlock) -> &str {
    block.properties.get("facing").map(|s| s.as_str()).unwrap_or("north")
}

/// Y rotation angle for a facing direction (radians).
/// In Minecraft, entity models face south by default (toward +Z).
/// North = 180deg, South = 0deg, East = -90deg (270), West = 90deg.
fn facing_rotation_rad(facing: &str) -> f32 {
    match facing {
        "north" => std::f32::consts::PI,
        "south" => 0.0,
        "east" => -std::f32::consts::FRAC_PI_2,
        "west" => std::f32::consts::FRAC_PI_2,
        _ => std::f32::consts::PI,
    }
}

/// Standing sign/skull rotation from `rotation` property (0-15, each = 22.5 degrees).
fn standing_rotation_rad(block: &InputBlock) -> f32 {
    let rot: u8 = block.properties.get("rotation")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    // Rotation 0 = south (facing +Z), each step is 22.5 degrees CW from above
    rot as f32 * std::f32::consts::PI / 8.0
}

// ── UV Computation ──────────────────────────────────────────────────────────

/// Compute UVs for one face of a cube using Minecraft's box-unwrap layout.
///
/// For a cube of size (W, H, D) at UV origin (u0, v0):
/// ```text
///                u0     u0+D   u0+D+W  u0+2D+W  u0+2D+2W
/// v0             | DOWN  |  UP   |       |        |
/// v0+D           |       |       |       |        |
/// v0+D    WEST   | NORTH | EAST  | SOUTH |
/// v0+D+H         |       |       |       |
/// ```
fn cube_face_uvs(
    tex_offset: [u32; 2],
    dimensions: [f32; 3],
    face: Direction,
    texture_size: [u32; 2],
    mirror: bool,
) -> [[f32; 2]; 4] {
    let u0 = tex_offset[0] as f32;
    let v0 = tex_offset[1] as f32;
    let w = dimensions[0]; // X dimension
    let h = dimensions[1]; // Y dimension
    let d = dimensions[2]; // Z dimension
    let tw = texture_size[0] as f32;
    let th = texture_size[1] as f32;

    // UV region in pixel space (left, top, right, bottom)
    let (left, top, right, bottom) = match face {
        Direction::Down => (u0 + d, v0, u0 + d + w, v0 + d),
        Direction::Up => (u0 + d + w, v0, u0 + d + w + w, v0 + d),
        Direction::North => (u0 + d, v0 + d, u0 + d + w, v0 + d + h),
        Direction::South => (u0 + d + w + d, v0 + d, u0 + d + w + d + w, v0 + d + h),
        Direction::West => (u0, v0 + d, u0 + d, v0 + d + h),
        Direction::East => (u0 + d + w, v0 + d, u0 + d + w + d, v0 + d + h),
    };

    // Normalize to [0,1] UV space
    let (nl, nt, nr, nb) = (left / tw, top / th, right / tw, bottom / th);

    // Minecraft's Polygon constructor assigns vertex[0]=(u1,v0) i.e. RIGHT-top first.
    // Our corner ordering matches Minecraft's for Down/N/S/E/W, so the default
    // (non-mirrored) state swaps U (puts u_right first).
    // The Up face has a different vertex order from Minecraft ([3,2,6,7] vs [2,3,7,6]),
    // so U should NOT be swapped, but V should be flipped (Minecraft inverts V for Up).
    match face {
        Direction::Up => {
            if mirror {
                [[nr, nb], [nl, nb], [nl, nt], [nr, nt]]
            } else {
                [[nl, nb], [nr, nb], [nr, nt], [nl, nt]]
            }
        }
        _ => {
            if mirror {
                [[nl, nt], [nr, nt], [nr, nb], [nl, nb]]
            } else {
                [[nr, nt], [nl, nt], [nl, nb], [nr, nb]]
            }
        }
    }
}

// ── Geometry Generation ─────────────────────────────────────────────────────

/// Generate all entity geometry for a block entity.
///
/// Returns (vertices, indices, face_textures) ready for MeshBuilder integration.
pub fn generate_entity_geometry(
    block: &InputBlock,
    entity_type: &BlockEntityType,
) -> (Vec<Vertex>, Vec<u32>, Vec<EntityFaceTexture>) {
    let model = build_model_def(entity_type);

    let facing = get_facing(block);
    let facing_angle = match entity_type {
        BlockEntityType::Sign { is_wall: false, .. } => standing_rotation_rad(block),
        BlockEntityType::Skull(_) => {
            let block_id = block.block_id();
            if block_id.contains("wall") {
                facing_rotation_rad(facing)
            } else {
                standing_rotation_rad(block)
            }
        }
        _ => facing_rotation_rad(facing),
    };

    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut face_textures = Vec::new();

    // Build facing rotation matrix (rotate around block center 0.5, 0, 0.5)
    let facing_mat = Mat4::from_translation(Vec3::new(0.5, 0.0, 0.5))
        * Mat4::from_rotation_y(facing_angle)
        * Mat4::from_translation(Vec3::new(-0.5, 0.0, -0.5));

    traverse_parts(
        &model.parts,
        Mat4::IDENTITY,
        &facing_mat,
        &model,
        &mut vertices,
        &mut indices,
        &mut face_textures,
    );

    (vertices, indices, face_textures)
}

/// Generate all geometry for a mob entity.
///
/// Returns (vertices, indices, face_textures) ready for MeshBuilder integration.
pub fn generate_mob_geometry(
    block: &InputBlock,
    mob_type: MobType,
) -> (Vec<Vertex>, Vec<u32>, Vec<EntityFaceTexture>) {
    let model = build_mob_model(mob_type);

    let facing = get_facing(block);
    let facing_angle = facing_rotation_rad(facing);

    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut face_textures = Vec::new();

    // Build facing rotation matrix (rotate around block center 0.5, 0, 0.5)
    let facing_mat = Mat4::from_translation(Vec3::new(0.5, 0.0, 0.5))
        * Mat4::from_rotation_y(facing_angle)
        * Mat4::from_translation(Vec3::new(-0.5, 0.0, -0.5));

    traverse_parts(
        &model.parts,
        Mat4::IDENTITY,
        &facing_mat,
        &model,
        &mut vertices,
        &mut indices,
        &mut face_textures,
    );

    (vertices, indices, face_textures)
}

/// Recursively traverse part hierarchy, accumulating transforms.
fn traverse_parts(
    parts: &[EntityPart],
    parent_transform: Mat4,
    facing_mat: &Mat4,
    model: &EntityModelDef,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    face_textures: &mut Vec<EntityFaceTexture>,
) {
    for part in parts {
        // Build part's local transform: translate -> rotateZYX -> scale
        let pose = &part.pose;
        let local = Mat4::from_translation(Vec3::new(
            pose.position[0] / 16.0,
            pose.position[1] / 16.0,
            pose.position[2] / 16.0,
        ))
        * Mat4::from_rotation_z(pose.rotation[2])
        * Mat4::from_rotation_y(pose.rotation[1])
        * Mat4::from_rotation_x(pose.rotation[0])
        * Mat4::from_scale(Vec3::new(pose.scale[0], pose.scale[1], pose.scale[2]));

        let combined = parent_transform * local;

        // Generate geometry for each cube in this part
        for cube in &part.cubes {
            generate_cube_faces(
                cube,
                &combined,
                facing_mat,
                model,
                vertices,
                indices,
                face_textures,
            );
        }

        // Recurse into children
        traverse_parts(
            &part.children,
            combined,
            facing_mat,
            model,
            vertices,
            indices,
            face_textures,
        );
    }
}

/// Generate 6 face quads for an entity cube.
fn generate_cube_faces(
    cube: &EntityCube,
    transform: &Mat4,
    facing_mat: &Mat4,
    model: &EntityModelDef,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    face_textures: &mut Vec<EntityFaceTexture>,
) {
    let [ox, oy, oz] = cube.origin;
    let [w, h, d] = cube.dimensions;
    let inf = cube.inflate;

    // Cube corners in 1/16th block units, with inflate
    let x0 = (ox - inf) / 16.0;
    let y0 = (oy - inf) / 16.0;
    let z0 = (oz - inf) / 16.0;
    let x1 = (ox + w + inf) / 16.0;
    let y1 = (oy + h + inf) / 16.0;
    let z1 = (oz + d + inf) / 16.0;

    // The 8 corners of the cube in local space
    let corners = [
        Vec3::new(x0, y0, z0), // 0: ---
        Vec3::new(x1, y0, z0), // 1: +--
        Vec3::new(x1, y1, z0), // 2: ++-
        Vec3::new(x0, y1, z0), // 3: -+-
        Vec3::new(x0, y0, z1), // 4: --+
        Vec3::new(x1, y0, z1), // 5: +-+
        Vec3::new(x1, y1, z1), // 6: +++
        Vec3::new(x0, y1, z1), // 7: -++
    ];

    // Transform corners: part hierarchy transform, then divide by... wait, already /16.
    // Apply part transform then facing transform.
    let full_transform = *facing_mat * *transform;
    let transformed: Vec<[f32; 3]> = corners.iter().map(|c| {
        let p = full_transform * Vec4::new(c.x, c.y, c.z, 1.0);
        [p.x, p.y, p.z]
    }).collect();

    // 6 faces: each defined by 4 corner indices and a direction
    let face_defs: [(Direction, [usize; 4]); 6] = [
        // Down: bottom face (y0), CCW from outside → looking from -Y
        (Direction::Down, [4, 5, 1, 0]),
        // Up: top face (y1)
        (Direction::Up, [3, 2, 6, 7]),
        // North: front face (z0)
        (Direction::North, [1, 0, 3, 2]),
        // South: back face (z1)
        (Direction::South, [4, 5, 6, 7]),
        // West: left face (x0)
        (Direction::West, [0, 4, 7, 3]),
        // East: right face (x1)
        (Direction::East, [5, 1, 2, 6]),
    ];

    for &(direction, corner_indices) in &face_defs {
        let uvs = cube_face_uvs(
            cube.tex_offset,
            cube.dimensions,
            direction,
            model.texture_size,
            cube.mirror,
        );

        // Compute normal from known direction, transformed by the full rotation
        let dn = direction.normal();
        let n4 = full_transform * Vec4::new(dn[0], dn[1], dn[2], 0.0);
        let normal = Vec3::new(n4.x, n4.y, n4.z).normalize_or_zero();
        let n = [normal.x, normal.y, normal.z];

        let v_start = vertices.len() as u32;

        for (i, &ci) in corner_indices.iter().enumerate() {
            vertices.push(Vertex::new(transformed[ci], n, uvs[i]));
        }

        // Two triangles: CCW winding for glTF
        // Down/Up faces are correct with (0,2,1)(0,3,2) winding;
        // side faces need reversed winding (0,1,2)(0,2,3) to face outward
        let is_side = matches!(direction,
            Direction::North | Direction::South | Direction::West | Direction::East);
        if is_side {
            indices.extend_from_slice(&[
                v_start, v_start + 1, v_start + 2,
                v_start, v_start + 2, v_start + 3,
            ]);
        } else {
            indices.extend_from_slice(&[
                v_start, v_start + 2, v_start + 1,
                v_start, v_start + 3, v_start + 2,
            ]);
        }

        face_textures.push(EntityFaceTexture {
            texture: model.texture_path.clone(),
            is_transparent: !model.is_opaque,
        });
    }
}

// ── Model Definitions ───────────────────────────────────────────────────────

fn build_model_def(entity_type: &BlockEntityType) -> EntityModelDef {
    match entity_type {
        BlockEntityType::Chest(variant) => chest_model(*variant),
        BlockEntityType::DoubleChest { variant, side } => double_chest_model(*variant, *side),
        BlockEntityType::Bed { color, is_head } => bed_model(color, *is_head),
        BlockEntityType::Bell => bell_model(),
        BlockEntityType::Sign { wood, is_wall } => sign_model(*wood, *is_wall),
        BlockEntityType::Skull(skull_type) => skull_model(*skull_type),
    }
}

/// Single chest model (64x64 texture).
/// Values from EntityModelJson dump (Minecraft 1.19).
fn chest_model(variant: ChestVariant) -> EntityModelDef {
    let texture_path = match variant {
        ChestVariant::Normal => "entity/chest/normal",
        ChestVariant::Trapped => "entity/chest/trapped",
        ChestVariant::Ender => "entity/chest/ender",
        ChestVariant::Christmas => "entity/chest/christmas",
    };

    // Bottom: origin [1,0,1], dims [14,10,14], texOffs(0,19)
    let bottom = EntityPart {
        cubes: vec![EntityCube {
            origin: [1.0, 0.0, 1.0],
            dimensions: [14.0, 10.0, 14.0],
            tex_offset: [0, 19],
            inflate: 0.0,
            mirror: false,
        }],
        pose: Default::default(),
        children: vec![],
    };

    // Lid: origin [1,0,0], dims [14,5,14], texOffs(0,0), pose position=[0,9,1]
    let lid = EntityPart {
        cubes: vec![EntityCube {
            origin: [1.0, 0.0, 0.0],
            dimensions: [14.0, 5.0, 14.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
        }],
        pose: EntityPartPose {
            position: [0.0, 9.0, 1.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Lock: origin [7,-1,15], dims [2,4,1], texOffs(0,0), pose position=[0,8,0]
    let lock = EntityPart {
        cubes: vec![EntityCube {
            origin: [7.0, -1.0, 15.0],
            dimensions: [2.0, 4.0, 1.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
        }],
        pose: EntityPartPose {
            position: [0.0, 8.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    EntityModelDef {
        texture_path: texture_path.to_string(),
        texture_size: [64, 64],
        parts: vec![bottom, lid, lock],
        is_opaque: true,
    }
}

/// Double chest model (uses left/right textures).
fn double_chest_model(variant: ChestVariant, side: DoubleChestSide) -> EntityModelDef {
    let base = match variant {
        ChestVariant::Normal => "entity/chest/normal",
        ChestVariant::Trapped => "entity/chest/trapped",
        ChestVariant::Ender => "entity/chest/ender",
        ChestVariant::Christmas => "entity/chest/christmas",
    };
    let suffix = match side {
        DoubleChestSide::Left => "_left",
        DoubleChestSide::Right => "_right",
    };
    let texture_path = format!("{}{}", base, suffix);

    // Double chest values from EntityModelJson dump.
    let (bottom_origin, lid_origin, lock_origin, lock_dims) = match side {
        DoubleChestSide::Left => ([0.0, 0.0, 1.0], [0.0, 0.0, 0.0], [0.0, -1.0, 15.0], [1.0, 4.0, 1.0]),
        DoubleChestSide::Right => ([1.0, 0.0, 1.0], [1.0, 0.0, 0.0], [15.0, -1.0, 15.0], [1.0, 4.0, 1.0]),
    };

    let bottom = EntityPart {
        cubes: vec![EntityCube {
            origin: bottom_origin,
            dimensions: [15.0, 10.0, 14.0],
            tex_offset: [0, 19],
            inflate: 0.0,
            mirror: false,
        }],
        pose: Default::default(),
        children: vec![],
    };

    let lid = EntityPart {
        cubes: vec![EntityCube {
            origin: lid_origin,
            dimensions: [15.0, 5.0, 14.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
        }],
        pose: EntityPartPose {
            position: [0.0, 9.0, 1.0],
            ..Default::default()
        },
        children: vec![],
    };

    let lock = EntityPart {
        cubes: vec![EntityCube {
            origin: lock_origin,
            dimensions: [lock_dims[0], lock_dims[1], lock_dims[2]],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
        }],
        pose: EntityPartPose {
            position: [0.0, 8.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    EntityModelDef {
        texture_path,
        texture_size: [64, 64],
        parts: vec![bottom, lid, lock],
        is_opaque: true,
    }
}

/// Bed model (64x64 texture).
/// The mattress is modeled upright (16x16x6) and rotated +90° X to lay flat.
/// Minecraft uses xRot=PI/2: North face UV becomes the visible sleeping surface.
/// Legs are simple unrotated 3x3x3 cubes at world positions.
fn bed_model(color: &str, is_head: bool) -> EntityModelDef {
    let texture_path = format!("entity/bed/{}", color);

    let tex_main: [u32; 2] = if is_head { [0, 0] } else { [0, 22] };

    // Main mattress: origin [0,0,0], dims [16,16,6], rotated +90° X.
    // RotX(PI/2): cube [0,0,0]-[16,16,6] → [0,-6,0]-[16,0,16].
    // Position [0, 9, 0] raises it to y=[3,9] in 1/16 units.
    let main = EntityPart {
        cubes: vec![EntityCube {
            origin: [0.0, 0.0, 0.0],
            dimensions: [16.0, 16.0, 6.0],
            tex_offset: tex_main,
            inflate: 0.0,
            mirror: false,
        }],
        pose: EntityPartPose {
            position: [0.0, 9.0, 0.0],
            rotation: [std::f32::consts::FRAC_PI_2, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Legs: 3x3x3 unrotated cubes at fixed positions.
    let (leg_tex_left, leg_tex_right, leg_z) = if is_head {
        // Head legs at z=0 side (headboard)
        ([50u32, 6u32], [50u32, 18u32], 0.0)
    } else {
        // Foot legs at z=13 side (footboard)
        ([50, 0], [50, 12], 13.0)
    };

    let left_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [0.0, 0.0, leg_z],
            dimensions: [3.0, 3.0, 3.0],
            tex_offset: leg_tex_left,
            inflate: 0.0,
            mirror: false,
        }],
        pose: Default::default(),
        children: vec![],
    };

    let right_leg = EntityPart {
        cubes: vec![EntityCube {
            origin: [13.0, 0.0, leg_z],
            dimensions: [3.0, 3.0, 3.0],
            tex_offset: leg_tex_right,
            inflate: 0.0,
            mirror: false,
        }],
        pose: Default::default(),
        children: vec![],
    };

    EntityModelDef {
        texture_path,
        texture_size: [64, 64],
        parts: vec![main, left_leg, right_leg],
        is_opaque: true,
    }
}

/// Bell model (32x32 texture). Only the bell body — frame is from JSON model.
fn bell_model() -> EntityModelDef {
    // Bell body: 6x7x6 at texOffs(0,0)
    let bell_body = EntityPart {
        cubes: vec![EntityCube {
            origin: [-3.0, -7.0, -3.0],
            dimensions: [6.0, 7.0, 6.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
        }],
        pose: EntityPartPose {
            position: [8.0, 12.0, 8.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Bell lip: 8x2x8 at bottom of body
    let bell_lip = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, -9.0, -4.0],
            dimensions: [8.0, 2.0, 8.0],
            tex_offset: [0, 13],
            inflate: 0.0,
            mirror: false,
        }],
        pose: EntityPartPose {
            position: [8.0, 12.0, 8.0],
            ..Default::default()
        },
        children: vec![],
    };

    EntityModelDef {
        texture_path: "entity/bell/bell_body".to_string(),
        texture_size: [32, 32],
        parts: vec![bell_body, bell_lip],
        is_opaque: true,
    }
}

/// Sign model.
fn sign_model(wood: SignWood, is_wall: bool) -> EntityModelDef {
    let texture_path = match wood {
        SignWood::Oak => "entity/signs/oak",
        SignWood::Spruce => "entity/signs/spruce",
        SignWood::Birch => "entity/signs/birch",
        SignWood::Jungle => "entity/signs/jungle",
        SignWood::Acacia => "entity/signs/acacia",
        SignWood::DarkOak => "entity/signs/dark_oak",
        SignWood::Crimson => "entity/signs/crimson",
        SignWood::Warped => "entity/signs/warped",
        SignWood::Mangrove => "entity/signs/mangrove",
        SignWood::Cherry => "entity/signs/cherry",
        SignWood::Bamboo => "entity/signs/bamboo",
    };

    // Sign uses Java Y-down coords. Renderer applies RotX(PI) + 2/3 scale.
    // Standing: position [8, 12, 8] (centered, raised to 0.75 blocks)
    // Wall: position [8, 7, 1] (centered X, lower Y, pushed against -Z wall face)
    let (pos_x, pos_y, pos_z) = if is_wall {
        (8.0, 7.0, 15.0)
    } else {
        (8.0, 12.0, 8.0)
    };
    let scale = 2.0 / 3.0;

    // Board: origin [-12,-14,-1], dims [24,12,2], texOffs(0,0)
    let board = EntityPart {
        cubes: vec![EntityCube {
            origin: [-12.0, -14.0, -1.0],
            dimensions: [24.0, 12.0, 2.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
        }],
        pose: EntityPartPose {
            position: [pos_x, pos_y, pos_z],
            rotation: [std::f32::consts::PI, 0.0, 0.0],
            scale: [scale, scale, scale],
        },
        children: vec![],
    };

    let mut parts = vec![board];

    // Standing signs have a stick
    if !is_wall {
        // Stick: origin [-1,-2,-1], dims [2,14,2], texOffs(0,14)
        let stick = EntityPart {
            cubes: vec![EntityCube {
                origin: [-1.0, -2.0, -1.0],
                dimensions: [2.0, 14.0, 2.0],
                tex_offset: [0, 14],
                inflate: 0.0,
                mirror: false,
            }],
            pose: EntityPartPose {
                position: [pos_x, pos_y, pos_z],
                rotation: [std::f32::consts::PI, 0.0, 0.0],
                scale: [scale, scale, scale],
            },
            children: vec![],
        };
        parts.push(stick);
    }

    EntityModelDef {
        texture_path: texture_path.to_string(),
        texture_size: [64, 32],
        parts,
        is_opaque: true,
    }
}

/// Skull/head model.
fn skull_model(skull_type: SkullType) -> EntityModelDef {
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

// ── Mob Model Definitions ──────────────────────────────────────────────────

fn build_mob_model(mob_type: MobType) -> EntityModelDef {
    match mob_type {
        MobType::Zombie => zombie_model(),
        MobType::Skeleton => skeleton_model(),
        MobType::Creeper => creeper_model(),
        MobType::Pig => pig_model(),
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

/// Pig model — texture `entity/pig/pig`, 64x32.
/// Snout is a child of head. Body has RotX(PI/2).
fn pig_model() -> EntityModelDef {
    let snout = EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 0.0, -9.0],
            dimensions: [4.0, 3.0, 1.0],
            tex_offset: [16, 16],
            inflate: 0.0,
            mirror: false,
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

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_chest() {
        let block = InputBlock::new("minecraft:chest");
        let entity = detect_block_entity(&block);
        assert!(matches!(entity, Some(BlockEntityType::Chest(ChestVariant::Normal))));
    }

    #[test]
    fn test_detect_trapped_chest() {
        let block = InputBlock::new("minecraft:trapped_chest");
        let entity = detect_block_entity(&block);
        assert!(matches!(entity, Some(BlockEntityType::Chest(ChestVariant::Trapped))));
    }

    #[test]
    fn test_detect_ender_chest() {
        let block = InputBlock::new("minecraft:ender_chest");
        let entity = detect_block_entity(&block);
        assert!(matches!(entity, Some(BlockEntityType::Chest(ChestVariant::Ender))));
    }

    #[test]
    fn test_detect_double_chest() {
        let block = InputBlock::new("minecraft:chest")
            .with_property("type", "left")
            .with_property("facing", "north");
        let entity = detect_block_entity(&block);
        assert!(matches!(entity, Some(BlockEntityType::DoubleChest {
            variant: ChestVariant::Normal,
            side: DoubleChestSide::Left,
        })));
    }

    #[test]
    fn test_detect_bed() {
        let block = InputBlock::new("minecraft:red_bed")
            .with_property("part", "head");
        let entity = detect_block_entity(&block);
        match entity {
            Some(BlockEntityType::Bed { color, is_head }) => {
                assert_eq!(color, "red");
                assert!(is_head);
            }
            _ => panic!("Expected Bed entity"),
        }
    }

    #[test]
    fn test_detect_bell() {
        let block = InputBlock::new("minecraft:bell");
        let entity = detect_block_entity(&block);
        assert!(matches!(entity, Some(BlockEntityType::Bell)));
    }

    #[test]
    fn test_detect_sign() {
        let block = InputBlock::new("minecraft:oak_sign");
        let entity = detect_block_entity(&block);
        assert!(matches!(entity, Some(BlockEntityType::Sign { wood: SignWood::Oak, is_wall: false })));

        let block = InputBlock::new("minecraft:wall_birch_sign");
        let entity = detect_block_entity(&block);
        assert!(matches!(entity, Some(BlockEntityType::Sign { wood: SignWood::Birch, is_wall: true })));
    }

    #[test]
    fn test_detect_skull() {
        let block = InputBlock::new("minecraft:skeleton_skull");
        let entity = detect_block_entity(&block);
        assert!(matches!(entity, Some(BlockEntityType::Skull(SkullType::Skeleton))));

        let block = InputBlock::new("minecraft:creeper_head");
        let entity = detect_block_entity(&block);
        assert!(matches!(entity, Some(BlockEntityType::Skull(SkullType::Creeper))));
    }

    #[test]
    fn test_detect_non_entity() {
        let block = InputBlock::new("minecraft:stone");
        assert!(detect_block_entity(&block).is_none());

        let block = InputBlock::new("minecraft:oak_planks");
        assert!(detect_block_entity(&block).is_none());
    }

    #[test]
    fn test_cube_face_uvs_north() {
        // A 4x4x4 cube at tex offset (0,0) on a 64x64 texture
        let uvs = cube_face_uvs([0, 0], [4.0, 4.0, 4.0], Direction::North, [64, 64], false);
        // North face: (u0+D, v0+D, u0+D+W, v0+D+H) = (4, 4, 8, 8) / 64
        // Non-mirrored: UV[0] = (nr, nt), UV[1] = (nl, nt)
        assert!((uvs[0][0] - 8.0 / 64.0).abs() < 0.001); // nr = right
        assert!((uvs[0][1] - 4.0 / 64.0).abs() < 0.001); // nt = top
        assert!((uvs[1][0] - 4.0 / 64.0).abs() < 0.001); // nl = left
        assert!((uvs[2][1] - 8.0 / 64.0).abs() < 0.001); // nb = bottom
    }

    #[test]
    fn test_cube_face_uvs_mirrored() {
        let normal = cube_face_uvs([0, 0], [4.0, 4.0, 4.0], Direction::North, [64, 64], false);
        let mirrored = cube_face_uvs([0, 0], [4.0, 4.0, 4.0], Direction::North, [64, 64], true);
        // Mirrored: left and right U coords should be swapped vs normal
        assert!((normal[0][0] - mirrored[1][0]).abs() < 0.001);
        assert!((normal[1][0] - mirrored[0][0]).abs() < 0.001);
    }

    #[test]
    fn test_chest_geometry_count() {
        let block = InputBlock::new("minecraft:chest")
            .with_property("facing", "north");
        let (verts, indices, faces) = generate_entity_geometry(
            &block,
            &BlockEntityType::Chest(ChestVariant::Normal),
        );

        // Chest has bottom (1 cube x 6 faces) + lid (1 cube x 6 faces) + lock (1 cube x 6 faces) = 18 faces
        // Each face: 4 vertices, 6 indices
        assert_eq!(faces.len(), 18);
        assert_eq!(verts.len(), 18 * 4);
        assert_eq!(indices.len(), 18 * 6);
    }

    #[test]
    fn test_bell_geometry_count() {
        let block = InputBlock::new("minecraft:bell")
            .with_property("facing", "north");
        let (verts, indices, faces) = generate_entity_geometry(
            &block,
            &BlockEntityType::Bell,
        );

        // Bell: body (1 cube x 6) + lip (1 cube x 6) = 12 faces
        assert_eq!(faces.len(), 12);
        assert_eq!(verts.len(), 12 * 4);
        assert_eq!(indices.len(), 12 * 6);
    }

    #[test]
    fn test_skull_geometry_count_no_hat() {
        // Skeleton skull: no hat overlay → 6 faces (head only)
        let block = InputBlock::new("minecraft:skeleton_skull");
        let (verts, indices, faces) = generate_entity_geometry(
            &block,
            &BlockEntityType::Skull(SkullType::Skeleton),
        );

        assert_eq!(faces.len(), 6);
        assert_eq!(verts.len(), 6 * 4);
        assert_eq!(indices.len(), 6 * 6);
    }

    #[test]
    fn test_skull_geometry_count_with_hat() {
        // Zombie head: has hat overlay → 12 faces (head + hat)
        let block = InputBlock::new("minecraft:zombie_head");
        let (verts, indices, faces) = generate_entity_geometry(
            &block,
            &BlockEntityType::Skull(SkullType::Zombie),
        );

        assert_eq!(faces.len(), 12);
        assert_eq!(verts.len(), 12 * 4);
        assert_eq!(indices.len(), 12 * 6);
    }

    #[test]
    fn test_sign_standing_geometry_count() {
        let block = InputBlock::new("minecraft:oak_sign");
        let (_, _, faces) = generate_entity_geometry(
            &block,
            &BlockEntityType::Sign { wood: SignWood::Oak, is_wall: false },
        );

        // Standing sign: board (1 cube x 6) + stick (1 cube x 6) = 12 faces
        assert_eq!(faces.len(), 12);
    }

    #[test]
    fn test_sign_wall_geometry_count() {
        let block = InputBlock::new("minecraft:wall_oak_sign")
            .with_property("facing", "north");
        let (_, _, faces) = generate_entity_geometry(
            &block,
            &BlockEntityType::Sign { wood: SignWood::Oak, is_wall: true },
        );

        // Wall sign: board only (1 cube x 6) = 6 faces
        assert_eq!(faces.len(), 6);
    }

    #[test]
    fn test_facing_rotation() {
        // Generate chest geometry facing north vs south — vertices should differ
        let block_n = InputBlock::new("minecraft:chest")
            .with_property("facing", "north");
        let block_s = InputBlock::new("minecraft:chest")
            .with_property("facing", "south");

        let (verts_n, _, _) = generate_entity_geometry(
            &block_n,
            &BlockEntityType::Chest(ChestVariant::Normal),
        );
        let (verts_s, _, _) = generate_entity_geometry(
            &block_s,
            &BlockEntityType::Chest(ChestVariant::Normal),
        );

        // At least some vertex positions should differ between orientations
        let mut any_different = false;
        for (vn, vs) in verts_n.iter().zip(verts_s.iter()) {
            if (vn.position[0] - vs.position[0]).abs() > 0.01
                || (vn.position[2] - vs.position[2]).abs() > 0.01
            {
                any_different = true;
                break;
            }
        }
        assert!(any_different, "North and south chests should have different vertex positions");
    }

    #[test]
    fn test_detect_mob() {
        assert!(matches!(detect_mob(&InputBlock::new("entity:zombie")), Some(MobType::Zombie)));
        assert!(matches!(detect_mob(&InputBlock::new("entity:skeleton")), Some(MobType::Skeleton)));
        assert!(matches!(detect_mob(&InputBlock::new("entity:creeper")), Some(MobType::Creeper)));
        assert!(matches!(detect_mob(&InputBlock::new("entity:pig")), Some(MobType::Pig)));
        assert!(detect_mob(&InputBlock::new("minecraft:stone")).is_none());
    }

    #[test]
    fn test_zombie_geometry_count() {
        let block = InputBlock::new("entity:zombie")
            .with_property("facing", "south");
        let (verts, indices, faces) = generate_mob_geometry(&block, MobType::Zombie);

        // Zombie: 7 parts (head, hat, body, 2 arms, 2 legs) × 6 faces = 42 faces
        assert_eq!(faces.len(), 42);
        assert_eq!(verts.len(), 42 * 4);
        assert_eq!(indices.len(), 42 * 6);
    }

    #[test]
    fn test_creeper_geometry_count() {
        let block = InputBlock::new("entity:creeper")
            .with_property("facing", "south");
        let (verts, indices, faces) = generate_mob_geometry(&block, MobType::Creeper);

        // Creeper: 6 parts (head, body, 4 legs) × 6 faces = 36 faces
        assert_eq!(faces.len(), 36);
        assert_eq!(verts.len(), 36 * 4);
        assert_eq!(indices.len(), 36 * 6);
    }

    #[test]
    fn test_pig_geometry_count() {
        let block = InputBlock::new("entity:pig")
            .with_property("facing", "south");
        let (verts, indices, faces) = generate_mob_geometry(&block, MobType::Pig);

        // Pig: 7 parts (head with snout child, body, 4 legs) × 6 faces = 42 faces
        assert_eq!(faces.len(), 42);
        assert_eq!(verts.len(), 42 * 4);
        assert_eq!(indices.len(), 42 * 6);
    }
}
