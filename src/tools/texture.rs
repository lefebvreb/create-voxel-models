// <ai-owned/>

//! Decodes a material's embedded PNG textures and samples them by UV. Nearest-neighbor only,
//! matching the `FILTER_NEAREST` sampler `glb.rs` always writes.

use png::Decoder;

use super::gltf;

#[derive(Debug)]
pub struct Texture {
    pub width: u32,
    pub height: u32,
    pub components: usize,
    pixels: Vec<u8>,
}

impl Texture {
    /// Nearest-neighbor sample by UV, clamped to the edge (matches `WRAP_CLAMP_TO_EDGE`).
    pub fn sample(&self, u: f32, v: f32) -> &[u8] {
        let x = ((u * self.width as f32) as i64).clamp(0, self.width as i64 - 1) as u32;
        let y = ((v * self.height as f32) as i64).clamp(0, self.height as i64 - 1) as u32;
        let start = (y as usize * self.width as usize + x as usize) * self.components;
        &self.pixels[start..start + self.components]
    }
}

pub fn decode_texture(root: &gltf::Root, bin: &[u8], texture_index: u32) -> Result<Texture, String> {
    let texture = root
        .textures
        .get(texture_index as usize)
        .ok_or_else(|| format!("texture index {texture_index} is out of range"))?;
    let image = root
        .images
        .get(texture.source as usize)
        .ok_or_else(|| format!("image index {} is out of range", texture.source))?;
    let view = root
        .buffer_views
        .get(image.buffer_view as usize)
        .ok_or_else(|| format!("bufferView index {} is out of range", image.buffer_view))?;
    let start = view.byte_offset as usize;
    let end = start + view.byte_length as usize;
    let bytes = bin.get(start..end).ok_or_else(|| "image bufferView is out of bounds".to_string())?;

    let mut reader = Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .map_err(|e| format!("invalid PNG image: {e}"))?;
    let buffer_size = reader
        .output_buffer_size()
        .ok_or_else(|| "PNG image is too large to decode".to_string())?;
    let mut buf = vec![0; buffer_size];
    let info = reader.next_frame(&mut buf).map_err(|e| format!("failed to decode PNG image: {e}"))?;
    let components = match info.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        other => return Err(format!("unsupported PNG color type {other:?}")),
    };
    buf.truncate(info.width as usize * info.height as usize * components);
    Ok(Texture {
        width: info.width,
        height: info.height,
        components,
        pixels: buf,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::utils::encode_png;

    #[test]
    fn decode_then_sample_round_trips_through_encode_png() {
        // A 2x1 RGB image: red pixel then blue pixel.
        let pixels = [255, 0, 0, 0, 0, 255];
        let png_bytes = encode_png(2, 1, &pixels, png::ColorType::Rgb);

        let mut root = gltf::Root::default();
        root.buffer_views.push(gltf::BufferView {
            buffer: 0,
            byte_offset: 0,
            byte_length: png_bytes.len() as u32,
            target: None,
        });
        root.images.push(gltf::Image {
            mime_type: "image/png".to_string(),
            buffer_view: 0,
        });
        root.textures.push(gltf::Texture { sampler: 0, source: 0 });

        let texture = decode_texture(&root, &png_bytes, 0).unwrap();
        assert_eq!((texture.width, texture.height, texture.components), (2, 1, 3));
        assert_eq!(texture.sample(0.1, 0.5), [255, 0, 0]);
        assert_eq!(texture.sample(0.9, 0.5), [0, 0, 255]);
    }

    #[test]
    fn rejects_invalid_png_data_without_panicking() {
        let mut root = gltf::Root::default();
        root.buffer_views.push(gltf::BufferView {
            buffer: 0,
            byte_offset: 0,
            byte_length: 4,
            target: None,
        });
        root.images.push(gltf::Image {
            mime_type: "image/png".to_string(),
            buffer_view: 0,
        });
        root.textures.push(gltf::Texture { sampler: 0, source: 0 });

        let err = decode_texture(&root, b"nope", 0).unwrap_err();
        assert!(err.contains("invalid PNG"));
    }
}
