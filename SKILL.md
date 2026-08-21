---
name: create-voxel-models
description: Create and animate voxel models with support for PBR materials, preview them and export them in glTF scenes
author: Benjamin Lefebvre
version: 0.1.0
tags: [3d, assets, gltf, voxel]
license: MIT
requires: [python]
---

# Introduction

This skill contains a python package that enables programmatic creation, edition and animation of voxel models, organized in hierarchical scenes. Similar to how the glTF format is structured, but with voxels. Voxels have colors and PBR properties, with up to 255 different materials per model.

# Installation

Before anything else, you must make sure the `voxels` package is installed, or install it yourself. It's up to you to decide where and how to install the `.whl` file bundled with this skill in the `dist/` directory. 

If operating in an user's machine, you should probably install it in a venv. Consult with the user to know their preferred python package manager and wether they want this package installed globally or in a venv.

If operating in a sandbox, it's probably fine to install this package globally. Be sure to follow all instructions in your prompts.

# Package Overview

There are a few major concepts:
* `Palette`s contain `Material`s, which are a `Color` and PBR properties (such as `metallic`, `transmission`...).
* `Model`s are a 3D array of voxels, they reference one `Palette`, and offer an API to edit them through primitive shapes such as boxes and spheres.
* `Scene`s are a tree of `Node`s. `Model`s can be attached to `Node`s. `Scene`s can be animated through the `Anim` API, by attaching translations, rotations and scalings (TRS) to `Node`s.

Finally, `Scene`s can be exported to `.glb` files, and they can be previewed to `.png` under different angles and at different times of an animation to get immediate feedback.

You can get a full reference of what APIs this package offers in the `reference/voxels.pyi` file bundled with this skill.

# Project structuring

`Palette`s can and should be shared between `Model`s. `Model`s are composable and should be easily accessible. This begs a project structure such as this one:

```
palettes/
models/
scenes/
```

`models/` can be further divided into subfolders, if the project size grows too large, to group sets of `Model`s used in a given `Scene`, or similar `Model`s, for example.


