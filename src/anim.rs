use crate::math::{Quaternion, Vec3};

pub struct Anim {
    name: Box<str>,
    nodes: Vec<NodeAnim>,
}

pub struct NodeAnim {
    node: usize,
    pub translation: Option<Vec<Keyframe<Vec3>>>,
    pub rotation: Option<Vec<Keyframe<Quaternion>>>,
    pub scale: Option<Vec<Keyframe<Vec3>>>,
}

pub struct Keyframe<T> {
    pub time: f32,
    pub value: T,
}
