# Roadmap

See [docs/](docs/) for detailed technical research on each system.
See [docs/architecture.md](docs/architecture.md) for the unified rendering approach.

## Completed
- [x] Block model rendering (JSON block models, block states, textures)
- [x] Texture atlas building
- [x] Face culling (hidden face removal, occlusion culling)
- [x] Greedy meshing (face merging with texture tiling)
- [x] Ambient occlusion (baked into textures for greedy, vertex AO otherwise)
- [x] GLB/OBJ/USD/USDZ export
- [x] GLB vertex quantization (KHR_mesh_quantization)

## Tier 1: High Impact, Moderate Complexity

### Liquid Blocks
- [ ] Fluid state resolution (level -> amount -> height)
- [ ] Per-corner height averaging (weighted, 10x bias for source blocks)
- [ ] Flow direction computation and UV rotation
- [ ] Face culling for fluid neighbors
- [ ] Water biome tinting (default to plains #3F76E4)
- [ ] Transparent mesh separation (water transparent, lava opaque)
- [ ] First-frame extraction from animated water/lava textures
- Docs: [liquid-rendering.md](docs/liquid-rendering.md)

### Lighting
- [ ] Block light emission lookup table (block state -> level 0-15)
- [ ] Block light BFS propagation from emitters
- [ ] Sky light computation (heightmap + horizontal spread)
- [ ] Brightness curve application (non-linear 0-15 -> 0.0-1.0)
- [ ] Per-face light sampling and vertex color baking
- [ ] Integration with existing AO system (multiplicative)
- [ ] Config: sky light on/off, ambient level, emissive fullbright
- Docs: [lighting.md](docs/lighting.md)

### Animated Textures
- [ ] Parse .mcmeta animation metadata during resource pack loading
- [ ] Frame size calculation from image dimensions
- [ ] Sprite sheet atlas building (all frames in grid)
- [ ] GLB animation via KHR_animation_pointer + KHR_texture_transform
- [ ] STEP vs LINEAR interpolation based on mcmeta `interpolate` flag
- Docs: [animated-textures.md](docs/animated-textures.md)

## Tier 2: Common Block Entities

### Entity Model Infrastructure
- [ ] Define unified model format (ModelDefinition/PartDefinition/CubeDefinition)
- [ ] Entity model JSON loader (load extracted model definitions)
- [ ] Box-unwrap UV generation from cube dimensions + texture size
- [ ] Part hierarchy traversal with transform accumulation (ZYX Euler)
- [ ] Java model extractor tool (or manual transcription of key models)
- Docs: [entity-models.md](docs/entity-models.md)

### Chests
- [ ] ChestModel geometry (body 14x10x14, lid 14x5x14, lock 2x4x1)
- [ ] Single, double-left, double-right variants
- [ ] Per-type textures (normal, trapped, ender, christmas)
- [ ] Facing rotation from block state
- [ ] Static pose: closed (openness=0)

### Beds
- [ ] Head and foot piece geometry (16x16x6 slab + 2 legs each)
- [ ] Per-color texture selection (16 dye colors)
- [ ] Two-block entity handling (head/foot from block state)
- [ ] Facing rotation

### Signs
- [ ] Board + stick geometry (standing and wall variants)
- [ ] Hanging sign variants (board + chains/plank)
- [ ] Per-wood-type texture selection
- [ ] Text rendering (optional, requires font pipeline)
  - [ ] Component JSON parsing
  - [ ] Font rasterization onto sign texture
  - [ ] Glow text outline

### Bells
- [ ] Bell body geometry (6x7x6 + 8x2x8 base)
- [ ] Combine with existing JSON block model frame
- [ ] Single texture: `entity/bell/bell_body`

### Skulls / Heads
- [ ] SkullModel geometry (8x8x8 box + hat overlay)
- [ ] Per-type textures (skeleton, wither_skeleton, zombie, creeper, dragon, piglin)
- [ ] Floor vs wall positioning from block state
- [ ] Rotation from block state
- [ ] Player heads (optional: skin API fetch or default Steve/Alex)

## Tier 3: Complex Block Entities

### Banners
- [ ] Pole + flag geometry
- [ ] Multi-layer pattern compositing (up to 16 layers)
- [ ] ~40 pattern mask textures from resource pack
- [ ] Per-layer dye color tinting with alpha blending
- [ ] Bake composited result into single flag texture

### Decorated Pots
- [ ] Pot geometry (neck + 4 side planes)
- [ ] Per-sherd pattern texture selection (~25 patterns)
- [ ] Facing rotation from block state

### Shulker Boxes
- [ ] Base + lid geometry
- [ ] Per-color texture selection (16 colors + default)
- [ ] Direction rotation from block state
- [ ] Static pose: closed (peekAmount=0)

### Lectern / Enchanting Table Books
- [ ] BookModel geometry (6 parts: left/right lids, seam, left/right pages, flip page)
- [ ] Enchanting table book texture
- [ ] Static pose for lectern (fixed open angle)
- [ ] Static pose for enchanting table (default floating position)

## Tier 4: Entities

### Armor Stands
- [ ] ArmorStandModel geometry (thin humanoid + structural parts)
- [ ] Per-limb rotation from NBT Rotations data
- [ ] Small variant support

### Item Frames
- [ ] Frame JSON model (already loadable)
- [ ] Contained item rendering (requires item model pipeline)
- [ ] Rotation (8 orientations)
- [ ] Glow item frame variant

### Mob Entities
- [ ] Extract mob model definitions to JSON
- [ ] HumanoidModel (zombie, skeleton, player variants)
- [ ] QuadrupedModel (pig, cow, sheep variants)
- [ ] Unique models (chicken, creeper, villager, etc.)
- [ ] Procedural pose from walk animation parameters
- [ ] Baby scaling variants
- [ ] Riding / passengers

### Players
- [ ] PlayerModel geometry (normal + slim arm variants)
- [ ] Skin texture loading (from profile UUID)
- [ ] Armor overlay layers
- [ ] Held items

## Tier 5: Polish

### Particles
- [ ] Ambient particle spawn point detection (torch flames, campfire smoke, etc.)
- [ ] Metadata export (positions + types as JSON sidecar or glTF extras)
- [ ] Optional: marker quads at spawn positions (cross-model style)
- Docs: [particles.md](docs/particles.md)

### Inventory Rendering
- [ ] Generate texture from inventory contents (chest, shulker box, etc.)
- [ ] Render item icons in grid layout
- [ ] Apply as secondary texture on container face

### Other
- [ ] Block breaking animation (not applicable for static export)
