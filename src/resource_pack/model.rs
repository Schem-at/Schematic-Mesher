//! Block model parsing.
//!
//! Block models define the 3D geometry of blocks using cuboid elements.

use crate::types::{Direction, ElementRotation};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A parsed block model from models/*.json.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BlockModel {
    /// Parent model to inherit from.
    #[serde(default)]
    pub parent: Option<String>,

    /// Whether to use ambient occlusion.
    #[serde(default = "default_ao", rename = "ambientocclusion")]
    pub ambient_occlusion: bool,

    /// Texture variable definitions.
    #[serde(default)]
    pub textures: HashMap<String, String>,

    /// Model elements (cuboids).
    #[serde(default)]
    pub elements: Vec<ModelElement>,

    /// Display transforms (for item rendering, not used for block meshing).
    #[serde(default)]
    pub display: Option<serde_json::Value>,
}

fn default_ao() -> bool {
    true
}

impl BlockModel {
    /// Create an empty model.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the full parent resource location.
    pub fn parent_location(&self) -> Option<String> {
        self.parent.as_ref().map(|p| {
            if p.contains(':') {
                p.clone()
            } else {
                format!("minecraft:{}", p)
            }
        })
    }

    /// Check if this model has its own elements (not inherited).
    pub fn has_elements(&self) -> bool {
        !self.elements.is_empty()
    }

    /// Resolve a texture reference (e.g., "#side") to a texture path.
    /// Returns None if the reference cannot be resolved.
    pub fn resolve_texture<'a>(&'a self, reference: &'a str) -> Option<&'a str> {
        if !reference.starts_with('#') {
            // Already a direct path
            return Some(reference);
        }

        let key = &reference[1..]; // Remove '#'
        self.textures.get(key).map(|s| s.as_str())
    }
}

/// A cuboid element within a model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelElement {
    /// Minimum corner (0-16 range).
    pub from: [f32; 3],
    /// Maximum corner (0-16 range).
    pub to: [f32; 3],
    /// Optional rotation.
    #[serde(default)]
    pub rotation: Option<ElementRotation>,
    /// Whether this element receives shade.
    #[serde(default = "default_shade")]
    pub shade: bool,
    /// Face definitions.
    #[serde(default)]
    pub faces: HashMap<Direction, ModelFace>,
}

fn default_shade() -> bool {
    true
}

impl ModelElement {
    /// Get the size of this element in Minecraft coordinates (0-16).
    pub fn size(&self) -> [f32; 3] {
        [
            self.to[0] - self.from[0],
            self.to[1] - self.from[1],
            self.to[2] - self.from[2],
        ]
    }

    /// Get the center of this element in Minecraft coordinates.
    pub fn center(&self) -> [f32; 3] {
        [
            (self.from[0] + self.to[0]) / 2.0,
            (self.from[1] + self.to[1]) / 2.0,
            (self.from[2] + self.to[2]) / 2.0,
        ]
    }

    /// Convert from Minecraft coordinates (0-16) to normalized (-0.5 to 0.5).
    pub fn normalized_from(&self) -> [f32; 3] {
        [
            self.from[0] / 16.0 - 0.5,
            self.from[1] / 16.0 - 0.5,
            self.from[2] / 16.0 - 0.5,
        ]
    }

    /// Convert to Minecraft coordinates (0-16) to normalized (-0.5 to 0.5).
    pub fn normalized_to(&self) -> [f32; 3] {
        [
            self.to[0] / 16.0 - 0.5,
            self.to[1] / 16.0 - 0.5,
            self.to[2] / 16.0 - 0.5,
        ]
    }

    /// Get the normalized center.
    pub fn normalized_center(&self) -> [f32; 3] {
        let c = self.center();
        [c[0] / 16.0 - 0.5, c[1] / 16.0 - 0.5, c[2] / 16.0 - 0.5]
    }

    /// Get the normalized size.
    pub fn normalized_size(&self) -> [f32; 3] {
        let s = self.size();
        [s[0] / 16.0, s[1] / 16.0, s[2] / 16.0]
    }

    /// Check if this element is very thin (could need double-sided rendering).
    pub fn is_thin(&self, threshold: f32) -> bool {
        let size = self.normalized_size();
        size[0] < threshold || size[1] < threshold || size[2] < threshold
    }
}

/// A face of a model element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelFace {
    /// UV coordinates [u1, v1, u2, v2] in 0-16 range.
    #[serde(default)]
    pub uv: Option<[f32; 4]>,
    /// Texture reference (e.g., "#side" or "block/stone").
    pub texture: String,
    /// Face direction for culling (if adjacent block is opaque, hide this face).
    #[serde(default)]
    pub cullface: Option<Direction>,
    /// UV rotation in degrees (0, 90, 180, 270).
    #[serde(default)]
    pub rotation: i32,
    /// Tint index for biome coloring (-1 = no tint).
    #[serde(default = "default_tint_index")]
    pub tintindex: i32,
}

fn default_tint_index() -> i32 {
    -1
}

impl ModelFace {
    /// Get the UV coordinates, defaulting to full texture if not specified.
    pub fn uv_or_default(&self) -> [f32; 4] {
        self.uv.unwrap_or([0.0, 0.0, 16.0, 16.0])
    }

    /// Get normalized UV coordinates (0-1 range).
    pub fn normalized_uv(&self) -> [f32; 4] {
        let uv = self.uv_or_default();
        [uv[0] / 16.0, uv[1] / 16.0, uv[2] / 16.0, uv[3] / 16.0]
    }

    /// Check if this face has a tint.
    pub fn has_tint(&self) -> bool {
        self.tintindex >= 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_model() {
        let json = r#"{
            "parent": "block/cube_all",
            "textures": {
                "all": "block/stone"
            }
        }"#;

        let model: BlockModel = serde_json::from_str(json).unwrap();
        assert_eq!(model.parent, Some("block/cube_all".to_string()));
        assert_eq!(model.textures.get("all"), Some(&"block/stone".to_string()));
        assert!(model.elements.is_empty());
    }

    #[test]
    fn test_parse_model_with_elements() {
        let json = r##"{
            "textures": {
                "texture": "block/stone"
            },
            "elements": [
                {
                    "from": [0, 0, 0],
                    "to": [16, 16, 16],
                    "faces": {
                        "down":  { "texture": "#texture", "cullface": "down" },
                        "up":    { "texture": "#texture", "cullface": "up" },
                        "north": { "texture": "#texture", "cullface": "north" },
                        "south": { "texture": "#texture", "cullface": "south" },
                        "west":  { "texture": "#texture", "cullface": "west" },
                        "east":  { "texture": "#texture", "cullface": "east" }
                    }
                }
            ]
        }"##;

        let model: BlockModel = serde_json::from_str(json).unwrap();
        assert_eq!(model.elements.len(), 1);

        let element = &model.elements[0];
        assert_eq!(element.from, [0.0, 0.0, 0.0]);
        assert_eq!(element.to, [16.0, 16.0, 16.0]);
        assert_eq!(element.faces.len(), 6);
        assert_eq!(
            element.faces.get(&Direction::Down).unwrap().cullface,
            Some(Direction::Down)
        );
    }

    #[test]
    fn test_parse_element_with_rotation() {
        let json = r#"{
            "from": [0, 0, 0],
            "to": [16, 16, 16],
            "rotation": {
                "origin": [8, 8, 8],
                "axis": "y",
                "angle": 45,
                "rescale": true
            },
            "faces": {}
        }"#;

        let element: ModelElement = serde_json::from_str(json).unwrap();
        let rotation = element.rotation.unwrap();
        assert_eq!(rotation.origin, [8.0, 8.0, 8.0]);
        assert_eq!(rotation.angle, 45.0);
        assert!(rotation.rescale);
    }

    #[test]
    fn test_element_normalized_coords() {
        let element = ModelElement {
            from: [0.0, 0.0, 0.0],
            to: [16.0, 16.0, 16.0],
            rotation: None,
            shade: true,
            faces: HashMap::new(),
        };

        assert_eq!(element.normalized_from(), [-0.5, -0.5, -0.5]);
        assert_eq!(element.normalized_to(), [0.5, 0.5, 0.5]);
        assert_eq!(element.normalized_center(), [0.0, 0.0, 0.0]);
        assert_eq!(element.normalized_size(), [1.0, 1.0, 1.0]);
    }

    #[test]
    fn test_face_uv_normalization() {
        let face = ModelFace {
            uv: Some([0.0, 0.0, 8.0, 8.0]),
            texture: "#test".to_string(),
            cullface: None,
            rotation: 0,
            tintindex: -1,
        };

        assert_eq!(face.normalized_uv(), [0.0, 0.0, 0.5, 0.5]);
    }

    #[test]
    fn test_resolve_texture() {
        let model = BlockModel {
            textures: [
                ("all".to_string(), "block/stone".to_string()),
                ("side".to_string(), "#all".to_string()),
            ]
            .into_iter()
            .collect(),
            ..Default::default()
        };

        assert_eq!(model.resolve_texture("#all"), Some("block/stone"));
        assert_eq!(model.resolve_texture("#side"), Some("#all")); // Only one level
        assert_eq!(model.resolve_texture("block/dirt"), Some("block/dirt"));
        assert_eq!(model.resolve_texture("#missing"), None);
    }
}
