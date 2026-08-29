// <ai-owned/>

//! Pure-data glTF 2.0 JSON schema (the "JSON chunk" of a .glb file), shared by the writer
//! (`glb.rs`'s `export_glb`) and the reader (`glb.rs`'s `read_glb`) via `Serialize`/`Deserialize`
//! on the same structs. No pyo3 dependency: this module only describes the wire format, so it
//! stays independently testable like `meshing.rs`.
//!
//! Deliberately not modeled (YAGNI, unused by this exporter): `uri` fields (GLB always embeds
//! via bufferView), sparse accessors, morph targets, skins, cameras, lights, multi-scene support,
//! and non-default `texCoord` (always implicit `TEXCOORD_0`).
//!
//! Reading a third-party GLB inherits the same gaps, plus one specific to the reader: this schema
//! has no `baseColorFactor`/`metallicFactor`/`roughnessFactor` fields, since `export_glb` never
//! writes them (colors are always baked into the base-color texture, see `PbrMetallicRoughness`'s
//! doc comment) — a third-party material that uses factors instead of textures (common in
//! hand-authored glTF) will read as an untextured default rather than its authored flat color.
//! Fixing that is out of scope here; flagging it rather than silently under-rendering such assets.

use serde::{Deserialize, Serialize};

use crate::utils::Dict;

pub const COMPONENT_TYPE_UNSIGNED_INT: u32 = 5125;
pub const COMPONENT_TYPE_FLOAT: u32 = 5126;
pub const TARGET_ARRAY_BUFFER: u32 = 34962;
pub const TARGET_ELEMENT_ARRAY_BUFFER: u32 = 34963;
pub const FILTER_NEAREST: u32 = 9728;
pub const WRAP_CLAMP_TO_EDGE: u32 = 33071;

#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
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

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub generator: Option<String>,
}

#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Scene {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<u32>,
}

#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
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

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Mesh {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub name: Option<String>,
    pub primitives: Vec<Primitive>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub extras: Option<Dict>,
}

/// `indices`/`material`/`normal`/`texcoord_0` are non-`Option`: `meshing::export_model` only ever
/// emits primitives with non-empty positions/normals/uvs/indices and a valid material index, so
/// there is no real "attribute might be absent" case here on write. Reading a third-party GLB
/// with a non-indexed primitive or one that omits `material` isn't supported by this shape — it
/// would fail to deserialize rather than silently mis-render; not a case this exporter produces
/// or (so far) needs to read back.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Primitive {
    pub attributes: Attributes,
    pub indices: u32,
    pub material: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
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
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Accessor {
    pub buffer_view: u32,
    pub component_type: u32,
    pub count: u32,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub min: Option<Vec<f32>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub max: Option<Vec<f32>>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BufferView {
    pub buffer: u32,
    pub byte_offset: u32,
    pub byte_length: u32,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub target: Option<u32>,
}

/// No `uri`: this is buffer 0, the GLB file's own BIN chunk.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Buffer {
    pub byte_length: u32,
}

