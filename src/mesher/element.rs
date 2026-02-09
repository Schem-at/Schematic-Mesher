//! Convert model elements to mesh geometry.

use crate::atlas::{AtlasBuilder, TextureAtlas};
use crate::error::{MesherError, Result};
use crate::mesher::face_culler::FaceCuller;
use crate::mesher::geometry::{Mesh, Vertex};
use crate::mesher::greedy::{FaceMergeKey, GreedyMesher, quantize_color};
use crate::mesher::entity;
use crate::mesher::liquid::{self, FluidState};
use crate::mesher::MesherConfig;
use crate::resolver::{resolve_block, ModelResolver, ResolvedModel};
use crate::resource_pack::{ModelElement, ModelFace, ResourcePack, TextureData};
use crate::types::{BlockPosition, BlockTransform, Direction, InputBlock};
use glam::{Mat3, Vec3};
use std::collections::{HashMap, HashSet};

/// Tracks texture mapping for a face (4 vertices).
struct FaceTextureMapping {
    /// Starting vertex index.
    vertex_start: u32,
    /// Starting index in the index buffer (6 indices per quad: 2 triangles).
    index_start: usize,
    /// Texture path for this face.
    texture_path: String,
    /// Whether this face uses a transparent texture.
    is_transparent: bool,
}

/// Tracks texture mapping for a greedy-merged face (bypasses atlas).
struct GreedyFaceMapping {
    /// Starting vertex index.
    vertex_start: u32,
    /// Starting index in the index buffer (6 indices per quad: 2 triangles).
    index_start: usize,
    /// Texture path for this face.
    texture_path: String,
    /// Whether this face uses a transparent texture.
    is_transparent: bool,
    /// Per-vertex AO values (from FaceMergeKey).
    ao: [u8; 4],
}

/// A material for greedy-merged faces, with its own texture (not atlas-packed).
/// UVs on these meshes exceed [0,1] for tiling via REPEAT wrapping.
#[derive(Debug)]
pub struct GreedyMaterial {
    /// Texture path (e.g., "block/stone").
    pub texture_path: String,
    /// Opaque geometry using this texture.
    pub opaque_mesh: Mesh,
    /// Transparent geometry using this texture.
    pub transparent_mesh: Mesh,
    /// PNG-encoded texture data for this material.
    pub texture_png: Vec<u8>,
}

/// Build a cache key from a block's name and properties.
/// Format: "name" for blocks with no properties, "name|k1=v1,k2=v2" with sorted keys otherwise.
fn block_cache_key(block: &InputBlock) -> String {
    if block.properties.is_empty() {
        block.name.clone()
    } else {
        let mut props: Vec<_> = block.properties.iter().collect();
        props.sort_by_key(|(k, _)| k.as_str());
        let mut key = block.name.clone();
        key.push('|');
        for (i, (k, v)) in props.iter().enumerate() {
            if i > 0 {
                key.push(',');
            }
            key.push_str(k);
            key.push('=');
            key.push_str(v);
        }
        key
    }
}

/// Builds a mesh from multiple blocks.
pub struct MeshBuilder<'a> {
    resource_pack: &'a ResourcePack,
    config: &'a MesherConfig,
    mesh: Mesh,
    texture_refs: HashSet<String>,
    model_resolver: ModelResolver<'a>,
    /// Track which texture each face uses for UV remapping.
    face_textures: Vec<FaceTextureMapping>,
    /// Track greedy-merged faces separately (bypass atlas, use tiled UVs).
    greedy_face_textures: Vec<GreedyFaceMapping>,
    /// Face culler for visibility and AO calculations.
    culler: Option<&'a FaceCuller<'a>>,
    /// Greedy mesher for merging adjacent coplanar faces.
    greedy: Option<GreedyMesher>,
    /// Cache of resolved models keyed by block identity (name + properties).
    resolve_cache: HashMap<String, Vec<ResolvedModel>>,
    /// Block map for neighbor lookups (used by liquid geometry).
    block_map: Option<&'a HashMap<BlockPosition, &'a InputBlock>>,
    /// Light map for brightness calculations.
    light_map: Option<&'a crate::mesher::lighting::LightMap>,
    /// Dynamic textures generated at build time (banners, inventories).
    /// Keys starting with `_` are synthetic texture paths.
    dynamic_textures: HashMap<String, TextureData>,
}

impl<'a> MeshBuilder<'a> {
    pub fn new(
        resource_pack: &'a ResourcePack,
        config: &'a MesherConfig,
        culler: Option<&'a FaceCuller<'a>>,
        block_map: Option<&'a HashMap<BlockPosition, &'a InputBlock>>,
        light_map: Option<&'a crate::mesher::lighting::LightMap>,
    ) -> Self {
        let greedy = if config.greedy_meshing {
            Some(GreedyMesher::new())
        } else {
            None
        };
        Self {
            resource_pack,
            config,
            mesh: Mesh::new(),
            texture_refs: HashSet::new(),
            model_resolver: ModelResolver::new(resource_pack),
            face_textures: Vec::new(),
            greedy_face_textures: Vec::new(),
            culler,
            greedy,
            resolve_cache: HashMap::new(),
            block_map,
            light_map,
            dynamic_textures: HashMap::new(),
        }
    }

    /// Add a block to the mesh.
    pub fn add_block(
        &mut self,
        pos: BlockPosition,
        block: &InputBlock,
    ) -> Result<()> {
        // Check if this is a mob entity — generate custom geometry, bypass model resolution
        if let Some(mob_type) = entity::detect_mob(block) {
            return self.add_mob(pos, block, mob_type);
        }

        // Check if this is a liquid block — generate custom geometry
        if let Some(fluid_state) = FluidState::from_block(block) {
            return self.add_liquid(pos, block, &fluid_state);
        }

        let cache_key = block_cache_key(block);

        // Check resolution cache first
        if let Some(cached) = self.resolve_cache.get(&cache_key).cloned() {
            for resolved in &cached {
                self.add_model(pos, block, resolved)?;
            }
        } else {
            // Resolve the block to models
            let resolved_models = match resolve_block(self.resource_pack, block) {
                Ok(models) => models,
                Err(e) => {
                    // Log warning but continue (don't return — entity check below)
                    eprintln!("Warning: Failed to resolve block {}: {}", block.name, e);
                    Vec::new()
                }
            };

            // Generate geometry for each model
            for resolved in &resolved_models {
                self.add_model(pos, block, resolved)?;
            }

            // Store in cache for future blocks with same identity
            self.resolve_cache.insert(cache_key, resolved_models);
        }

        // Check for block entity — generates additive geometry
        if let Some(entity_type) = entity::detect_block_entity(block) {
            self.add_entity(pos, block, &entity_type)?;
        }

        // Check for inventory property — render hologram above container
        if let Some(inventory_str) = block.properties.get("inventory") {
            self.add_inventory_hologram(pos, inventory_str)?;
        }

        // Check for particle sources (torches, campfires, candles, etc.)
        if self.config.enable_particles {
            if let Some(source) = entity::particle::detect_particle_source(block) {
                self.add_particles(pos, &source)?;
            }
        }

        // Waterlogged blocks: add water source overlay
        if liquid::is_waterlogged(block) {
            let water_state = FluidState {
                fluid_type: liquid::FluidType::Water,
                amount: 8,
                is_source: true,
                is_falling: false,
            };
            self.add_liquid(pos, block, &water_state)?;
        }

        Ok(())
    }

    /// Add liquid geometry for a water/lava block.
    fn add_liquid(
        &mut self,
        pos: BlockPosition,
        block: &InputBlock,
        state: &FluidState,
    ) -> Result<()> {
        // Get the block map for neighbor lookups
        let empty_map = HashMap::new();
        let block_map = self.block_map.unwrap_or(&empty_map);

        // Determine base color: water uses tint, lava uses white
        let base_color = match state.fluid_type {
            liquid::FluidType::Water => {
                let mut c = self.config.tint_provider.get_tint(block, 0);
                c[3] = 0.8; // Water is semi-transparent
                c
            }
            liquid::FluidType::Lava => [1.0, 1.0, 1.0, 1.0],
        };

        // Opacity check function using the culler
        let is_opaque = |p: BlockPosition| -> bool {
            self.culler.map(|c| c.is_fully_opaque_at(p)).unwrap_or(false)
        };

        let (vertices, indices, face_textures) =
            liquid::generate_fluid_geometry(pos, state, block_map, is_opaque, base_color);

        // Register texture refs
        self.texture_refs.insert(state.still_texture().to_string());
        self.texture_refs.insert(state.flow_texture().to_string());

        // Add vertices and track face texture mappings.
        // Each FaceTexture corresponds to one quad (4 vertices, 6 indices).
        let base_vertex = self.mesh.vertex_count() as u32;

        for v in &vertices {
            self.mesh.add_vertex(*v);
        }

        // Each face in face_textures corresponds to sequential groups of 4 verts / 6 indices
        let base_index = self.mesh.indices.len();

        for &idx in &indices {
            self.mesh.indices.push(base_vertex + idx);
        }

        let mut idx_offset = base_index;
        for ft in &face_textures {
            // Find the minimum vertex index in this face's 6 indices to determine vertex_start
            let face_v_start = (idx_offset - base_index) as u32 / 6 * 4 + base_vertex;
            self.face_textures.push(FaceTextureMapping {
                vertex_start: face_v_start,
                index_start: idx_offset,
                texture_path: ft.texture.to_string(),
                is_transparent: ft.is_transparent,
            });
            idx_offset += 6;
        }

        Ok(())
    }

