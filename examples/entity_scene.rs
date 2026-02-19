//! Block entity test scene.
//!
//!   cargo run --example entity_scene
//!
//! Exports GLB to artifacts/entity.glb. Tests chests, beds, bells, signs, skulls.

use schematic_mesher::{
    export_glb, load_resource_pack, Mesher, MesherConfig, TintProvider,
    types::{BlockPosition, BlockSource, BoundingBox, InputBlock},
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

fn build_scene(s: &mut Scene) {
    // Stone floor 24x80
    for x in 0..24 {
        for z in 0..80 {
            s.set(x, 0, z, "minecraft:stone");
        }
    }

    // === Section 1 (z=1-2): Chests in 4 facings ===
    for (i, facing) in ["north", "south", "east", "west"].iter().enumerate() {
        let x = i as i32 * 3 + 1;
        s.set_with(x, 1, 1, "minecraft:chest", &[("facing", facing)]);
    }

    // Trapped chest
    s.set_with(13, 1, 1, "minecraft:trapped_chest", &[("facing", "north")]);

    // Ender chest
    s.set_with(16, 1, 1, "minecraft:ender_chest", &[("facing", "north")]);

    // Double chest (left + right blocks side by side)
    s.set_with(19, 1, 1, "minecraft:chest", &[("facing", "north"), ("type", "left")]);
    s.set_with(20, 1, 1, "minecraft:chest", &[("facing", "north"), ("type", "right")]);

    // === Section 2 (z=4-5): Beds ===
    let colors = ["red", "blue", "green", "white"];
    for (i, color) in colors.iter().enumerate() {
        let x = i as i32 * 4 + 1;
        let name = format!("minecraft:{}_bed", color);
        // Head at z=4, foot at z=5 (bed faces north toward z=4)
        s.set_with(x, 1, 4, &name, &[("part", "head"), ("facing", "south")]);
        s.set_with(x, 1, 5, &name, &[("part", "foot"), ("facing", "south")]);
    }

    // === Section 3 (z=8): Bell ===
    s.set_with(1, 1, 8, "minecraft:bell", &[("facing", "north"), ("attachment", "floor")]);

    // === Section 4 (z=10-11): Signs ===
    // Standing signs
    s.set_with(1, 1, 10, "minecraft:oak_sign", &[("rotation", "0")]);
    s.set_with(4, 1, 10, "minecraft:birch_sign", &[("rotation", "4")]);
    s.set_with(7, 1, 10, "minecraft:spruce_sign", &[("rotation", "8")]);

    // Wall signs (on stone wall)
    s.set(1, 2, 12, "minecraft:stone");
    s.set_with(1, 2, 11, "minecraft:wall_oak_sign", &[("facing", "south")]);
    s.set(5, 2, 12, "minecraft:stone");
    s.set_with(5, 2, 11, "minecraft:wall_birch_sign", &[("facing", "south")]);

    // === Section 5 (z=14): Skulls ===
    s.set_with(1, 1, 14, "minecraft:skeleton_skull", &[("rotation", "0")]);
    s.set_with(4, 1, 14, "minecraft:creeper_head", &[("rotation", "8")]);
    s.set_with(7, 1, 14, "minecraft:zombie_head", &[("rotation", "4")]);

    // Wall skulls (on stone wall)
    s.set(1, 2, 16, "minecraft:stone");
    s.set_with(1, 2, 15, "minecraft:skeleton_wall_skull", &[("facing", "south")]);
    s.set(4, 2, 16, "minecraft:stone");
    s.set_with(4, 2, 15, "minecraft:creeper_wall_head", &[("facing", "south")]);

    // === Section 6 (z=18): Mobs ===
    s.set_with(1, 1, 18, "entity:zombie", &[("facing", "south")]);
    s.set_with(4, 1, 18, "entity:skeleton", &[("facing", "south")]);
    s.set_with(7, 1, 18, "entity:creeper", &[("facing", "south")]);
    s.set_with(10, 1, 18, "entity:pig", &[("facing", "south")]);
    s.set_with(13, 1, 18, "entity:chicken", &[("facing", "south")]);
    s.set_with(16, 1, 18, "entity:cow", &[("facing", "south")]);
    s.set_with(19, 1, 18, "entity:sheep", &[("facing", "south")]);
    s.set_with(22, 1, 18, "entity:villager", &[("facing", "south")]);
    // Sheep with wool color
    s.set_with(19, 1, 19, "entity:sheep", &[("facing", "south"), ("color", "red")]);
    s.set_with(22, 1, 19, "entity:sheep", &[("facing", "south"), ("color", "blue")]);

    // === Section 7 (z=20-22): Shulker Boxes ===
    // Default (purple) + colored variants in different facings
    s.set_with(1, 1, 20, "minecraft:shulker_box", &[("facing", "up")]);
    s.set_with(4, 1, 20, "minecraft:red_shulker_box", &[("facing", "up")]);
    s.set_with(7, 1, 20, "minecraft:blue_shulker_box", &[("facing", "north")]);
    s.set_with(10, 1, 20, "minecraft:green_shulker_box", &[("facing", "east")]);
    s.set_with(13, 1, 20, "minecraft:yellow_shulker_box", &[("facing", "down")]);
    // Wall-mounted shulker
    s.set(1, 2, 22, "minecraft:stone");
    s.set_with(1, 2, 21, "minecraft:white_shulker_box", &[("facing", "south")]);

    // === Section 8 (z=24-26): New Entities ===
    // Armor stands (plain + armored)
    s.set_with(1, 1, 24, "entity:armor_stand", &[("facing", "south")]);
    s.set_with(4, 1, 24, "entity:armor_stand", &[
        ("facing", "south"),
        ("helmet", "minecraft:diamond_helmet"),
        ("chestplate", "minecraft:diamond_chestplate"),
        ("leggings", "minecraft:diamond_leggings"),
        ("boots", "minecraft:diamond_boots"),
    ]);
    s.set_with(7, 1, 24, "entity:armor_stand", &[
        ("facing", "east"),
        ("helmet", "minecraft:iron_helmet"),
        ("chestplate", "minecraft:iron_chestplate"),
    ]);

    // Minecarts
    s.set_with(10, 1, 24, "entity:minecart", &[("facing", "south")]);
    s.set_with(13, 1, 24, "entity:minecart", &[("facing", "east")]);

    // Item frames (on stone wall)
    s.set(13, 2, 26, "minecraft:stone");
    s.set_with(13, 2, 25, "entity:item_frame", &[("facing", "south")]);
    s.set(16, 2, 26, "minecraft:stone");
    s.set_with(16, 2, 25, "entity:glow_item_frame", &[("facing", "south")]);
    // Floor-mounted item frame
    s.set_with(19, 1, 25, "entity:item_frame", &[("facing", "up")]);

    // === Section 9 (z=28-29): Item Frames with Items ===
    // Wall backing for item frames
    for x in [1, 4, 7, 10, 13, 16] {
        s.set(x, 2, 29, "minecraft:stone");
    }

    // Flat items
    s.set_with(1, 2, 28, "entity:item_frame", &[
        ("facing", "south"), ("item", "minecraft:diamond_sword"),
    ]);
    s.set_with(4, 2, 28, "entity:item_frame", &[
        ("facing", "south"), ("item", "minecraft:apple"),
    ]);
    s.set_with(7, 2, 28, "entity:item_frame", &[
        ("facing", "south"), ("item", "minecraft:diamond_pickaxe"), ("item_rotation", "2"),
    ]);

    // Block items
    s.set_with(10, 2, 28, "entity:item_frame", &[
        ("facing", "south"), ("item", "minecraft:oak_planks"),
    ]);
    s.set_with(13, 2, 28, "entity:item_frame", &[
        ("facing", "south"), ("item", "minecraft:diamond_block"),
    ]);

    // Glow frame with item
    s.set(16, 2, 29, "minecraft:stone");
    s.set_with(16, 2, 28, "entity:glow_item_frame", &[
        ("facing", "south"), ("item", "minecraft:compass"),
    ]);

    // === Section 10 (z=31-32): Dropped Items ===
    s.set_with(1, 1, 31, "entity:item", &[
        ("facing", "south"), ("item", "minecraft:diamond_sword"),
    ]);
    s.set_with(4, 1, 31, "entity:item", &[
        ("facing", "south"), ("item", "minecraft:apple"),
    ]);
    s.set_with(7, 1, 31, "entity:item", &[
        ("facing", "east"), ("item", "minecraft:oak_planks"),
    ]);
    s.set_with(10, 1, 31, "entity:item", &[
        ("facing", "south"), ("item", "minecraft:diamond_block"),
    ]);

    // === Section 11 (z=34-36): Banners ===
    // Plain standing banners
    s.set_with(1, 1, 34, "minecraft:white_banner", &[("rotation", "0")]);
    s.set_with(4, 1, 34, "minecraft:red_banner", &[("rotation", "8")]);
    s.set_with(7, 1, 34, "minecraft:blue_banner", &[("rotation", "4")]);

    // Banner with patterns
    s.set_with(10, 1, 34, "minecraft:white_banner", &[
        ("rotation", "0"),
        ("patterns", "stripe_bottom:red,cross:blue"),
    ]);
    s.set_with(13, 1, 34, "minecraft:yellow_banner", &[
        ("rotation", "0"),
        ("patterns", "stripe_left:green,stripe_right:green"),
    ]);

    // Wall banners
    s.set(1, 2, 37, "minecraft:stone");
    s.set_with(1, 2, 36, "minecraft:green_wall_banner", &[("facing", "south")]);
    s.set(4, 2, 37, "minecraft:stone");
    s.set_with(4, 2, 36, "minecraft:purple_wall_banner", &[
        ("facing", "south"),
        ("patterns", "rhombus:yellow"),
    ]);

    // === Section 12 (z=39-40): Inventory Rendering ===
    s.set_with(1, 1, 39, "minecraft:chest", &[
        ("facing", "south"),
        ("inventory", "diamond_sword,apple,stone:64,,iron_ore:32"),
    ]);
    s.set_with(4, 1, 39, "minecraft:chest", &[
        ("facing", "south"),
        ("inventory", "oak_planks,diamond_block,gold_ingot,iron_ingot,coal,emerald,redstone,lapis_lazuli,diamond"),
    ]);

    // === Section 13 (z=42-45): Particles ===
    // Torches
    s.set(1, 1, 42, "minecraft:torch");
    s.set(4, 1, 42, "minecraft:soul_torch");

    // Wall torches (on stone wall at z=43, torches face north away from wall)
    s.set(7, 2, 43, "minecraft:stone");
    s.set_with(7, 2, 42, "minecraft:wall_torch", &[("facing", "north")]);
    s.set(10, 2, 43, "minecraft:stone");
    s.set_with(10, 2, 42, "minecraft:soul_wall_torch", &[("facing", "north")]);

    // Campfires
    s.set_with(1, 1, 44, "minecraft:campfire", &[("lit", "true")]);
    s.set_with(4, 1, 44, "minecraft:soul_campfire", &[("lit", "true")]);
    s.set_with(7, 1, 44, "minecraft:campfire", &[("lit", "false")]); // unlit, no particles

    // Candles (1-4)
    s.set_with(10, 1, 44, "minecraft:candle", &[("lit", "true"), ("candles", "1")]);
    s.set_with(12, 1, 44, "minecraft:red_candle", &[("lit", "true"), ("candles", "2")]);
    s.set_with(14, 1, 44, "minecraft:blue_candle", &[("lit", "true"), ("candles", "3")]);
    s.set_with(16, 1, 44, "minecraft:green_candle", &[("lit", "true"), ("candles", "4")]);

    // Lanterns
    s.set(1, 1, 46, "minecraft:lantern");
    s.set(4, 1, 46, "minecraft:soul_lantern");

    // End rod
    s.set_with(7, 1, 46, "minecraft:end_rod", &[("facing", "up")]);
    s.set_with(10, 1, 46, "minecraft:end_rod", &[("facing", "north")]);

    // Lit furnace
    s.set_with(13, 1, 46, "minecraft:furnace", &[("facing", "south"), ("lit", "true")]);
    s.set_with(16, 1, 46, "minecraft:smoker", &[("facing", "south"), ("lit", "true")]);

    // === Section 14 (z=48-49): New Mobs ===
    s.set_with(1, 1, 48, "entity:wolf", &[("facing", "south")]);
    s.set_with(4, 1, 48, "entity:cat", &[("facing", "south")]);
    s.set_with(7, 1, 48, "entity:spider", &[("facing", "south")]);
    s.set_with(10, 1, 48, "entity:horse", &[("facing", "south")]);
    s.set_with(13, 1, 48, "entity:enderman", &[("facing", "south")]);
    s.set_with(16, 1, 48, "entity:slime", &[("facing", "south")]);
    s.set_with(19, 1, 48, "entity:iron_golem", &[("facing", "south")]);
    s.set_with(22, 1, 48, "entity:bat", &[("facing", "south")]);

    // === Section 15 (z=50-51): Posed Armor Stands ===
    // Waving right arm
    s.set_with(1, 1, 50, "entity:armor_stand", &[
        ("facing", "south"),
        ("RightArmPose", "-110,0,0"),
    ]);
    // Arms out (T-pose variation)
    s.set_with(4, 1, 50, "entity:armor_stand", &[
        ("facing", "south"),
        ("RightArmPose", "0,0,-90"),
        ("LeftArmPose", "0,0,90"),
    ]);
    // Sitting pose
    s.set_with(7, 1, 50, "entity:armor_stand", &[
        ("facing", "south"),
        ("RightLegPose", "-90,0,0"),
        ("LeftLegPose", "-90,0,0"),
    ]);
    // Dynamic combat pose with armor
    s.set_with(10, 1, 50, "entity:armor_stand", &[
        ("facing", "south"),
        ("RightArmPose", "-60,20,0"),
        ("LeftArmPose", "-10,0,10"),
        ("HeadPose", "-15,0,0"),
        ("helmet", "minecraft:netherite_helmet"),
        ("chestplate", "minecraft:netherite_chestplate"),
        ("leggings", "minecraft:netherite_leggings"),
        ("boots", "minecraft:netherite_boots"),
    ]);

    // === Section 16 (z=52-53): Signs with Text ===
    s.set_with(1, 1, 52, "minecraft:oak_sign", &[
        ("rotation", "0"),
        ("text1", "Hello"),
        ("text2", "World"),
    ]);
    s.set_with(4, 1, 52, "minecraft:birch_sign", &[
        ("rotation", "0"),
        ("text1", "Welcome to"),
        ("text2", "Minecraft"),
        ("text3", "Mesher"),
        ("color", "dark_blue"),
    ]);
    // Wall sign with text
    s.set(7, 2, 53, "minecraft:stone");
    s.set_with(7, 2, 52, "minecraft:wall_oak_sign", &[
        ("facing", "south"),
        ("text1", "Wall Sign"),
        ("text2", "Line Two"),
        ("color", "dark_red"),
    ]);

    // === Section 17 (z=54-55): Baby Mobs ===
    s.set_with(1, 1, 54, "entity:pig", &[("facing", "south"), ("is_baby", "true")]);
    s.set_with(4, 1, 54, "entity:cow", &[("facing", "south"), ("is_baby", "true")]);
    s.set_with(7, 1, 54, "entity:sheep", &[("facing", "south"), ("is_baby", "true")]);
    s.set_with(10, 1, 54, "entity:chicken", &[("facing", "south"), ("is_baby", "true")]);
    s.set_with(13, 1, 54, "entity:wolf", &[("facing", "south"), ("is_baby", "true")]);
    s.set_with(16, 1, 54, "entity:cat", &[("facing", "south"), ("is_baby", "true")]);
    s.set_with(19, 1, 54, "entity:horse", &[("facing", "south"), ("is_baby", "true")]);
    s.set_with(22, 1, 54, "entity:zombie", &[("facing", "south"), ("is_baby", "true")]);
    // Baby sheep with color
    s.set_with(7, 1, 55, "entity:sheep", &[("facing", "south"), ("is_baby", "true"), ("color", "pink")]);
    // Adult vs baby comparison
    s.set_with(1, 1, 55, "entity:pig", &[("facing", "south")]); // adult for comparison

    // === Section 18 (z=57-58): Hanging Signs ===
    // Ceiling-mounted hanging signs
    s.set_with(1, 1, 57, "minecraft:oak_hanging_sign", &[("rotation", "0")]);
    s.set_with(4, 1, 57, "minecraft:birch_hanging_sign", &[("rotation", "4")]);
    s.set_with(7, 1, 57, "minecraft:cherry_hanging_sign", &[("rotation", "8")]);

    // Wall-mounted hanging signs
    s.set(1, 2, 59, "minecraft:stone");
    s.set_with(1, 2, 58, "minecraft:oak_wall_hanging_sign", &[("facing", "south")]);
    s.set(5, 2, 59, "minecraft:stone");
    s.set_with(5, 2, 58, "minecraft:birch_wall_hanging_sign", &[("facing", "south")]);

    // Hanging sign with text
    s.set_with(10, 1, 57, "minecraft:oak_hanging_sign", &[
        ("rotation", "0"),
        ("text1", "Hanging"),
        ("text2", "Sign"),
    ]);
    s.set_with(13, 1, 57, "minecraft:cherry_hanging_sign", &[
        ("rotation", "0"),
        ("text1", "Cherry"),
        ("text2", "Wood"),
        ("color", "dark_red"),
    ]);

    // Wall hanging sign with text
    s.set(16, 2, 59, "minecraft:stone");
    s.set_with(16, 2, 58, "minecraft:spruce_wall_hanging_sign", &[
        ("facing", "south"),
        ("text1", "Wall"),
        ("text2", "Hanging"),
        ("color", "gold"),
    ]);

    // === Section 19 (z=61-62): Decorated Pots ===
    // Plain pot
    s.set_with(1, 1, 61, "minecraft:decorated_pot", &[("facing", "south")]);

    // Pot with all same sherds
    s.set_with(4, 1, 61, "minecraft:decorated_pot", &[
        ("facing", "south"),
        ("sherds", "angler,angler,angler,angler"),
    ]);

    // Pot with mixed sherds
    s.set_with(7, 1, 61, "minecraft:decorated_pot", &[
        ("facing", "south"),
        ("sherds", "arms_up,blade,brewer,burn"),
    ]);

    // Pot with partial sherds (brick = default)
    s.set_with(10, 1, 61, "minecraft:decorated_pot", &[
        ("facing", "north"),
        ("sherds", "brick,archer,brick,heart"),
    ]);

    // Pots in different facings
    s.set_with(13, 1, 61, "minecraft:decorated_pot", &[
        ("facing", "east"),
        ("sherds", "danger,explorer,friend,howl"),
    ]);
    s.set_with(16, 1, 61, "minecraft:decorated_pot", &[
        ("facing", "west"),
        ("sherds", "miner,mourner,plenty,prize"),
    ]);

    // === Section 20 (z=63-65): Lecterns & Enchanting Tables ===
    // Lectern with book in different facings
    s.set_with(1, 1, 63, "minecraft:lectern", &[("facing", "south"), ("has_book", "true")]);
    s.set_with(4, 1, 63, "minecraft:lectern", &[("facing", "north"), ("has_book", "true")]);
    s.set_with(7, 1, 63, "minecraft:lectern", &[("facing", "east"), ("has_book", "true")]);
    s.set_with(10, 1, 63, "minecraft:lectern", &[("facing", "west"), ("has_book", "true")]);

    // Lectern without book (no entity geometry, just JSON model)
    s.set_with(13, 1, 63, "minecraft:lectern", &[("facing", "south"), ("has_book", "false")]);

    // Enchanting tables
    s.set_with(1, 1, 65, "minecraft:enchanting_table", &[("facing", "south")]);
    s.set_with(4, 1, 65, "minecraft:enchanting_table", &[("facing", "north")]);
    s.set_with(7, 1, 65, "minecraft:enchanting_table", &[("facing", "east")]);
    s.set_with(10, 1, 65, "minecraft:enchanting_table", &[("facing", "west")]);

    // === Section 21 (z=67-69): Cauldron Fluid Levels ===
    s.set(1, 1, 67, "minecraft:cauldron"); // empty
    s.set_with(4, 1, 67, "minecraft:water_cauldron", &[("level", "1")]);
    s.set_with(7, 1, 67, "minecraft:water_cauldron", &[("level", "2")]);
    s.set_with(10, 1, 67, "minecraft:water_cauldron", &[("level", "3")]);
    s.set(13, 1, 67, "minecraft:lava_cauldron");
    s.set_with(16, 1, 67, "minecraft:powder_snow_cauldron", &[("level", "1")]);
    s.set_with(19, 1, 67, "minecraft:powder_snow_cauldron", &[("level", "2")]);
    s.set_with(22, 1, 67, "minecraft:powder_snow_cauldron", &[("level", "3")]);
    s.set(1, 1, 69, "minecraft:powder_snow"); // standalone block

    // === Section 22 (z=71-72): Waterlogged Blocks ===
    s.set_with(1, 1, 71, "minecraft:oak_stairs", &[
        ("facing", "south"), ("waterlogged", "true"),
    ]);
    s.set_with(4, 1, 71, "minecraft:oak_slab", &[
        ("type", "bottom"), ("waterlogged", "true"),
    ]);
    s.set_with(7, 1, 71, "minecraft:oak_fence", &[("waterlogged", "true")]);
    s.set_with(10, 1, 71, "minecraft:iron_bars", &[("waterlogged", "true")]);
    // Adjacent waterlogged blocks (water faces should cull between them)
    s.set_with(13, 1, 71, "minecraft:oak_slab", &[
        ("type", "bottom"), ("waterlogged", "true"),
    ]);
    s.set_with(14, 1, 71, "minecraft:oak_slab", &[
        ("type", "bottom"), ("waterlogged", "true"),
    ]);

    // === Section 23 (z=73-74): Signs with JSON Text + Glow ===
    // JSON text component sign
    s.set_with(1, 1, 73, "minecraft:oak_sign", &[
        ("rotation", "0"),
        ("text1", r#"{"text":"Hello","color":"red"}"#),
        ("text2", r#"{"text":"World","color":"blue"}"#),
    ]);
    // Array text component
    s.set_with(4, 1, 73, "minecraft:birch_sign", &[
        ("rotation", "0"),
        ("text1", r#"[{"text":"Multi","color":"green"},{"text":"Color","color":"gold"}]"#),
    ]);
    // Glowing sign
    s.set_with(7, 1, 73, "minecraft:oak_sign", &[
        ("rotation", "0"),
        ("text1", "Glowing"),
        ("text2", "Sign"),
        ("color", "gold"),
        ("glowing", "true"),
    ]);
    // Glowing sign with JSON
    s.set_with(10, 1, 73, "minecraft:spruce_sign", &[
        ("rotation", "0"),
        ("text1", r#"{"text":"Glow","color":"aqua"}"#),
        ("text2", r#"{"text":"JSON","color":"light_purple"}"#),
        ("glowing", "true"),
    ]);
    // Glowing wall sign
    s.set(13, 2, 74, "minecraft:stone");
    s.set_with(13, 2, 73, "minecraft:wall_oak_sign", &[
        ("facing", "south"),
        ("text1", "Glow Wall"),
        ("color", "green"),
        ("glowing", "true"),
    ]);
    // Glowing hanging sign
    s.set_with(16, 1, 73, "minecraft:oak_hanging_sign", &[
        ("rotation", "0"),
        ("text1", "Glow Hang"),
        ("color", "aqua"),
        ("glowing", "true"),
    ]);

    // === Section 24 (z=75-76): Player Heads ===
    // Steve fallback (no skin property)
    s.set_with(1, 1, 75, "minecraft:player_head", &[("rotation", "0")]);
    // Alex fallback via UUID with odd first digit
    s.set_with(4, 1, 75, "minecraft:player_head", &[
        ("rotation", "8"),
        ("uuid", "1a2b3c4d"),
    ]);
    // Wall player head
    s.set(7, 2, 76, "minecraft:stone");
    s.set_with(7, 2, 75, "minecraft:player_wall_head", &[("facing", "south")]);

    // === Section 25 (z=77-78): Player Entities ===
    // Steve (default, no skin/UUID)
    s.set_with(1, 1, 77, "entity:player", &[("facing", "south")]);
    // Alex via UUID with odd first digit
    s.set_with(4, 1, 77, "entity:player", &[("facing", "south"), ("uuid", "1a2b3c4d")]);
    // Alex via explicit slim property
    s.set_with(7, 1, 77, "entity:player", &[("facing", "south"), ("slim", "true")]);
    // Player facing east
    s.set_with(10, 1, 77, "entity:player", &[("facing", "east")]);
    // Player with pose
    s.set_with(13, 1, 77, "entity:player", &[
        ("facing", "south"),
        ("RightArmPose", "-60,20,0"),
        ("LeftArmPose", "-10,0,10"),
        ("HeadPose", "-15,0,0"),
    ]);
    // Player with walking pose
    s.set_with(16, 1, 77, "entity:player", &[
        ("facing", "south"),
        ("RightArmPose", "30,0,0"),
        ("LeftArmPose", "-30,0,0"),
        ("RightLegPose", "-30,0,0"),
        ("LeftLegPose", "30,0,0"),
    ]);
    // Player with full diamond armor
    s.set_with(19, 1, 77, "entity:player", &[
        ("facing", "south"),
        ("helmet", "diamond"),
        ("chestplate", "diamond"),
        ("leggings", "diamond"),
        ("boots", "diamond"),
    ]);
    // Player with mixed armor + pose
    s.set_with(22, 1, 77, "entity:player", &[
        ("facing", "south"),
        ("helmet", "iron"),
        ("chestplate", "diamond"),
        ("leggings", "gold"),
        ("boots", "chainmail"),
        ("RightArmPose", "45,0,0"),
        ("LeftArmPose", "-45,0,0"),
    ]);

    // === Reference blocks nearby ===
    s.set(22, 1, 1, "minecraft:oak_planks");
    s.set(22, 1, 4, "minecraft:diamond_block");
    s.set(22, 1, 8, "minecraft:iron_block");
}

fn config() -> MesherConfig {
    MesherConfig {
        cull_hidden_faces: true,
        cull_occluded_blocks: true,
        greedy_meshing: false,
        atlas_max_size: 4096,
        atlas_padding: 1,
        include_air: false,
        tint_provider: TintProvider::new(),
        ambient_occlusion: true,
        ao_intensity: 0.4,
        enable_block_light: false,
        enable_sky_light: false,
        sky_light_level: 15,
        enable_particles: true,
        pre_built_atlas: None,
    }
}

struct Scene {
    blocks: HashMap<BlockPosition, InputBlock>,
    bounds: BoundingBox,
}

impl Scene {
    fn new() -> Self {
        Self {
            blocks: HashMap::new(),
            bounds: BoundingBox::new([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
        }
    }

    fn set(&mut self, x: i32, y: i32, z: i32, name: &str) {
        self.blocks.insert(BlockPosition::new(x, y, z), InputBlock::new(name));
        self.grow(x, y, z);
    }

    fn set_with(&mut self, x: i32, y: i32, z: i32, name: &str, props: &[(&str, &str)]) {
        let mut block = InputBlock::new(name);
        for &(k, v) in props {
            block.properties.insert(k.to_string(), v.to_string());
        }
        self.blocks.insert(BlockPosition::new(x, y, z), block);
        self.grow(x, y, z);
    }

    fn grow(&mut self, x: i32, y: i32, z: i32) {
        self.bounds.min[0] = self.bounds.min[0].min(x as f32);
        self.bounds.min[1] = self.bounds.min[1].min(y as f32);
        self.bounds.min[2] = self.bounds.min[2].min(z as f32);
        self.bounds.max[0] = self.bounds.max[0].max(x as f32 + 1.0);
        self.bounds.max[1] = self.bounds.max[1].max(y as f32 + 1.0);
        self.bounds.max[2] = self.bounds.max[2].max(z as f32 + 1.0);
    }
}

impl BlockSource for Scene {
    fn get_block(&self, pos: BlockPosition) -> Option<&InputBlock> { self.blocks.get(&pos) }
    fn iter_blocks(&self) -> Box<dyn Iterator<Item = (BlockPosition, &InputBlock)> + '_> {
        Box::new(self.blocks.iter().map(|(p, b)| (*p, b)))
    }
    fn bounds(&self) -> BoundingBox { self.bounds }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let artifacts_dir = Path::new("artifacts");
    let out = artifacts_dir.join("entity.glb");
    fs::create_dir_all(artifacts_dir)?;

    let mut scene = Scene::new();
    build_scene(&mut scene);
    println!("{} blocks (entity scene)", scene.blocks.len());

    let pack = load_resource_pack("pack.zip")?;
    let mesher = Mesher::with_config(pack, config());
    let output = mesher.mesh(&scene)?;

    println!(
        "{} tris, {} verts, atlas {}x{}, {} greedy mats, {} animated textures",
        output.total_triangles(),
        output.total_vertices(),
        output.atlas.width,
        output.atlas.height,
        output.greedy_materials.len(),
        output.animated_textures.len(),
    );

    let glb = export_glb(&output)?;
    fs::write(&out, &glb)?;
    println!("wrote {}", out.display());

    Ok(())
}
