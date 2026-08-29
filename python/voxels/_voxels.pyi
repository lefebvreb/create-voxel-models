from collections.abc import Sequence
from os import PathLike
from typing import Final, final

@final
class Anim:
    """
    A named animation, holding per-node keyframe tracks for translation, rotation and scale.
    """
    def add_rotation(self, /, node: Node, input: Sequence[float], output: Sequence[Quat], *, interpolation: Interpolation |None = None) -> None:
        """
        Set `node`'s rotation track, replacing any existing one.
        
        Args:
            node: The node to animate; it must belong to this animation's scene.
            input: Keyframe times, in seconds, in ascending order.
            output: The rotation at each keyframe — or, with cubic-spline interpolation, an
                in-tangent, value and out-tangent for each.
            interpolation: How values between keyframes are interpolated; linear when omitted.
        
        Raises:
            ValueError: If `node` is from another scene, or `output` has the wrong length.
        """
    def add_scale(self, /, node: Node, input: Sequence[float], output: Sequence[Vec3], *, interpolation: Interpolation |None = None) -> None:
        """
        Set `node`'s scale track, replacing any existing one.
        
        Args:
            node: The node to animate; it must belong to this animation's scene.
            input: Keyframe times, in seconds, in ascending order.
            output: The per-axis scale at each keyframe — or, with cubic-spline interpolation,
                an in-tangent, value and out-tangent for each.
            interpolation: How values between keyframes are interpolated; linear when omitted.
        
        Raises:
            ValueError: If `node` is from another scene, or `output` has the wrong length.
        """
    def add_translation(self, /, node: Node, input: Sequence[float], output: Sequence[Vec3], *, interpolation: Interpolation |None = None) -> None:
        """
        Set `node`'s translation track, replacing any existing one.
        
        Args:
            node: The node to animate; it must belong to this animation's scene.
            input: Keyframe times, in seconds, in ascending order.
            output: The translation at each keyframe — or, with cubic-spline interpolation, an
                in-tangent, value and out-tangent for each.
            interpolation: How values between keyframes are interpolated; linear when omitted.
        
        Raises:
            ValueError: If `node` is from another scene, or `output` has the wrong length.
        """
    @property
    def extras(self, /) -> dict[str, str] |None: ...
    @property
    def name(self, /) -> str: ...
    @property
    def scene(self, /) -> Scene: ...

@final
class Color:
    """
    An RGB color with 8 bits per channel.
    """
    def __new__(cls, /, r: int, g: int, b: int) -> Color:
        """
        Create a color from its red, green and blue channels, each from 0 to 255.
        """
    @property
    def b(self, /) -> int: ...
    @property
    def g(self, /) -> int: ...
    @property
    def r(self, /) -> int: ...

@final
class Dimensions:
    """
    The size of a model's voxel grid along each axis.
    """
    def __new__(cls, /, x: int, y: int, z: int) -> Dimensions:
        """
        Create dimensions of `x` by `y` by `z` voxels, each from 1 to 256.
        """
    def as_vec(self, /) -> Vec3:
        """
        Return these dimensions as a `Vec3`.
        """
    def contains(self, /, a: tuple[int, int, int]) -> bool:
        """
        Return whether `a` is a valid voxel coordinate within these dimensions.
        """
    @property
    def x(self, /) -> int: ...
    @property
    def y(self, /) -> int: ...
    @property
    def z(self, /) -> int: ...

@final
class Interpolation:
    """
    How an animation track interpolates between keyframes.
    
    `Linear` blends between neighboring keyframes (spherically, for rotations); `Step` holds
    each keyframe until the next; `CubicSpline` uses Hermite splines and needs three output
    values per keyframe.
    """
    CubicSpline: Final[Interpolation]
    Linear: Final[Interpolation]
    Step: Final[Interpolation]
    def __int__(self, /) -> int: ...
    def __repr__(self, /) -> str: ...

@final
class Material:
    """
    A PBR material held by a `Palette`. Create one with `Palette.add_material`.
    """
    @property
    def color(self, /) -> Color: ...
    @property
    def emissive(self, /) -> float: ...
    @property
    def ior(self, /) -> float: ...
    @property
    def metallic(self, /) -> float: ...
    @property
    def palette(self, /) -> Palette: ...
    @property
    def roughness(self, /) -> float: ...
    @property
    def transmission(self, /) -> float: ...
    @property
    def volume(self, /) -> Volume |None: ...

@final
class Mesh:
    """
    A model attached to a node, created by `Node.add_model`.
    """
    @property
    def extras(self, /) -> dict[str, str] |None: ...
    @property
    def model(self, /) -> Model: ...
    @property
    def name(self, /) -> str: ...
    @property
    def parent(self, /) -> Node: ...

