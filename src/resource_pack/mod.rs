//! Resource pack loading and parsing.
//!
//! This module handles loading Minecraft resource packs (ZIP files or directories)
//! and parsing their contents including blockstates, models, and textures.

pub mod loader;
pub mod blockstate;
pub mod model;
pub mod texture;

pub use blockstate::{BlockstateDefinition, ModelVariant, MultipartCase, MultipartCondition};
pub use model::{BlockModel, ModelElement, ModelFace};
pub use texture::TextureData;

use std::collections::HashMap;

/// A loaded Minecraft resource pack.
#[derive(Debug, Default, Clone)]
pub struct ResourcePack {
    /// Blockstate definitions by namespace and block ID.
    /// Key: namespace (e.g., "minecraft"), Value: map of block_id to definition.
    pub blockstates: HashMap<String, HashMap<String, BlockstateDefinition>>,

    /// Model definitions by namespace and model path.
    /// Key: namespace, Value: map of model_path to model.
    pub models: HashMap<String, HashMap<String, BlockModel>>,

    /// Texture data by namespace and texture path.
    /// Key: namespace, Value: map of texture_path to data.
    pub textures: HashMap<String, HashMap<String, TextureData>>,
}

impl ResourcePack {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a blockstate definition by full resource location (e.g., "minecraft:stone").
    pub fn get_blockstate(&self, resource_location: &str) -> Option<&BlockstateDefinition> {
        let (namespace, path) = parse_resource_location(resource_location);
        self.blockstates
            .get(namespace)
            .and_then(|ns| ns.get(path))
    }

    /// Get a model by full resource location (e.g., "minecraft:block/stone").
    pub fn get_model(&self, resource_location: &str) -> Option<&BlockModel> {
        let (namespace, path) = parse_resource_location(resource_location);
        self.models.get(namespace).and_then(|ns| ns.get(path))
    }

    /// Get a texture by full resource location (e.g., "minecraft:block/stone").
    pub fn get_texture(&self, resource_location: &str) -> Option<&TextureData> {
        let (namespace, path) = parse_resource_location(resource_location);
        self.textures.get(namespace).and_then(|ns| ns.get(path))
    }

    /// Add a blockstate definition.
    pub fn add_blockstate(
        &mut self,
        namespace: &str,
        block_id: &str,
        definition: BlockstateDefinition,
    ) {
        self.blockstates
            .entry(namespace.to_string())
            .or_default()
            .insert(block_id.to_string(), definition);
    }

    /// Add a model.
    pub fn add_model(&mut self, namespace: &str, model_path: &str, model: BlockModel) {
        self.models
            .entry(namespace.to_string())
            .or_default()
            .insert(model_path.to_string(), model);
    }

    /// Add a texture.
    pub fn add_texture(&mut self, namespace: &str, texture_path: &str, texture: TextureData) {
        self.textures
            .entry(namespace.to_string())
            .or_default()
            .insert(texture_path.to_string(), texture);
    }

    /// Get the total number of blockstate definitions.
    pub fn blockstate_count(&self) -> usize {
        self.blockstates.values().map(|m| m.len()).sum()
    }

    /// Get the total number of models.
    pub fn model_count(&self) -> usize {
        self.models.values().map(|m| m.len()).sum()
    }

    /// Get the total number of textures.
    pub fn texture_count(&self) -> usize {
        self.textures.values().map(|m| m.len()).sum()
    }

    /// Get all namespaces in the resource pack.
    pub fn namespaces(&self) -> Vec<&str> {
        let mut namespaces: Vec<_> = self.blockstates.keys()
            .chain(self.models.keys())
            .chain(self.textures.keys())
            .map(|s| s.as_str())
            .collect();
        namespaces.sort();
        namespaces.dedup();
        namespaces
    }
}

/// Parse a resource location into namespace and path.
/// "minecraft:block/stone" -> ("minecraft", "block/stone")
/// "block/stone" -> ("minecraft", "block/stone")
fn parse_resource_location(resource_location: &str) -> (&str, &str) {
    if let Some((namespace, path)) = resource_location.split_once(':') {
        (namespace, path)
    } else {
        ("minecraft", resource_location)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_resource_location() {
        assert_eq!(
            parse_resource_location("minecraft:block/stone"),
            ("minecraft", "block/stone")
        );
        assert_eq!(
            parse_resource_location("mymod:block/custom"),
            ("mymod", "block/custom")
        );
        assert_eq!(
            parse_resource_location("block/stone"),
            ("minecraft", "block/stone")
        );
    }
}