    /// Add entity geometry for a block entity (chest, bed, bell, sign, skull, banner).
    fn add_entity(
        &mut self,
        pos: BlockPosition,
        block: &InputBlock,
        entity_type: &entity::BlockEntityType,
    ) -> Result<()> {
        // Banner: composite texture before generating geometry
        if let entity::BlockEntityType::Banner { color, is_wall } = entity_type {
            return self.add_banner(pos, block, color, *is_wall);
        }

        // Sign with text: composite text onto texture
        if let entity::BlockEntityType::Sign { wood, is_wall } = entity_type {
            if block.properties.contains_key("text1")
                || block.properties.contains_key("text2")
                || block.properties.contains_key("text3")
                || block.properties.contains_key("text4")
            {
                return self.add_sign_with_text(pos, block, *wood, *is_wall);
            }
        }

        // Hanging sign with text
        if let entity::BlockEntityType::HangingSign { wood, is_wall } = entity_type {
            if block.properties.contains_key("text1")
                || block.properties.contains_key("text2")
                || block.properties.contains_key("text3")
                || block.properties.contains_key("text4")
            {
                return self.add_hanging_sign_with_text(pos, block, *wood, *is_wall);
            }
        }

        // Player head: custom texture handling with dynamic skin
        if let entity::BlockEntityType::Skull(entity::SkullType::Player) = entity_type {
            return self.add_player_head(pos, block);
        }

        // Decorated pot: custom per-face geometry
        if matches!(entity_type, entity::BlockEntityType::DecoratedPot) {
            return self.add_decorated_pot(pos, block);
        }

        let (vertices, indices, face_textures) =
            entity::generate_entity_geometry(block, entity_type);

        if vertices.is_empty() {
            return Ok(());
        }

        // Register entity texture
        if let Some(ft) = face_textures.first() {
            self.texture_refs.insert(ft.texture.clone());
        }

        // Add vertices with block position offset and optional lighting
        let base_vertex = self.mesh.vertex_count() as u32;
        let base_index = self.mesh.indices.len();

        let is_emissive = self.light_map.map(|lm| lm.is_emissive(pos)).unwrap_or(false);

        for v in &vertices {
            let mut vertex = *v;
            // Offset to world position (entity geometry is in [0,1] block-local space,
            // but regular blocks use [-0.5, 0.5] centered convention, so subtract 0.5)
            vertex.position[0] += pos.x as f32 - 0.5;
            vertex.position[1] += pos.y as f32 - 0.5;
            vertex.position[2] += pos.z as f32 - 0.5;

            // Apply lighting if enabled
            if !is_emissive {
                if let Some(lm) = self.light_map {
                    // Use average brightness for entity (no per-face direction available yet)
                    let brightness = lm.face_brightness(pos, Direction::Up);
                    vertex.color[0] *= brightness;
                    vertex.color[1] *= brightness;
                    vertex.color[2] *= brightness;
                }
            }

            self.mesh.add_vertex(vertex);
        }

        for &idx in &indices {
            self.mesh.indices.push(base_vertex + idx);
        }

        // Track face texture mappings for atlas UV remapping
        let mut idx_offset = base_index;
        for ft in &face_textures {
            let face_v_start = (idx_offset - base_index) as u32 / 6 * 4 + base_vertex;
            self.face_textures.push(FaceTextureMapping {
                vertex_start: face_v_start,
                index_start: idx_offset,
                texture_path: ft.texture.clone(),
                is_transparent: ft.is_transparent,
            });
            idx_offset += 6;
        }

        Ok(())
    }

    /// Add mob entity geometry (zombie, skeleton, creeper, pig, item frames, dropped items).
    fn add_mob(
        &mut self,
        pos: BlockPosition,
        block: &InputBlock,
        mob_type: entity::MobType,
    ) -> Result<()> {
        let (vertices, indices, face_textures) =
            entity::generate_mob_geometry(block, mob_type);

        // Add base geometry (may be empty for dropped items)
        if !vertices.is_empty() {
            for ft in &face_textures {
                self.texture_refs.insert(ft.texture.clone());
            }

            let base_vertex = self.mesh.vertex_count() as u32;
            let base_index = self.mesh.indices.len();

            for v in &vertices {
                let mut vertex = *v;
                vertex.position[0] += pos.x as f32 - 0.5;
                vertex.position[1] += pos.y as f32 - 0.5;
                vertex.position[2] += pos.z as f32 - 0.5;
                self.mesh.add_vertex(vertex);
            }

            for &idx in &indices {
                self.mesh.indices.push(base_vertex + idx);
            }

            let mut idx_offset = base_index;
            for ft in &face_textures {
                let face_v_start = (idx_offset - base_index) as u32 / 6 * 4 + base_vertex;
                self.face_textures.push(FaceTextureMapping {
                    vertex_start: face_v_start,
                    index_start: idx_offset,
                    texture_path: ft.texture.clone(),
                    is_transparent: ft.is_transparent,
                });
                idx_offset += 6;
            }
        }

        // Item frames: render item inside the frame if "item" property is set
        if matches!(mob_type, entity::MobType::ItemFrame | entity::MobType::GlowItemFrame) {
            if let Some(item_id) = block.properties.get("item") {
                let item_rotation: u8 = block.properties.get("item_rotation")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
                let facing = block.properties.get("facing")
                    .map(|s| s.as_str())
                    .unwrap_or("south");

                if let Some((item_verts, item_indices, item_faces)) =
                    entity::item_render::render_item_in_frame(
                        self.resource_pack, &self.model_resolver,
                        item_id, item_rotation, facing,
                    )
                {
                    self.add_item_geometry(pos, &item_verts, &item_indices, &item_faces);
                }
            }
        }

        // Dropped items: render via item_render module
        if matches!(mob_type, entity::MobType::DroppedItem) {
            if let Some(item_id) = block.properties.get("item") {
                let facing = block.properties.get("facing")
                    .map(|s| s.as_str())
                    .unwrap_or("south");

                if let Some((item_verts, item_indices, item_faces)) =
                    entity::item_render::render_dropped_item(
                        self.resource_pack, &self.model_resolver,
                        item_id, facing,
                    )
                {
                    self.add_item_geometry(pos, &item_verts, &item_indices, &item_faces);
                }
            }
        }

        // Sheep: render wool overlay
        if matches!(mob_type, entity::MobType::Sheep) {
            let mut wool_model = crate::mesher::entity::sheep::sheep_wool_model();
            // Apply baby scaling to wool overlay too
            if block.properties.get("is_baby").map(|v| v == "true").unwrap_or(false) {
                if let Some(root) = wool_model.parts.first_mut() {
                    root.pose.scale = [0.5, 0.5, 0.5];
                    root.pose.position[1] = 12.0;
                }
            }
            let facing = block.properties.get("facing")
                .map(|s| s.as_str())
                .unwrap_or("south");
            let facing_angle = entity::facing_rotation_rad(facing);
            let facing_mat = glam::Mat4::from_translation(glam::Vec3::new(0.5, 0.0, 0.5))
                * glam::Mat4::from_rotation_y(facing_angle)
                * glam::Mat4::from_translation(glam::Vec3::new(-0.5, 0.0, -0.5));

            let mut wool_verts = Vec::new();
            let mut wool_indices = Vec::new();
            let mut wool_faces = Vec::new();

            entity::traverse_parts(
                &wool_model.parts,
                glam::Mat4::IDENTITY,
                &facing_mat,
                &wool_model,
                &mut wool_verts,
                &mut wool_indices,
                &mut wool_faces,
            );

            // Apply dye color tint to wool vertices
            let dye_color = block.properties.get("color")
                .map(|c| dye_rgb(c))
                .unwrap_or([1.0, 1.0, 1.0, 1.0]);
            for v in &mut wool_verts {
                v.color[0] *= dye_color[0];
                v.color[1] *= dye_color[1];
                v.color[2] *= dye_color[2];
            }

            if !wool_verts.is_empty() {
                self.add_item_geometry(pos, &wool_verts, &wool_indices, &wool_faces);
            }
        }

        // Players: dynamic skin texture + direct model generation
        if matches!(mob_type, entity::MobType::Player) {
            let tex_key = format!("_player/{}_{}_{}", pos.x, pos.y, pos.z);
            if !self.dynamic_textures.contains_key(&tex_key) {
                // Try hex skin property
                if let Some(skin_hex) = block.properties.get("skin") {
                    if let Some(skin_tex) = entity::skull::decode_hex_skin(skin_hex) {
                        self.dynamic_textures.insert(tex_key.clone(), skin_tex);
                    }
                }
                // Fallback to Steve/Alex from resource pack
                if !self.dynamic_textures.contains_key(&tex_key) {
                    let fallback = entity::skull::player_skin_fallback_path(block);
                    if let Some(tex) = self.resource_pack.get_texture(fallback) {
                        self.dynamic_textures.insert(tex_key.clone(), tex.clone());
                    }
                }
            }

            // Build player model with dynamic texture key
            let model = entity::player::player_model(block, &tex_key);

            let facing = block.properties.get("facing")
                .map(|s| s.as_str())
                .unwrap_or("south");
            let facing_angle = entity::facing_rotation_rad(facing);
            let facing_mat = glam::Mat4::from_translation(glam::Vec3::new(0.5, 0.0, 0.5))
                * glam::Mat4::from_rotation_y(facing_angle)
                * glam::Mat4::from_translation(glam::Vec3::new(-0.5, 0.0, -0.5));

            let mut player_verts = Vec::new();
            let mut player_indices = Vec::new();
            let mut player_faces = Vec::new();

            entity::traverse_parts(
                &model.parts,
                glam::Mat4::IDENTITY,
                &facing_mat,
                &model,
                &mut player_verts,
                &mut player_indices,
                &mut player_faces,
            );

            if !player_verts.is_empty() {
                self.add_item_geometry(pos, &player_verts, &player_indices, &player_faces);
            }
        }

        // Armor stands and players: render armor overlay if armor properties are set
        if matches!(mob_type, entity::MobType::ArmorStand | entity::MobType::Player) {
            let facing = block.properties.get("facing")
                .map(|s| s.as_str())
                .unwrap_or("south");
            let (armor_verts, armor_indices, armor_faces) =
                entity::armor_stand::generate_armor_geometry(block, facing);
            if !armor_verts.is_empty() {
                self.add_item_geometry(pos, &armor_verts, &armor_indices, &armor_faces);
            }
        }

        Ok(())
    }

