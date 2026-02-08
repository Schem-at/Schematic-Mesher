# Architecture: Unified Rendering System

## Design Philosophy

The goal is a **high-fidelity rendering pipeline** with **minimal special cases**. Minecraft's rendering is split between:
1. **JSON block models** — data-driven, already supported by the mesher
2. **Hardcoded Java entity models** — procedural geometry using the ModelPart/CubeDefinition system
3. **Custom renderers** — liquid rendering, particles, etc.

The key insight is that hardcoded entity models use the **exact same primitives** as block models: axis-aligned boxes with UV mapping, positioned and rotated in a hierarchy. The mesher can unify these under a single geometry pipeline.

## Unified Model Format

All geometry sources (block models, entity models, block entity models) can be represented as:

```rust
/// A model definition that can represent both block and entity geometry.
struct ModelDefinition {
    texture_width: u32,
    texture_height: u32,
    texture_path: String,
    parts: HashMap<String, PartDefinition>,
}

struct PartDefinition {
    cubes: Vec<CubeDefinition>,
    pose: PartPose,
    children: HashMap<String, PartDefinition>,
}

struct CubeDefinition {
    origin: [f32; 3],       // in 1/16th block units
    dimensions: [f32; 3],   // in 1/16th block units
    uv_offset: [u32; 2],    // texOffs
    inflate: f32,           // CubeDeformation (uniform grow)
    mirror: bool,
    visible_faces: HashSet<Direction>,  // default: all 6
}

struct PartPose {
    position: [f32; 3],     // in 1/16th block units
    rotation: [f32; 3],     // Euler ZYX in radians
    scale: [f32; 3],        // default [1,1,1]
}
```

This is the same structure used by Minecraft's `MeshDefinition`/`PartDefinition`/`CubeDefinition` hierarchy. The UV box-unwrap layout is deterministic given the cube dimensions and texture size.

## Data Sources

### Already Supported
- **Block models**: JSON from resource pack (`assets/minecraft/models/block/`)
- **Block states**: JSON mapping properties to model variants
- **Block textures**: PNG from resource pack
- **Atlas building**: Packing block textures into atlas

### Needs Extraction (One-Time)
- **Entity/block entity models**: Java -> JSON extraction tool
  - Run `LayerDefinitions.createRoots()` to get all `LayerDefinition`s
  - Serialize part hierarchy + cube definitions to JSON
  - Store in `models/entity/` directory alongside resource pack
  - ~100+ entity models, ~15 block entity models

### Needs Implementation
- **Fluid geometry**: Custom renderer (see [liquid-rendering.md](liquid-rendering.md))
- **Animated textures**: .mcmeta parsing + sprite sheet handling (see [animated-textures.md](animated-textures.md))
- **Lighting**: BFS light propagation + brightness baking (see [lighting.md](lighting.md))
- **Particles**: Metadata export or marker quads (see [particles.md](particles.md))

## Rendering Pipeline (Extended)

```
ResourcePack
  -> StateResolver (block states -> models)
  -> ModelResolver (model JSON -> elements)
  -> EntityModelLoader (entity JSON -> parts)  [NEW]
  -> FluidRenderer (fluid states -> geometry)  [NEW]
  -> LightEngine (block/sky light propagation) [NEW]
  -> MeshBuilder (elements/parts -> vertices)
  -> FaceCuller (hidden face removal)
  -> GreedyMesher (face merging)
  -> AtlasBuilder (texture packing)
  -> LightBaker (vertex color modulation)       [NEW]
  -> Export (GLB/OBJ/USD)
```

## Nucleation Integration API

The mesher currently takes a `BlockSource` trait for block data. For entities, we need additional data:

```rust
/// Provides entity data from the schematic.
pub trait EntitySource {
    /// Iterate all entities in the schematic.
    fn iter_entities(&self) -> Box<dyn Iterator<Item = &EntityData>>;

    /// Iterate all block entities (chests, signs, etc.)
    fn iter_block_entities(&self) -> Box<dyn Iterator<Item = (BlockPosition, &BlockEntityData)>>;
}

pub struct EntityData {
    pub entity_type: String,          // e.g. "minecraft:zombie"
    pub position: [f64; 3],
    pub rotation: [f32; 2],           // yaw, pitch
    pub nbt: HashMap<String, NbtValue>,
}

pub struct BlockEntityData {
    pub entity_type: String,          // e.g. "minecraft:chest"
    pub nbt: HashMap<String, NbtValue>,
}
```

Nucleation already parses schematic NBT — it would implement these traits to pass entity data to the mesher.

### What Nucleation Needs to Provide
- Block entity NBT for: chests (facing, type), beds (color, part), signs (text, color), banners (patterns), skulls (profile), decorated pots (sherds), shulker boxes (color), lecterns (has_book), bells, enchanting tables
- Entity NBT for: armor stands (pose), item frames (item, rotation), mobs (position, type)
- Biome data for: water tinting, grass/foliage tinting

## Priority Order for Implementation

Based on visual impact, complexity, and how often these appear in schematics:

### Tier 1: High Impact, Moderate Complexity
1. **Liquid blocks** — Water and lava appear in many builds
2. **Lighting** — Dramatically improves visual fidelity
3. **Animated textures** — Water, lava, fire, portal are iconic

### Tier 2: Common Block Entities
4. **Chests** — Extremely common, simple geometry
5. **Beds** — Common in builds, simple per-color textures
6. **Signs** — Very common (geometry only, text is optional)
7. **Bells** — Simple geometry, appears in villages
8. **Skulls** — Popular decorative element

### Tier 3: Complex Block Entities
9. **Banners** — Complex pattern compositing
10. **Decorated Pots** — Moderate complexity
11. **Shulker Boxes** — Similar to chests
12. **Lectern/Enchanting Table books** — BookModel geometry

### Tier 4: Entities
13. **Armor Stands** — Common, poses from NBT
14. **Item Frames** — Requires item model rendering
15. **Mob entities** — Very complex, many model types

### Tier 5: Polish
16. **Particles** — Metadata export or marker quads
17. **Block breaking** — Not applicable for static
18. **Inventory rendering** — Niche use case
