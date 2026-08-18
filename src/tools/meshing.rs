// <ai-owned/>

use std::collections::HashMap;

use pyo3::{Bound, PyResult};

use crate::model::{Dimensions, Model};
use crate::palette::{Color, Material, MaterialCode, Palette, Volume};

// Voxel grid axes map directly onto glTF's X/Y/Z with no remap: x -> X, y -> Y (up), z -> Z.

/// Plain snapshot of `Volume`, decoupled from pyo3 for the same reason as `MaterialProps` below.
#[derive(Clone, Copy)]
pub struct VolumeProps {
    pub color: Color,
    pub distance: f64,
    pub thickness: f64,
}

impl From<Volume> for VolumeProps {
    fn from(v: Volume) -> Self {
        Self {
            color: v.color,
            distance: v.distance,
            thickness: v.thickness,
        }
    }
}

/// Plain snapshot of the palette-relevant `Material` fields, decoupled from pyo3 so the meshing
/// and atlas-layout logic below is pure Rust and independently testable.
#[derive(Clone, Copy)]
struct MaterialProps {
    color: Color,
    roughness: f64,
    metallic: f64,
    ior: f64,
    transmission: f64,
    emissive: f64,
    volume: Option<VolumeProps>,
}

impl From<&Material> for MaterialProps {
    fn from(m: &Material) -> Self {
        Self {
            color: m.color,
            roughness: m.roughness,
            metallic: m.metallic,
            ior: m.ior,
            transmission: m.transmission,
            emissive: m.emissive,
            volume: m.volume.map(VolumeProps::from),
        }
    }
}

pub struct Primitive {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
    pub material_index: usize,
}

pub struct MeshData {
    pub primitives: Vec<Primitive>,
}

pub struct MaterialData {
    pub ior: f64,
    pub emissive: f64,
    pub volume: Option<VolumeProps>,
    pub atlas_width: u32,
    pub atlas_height: u32,
    pub base_color: Vec<[u8; 3]>,
    pub metallic_roughness: Vec<[u8; 2]>,
    pub transmission: Vec<u8>,
}

pub struct PaletteData {
    pub materials: Vec<MaterialData>,
}

pub fn export_model(model: Bound<Model>) -> PyResult<MeshData> {
    let model_ref = model.borrow();
    let palette_ref = model_ref.palette.bind(model.py()).borrow();
    let materials: Vec<MaterialProps> = palette_ref
        .materials
        .iter()
        .map(|m| MaterialProps::from(m.get()))
        .collect();
    let layout = build_palette_layout(&materials);
    let Dimensions { x, y, z } = model_ref.dimensions;
    Ok(build_mesh([x, y, z], &model_ref.data, &materials, &layout))
}

pub fn export_palette(palette: Bound<Palette>) -> PyResult<PaletteData> {
    let palette_ref = palette.borrow();
    let materials: Vec<MaterialProps> = palette_ref
        .materials
        .iter()
        .map(|m| MaterialProps::from(m.get()))
        .collect();
    let layout = build_palette_layout(&materials);
    Ok(build_palette_data(&materials, &layout))
}

// --- Palette layout: groups materials into buckets and lays out atlas texels ---
//
// `color`, `roughness`/`metallic` and `transmission` all have direct glTF texture equivalents
// (baseColor, the metallicRoughness texture's G/B channels, and KHR_materials_transmission's
// transmissionTexture), so they're baked per-material into small atlas rows below. `ior`,
// `emissive` and `volume` have no texture variant in glTF - they're material-level scalars only -
// so they are the sole reason a palette ever needs more than one material bucket.

struct MaterialBucket {
    ior: f64,
    emissive: f64,
    volume: Option<VolumeProps>,
    /// Palette material codes, in atlas texel order.
    material_codes: Vec<MaterialCode>,
}

