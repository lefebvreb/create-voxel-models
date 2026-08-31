// <ai-owned/>

//! A minimal software triangle rasterizer: an edge-function scanline fill with a z-buffer and
//! perspective-correct attribute interpolation, plus a linear-space supersample-downsample for
//! antialiasing. Deliberately generic over *what* gets interpolated (`ScreenVertex<N>`'s `N`
//! float attributes - normal, uv, whatever a caller needs) and knows nothing about cameras,
//! materials or lighting: those are the next layer up. No pyo3, independently testable.
//!
//! **Known simplification, not a bug to fix here**: no near-plane/frustum clipping. A triangle
//! with any vertex behind the camera isn't something this module is asked to handle - the caller
//! (which owns the projection) is expected to discard such triangles before calling
//! [`rasterize_triangle`], rather than this module attempting to clip them. For a voxel-preview
//! renderer with a fitted, padded camera this only matters at extreme zoom levels; degrading to
//! "the triangle doesn't draw" rather than mis-rendering it is an acceptable, disclosed bound for
//! that case.

/// An RGB framebuffer with a z-buffer, both at the given resolution. `color` holds **linear**
/// light values, not display-encoded ones - see [`downsample_to_srgb8`] for why that matters.
pub struct Framebuffer {
    pub width: u32,
    pub height: u32,
    pub color: Vec<[f32; 3]>,
    pub depth: Vec<f32>,
}

impl Framebuffer {
    pub fn new(width: u32, height: u32, clear_color: [f32; 3]) -> Self {
        let n = (width * height) as usize;
        Self {
            width,
            height,
            color: vec![clear_color; n],
            depth: vec![f32::INFINITY; n],
        }
    }

    fn index(&self, x: u32, y: u32) -> usize {
        (y * self.width + x) as usize
    }

    /// Repaints every pixel from `f`, called with the pixel's vertical position from `0.0` (top
    /// row) to `1.0` (bottom row) - used to lay down a gradient backdrop before any geometry is
    /// drawn. Leaves the z-buffer untouched.
    pub fn fill_background(&mut self, f: impl Fn(f32) -> [f32; 3]) {
        let denom = (self.height.max(2) - 1) as f32;
        for y in 0..self.height {
            let color = f(y as f32 / denom);
            for x in 0..self.width {
                let index = self.index(x, y);
                self.color[index] = color;
            }
        }
    }
}

/// A triangle vertex already projected to screen space: `x`/`y` in pixel coordinates, `depth` a
/// value where smaller means closer (compared directly for the depth test - any monotonic
/// projected-depth convention works, this module doesn't care which), and `inv_w`/`attributes`
/// for perspective-correct interpolation of arbitrary per-vertex data (e.g. world normal, uv).
#[derive(Clone, Copy)]
pub struct ScreenVertex<const N: usize> {
    pub x: f32,
    pub y: f32,
    pub depth: f32,
    pub inv_w: f32,
    pub attributes: [f32; N],
}

/// The doubled signed area of `(v0, v1, v2)` in screen space - positive for one winding order,
/// negative for the other, near zero for a degenerate triangle.
fn signed_area<const N: usize>(v0: &ScreenVertex<N>, v1: &ScreenVertex<N>, v2: &ScreenVertex<N>) -> f32 {
    edge(v0.x, v0.y, v1.x, v1.y, v2.x, v2.y)
}