    /// Add inventory hologram above a container block.
    fn add_inventory_hologram(
        &mut self,
        pos: BlockPosition,
        inventory_str: &str,
    ) -> Result<()> {
        if let Some((mut verts, indices, mut face_textures, tex_data)) =
            entity::inventory::render_inventory_hologram(
                self.resource_pack,
                &self.model_resolver,
                inventory_str,
            )
        {
            // Generate unique texture key for this inventory
            let tex_key = format!("_inventory/{}_{}_{}", pos.x, pos.y, pos.z);

            // Store the composited texture
            self.dynamic_textures.insert(tex_key.clone(), tex_data);

            // Set the texture path on all face textures
            for ft in &mut face_textures {
                ft.texture = tex_key.clone();
            }

            // Offset vertices to world position
            for v in &mut verts {
                v.position[0] += pos.x as f32 - 0.5;
                v.position[1] += pos.y as f32 - 0.5;
                v.position[2] += pos.z as f32 - 0.5;
            }

            // Register texture and add geometry
            self.texture_refs.insert(tex_key);

            let base_vertex = self.mesh.vertex_count() as u32;
            let base_index = self.mesh.indices.len();

            for v in &verts {
                self.mesh.add_vertex(*v);
            }

            for &idx in &indices {
                self.mesh.indices.push(base_vertex + idx);
            }

            let mut idx_offset = base_index;
            for ft in &face_textures {
                let face_v_start = (idx_offset - base_index) as u32 / 6 * 4 + base_vertex;
                self.face_textures.push(FaceTextureMapping {
                    vertex_start: face_v_start,
                    index_start: idx_offset,
                    texture_path: ft.texture.clone(),
                    is_transparent: ft.is_transparent,
                });
                idx_offset += 6;
            }
        }

        Ok(())
    }

    /// Add static particle marker quads (cross-quads for flames, smoke, etc.).
    /// Builds animated sprite sheets for particle textures and stores them as
    /// dynamic textures so the viewer can cycle frames automatically.
    fn add_particles(
        &mut self,
        pos: BlockPosition,
        source: &entity::particle::ParticleSource,
    ) -> Result<()> {
        // Build animated sprite sheets for particle textures (if not already built)
        for quad in &source.quads {
            if let Some(anim) = entity::particle::particle_anim_def(quad.texture) {
                if !self.dynamic_textures.contains_key(anim.key) {
                    if let Some(tex) = entity::particle::build_particle_sprite_sheet(
                        self.resource_pack, anim,
                    ) {
                        self.dynamic_textures.insert(anim.key.to_string(), tex);
                    }
                }
            }
        }

        let (vertices, indices, face_textures) =
            entity::particle::generate_particle_geometry(source);

        if vertices.is_empty() {
            return Ok(());
        }

        // Register texture refs using animated key when available
        for ft in &face_textures {
            let tex_key = entity::particle::particle_anim_def(&ft.texture)
                .map(|a| a.key.to_string())
                .unwrap_or_else(|| ft.texture.clone());
            self.texture_refs.insert(tex_key);
        }

        let base_vertex = self.mesh.vertex_count() as u32;
        let base_index = self.mesh.indices.len();

        for v in &vertices {
            let mut vertex = *v;
            vertex.position[0] += pos.x as f32 - 0.5;
            vertex.position[1] += pos.y as f32 - 0.5;
            vertex.position[2] += pos.z as f32 - 0.5;
            self.mesh.add_vertex(vertex);
        }

        for &idx in &indices {
            self.mesh.indices.push(base_vertex + idx);
        }

        let mut idx_offset = base_index;
        for ft in &face_textures {
            // Use animated texture key if this particle has an animation def
            let tex_key = entity::particle::particle_anim_def(&ft.texture)
                .map(|a| a.key.to_string())
                .unwrap_or_else(|| ft.texture.clone());
            let face_v_start = (idx_offset - base_index) as u32 / 6 * 4 + base_vertex;
            self.face_textures.push(FaceTextureMapping {
                vertex_start: face_v_start,
                index_start: idx_offset,
                texture_path: tex_key,
                is_transparent: ft.is_transparent,
            });
            idx_offset += 6;
        }

        Ok(())
    }

    /// Add banner entity geometry with composited texture.
    fn add_banner(
        &mut self,
        pos: BlockPosition,
        block: &InputBlock,
        base_color: &str,
        is_wall: bool,
    ) -> Result<()> {
        // Parse pattern property
        let patterns = block.properties.get("patterns")
            .map(|s| entity::banner::parse_patterns(s))
            .unwrap_or_default();

        // Create a cache key for this banner's texture
        let mut tex_key = format!("_banner/{}", base_color);
        for (p, c) in &patterns {
            tex_key.push_str(&format!("_{}{}", p, c));
        }

        // Composite the texture if not already cached
        if !self.dynamic_textures.contains_key(&tex_key) {
            if let Some(tex) = entity::banner::composite_banner_texture(
                self.resource_pack,
                base_color,
                &patterns,
            ) {
                self.dynamic_textures.insert(tex_key.clone(), tex);
            }
        }

        // Build the banner model with the composited texture path
        let model = entity::banner::banner_model(!is_wall, &tex_key);

        // Compute facing
        let facing = entity::get_facing(block);
        let facing_angle = if !is_wall {
            entity::standing_rotation_rad(block)
        } else {
            entity::facing_rotation_rad(facing)
        };
        let facing_mat = glam::Mat4::from_translation(glam::Vec3::new(0.5, 0.0, 0.5))
            * glam::Mat4::from_rotation_y(facing_angle)
            * glam::Mat4::from_translation(glam::Vec3::new(-0.5, 0.0, -0.5));

        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut face_textures = Vec::new();

        entity::traverse_parts(
            &model.parts,
            glam::Mat4::IDENTITY,
            &facing_mat,
            &model,
            &mut vertices,
            &mut indices,
            &mut face_textures,
        );

        if vertices.is_empty() {
            return Ok(());
        }

        self.add_item_geometry(pos, &vertices, &indices, &face_textures);

        Ok(())
    }

