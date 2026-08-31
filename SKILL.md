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

This skill contains a python package that enables programmatic creation, edition and animation of voxel models, organized in hierarchical scenes. Similar to how the glTF format is structured, but with voxels. Voxels have colors and PBR properties, with up to 255 different materials per palettes, one palette per model.

## Installation

The `voxels` package must be importable before you do anything else. A wheel is bundled with this skill in the `dist/` directory.

- **On a user's machine:** install into a virtual environment. Ask the user for their preferred package manager and whether they want a venv or a global install, then e.g. `python -m venv .venv && .venv/bin/pip install dist/*.whl`.
- **In a sandbox:** a global `pip install dist/*.whl` is fine. Also follow any install instructions in your prompts.

Verify with `python -c "import voxels"`.

## Package Overview

There are a few major concepts:
* `Palette`s contain `Material`s, which are a `Color` and PBR properties (such as `metallic`, `transmission`...).
* `Model`s are a 3D array of voxels, they reference one `Palette`, and offer an API to edit them through primitive shapes such as boxes and spheres.
* `Scene`s are a tree of `Node`s. `Model`s can be attached to `Node`s. `Scene`s can be animated through the `Anim` API, by attaching translations, rotations and scalings (TRS) to `Node`s.

Finally, `Scene`s can be exported to `.glb` files (standard glTF 2.0, which loads into Blender, three.js, Godot, Bevy and most engines), and they can be previewed to `.png` under different angles and at different times of an animation to get immediate feedback.

`references/voxels.pyi` bundled with this skill is the authoritative API reference — every class, method and argument is documented there. Read it before writing code; this file only covers workflow and conventions.