@final
class Model:
    """
    A 3D grid of voxels, each empty or set to one material of the model's palette.
    
    Voxel coordinates are `(x, y, z)` tuples indexing from `(0, 0, 0)`; y is up. Coordinates
    outside the grid raise `ValueError`, though spheres and ellipsoids reaching past an edge
    are clipped to it.
    """
    def __new__(cls, /, dims: Dimensions, palette: Palette, pivot: Pivot |Vec3) -> Model:
        """
        Create an empty model.
        
        Args:
            dims: The grid's size along each axis.
            palette: The palette whose materials this model's voxels may use.
            pivot: Which point of the grid to place at the node's origin — a `Pivot`, or a
                `Vec3` in grid coordinates (the same voxel space as `put` and `aabb`).
        """
    def aabb(self, /, material: Material |None, p1: tuple[int, int, int], p2: tuple[int, int, int]) -> None:
        """
        Fill the axis-aligned box between corners `p1` and `p2` inclusive.
        
        A `material` of `None` clears the box instead.
        """
    def clip(self, /, p1: tuple[int, int, int], p2: tuple[int, int, int]) -> Model:
        """
        Return a new model holding just the voxels in the box between `p1` and `p2` inclusive.
        
        The new model keeps this model's palette and pivot.
        """
    def copy(self, /) -> Model:
        """
        Return an independent copy of the model that shares the same palette.
        """
    @property
    def dims(self, /) -> Dimensions: ...
    def ellipsoid(self, /, material: Material |None, c: tuple[int, int, int], rx: int, ry: int, rz: int) -> None:
        """
        Fill an ellipsoid centered on `c`, with radii `rx`, `ry` and `rz` along each axis.
        
        A `material` of `None` clears the ellipsoid instead.
        """
    def flip_x(self, /) -> None:
        """
        Mirror the model in place across the x axis.
        """
    def flip_y(self, /) -> None:
        """
        Mirror the model in place across the y axis.
        """
    def flip_z(self, /) -> None:
        """
        Mirror the model in place across the z axis.
        """
    def include(self, /, other: Model, offset: tuple[int, int, int]) -> None:
        """
        Stamp every set voxel of `other` into this model, shifted by `offset`.
        
        Both models must share the same palette. Empty voxels in `other` leave this model
        unchanged; `other` must fit entirely within bounds once shifted.
        """
    @property
    def palette(self, /) -> Palette: ...
    @property
    def pivot(self, /) -> Pivot |Vec3: ...
    def put(self, /, material: Material |None, p: tuple[int, int, int]) -> None:
        """
        Set the voxel at `p`, or clear it when `material` is `None`.
        """
    def sphere(self, /, material: Material |None, c: tuple[int, int, int], r: int) -> None:
        """
        Fill a sphere of radius `r`, in voxels, centered on `c`.
        
        A `material` of `None` clears the sphere instead.
        """
    def spheroid(self, /, material: Material |None, c: tuple[int, int, int], r_eq: int, r_polar: int) -> None:
        """
        Fill a spheroid centered on `c`, with horizontal radius `r_eq` and vertical radius `r_polar`.
        
        A `material` of `None` clears the spheroid instead.
        """

@final
class Node:
    """
    A transform node in a scene's tree.
    
    A node's translation, rotation and scale are relative to its parent and apply to its
    descendants and any attached meshes. Each is `None` when left at its default.
    """
    def add_model(self, /, name: str, model: Model, *, extras: dict[str, str] |None = None) -> Mesh:
        """
        Attach `model` to this node as a named mesh and return it.
        
        Args:
            name: A scene-unique mesh name.
            model: The model to attach.
            extras: Arbitrary string key/value pairs attached to the mesh in glTF exports.
        """
    def create_child_node(self, /, name: str, *, translation: Vec3 |None = None, rotation: Quat |None = None, scale: Vec3 |None = None, extras: dict[str, str] |None = None) -> Node:
        """
        Create a child of this node.
        
        Args:
            name: A scene-unique name for the child.
            translation: Offset from the parent, in voxels.
            rotation: Rotation relative to the parent.
            scale: Per-axis scale relative to the parent.
            extras: Arbitrary string key/value pairs attached to the node in glTF exports.
        """
    @property
    def extras(self, /) -> dict[str, str] |None: ...
    @property
    def name(self, /) -> str: ...
    @property
    def parent(self, /) -> Node |None: ...
    @property
    def rotation(self, /) -> Quat |None: ...
    @property
    def scale(self, /) -> Vec3 |None: ...
    @property
    def scene(self, /) -> Scene: ...
    @property
    def translation(self, /) -> Vec3 |None: ...

@final
class Palette:
    """
    An ordered set of up to 255 materials, shared between the models that use them.
    """
    def __len__(self, /) -> int:
        """
        Return the number of materials in the palette.
        """
    def __new__(cls, /) -> Palette:
        """
        Create an empty palette.
        """
    def add_material(self, /, color: Color, *, roughness: float = 1.0, metallic: float = 0.0, ior: float = 1.5, transmission: float = 0.0, emissive: float = 0.0, volume: Volume |None = None) -> Material:
        """
        Add a material to the palette and return it.
        
        Args:
            color: The material's base color.
            roughness: Surface roughness, from 0.0 (mirror-smooth) to 1.0 (fully matte).
            metallic: 0.0 for a dielectric, 1.0 for a metal.
            ior: Index of refraction; either 0.0 or at least 1.0. Only affects transmissive
                materials.
            transmission: Fraction of light passing through the surface, from 0.0 (opaque) to
                1.0 (fully transmissive).
            emissive: Emitted light strength; 0.0 for a non-emitter, larger values glow brighter.
            volume: Volumetric attenuation applied to transmitted light, if any.
        
        Raises:
            ValueError: If an argument is out of range, or the palette already holds 255 materials.
        """

