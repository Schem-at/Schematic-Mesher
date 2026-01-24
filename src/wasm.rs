//! WASM bindings for schematic-mesher.
//!
//! This module provides JavaScript-friendly APIs for use in the browser.

use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn init() {
    // Set up better panic messages in the browser console
    console_error_panic_hook::set_once();
}

/// Load a resource pack from bytes and return a handle.
#[wasm_bindgen]
pub struct ResourcePackHandle {
    inner: crate::ResourcePack,
}

#[wasm_bindgen]
impl ResourcePackHandle {
    /// Load a resource pack from a ZIP file's bytes.
    #[wasm_bindgen(constructor)]
    pub fn new(data: &[u8]) -> Result<ResourcePackHandle, JsError> {
        let pack = crate::load_resource_pack_from_bytes(data)
            .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(ResourcePackHandle { inner: pack })
    }

    /// Get the number of blockstates in the pack.
    #[wasm_bindgen(getter)]
    pub fn blockstate_count(&self) -> usize {
        self.inner.blockstate_count()
    }

    /// Get the number of models in the pack.
    #[wasm_bindgen(getter)]
    pub fn model_count(&self) -> usize {
        self.inner.model_count()
    }

    /// Get the number of textures in the pack.
    #[wasm_bindgen(getter)]
    pub fn texture_count(&self) -> usize {
        self.inner.texture_count()
    }
}

/// Mesher configuration options.
#[wasm_bindgen]
#[derive(Default)]
pub struct MesherOptions {
    cull_hidden_faces: bool,
    ambient_occlusion: bool,
    ao_intensity: f32,
    atlas_max_size: u32,
    biome: Option<String>,
}

#[wasm_bindgen]
impl MesherOptions {
    #[wasm_bindgen(constructor)]
    pub fn new() -> MesherOptions {
        MesherOptions {
            cull_hidden_faces: true,
            ambient_occlusion: true,
            ao_intensity: 0.4,
            atlas_max_size: 4096,
            biome: None,
        }
    }

    #[wasm_bindgen(setter)]
    pub fn set_cull_hidden_faces(&mut self, value: bool) {
        self.cull_hidden_faces = value;
    }

    #[wasm_bindgen(setter)]
    pub fn set_ambient_occlusion(&mut self, value: bool) {
        self.ambient_occlusion = value;
    }

    #[wasm_bindgen(setter)]
    pub fn set_ao_intensity(&mut self, value: f32) {
        self.ao_intensity = value;
    }

    #[wasm_bindgen(setter)]
    pub fn set_atlas_max_size(&mut self, value: u32) {
        self.atlas_max_size = value;
    }

    #[wasm_bindgen(setter)]
    pub fn set_biome(&mut self, value: String) {
        self.biome = Some(value);
    }
}

/// A block to be meshed.
#[wasm_bindgen]
pub struct BlockInput {
    x: i32,
    y: i32,
    z: i32,
    name: String,
    properties: std::collections::HashMap<String, String>,
}

#[wasm_bindgen]
impl BlockInput {
    #[wasm_bindgen(constructor)]
    pub fn new(x: i32, y: i32, z: i32, name: &str) -> BlockInput {
        BlockInput {
            x,
            y,
            z,
            name: name.to_string(),
            properties: std::collections::HashMap::new(),
        }
    }

    /// Add a property to this block.
    pub fn set_property(&mut self, key: &str, value: &str) {
        self.properties.insert(key.to_string(), value.to_string());
    }
}

/// Mesh result containing GLB data.
#[wasm_bindgen]
pub struct MeshResult {
    glb_data: Vec<u8>,
    vertex_count: usize,
    triangle_count: usize,
    has_transparency: bool,
}

#[wasm_bindgen]
impl MeshResult {
    /// Get the GLB binary data.
    #[wasm_bindgen(getter)]
    pub fn glb_data(&self) -> Vec<u8> {
        self.glb_data.clone()
    }

    /// Get the total vertex count.
    #[wasm_bindgen(getter)]
    pub fn vertex_count(&self) -> usize {
        self.vertex_count
    }

    /// Get the total triangle count.
    #[wasm_bindgen(getter)]
    pub fn triangle_count(&self) -> usize {
        self.triangle_count
    }

    /// Check if the mesh has transparent geometry.
    #[wasm_bindgen(getter)]
    pub fn has_transparency(&self) -> bool {
        self.has_transparency
    }
}

