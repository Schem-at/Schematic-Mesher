use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose, SkullType};
use crate::resource_pack::TextureData;
use crate::types::{Direction, InputBlock};

/// Skull/head model.
pub(super) fn skull_model(skull_type: SkullType) -> EntityModelDef {
    // Dragon and piglin heads have their own vanilla models (DragonHeadModel,
    // PiglinHeadModel); everything else is the 8x8x8 SkullModel head.
    match skull_type {
        SkullType::Dragon => return dragon_head_model(),
        SkullType::Piglin => return piglin_head_model(),
        _ => {}
    }

    let texture_path = match skull_type {
        SkullType::Skeleton => "entity/skeleton/skeleton",
        SkullType::WitherSkeleton => "entity/skeleton/wither_skeleton",
        SkullType::Zombie => "entity/zombie/zombie",
        SkullType::Creeper => "entity/creeper/creeper",
        SkullType::Piglin => "entity/piglin/piglin",
        SkullType::Dragon => "entity/enderdragon/dragon",
        SkullType::Player => "entity/player/wide/steve", // fallback, overridden by add_player_head
    };

    let texture_size: [u32; 2] = match skull_type {
        SkullType::Piglin => [64, 64],
        SkullType::Dragon => [256, 256],
        SkullType::Zombie | SkullType::Player => [64, 64],
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

    // Hat overlay for types that have hat texture region (zombie, player).
    let has_hat = matches!(skull_type, SkullType::Zombie | SkullType::Player);
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
        is_opaque: !has_hat,
    }
}

// ── Dragon / piglin heads ───────────────────────────────────────────────────
//
// Every number below is read from the unobfuscated 26.2 client jar bytecode
// (`javap -c -p`).
//
// Placement: the shared skull root (T(8,0,8)·RotX(π)) composed with the
// skull facing matrix equals SkullBlockRenderer's createGroundTransformation /
// createWallTransformation (T · Ry(-yRot) · S(-1,-1,1)), so the vanilla model
// parts go under that root unchanged.
//
// Pose: SkullBlockRenderer.submitSkull builds a fresh SkullModelBase.State
// with only `animationPos` set (SkullBlockEntity.getAnimation, 0 for an
// unpowered head) and Model.setupAnim (resetPose + the per-model setupAnim)
// runs before drawing. State.yRot/xRot stay 0 (rotation lives in the block
// transform), so the static pose is the setupAnim result at animationPos = 0.

/// Leaf part holding one cube.
fn cube_part(cube: EntityCube, pose: EntityPartPose) -> EntityPart {
    EntityPart { cubes: vec![cube], pose, children: vec![] }
}

fn plain_cube(origin: [f32; 3], dimensions: [f32; 3], tex_offset: [u32; 2]) -> EntityCube {
    EntityCube { origin, dimensions, tex_offset, inflate: 0.0, mirror: false, skip_faces: vec![] }
}

/// The shared skull root: Y-down entity space → Y-up, centred on the block.
fn skull_root(children: Vec<EntityPart>) -> EntityPart {
    EntityPart {
        cubes: vec![],
        pose: EntityPartPose {
            position: [8.0, 0.0, 8.0],
            rotation: [std::f32::consts::PI, 0.0, 0.0],
            ..Default::default()
        },
        children,
    }
}