    /// Add sign entity with text composited onto the texture.
    fn add_sign_with_text(
        &mut self,
        pos: BlockPosition,
        block: &InputBlock,
        wood: entity::SignWood,
        is_wall: bool,
    ) -> Result<()> {
        let base_texture = entity::sign::sign_texture_path(wood);
        let color = block.properties.get("color")
            .map(|s| s.as_str())
            .unwrap_or("black");
        let glowing = block.properties.get("glowing")
            .map(|s| s == "true")
            .unwrap_or(false);

        let lines: Vec<&str> = (1..=4)
            .filter_map(|i| {
                block.properties.get(&format!("text{}", i))
                    .map(|s| s.as_str())
            })
            .collect();

        // Generate unique texture key (includes glowing flag)
        let mut tex_key = format!("_sign/{}_{}_{}", base_texture, color, if glowing { "glow" } else { "normal" });
        for line in &lines {
            tex_key.push('_');
            tex_key.push_str(line);
        }

        // Composite texture if not cached
        if !self.dynamic_textures.contains_key(&tex_key) {
            if let Some(tex) = entity::sign_text::composite_sign_with_text(
                self.resource_pack, base_texture, &lines, color, glowing,
            ) {
                self.dynamic_textures.insert(tex_key.clone(), tex);
            } else {
                // Fallback to normal sign rendering (no font available)
                let (vertices, indices, face_textures) =
                    entity::generate_entity_geometry(block, &entity::BlockEntityType::Sign { wood, is_wall });
                if !vertices.is_empty() {
                    self.add_item_geometry(pos, &vertices, &indices, &face_textures);
                }
                return Ok(());
            }
        }

        // Build sign model with custom texture (4x upscaled)
        let model = entity::sign::sign_model_upscaled(&tex_key, is_wall);

        // Compute facing
        let facing_angle = if !is_wall {
            entity::standing_rotation_rad(block)
        } else {
            entity::facing_rotation_rad(entity::get_facing(block))
        };
        let facing_mat = glam::Mat4::from_translation(glam::Vec3::new(0.5, 0.0, 0.5))
            * glam::Mat4::from_rotation_y(facing_angle)
            * glam::Mat4::from_translation(glam::Vec3::new(-0.5, 0.0, -0.5));

        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut face_textures = Vec::new();

        entity::traverse_parts(
            &model.parts,
            glam::Mat4::IDENTITY,
            &facing_mat,
            &model,
            &mut vertices,
            &mut indices,
            &mut face_textures,
        );

        if !vertices.is_empty() {
            self.add_item_geometry(pos, &vertices, &indices, &face_textures);
        }

        Ok(())
    }

    /// Add decorated pot with per-face sherd textures.
    fn add_decorated_pot(
        &mut self,
        pos: BlockPosition,
        block: &InputBlock,
    ) -> Result<()> {
        let (vertices, indices, face_textures) =
            entity::decorated_pot::generate_decorated_pot_geometry(block);

        if vertices.is_empty() {
            return Ok(());
        }

        self.add_item_geometry(pos, &vertices, &indices, &face_textures);

        Ok(())
    }

    /// Add player head entity with skin texture support.
    fn add_player_head(
        &mut self,
        pos: BlockPosition,
        block: &InputBlock,
    ) -> Result<()> {
        let block_id = block.block_id();
        let is_wall = block_id.contains("wall");

        // Determine texture key
        let tex_key = format!("_player_head/{}_{}_{}", pos.x, pos.y, pos.z);

        // Try to decode hex skin property, otherwise use fallback
        if !self.dynamic_textures.contains_key(&tex_key) {
            if let Some(skin_hex) = block.properties.get("skin") {
                if let Some(skin_tex) = entity::skull::decode_hex_skin(skin_hex) {
                    self.dynamic_textures.insert(tex_key.clone(), skin_tex);
                }
            }
            // If no skin decoded, load fallback from resource pack
            if !self.dynamic_textures.contains_key(&tex_key) {
                let fallback_path = entity::skull::player_skin_fallback_path(block);
                if let Some(fallback_tex) = self.resource_pack.get_texture(fallback_path) {
                    self.dynamic_textures.insert(tex_key.clone(), fallback_tex.clone());
                }
            }
        }

        // Build player skull model with the texture key
        let model = entity::skull::player_skull_model(&tex_key);

        // Compute facing/rotation (same logic as regular skulls)
        let facing_angle = if is_wall {
            entity::facing_rotation_rad(entity::get_facing(block))
        } else {
            entity::standing_rotation_rad(block)
        };
        let facing_mat = glam::Mat4::from_translation(glam::Vec3::new(0.5, 0.0, 0.5))
            * glam::Mat4::from_rotation_y(facing_angle)
            * glam::Mat4::from_translation(glam::Vec3::new(-0.5, 0.0, -0.5));

        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut face_textures = Vec::new();

        entity::traverse_parts(
            &model.parts,
            glam::Mat4::IDENTITY,
            &facing_mat,
            &model,
            &mut vertices,
            &mut indices,
            &mut face_textures,
        );

        if !vertices.is_empty() {
            self.add_item_geometry(pos, &vertices, &indices, &face_textures);
        }

        Ok(())
    }

    /// Add hanging sign entity with text composited onto the texture.
    fn add_hanging_sign_with_text(
        &mut self,
        pos: BlockPosition,
        block: &InputBlock,
        wood: entity::SignWood,
        is_wall: bool,
    ) -> Result<()> {
        let base_texture = entity::hanging_sign::hanging_sign_texture_path(wood);
        let color = block.properties.get("color")
            .map(|s| s.as_str())
            .unwrap_or("black");
        let glowing = block.properties.get("glowing")
            .map(|s| s == "true")
            .unwrap_or(false);

        let lines: Vec<&str> = (1..=4)
            .filter_map(|i| {
                block.properties.get(&format!("text{}", i))
                    .map(|s| s.as_str())
            })
            .collect();

        let mut tex_key = format!("_hanging_sign/{}_{}_{}", base_texture, color, if glowing { "glow" } else { "normal" });
        for line in &lines {
            tex_key.push('_');
            tex_key.push_str(line);
        }

        if !self.dynamic_textures.contains_key(&tex_key) {
            if let Some(tex) = entity::sign_text::composite_sign_with_text(
                self.resource_pack, base_texture, &lines, color, glowing,
            ) {
                self.dynamic_textures.insert(tex_key.clone(), tex);
            } else {
                // Fallback to normal hanging sign rendering (no font available)
                let (vertices, indices, face_textures) =
                    entity::generate_entity_geometry(block, &entity::BlockEntityType::HangingSign { wood, is_wall });
                if !vertices.is_empty() {
                    self.add_item_geometry(pos, &vertices, &indices, &face_textures);
                }
                return Ok(());
            }
        }

        let model = entity::hanging_sign::hanging_sign_model_upscaled(&tex_key, is_wall);

        let facing_angle = if !is_wall {
            entity::standing_rotation_rad(block)
        } else {
            entity::facing_rotation_rad(entity::get_facing(block))
        };
        let facing_mat = glam::Mat4::from_translation(glam::Vec3::new(0.5, 0.0, 0.5))
            * glam::Mat4::from_rotation_y(facing_angle)
            * glam::Mat4::from_translation(glam::Vec3::new(-0.5, 0.0, -0.5));

        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut face_textures = Vec::new();

        entity::traverse_parts(
            &model.parts,
            glam::Mat4::IDENTITY,
            &facing_mat,
            &model,
            &mut vertices,
            &mut indices,
            &mut face_textures,
        );

        if !vertices.is_empty() {
            self.add_item_geometry(pos, &vertices, &indices, &face_textures);
        }

        Ok(())
    }

    /// Add pre-transformed item/overlay geometry at a world position.
    fn add_item_geometry(
        &mut self,
        pos: BlockPosition,
        item_verts: &[Vertex],
        item_indices: &[u32],
        item_faces: &[entity::EntityFaceTexture],
    ) {
        for ft in item_faces {
            self.texture_refs.insert(ft.texture.clone());
        }

        let item_base_vertex = self.mesh.vertex_count() as u32;
        let item_base_index = self.mesh.indices.len();

        for v in item_verts {
            let mut vertex = *v;
            vertex.position[0] += pos.x as f32 - 0.5;
            vertex.position[1] += pos.y as f32 - 0.5;
            vertex.position[2] += pos.z as f32 - 0.5;
            self.mesh.add_vertex(vertex);
        }

        for &idx in item_indices {
            self.mesh.indices.push(item_base_vertex + idx);
        }

        let mut item_idx_offset = item_base_index;
        for ft in item_faces {
            let face_v_start = (item_idx_offset - item_base_index) as u32 / 6 * 4
                + item_base_vertex;
            self.face_textures.push(FaceTextureMapping {
                vertex_start: face_v_start,
                index_start: item_idx_offset,
                texture_path: ft.texture.clone(),
                is_transparent: ft.is_transparent,
            });
            item_idx_offset += 6;
        }
    }

