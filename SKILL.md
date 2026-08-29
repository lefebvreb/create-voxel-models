---
name: create-voxel-models
description: Create and animate voxel models with support for PBR materials, preview them and export them in glTF scenes.
license: MIT
compatibility: Requires Python 3.8+
metadata:
  author: Benjamin Lefebvre
  version: 0.1.0
---

# Introduction

This skill contains a python package that enables programmatic creation, edition and animation of voxel models, organized in hierarchical scenes. Similar to how the glTF format is structured, but with voxels. Voxels have colors and PBR properties, with up to 255 different materials per model.

## Installation

Before anything else, you must make sure the `voxels` package is installed, or install it yourself. It's up to you to decide where and how to install the `.whl` file bundled with this skill in the `dist/` directory. 

If operating in an user's machine, you should probably install it in a venv. Consult with the user to know their preferred python package manager and wether they want this package installed globally or in a venv.

If operating in a sandbox, it's probably fine to install this package globally. Be sure to follow all instructions in your prompts.

## Package Overview

There are a few major concepts:
* `Palette`s contain `Material`s, which are a `Color` and PBR properties (such as `metallic`, `transmission`...).
* `Model`s are a 3D array of voxels, they reference one `Palette`, and offer an API to edit them through primitive shapes such as boxes and spheres.
* `Scene`s are a tree of `Node`s. `Model`s can be attached to `Node`s. `Scene`s can be animated through the `Anim` API, by attaching translations, rotations and scalings (TRS) to `Node`s.

Finally, `Scene`s can be exported to `.glb` files, and they can be previewed to `.png` under different angles and at different times of an animation to get immediate feedback.

You should get a full reference of what APIs this package offers in the `reference/voxels.pyi` file bundled with this skill.

# Usage

## Project structuring

`Palette`s can and should be shared between `Model`s. `Model`s are composable and should be easily accessible. This naturally leads to adopting such a project structure as:

```
voxels
├── palettes
│   ├── furniture.py
│   └── ...
├── models
│   ├── furniture
│   │   ├── chair.py
│   │   └── ...
│   └── ...
├── scenes
│   ├── furniture.py
│   └── ...
└── GUIDELINES.md
```

Use `GUIDELINES.md` to store general guidelines for keeping consistency between models. These guidelines can be user or agent provided.

## File structure

This section uses the project structure above.

### Palettes

Here's an example of a palette, that would be used for a set of simple wood furniture: 

```python
# voxels/palettes/furniture.py

from voxels import *

palette = Palette()
wood_light = palette.add_material(Color(133, 94, 66), roughness=0.65)
wood_dark = palette.add_material(Color(92, 61, 40), roughness=0.65)
metal = palette.add_material(Color(180, 180, 190), roughness=0.25, metallic=1.0)
cushion = palette.add_material(Color(150, 30, 30), roughness=0.9)
```

Make sure to bind each material to a variable.

### Models

Here's an example of a model, a simple chair making use of the previously defined palette:

```python
# voxels/models/furniture/chair.py

from voxels import *

from ...palettes.furniture import *

model = Model(Dimensions(16, 16, 16), palette, Pivot.Corner)

# Front legs: floor up to the underside of the seat.
model.aabb(wood_light, (3, 0, 3), (4, 5, 4))
model.aabb(wood_light, (11, 0, 3), (12, 5, 4))

# Back legs: run the full height, continuing above the seat as the backrest posts.
model.aabb(wood_light, (3, 0, 9), (4, 15, 10))
model.aabb(wood_light, (11, 0, 9), (12, 15, 10))

# Stretchers bracing the legs.
model.aabb(wood_light, (5, 2, 3), (10, 3, 4))
model.aabb(wood_light, (5, 2, 9), (10, 3, 10))
model.aabb(wood_light, (3, 2, 5), (4, 3, 8))
model.aabb(wood_light, (11, 2, 5), (12, 3, 8))

# Seat slab.
model.aabb(wood_dark, (3, 6, 3), (12, 7, 10))

# Seat cushion pad, inset from the front and side edges.
model.aabb(cushion, (4, 8, 4), (11, 8, 8))

# Backrest panel between the back posts.
model.aabb(wood_dark, (5, 9, 9), (10, 14, 10))

# Rivet accents where the legs meet the seat.
model.put(metal, (3, 5, 3))
model.put(metal, (12, 5, 3))
model.put(metal, (3, 6, 10))
model.put(metal, (12, 6, 10))

# Rev 1: the seat slab extended beyond the backrest
```

Be sure to label each logical block with what they are supposed to code for. You may use control flow constructs (loops, if statements, lists...) to help if needed. Add a short, numbered comment for each revision the user asks you to do on that model.

### Scenes

Here is a minimal example for a scene:

```python
# voxels/scenes/furniture.py

from voxels import *

from ..models.furniture import chair

scene = Scene()
root = scene.create_root_node("root")
root.add_model("chair", chair.model)

scene.export("assets/glb/furniture.glb")
```

If the user asks for it, you can directly put the call to `export` in this file. Here, it is saving the scene as a `.glb` file in their assets folder.

## Previewing

Once you think you are done with a model or a scene, you must render it to `.png`s to preview it. You can write an inline script for this purpose, using `python -c`, `print`ing the resulting `RenderOutput` object and then reading the `.png`s.