/// A vanilla `.mirror(true)` cube, reproduced exactly.
///
/// Vanilla's ModelPart.Cube swaps minX/maxX when mirrored and reverses each
/// polygon's vertex order, i.e. the cube is the X-reflection of its unmirrored
/// self: every face shows its UV region flipped horizontally AND the -X face
/// shows the +X region (and vice versa). `EntityCube::mirror` only does the
/// flip, so the two side faces are emitted separately with their regions
/// swapped:
///   * N/S/U/D: a mirrored cube with the side faces skipped;
///   * -X face: a mirrored cube whose West region is moved onto the vanilla
///     +X region (tex u + d + w);
///   * +X face: the West face of a mirrored cube turned 180° about Y around
///     the cube centre, which lands it on +X showing the -X region flipped.
fn vanilla_mirrored_cube(origin: [f32; 3], dimensions: [f32; 3], tex_offset: [u32; 2]) -> Vec<EntityPart> {
    use Direction::*;
    let [w, h, d] = dimensions;
    let only_west = vec![Down, Up, North, South, East];
    let mirrored = |origin, tex_offset, skip_faces| EntityCube {
        origin,
        dimensions,
        tex_offset,
        inflate: 0.0,
        mirror: true,
        skip_faces,
    };
    let centre = [origin[0] + w / 2.0, origin[1] + h / 2.0, origin[2] + d / 2.0];
    vec![
        cube_part(mirrored(origin, tex_offset, vec![West, East]), Default::default()),
        cube_part(
            mirrored(origin, [tex_offset[0] + (d + w) as u32, tex_offset[1]], only_west.clone()),
            Default::default(),
        ),
        cube_part(
            mirrored([-w / 2.0, -h / 2.0, -d / 2.0], tex_offset, only_west),
            EntityPartPose {
                position: centre,
                rotation: [0.0, std::f32::consts::PI, 0.0],
                ..Default::default()
            },
        ),
    ]
}

/// Ender dragon head — `DragonHeadModel.createHeadLayer()` (256x256 sheet,
/// `LayerDefinitions` registers it for `DRAGON_SKULL` with no MeshTransformer).
fn dragon_head_model() -> EntityModelDef {
    // addBox(name, x, y, z, w, h, d, texU, texV); mirror(true) is sticky on
    // the CubeListBuilder until mirror(false).
    let mut head_children = Vec::new();
    // mirror(true): left scale + left nostril.
    head_children.extend(vanilla_mirrored_cube([-5.0, -12.0, -4.0], [2.0, 4.0, 6.0], [0, 0]));
    head_children.extend(vanilla_mirrored_cube([-5.0, -3.0, -22.0], [2.0, 2.0, 4.0], [112, 0]));

    // "jaw": texOffs(176,65) addBox(-6,0,-16, 12,4,16), PartPose.offset(0,4,-8).
    // setupAnim: jaw.xRot = (sin(animationPos·π·0.2) + 1)·0.2 = 0.2 at rest.
    head_children.push(cube_part(
        plain_cube([-6.0, 0.0, -16.0], [12.0, 4.0, 16.0], [176, 65]),
        EntityPartPose { position: [0.0, 4.0, -8.0], rotation: [0.2, 0.0, 0.0], ..Default::default() },
    ));

    let head = EntityPart {
        cubes: vec![
            // upper_lip
            plain_cube([-6.0, -1.0, -24.0], [12.0, 5.0, 16.0], [176, 44]),
            // upper_head
            plain_cube([-8.0, -8.0, -10.0], [16.0, 16.0, 16.0], [112, 30]),
            // mirror(false): right scale + right nostril.
            plain_cube([3.0, -12.0, -4.0], [2.0, 4.0, 6.0], [0, 0]),
            plain_cube([3.0, -3.0, -22.0], [2.0, 2.0, 4.0], [112, 0]),
        ],
        // PartPose.offset(0, -7.986666, 0).scaled(0.75): PartPose.scaled(f)
        // multiplies the offset AND sets the scale; ModelPart.translateAndRotate
        // applies translate → rotateZYX → scale (same as traverse_parts).
        // head.xRot/yRot are set from State (0) by setupAnim.
        pose: EntityPartPose {
            position: [0.0, -7.986666 * 0.75, 0.0],
            rotation: [0.0, 0.0, 0.0],
            scale: [0.75, 0.75, 0.75],
        },
        children: head_children,
    };

    EntityModelDef {
        texture_path: "entity/enderdragon/dragon".to_string(),
        texture_size: [256, 256],
        parts: vec![skull_root(vec![head])],
        is_opaque: true,
    }
}

