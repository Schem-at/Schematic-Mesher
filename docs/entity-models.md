# Entity Models & Block Entity Rendering

## How Minecraft Handles It

### Entity Model System

All entity models in vanilla Minecraft are **100% hardcoded in Java**. There is NO JSON format for entity models (unlike block models). Resource packs can only override textures, not geometry.

### Model Definition Hierarchy

**Builder Layer** (definition time):
- `MeshDefinition` — Top-level container, holds the root `PartDefinition`
- `PartDefinition` — Named node in the part tree. Contains `List<CubeDefinition>` + `PartPose` + children
- `CubeDefinition` — Single axis-aligned box: origin, dimensions, UV offset, CubeDeformation, mirror, visible faces
- `CubeListBuilder` — Fluent builder that accumulates cubes with shared texOffs/mirror state
- `MaterialDefinition` — Texture dimensions `(xTexSize, yTexSize)` for UV normalization
- `LayerDefinition` — Combines `MeshDefinition` + `MaterialDefinition`, has `bakeRoot()` method

**Runtime Layer** (render time):
- `ModelPart` — Mutable transform: position `(x,y,z)`, rotation `(xRot,yRot,zRot)` as Euler ZYX, scale `(xScale,yScale,zScale)`, plus `visible`, `skipDraw`. Contains `List<Cube>` + `Map<String, ModelPart>` children
- `ModelPart.Cube` — Baked geometry: up to 6 `Polygon`s (one per visible face)
- `ModelPart.Polygon` — Quad with 4 `Vertex`s + normal vector
- `ModelPart.Vertex` — `Vector3f pos`, `float u`, `float v`

### UV Box-Unwrap Layout
For a box of size `(W, H, D)` at UV origin `(u0, v0)`:
```
              u0     u0+D   u0+D+W  u0+2D+W  u0+2D+2W
               |      |       |       |        |
v0             | DOWN  |  UP   |       |        |
v0+D           |       |       |       |        |
               |       |       |       |        |
v0+D    WEST   | NORTH | EAST  | SOUTH |
v0+D+H         |       |       |       |
```
UV coordinates are normalized by dividing by texture dimensions.

### Transform Order (ModelPart.translateAndRotate)
```java
translate(x / 16.0, y / 16.0, z / 16.0);  // positions in 1/16th block units
mulPose(Quaternion.rotationZYX(zRot, yRot, xRot));  // ZYX Euler
scale(xScale, yScale, zScale);
```

### PartPose
Record with 9 floats: `(x, y, z, xRot, yRot, zRot, xScale, yScale, zScale)`. Represents the rest-state transform. `resetPose()` restores this before each animation frame.

### CubeDeformation
`(growX, growY, growZ)` — inflates the cube for overlay layers (armor=+1.0, hat=+0.5, sleeve=+0.25).

### Two Animation Systems

**Procedural (most entities)**: `setupAnim(renderState)` computes limb rotations using sin/cos on walk position and time. Example from HumanoidModel:
```java
rightArm.xRot = cos(walkPos * 0.6662 + PI) * 2.0 * walkSpeed * 0.5;
leftArm.xRot = cos(walkPos * 0.6662) * 2.0 * walkSpeed * 0.5;
```

**Keyframe (newer entities like Warden, Frog)**: `AnimationDefinition` with channels targeting `POSITION`, `ROTATION`, or `SCALE` on named bones. Uses `LINEAR` or `CATMULLROM` interpolation between keyframes.

### Coordinate System
- Positions in **1/16th block units** (divided by 16 in translateAndRotate)
- Y=0 at entity feet, Y=24 at top of standard humanoid
- Renderer applies `scale(-1, -1, 1)` and `translate(0, -1.501, 0)` before rendering

## Block Entity Renderers

These render extra geometry for special blocks. They use hardcoded `ModelPart` trees, NOT JSON models.

| Block Entity | Geometry | Texture | Complexity |
|-------------|----------|---------|------------|
| Bell | 2 boxes (body + base) | `entity/bell/bell_body` | Low |
| Bed | 2 slabs + 4 legs | Per-color `entity/bed/<color>` | Medium |
| Chest | Body + lid + lock | Per-type `entity/chest/normal` | Medium |
| Shulker Box | Base + lid | Per-color `entity/shulker/shulker_<color>` | Medium |
| Decorated Pot | Neck + 4 side planes | Per-sherd pattern textures | Medium |
| Skull (mob) | 8x8x8 box (+ hat overlay) | Known mob texture paths | Medium |
| Skull (player) | 8x8x8 box + hat | Requires skin API fetch | Medium-High |
| Lectern (book) | BookModel (6 parts) | `entity/enchanting_table_book` | Medium |
| Enchanting Table (book) | BookModel (6 parts) | `entity/enchanting_table_book` | Medium |
| Sign | Board + stick | Per-wood-type texture | Medium |
| Sign (with text) | Board + stick + font rendering | Requires font pipeline | High |
| Banner | Pole + flag | Multi-layer pattern compositing | High |
| Item Frame | JSON model frame | Arbitrary item rendering | Very High |