    /// Add a resolved model to the mesh.
    fn add_model(
        &mut self,
        pos: BlockPosition,
        block: &InputBlock,
        resolved: &ResolvedModel,
    ) -> Result<()> {
        let model = &resolved.model;
        let transform = &resolved.transform;

        // Resolve textures for this model
        let resolved_textures = self.model_resolver.resolve_textures(model);

        // Process each element
        for element in &model.elements {
            self.add_element(pos, block, element, transform, &resolved_textures)?;
        }

        Ok(())
    }

    /// Check if an element/face is eligible for greedy merging.
    fn is_greedy_eligible(
        &self,
        element: &ModelElement,
        face: &ModelFace,
        transform: &BlockTransform,
    ) -> bool {
        // Must be a full cube element: from=[0,0,0], to=[16,16,16]
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

        // No element rotation
        if element.rotation.is_some() {
            return false;
        }

        // Block transform must be identity
        if !transform.is_identity() {
            return false;
        }

        // UV must cover full texture (default [0,0,16,16])
        let uv = face.uv_or_default();
        if uv[0].abs() > EPSILON
            || uv[1].abs() > EPSILON
            || (uv[2] - 16.0).abs() > EPSILON
            || (uv[3] - 16.0).abs() > EPSILON
        {
            return false;
        }

        // UV rotation must be 0
        if face.rotation != 0 {
            return false;
        }

        true
    }

    /// Add an element to the mesh.
    fn add_element(
        &mut self,
        pos: BlockPosition,
        block: &InputBlock,
        element: &ModelElement,
        transform: &BlockTransform,
        resolved_textures: &std::collections::HashMap<String, String>,
    ) -> Result<()> {
        // Compute lighting factor for this block position
        let is_emissive = self.light_map.map(|lm| lm.is_emissive(pos)).unwrap_or(false);

        // Process each face
        for (direction, face) in &element.faces {
            // Transform the face direction by block rotation (for AO and cullface)
            let world_direction = direction.rotate_by_transform(transform.x, transform.y);

            // Check if face should be culled
            if let Some(cullface) = &face.cullface {
                if let Some(culler) = self.culler {
                    // Transform cullface direction to world space
                    let world_cullface = cullface.rotate_by_transform(transform.x, transform.y);
                    if culler.should_cull(pos, world_cullface) {
                        continue;
                    }
                }
            }

            // Resolve the texture reference
            let texture_path = self.resolve_face_texture(&face.texture, resolved_textures);
            self.texture_refs.insert(texture_path.clone());

            // Check if texture has transparency
            let is_transparent = self.resource_pack
                .get_texture(&texture_path)
                .map(|t| t.has_transparency())
                .unwrap_or(false);

            // Compute light factor for this face
            let light_factor = if is_emissive {
                1.0 // Emissive blocks are always fully bright
            } else if let Some(lm) = self.light_map {
                lm.face_brightness(pos, world_direction)
            } else {
                1.0 // No lighting → full brightness
            };

            // Quantized light level for greedy merge key (0-15)
            let light_key = if self.light_map.is_some() {
                (light_factor * 15.0).round() as u8
            } else {
                15
            };

            // Route to greedy mesher if eligible
            if self.greedy.is_some() && self.is_greedy_eligible(element, face, transform) {
                let mut base_color = self.config.tint_provider.get_tint(block, face.tintindex);
                // Apply lighting to tint color before quantization
                base_color[0] *= light_factor;
                base_color[1] *= light_factor;
                base_color[2] *= light_factor;
                // Compute per-vertex AO for this face so it's included in the merge key
                let ao = if self.config.ambient_occlusion && !is_emissive {
                    self.culler
                        .map(|c| c.calculate_ao(pos, world_direction))
                        .unwrap_or([3, 3, 3, 3])
                } else {
                    [3, 3, 3, 3]
                };
                let key = FaceMergeKey {
                    texture: texture_path,
                    tint: quantize_color(base_color),
                    ao,
                    light: light_key,
                };
                self.greedy.as_mut().unwrap().add_face(
                    pos,
                    world_direction,
                    key,
                    is_transparent,
                );
                continue;
            }

            // Track texture mapping for UV remapping
            let vertex_start = self.mesh.vertex_count() as u32;
            let index_start = self.mesh.indices.len();
            self.face_textures.push(FaceTextureMapping {
                vertex_start,
                index_start,
                texture_path,
                is_transparent,
            });

            // Calculate AO if enabled (use world direction for neighbor checks)
            // Skip AO for emissive blocks
            let ao_values = if self.config.ambient_occlusion && !is_emissive {
                self.culler.map(|c| c.calculate_ao(pos, world_direction))
            } else {
                None
            };

            // Generate face geometry (with lighting applied)
            self.add_face(pos, block, element, *direction, face, transform, ao_values, light_factor)?;
        }

        Ok(())
    }

    /// Resolve a texture reference to a path.
    fn resolve_face_texture(
        &self,
        reference: &str,
        resolved_textures: &std::collections::HashMap<String, String>,
    ) -> String {
        if reference.starts_with('#') {
            let key = &reference[1..];
            resolved_textures
                .get(key)
                .cloned()
                .unwrap_or_else(|| "block/missing".to_string())
        } else {
            reference.to_string()
        }
    }

    /// Add a face to the mesh.
    fn add_face(
        &mut self,
        pos: BlockPosition,
        block: &InputBlock,
        element: &ModelElement,
        direction: Direction,
        face: &ModelFace,
        transform: &BlockTransform,
        ao_values: Option<[u8; 4]>,
        light_factor: f32,
    ) -> Result<()> {
        let normal = direction.normal();
        let uv = face.normalized_uv();

        // Get element bounds in normalized space
        let from = element.normalized_from();
        let to = element.normalized_to();

        // Generate the 4 vertices for this face
        let (positions, uvs) = self.generate_face_vertices(direction, from, to, uv, face.rotation);

        // Apply element rotation if present
        let positions = if let Some(rot) = &element.rotation {
            self.apply_element_rotation(&positions, rot)
        } else {
            positions
        };

        // Apply block transform rotation
        let positions = self.apply_block_transform(&positions, transform);
        let normal = self.rotate_normal(normal, transform);

        // Translate to world position
        let offset = [pos.x as f32, pos.y as f32, pos.z as f32];

        // Get tint color from the tint provider based on block type and tint index
        let base_color = self.config.tint_provider.get_tint(block, face.tintindex);

        // Calculate per-vertex colors with AO and lighting
        let colors = if let Some(ao) = ao_values {
            let intensity = self.config.ao_intensity;
            [
                apply_ao_and_light(base_color, ao[0], intensity, light_factor),
                apply_ao_and_light(base_color, ao[1], intensity, light_factor),
                apply_ao_and_light(base_color, ao[2], intensity, light_factor),
                apply_ao_and_light(base_color, ao[3], intensity, light_factor),
            ]
        } else {
            let lit = [
                base_color[0] * light_factor,
                base_color[1] * light_factor,
                base_color[2] * light_factor,
                base_color[3],
            ];
            [lit; 4]
        };

        let v0 = self.mesh.add_vertex(
            Vertex::new(
                [
                    positions[0][0] + offset[0],
                    positions[0][1] + offset[1],
                    positions[0][2] + offset[2],
                ],
                normal,
                uvs[0],
            )
            .with_color(colors[0]),
        );
        let v1 = self.mesh.add_vertex(
            Vertex::new(
                [
                    positions[1][0] + offset[0],
                    positions[1][1] + offset[1],
                    positions[1][2] + offset[2],
                ],
                normal,
                uvs[1],
            )
            .with_color(colors[1]),
        );
        let v2 = self.mesh.add_vertex(
            Vertex::new(
                [
                    positions[2][0] + offset[0],
                    positions[2][1] + offset[1],
                    positions[2][2] + offset[2],
                ],
                normal,
                uvs[2],
            )
            .with_color(colors[2]),
        );
        let v3 = self.mesh.add_vertex(
            Vertex::new(
                [
                    positions[3][0] + offset[0],
                    positions[3][1] + offset[1],
                    positions[3][2] + offset[2],
                ],
                normal,
                uvs[3],
            )
            .with_color(colors[3]),
        );

        // Use AO-aware quad triangulation to fix anisotropy
        if let Some(ao) = ao_values {
            self.mesh.add_quad_ao(v0, v1, v2, v3, ao);
        } else {
            self.mesh.add_quad(v0, v1, v2, v3);
        }

        Ok(())
    }