/// Piglin head — `PiglinHeadModel.createHeadModel()` = `AbstractPiglinModel.addHead(
/// CubeDeformation.NONE, mesh)`, registered as a 64x64 layer in
/// `LayerDefinitions`. There is no hat layer.
fn piglin_head_model() -> EntityModelDef {
    // setupAnim at animationPos = 0 overrides the addHead ear zRot (∓π/6):
    //   leftEar.zRot  = -(cos(0·π·0.2·1.2) + 2.5)·0.2 = -0.7
    //   rightEar.zRot =  (cos(0·π·0.2)     + 2.5)·0.2 =  0.7
    let left_ear = cube_part(
        plain_cube([0.0, 0.0, -2.0], [1.0, 5.0, 4.0], [51, 6]),
        EntityPartPose { position: [4.5, -6.0, 0.0], rotation: [0.0, 0.0, -0.7], ..Default::default() },
    );
    let right_ear = cube_part(
        plain_cube([-1.0, 0.0, -2.0], [1.0, 5.0, 4.0], [39, 6]),
        EntityPartPose { position: [-4.5, -6.0, 0.0], rotation: [0.0, 0.0, 0.7], ..Default::default() },
    );

    let head = EntityPart {
        cubes: vec![
            plain_cube([-5.0, -8.0, -4.0], [10.0, 8.0, 8.0], [0, 0]),  // head
            plain_cube([-2.0, -4.0, -5.0], [4.0, 4.0, 1.0], [31, 1]),  // snout
            plain_cube([2.0, -2.0, -5.0], [1.0, 2.0, 1.0], [2, 4]),    // tusk
            plain_cube([-3.0, -2.0, -5.0], [1.0, 2.0, 1.0], [2, 0]),   // tusk
        ],
        pose: Default::default(), // PartPose.ZERO
        children: vec![left_ear, right_ear],
    };

    EntityModelDef {
        texture_path: "entity/piglin/piglin".to_string(),
        texture_size: [64, 64],
        parts: vec![skull_root(vec![head])],
        is_opaque: true,
    }
}

/// Build a player skull model with a custom texture path.
pub(crate) fn player_skull_model(texture_path: &str) -> EntityModelDef {
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

    // Hat overlay always present for player heads
    let hat = EntityPart {
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
    };

    let root = EntityPart {
        cubes: vec![],
        pose: EntityPartPose {
            position: [8.0, 0.0, 8.0],
            rotation: [std::f32::consts::PI, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![head, hat],
    };

    EntityModelDef {
        texture_path: texture_path.to_string(),
        texture_size: [64, 64],
        parts: vec![root],
        is_opaque: false, // hat layer is transparent
    }
}

/// Decode a hex-encoded PNG skin into TextureData.
pub(crate) fn decode_hex_skin(hex_str: &str) -> Option<TextureData> {
    // Decode hex string to bytes
    let hex_clean = hex_str.trim();
    if hex_clean.len() % 2 != 0 {
        return None;
    }
    let mut bytes = Vec::with_capacity(hex_clean.len() / 2);
    for i in (0..hex_clean.len()).step_by(2) {
        let byte = u8::from_str_radix(&hex_clean[i..i + 2], 16).ok()?;
        bytes.push(byte);
    }

    png_bytes_to_texture(&bytes)
}

/// Decode a base64-encoded PNG skin into TextureData.
///
/// Accepts both standard (+/) and URL-safe (-_) alphabets, with or without
/// padding. This is the intended path for WASM/JS callers: resolve a username
/// to a skin PNG in JavaScript (via Mojang's session API), encode as base64,
/// and pass it through the `skin_base64` block property.
pub(crate) fn decode_base64_skin(b64: &str) -> Option<TextureData> {
    let bytes = decode_base64(b64)?;
    png_bytes_to_texture(&bytes)
}

fn png_bytes_to_texture(bytes: &[u8]) -> Option<TextureData> {
    let img = image::load_from_memory(bytes).ok()?;
    let rgba = img.to_rgba8();
    Some(TextureData {
        width: rgba.width(),
        height: rgba.height(),
        pixels: rgba.into_raw(),
        is_animated: false,
        frame_count: 1,
        animation: None,
    })
}

/// Base64 decoder. Supports both `+/` and URL-safe `-_` alphabets; `=` padding
/// is tolerated but not required. Returns None on invalid characters.
fn decode_base64(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;
    for b in s.bytes() {
        let v: u32 = match b {
            b'A'..=b'Z' => (b - b'A') as u32,
            b'a'..=b'z' => (b - b'a' + 26) as u32,
            b'0'..=b'9' => (b - b'0' + 52) as u32,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => continue,
            c if c.is_ascii_whitespace() => continue,
            _ => return None,
        };
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1u32 << bits).wrapping_sub(1);
        }
    }
    Some(out)
}

