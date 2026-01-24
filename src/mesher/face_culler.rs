//! Face culling for hidden faces between adjacent blocks.
//!
//! Determines block opacity by analyzing the resolved model rather than hardcoded lists.
//! A block is considered opaque only if its model is a full cube (0-16 on all axes)
//! with all 6 faces defined.
//!
//! Transparent blocks (like glass) are handled specially - they only cull against
//! the same type of transparent block.

use crate::resolver::{ModelResolver, StateResolver};
use crate::resource_pack::ResourcePack;
use crate::types::{BlockPosition, Direction, InputBlock};
use std::collections::{HashMap, HashSet};

/// Classification of a block for culling purposes.
#[derive(Debug, Clone, PartialEq)]
enum BlockCullType {
    /// Air or non-solid blocks - don't cull anything.
    NonSolid,
    /// Fully opaque solid blocks - cull all adjacent faces.
    Opaque,
    /// Transparent blocks that only cull against same type (e.g., glass, ice).
    /// Contains the block's base name for same-type matching.
    Transparent(String),
}

/// Face culler for determining which faces should be hidden.
pub struct FaceCuller<'a> {
    /// Map of block positions to their cull type.
    block_types: HashMap<BlockPosition, BlockCullType>,
    /// Set of occupied positions (for quick lookup).
    occupied: HashSet<BlockPosition>,
    /// Resource pack reference for model resolution.
    pack: &'a ResourcePack,
    /// Cache of block name -> cull type results.
    cull_cache: HashMap<String, BlockCullType>,
}

impl<'a> FaceCuller<'a> {
    /// Create a face culler from a list of blocks, using model data to determine opacity.
    pub fn new(pack: &'a ResourcePack, blocks: &[(BlockPosition, &InputBlock)]) -> Self {
        let mut culler = Self {
            block_types: HashMap::new(),
            occupied: HashSet::new(),
            pack,
            cull_cache: HashMap::new(),
        };

        for (pos, block) in blocks {
            culler.occupied.insert(*pos);
            let cull_type = culler.classify_block(block);
            culler.block_types.insert(*pos, cull_type);
        }

        culler
    }

    /// Classify a block for culling purposes.
    fn classify_block(&mut self, block: &InputBlock) -> BlockCullType {
        // Air is never solid
        if block.is_air() {
            return BlockCullType::NonSolid;
        }

        // Check cache first
        if let Some(cached) = self.cull_cache.get(&block.name) {
            return cached.clone();
        }

        // Classify the block
        let cull_type = self.resolve_and_classify(block);
        self.cull_cache.insert(block.name.clone(), cull_type.clone());
        cull_type
    }

    /// Resolve a block's model and classify it for culling.
    fn resolve_and_classify(&self, block: &InputBlock) -> BlockCullType {
        // Check if this is a transparent block type first
        if let Some(transparent_group) = self.get_transparent_group(&block.name) {
            return BlockCullType::Transparent(transparent_group);
        }

        // Try to resolve the block's variants
        let state_resolver = StateResolver::new(self.pack);
        let variants = match state_resolver.resolve(block) {
            Ok(v) => v,
            Err(_) => return BlockCullType::NonSolid, // Can't resolve = non-solid
        };

        // Check each variant's model
        let model_resolver = ModelResolver::new(self.pack);
        for variant in &variants {
            let model = match model_resolver.resolve(&variant.model_location()) {
                Ok(m) => m,
                Err(_) => return BlockCullType::NonSolid, // Can't resolve model = non-solid
            };

            // Check if this model is a full opaque cube
            if !self.is_full_opaque_cube(&model) {
                return BlockCullType::NonSolid;
            }
        }

        // All variants are full opaque cubes
        if variants.is_empty() {
            BlockCullType::NonSolid
        } else {
            BlockCullType::Opaque
        }
    }

