//! Transform types for block and element rotations.

use super::Axis;
use serde::{Deserialize, Serialize};

/// Block-level transform from blockstate variant.
#[derive(Debug, Clone, Copy, Default)]
pub struct BlockTransform {
    /// X rotation in degrees (0, 90, 180, 270).
    pub x: i32,
    /// Y rotation in degrees (0, 90, 180, 270).
    pub y: i32,
    /// If true, UV coordinates don't rotate with the block.
    pub uvlock: bool,
}

impl BlockTransform {
    pub fn new(x: i32, y: i32, uvlock: bool) -> Self {
        Self { x, y, uvlock }
    }

    /// Check if this is an identity transform (no rotation).
    pub fn is_identity(&self) -> bool {
        self.x == 0 && self.y == 0
    }
}

/// Element-level rotation from model element.
///
/// Two JSON forms exist, mirroring vanilla's `CuboidModelElement` deserializer:
///
/// - single axis: `{"origin": [..], "axis": "y", "angle": 22.5}`
/// - Euler XYZ (Minecraft 1.21.11+): `{"origin": [..], "x": 180, "y": -67.5, "z": 0}`,
///   each angle defaulting to 0. Vanilla builds `Matrix4f.rotationZYX(z, y, x)`,
///   so a vertex is rotated about X first, then Y, then Z.
///
/// When both are present, `axis`/`angle` wins, as in vanilla.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "RawElementRotation", into = "RawElementRotation")]
pub struct ElementRotation {
    /// Origin point for rotation (in 0-16 Minecraft coordinates).
    pub origin: [f32; 3],
    /// Axis to rotate around (single-axis form; ignored when `euler` is set).
    pub axis: Axis,
    /// Rotation angle in degrees (single-axis form; ignored when `euler` is set).
    pub angle: f32,
    /// Whether to rescale the element after rotation.
    pub rescale: bool,
    /// Euler angles in degrees `[x, y, z]`, applied X then Y then Z
    /// (the 1.21.11 form). `None` for the single-axis form.
    pub euler: Option<[f32; 3]>,
}

/// Wire form accepting both rotation spellings.
#[derive(Serialize, Deserialize)]
struct RawElementRotation {
    #[serde(default = "default_origin")]
    origin: [f32; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    axis: Option<Axis>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    angle: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    x: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    y: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    z: Option<f32>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    rescale: bool,
}

impl TryFrom<RawElementRotation> for ElementRotation {
    type Error = String;

    fn try_from(raw: RawElementRotation) -> Result<Self, Self::Error> {
        match (raw.axis, raw.angle) {
            (Some(axis), Some(angle)) => Ok(Self {
                origin: raw.origin,
                axis,
                angle,
                rescale: raw.rescale,
                euler: None,
            }),
            _ if raw.x.is_some() || raw.y.is_some() || raw.z.is_some() => Ok(Self {
                origin: raw.origin,
                axis: Axis::Y,
                angle: 0.0,
                rescale: raw.rescale,
                euler: Some([
                    raw.x.unwrap_or(0.0),
                    raw.y.unwrap_or(0.0),
                    raw.z.unwrap_or(0.0),
                ]),
            }),
            _ => Err(
                "element rotation needs either `axis` and `angle`, or at least one of `x`, `y`, `z`"
                    .to_string(),
            ),
        }
    }
}

impl From<ElementRotation> for RawElementRotation {
    fn from(r: ElementRotation) -> Self {
        match r.euler {
            Some([x, y, z]) => Self {
                origin: r.origin,
                axis: None,
                angle: None,
                x: Some(x),
                y: Some(y),
                z: Some(z),
                rescale: r.rescale,
            },
            None => Self {
                origin: r.origin,
                axis: Some(r.axis),
                angle: Some(r.angle),
                x: None,
                y: None,
                z: None,
                rescale: r.rescale,
            },
        }
    }
}

fn default_origin() -> [f32; 3] {
    [8.0, 8.0, 8.0]
}

impl ElementRotation {
    /// Single-axis rotation (the pre-1.21.11 form).
    pub fn single_axis(origin: [f32; 3], axis: Axis, angle: f32, rescale: bool) -> Self {
        Self { origin, axis, angle, rescale, euler: None }
    }

    /// Euler XYZ rotation in degrees (the 1.21.11 form).
    pub fn euler_xyz(origin: [f32; 3], x: f32, y: f32, z: f32, rescale: bool) -> Self {
        Self { origin, axis: Axis::Y, angle: 0.0, rescale, euler: Some([x, y, z]) }
    }

    /// The pure rotation as a matrix (no rescale), matching vanilla:
    /// single axis → `Matrix4f.rotation(angle, axis)`, Euler → `rotationZYX(z, y, x)`.
    pub fn rotation_matrix(&self) -> glam::Mat3 {
        match self.euler {
            Some([x, y, z]) => {
                glam::Mat3::from_rotation_z(z.to_radians())
                    * glam::Mat3::from_rotation_y(y.to_radians())
                    * glam::Mat3::from_rotation_x(x.to_radians())
            }
            None => match self.axis {
                Axis::X => glam::Mat3::from_rotation_x(self.angle_radians()),
                Axis::Y => glam::Mat3::from_rotation_y(self.angle_radians()),
                Axis::Z => glam::Mat3::from_rotation_z(self.angle_radians()),
            },
        }
    }

    /// Per-axis rescale applied after rotation, matching vanilla
    /// `CuboidRotation.computeRescale`: for each axis, rotate its unit vector and
    /// divide by the largest absolute component. `[1, 1, 1]` when `rescale` is off.
    pub fn rescale_vector(&self) -> glam::Vec3 {
        if !self.rescale {
            return glam::Vec3::ONE;
        }
        let m = self.rotation_matrix();
        let factor = |axis: glam::Vec3| {
            let d = m * axis;
            let max = d.x.abs().max(d.y.abs()).max(d.z.abs());
            if max > 1e-6 { 1.0 / max } else { 1.0 }
        };
        glam::Vec3::new(factor(glam::Vec3::X), factor(glam::Vec3::Y), factor(glam::Vec3::Z))
    }

    /// Convert origin from Minecraft coordinates (0-16) to normalized (-0.5 to 0.5).
    pub fn normalized_origin(&self) -> [f32; 3] {
        [
            self.origin[0] / 16.0 - 0.5,
            self.origin[1] / 16.0 - 0.5,
            self.origin[2] / 16.0 - 0.5,
        ]
    }

    /// Get the angle in radians.
    pub fn angle_radians(&self) -> f32 {
        self.angle.to_radians()
    }

    /// Get the rescale factor for this rotation.
    /// When rescale is true, the element is scaled to maintain its original size.
    pub fn rescale_factor(&self) -> f32 {
        if self.rescale {
            1.0 / self.angle_radians().cos()
        } else {
            1.0
        }
    }
}