/// Vanilla's default skins in `DefaultPlayerSkin.DEFAULT_SKINS` order
/// (26.2 client): nine slim, then the same nine wide. `true` = slim.
const DEFAULT_SKINS: [(&str, bool); 18] = [
    ("entity/player/slim/alex", true),
    ("entity/player/slim/ari", true),
    ("entity/player/slim/efe", true),
    ("entity/player/slim/kai", true),
    ("entity/player/slim/makena", true),
    ("entity/player/slim/noor", true),
    ("entity/player/slim/steve", true),
    ("entity/player/slim/sunny", true),
    ("entity/player/slim/zuri", true),
    ("entity/player/wide/alex", false),
    ("entity/player/wide/ari", false),
    ("entity/player/wide/efe", false),
    ("entity/player/wide/kai", false),
    ("entity/player/wide/makena", false),
    ("entity/player/wide/noor", false),
    ("entity/player/wide/steve", false),
    ("entity/player/wide/sunny", false),
    ("entity/player/wide/zuri", false),
];

/// Parse a UUID (hyphenated or 32 hex digits) into Java's (most, least) halves.
pub(crate) fn parse_uuid(s: &str) -> Option<(u64, u64)> {
    let hex: String = s.chars().filter(|c| *c != '-').collect();
    if hex.len() != 32 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let most = u64::from_str_radix(&hex[..16], 16).ok()?;
    let least = u64::from_str_radix(&hex[16..], 16).ok()?;
    Some((most, least))
}

/// Vanilla `DefaultPlayerSkin.get(UUID)`: `DEFAULT_SKINS[floorMod(uuid.hashCode(), 18)]`,
/// where `UUID.hashCode()` folds `most ^ least` to 32 bits. Returns `(texture path, slim)`.
pub(crate) fn default_player_skin(uuid: &str) -> Option<(&'static str, bool)> {
    let (most, least) = parse_uuid(uuid)?;
    let hilo = most ^ least;
    let hash = ((hilo >> 32) as i32) ^ (hilo as i32);
    Some(DEFAULT_SKINS[hash.rem_euclid(DEFAULT_SKINS.len() as i32) as usize])
}

/// The resource-pack skin to use when a player has no explicit skin: vanilla's
/// default for its `uuid`, or wide Steve without one.
pub(crate) fn player_skin_fallback_path(block: &InputBlock) -> &'static str {
    block
        .properties
        .get("uuid")
        .and_then(|u| default_player_skin(u))
        .map(|(path, _)| path)
        .unwrap_or("entity/player/wide/steve")
}