    /// Get the transparent group for a block, if it's a transparent block.
    /// Transparent blocks only cull against the same type.
    fn get_transparent_group(&self, name: &str) -> Option<String> {
        // Extract the base block name without namespace
        let block_id = name.split(':').nth(1).unwrap_or(name);

        // Glass blocks - all glass variants cull against each other
        if block_id == "glass" || block_id.ends_with("_glass") {
            // Group all stained glass together, but separate from regular glass
            if block_id.contains("stained") {
                return Some("stained_glass".to_string());
            }
            if block_id == "tinted_glass" {
                return Some("tinted_glass".to_string());
            }
            return Some("glass".to_string());
        }

        // Glass panes - separate from glass blocks
        if block_id == "glass_pane" || block_id.ends_with("_glass_pane") {
            if block_id.contains("stained") {
                return Some("stained_glass_pane".to_string());
            }
            return Some("glass_pane".to_string());
        }

        // Ice blocks
        if block_id == "ice" || block_id == "packed_ice" || block_id == "blue_ice" {
            return Some(block_id.to_string());
        }
        if block_id == "frosted_ice" {
            return Some("frosted_ice".to_string());
        }

        // Leaves - each type culls against itself
        if block_id.ends_with("_leaves") {
            return Some("leaves".to_string());
        }

        // Slime and honey blocks
        if block_id == "slime_block" {
            return Some("slime_block".to_string());
        }
        if block_id == "honey_block" {
            return Some("honey_block".to_string());
        }

        None
    }

    /// Check if a resolved model is a full opaque cube.
    fn is_full_opaque_cube(&self, model: &crate::resource_pack::BlockModel) -> bool {
        // Must have exactly one element
        if model.elements.len() != 1 {
            return false;
        }

        let element = &model.elements[0];

        // Element must span the full block (0-16 on each axis)
        const EPSILON: f32 = 0.001;
        if element.from[0].abs() > EPSILON
            || element.from[1].abs() > EPSILON
            || element.from[2].abs() > EPSILON
        {
            return false;
        }
        if (element.to[0] - 16.0).abs() > EPSILON
            || (element.to[1] - 16.0).abs() > EPSILON
            || (element.to[2] - 16.0).abs() > EPSILON
        {
            return false;
        }

        // Must have all 6 faces defined
        if element.faces.len() != 6 {
            return false;
        }

        // All faces must exist
        for direction in Direction::ALL.iter() {
            if !element.faces.contains_key(direction) {
                return false;
            }
        }

        true
    }

    /// Check if a face should be culled.
    pub fn should_cull(&self, pos: BlockPosition, cullface: Direction) -> bool {
        let neighbor_pos = pos.neighbor(cullface);

        // If there's no block in that direction, don't cull
        if !self.occupied.contains(&neighbor_pos) {
            return false;
        }

        let neighbor_type = self.block_types.get(&neighbor_pos);
        let current_type = self.block_types.get(&pos);

        match (current_type, neighbor_type) {
            // If neighbor is opaque, always cull
            (_, Some(BlockCullType::Opaque)) => true,

            // If current block is transparent, only cull if neighbor is same transparent group
            (Some(BlockCullType::Transparent(current_group)), Some(BlockCullType::Transparent(neighbor_group))) => {
                current_group == neighbor_group
            }

            // Transparent blocks don't cull against opaque blocks at their own position
            // (handled by the first match arm checking neighbor)

            // All other cases - don't cull
            _ => false,
        }
    }

    /// Check if a position has an opaque block (for AO calculations).
    /// Note: Both opaque and transparent blocks contribute to AO.
    pub fn is_opaque_at(&self, pos: BlockPosition) -> bool {
        match self.block_types.get(&pos) {
            Some(BlockCullType::Opaque) => true,
            Some(BlockCullType::Transparent(_)) => true, // Transparent blocks also cast AO
            _ => false,
        }
    }

    /// Check if a position has a fully opaque (non-transparent) block.
    pub fn is_fully_opaque_at(&self, pos: BlockPosition) -> bool {
        matches!(self.block_types.get(&pos), Some(BlockCullType::Opaque))
    }

