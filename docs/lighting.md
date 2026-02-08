# Lighting System

## How Minecraft Handles It

### Two Light Channels
Minecraft maintains two independent light values per block, both in range **0-15**:

1. **Block Light** — Emitted by light-emitting blocks (torches, glowstone, etc.). Propagated via BFS flood-fill.
2. **Sky Light** — Natural daylight from above. Propagates vertically with zero attenuation, then horizontally with standard decay.

### Light Emission
Light emission is a `BlockState` property, set via `lightLevel()` in block registration. Purely determined from block name + properties.

**Constant emitters:**
| Level | Blocks |
|-------|--------|
| 15 | Lava, Fire, Glowstone, Jack o'Lantern, Sea Lantern, End Portal/Gateway, Beacon, Conduit, Shroomlight, Lantern, Froglights |
| 14 | Torch, End Rod |
| 11 | Nether Portal |
| 10 | Soul Fire, Soul Torch, Soul Lantern, Crying Obsidian |
| 7 | Enchanting Table, Ender Chest, Glow Lichen |
| 6 | Sculk Catalyst |
| 5 | Amethyst Cluster |
| 4 | Large Amethyst Bud |
| 3 | Magma Block |
| 2 | Medium Amethyst Bud |
| 1 | Brown Mushroom, Brewing Stand, End Portal Frame, Dragon Egg, Small Amethyst Bud, Sculk Sensor |

**State-dependent emitters:**
- `litBlockEmission(N)`: Returns N when `LIT=true`, else 0. Furnace(13), Smoker(13), Blast Furnace(13), Redstone Lamp(15), Campfire(15), Soul Campfire(10).
- Candles: `3 * count` when `LIT=true` (3, 6, 9, or 12)
- Sea Pickle: `3 + 3 * count` when alive (6, 9, 12, or 15)
- Cave Vines: 14 when `BERRIES=true`
- Light Block: Equal to `LEVEL` property (0-15)
- Respawn Anchor: Scaled by charge (0-15)
- Copper Bulbs: 15/12/8/4 by weathering stage

### Light Propagation

**Block Light BFS:**
```
for each block with emission > 0:
    enqueue(block, emission_level)

while queue not empty:
    (pos, level) = dequeue()
    for each of 6 neighbors:
        opacity = max(1, neighbor.getLightBlock())
        new_level = level - opacity
        if new_level > stored_level(neighbor):
            set_level(neighbor, new_level)
            enqueue(neighbor, new_level)
```

Key: Light always decreases by at least 1 per block (even through air), giving a maximum range of 15 blocks. Solid opaque blocks have `getLightBlock() = 15` and stop light entirely.

**Sky Light:**
1. Start at level 15 from above the schematic
2. Propagate downward with zero attenuation through transparent columns
3. Everything above the first opaque block in each column gets sky=15
4. Horizontal spread uses the same BFS as block light (decays by at least 1 per step)

### Opacity Categories
- **Solid opaque** (stone, dirt, etc.): opacity 15 — blocks all light
- **Transparent + propagates skylight** (air, glass): opacity 0 (but min 1 per step)
- **Translucent** (water, etc.): opacity 1

### Brightness Curve
Light level (0-15) maps to brightness (0.0-1.0) via:
```
brightness = ratio / (4.0 - 3.0 * ratio)    where ratio = level / 15.0
```
With ambient light factor (overworld = 0.0, nether = 0.1):
```
final = lerp(ambientLight, brightness, 1.0)
```

| Level | Overworld | Nether |
|-------|-----------|--------|
| 0 | 0.000 | 0.100 |
| 4 | 0.083 | 0.175 |
| 7 | 0.187 | 0.268 |
| 10 | 0.333 | 0.400 |
| 13 | 0.619 | 0.657 |
| 15 | 1.000 | 1.000 |

### Direction-Based Shading
Per-face shade multipliers (already partially implemented as AO shading):
| Direction | Overworld | Nether |
|-----------|-----------|--------|
| UP | 1.0 | 0.9 |
| DOWN | 0.5 | 0.9 |
| NORTH/SOUTH | 0.8 | 0.8 |
| EAST/WEST | 0.6 | 0.6 |

### AO + Lighting Interaction
- **Light-emitting blocks skip AO entirely** and use flat shading
- AO samples light values from 4 corner neighbors + 4 diagonal neighbors per face
- Uses max-biased averaging: zero-light neighbors are replaced with center light to prevent dark edges

### Data Storage
Light data is stored as **nibble arrays** (4 bits per block, 2048 bytes per 16x16x16 section). Schematics do NOT store light data — it must be computed.

## Implementation Strategy

### Phase 1: Block Light (Self-Contained)
1. Build a lookup table: `(block_name, properties) -> emission_level`
2. BFS flood-fill from all emitting blocks within schematic bounds
3. Store as a 3D `Vec<u8>` indexed by block position
4. Formula: `new_light = current - max(1, opacity_at(neighbor))`
5. This is entirely self-contained within the schematic

### Phase 2: Sky Light (Policy Decision)
1. Config option: `sky_light: bool` (default true) — assume open sky above schematic
2. Heightmap pass: for each (x,z) column, find highest opaque block
3. Everything above gets sky=15
4. Horizontal BFS spread same as block light

### Phase 3: Brightness Baking
1. `combined = max(block_brightness, sky_brightness)` using the brightness curve
2. Per-face: `final = combined * direction_shade * ao_factor`
3. Bake into vertex colors (multiplicative with existing AO)

### Integration with Existing AO
The current AO system already handles `direction_shade * ao_factor`. Light adds another multiplicative layer. Since both are multiplicative, they can be combined:
```
final_brightness = light_brightness * ao_brightness * direction_shade
```

### Config
```rust
pub struct LightingConfig {
    pub compute_block_light: bool,    // default true
    pub compute_sky_light: bool,      // default true
    pub sky_light_level: u8,          // default 15 (daytime)
    pub ambient_light: f32,           // default 0.0 (overworld)
    pub emissive_fullbright: bool,    // default true (skip AO for emitters)
}
```

### Data Required
No additional resource pack data needed. Light emission and opacity are block state properties — a hardcoded lookup table suffices. The mesher already tracks block state properties for model resolution.

## Source Files (MC 1.21.4)
- `Blocks.java` — Light emission values per block
- `BlockBehaviour.java` — `getLightBlock()`, `getShadeBrightness()`
- `LightEngine.java` — Abstract BFS propagation
- `BlockLightEngine.java` — Block light propagation
- `SkyLightEngine.java` — Sky light propagation
- `LightTexture.java` — Lightmap texture generation, brightness curve
- `LevelRenderer.java` — `getLightColor()` sampling
- `ClientLevel.java` — Direction shade values
- `DataLayer.java` — Nibble array storage