/// Texture paths to try, in order, for a player skin that is missing from the
/// pack: packs before 1.19.3 only ship `entity/steve` and `entity/alex`.
pub(crate) const PLAYER_SKIN_LAST_RESORTS: [&str; 2] = ["entity/player/wide/steve", "entity/steve"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_player_head() {
        let block = InputBlock::new("minecraft:player_head");
        let entity = super::super::detect_block_entity(&block);
        assert!(matches!(entity, Some(super::super::BlockEntityType::Skull(SkullType::Player))));
    }

    #[test]
    fn test_detect_player_wall_head() {
        let block = InputBlock::new("minecraft:player_wall_head");
        let entity = super::super::detect_block_entity(&block);
        assert!(matches!(entity, Some(super::super::BlockEntityType::Skull(SkullType::Player))));
    }

    #[test]
    fn test_decode_hex_skin_invalid() {
        assert!(decode_hex_skin("not_hex").is_none());
        assert!(decode_hex_skin("abc").is_none()); // odd length
        assert!(decode_hex_skin("0000").is_none()); // valid hex but not a PNG
    }

    #[test]
    fn test_decode_hex_skin_valid() {
        // Create a minimal 1x1 PNG and hex-encode it
        use image::{ImageBuffer, Rgba};
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_pixel(1, 1, Rgba([255, 0, 0, 255]));
        let mut buf = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut buf);
        img.write_to(&mut cursor, image::ImageFormat::Png).unwrap();
        let hex: String = buf.iter().map(|b| format!("{:02x}", b)).collect();

        let tex = decode_hex_skin(&hex).unwrap();
        assert_eq!(tex.width, 1);
        assert_eq!(tex.height, 1);
    }

    #[test]
    fn test_player_skin_fallback_steve() {
        let block = InputBlock::new("minecraft:player_head");
        assert_eq!(player_skin_fallback_path(&block), "entity/player/wide/steve");

        // Not a UUID → Steve
        let block = InputBlock::new("minecraft:player_head")
            .with_property("uuid", "2a3b4c5d");
        assert_eq!(player_skin_fallback_path(&block), "entity/player/wide/steve");
    }

    /// Expected values from Java: `Math.floorMod(UUID.fromString(s).hashCode(), 18)`.
    #[test]
    fn test_default_skin_matches_java_uuid_hash() {
        // Values checked with jshell on JDK 25.
        assert_eq!(
            default_player_skin("069a79f4-44e9-4726-a5be-fca90e38aaf5"),
            Some(("entity/player/slim/alex", true))
        );
        assert_eq!(
            default_player_skin("853c80ef-3c37-49fd-aa49-938b674adae6"),
            Some(("entity/player/slim/ari", true))
        );
        // All-zero UUID hashes to 0 → slim Alex.
        assert_eq!(
            default_player_skin("00000000000000000000000000000000"),
            Some(("entity/player/slim/alex", true))
        );
        // most=0, least=0x0000000f_00000000 → hashCode 15 → wide Steve.
        assert_eq!(
            default_player_skin("00000000-0000-0000-0000-000f00000000"),
            Some(("entity/player/wide/steve", false))
        );
        assert_eq!(default_player_skin("not-a-uuid"), None);
    }

    #[test]
    fn test_player_skull_geometry_count() {
        // Player skull: head (6 faces) + hat overlay (6 faces) = 12 faces
        let model = player_skull_model("test_texture");
        fn count_cubes(parts: &[super::super::EntityPart]) -> usize {
            let mut count = 0;
            for part in parts {
                count += part.cubes.len();
                count += count_cubes(&part.children);
            }
            count
        }
        let total_cubes = count_cubes(&model.parts);
        assert_eq!(total_cubes, 2); // head + hat
        // Each cube = 6 faces, so 12 faces total
        assert_eq!(total_cubes * 6, 12);
    }
}

#[cfg(test)]
mod dragon_piglin_tests {
    use super::super::{generate_entity_geometry, traverse_parts, BlockEntityType, EntityPart, SkullType};
    use super::*;
    use crate::mesher::geometry::Vertex;
    use glam::Mat4;