    /// Check if a position is occupied.
    pub fn is_occupied(&self, pos: BlockPosition) -> bool {
        self.occupied.contains(&pos)
    }

    /// Calculate ambient occlusion values for the 4 vertices of a face.
    /// Returns [v0_ao, v1_ao, v2_ao, v3_ao] where values are 0-3 (0=darkest, 3=brightest).
    pub fn calculate_ao(&self, pos: BlockPosition, direction: Direction) -> [u8; 4] {
        // Get the neighbor offsets for the 4 corners of this face
        let corner_neighbors = get_ao_neighbors(direction);

        let mut ao_values = [3u8; 4];
        for (i, (side1_offset, side2_offset, corner_offset)) in corner_neighbors.iter().enumerate() {
            let side1_pos = BlockPosition::new(
                pos.x + side1_offset[0],
                pos.y + side1_offset[1],
                pos.z + side1_offset[2],
            );
            let side2_pos = BlockPosition::new(
                pos.x + side2_offset[0],
                pos.y + side2_offset[1],
                pos.z + side2_offset[2],
            );
            let corner_pos = BlockPosition::new(
                pos.x + corner_offset[0],
                pos.y + corner_offset[1],
                pos.z + corner_offset[2],
            );

            let side1 = self.is_opaque_at(side1_pos) as u8;
            let side2 = self.is_opaque_at(side2_pos) as u8;
            let corner = self.is_opaque_at(corner_pos) as u8;

            ao_values[i] = vertex_ao(side1, side2, corner);
        }

        ao_values
    }
}

/// For backwards compatibility - create from blocks without pack reference.
/// This uses a simplified opacity check (occupied = opaque).
impl<'a> FaceCuller<'a> {
    /// Create a face culler without model resolution (legacy mode).
    /// Uses simple occupancy for opacity - all blocks are considered opaque.
    pub fn from_blocks(blocks: &[(BlockPosition, &InputBlock)]) -> FaceCullerSimple {
        FaceCullerSimple::from_blocks(blocks)
    }
}

/// Simplified cull type for heuristic-based culling.
#[derive(Debug, Clone, PartialEq)]
enum SimpleCullType {
    NonSolid,
    Opaque,
    Transparent(String),
}

/// Simplified face culler without model resolution.
/// Uses basic heuristics to determine opacity.
pub struct FaceCullerSimple {
    /// Map of block positions to their cull type.
    block_types: HashMap<BlockPosition, SimpleCullType>,
    /// Set of occupied positions.
    occupied: HashSet<BlockPosition>,
}

impl FaceCullerSimple {
    /// Create from blocks using heuristic opacity detection.
    pub fn from_blocks(blocks: &[(BlockPosition, &InputBlock)]) -> Self {
        let mut block_types = HashMap::new();
        let mut occupied = HashSet::new();

        for (pos, block) in blocks {
            occupied.insert(*pos);
            let cull_type = Self::classify_block_heuristic(&block.name);
            block_types.insert(*pos, cull_type);
        }

        Self {
            block_types,
            occupied,
        }
    }

    /// Classify a block using heuristics (no model resolution).
    fn classify_block_heuristic(name: &str) -> SimpleCullType {
        let block_id = name.split(':').nth(1).unwrap_or(name);

        // Air is never solid
        if name.contains("air") {
            return SimpleCullType::NonSolid;
        }

        // Check for transparent blocks
        if let Some(group) = Self::get_transparent_group_heuristic(block_id) {
            return SimpleCullType::Transparent(group);
        }

        // Check if likely a full cube
        if is_likely_full_cube(name) {
            SimpleCullType::Opaque
        } else {
            SimpleCullType::NonSolid
        }
    }

