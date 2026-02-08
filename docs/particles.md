# Particles & Block Breaking

## How Minecraft Handles It

### Particle Architecture
Particles are **camera-facing billboarded quads** rendered in screen space:

```
Particle (abstract base)
  SingleQuadParticle — renders a camera-facing quad
    TextureSheetParticle — reads UV from TextureAtlasSprite
      FlameParticle, LavaParticle, DripParticle, etc.
      SimpleAnimatedParticle — cycles through sprite frames
    RisingParticle — adds friction + randomized upward motion
```

Each particle stores: position, velocity, color (RGBA), lifetime, gravity, friction, size, and rotation.

### Billboard Rendering
The quad has 4 vertices at `(+/-1, +/-1)` in local space, rotated by the camera quaternion, scaled by `quadSize`, and offset to world position. Two facing modes:
- `LOOKAT_XYZ`: Full billboard (always faces camera)
- `LOOKAT_Y`: Y-axis only billboard (for vertical effects)

### Ambient Block Particles
`Block.animateTick()` is called for ~1334 random blocks near the player each tick. Blocks that produce ambient particles:

**High Visual Impact (always visible):**
| Block | Particles | Position |
|-------|-----------|----------|
| Torch | `FLAME` + `SMOKE` | (0.5, 0.7, 0.5) fixed |
| Wall Torch | `FLAME` + `SMOKE` | Offset 0.27 from wall |
| Soul Torch | `SOUL_FIRE_FLAME` + `SMOKE` | Same as torch |
| Campfire | `CAMPFIRE_COSY_SMOKE` / `SIGNAL_SMOKE` | Rising column |
| Nether Portal | `PORTAL` (x4/tick) | Purple, directional |
| End Rod | `END_ROD` (1/5 chance) | Glowing white |
| Fire | `LARGE_SMOKE` | From burning faces |
| Candles | `SMALL_FLAME` + `SMOKE` | Per-candle wick positions |

**Medium Visual Impact:**
| Block | Particles | Notes |
|-------|-----------|-------|
| Lava surface | `LAVA` (1/100 chance) | Popping up |
| Dripping lava/water | `DRIPPING_LAVA` / `DRIPPING_WATER` | Under blocks |
| Crying Obsidian | `DRIPPING_OBSIDIAN_TEAR` | Random face, 1/5 chance |
| Cherry/Pale Oak Leaves | `CHERRY_LEAVES` / `PALE_OAK_LEAVES` | Falling pattern |
| Spore Blossom | `FALLING_SPORE_BLOSSOM` + area spores | 10-block radius |
| Enchanting Table | `ENCHANT` | From bookshelves, 1/16 chance |
| Mycelium | `MYCELIUM` (1/10 chance) | Just above block |
| Ender Chest | `PORTAL` (x3/tick) | Purple |
| Furnace/Smoker/Blast Furnace | `SMOKE` + `FLAME` | When LIT |
| Brewing Stand | `SMOKE` | Every tick |

**Low Visual Impact / State-Dependent:**
- Sculk Sensor (only when ACTIVE)
- Redstone Ore (only when LIT)
- Respawn Anchor (only when charged)
- Bubble Columns (underwater only)

### Particle Render Types
- `TERRAIN_SHEET`: Block texture atlas, translucent (block debris)
- `PARTICLE_SHEET_OPAQUE`: Particle atlas, opaque (flames)
- `PARTICLE_SHEET_TRANSLUCENT`: Particle atlas, alpha blending (smoke, souls)
- `CUSTOM`: Self-rendered (mob appearance)
- `NO_RENDER`: Invisible emitters

### Block Breaking Animation
Uses 10 crack texture stages (`destroy_stage_0.png` through `destroy_stage_9.png`):
- Rendered as a **projected decal** on the block's existing geometry
- `SheetedDecalTextureGenerator` replaces UVs with world-space projected coordinates
- `RenderType::crumbling` blend-on-top render pass
- Not relevant for static meshing (only occurs during player mining)

## Implementation Strategy

### Option A: Metadata-Only (Recommended for GLB)
Output particle spawn points as metadata (JSON sidecar or glTF extras):
```json
{
  "particles": [
    {
      "type": "minecraft:flame",
      "position": [5.5, 3.7, 10.5],
      "source_block": "minecraft:torch"
    }
  ]
}
```
Let the viewer handle billboard rendering at runtime. This is the most faithful approach since particles are inherently dynamic and camera-dependent.

### Option B: Static Marker Quads
For blocks like torches and candles, place small emissive quads at known spawn positions as visual indicators. Use 2-3 cross-intersecting quads (like Minecraft's cross-model plants) to approximate volumetric appearance.

### Option C: Point Cloud / Instanced
Export particle positions as a point cloud or instanced mesh data. The custom viewer can render them as sprites.

### Which Blocks to Support First
1. **Torches** (most common, fixed position, iconic flame)
2. **Campfire smoke** (very visible, rising column)
3. **Candles** (fixed positions per candle count)
4. **Nether Portal** (dense purple effect)
5. **End Rod** (simple glowing particle)

### Block Breaking — Not Applicable
Block breaking is a real-time gameplay mechanic with no application in static schematic meshing. The technique (projected decal textures) could theoretically be used for weathering effects.

## Source Files (MC 1.21.4)
- `Particle.java` — Base class with physics simulation
- `SingleQuadParticle.java` — Billboard quad rendering
- `TextureSheetParticle.java` — Atlas-based sprite UVs
- `ParticleEngine.java` — Particle management and rendering
- `ParticleTypes.java` — Registry of all particle types
- `ModelBakery.java` — `DESTROY_STAGES` texture paths
- `SheetedDecalTextureGenerator.java` — Block breaking decal projection
- `Block.animateTick()` — Per-block ambient particle spawning