    /// Generate the 4 vertices for a face.
    /// Returns (positions, uvs) in CCW order.
    fn generate_face_vertices(
        &self,
        direction: Direction,
        from: [f32; 3],
        to: [f32; 3],
        uv: [f32; 4],
        rotation: i32,
    ) -> ([[f32; 3]; 4], [[f32; 2]; 4]) {
        let (u1, v1, u2, v2) = (uv[0], uv[1], uv[2], uv[3]);

        // Base UVs (before rotation)
        // glTF uses v=0 at top (same as Minecraft), so no V flip needed
        // UV order: top-left, top-right, bottom-right, bottom-left (CCW)
        let base_uvs = [[u1, v1], [u2, v1], [u2, v2], [u1, v2]];

        // Rotate UVs
        let uvs = self.rotate_uvs(base_uvs, rotation);

        // Generate positions based on direction
        let positions = match direction {
            Direction::Down => [
                [from[0], from[1], to[2]],
                [to[0], from[1], to[2]],
                [to[0], from[1], from[2]],
                [from[0], from[1], from[2]],
            ],
            Direction::Up => [
                [from[0], to[1], from[2]],
                [to[0], to[1], from[2]],
                [to[0], to[1], to[2]],
                [from[0], to[1], to[2]],
            ],
            Direction::North => [
                [to[0], to[1], from[2]],
                [from[0], to[1], from[2]],
                [from[0], from[1], from[2]],
                [to[0], from[1], from[2]],
            ],
            Direction::South => [
                [from[0], to[1], to[2]],
                [to[0], to[1], to[2]],
                [to[0], from[1], to[2]],
                [from[0], from[1], to[2]],
            ],
            Direction::West => [
                [from[0], to[1], from[2]],
                [from[0], to[1], to[2]],
                [from[0], from[1], to[2]],
                [from[0], from[1], from[2]],
            ],
            Direction::East => [
                [to[0], to[1], to[2]],
                [to[0], to[1], from[2]],
                [to[0], from[1], from[2]],
                [to[0], from[1], to[2]],
            ],
        };

        (positions, uvs)
    }

    /// Rotate UV coordinates.
    fn rotate_uvs(&self, uvs: [[f32; 2]; 4], rotation: i32) -> [[f32; 2]; 4] {
        let steps = ((rotation / 90) % 4 + 4) % 4;
        let mut result = uvs;
        for _ in 0..steps {
            result = [result[3], result[0], result[1], result[2]];
        }
        result
    }

    /// Apply element rotation to positions.
    fn apply_element_rotation(
        &self,
        positions: &[[f32; 3]; 4],
        rotation: &crate::types::ElementRotation,
    ) -> [[f32; 3]; 4] {
        let origin = rotation.normalized_origin();
        let angle = rotation.angle_radians();
        let rescale = rotation.rescale_factor();

        let rotation_matrix = match rotation.axis {
            crate::types::Axis::X => Mat3::from_rotation_x(angle),
            crate::types::Axis::Y => Mat3::from_rotation_y(angle),
            crate::types::Axis::Z => Mat3::from_rotation_z(angle),
        };

        let mut result = [[0.0; 3]; 4];
        for (i, pos) in positions.iter().enumerate() {
            // Translate to origin
            let p = Vec3::new(pos[0] - origin[0], pos[1] - origin[1], pos[2] - origin[2]);

            // Rotate
            let rotated = rotation_matrix * p;

            // Rescale if needed
            let scaled = if rescale != 1.0 {
                match rotation.axis {
                    crate::types::Axis::X => {
                        Vec3::new(rotated.x, rotated.y * rescale, rotated.z * rescale)
                    }
                    crate::types::Axis::Y => {
                        Vec3::new(rotated.x * rescale, rotated.y, rotated.z * rescale)
                    }
                    crate::types::Axis::Z => {
                        Vec3::new(rotated.x * rescale, rotated.y * rescale, rotated.z)
                    }
                }
            } else {
                rotated
            };

            // Translate back
            result[i] = [
                scaled.x + origin[0],
                scaled.y + origin[1],
                scaled.z + origin[2],
            ];
        }
        result
    }

    /// Apply block-level transform to positions.
    fn apply_block_transform(
        &self,
        positions: &[[f32; 3]; 4],
        transform: &BlockTransform,
    ) -> [[f32; 3]; 4] {
        if transform.is_identity() {
            return *positions;
        }

        // Negate angles: Minecraft rotations are clockwise from above (Y) and
        // from +X looking toward origin (X), but glam uses right-hand rule (CCW).
        let x_rot = Mat3::from_rotation_x((-transform.x as f32).to_radians());
        let y_rot = Mat3::from_rotation_y((-transform.y as f32).to_radians());
        let rotation_matrix = y_rot * x_rot;

        let mut result = [[0.0; 3]; 4];
        for (i, pos) in positions.iter().enumerate() {
            let p = Vec3::new(pos[0], pos[1], pos[2]);
            let rotated = rotation_matrix * p;
            result[i] = [rotated.x, rotated.y, rotated.z];
        }
        result
    }

    /// Rotate a normal by the block transform.
    fn rotate_normal(&self, normal: [f32; 3], transform: &BlockTransform) -> [f32; 3] {
        if transform.is_identity() {
            return normal;
        }

        let x_rot = Mat3::from_rotation_x((-transform.x as f32).to_radians());
        let y_rot = Mat3::from_rotation_y((-transform.y as f32).to_radians());
        let rotation_matrix = y_rot * x_rot;

        let n = Vec3::new(normal[0], normal[1], normal[2]);
        let rotated = rotation_matrix * n;
        [rotated.x, rotated.y, rotated.z]
    }

    /// Emit merged quads from the greedy mesher into the mesh.
    /// Greedy faces use tiled UVs [0, width] x [0, height] and bypass the atlas.
    /// AO is baked into the tile texture (not vertex colors) for universal viewer support.
    fn emit_greedy_quads(&mut self) {
        let greedy = match self.greedy.take() {
            Some(g) => g,
            None => return,
        };

        let merged_quads = greedy.merge();

        for quad in &merged_quads {
            let positions = quad.world_positions();
            let normal = quad.direction.normal();

            // Convert quantized tint back to f32 color (no AO — it's baked into the texture)
            let base_color = [
                quad.tint[0] as f32 / 255.0,
                quad.tint[1] as f32 / 255.0,
                quad.tint[2] as f32 / 255.0,
                quad.tint[3] as f32 / 255.0,
            ];

            // Tiled UVs: [0, width] x [0, height] so texture repeats per block
            let w = quad.width as f32;
            let h = quad.height as f32;
            let uvs = [[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]];

            // Track greedy face separately (bypasses atlas UV remapping)
            let vertex_start = self.mesh.vertex_count() as u32;
            let index_start = self.mesh.indices.len();
            self.greedy_face_textures.push(GreedyFaceMapping {
                vertex_start,
                index_start,
                texture_path: quad.texture.clone(),
                is_transparent: quad.is_transparent,
                ao: quad.ao,
            });

            let v0 = self.mesh.add_vertex(
                Vertex::new(positions[0], normal, uvs[0]).with_color(base_color),
            );
            let v1 = self.mesh.add_vertex(
                Vertex::new(positions[1], normal, uvs[1]).with_color(base_color),
            );
            let v2 = self.mesh.add_vertex(
                Vertex::new(positions[2], normal, uvs[2]).with_color(base_color),
            );
            let v3 = self.mesh.add_vertex(
                Vertex::new(positions[3], normal, uvs[3]).with_color(base_color),
            );

            // Use AO-aware triangulation even though colors are uniform,
            // to keep consistent winding with the AO baked into the texture
            if self.config.ambient_occlusion && quad.ao != [3, 3, 3, 3] {
                self.mesh.add_quad_ao(v0, v1, v2, v3, quad.ao);
            } else {
                self.mesh.add_quad(v0, v1, v2, v3);
            }
        }
    }

