//! Texture loading and handling.

/// Raw texture data loaded from PNG.
#[derive(Debug, Clone)]
pub struct TextureData {
    /// Texture width in pixels.
    pub width: u32,
    /// Texture height in pixels.
    pub height: u32,
    /// RGBA8 pixel data (4 bytes per pixel).
    pub pixels: Vec<u8>,
    /// Whether this texture has animation metadata.
    pub is_animated: bool,
    /// Animation frame count (1 if not animated).
    pub frame_count: u32,
}

impl TextureData {
    /// Create a new texture from RGBA data.
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Self {
        Self {
            width,
            height,
            pixels,
            is_animated: false,
            frame_count: 1,
        }
    }

    /// Create a placeholder texture (magenta/black checkerboard).
    pub fn placeholder() -> Self {
        let size = 16;
        let mut pixels = vec![0u8; (size * size * 4) as usize];

        for y in 0..size {
            for x in 0..size {
                let idx = ((y * size + x) * 4) as usize;
                let is_magenta = ((x / 2) + (y / 2)) % 2 == 0;

                if is_magenta {
                    pixels[idx] = 255; // R
                    pixels[idx + 1] = 0; // G
                    pixels[idx + 2] = 255; // B
                    pixels[idx + 3] = 255; // A
                } else {
                    pixels[idx] = 0; // R
                    pixels[idx + 1] = 0; // G
                    pixels[idx + 2] = 0; // B
                    pixels[idx + 3] = 255; // A
                }
            }
        }

        Self {
            width: size,
            height: size,
            pixels,
            is_animated: false,
            frame_count: 1,
        }
    }

    /// Check if this texture has transparency.
    pub fn has_transparency(&self) -> bool {
        self.pixels.chunks(4).any(|pixel| pixel[3] < 255)
    }

    /// Get a pixel at (x, y).
    pub fn get_pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let idx = ((y * self.width + x) * 4) as usize;
        [
            self.pixels[idx],
            self.pixels[idx + 1],
            self.pixels[idx + 2],
            self.pixels[idx + 3],
        ]
    }

    /// Get the first frame of an animated texture (or the whole texture if not animated).
    pub fn first_frame(&self) -> TextureData {
        if !self.is_animated || self.frame_count <= 1 {
            return self.clone();
        }

        let frame_height = self.height / self.frame_count;
        let frame_size = (self.width * frame_height * 4) as usize;

        Self {
            width: self.width,
            height: frame_height,
            pixels: self.pixels[..frame_size].to_vec(),
            is_animated: false,
            frame_count: 1,
        }
    }
}

/// Load a texture from PNG bytes.
pub fn load_texture_from_bytes(data: &[u8]) -> Result<TextureData, image::ImageError> {
    let img = image::load_from_memory(data)?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();

    // Check for animation (texture is taller than wide, height is multiple of width)
    let is_animated = height > width && height % width == 0;
    let frame_count = if is_animated { height / width } else { 1 };

    Ok(TextureData {
        width,
        height,
        pixels: rgba.into_raw(),
        is_animated,
        frame_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_placeholder_texture() {
        let tex = TextureData::placeholder();
        assert_eq!(tex.width, 16);
        assert_eq!(tex.height, 16);
        assert_eq!(tex.pixels.len(), 16 * 16 * 4);
        assert!(!tex.has_transparency());
    }

    #[test]
    fn test_get_pixel() {
        let tex = TextureData::new(2, 2, vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255]);

        assert_eq!(tex.get_pixel(0, 0), [255, 0, 0, 255]); // Red
        assert_eq!(tex.get_pixel(1, 0), [0, 255, 0, 255]); // Green
        assert_eq!(tex.get_pixel(0, 1), [0, 0, 255, 255]); // Blue
        assert_eq!(tex.get_pixel(1, 1), [255, 255, 255, 255]); // White
    }

    #[test]
    fn test_has_transparency() {
        let opaque = TextureData::new(1, 1, vec![255, 0, 0, 255]);
        assert!(!opaque.has_transparency());

        let transparent = TextureData::new(1, 1, vec![255, 0, 0, 128]);
        assert!(transparent.has_transparency());
    }
}