    /// Get transparent group using heuristics.
    fn get_transparent_group_heuristic(block_id: &str) -> Option<String> {
        // Glass blocks
        if block_id == "glass" || block_id.ends_with("_glass") {
            if block_id.contains("stained") {
                return Some("stained_glass".to_string());
            }
            if block_id == "tinted_glass" {
                return Some("tinted_glass".to_string());
            }
            return Some("glass".to_string());
        }

        // Glass panes
        if block_id == "glass_pane" || block_id.ends_with("_glass_pane") {
            if block_id.contains("stained") {
                return Some("stained_glass_pane".to_string());
            }
            return Some("glass_pane".to_string());
        }

        // Ice
        if block_id == "ice" || block_id == "packed_ice" || block_id == "blue_ice" || block_id == "frosted_ice" {
            return Some(block_id.to_string());
        }

        // Leaves
        if block_id.ends_with("_leaves") {
            return Some("leaves".to_string());
        }

        // Slime and honey
        if block_id == "slime_block" || block_id == "honey_block" {
            return Some(block_id.to_string());
        }

        None
    }

    /// Check if a face should be culled.
    pub fn should_cull(&self, pos: BlockPosition, cullface: Direction) -> bool {
        let neighbor_pos = pos.neighbor(cullface);

        if !self.occupied.contains(&neighbor_pos) {
            return false;
        }

        let neighbor_type = self.block_types.get(&neighbor_pos);
        let current_type = self.block_types.get(&pos);

        match (current_type, neighbor_type) {
            (_, Some(SimpleCullType::Opaque)) => true,
            (Some(SimpleCullType::Transparent(current)), Some(SimpleCullType::Transparent(neighbor))) => {
                current == neighbor
            }
            _ => false,
        }
    }

    /// Check if a position has an opaque block (for AO).
    pub fn is_opaque_at(&self, pos: BlockPosition) -> bool {
        match self.block_types.get(&pos) {
            Some(SimpleCullType::Opaque) => true,
            Some(SimpleCullType::Transparent(_)) => true,
            _ => false,
        }
    }

    /// Check if a position is occupied.
    pub fn is_occupied(&self, pos: BlockPosition) -> bool {
        self.occupied.contains(&pos)
    }

    /// Calculate ambient occlusion values for the 4 vertices of a face.
    pub fn calculate_ao(&self, pos: BlockPosition, direction: Direction) -> [u8; 4] {
        let corner_neighbors = get_ao_neighbors(direction);

        let mut ao_values = [3u8; 4];
        for (i, (side1_offset, side2_offset, corner_offset)) in corner_neighbors.iter().enumerate() {
            let side1_pos = BlockPosition::new(
                pos.x + side1_offset[0],
                pos.y + side1_offset[1],
                pos.z + side1_offset[2],
            );
            let side2_pos = BlockPosition::new(
                pos.x + side2_offset[0],
                pos.y + side2_offset[1],
                pos.z + side2_offset[2],
            );
            let corner_pos = BlockPosition::new(
                pos.x + corner_offset[0],
                pos.y + corner_offset[1],
                pos.z + corner_offset[2],
            );

            let side1 = self.is_opaque_at(side1_pos) as u8;
            let side2 = self.is_opaque_at(side2_pos) as u8;
            let corner = self.is_opaque_at(corner_pos) as u8;

            ao_values[i] = vertex_ao(side1, side2, corner);
        }

        ao_values
    }
}