### Key Architectural Observation
Many blocks use **both** JSON block models AND hardcoded renderers:
- Bell: JSON frame + hardcoded bell body
- Lectern: JSON base + hardcoded BookModel
- Enchanting Table: JSON base + hardcoded floating BookModel
- Bed/Sign: Empty JSON model, entirely hardcoded renderer

## Specific Model Definitions

### Player Model (64x64 texture)
```
root
  head     (8x8x8 at texOffs(0,0), pos(0,0,0))
    hat    (8x8x8 at texOffs(32,0), +0.5 deformation)
  body     (8x12x4 at texOffs(16,16), pos(0,0,0))
    jacket (overlay, texOffs(16,32), +0.25)
  right_arm (4x12x4 at texOffs(40,16), pos(-5,2,0))
    right_sleeve (texOffs(40,32), +0.25)
  left_arm  (4x12x4 at texOffs(32,48), pos(5,2,0))
    left_sleeve (texOffs(48,48), +0.25)
  right_leg (4x12x4 at texOffs(0,16), pos(-1.9,12,0))
    right_pants (texOffs(0,32), +0.25)
  left_leg  (4x12x4 at texOffs(16,48), pos(1.9,12,0))
    left_pants (texOffs(0,48), +0.25)
```
Two variants: normal (4-wide arms) and slim (3-wide arms).

### Armor Stand Model (64x64 texture)
Uses thin 2-wide parts instead of normal body proportions, plus extra structural pieces (body sticks, shoulder stick, base plate).

### Chest Model (64x64 texture)
```
root
  bottom (14x10x14 at texOffs(0,19), pos(1,0,1))
  lid    (14x5x14 at texOffs(0,0), pos(1,9,1))  // pivots at Y=9
  lock   (2x4x1 at texOffs(0,0), pos(7,7,15))   // same pivot as lid
```
Animation: `lid.xRot = -(openness * PI/2)`. For static: openness=0 (closed).

## Implementation Strategy

### Entity Model Format
Since all models are hardcoded Java, options:
1. **Manual transcription** — Convert Java cube definitions to a Rust/JSON data format
2. **Java extractor tool** — Run `LayerDefinitions.createRoots()` and serialize to JSON
3. **Bedrock `.geo.json` support** — Different UV layout and coordinates but JSON-based
4. **Hardcode a subset** — Support the most common block entities first

### Recommended Approach: Extracted Model JSON
Write a Java tool (or use an existing one) to extract all `LayerDefinition` data from Minecraft and serialize to a JSON format:

```json
{
  "texture_width": 64,
  "texture_height": 64,
  "parts": {
    "root": {
      "cubes": [],
      "pose": { "x": 0, "y": 0, "z": 0, "rx": 0, "ry": 0, "rz": 0 },
      "children": {
        "head": {
          "cubes": [
            { "origin": [-4, -8, -4], "size": [8, 8, 8], "uv": [0, 0], "inflate": 0.0 }
          ],
          "pose": { "x": 0, "y": 0, "z": 0, "rx": 0, "ry": 0, "rz": 0 }
        }
      }
    }
  }
}
```

The mesher can then load these JSON files and generate geometry using the same cube-building logic as the existing block model element system.

### Posing API
For nucleation integration, entities need pose data:
```rust
pub struct EntityPose {
    pub entity_type: String,         // e.g. "minecraft:zombie"
    pub position: [f64; 3],          // world position
    pub body_rotation: f32,          // yaw
    pub head_rotation: [f32; 2],     // yaw, pitch
    pub limb_swing: f32,             // walk animation progress
    pub limb_swing_amount: f32,      // walk speed (0-1)
    pub custom_poses: HashMap<String, [f32; 3]>,  // bone -> (xRot, yRot, zRot)
}
```

For armor stands, the NBT directly provides per-limb rotations. For other entities, `limb_swing` + `limb_swing_amount` drive the procedural animation.

## Source Files (MC 1.21.4)
- `ModelPart.java` — Runtime model part with transform and cube geometry
- `CubeDefinition.java` — Box definition with UV and deformation
- `PartDefinition.java` — Named tree node with children
- `LayerDefinitions.java` — Registry of all model definitions
- `EntityModel.java` — Base class with `setupAnim()`
- `HumanoidModel.java` — Biped procedural animation (~110 lines)
- `KeyframeAnimations.java` — Keyframe interpolation system