/// Fills triangle `(v0, v1, v2)` into `fb`, depth-testing each covered pixel (closer wins) and
/// calling `shade` with its screen-space pixel center, perspective-correct interpolated
/// attributes, and the color already in the framebuffer at that pixel (so a transmissive/blended
/// fragment can composite against whatever was drawn there first). `shade` returns `None` to
/// discard the fragment without writing color or depth (e.g. an alpha cutout), or `Some(color)`
/// to write it. Fills either winding order; a (near-)zero-area triangle is skipped as degenerate.
pub fn rasterize_triangle<const N: usize>(
    fb: &mut Framebuffer,
    v0: ScreenVertex<N>,
    v1: ScreenVertex<N>,
    v2: ScreenVertex<N>,
    mut shade: impl FnMut(f32, f32, [f32; N], [f32; 3]) -> Option<[f32; 3]>,
) {
    let area = signed_area(&v0, &v1, &v2);
    if area.abs() < f32::EPSILON {
        return;
    }

    let min_x = v0.x.min(v1.x).min(v2.x).floor().max(0.0) as u32;
    let max_x = (v0.x.max(v1.x).max(v2.x).ceil() as i64).clamp(0, fb.width as i64) as u32;
    let min_y = v0.y.min(v1.y).min(v2.y).floor().max(0.0) as u32;
    let max_y = (v0.y.max(v1.y).max(v2.y).ceil() as i64).clamp(0, fb.height as i64) as u32;
    if min_x >= max_x || min_y >= max_y {
        return;
    }

    let attrs_over_w: [[f32; N]; 3] = [
        std::array::from_fn(|i| v0.attributes[i] * v0.inv_w),
        std::array::from_fn(|i| v1.attributes[i] * v1.inv_w),
        std::array::from_fn(|i| v2.attributes[i] * v2.inv_w),
    ];

    for y in min_y..max_y {
        for x in min_x..max_x {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let w0 = edge(v1.x, v1.y, v2.x, v2.y, px, py);
            let w1 = edge(v2.x, v2.y, v0.x, v0.y, px, py);
            let w2 = edge(v0.x, v0.y, v1.x, v1.y, px, py);
            let inside = if area > 0.0 {
                w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0
            } else {
                w0 <= 0.0 && w1 <= 0.0 && w2 <= 0.0
            };
            if !inside {
                continue;
            }
            let (b0, b1, b2) = (w0 / area, w1 / area, w2 / area);

            let depth = b0 * v0.depth + b1 * v1.depth + b2 * v2.depth;
            let index = fb.index(x, y);
            if depth >= fb.depth[index] {
                continue;
            }

            let inv_w = b0 * v0.inv_w + b1 * v1.inv_w + b2 * v2.inv_w;
            let attributes: [f32; N] = std::array::from_fn(|i| {
                (b0 * attrs_over_w[0][i] + b1 * attrs_over_w[1][i] + b2 * attrs_over_w[2][i]) / inv_w
            });

            if let Some(color) = shade(px, py, attributes, fb.color[index]) {
                fb.color[index] = color;
                fb.depth[index] = depth;
            }
        }
    }
}

fn edge(ax: f32, ay: f32, bx: f32, by: f32, px: f32, py: f32) -> f32 {
    (bx - ax) * (py - ay) - (by - ay) * (px - ax)
}

/// Box-downsamples `fb.color` from `fb.width x fb.height` to `(fb.width/factor) x
/// (fb.height/factor)`, averaging each `factor x factor` block **in linear space** before
/// converting the result to sRGB8 - averaging happens before the sRGB encode, not after,
/// otherwise the result is measurably too dark (naively averaging already-gamma-encoded values
/// is a common antialiasing bug). This is the renderer's only antialiasing: rendering at
/// `factor`x the target resolution and box-filtering down, rather than any dedicated MSAA
/// machinery. Returns `(width, height, rgb8_bytes)`, ready for `utils::encode_png`.
pub fn downsample_to_srgb8(fb: &Framebuffer, factor: u32) -> (u32, u32, Vec<u8>) {
    assert!(factor >= 1, "downsample factor must be at least 1");
    let out_w = fb.width / factor;
    let out_h = fb.height / factor;
    let mut out = Vec::with_capacity((out_w * out_h * 3) as usize);
    let n = (factor * factor) as f32;

    for oy in 0..out_h {
        for ox in 0..out_w {
            let mut sum = [0.0f32; 3];
            for sy in 0..factor {
                for sx in 0..factor {
                    let c = fb.color[fb.index(ox * factor + sx, oy * factor + sy)];
                    sum[0] += c[0];
                    sum[1] += c[1];
                    sum[2] += c[2];
                }
            }
            for channel in sum {
                out.push((linear_to_srgb(channel / n).clamp(0.0, 1.0) * 255.0).round() as u8);
            }
        }
    }
    (out_w, out_h, out)
}

