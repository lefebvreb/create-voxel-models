# voxels

A native Python package written in Rust that allows programmatically creating and editing game-ready 3d voxel models. Features:
* An API to create and edit models by adding primitive shapes or single voxels.
* PBR materials for diffuse materials, glasses, light emitters and metals.
* Hierarchical scenes with support for simple animations (translation, scale, rotation...), using nodes as controllers.
* Efficient greedy meshing of 3d models.
* Exporting scenes to the binary glTF 2.0 format (`.glb`).
* Rendering models under different angles and at different keyframes with headless bevy, exporting these renders to `.png`s for LLM vision.

Human editing of AI-generated models isn't supported for now. That would most likely entail a system to export/import
scenes to the `.vox` file format, which comes with its own challenges.

MIT License.