    fn count_cubes(parts: &[EntityPart]) -> usize {
        parts.iter().map(|p| p.cubes.len() + count_cubes(&p.children)).sum()
    }

    fn mesh(block: &InputBlock, ty: SkullType) -> Vec<Vertex> {
        generate_entity_geometry(block, &BlockEntityType::Skull(ty)).0
    }

    fn bounds(verts: &[Vertex]) -> ([f32; 3], [f32; 3]) {
        let mut lo = [f32::MAX; 3];
        let mut hi = [f32::MIN; 3];
        for v in verts {
            for i in 0..3 {
                lo[i] = lo[i].min(v.position[i]);
                hi[i] = hi[i].max(v.position[i]);
            }
        }
        (lo, hi)
    }

    /// Normal of the unique quad whose UV rectangle starts at `uv_min` (pixels).
    fn face_normal(verts: &[Vertex], uv_min: [f32; 2], tex: [f32; 2]) -> [f32; 3] {
        let hits: Vec<[f32; 3]> = verts
            .chunks(4)
            .filter(|q| {
                let u = q.iter().map(|v| v.uv[0]).fold(f32::MAX, f32::min);
                let v = q.iter().map(|v| v.uv[1]).fold(f32::MAX, f32::min);
                (u * tex[0] - uv_min[0]).abs() < 1e-3 && (v * tex[1] - uv_min[1]).abs() < 1e-3
            })
            .map(|q| q[0].normal)
            .collect();
        assert_eq!(hits.len(), 1, "expected one quad at uv {:?}", uv_min);
        hits[0]
    }

    fn assert_vec_eq(a: [f32; 3], b: [f32; 3], what: &str) {
        for i in 0..3 {
            assert!((a[i] - b[i]).abs() < 1e-4, "{what}: {a:?} != {b:?}");
        }
    }

    /// Every placement the tests exercise: standing rotations 0..15 and wall facings.
    fn placements(kind: &str) -> Vec<InputBlock> {
        let mut out: Vec<InputBlock> = (0..16)
            .map(|r| InputBlock::new(&format!("minecraft:{kind}_head")).with_property("rotation", &r.to_string()))
            .collect();
        for f in ["north", "south", "east", "west"] {
            out.push(InputBlock::new(&format!("minecraft:{kind}_wall_head")).with_property("facing", f));
        }
        out
    }

    fn zombie_for(block: &InputBlock) -> InputBlock {
        let id = block.block_id().replace("dragon", "zombie").replace("piglin", "zombie");
        let mut z = InputBlock::new(&id);
        for (k, v) in &block.properties {
            z = z.with_property(k, v);
        }
        z
    }

    #[test]
    fn dragon_head_geometry_counts() {
        let model = skull_model(SkullType::Dragon);
        assert_eq!(model.texture_size, [256, 256]);
        assert_eq!(model.texture_path, "entity/enderdragon/dragon");
        // upper_lip, upper_head, 2 right scale/nostril, jaw + 2 mirrored cubes × 3 pieces.
        assert_eq!(count_cubes(&model.parts), 5 + 2 * 3);
        // 5 full cubes (30 faces) + 2 mirrored cubes (6 faces each).
        let verts = mesh(&InputBlock::new("minecraft:dragon_head"), SkullType::Dragon);
        assert_eq!(verts.len(), (30 + 12) * 4);
    }

    #[test]
    fn piglin_head_geometry_counts() {
        let model = skull_model(SkullType::Piglin);
        assert_eq!(model.texture_size, [64, 64]);
        assert!(model.is_opaque, "vanilla piglin head has no hat layer");
        // head, snout, 2 tusks, 2 ears
        assert_eq!(count_cubes(&model.parts), 6);
        let verts = mesh(&InputBlock::new("minecraft:piglin_head"), SkullType::Piglin);
        assert_eq!(verts.len(), 36 * 4);
    }

