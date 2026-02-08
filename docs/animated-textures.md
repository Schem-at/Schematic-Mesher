# Animated Textures

## How Minecraft Handles It

### .mcmeta Format
Animated textures use a companion `.png.mcmeta` file alongside the PNG. The PNG is a vertical strip where each frame is stacked top-to-bottom. The `.mcmeta` file contains an `"animation"` section:

```json
{
  "animation": {
    "frametime": 2,
    "interpolate": false,
    "frames": [0, 1, 2, 3, 2, 1]
  }
}
```

Fields:
- `frametime` (int, default 1): Ticks per frame (1 tick = 50ms, so frametime=2 = 100ms)
- `interpolate` (bool, default false): Per-pixel ARGB lerp between adjacent frames
- `frames` (optional list): Explicit frame order. Each entry is either an int (frame index) or `{"index": N, "time": T}` for per-frame duration. If omitted, frames play 0..N-1 in order.
- `width`/`height` (optional ints): Override frame dimensions

### Frame Size Calculation
```
if width AND height specified -> (width, height)
if only width              -> (width, image_height)
if only height             -> (image_width, height)
else                       -> min(image_width, image_height) squared
```
Standard case: 16px wide strip -> frames are 16x16.

### Frame Layout in Source Image
```
frame_x(i) = i % frames_per_row
frame_y(i) = i / frames_per_row
pixel_offset = (frame_x * frame_width, frame_y * frame_height)
```
Most textures have 1 frame per row (vertical strip), so `frame_x = 0`, `frame_y = i`.

### Key Textures (All Standard Animated PNGs)
Modern Minecraft (1.21.4) has **no procedural textures**. Water, lava, fire, and portal are all animated strip PNGs:

| Texture | Size | Frames | Notes |
|---------|------|--------|-------|
| `water_still` | 16x512 | 32 | Biome-tinted |
| `water_flow` | 32x1024 | 32 | 2x wider for flow pattern |
| `lava_still` | 16x320 | 20 | |
| `lava_flow` | 32x1024 | 32 | |
| `fire_0`, `fire_1` | 16xN | varies | Two fire layers |
| `nether_portal` | 16xN | varies | Purple swirl |
| `prismarine` | 16xN | varies | Color-shifting |
| `sea_lantern` | 16xN | varies | Pulsing glow |
| `magma` | 16xN | varies | Flowing surface |

### Interpolation
When `interpolate=true`, Minecraft does per-pixel ARGB linear interpolation between current and next frame based on `subFrame / frameDuration` progress. This produces smooth transitions but is purely a runtime GPU operation.

## Implementation Strategy

### For Static Export (Current)
Use frame 0 (or the first frame in the explicit frames list). This is what Minecraft's `uploadFirstFrame()` does during atlas building. **No changes needed** — the current mesher already does this implicitly since it only reads the first frame-sized region.

### For Animated Export (GLB)
Use **KHR_animation_pointer + KHR_texture_transform** to animate UV offsets across a sprite sheet:

1. **Parse .mcmeta** files during resource pack loading. Data structure:
   ```rust
   struct AnimationMeta {
       frames: Option<Vec<AnimFrame>>,
       width: Option<u32>,
       height: Option<u32>,
       frametime: u32,       // default 1
       interpolate: bool,    // default false
   }
   struct AnimFrame {
       index: u32,
       time: Option<u32>,    // overrides frametime
   }
   ```

2. **Build sprite sheet atlas**: Instead of extracting frame 0, include all frames arranged in a grid. Each animated texture gets its own material with the full strip as the texture.

3. **Generate glTF animation**: Use `KHR_animation_pointer` targeting `KHR_texture_transform.offset` on the material's baseColorTexture. Create keyframes that step through frame positions at the correct timing.

4. **Interpolation**: For `interpolate=true`, use LINEAR interpolation between keyframes. For `interpolate=false`, use STEP interpolation.

### Viewer Support Considerations
- `KHR_animation_pointer` is ratified (2024) but viewer support varies
- Three.js supports it; Blender's glTF importer may not yet
- A custom viewer (planned) can implement this directly
- Fallback: viewers that don't support it will show frame 0 (graceful degradation)

## Data Flow

```
Resource Pack (.png + .mcmeta)
  -> Parse animation metadata
  -> Extract frame dimensions
  -> Build sprite sheet (all frames) OR single frame
  -> Atlas placement
  -> GLB animation channels (if animated export)
```

## Source Files (MC 1.21.4)
- `AnimationMetadataSection.java` — .mcmeta codec
- `AnimationFrame.java` — Frame index + optional time
- `SpriteContents.java` — Frame cycling logic, interpolation
- `SpriteResourceLoader.java` — .mcmeta discovery and parsing
- `TextureAtlas.java` — Atlas animation ticking