/// Mesh a single block and return GLB data.
#[wasm_bindgen]
pub fn mesh_block(
    pack: &ResourcePackHandle,
    block_name: &str,
    options: Option<MesherOptions>,
) -> Result<MeshResult, JsError> {
    let options = options.unwrap_or_default();

    let mut config = crate::MesherConfig::default();
    config.cull_hidden_faces = options.cull_hidden_faces;
    config.ambient_occlusion = options.ambient_occlusion;
    config.ao_intensity = options.ao_intensity;
    config.atlas_max_size = options.atlas_max_size;

    if let Some(biome) = &options.biome {
        config = config.with_biome(biome);
    }

    // Normalize block name
    let block_name = if block_name.contains(':') {
        block_name.to_string()
    } else {
        format!("minecraft:{}", block_name)
    };

    let block = crate::InputBlock::new(&block_name);
    let pos = crate::BlockPosition::new(0, 0, 0);

    // Create a simple block source
    let blocks = vec![(pos, block)];
    let bounds = crate::BoundingBox::new([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);

    let mesher = crate::Mesher::with_config(pack.inner.clone(), config);
    let output = mesher.mesh_blocks(
        blocks.iter().map(|(p, b)| (*p, b)),
        bounds,
    ).map_err(|e| JsError::new(&e.to_string()))?;

    let glb_data = crate::export_glb(&output)
        .map_err(|e| JsError::new(&e.to_string()))?;

    Ok(MeshResult {
        glb_data,
        vertex_count: output.total_vertices(),
        triangle_count: output.total_triangles(),
        has_transparency: output.has_transparency(),
    })
}

/// Mesh multiple blocks from a JSON string and return GLB data.
///
/// JSON format:
/// ```json
/// {
///   "blocks": [
///     { "x": 0, "y": 0, "z": 0, "name": "minecraft:stone", "properties": {} }
///   ]
/// }
/// ```
#[wasm_bindgen]
pub fn mesh_blocks_json(
    pack: &ResourcePackHandle,
    json: &str,
    options: Option<MesherOptions>,
) -> Result<MeshResult, JsError> {
    #[derive(serde::Deserialize)]
    struct BlockData {
        blocks: Vec<BlockEntry>,
    }

    #[derive(serde::Deserialize)]
    struct BlockEntry {
        x: i32,
        y: i32,
        z: i32,
        #[serde(default = "default_name")]
        name: String,
        #[serde(default)]
        properties: std::collections::HashMap<String, String>,
    }

    fn default_name() -> String {
        "minecraft:stone".to_string()
    }

    let data: BlockData = serde_json::from_str(json)
        .map_err(|e| JsError::new(&format!("Invalid JSON: {}", e)))?;

    let options = options.unwrap_or_default();

    let mut config = crate::MesherConfig::default();
    config.cull_hidden_faces = options.cull_hidden_faces;
    config.ambient_occlusion = options.ambient_occlusion;
    config.ao_intensity = options.ao_intensity;
    config.atlas_max_size = options.atlas_max_size;

    if let Some(biome) = &options.biome {
        config = config.with_biome(biome);
    }

    // Convert to InputBlocks
    let mut blocks = Vec::new();
    let mut min = [i32::MAX; 3];
    let mut max = [i32::MIN; 3];

    for entry in &data.blocks {
        let pos = crate::BlockPosition::new(entry.x, entry.y, entry.z);

        let name = if entry.name.contains(':') {
            entry.name.clone()
        } else {
            format!("minecraft:{}", entry.name)
        };

        let mut block = crate::InputBlock::new(name);
        for (k, v) in &entry.properties {
            block.properties.insert(k.clone(), v.clone());
        }

        blocks.push((pos, block));

        min[0] = min[0].min(entry.x);
        min[1] = min[1].min(entry.y);
        min[2] = min[2].min(entry.z);
        max[0] = max[0].max(entry.x);
        max[1] = max[1].max(entry.y);
        max[2] = max[2].max(entry.z);
    }

    let bounds = if blocks.is_empty() {
        crate::BoundingBox::new([0.0, 0.0, 0.0], [0.0, 0.0, 0.0])
    } else {
        crate::BoundingBox::new(
            [min[0] as f32, min[1] as f32, min[2] as f32],
            [(max[0] + 1) as f32, (max[1] + 1) as f32, (max[2] + 1) as f32],
        )
    };

    let mesher = crate::Mesher::with_config(pack.inner.clone(), config);
    let output = mesher.mesh_blocks(
        blocks.iter().map(|(p, b)| (*p, b)),
        bounds,
    ).map_err(|e| JsError::new(&e.to_string()))?;

    let glb_data = crate::export_glb(&output)
        .map_err(|e| JsError::new(&e.to_string()))?;

    Ok(MeshResult {
        glb_data,
        vertex_count: output.total_vertices(),
        triangle_count: output.total_triangles(),
        has_transparency: output.has_transparency(),
    })
}
