use std::collections::HashMap;

use pyo3::{Bound, PyResult};

use crate::model::Model;
use crate::palette::{Color, Palette};

// Voxel grid axes map directly onto glTF's X/Y/Z with no remap: x -> X, y -> Y (up), z -> Z.

/// Plain snapshot of the palette-relevant `Color` fields, decoupled from pyo3 so the meshing and
/// atlas-layout logic below is pure Rust and independently testable.
#[derive(Clone, Copy)]
struct ColorProps {
    rgb: (u8, u8, u8),
    roughness: f64,
    metallic: f64,
    ior: f64,
    transmission: f64,
    emissive: f64,
}

impl From<&Color> for ColorProps {
    fn from(c: &Color) -> Self {
        Self {
            rgb: c.rgb,
            roughness: c.roughness,
            metallic: c.metallic,
            ior: c.ior,
            transmission: c.transmission,
            emissive: c.emissive,
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
    let colors: Vec<ColorProps> = palette_ref.colors.iter().map(|c| ColorProps::from(c.get())).collect();
    let layout = build_palette_layout(&colors);
    let (dx, dy, dz) = model_ref.dimensions;
    Ok(build_mesh([dx, dy, dz], &model_ref.data, &colors, &layout))
}

pub fn export_palette(palette: Bound<Palette>) -> PyResult<PaletteData> {
    let palette_ref = palette.borrow();
    let colors: Vec<ColorProps> = palette_ref.colors.iter().map(|c| ColorProps::from(c.get())).collect();
    let layout = build_palette_layout(&colors);
    Ok(build_palette_data(&colors, &layout))
}

// --- Palette layout: groups colors into material buckets and lays out atlas texels ---
//
// `rgb`, `roughness`/`metallic` and `transmission` all have direct glTF texture equivalents
// (baseColor, the metallicRoughness texture's G/B channels, and KHR_materials_transmission's
// transmissionTexture), so they're baked per-color into small atlas rows below. `ior` and
// `emissive` have no texture variant in glTF - they're material-level scalars only - so they are
// the sole reason a palette ever needs more than one material bucket.

struct MaterialBucket {
    ior: f64,
    emissive: f64,
    /// Palette color ids, in atlas texel order.
    colors: Vec<u8>,
}

struct PaletteLayout {
    buckets: Vec<MaterialBucket>,
    /// Indexed by color id: (bucket index, texel index within that bucket's atlas row).
    color_refs: Vec<(usize, u32)>,
}

impl PaletteLayout {
    fn uv(&self, color_id: u8) -> (usize, [f32; 2]) {
        let (bucket, texel) = self.color_refs[color_id as usize];
        let width = self.buckets[bucket].colors.len() as f32;
        (bucket, [(texel as f32 + 0.5) / width, 0.5])
    }
}

fn bucket_key(ior: f64, emissive: f64) -> (u64, u64) {
    let norm = |x: f64| if x == 0.0 { 0.0 } else { x }; // collapse -0.0 into +0.0
    (norm(ior).to_bits(), norm(emissive).to_bits())
}

fn build_palette_layout(colors: &[ColorProps]) -> PaletteLayout {
    let mut buckets_by_key: HashMap<(u64, u64), usize> = HashMap::new();
    let mut layout = PaletteLayout {
        buckets: Vec::new(),
        color_refs: Vec::with_capacity(colors.len()),
    };
    for (id, c) in colors.iter().enumerate() {
        let key = bucket_key(c.ior, c.emissive);
        let bucket = *buckets_by_key.entry(key).or_insert_with(|| {
            layout.buckets.push(MaterialBucket {
                ior: c.ior,
                emissive: c.emissive,
                colors: Vec::new(),
            });
            layout.buckets.len() - 1
        });
        let texel = layout.buckets[bucket].colors.len() as u32;
        layout.buckets[bucket].colors.push(id as u8);
        layout.color_refs.push((bucket, texel));
    }
    layout
}

fn to_u8(x: f64) -> u8 {
    (x.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn build_palette_data(colors: &[ColorProps], layout: &PaletteLayout) -> PaletteData {
    let materials = layout
        .buckets
        .iter()
        .map(|bucket| {
            let mut base_color = Vec::with_capacity(bucket.colors.len());
            let mut metallic_roughness = Vec::with_capacity(bucket.colors.len());
            let mut transmission = Vec::with_capacity(bucket.colors.len());
            for &id in &bucket.colors {
                let c = &colors[id as usize];
                base_color.push([c.rgb.0, c.rgb.1, c.rgb.2]);
                metallic_roughness.push([to_u8(c.roughness), to_u8(c.metallic)]);
                transmission.push(to_u8(c.transmission));
            }
            MaterialData {
                ior: bucket.ior,
                emissive: bucket.emissive,
                atlas_width: bucket.colors.len() as u32,
                atlas_height: 1,
                base_color,
                metallic_roughness,
                transmission,
            }
        })
        .collect();
    PaletteData { materials }
}

// --- Greedy meshing ---
//
// A face is culled only when its neighbor voxel is present and either opaque or the exact same
// color as the current voxel - so color id doubles as the merge key for growing quads, since two
// voxels only ever render identically (same UV, same visibility) when they share a color id.
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

fn build_mesh(dims: [usize; 3], data: &[u8], colors: &[ColorProps], layout: &PaletteLayout) -> MeshData {
    let [dx, dy, _dz] = dims;
    let voxel_at = |p: [usize; 3]| -> Option<u8> {
        let v = data[p[0] + p[1] * dx + p[2] * dx * dy];
        (v != 0).then(|| v - 1)
    };
    let is_opaque = |id: u8| colors[id as usize].transmission == 0.0;

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
    voxel_at: &impl Fn([usize; 3]) -> Option<u8>,
    is_opaque: &impl Fn(u8) -> bool,
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

fn face_visible(a_id: u8, neighbor: Option<u8>, is_opaque: &impl Fn(u8) -> bool) -> bool {
    match neighbor {
        None => true,
        Some(b_id) => !(is_opaque(b_id) || b_id == a_id),
    }
}

/// Consumes a 2D mask of (color id or empty), emitting one maximal rectangle per contiguous
/// same-id region via the standard greedy-growth sweep.
fn greedy_rects(
    mask: &mut [Option<u8>],
    dim_u: usize,
    dim_v: usize,
    mut emit: impl FnMut(usize, usize, usize, usize, u8),
) {
    for v in 0..dim_v {
        let mut u = 0;
        while u < dim_u {
            let Some(id) = mask[v * dim_u + u] else {
                u += 1;
                continue;
            };
            let mut w = 1;
            while u + w < dim_u && mask[v * dim_u + u + w] == Some(id) {
                w += 1;
            }
            let mut h = 1;
            'grow: while v + h < dim_v {
                for k in 0..w {
                    if mask[(v + h) * dim_u + u + k] != Some(id) {
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
            emit(u, v, u + w, v + h, id);
            u += w;
        }
    }
}

fn emit_quad(
    basis: Basis,
    i: usize,
    (u0, v0, u1, v1): (usize, usize, usize, usize),
    color_id: u8,
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
    let (bucket, uv) = layout.uv(color_id);
    builders[bucket].push_quad(corners, normal, uv);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn color(rgb: (u8, u8, u8), transmission: f64) -> ColorProps {
        ColorProps {
            rgb,
            roughness: 1.0,
            metallic: 0.0,
            ior: 1.5,
            transmission,
            emissive: 0.0,
        }
    }

    fn quad_count(mesh: &MeshData) -> usize {
        mesh.primitives.iter().map(|p| p.indices.len() / 6).sum()
    }

    #[test]
    fn single_opaque_voxel_has_six_faces() {
        let colors = vec![color((255, 0, 0), 0.0)];
        let layout = build_palette_layout(&colors);
        let mesh = build_mesh([1, 1, 1], &[1], &colors, &layout);

        assert_eq!(mesh.primitives.len(), 1);
        let prim = &mesh.primitives[0];
        assert_eq!(prim.positions.len(), 24);
        assert_eq!(prim.indices.len(), 36);
    }

    #[test]
    fn adjacent_same_color_merges_sides_and_culls_shared_face() {
        let colors = vec![color((0, 255, 0), 0.0)];
        let layout = build_palette_layout(&colors);
        let mesh = build_mesh([2, 1, 1], &[1, 1], &colors, &layout);

        assert_eq!(quad_count(&mesh), 6);
        // No vertex should ever land on the internal boundary plane x=1: end caps sit at x=0/x=2,
        // and merged side quads span directly from x=0 to x=2.
        let prim = &mesh.primitives[0];
        assert!(prim.positions.iter().all(|p| p[0] != 1.0));
    }

    #[test]
    fn different_opaque_colors_cull_shared_face_but_dont_merge_sides() {
        let colors = vec![color((255, 0, 0), 0.0), color((0, 0, 255), 0.0)];
        let layout = build_palette_layout(&colors);
        let mesh = build_mesh([2, 1, 1], &[1, 2], &colors, &layout);

        assert_eq!(quad_count(&mesh), 10);
    }

    #[test]
    fn transparent_same_color_still_merges_and_culls() {
        let colors = vec![color((0, 255, 0), 0.5)];
        let layout = build_palette_layout(&colors);
        let mesh = build_mesh([2, 1, 1], &[1, 1], &colors, &layout);

        assert_eq!(quad_count(&mesh), 6);
    }

    #[test]
    fn transparent_different_colors_do_not_cull() {
        let colors = vec![color((255, 0, 0), 0.5), color((0, 0, 255), 0.5)];
        let layout = build_palette_layout(&colors);
        let mesh = build_mesh([2, 1, 1], &[1, 2], &colors, &layout);

        assert_eq!(quad_count(&mesh), 12);
    }

    #[test]
    fn distinct_ior_creates_separate_buckets() {
        let colors = vec![
            ColorProps {
                ior: 1.0,
                ..color((255, 0, 0), 0.0)
            },
            ColorProps {
                ior: 1.5,
                ..color((0, 255, 0), 0.0)
            },
        ];
        let layout = build_palette_layout(&colors);

        assert_eq!(layout.buckets.len(), 2);
        assert_eq!(layout.buckets[0].colors.len(), 1);
        assert_eq!(layout.buckets[1].colors.len(), 1);
    }

    #[test]
    fn shared_bucket_atlas_orders_colors_by_index() {
        let colors = vec![color((10, 20, 30), 0.0), color((40, 50, 60), 0.0)];
        let layout = build_palette_layout(&colors);

        assert_eq!(layout.buckets.len(), 1);
        assert_eq!(layout.buckets[0].colors.len(), 2);
        let (b0, uv0) = layout.uv(0);
        let (b1, uv1) = layout.uv(1);
        assert_eq!(b0, b1);
        assert_eq!(uv0, [0.25, 0.5]);
        assert_eq!(uv1, [0.75, 0.5]);

        let palette_data = build_palette_data(&colors, &layout);
        assert_eq!(palette_data.materials[0].base_color, vec![[10, 20, 30], [40, 50, 60]]);
    }

    #[test]
    fn bucket_key_normalizes_negative_zero() {
        assert_eq!(bucket_key(0.0, 0.0), bucket_key(-0.0, -0.0));
    }
}