    /// Build the final meshes (opaque, cutout, and transparent) and atlas.
    /// Returns (opaque_mesh, cutout_mesh, transparent_mesh, atlas, greedy_materials, animated_exports).
    /// Render order: opaque first, then cutout (alpha-tested), then transparent (alpha-blended).
    /// Greedy materials have their own textures with REPEAT wrapping for tiling.
    /// Animated exports contain sprite sheets from dynamic textures (particles, etc.).
    pub fn build(mut self) -> Result<(Mesh, Mesh, Mesh, TextureAtlas, Vec<GreedyMaterial>, Vec<super::AnimatedTextureExport>)> {
        // Emit greedy-merged quads into the mesh before atlas building
        self.emit_greedy_quads();

        // Build texture atlas (only for non-greedy faces)
        let mut atlas_builder = AtlasBuilder::new(
            self.config.atlas_max_size,
            self.config.atlas_padding,
        );

        for texture_ref in &self.texture_refs {
            // Check dynamic textures first, then resource pack
            if let Some(texture) = self.dynamic_textures.get(texture_ref) {
                atlas_builder.add_texture(texture_ref.clone(), texture.first_frame());
            } else if let Some(texture) = self.resource_pack.get_texture(texture_ref) {
                atlas_builder.add_texture(texture_ref.clone(), texture.first_frame());
            }
        }

        let atlas = atlas_builder.build()?;

        // Remap UVs to atlas coordinates (only non-greedy faces)
        for face_mapping in &self.face_textures {
            if let Some(region) = atlas.get_region(&face_mapping.texture_path) {
                // Each face has 4 vertices
                for i in 0..4 {
                    let vertex_idx = face_mapping.vertex_start as usize + i;
                    if vertex_idx < self.mesh.vertices.len() {
                        let vertex = &mut self.mesh.vertices[vertex_idx];
                        // Transform UV from [0,1] local space to atlas region
                        vertex.uv = region.transform_uv(vertex.uv[0], vertex.uv[1]);
                    }
                }
            }
        }

        // Separate atlas-based faces into opaque, cutout, and transparent meshes
        let (opaque_mesh, cutout_mesh, transparent_mesh) = self.separate_by_transparency();

        // Build greedy materials: group greedy faces by texture path
        let greedy_materials = self.build_greedy_materials();

        // Collect animated texture exports from dynamic textures (particles, etc.)
        let mut animated_exports = Vec::new();
        for (key, tex) in &self.dynamic_textures {
            if tex.is_animated && tex.frame_count > 1 {
                if let Some(region) = atlas.get_region(key) {
                    let sprite_sheet_png = match tex.to_png() {
                        Ok(png) => png,
                        Err(_) => continue,
                    };
                    let anim = tex.animation.as_ref();
                    let frame_width = anim.and_then(|a| a.frame_width).unwrap_or(tex.width);
                    let frame_height = anim.and_then(|a| a.frame_height).unwrap_or(frame_width);
                    let frametime = anim.map(|a| a.frametime).unwrap_or(1);
                    let interpolate = anim.map(|a| a.interpolate).unwrap_or(false);
                    let frames = anim.and_then(|a| a.frames.as_ref()).map(|fs| {
                        fs.iter().map(|f| f.index).collect()
                    });
                    let atlas_x = (region.u_min * atlas.width as f32).round() as u32;
                    let atlas_y = (region.v_min * atlas.height as f32).round() as u32;
                    animated_exports.push(super::AnimatedTextureExport {
                        sprite_sheet_png,
                        frame_count: tex.frame_count,
                        frametime,
                        interpolate,
                        frames,
                        frame_width,
                        frame_height,
                        atlas_x,
                        atlas_y,
                    });
                }
            }
        }

        Ok((opaque_mesh, cutout_mesh, transparent_mesh, atlas, greedy_materials, animated_exports))
    }

    /// Build per-texture GreedyMaterial meshes from greedy faces.
    /// Groups by (texture_path, ao_pattern) so each AO variant gets its own
    /// baked texture with AO darkening applied at the pixel level.
    fn build_greedy_materials(&self) -> Vec<GreedyMaterial> {
        use std::collections::HashMap;

        // Group greedy faces by (texture_path, ao_pattern)
        // This ensures faces with different AO get different baked textures.
        type MaterialKey = (String, [u8; 4]);
        let mut material_map: HashMap<MaterialKey, (Mesh, Mesh)> = HashMap::new();

        for face_mapping in &self.greedy_face_textures {
            let vstart = face_mapping.vertex_start as usize;
            let istart = face_mapping.index_start;

            if vstart + 4 > self.mesh.vertices.len() || istart + 6 > self.mesh.indices.len() {
                continue;
            }

            let key = (face_mapping.texture_path.clone(), face_mapping.ao);
            let (opaque_mesh, transparent_mesh) = material_map
                .entry(key)
                .or_insert_with(|| (Mesh::new(), Mesh::new()));

            let target_mesh = if face_mapping.is_transparent {
                transparent_mesh
            } else {
                opaque_mesh
            };

            let orig_v0 = face_mapping.vertex_start;
            let v0 = target_mesh.add_vertex(self.mesh.vertices[vstart]);
            let v1 = target_mesh.add_vertex(self.mesh.vertices[vstart + 1]);
            let v2 = target_mesh.add_vertex(self.mesh.vertices[vstart + 2]);
            let v3 = target_mesh.add_vertex(self.mesh.vertices[vstart + 3]);

            // Directly read the 6 indices (2 triangles) from the tracked position
            for tri in 0..2 {
                let base = istart + tri * 3;
                let i0 = self.mesh.indices[base];
                let i1 = self.mesh.indices[base + 1];
                let i2 = self.mesh.indices[base + 2];
                let new_i0 = match i0 - orig_v0 { 0 => v0, 1 => v1, 2 => v2, _ => v3 };
                let new_i1 = match i1 - orig_v0 { 0 => v0, 1 => v1, 2 => v2, _ => v3 };
                let new_i2 = match i2 - orig_v0 { 0 => v0, 1 => v1, 2 => v2, _ => v3 };
                target_mesh.add_triangle(new_i0, new_i1, new_i2);
            }
        }

        let ao_intensity = self.config.ao_intensity;

        material_map
            .into_iter()
            .map(|((texture_path, ao), (opaque_mesh, transparent_mesh))| {
                let texture_png = if ao == [3, 3, 3, 3] {
                    // No AO darkening — use original tile texture
                    self.resource_pack
                        .get_texture(&texture_path)
                        .and_then(|t| t.first_frame().to_png().ok())
                        .unwrap_or_default()
                } else {
                    // Bake AO gradient into tile texture pixels
                    self.resource_pack
                        .get_texture(&texture_path)
                        .map(|t| {
                            let frame = t.first_frame();
                            bake_ao_into_tile(&frame.pixels, frame.width, frame.height, ao, ao_intensity)
                        })
                        .unwrap_or_default()
                };
                GreedyMaterial {
                    texture_path,
                    opaque_mesh,
                    transparent_mesh,
                    texture_png,
                }
            })
            .collect()
    }

    /// Separate the mesh into opaque, cutout, and transparent parts.
    /// - Opaque: no transparency at all
    /// - Cutout: binary alpha (texture has transparency but vertex alpha ≈ 1.0) — uses MASK mode
    /// - Transparent: semi-transparent (vertex alpha < 1.0, e.g. water) — uses BLEND mode
    fn separate_by_transparency(&self) -> (Mesh, Mesh, Mesh) {
        let mut opaque_mesh = Mesh::new();
        let mut cutout_mesh = Mesh::new();
        let mut transparent_mesh = Mesh::new();

        // Process each face using tracked index positions (O(n) instead of O(n²))
        for face_mapping in &self.face_textures {
            let vstart = face_mapping.vertex_start as usize;
            let istart = face_mapping.index_start;

            // Get the 4 vertices for this face
            if vstart + 4 > self.mesh.vertices.len() || istart + 6 > self.mesh.indices.len() {
                continue;
            }

            let target_mesh = if !face_mapping.is_transparent {
                &mut opaque_mesh
            } else {
                // Check vertex alpha to distinguish cutout from blend:
                // If any vertex has alpha < 0.99, it's semi-transparent (BLEND).
                // Otherwise it's binary alpha (CUTOUT/MASK).
                let has_blend_alpha = (0..4).any(|i| {
                    self.mesh.vertices[vstart + i].color[3] < 0.99
                });
                if has_blend_alpha {
                    &mut transparent_mesh
                } else {
                    &mut cutout_mesh
                }
            };

            let orig_v0 = face_mapping.vertex_start;
            let v0 = target_mesh.add_vertex(self.mesh.vertices[vstart]);
            let v1 = target_mesh.add_vertex(self.mesh.vertices[vstart + 1]);
            let v2 = target_mesh.add_vertex(self.mesh.vertices[vstart + 2]);
            let v3 = target_mesh.add_vertex(self.mesh.vertices[vstart + 3]);

            // Directly read the 6 indices (2 triangles) from the tracked position
            for tri in 0..2 {
                let base = istart + tri * 3;
                let i0 = self.mesh.indices[base];
                let i1 = self.mesh.indices[base + 1];
                let i2 = self.mesh.indices[base + 2];
                let new_i0 = match i0 - orig_v0 { 0 => v0, 1 => v1, 2 => v2, _ => v3 };
                let new_i1 = match i1 - orig_v0 { 0 => v0, 1 => v1, 2 => v2, _ => v3 };
                let new_i2 = match i2 - orig_v0 { 0 => v0, 1 => v1, 2 => v2, _ => v3 };
                target_mesh.add_triangle(new_i0, new_i1, new_i2);
            }
        }

        (opaque_mesh, cutout_mesh, transparent_mesh)
    }
}