/// The standard IEC 61966-2-1 linear-to-sRGB transfer function.
fn linear_to_srgb(c: f32) -> f32 {
    let c = c.max(0.0);
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vertex(x: f32, y: f32, depth: f32, inv_w: f32) -> ScreenVertex<1> {
        ScreenVertex {
            x,
            y,
            depth,
            inv_w,
            attributes: [0.0],
        }
    }

    #[test]
    fn fills_pixels_inside_a_triangle_and_leaves_pixels_outside_it() {
        let mut fb = Framebuffer::new(4, 4, [0.0, 0.0, 0.0]);
        // A right triangle covering roughly the top-left quadrant, CCW-wound.
        rasterize_triangle(
            &mut fb,
            vertex(0.0, 0.0, 0.5, 1.0),
            vertex(4.0, 0.0, 0.5, 1.0),
            vertex(0.0, 4.0, 0.5, 1.0),
            |_, _, _, _| Some([1.0, 1.0, 1.0]),
        );
        assert_eq!(fb.color[fb.index(0, 0)], [1.0, 1.0, 1.0]); // inside
        assert_eq!(fb.color[fb.index(3, 3)], [0.0, 0.0, 0.0]); // outside (bottom-right corner)
    }

    #[test]
    fn fills_either_winding_order() {
        let mut fb_ccw = Framebuffer::new(4, 4, [0.0, 0.0, 0.0]);
        rasterize_triangle(
            &mut fb_ccw,
            vertex(0.0, 0.0, 0.5, 1.0),
            vertex(4.0, 0.0, 0.5, 1.0),
            vertex(0.0, 4.0, 0.5, 1.0),
            |_, _, _, _| Some([1.0, 0.0, 0.0]),
        );
        let mut fb_cw = Framebuffer::new(4, 4, [0.0, 0.0, 0.0]);
        rasterize_triangle(
            &mut fb_cw,
            vertex(0.0, 0.0, 0.5, 1.0),
            vertex(0.0, 4.0, 0.5, 1.0),
            vertex(4.0, 0.0, 0.5, 1.0),
            |_, _, _, _| Some([1.0, 0.0, 0.0]),
        );
        assert_eq!(fb_ccw.color[fb_ccw.index(0, 0)], fb_cw.color[fb_cw.index(0, 0)]);
    }

    #[test]
    fn degenerate_triangle_does_not_panic_or_draw() {
        let mut fb = Framebuffer::new(4, 4, [0.5, 0.5, 0.5]);
        rasterize_triangle(
            &mut fb,
            vertex(1.0, 1.0, 0.5, 1.0),
            vertex(1.0, 1.0, 0.5, 1.0),
            vertex(1.0, 1.0, 0.5, 1.0),
            |_, _, _, _| Some([1.0, 0.0, 0.0]),
        );
        assert!(fb.color.iter().all(|&c| c == [0.5, 0.5, 0.5]));
    }

    #[test]
    fn closer_triangle_wins_the_depth_test_regardless_of_draw_order() {
        let mut fb = Framebuffer::new(4, 4, [0.0, 0.0, 0.0]);
        let full_quad = |depth: f32, color: [f32; 3]| {
            move |fb: &mut Framebuffer| {
                rasterize_triangle(
                    fb,
                    vertex(0.0, 0.0, depth, 1.0),
                    vertex(4.0, 0.0, depth, 1.0),
                    vertex(0.0, 4.0, depth, 1.0),
                    {
                        let color = color;
                        move |_, _, _, _| Some(color)
                    },
                );
                rasterize_triangle(
                    fb,
                    vertex(4.0, 0.0, depth, 1.0),
                    vertex(4.0, 4.0, depth, 1.0),
                    vertex(0.0, 4.0, depth, 1.0),
                    {
                        let color = color;
                        move |_, _, _, _| Some(color)
                    },
                );
            }
        };
        // Draw the *farther* (larger-depth) triangle second; it must not overwrite the nearer one.
        full_quad(0.2, [1.0, 0.0, 0.0])(&mut fb);
        full_quad(0.8, [0.0, 1.0, 0.0])(&mut fb);
        assert_eq!(fb.color[fb.index(1, 1)], [1.0, 0.0, 0.0]);
    }

    #[test]
    fn shade_returning_none_discards_without_writing_color_or_depth() {
        let mut fb = Framebuffer::new(4, 4, [0.25, 0.25, 0.25]);
        rasterize_triangle(
            &mut fb,
            vertex(0.0, 0.0, 0.1, 1.0),
            vertex(4.0, 0.0, 0.1, 1.0),
            vertex(0.0, 4.0, 0.1, 1.0),
            |_, _, _, _| None,
        );
        assert!(fb.color.iter().all(|&c| c == [0.25, 0.25, 0.25]));
        assert!(fb.depth.iter().all(|&d| d == f32::INFINITY));
    }

    #[test]
    fn perspective_correct_interpolation_differs_from_naive_linear() {
        // Two vertices share an edge; give them very different inv_w so the perspective-correct
        // midpoint attribute visibly differs from a naive (non-perspective) 0.5/0.5 blend.
        let mut fb = Framebuffer::new(2, 1, [0.0, 0.0, 0.0]);
        let v0 = ScreenVertex {
            x: 0.0,
            y: 0.0,
            depth: 0.5,
            inv_w: 1.0,
            attributes: [0.0],
        };
        let v1 = ScreenVertex {
            x: 2.0,
            y: 0.0,
            depth: 0.5,
            inv_w: 0.1,
            attributes: [10.0],
        };
        let v2 = ScreenVertex {
            x: 0.0,
            y: 1.0,
            depth: 0.5,
            inv_w: 1.0,
            attributes: [0.0],
        };
        let mut sampled = None;
        rasterize_triangle(&mut fb, v0, v1, v2, |_, _, attrs, _| {
            sampled = Some(attrs[0]);
            Some([1.0, 1.0, 1.0])
        });
        // A naive (non-perspective) barycentric blend halfway along the v0-v1 edge would give
        // 5.0; perspective correction pulls it toward v1's side since v1's inv_w is smaller
        // (farther away), so the true value is well below the naive midpoint.
        assert!(sampled.unwrap() < 5.0);
    }

    #[test]
    fn downsample_averages_in_linear_space_not_after_srgb_encoding() {
        let mut fb = Framebuffer::new(2, 2, [0.0, 0.0, 0.0]);
        fb.color[0] = [1.0, 1.0, 1.0]; // white
        fb.color[1] = [0.0, 0.0, 0.0]; // black
        fb.color[2] = [1.0, 1.0, 1.0];
        fb.color[3] = [0.0, 0.0, 0.0];

        let (w, h, pixels) = downsample_to_srgb8(&fb, 2);
        assert_eq!((w, h), (1, 1));
        // Correct: average 0.5 in *linear* space, then sRGB-encode -> ~188. A naive
        // average-after-encoding bug would give 127 instead.
        assert!(pixels[0] > 180 && pixels[0] < 195, "got {}", pixels[0]);
    }

    #[test]
    fn downsample_of_a_uniform_image_reproduces_its_srgb_value() {
        let fb = Framebuffer::new(2, 2, [1.0, 0.0, 0.0]);
        let (_, _, pixels) = downsample_to_srgb8(&fb, 2);
        assert_eq!(pixels, vec![255, 0, 0]);
    }
}
