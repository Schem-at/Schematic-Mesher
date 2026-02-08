# Liquid Block Rendering

## How Minecraft Handles It

### Fluid State
Liquids use a `FluidState` with:
- `amount` (1-8): Fluid level. Source blocks = 8.
- `isSource()`: True for source blocks
- `FALLING`: Boolean for fluids falling straight down

Block `level` property (0-15) maps to fluid state:
- `level=0` -> source (amount=8)
- `level=1..7` -> flowing (amount = 8 - level)
- `level=8+` -> falling (amount=8, falling=true)

### Height Calculation
```
ownHeight = amount / 9.0
```

| Amount | Height | Approx Blocks |
|--------|--------|---------------|
| 8 (source) | 0.8889 | ~14.2/16 |
| 7 | 0.7778 | ~12.4/16 |
| 6 | 0.6667 | ~10.7/16 |
| 5 | 0.5556 | ~8.9/16 |
| 4 | 0.4444 | ~7.1/16 |
| 3 | 0.3333 | ~5.3/16 |
| 2 | 0.2222 | ~3.6/16 |
| 1 | 0.1111 | ~1.8/16 |

**Exception**: If the block above contains the same fluid, height = 1.0 (full block).

Constant: `MAX_FLUID_HEIGHT = 0.8888889` (8/9). Fluids never fill the full block height unless there's fluid above.

### Corner Height Averaging
The top face has 4 corners, each computed as a weighted average of the center block + 2 adjacent cardinal neighbors + the diagonal neighbor:

```
NE_height = average(center, north, east, diagonal_NE)
NW_height = average(center, north, west, diagonal_NW)
SE_height = average(center, south, east, diagonal_SE)
SW_height = average(center, south, west, diagonal_SW)
```

Neighbor height values:
- Same fluid + fluid above: 1.0
- Same fluid, no fluid above: `ownHeight`
- Non-solid (air): 0.0
- Solid block: -1.0

Weighted averaging with bias toward high values:
```
if height >= 0.8: weight = 10.0  (source blocks dominate)
if height >= 0.0: weight = 1.0   (normal contribution)
if height < 0.0:  ignored         (solid blocks don't participate)
```

If ANY adjacent neighbor has full height (1.0), the corner is forced to 1.0.

### Flow Direction
Computed as normalized horizontal vector from height differences with neighbors:
```
for each horizontal direction:
    heightDiff = ownHeight - neighborHeight
    dx += direction.x * heightDiff
    dz += direction.z * heightDiff
flow = normalize(dx, dz)
```
If flow is zero -> use still texture. Otherwise -> use flow texture with UV rotation.

### Face Generation

**Top face (UP):**
- Rendered if block above is NOT the same fluid
- Still water (no flow): UVs map [0,0]-[1,1] on still texture
- Flowing water: UVs rotated by flow angle in a 0.5x0.5 window centered on flow texture
- 4 vertices use per-corner Y heights
- Optional back-face for visibility from below

**Bottom face (DOWN):**
- Y = 0.001 epsilon offset (prevents z-fighting)
- Uses still texture

**Side faces (N/S/E/W):**
- Uses flow texture (half-width UV mapping)
- V maps fluid height on the texture
- Exception: water next to glass/leaves uses `water_overlay` texture
- Double-sided (front + back winding)
- X/Z inset by 0.001 epsilon

### Water Tinting
Water is tinted per-biome via `BiomeColors.getAverageWaterColor()`:
- Plains: `#3F76E4`
- Swamp: `#617B64`
- Ocean: varies by biome type

Lava is NOT tinted (always white/0xFFFFFF — texture used as-is).

Direction shade multipliers applied to tint:
- UP: 1.0, DOWN: 0.5, N/S: 0.8, E/W: 0.6

### Textures
| Texture | Size | Animated | Notes |
|---------|------|----------|-------|
| `water_still` | 16x16 frame | Yes (32 frames) | Used for top (still) and bottom |
| `water_flow` | 32x32 frame | Yes (32 frames) | Used for sides and top (flowing) |
| `water_overlay` | 16x16 | No | Used for sides next to transparent blocks |
| `lava_still` | 16x16 frame | Yes (20 frames) | Used for top (still) and bottom |
| `lava_flow` | 32x32 frame | Yes (32 frames) | Used for sides and top (flowing) |

For static export, use first frame of each animated texture.

## Implementation Strategy

### Phase 1: Basic Fluid Geometry
1. Detect fluid blocks: `minecraft:water[level=N]`, `minecraft:lava[level=N]`
2. Compute fluid amount from level property
3. Compute per-block height (check above for same fluid)
4. Generate flat-top geometry using `ownHeight` for all 4 corners

### Phase 2: Corner Height Averaging
1. Sample 4 cardinal neighbors + 4 diagonals
2. Apply weighted averaging algorithm (10x weight for heights >= 0.8)
3. Generate sloped top faces with per-corner Y positions

### Phase 3: Flow Direction
1. Compute flow vector from height gradients
2. Select still vs flow texture based on flow magnitude
3. Apply UV rotation on top face based on flow angle

### Face Culling
- Top: skip if above is same fluid, or above block fully occludes
- Bottom: skip if below is same fluid
- Sides: skip if neighbor is same fluid, or neighbor block fully occludes

### Transparency
- Water: transparent mesh (separate from opaque)
- Lava: opaque mesh
- Side faces: double-sided
- Top face: optional back-face

### Default Water Color
For schematics without biome data, use plains water color `#3F76E4` as default. The `TintProvider` already handles biome tinting for grass/foliage — water tinting can use the same mechanism.

## Source Files (MC 1.21.4)
- `LiquidBlockRenderer.java` — Main rendering class, builds fluid mesh
- `FlowingFluid.java` — `getFlow()`, `getOwnHeight()`, propagation logic
- `FluidState.java` — State holder for fluid properties
- `WaterFluid.java` / `LavaFluid.java` — Fluid type specifics
- `LiquidBlock.java` — Block-to-FluidState mapping
- `BiomeColors.java` — Water color lookup