    #[test]
    fn dragon_head_bounds_rotation_0() {
        let (lo, hi) = bounds(&mesh(
            &InputBlock::new("minecraft:dragon_head").with_property("rotation", "0"),
            SkullType::Dragon,
        ));
        let head_y = -7.986666_f32 * 0.75;
        // x: upper_head ±8 px scaled 0.75.
        assert!((lo[0] - (0.5 - 6.0 / 16.0)).abs() < 1e-4, "{lo:?}");
        assert!((hi[0] - (0.5 + 6.0 / 16.0)).abs() < 1e-4, "{hi:?}");
        // top: scales at y = -12 (Y-down) in head space.
        assert!((hi[1] - (-(-12.0 * 0.75 + head_y)) / 16.0).abs() < 1e-4, "{hi:?}");
        // bottom: jaw's front-bottom edge, (y=4, z=-16) in jaw space, rotated by
        // xRot 0.2 (y' = y·cos − z·sin) about the jaw pivot at head-space y=4.
        // Vanilla's open jaw dips ~2.3 px below the block it stands on.
        let (s, c) = 0.2_f32.sin_cos();
        let jaw_y = 4.0 + (4.0 * c - (-16.0) * s);
        assert!((lo[1] - (-(jaw_y * 0.75 + head_y)) / 16.0).abs() < 1e-4, "{lo:?}");
        // rotation 0 faces north (-Z): snout (upper_lip front, z=-24) out of the block,
        // back of upper_head (z=+6) behind the centre.
        assert!((lo[2] - (0.5 - 24.0 * 0.75 / 16.0)).abs() < 1e-4, "{lo:?}");
        assert!((hi[2] - (0.5 + 6.0 * 0.75 / 16.0)).abs() < 1e-4, "{hi:?}");
    }

    #[test]
    fn piglin_head_bounds_rotation_0() {
        let (lo, hi) = bounds(&mesh(
            &InputBlock::new("minecraft:piglin_head").with_property("rotation", "0"),
            SkullType::Piglin,
        ));
        // Ears: pivot x=±4.5, far corner (1,5) rotated by zRot ∓0.7.
        let (s, c) = 0.7_f32.sin_cos();
        let ear_x = 4.5 + c + 5.0 * s;
        assert!((lo[0] - (0.5 - ear_x / 16.0)).abs() < 1e-4, "{lo:?}");
        assert!((hi[0] - (0.5 + ear_x / 16.0)).abs() < 1e-4, "{hi:?}");
        assert!(lo[1].abs() < 1e-4 && (hi[1] - 0.5).abs() < 1e-4, "{lo:?} {hi:?}");
        // snout front at z=-5 → north for rotation 0
        assert!((lo[2] - (0.5 - 5.0 / 16.0)).abs() < 1e-4, "{lo:?}");
        assert!((hi[2] - (0.5 + 4.0 / 16.0)).abs() < 1e-4, "{hi:?}");
    }

    /// The dragon's snout and the piglin's snout face the same way as the
    /// zombie's face for every rotation and wall facing.
    #[test]
    fn faces_match_zombie_orientation() {
        for (kind, ty, uv, tex) in [
            // upper_lip north region: (176+16, 44+16)
            ("dragon", SkullType::Dragon, [192.0, 60.0], [256.0, 256.0]),
            // snout north region: (31+1, 1+1)
            ("piglin", SkullType::Piglin, [32.0, 2.0], [64.0, 64.0]),
        ] {
            for block in placements(kind) {
                let zombie = mesh(&zombie_for(&block), SkullType::Zombie);
                // zombie head front: north region of texOffs(0,0) 8x8x8 → (8, 8)
                let want = face_normal(&zombie, [8.0, 8.0], [64.0, 64.0]);
                let got = face_normal(&mesh(&block, ty), uv, tex);
                assert_vec_eq(got, want, &format!("{} {:?}", block.block_id(), block.properties));
            }
        }
    }

