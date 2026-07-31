pub struct Color {
    pub rgba: [u8; 4],
    pub roughness: f32,
    pub metallic: f32,
    pub ior: f32,
    pub transmission: f32,
    pub emissive: f32,
}

pub struct ColorIndex {
    palette_id: usize,
    index: u8,
}

impl Default for Color {
    fn default() -> Self {
        Self {
            rgba: [0, 0, 0, 255],
            roughness: 1.0,
            metallic: 0.0,
            ior: 1.5,
            transmission: 0.0,
            emissive: 0.0,
        }
    }
}

pub struct Palette {}
