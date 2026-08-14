//! Pure-data glTF 2.0 JSON schema (the "JSON chunk" of a .glb file). No pyo3 dependency: this
//! module only describes the wire format, so it stays independently testable like `meshing.rs`.
//!
//! Deliberately not modeled (YAGNI, unused by this exporter): `uri` fields (GLB always embeds
//! via bufferView), sparse accessors, morph targets, skins, cameras, lights, multi-scene support,
//! and non-default `texCoord` (always implicit `TEXCOORD_0`).

use serde::Serialize;

use crate::utils::Dict;

pub const COMPONENT_TYPE_UNSIGNED_INT: u32 = 5125;
pub const COMPONENT_TYPE_FLOAT: u32 = 5126;
pub const TARGET_ARRAY_BUFFER: u32 = 34962;
pub const TARGET_ELEMENT_ARRAY_BUFFER: u32 = 34963;
pub const FILTER_NEAREST: u32 = 9728;
pub const WRAP_CLAMP_TO_EDGE: u32 = 33071;

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Root {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extensions_used: Vec<String>,
    pub asset: Asset,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene: Option<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub scenes: Vec<Scene>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<Node>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub meshes: Vec<Mesh>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub materials: Vec<Material>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub textures: Vec<Texture>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<Image>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub samplers: Vec<Sampler>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub accessors: Vec<Accessor>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub buffer_views: Vec<BufferView>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub buffers: Vec<Buffer>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub animations: Vec<Animation>,
}