One thing to fix in your head up front: coordinates are `(x, y, z)` and **`y` is up**. One voxel is one glTF unit, so node translations are in voxels too. Per the glTF convention a model's **front faces `+z`** — build models facing that way so scenes can assume it, and the preview camera looks straight at the front at `--angle 0,0` (see Previewing).

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
└── STYLE.md
```

`STYLE.md` holds **high-level authoring guidance only** — the decisions that keep a set of models feeling like one set, which nothing else in the project records. For example: the world scale (how many voxels to a metre), shared proportions, a common colour language, recurring motifs and how to build them, lighting/orientation conventions, naming schemes. Entries can be user- or agent-provided.

Keep out of it anything already recorded elsewhere: the API (that's `references/voxels.pyi`), what materials a palette defines or what each one is for (read `palettes/*.py`), the list of existing models or their geometry (read `models/`), or how a specific model is built (its own file, with its revision comments). If you catch yourself restating code, delete it — a stale copy is worse than none.

> Good: "Chairs and tables share a 16-voxel seat/table height so they read as a set." / "Metal accents only ever appear as single-voxel rivets, never as surfaces."
> Bad: "The furniture palette has wood_light, wood_dark, metal and cushion." / "chair.py builds a chair with four legs and a backrest."

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

A `Model` takes a grid size, a palette and a pivot (see the `Pivot` docstring). Prefer `Pivot.BottomCenter` for anything that rests on a floor, so a node's translation reads as "where the object stands".

Here's an example of a model, a simple chair making use of the previously defined palette:

```python
# voxels/models/furniture/chair.py

from voxels import *

from palettes.furniture import *

model = Model(Dimensions(16, 16, 16), palette, Pivot.BottomCenter)

# Front legs (+z, the side you face): floor up to the underside of the seat.
model.aabb(wood_light, (3, 0, 11), (4, 5, 12))
model.aabb(wood_light, (11, 0, 11), (12, 5, 12))

# Back legs (-z): run the full height, continuing above the seat as the backrest posts.
model.aabb(wood_light, (3, 0, 5), (4, 15, 6))
model.aabb(wood_light, (11, 0, 5), (12, 15, 6))

# Stretchers bracing the legs.
model.aabb(wood_light, (5, 2, 11), (10, 3, 12))
model.aabb(wood_light, (5, 2, 5), (10, 3, 6))
model.aabb(wood_light, (3, 2, 7), (4, 3, 10))
model.aabb(wood_light, (11, 2, 7), (12, 3, 10))

# Seat slab.
model.aabb(wood_dark, (3, 6, 5), (12, 7, 12))

# Seat cushion pad, inset from the front and side edges.
model.aabb(cushion, (4, 8, 7), (11, 8, 11))

# Backrest panel between the back posts.
model.aabb(wood_dark, (5, 9, 5), (10, 14, 6))

# Rivet accents where the legs meet the seat.
model.put(metal, (3, 5, 12))
model.put(metal, (12, 5, 12))
model.put(metal, (3, 6, 5))
model.put(metal, (12, 6, 5))
```

Be sure to label each logical block with what they are supposed to code for. You may use control flow constructs (loops, if statements, lists...) to help if needed. Add a short, numbered comment for each revision the user asks you to do on that model, for example:

```python
# Rev 1: the seat slab extended beyond the backrest
```

**Symmetry.** Rather than repeating mirrored geometry by hand, build one side, then `copy` / `flip_*` / `include` it back in. See the `Model` docstrings for `copy`, `flip_x`, `include` and `clip`.

**Checking geometry in code.** `Model.get((x, y, z))` returns the material at a voxel (or `None`), `Model.filled` counts set voxels, and `Model.occupied_bounds()` gives the tight `(min, max)` box the model actually occupies — use these to assert placement and extent instead of only eyeballing renders.

### Scenes

A scene imports the models it needs, arranges them on a tree of nodes, optionally animates some nodes, and exports. Node transforms (`translation` in voxels, `rotation` as a `Quat`, `scale` as a `Vec3`) apply to a node's whole subtree, so a model gets its own transform by living on its own child node. Node, mesh and animation names must each be unique within the scene.

```python
# voxels/scenes/furniture.py

from voxels import *

from models.furniture import chair

scene = Scene()
root = scene.create_root_node("root")
controller = root.create_child_node("spinner")

controller.add_model("chair", chair.model)

spin = scene.create_anim("spin")
spin.add_rotation(controller, [0.0, 4.0], [Quat.IDENTITY, Quat.from_rotation_y(360)])
```

`add_translation` and `add_scale` follow the same shape as `add_rotation`, with `Vec3` outputs.

The preview CLI exports the scene itself (see Previewing), so a scene file needs no `export_glb` call. Add one — `scene.export_glb("assets/glb/furniture.glb")` at the end — only if the user wants the `.glb` written as a deliverable.

## Previewing

Once you think a model or scene is done, you **must** render it to `.png` and look at the result, then adjust and re-render. Repeat until it reads correctly from every angle — this loop is the core of the workflow, not an afterthought.

Rendering is CLI-only — `python -m voxels.preview TARGET`, a pure-CPU rasterizer, no GPU required. `TARGET` is either a `.glb` file or a **`.py` file** that defines a module-level `scene` (or a lone `model`, which the CLI wraps in a one-node scene and renders on its own — the fastest way to eyeball a single model). Each `--angle` is `yaw,pitch` in degrees, with an optional third `zoom` factor (`45,25,1.5` frames tighter, a value below `1` pulls back). `yaw 0` faces the model's front, `yaw 180` its back, and the key light rakes the front, so `yaw` 315 and 45 are the two well-lit front three-quarter views; a sensible default coverage set is:

```sh
python -m voxels.preview scenes/furniture.py --angle 315,25 --angle 45,25 --angle 180,40
```

**Run it from the project directory** (the one holding `models/`, `palettes/`, `scenes/`) — the `.py` target's own imports resolve against the working directory, and a `.glb` path passed to `export_glb` or the CLI is relative to it too. Run `python -m voxels.preview --help` for the full flag list.

This prints the written PNG paths, one per line — read those. Other flags: `--anim NAME` poses an animated scene before rendering, and `--time T` (repeatable, seconds) picks which moments to sample — `--time` is ignored unless `--anim` is given; `--include NAME`/`--exclude NAME` (repeatable, matched against node or mesh names) to show only or hide parts of the scene; `--out DIR` to control where the PNGs land (a fresh temp directory by default).