@final
class Pivot:
    """
    Which point of a model's voxel grid to place at its node's origin once in a scene.
    
    `Corner` uses the grid's `(0, 0, 0)` corner; `Center` uses its center; `BottomCenter` uses
    the center of its base. For an arbitrary point, pass a `Vec3` in grid coordinates (voxels,
    one per unit) instead of a `Pivot`.
    """
    BottomCenter: Final[Pivot]
    Center: Final[Pivot]
    Corner: Final[Pivot]
    def __int__(self, /) -> int: ...
    def __repr__(self, /) -> str: ...

@final
class Quat:
    """
    A unit quaternion with double-precision components, representing a 3D rotation.
    """
    IDENTITY: Final[Quat]
    """
    The identity quaternion, representing no rotation.
    """
    def __mul__(self, other: object, /) -> Quat:
        """
        Compose `self` with `other`, another `Quat`, applying `other` first.
        """
    def conjugate(self, /) -> Quat:
        """
        Return the conjugate of `self`, which is the inverse rotation.
        """
    @staticmethod
    def from_axis_angle(axis: Vec3, angle: float) -> Quat:
        """
        Create a quaternion rotating `angle` degrees about `axis`.
        
        `axis` need not be normalized.
        """
    @staticmethod
    def from_rotation_x(angle: float) -> Quat:
        """
        Create a quaternion rotating `angle` degrees about the x axis.
        """
    @staticmethod
    def from_rotation_y(angle: float) -> Quat:
        """
        Create a quaternion rotating `angle` degrees about the y axis.
        """
    @staticmethod
    def from_rotation_z(angle: float) -> Quat:
        """
        Create a quaternion rotating `angle` degrees about the z axis.
        """

@final
class Scene:
    """
    A tree of nodes, the models attached to them, and any named animations.
    
    Node, mesh and animation names must each be unique within the scene.
    """
    def __new__(cls, /) -> Scene:
        """
        Create an empty scene.
        """
    def create_anim(self, /, name: str, *, extras: dict[str, str] |None = None) -> Anim:
        """
        Create an empty animation.
        
        Args:
            name: A scene-unique name for the animation.
            extras: Arbitrary string key/value pairs attached to the animation in glTF exports.
        """
    def create_root_node(self, /, name: str, *, extras: dict[str, str] |None = None) -> Node:
        """
        Create a top-level node.
        
        Args:
            name: A scene-unique name, also used for the node in exports.
            extras: Arbitrary string key/value pairs attached to the node in glTF exports.
        """
    def export_glb(self, /, path: str |PathLike[str]) -> None:
        """
        Export the scene to a binary glTF (`.glb`) file at `path`.
        """

@final
class Vec3:
    """
    A 3D vector with double-precision components.
    """
    ONES: Final[Vec3]
    """
    The vector whose components are all one.
    """
    ZERO: Final[Vec3]
    """
    The zero vector.
    """
    def __add__(self, other: object, /) -> Vec3:
        """
        Add `self` and `other`, another `Vec3`, component-wise.
        """
    def __mul__(self, other: object, /) -> Vec3:
        """
        Scale `self` by the scalar `other`.
        """
    def __neg__(self, /) -> Vec3:
        """
        Negate each component.
        """
    def __new__(cls, /, x: float, y: float, z: float) -> Vec3: ...
    def __sub__(self, other: object, /) -> Vec3:
        """
        Subtract `other`, another `Vec3`, from `self` component-wise.
        """
    def __truediv__(self, other: object, /) -> Vec3:
        """
        Divide `self` by the scalar `other`.
        """
    @staticmethod
    def splat(t: float) -> Vec3:
        """
        Create a vector with every component equal to `t`.
        """
    @property
    def x(self, /) -> float: ...
    @property
    def y(self, /) -> float: ...
    @property
    def z(self, /) -> float: ...

@final
class Volume:
    """
    Volumetric light attenuation for a transmissive material, giving colored glass its tint.
    """
    def __new__(cls, /, color: Color, distance: float, *, thickness: float = 1.0) -> Volume:
        """
        Define how transmitted light is absorbed inside a material.
        
        Args:
            color: The color light is attenuated toward as it travels through the volume.
            distance: Distance, in voxels, over which light is attenuated to `color`. Must be
                positive.
            thickness: Thickness, in voxels, of the material's walls.
        """
    @property
    def color(self, /) -> Color: ...
    @property
    def distance(self, /) -> float: ...
    @property
    def thickness(self, /) -> float: ...

def _preview() -> None:
    """
    Entry point for `python -m voxels.preview`.
    """