struct PaletteLayout {
    buckets: Vec<MaterialBucket>,
    /// Indexed by material code: (bucket index, texel index within that bucket's atlas row).
    material_refs: Vec<(usize, u32)>,
}

impl PaletteLayout {
    fn uv(&self, material_code: MaterialCode) -> (usize, [f32; 2]) {
        let (bucket, texel) = self.material_refs[material_code.index()];
        let width = self.buckets[bucket].material_codes.len() as f32;
        (bucket, [(texel as f32 + 0.5) / width, 0.5])
    }
}

type BucketKey = (u64, u64, Option<(Color, u64, u64)>);

fn bucket_key(ior: f64, emissive: f64, volume: Option<VolumeProps>) -> BucketKey {
    let norm = |x: f64| if x == 0.0 { 0.0 } else { x }; // collapse -0.0 into +0.0
    let volume_key = volume.map(|v| (v.color, norm(v.distance).to_bits(), norm(v.thickness).to_bits()));
    (norm(ior).to_bits(), norm(emissive).to_bits(), volume_key)
}

fn build_palette_layout(materials: &[MaterialProps]) -> PaletteLayout {
    let mut buckets_by_key: HashMap<BucketKey, usize> = HashMap::new();
    let mut layout = PaletteLayout {
        buckets: Vec::new(),
        material_refs: Vec::with_capacity(materials.len()),
    };
    for (index, m) in materials.iter().enumerate() {
        let code = MaterialCode::new(index);
        let key = bucket_key(m.ior, m.emissive, m.volume);
        let bucket = *buckets_by_key.entry(key).or_insert_with(|| {
            layout.buckets.push(MaterialBucket {
                ior: m.ior,
                emissive: m.emissive,
                volume: m.volume,
                material_codes: Vec::new(),
            });
            layout.buckets.len() - 1
        });
        let texel = layout.buckets[bucket].material_codes.len() as u32;
        layout.buckets[bucket].material_codes.push(code);
        layout.material_refs.push((bucket, texel));
    }
    layout
}