#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
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
/// what's wanted since the real values live in the baked atlas textures. They're not modeled for
/// reading either — see the module doc comment's note on third-party factor-only materials.
#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct PbrMetallicRoughness {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_color_texture: Option<TextureInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metallic_roughness_texture: Option<TextureInfo>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TextureInfo {
    pub index: u32,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Texture {
    pub sampler: u32,
    pub source: u32,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Sampler {
    pub mag_filter: u32,
    pub min_filter: u32,
    pub wrap_s: u32,
    pub wrap_t: u32,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Image {
    pub mime_type: String,
    pub buffer_view: u32,
}

#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(default)]
pub struct MaterialExtensions {
    #[serde(rename = "KHR_materials_ior", skip_serializing_if = "Option::is_none")]
    pub ior: Option<KhrMaterialsIor>,
    #[serde(rename = "KHR_materials_transmission", skip_serializing_if = "Option::is_none")]
    pub transmission: Option<KhrMaterialsTransmission>,
    #[serde(rename = "KHR_materials_emissive_strength", skip_serializing_if = "Option::is_none")]
    pub emissive_strength: Option<KhrMaterialsEmissiveStrength>,
    #[serde(rename = "KHR_materials_volume", skip_serializing_if = "Option::is_none")]
    pub volume: Option<KhrMaterialsVolume>,
}

impl MaterialExtensions {
    pub fn is_empty(&self) -> bool {
        self.ior.is_none() && self.transmission.is_none() && self.emissive_strength.is_none() && self.volume.is_none()
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct KhrMaterialsIor {
    pub ior: f64,
}

/// `transmissionFactor` defaults to 0.0 in the spec (unlike the other pbr factors, which default
/// to 1.0), so it must always be written as 1.0 here since the real value lives in the texture.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct KhrMaterialsTransmission {
    pub transmission_factor: f64,
    pub transmission_texture: TextureInfo,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct KhrMaterialsEmissiveStrength {
    pub emissive_strength: f64,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct KhrMaterialsVolume {
    pub thickness_factor: f64,
    pub attenuation_color: [f32; 3],
    pub attenuation_distance: f64,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Animation {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub name: Option<String>,
    pub channels: Vec<AnimationChannel>,
    pub samplers: Vec<AnimationSampler>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub extras: Option<Dict>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AnimationChannel {
    pub sampler: u32,
    pub target: AnimationChannelTarget,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AnimationChannelTarget {
    pub node: u32,
    pub path: String,
}

/// `interpolation` omitted means the spec default, LINEAR.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AnimationSampler {
    pub input: u32,
    pub output: u32,
    #[serde(skip_serializing_if = "Option::is_none", default)]
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
            volume: Some(KhrMaterialsVolume {
                thickness_factor: 1.0,
                attenuation_color: [0.0, 0.5, 1.0],
                attenuation_distance: 2.5,
            }),
        };
        let json = serde_json::to_string(&extensions).unwrap();
        assert!(json.contains("\"KHR_materials_ior\":{\"ior\":1.33}"));
        assert!(json.contains("\"KHR_materials_transmission\""));
        assert!(json.contains("\"transmissionFactor\":1.0"));
        assert!(!json.contains("KHR_materials_emissive_strength"));
        assert!(json.contains("\"KHR_materials_volume\""));
        assert!(json.contains("\"thicknessFactor\":1.0"));
        assert!(json.contains("\"attenuationColor\":[0.0,0.5,1.0]"));
        assert!(json.contains("\"attenuationDistance\":2.5"));
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

    #[test]
    fn root_round_trips_through_json() {
        let mut root = Root::default();
        root.asset = Asset::default();
        root.scene = Some(0);
        root.scenes.push(Scene { nodes: vec![0] });
        root.nodes.push(Node {
            name: Some("root".to_string()),
            translation: Some([1.0, 2.0, 3.0]),
            ..Default::default()
        });
        root.materials.push(Material {
            extensions: Some(MaterialExtensions {
                ior: Some(KhrMaterialsIor { ior: 1.33 }),
                ..Default::default()
            }),
            ..Default::default()
        });

        let json = serde_json::to_vec(&root).unwrap();
        let parsed: Root = serde_json::from_slice(&json).unwrap();

        assert_eq!(parsed.scene, Some(0));
        assert_eq!(parsed.scenes.len(), 1);
        assert_eq!(parsed.nodes[0].name.as_deref(), Some("root"));
        assert_eq!(parsed.nodes[0].translation, Some([1.0, 2.0, 3.0]));
        assert_eq!(parsed.nodes[0].children, Vec::<u32>::new());
        assert_eq!(
            parsed.materials[0]
                .extensions
                .as_ref()
                .unwrap()
                .ior
                .as_ref()
                .unwrap()
                .ior,
            1.33
        );
        assert!(parsed.materials[0].extensions.as_ref().unwrap().transmission.is_none());
    }

    #[test]
    fn missing_optional_fields_deserialize_to_their_write_side_defaults() {
        // What `export_glb` actually omits when empty (per `skip_serializing_if`) must still
        // parse: a minimal glTF with no top-level arrays at all.
        let json = br#"{"asset":{"version":"2.0"}}"#;
        let root: Root = serde_json::from_slice(json).unwrap();
        assert!(root.nodes.is_empty());
        assert!(root.materials.is_empty());
        assert_eq!(root.scene, None);
    }
}