    /// Wall heads sit on the same block-space anchor as the zombie wall head:
    /// the back of the head (model +Z side of the 8-px-deep reference box) is
    /// flush with the wall for the piglin, exactly like the zombie.
    #[test]
    fn piglin_wall_head_flush_like_zombie() {
        for f in ["north", "south", "east", "west"] {
            let b = InputBlock::new("minecraft:piglin_wall_head").with_property("facing", f);
            let (plo, phi) = bounds(&mesh(&b, SkullType::Piglin));
            let (zlo, zhi) = bounds(&mesh(&zombie_for(&b), SkullType::Zombie));
            let (axis, back_is_max) = match f {
                "north" => (2, true),
                "south" => (2, false),
                "east" => (0, false),
                _ => (0, true),
            };
            // zombie includes a 0.25 inflated hat; compare to its inner head plane.
            let hat = 0.25 / 16.0;
            if back_is_max {
                assert!((phi[axis] - (zhi[axis] - hat)).abs() < 1e-4, "{f}");
            } else {
                assert!((plo[axis] - (zlo[axis] + hat)).abs() < 1e-4, "{f}");
            }
            assert!((plo[1] - (zlo[1] + hat)).abs() < 1e-4, "{f} y");
        }
    }

    type Quad = Vec<([i32; 3], [i32; 2])>;

    fn quads(parts: &[EntityPart], tex: [u32; 2], reflect_x_about: Option<f32>) -> Vec<(Quad, [i32; 3])> {
        let model = EntityModelDef {
            texture_path: String::new(),
            texture_size: tex,
            parts: parts.to_vec(),
            is_opaque: true,
        };
        let (mut v, mut i, mut f) = (Vec::new(), Vec::new(), Vec::new());
        traverse_parts(&model.parts, Mat4::IDENTITY, &Mat4::IDENTITY, &model, &mut v, &mut i, &mut f);
        let r = |x: f32| (x * 4096.0).round() as i32;
        let mut out: Vec<(Quad, [i32; 3])> = v
            .chunks(4)
            .map(|q| {
                let mut quad: Quad = q
                    .iter()
                    .map(|vx| {
                        let mut p = vx.position;
                        if let Some(c) = reflect_x_about {
                            p[0] = 2.0 * c - p[0];
                        }
                        ([r(p[0]), r(p[1]), r(p[2])], [r(vx.uv[0]), r(vx.uv[1])])
                    })
                    .collect();
                quad.sort();
                let mut n = q[0].normal;
                if reflect_x_about.is_some() {
                    n[0] = -n[0];
                }
                (quad, [r(n[0]), r(n[1]), r(n[2])])
            })
            .collect();
        out.sort();
        out
    }

    /// `vanilla_mirrored_cube` must be exactly the X-reflection (about the cube
    /// centre) of the unmirrored cube, textures included — that is what
    /// vanilla's ModelPart.Cube does when `mirror` is set (swap minX/maxX and
    /// reverse polygon vertex order).
    #[test]
    fn vanilla_mirrored_cube_is_reflection_of_plain_cube() {
        for (origin, dims, tex) in [
            ([-5.0, -12.0, -4.0], [2.0, 4.0, 6.0], [0u32, 0u32]),
            ([-5.0, -3.0, -22.0], [2.0, 2.0, 4.0], [112, 0]),
        ] {
            let centre_x = (origin[0] + dims[0] / 2.0) / 16.0;
            let plain = vec![cube_part(plain_cube(origin, dims, tex), Default::default())];
            let want = quads(&plain, [256, 256], Some(centre_x));
            let got = quads(&vanilla_mirrored_cube(origin, dims, tex), [256, 256], None);
            assert_eq!(got.len(), 6);
            assert_eq!(got, want, "mirrored cube at {origin:?}");
        }
    }
}
