#[derive(Copy, Clone, PartialEq, Eq, Default)]
pub struct Vec3<T = f32> {
    pub x: T,
    pub y: T,
    pub z: T,
}

impl<T> Vec3<T> {
    pub fn new(x: T, y: T, z: T) -> Self {
        Self { x, y, z }
    }
}

#[derive(Copy, Clone, PartialEq)]
pub struct Quaternion {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32
}

impl Quaternion {
    pub fn new(a: f32, b: f32, c: f32, d: f32) -> Self {
        Self { a, b, c, d }
    }
}
