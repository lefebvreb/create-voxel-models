# voxel_lib

A library that allows programmatically creating and editing game-ready Voxel models. Features:
* An API to create and edit models by adding primitive shapes or single voxels.
* PBR materials for glass, light emitters and metals.
* Hierarchical scenes with support for simple animations (translation, scale, rotation...), using empty nodes as controllers.
* Efficient and correct meshing algorithm (such as greedy meshing).
* Exporting scenes to the .GLB format.
* Rendering models under different angles and at different keyframes with headless bevy, exporting these renders to PNGs.

The goal is then to make a native python lib that wraps these APIs and allows constructing such scenes 
from python scripts. Along with an agentic skill, this would allow AI agents to create voxel models. The rendering +
vision capabilities would allow them to get quality feedback on their own work.

We won't support human editing of AI-generated models for now. That would most likely entail a system to export/import
scenes to the .VOX file format. Even though we won't support that yet, we still need to be careful to make whatever
data model we use for our scenes and models be compatible with both .VOX and .GLB

All code already written can be thrown away if necessary.