/// Apply AO and lighting to a color.
/// ao_level: 0-3 (0=darkest, 3=brightest)
/// intensity: AO intensity (0.0-1.0)
/// light_factor: lighting brightness multiplier (0.0-1.0)
fn apply_ao_and_light(color: [f32; 4], ao_level: u8, intensity: f32, light_factor: f32) -> [f32; 4] {
    let ao_brightness = 1.0 - intensity * (1.0 - ao_level as f32 / 3.0);
    let combined = ao_brightness * light_factor;
    [
        color[0] * combined,
        color[1] * combined,
        color[2] * combined,
        color[3],
    ]
}

/// Bake ambient occlusion into a tile texture's pixels.
///
/// Creates a new PNG where each pixel is darkened according to a bilinear
/// interpolation of the 4 corner AO values. The tile repeats with REPEAT
/// wrapping, so each block in a merged quad independently shows the AO gradient.
///
/// AO corner mapping (image coordinates, row 0 = top):
///   (0,0)=AO[0]  (w,0)=AO[1]
///   (0,h)=AO[3]  (w,h)=AO[2]
///
/// In glTF, UV (0,0) = top-left of image. So emit_greedy_quads maps:
///   vertex 0 UV(0,0) → image top-left    → AO[0]
///   vertex 1 UV(w,0) → image top-right   → AO[1]
///   vertex 2 UV(w,h) → image bottom-right→ AO[2]
///   vertex 3 UV(0,h) → image bottom-left → AO[3]
fn bake_ao_into_tile(
    pixels: &[u8],
    width: u32,
    height: u32,
    ao: [u8; 4],
    intensity: f32,
) -> Vec<u8> {
    use image::{ImageBuffer, Rgba};

    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(width, height, pixels.to_vec())
            .expect("Failed to create image buffer for AO baking");

    let w = width.max(1) as f32;
    let h = height.max(1) as f32;

    // Precompute brightness for each AO level
    let brightness: [f32; 4] = std::array::from_fn(|i| {
        1.0 - intensity * (1.0 - ao[i] as f32 / 3.0)
    });

    for row in 0..height {
        for col in 0..width {
            // Normalized position in image space (row 0 = top)
            let cx = col as f32 / (w - 1.0).max(1.0); // 0=left, 1=right
            let cy = row as f32 / (h - 1.0).max(1.0); // 0=top, 1=bottom

            // Bilinear interpolation of corner brightness values
            // Image corners: top-left=AO[0], top-right=AO[1], bottom-left=AO[3], bottom-right=AO[2]
            let b = (1.0 - cx) * (1.0 - cy) * brightness[0]
                  + cx * (1.0 - cy) * brightness[1]
                  + (1.0 - cx) * cy * brightness[3]
                  + cx * cy * brightness[2];

            let pixel = img.get_pixel_mut(col, row);
            pixel[0] = (pixel[0] as f32 * b).round().min(255.0) as u8;
            pixel[1] = (pixel[1] as f32 * b).round().min(255.0) as u8;
            pixel[2] = (pixel[2] as f32 * b).round().min(255.0) as u8;
            // Alpha unchanged
        }
    }

    let mut bytes = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut bytes);
    img.write_to(&mut cursor, image::ImageFormat::Png)
        .expect("Failed to encode AO-baked tile as PNG");
    bytes
}

/// Standard Minecraft dye color RGB values.
fn dye_rgb(color: &str) -> [f32; 4] {
    match color {
        "white" => [1.0, 1.0, 1.0, 1.0],
        "orange" => [0.85, 0.52, 0.18, 1.0],
        "magenta" => [0.70, 0.33, 0.85, 1.0],
        "light_blue" => [0.40, 0.60, 0.85, 1.0],
        "yellow" => [0.96, 0.86, 0.26, 1.0],
        "lime" => [0.50, 0.80, 0.10, 1.0],
        "pink" => [0.95, 0.55, 0.65, 1.0],
        "gray" => [0.37, 0.42, 0.46, 1.0],
        "light_gray" => [0.60, 0.60, 0.55, 1.0],
        "cyan" => [0.10, 0.55, 0.60, 1.0],
        "purple" => [0.50, 0.25, 0.70, 1.0],
        "blue" => [0.20, 0.25, 0.70, 1.0],
        "brown" => [0.50, 0.32, 0.20, 1.0],
        "green" => [0.35, 0.45, 0.14, 1.0],
        "red" => [0.70, 0.20, 0.20, 1.0],
        "black" => [0.10, 0.10, 0.13, 1.0],
        _ => [1.0, 1.0, 1.0, 1.0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_rotate_uvs() {
        let pack = ResourcePack::new();
        let config = MesherConfig::default();
        let builder = MeshBuilder::new(&pack, &config, None, None, None);

        let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];

        // 0 degrees - no change
        assert_eq!(builder.rotate_uvs(uvs, 0), uvs);

        // 90 degrees
        let rotated_90 = builder.rotate_uvs(uvs, 90);
        assert_eq!(rotated_90[0], uvs[3]);
        assert_eq!(rotated_90[1], uvs[0]);

        // 180 degrees
        let rotated_180 = builder.rotate_uvs(uvs, 180);
        assert_eq!(rotated_180[0], uvs[2]);
        assert_eq!(rotated_180[2], uvs[0]);
    }

    fn full_cube_element() -> ModelElement {
        ModelElement {
            from: [0.0, 0.0, 0.0],
            to: [16.0, 16.0, 16.0],
            rotation: None,
            shade: true,
            faces: HashMap::new(),
        }
    }

    fn full_face() -> ModelFace {
        ModelFace {
            uv: None, // defaults to [0,0,16,16]
            texture: "#all".to_string(),
            cullface: Some(Direction::Up),
            rotation: 0,
            tintindex: -1,
        }
    }

    #[test]
    fn test_greedy_eligible_full_cube() {
        let pack = ResourcePack::new();
        let config = MesherConfig::default();
        let builder = MeshBuilder::new(&pack, &config, None, None, None);

        let element = full_cube_element();
        let face = full_face();
        let identity = BlockTransform::default();

        assert!(builder.is_greedy_eligible(&element, &face, &identity));
    }

    #[test]
    fn test_greedy_ineligible_partial_element() {
        let pack = ResourcePack::new();
        let config = MesherConfig::default();
        let builder = MeshBuilder::new(&pack, &config, None, None, None);

        // Slab-like element (half height)
        let element = ModelElement {
            from: [0.0, 0.0, 0.0],
            to: [16.0, 8.0, 16.0],
            rotation: None,
            shade: true,
            faces: HashMap::new(),
        };
        let face = full_face();
        let identity = BlockTransform::default();

        assert!(!builder.is_greedy_eligible(&element, &face, &identity));
    }

    #[test]
    fn test_greedy_ineligible_rotated_block() {
        let pack = ResourcePack::new();
        let config = MesherConfig::default();
        let builder = MeshBuilder::new(&pack, &config, None, None, None);

        let element = full_cube_element();
        let face = full_face();
        let rotated = BlockTransform::new(0, 90, false);

        assert!(!builder.is_greedy_eligible(&element, &face, &rotated));
    }

    #[test]
    fn test_greedy_ineligible_custom_uv() {
        let pack = ResourcePack::new();
        let config = MesherConfig::default();
        let builder = MeshBuilder::new(&pack, &config, None, None, None);

        let element = full_cube_element();
        let face = ModelFace {
            uv: Some([0.0, 0.0, 8.0, 8.0]), // Half-size UV
            texture: "#all".to_string(),
            cullface: None,
            rotation: 0,
            tintindex: -1,
        };
        let identity = BlockTransform::default();

        assert!(!builder.is_greedy_eligible(&element, &face, &identity));
    }

    #[test]
    fn test_greedy_ineligible_rotated_uv() {
        let pack = ResourcePack::new();
        let config = MesherConfig::default();
        let builder = MeshBuilder::new(&pack, &config, None, None, None);

        let element = full_cube_element();
        let face = ModelFace {
            uv: None,
            texture: "#all".to_string(),
            cullface: None,
            rotation: 90, // Rotated UV
            tintindex: -1,
        };
        let identity = BlockTransform::default();

        assert!(!builder.is_greedy_eligible(&element, &face, &identity));
    }

    #[test]
    fn test_greedy_ineligible_element_rotation() {
        let pack = ResourcePack::new();
        let config = MesherConfig::default();
        let builder = MeshBuilder::new(&pack, &config, None, None, None);

        let element = ModelElement {
            from: [0.0, 0.0, 0.0],
            to: [16.0, 16.0, 16.0],
            rotation: Some(crate::types::ElementRotation {
                origin: [8.0, 8.0, 8.0],
                axis: crate::types::Axis::Y,
                angle: 22.5,
                rescale: false,
            }),
            shade: true,
            faces: HashMap::new(),
        };
        let face = full_face();
        let identity = BlockTransform::default();

        assert!(!builder.is_greedy_eligible(&element, &face, &identity));
    }
}
