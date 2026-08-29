// <ai-owned/>

mod animation;
mod glb;
mod gltf;
mod meshing;
mod raster;
mod rendering;
mod scene_graph;
mod utils;

pub use glb::export_glb;
pub use rendering::render;

// `evaluate_node_trs`/`read_glb` aren't consumed anywhere yet - node traversal (the next piece
// of the CPU-rasterizer rewrite) is what wires them in. Re-exported now so this module's public
// shape doesn't need revisiting once that lands.
#[allow(unused_imports)]
pub use animation::evaluate_node_trs;
#[allow(unused_imports)]
pub use glb::read_glb;
#[allow(unused_imports)]
pub use scene_graph::collect_world_primitives;