/// Heuristic check if a block name likely represents a full cube.
/// This is a fallback when we can't resolve the model.
fn is_likely_full_cube(name: &str) -> bool {
    // Air is never a full cube
    if name.contains("air") {
        return false;
    }

    // Common patterns for non-full blocks
    let non_full_patterns = [
        "slab", "stairs", "fence", "wall", "door", "trapdoor",
        "sign", "banner", "button", "lever", "torch", "lantern",
        "pressure_plate", "carpet", "rail", "flower", "sapling",
        "glass_pane", "iron_bars", "chain", "rod", "candle",
        "head", "skull", "pot", "campfire", "anvil", "bell",
        "brewing_stand", "cauldron", "hopper", "lectern",
        "grindstone", "stonecutter", "enchanting_table",
        "repeater", "comparator", "daylight_detector",
        "piston", "tripwire", "string", "cobweb", "vine",
        "ladder", "scaffolding", "coral_fan", "pickle",
        "egg", "frogspawn", "dripleaf", "azalea", "roots",
        "sprouts", "fungus", "mushroom", "grass", "fern",
        "bush", "berry", "wart", "stem", "crop", "wheat",
        "carrots", "potatoes", "beetroots", "cocoa", "cactus",
        "sugar_cane", "bamboo", "kelp", "seagrass", "lichen",
        "vein", "fire", "snow", "layer",
        // Specific flowers that don't have "flower" in name
        "poppy", "dandelion", "orchid", "allium", "tulip",
        "oxeye_daisy", "cornflower", "lily_of_the_valley",
        "wither_rose", "sunflower", "lilac", "rose_bush",
        "peony", "pitcher_plant", "torchflower", "pink_petals",
    ];

    for pattern in &non_full_patterns {
        if name.contains(pattern) {
            // Special case: some blocks contain these patterns but ARE full cubes
            // e.g., "mushroom_block", "grass_block", "hay_block"
            if name.ends_with("_block") && !name.contains("piston") {
                continue;
            }
            return false;
        }
    }

    // Default: assume it's a full cube
    true
}

/// Calculate vertex AO value from neighbor occupancy.
fn vertex_ao(side1: u8, side2: u8, corner: u8) -> u8 {
    if side1 == 1 && side2 == 1 {
        0
    } else {
        3 - (side1 + side2 + corner)
    }
}