fn to_u8(x: f64) -> u8 {
    (x.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn build_palette_data(materials: &[MaterialProps], layout: &PaletteLayout) -> PaletteData {
    let materials_data = layout
        .buckets
        .iter()
        .map(|bucket| {
            let mut base_color = Vec::with_capacity(bucket.material_codes.len());
            let mut metallic_roughness = Vec::with_capacity(bucket.material_codes.len());
            let mut transmission = Vec::with_capacity(bucket.material_codes.len());
            for &code in &bucket.material_codes {
                let m = &materials[code.index()];
                base_color.push([m.color.r, m.color.g, m.color.b]);
                metallic_roughness.push([to_u8(m.roughness), to_u8(m.metallic)]);
                transmission.push(to_u8(m.transmission));
            }
            MaterialData {
                ior: bucket.ior,
                emissive: bucket.emissive,
                volume: bucket.volume,
                atlas_width: bucket.material_codes.len() as u32,
                atlas_height: 1,
                base_color,
                metallic_roughness,
                transmission,
            }
        })
        .collect();
    PaletteData {
        materials: materials_data,
    }
}

// --- Greedy meshing ---
//
// A face is culled only when its neighbor voxel is present and either opaque or the exact same
// material as the current voxel - so material id doubles as the merge key for growing quads,
// since two voxels only ever render identically (same UV, same visibility) when they share a
// material id.
//
// Every quad gets 4 fresh vertices (no sharing across quads) with one flat per-quad normal, and
// every position is an exact integer grid coordinate cast to f32. That, combined with never
// interpolating any attribute across a quad boundary, is what keeps T-junctions from ever
// producing visible cracks or shading seams here, regardless of how differently adjacent quads
// are merged.

#[derive(Default)]
struct PrimitiveBuilder {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

impl PrimitiveBuilder {
    fn push_quad(&mut self, corners: [[f32; 3]; 4], normal: [f32; 3], uv: [f32; 2]) {
        let base = self.positions.len() as u32;
        self.positions.extend(corners);
        self.normals.extend([normal; 4]);
        self.uvs.extend([uv; 4]);
        self.indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    fn into_primitive(self, material_index: usize) -> Primitive {
        Primitive {
            positions: self.positions,
            normals: self.normals,
            uvs: self.uvs,
            indices: self.indices,
            material_index,
        }
    }
}

fn build_mesh(
    dims: [usize; 3],
    data: &[Option<MaterialCode>],
    materials: &[MaterialProps],
    layout: &PaletteLayout,
) -> MeshData {
    let [dx, dy, _dz] = dims;
    let voxel_at = |p: [usize; 3]| -> Option<MaterialCode> { data[p[0] + p[1] * dx + p[2] * dx * dy] };
    let is_opaque = |code: MaterialCode| materials[code.index()].transmission == 0.0;

    let mut builders: Vec<PrimitiveBuilder> = (0..layout.buckets.len()).map(|_| PrimitiveBuilder::default()).collect();
    for d in 0..3 {
        let basis = Basis {
            d,
            u_axis: (d + 1) % 3,
            v_axis: (d + 2) % 3,
        };
        mesh_axis(basis, dims, &voxel_at, &is_opaque, layout, &mut builders);
    }

    let primitives = builders
        .into_iter()
        .enumerate()
        .filter(|(_, b)| !b.indices.is_empty())
        .map(|(i, b)| b.into_primitive(i))
        .collect();
    MeshData { primitives }
}

/// A right-handed `(d, u_axis, v_axis)` frame for one of the 3 sweep axes: `d` is the face-normal
/// axis, `u_axis`/`v_axis` span the slice plane. Always right-handed since X x Y = Z, Y x Z = X,
/// Z x X = Y, which is what makes the winding in `emit_quad` correct for every axis.
#[derive(Clone, Copy)]
struct Basis {
    d: usize,
    u_axis: usize,
    v_axis: usize,
}

fn mesh_axis(
    basis: Basis,
    dims: [usize; 3],
    voxel_at: &impl Fn([usize; 3]) -> Option<MaterialCode>,
    is_opaque: &impl Fn(MaterialCode) -> bool,
    layout: &PaletteLayout,
    builders: &mut [PrimitiveBuilder],
) {
    let Basis { d, u_axis, v_axis } = basis;
    let dim_u = dims[u_axis];
    let dim_v = dims[v_axis];

    for i in 0..=dims[d] {
        let mut mask_pos = vec![None; dim_u * dim_v];
        let mut mask_neg = vec![None; dim_u * dim_v];

        for v in 0..dim_v {
            for u in 0..dim_u {
                let place = |along_d: usize| {
                    let mut p = [0; 3];
                    p[d] = along_d;
                    p[u_axis] = u;
                    p[v_axis] = v;
                    p
                };
                let neg = if i > 0 { voxel_at(place(i - 1)) } else { None };
                let pos = if i < dims[d] { voxel_at(place(i)) } else { None };
                let idx = v * dim_u + u;
                if let Some(id) = neg
                    && face_visible(id, pos, is_opaque)
                {
                    mask_pos[idx] = Some(id);
                }
                if let Some(id) = pos
                    && face_visible(id, neg, is_opaque)
                {
                    mask_neg[idx] = Some(id);
                }
            }
        }

        greedy_rects(&mut mask_pos, dim_u, dim_v, |u0, v0, u1, v1, id| {
            emit_quad(basis, i, (u0, v0, u1, v1), id, true, layout, builders);
        });
        greedy_rects(&mut mask_neg, dim_u, dim_v, |u0, v0, u1, v1, id| {
            emit_quad(basis, i, (u0, v0, u1, v1), id, false, layout, builders);
        });
    }
}

fn face_visible(a: MaterialCode, neighbor: Option<MaterialCode>, is_opaque: &impl Fn(MaterialCode) -> bool) -> bool {
    match neighbor {
        None => true,
        Some(b) => !(is_opaque(b) || b == a),
    }
}

/// Consumes a 2D mask of (material code or empty), emitting one maximal rectangle per contiguous
/// same-code region via the standard greedy-growth sweep.
fn greedy_rects(
    mask: &mut [Option<MaterialCode>],
    dim_u: usize,
    dim_v: usize,
    mut emit: impl FnMut(usize, usize, usize, usize, MaterialCode),
) {
    for v in 0..dim_v {
        let mut u = 0;
        while u < dim_u {
            let Some(code) = mask[v * dim_u + u] else {
                u += 1;
                continue;
            };
            let mut w = 1;
            while u + w < dim_u && mask[v * dim_u + u + w] == Some(code) {
                w += 1;
            }
            let mut h = 1;
            'grow: while v + h < dim_v {
                for k in 0..w {
                    if mask[(v + h) * dim_u + u + k] != Some(code) {
                        break 'grow;
                    }
                }
                h += 1;
            }
            for hh in 0..h {
                for ww in 0..w {
                    mask[(v + hh) * dim_u + u + ww] = None;
                }
            }
            emit(u, v, u + w, v + h, code);
            u += w;
        }
    }
}

fn emit_quad(
    basis: Basis,
    i: usize,
    (u0, v0, u1, v1): (usize, usize, usize, usize),
    material_code: MaterialCode,
    positive: bool,
    layout: &PaletteLayout,
    builders: &mut [PrimitiveBuilder],
) {
    let Basis { d, u_axis, v_axis } = basis;
    let corner = |u: usize, v: usize| {
        let mut p = [0.0f32; 3];
        p[d] = i as f32;
        p[u_axis] = u as f32;
        p[v_axis] = v as f32;
        p
    };
    let (p0, p1, p2, p3) = (corner(u0, v0), corner(u1, v0), corner(u1, v1), corner(u0, v1));
    // [p0,p1,p2,p3] winds counter-clockwise for a +d-facing quad (see `Basis`'s right-handedness
    // note); reversing it flips the normal.
    let corners = if positive { [p0, p1, p2, p3] } else { [p0, p3, p2, p1] };
    let mut normal = [0.0f32; 3];
    normal[d] = if positive { 1.0 } else { -1.0 };
    let (bucket, uv) = layout.uv(material_code);
    builders[bucket].push_quad(corners, normal, uv);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color { r, g, b }
    }

    fn code(index: usize) -> Option<MaterialCode> {
        Some(MaterialCode::new(index))
    }

    fn material(color: Color, transmission: f64) -> MaterialProps {
        MaterialProps {
            color,
            roughness: 1.0,
            metallic: 0.0,
            ior: 1.5,
            transmission,
            emissive: 0.0,
            volume: None,
        }
    }

    fn quad_count(mesh: &MeshData) -> usize {
        mesh.primitives.iter().map(|p| p.indices.len() / 6).sum()
    }

    #[test]
    fn single_opaque_voxel_has_six_faces() {
        let materials = vec![material(rgb(255, 0, 0), 0.0)];
        let layout = build_palette_layout(&materials);
        let mesh = build_mesh([1, 1, 1], &[code(0)], &materials, &layout);

        assert_eq!(mesh.primitives.len(), 1);
        let prim = &mesh.primitives[0];
        assert_eq!(prim.positions.len(), 24);
        assert_eq!(prim.indices.len(), 36);
    }

    #[test]
    fn adjacent_same_color_merges_sides_and_culls_shared_face() {
        let materials = vec![material(rgb(0, 255, 0), 0.0)];
        let layout = build_palette_layout(&materials);
        let mesh = build_mesh([2, 1, 1], &[code(0), code(0)], &materials, &layout);

        assert_eq!(quad_count(&mesh), 6);
        // No vertex should ever land on the internal boundary plane x=1: end caps sit at x=0/x=2,
        // and merged side quads span directly from x=0 to x=2.
        let prim = &mesh.primitives[0];
        assert!(prim.positions.iter().all(|p| p[0] != 1.0));
    }

    #[test]
    fn different_opaque_colors_cull_shared_face_but_dont_merge_sides() {
        let materials = vec![material(rgb(255, 0, 0), 0.0), material(rgb(0, 0, 255), 0.0)];
        let layout = build_palette_layout(&materials);
        let mesh = build_mesh([2, 1, 1], &[code(0), code(1)], &materials, &layout);

        assert_eq!(quad_count(&mesh), 10);
    }

    #[test]
    fn transparent_same_color_still_merges_and_culls() {
        let materials = vec![material(rgb(0, 255, 0), 0.5)];
        let layout = build_palette_layout(&materials);
        let mesh = build_mesh([2, 1, 1], &[code(0), code(0)], &materials, &layout);

        assert_eq!(quad_count(&mesh), 6);
    }

    #[test]
    fn transparent_different_colors_do_not_cull() {
        let materials = vec![material(rgb(255, 0, 0), 0.5), material(rgb(0, 0, 255), 0.5)];
        let layout = build_palette_layout(&materials);
        let mesh = build_mesh([2, 1, 1], &[code(0), code(1)], &materials, &layout);

        assert_eq!(quad_count(&mesh), 12);
    }

    #[test]
    fn distinct_ior_creates_separate_buckets() {
        let materials = vec![
            MaterialProps {
                ior: 1.0,
                ..material(rgb(255, 0, 0), 0.0)
            },
            MaterialProps {
                ior: 1.5,
                ..material(rgb(0, 255, 0), 0.0)
            },
        ];
        let layout = build_palette_layout(&materials);

        assert_eq!(layout.buckets.len(), 2);
        assert_eq!(layout.buckets[0].material_codes.len(), 1);
        assert_eq!(layout.buckets[1].material_codes.len(), 1);
    }

    #[test]
    fn distinct_volume_creates_separate_buckets() {
        let materials = vec![
            MaterialProps {
                volume: Some(VolumeProps {
                    color: rgb(0, 128, 255),
                    distance: 1.0,
                    thickness: 1.0,
                }),
                ..material(rgb(255, 0, 0), 0.8)
            },
            MaterialProps {
                volume: Some(VolumeProps {
                    color: rgb(255, 128, 0),
                    distance: 1.0,
                    thickness: 1.0,
                }),
                ..material(rgb(255, 0, 0), 0.8)
            },
        ];
        let layout = build_palette_layout(&materials);

        assert_eq!(layout.buckets.len(), 2);
        assert_eq!(layout.buckets[0].material_codes.len(), 1);
        assert_eq!(layout.buckets[1].material_codes.len(), 1);
    }

    #[test]
    fn shared_bucket_atlas_orders_colors_by_index() {
        let materials = vec![material(rgb(10, 20, 30), 0.0), material(rgb(40, 50, 60), 0.0)];
        let layout = build_palette_layout(&materials);

        assert_eq!(layout.buckets.len(), 1);
        assert_eq!(layout.buckets[0].material_codes.len(), 2);
        let (b0, uv0) = layout.uv(MaterialCode::new(0));
        let (b1, uv1) = layout.uv(MaterialCode::new(1));
        assert_eq!(b0, b1);
        assert_eq!(uv0, [0.25, 0.5]);
        assert_eq!(uv1, [0.75, 0.5]);

        let palette_data = build_palette_data(&materials, &layout);
        assert_eq!(palette_data.materials[0].base_color, vec![[10, 20, 30], [40, 50, 60]]);
    }

    #[test]
    fn bucket_key_normalizes_negative_zero() {
        assert_eq!(bucket_key(0.0, 0.0, None), bucket_key(-0.0, -0.0, None));
    }
}