impl Default for Asset {
    fn default() -> Self {
        Self {
            version: "2.0".to_string(),
            generator: Some("voxels".to_string()),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generator: Option<String>,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Scene {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<u32>,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mesh: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translation: Option<[f32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<[f32; 4]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<[f32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extras: Option<Dict>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Mesh {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub primitives: Vec<Primitive>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extras: Option<Dict>,
}

/// `indices`/`material`/`normal`/`texcoord_0` are non-`Option`: `meshing::export_model` only ever
/// emits primitives with non-empty positions/normals/uvs/indices and a valid material index, so
/// there is no real "attribute might be absent" case here.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Primitive {
    pub attributes: Attributes,
    pub indices: u32,
    pub material: u32,
}

#[derive(Serialize, Clone)]
pub struct Attributes {
    #[serde(rename = "POSITION")]
    pub position: u32,
    #[serde(rename = "NORMAL")]
    pub normal: u32,
    #[serde(rename = "TEXCOORD_0")]
    pub texcoord_0: u32,
}

/// No `byteOffset` field: every accessor gets its own dedicated bufferView, so the accessor-level
/// offset is always the implicit spec default (0) and doesn't need modeling.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Accessor {
    pub buffer_view: u32,
    pub component_type: u32,
    pub count: u32,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<Vec<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<Vec<f32>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BufferView {
    pub buffer: u32,
    pub byte_offset: u32,
    pub byte_length: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<u32>,
}

/// No `uri`: this is buffer 0, the GLB file's own BIN chunk.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Buffer {
    pub byte_length: u32,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Material {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub pbr_metallic_roughness: PbrMetallicRoughness,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emissive_factor: Option<[f32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emissive_texture: Option<TextureInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<MaterialExtensions>,
}

/// `baseColorFactor`/`metallicFactor`/`roughnessFactor` are never written: their spec defaults
/// ([1,1,1,1] / 1.0 / 1.0) already mean "pass the texture through unmodified", which is exactly
/// what's wanted since the real values live in the baked atlas textures.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PbrMetallicRoughness {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_color_texture: Option<TextureInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metallic_roughness_texture: Option<TextureInfo>,
}

#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct TextureInfo {
    pub index: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Texture {
    pub sampler: u32,
    pub source: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sampler {
    pub mag_filter: u32,
    pub min_filter: u32,
    pub wrap_s: u32,
    pub wrap_t: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Image {
    pub mime_type: String,
    pub buffer_view: u32,
}

#[derive(Serialize, Default)]
pub struct MaterialExtensions {
    #[serde(rename = "KHR_materials_ior", skip_serializing_if = "Option::is_none")]
    pub ior: Option<KhrMaterialsIor>,
    #[serde(rename = "KHR_materials_transmission", skip_serializing_if = "Option::is_none")]
    pub transmission: Option<KhrMaterialsTransmission>,
    #[serde(rename = "KHR_materials_emissive_strength", skip_serializing_if = "Option::is_none")]
    pub emissive_strength: Option<KhrMaterialsEmissiveStrength>,
}

impl MaterialExtensions {
    pub fn is_empty(&self) -> bool {
        self.ior.is_none() && self.transmission.is_none() && self.emissive_strength.is_none()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KhrMaterialsIor {
    pub ior: f64,
}

/// `transmissionFactor` defaults to 0.0 in the spec (unlike the other pbr factors, which default
/// to 1.0), so it must always be written as 1.0 here since the real value lives in the texture.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KhrMaterialsTransmission {
    pub transmission_factor: f64,
    pub transmission_texture: TextureInfo,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KhrMaterialsEmissiveStrength {
    pub emissive_strength: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Animation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub channels: Vec<AnimationChannel>,
    pub samplers: Vec<AnimationSampler>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extras: Option<Dict>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimationChannel {
    pub sampler: u32,
    pub target: AnimationChannelTarget,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimationChannelTarget {
    pub node: u32,
    pub path: String,
}

/// `interpolation` omitted means the spec default, LINEAR.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimationSampler {
    pub input: u32,
    pub output: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interpolation: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_serializes_with_camel_case_keys() {
        let mut root = Root::default();
        root.buffer_views.push(BufferView {
            buffer: 0,
            byte_offset: 4,
            byte_length: 12,
            target: Some(TARGET_ARRAY_BUFFER),
        });
        root.accessors.push(Accessor {
            buffer_view: 0,
            component_type: COMPONENT_TYPE_FLOAT,
            count: 3,
            type_: "VEC3".to_string(),
            min: Some(vec![0.0, 0.0, 0.0]),
            max: Some(vec![1.0, 1.0, 1.0]),
        });
        let json = serde_json::to_string(&root).unwrap();
        assert!(json.contains("\"byteOffset\":4"));
        assert!(json.contains("\"byteLength\":12"));
        assert!(json.contains("\"bufferView\":0"));
        assert!(json.contains("\"componentType\":5126"));
        assert!(json.contains("\"type\":\"VEC3\""));
        assert!(!json.contains("\"scenes\""));
        assert!(!json.contains("\"materials\""));
    }

    #[test]
    fn material_extensions_use_khronos_names() {
        let extensions = MaterialExtensions {
            ior: Some(KhrMaterialsIor { ior: 1.33 }),
            transmission: Some(KhrMaterialsTransmission {
                transmission_factor: 1.0,
                transmission_texture: TextureInfo { index: 2 },
            }),
            emissive_strength: None,
        };
        let json = serde_json::to_string(&extensions).unwrap();
        assert!(json.contains("\"KHR_materials_ior\":{\"ior\":1.33}"));
        assert!(json.contains("\"KHR_materials_transmission\""));
        assert!(json.contains("\"transmissionFactor\":1.0"));
        assert!(!json.contains("KHR_materials_emissive_strength"));
    }

    #[test]
    fn primitive_attributes_use_gltf_semantic_names() {
        let primitive = Primitive {
            attributes: Attributes {
                position: 0,
                normal: 1,
                texcoord_0: 2,
            },
            indices: 3,
            material: 0,
        };
        let json = serde_json::to_string(&primitive).unwrap();
        assert!(json.contains("\"POSITION\":0"));
        assert!(json.contains("\"NORMAL\":1"));
        assert!(json.contains("\"TEXCOORD_0\":2"));
    }
}