/// Get the neighbor offsets for AO calculation for each vertex of a face.
fn get_ao_neighbors(direction: Direction) -> [([i32; 3], [i32; 3], [i32; 3]); 4] {
    match direction {
        Direction::Up => [
            ([0, 1, -1], [-1, 1, 0], [-1, 1, -1]),
            ([0, 1, -1], [1, 1, 0], [1, 1, -1]),
            ([0, 1, 1], [1, 1, 0], [1, 1, 1]),
            ([0, 1, 1], [-1, 1, 0], [-1, 1, 1]),
        ],
        Direction::Down => [
            ([0, -1, 1], [-1, -1, 0], [-1, -1, 1]),
            ([0, -1, 1], [1, -1, 0], [1, -1, 1]),
            ([0, -1, -1], [1, -1, 0], [1, -1, -1]),
            ([0, -1, -1], [-1, -1, 0], [-1, -1, -1]),
        ],
        Direction::North => [
            ([1, 0, -1], [0, 1, -1], [1, 1, -1]),
            ([-1, 0, -1], [0, 1, -1], [-1, 1, -1]),
            ([-1, 0, -1], [0, -1, -1], [-1, -1, -1]),
            ([1, 0, -1], [0, -1, -1], [1, -1, -1]),
        ],
        Direction::South => [
            ([-1, 0, 1], [0, 1, 1], [-1, 1, 1]),
            ([1, 0, 1], [0, 1, 1], [1, 1, 1]),
            ([1, 0, 1], [0, -1, 1], [1, -1, 1]),
            ([-1, 0, 1], [0, -1, 1], [-1, -1, 1]),
        ],
        Direction::West => [
            ([-1, 0, -1], [-1, 1, 0], [-1, 1, -1]),
            ([-1, 0, 1], [-1, 1, 0], [-1, 1, 1]),
            ([-1, 0, 1], [-1, -1, 0], [-1, -1, 1]),
            ([-1, 0, -1], [-1, -1, 0], [-1, -1, -1]),
        ],
        Direction::East => [
            ([1, 0, 1], [1, 1, 0], [1, 1, 1]),
            ([1, 0, -1], [1, 1, 0], [1, 1, -1]),
            ([1, 0, -1], [1, -1, 0], [1, -1, -1]),
            ([1, 0, 1], [1, -1, 0], [1, -1, 1]),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_likely_full_cube() {
        // Full cubes
        assert!(is_likely_full_cube("minecraft:stone"));
        assert!(is_likely_full_cube("minecraft:dirt"));
        assert!(is_likely_full_cube("minecraft:oak_log"));
        assert!(is_likely_full_cube("minecraft:diamond_block"));
        assert!(is_likely_full_cube("minecraft:grass_block"));
        assert!(is_likely_full_cube("minecraft:mushroom_block"));

        // Non-full blocks
        assert!(!is_likely_full_cube("minecraft:air"));
        assert!(!is_likely_full_cube("minecraft:oak_slab"));
        assert!(!is_likely_full_cube("minecraft:oak_stairs"));
        assert!(!is_likely_full_cube("minecraft:oak_fence"));
        assert!(!is_likely_full_cube("minecraft:glass_pane"));
        assert!(!is_likely_full_cube("minecraft:torch"));
        assert!(!is_likely_full_cube("minecraft:poppy"));
        assert!(!is_likely_full_cube("minecraft:oak_door"));
    }

    #[test]
    fn test_simple_culler() {
        let stone1 = InputBlock::new("minecraft:stone");
        let stone2 = InputBlock::new("minecraft:stone");
        let flower = InputBlock::new("minecraft:poppy");

        let blocks = vec![
            (BlockPosition::new(0, 0, 0), &stone1),
            (BlockPosition::new(1, 0, 0), &stone2),
            (BlockPosition::new(0, 1, 0), &flower),
        ];

        let culler = FaceCullerSimple::from_blocks(&blocks);

        // Face between two stones should be culled
        assert!(culler.should_cull(BlockPosition::new(0, 0, 0), Direction::East));
        assert!(culler.should_cull(BlockPosition::new(1, 0, 0), Direction::West));

        // Flower on top of stone should NOT cull stone's top face
        assert!(!culler.should_cull(BlockPosition::new(0, 0, 0), Direction::Up));

        // Outer faces should not be culled
        assert!(!culler.should_cull(BlockPosition::new(0, 0, 0), Direction::West));
    }

    #[test]
    fn test_glass_culling() {
        let glass1 = InputBlock::new("minecraft:glass");
        let glass2 = InputBlock::new("minecraft:glass");
        let stained_glass = InputBlock::new("minecraft:red_stained_glass");
        let stone = InputBlock::new("minecraft:stone");

        let blocks = vec![
            (BlockPosition::new(0, 0, 0), &glass1),
            (BlockPosition::new(1, 0, 0), &glass2),
            (BlockPosition::new(2, 0, 0), &stained_glass),
            (BlockPosition::new(0, 1, 0), &stone),
        ];

        let culler = FaceCullerSimple::from_blocks(&blocks);

        // Glass should cull against same glass type
        assert!(culler.should_cull(BlockPosition::new(0, 0, 0), Direction::East));
        assert!(culler.should_cull(BlockPosition::new(1, 0, 0), Direction::West));

        // Glass should NOT cull against stained glass (different group)
        assert!(!culler.should_cull(BlockPosition::new(1, 0, 0), Direction::East));
        assert!(!culler.should_cull(BlockPosition::new(2, 0, 0), Direction::West));

        // Glass should cull when neighbor is opaque stone
        assert!(culler.should_cull(BlockPosition::new(0, 0, 0), Direction::Up));
    }

    #[test]
    fn test_transparent_groups() {
        // Test that transparent group detection works correctly
        assert_eq!(
            FaceCullerSimple::get_transparent_group_heuristic("glass"),
            Some("glass".to_string())
        );
        assert_eq!(
            FaceCullerSimple::get_transparent_group_heuristic("red_stained_glass"),
            Some("stained_glass".to_string())
        );
        assert_eq!(
            FaceCullerSimple::get_transparent_group_heuristic("tinted_glass"),
            Some("tinted_glass".to_string())
        );
        assert_eq!(
            FaceCullerSimple::get_transparent_group_heuristic("oak_leaves"),
            Some("leaves".to_string())
        );
        assert_eq!(
            FaceCullerSimple::get_transparent_group_heuristic("ice"),
            Some("ice".to_string())
        );
        assert_eq!(
            FaceCullerSimple::get_transparent_group_heuristic("stone"),
            None
        );
    }
}
