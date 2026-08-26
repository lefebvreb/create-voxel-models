// <ai-owned/>

use png::{BitDepth, ColorType, Encoder};

/// Encodes `pixels` as a PNG. `pixels` must be exactly `width * height * color.bytes_per_pixel()`
/// bytes, which every caller in this crate guarantees by construction.
pub fn encode_png(width: u32, height: u32, pixels: &[u8], color: ColorType) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = Encoder::new(&mut bytes, width, height);
    encoder.set_color(color);
    encoder.set_depth(BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .expect("encoder was just constructed with valid dimensions/color/depth");
    writer
        .write_image_data(pixels)
        .expect("pixel buffer length matches the encoder-declared dimensions and color type");
    writer.finish().expect("writing to an in-memory Vec<u8> cannot fail");
    bytes
}
