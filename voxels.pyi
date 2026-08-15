from collections.abc import Sequence
from os import PathLike
from pathlib import Path
from typing import Final, final

@final
class Anim:
    def add_rotation(self, /, node: Node, input: Sequence[float], output: Sequence[Quat], *, interpolation: Interpolation |None = None) -> None: ...
    def add_scale(self, /, node: Node, input: Sequence[float], output: Sequence[Vec3], *, interpolation: Interpolation |None = None) -> None: ...
    def add_translation(self, /, node: Node, input: Sequence[float], output: Sequence[Vec3], *, interpolation: Interpolation |None = None) -> None: ...
    @property
    def extras(self, /) -> dict[str, str] |None: ...
    @property
    def name(self, /) -> str: ...
    @property
    def scene(self, /) -> Scene: ...

@final
class CameraAngle:
    def __new__(cls, /, yaw: float, pitch: float, *, zoom: float |None = None) -> CameraAngle: ...
    @property
    def pitch(self, /) -> float: ...
    @property
    def yaw(self, /) -> float: ...
    @property
    def zoom(self, /) -> float |None: ...

@final
class Color:
    @property
    def emissive(self, /) -> float: ...
    @property
    def ior(self, /) -> float: ...
    @property
    def metallic(self, /) -> float: ...
    @property
    def palette(self, /) -> Palette: ...
    @property
    def rgb(self, /) -> tuple[int, int, int]: ...
    @property
    def roughness(self, /) -> float: ...
    @property
    def transmission(self, /) -> float: ...
    @property
    def volume(self, /) -> Volume |None: ...

@final
class Interpolation:
    CubicSpline: Final[Interpolation]
    Linear: Final[Interpolation]
    Step: Final[Interpolation]
    def __int__(self, /) -> int: ...
    def __repr__(self, /) -> str: ...

@final
class Mesh:
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
    def __new__(cls, /, dimensions: tuple[int, int, int], palette: Palette) -> Model: ...
    def aabb(self, /, color: Color |None, a: tuple[int, int, int], b: tuple[int, int, int]) -> None: ...
    def copy(self, /) -> Model: ...
    @property
    def dimensions(self, /) -> tuple[int, int, int]: ...
    @property
    def palette(self, /) -> Palette: ...
    def put(self, /, color: Color |None, a: tuple[int, int, int]) -> None: ...
    def render(self, /, angles: Sequence[CameraAngle], *, background: tuple[int, int, int] |None = None, output_dir: str |PathLike[str] |None = None) -> RenderOutput: ...

@final
class Node:
    def add_model(self, /, name: str, model: Model, *, extras: dict[str, str] |None = None) -> Mesh: ...
    def create_child_node(self, /, name: str, *, translation: Vec3 |None = None, rotation: Quat |None = None, scale: Vec3 |None = None, extras: dict[str, str] |None = None) -> Node: ...
    @property
    def extras(self, /) -> dict[str, str] |None: ...
    @property
    def name(self, /) -> str: ...
    @property
    def parent(self, /) -> Node |None: ...
    @property
    def scene(self, /) -> Scene: ...

@final
class Palette:
    def __len__(self, /) -> int: ...
    def __new__(cls, /) -> Palette: ...
    def add_color(self, /, rgb: tuple[int, int, int], *, roughness: float = 1.0, metallic: float = 0.0, ior: float = 1.5, transmission: float = 0.0, emissive: float = 0.0, volume: Volume |None = None) -> Color: ...

@final
class Quat:
    """
    A quaternion, with double precisions components.
    """
    IDENTITY: Final[Quat]
    """
    The identity quaternion, equivalent to no rotation.
    """
    def __mul__(self, other: object, /) -> Quat:
        """
        Quaternion product, `other` must be a `Quat`.
        """
    def conjugate(self, /) -> Quat: ...
    @staticmethod
    def from_axis_angle(axis: Vec3, angle: float) -> Quat:
        """
        Creates a quaternion that rotates `angle` degrees around `axis`.
        `axis` does not need to be normalized.
        """
    @staticmethod
    def from_rotation_x(angle: float) -> Quat:
        """
        Creates a quaternion that rotates `angle` degrees around the x axis.
        """
    @staticmethod
    def from_rotation_y(angle: float) -> Quat:
        """
        Creates a quaternion that rotates `angle` degrees around the y axis.
        """
    @staticmethod
    def from_rotation_z(angle: float) -> Quat:
        """
        Creates a quaternion that rotates `angle` degrees around the z axis.
        """

@final
class RenderOutput:
    @property
    def dir(self, /) -> Path: ...
    @property
    def files(self, /) -> list[Path]: ...

@final
class Scene:
    def __new__(cls, /) -> Scene: ...
    def create_anim(self, /, name: str, *, extras: dict[str, str] |None = None) -> Anim: ...
    def create_root_node(self, /, name: str, *, extras: dict[str, str] |None = None) -> Node: ...
    def export_glb(self, /, path: str |PathLike[str]) -> None: ...
    def render(self, /, angles: Sequence[CameraAngle], *, times: Sequence[float] = ..., animation: str |None = None, include: Sequence[str] |None = None, exclude: Sequence[str] |None = None, background: tuple[int, int, int] |None = None, output_dir: str |PathLike[str] |None = None) -> RenderOutput: ...

@final
class Vec3:
    """
    A 3-dimensional real column vector, with double precision components.
    """
    ONES: Final[Vec3]
    """
    A vector that only contains ones.
    """
    ZERO: Final[Vec3]
    """
    A vector that only contains zeros.
    """
    def __add__(self, other: object, /) -> Vec3:
        """
        Component-wise addition, `other` must be a `Vec3`.
        """
    def __mul__(self, other: object, /) -> Vec3:
        """
        Component-wise multiplication when `other` is a `Vec3`.
        Otherwise just scales `self` by `other`, which is then required to be a `float`.
        """
    def __neg__(self, /) -> Vec3: ...
    def __new__(cls, /, x: float, y: float, z: float) -> Vec3: ...
    def __sub__(self, other: object, /) -> Vec3:
        """
        Component-wise subtraction, `other` must be a `Vec3`.
        """
    def __truediv__(self, other: object, /) -> Vec3:
        """
        Scales `self` by `other⁻¹`, which is required to be a `float`.
        """
    @staticmethod
    def splat(t: float) -> Vec3: ...
    @property
    def x(self, /) -> float: ...
    @property
    def y(self, /) -> float: ...
    @property
    def z(self, /) -> float: ...

@final
class Volume:
    """
    How a transmissive `Color` attenuates (tints/absorbs) light travelling through its volume,
    via `KHR_materials_volume`'s `attenuationColor`/`attenuationDistance`. Only meaningful
    alongside a non-zero `transmission`.
    """
    def __new__(cls, /, color: tuple[int, int, int], distance: float) -> Volume: ...
    @property
    def color(self, /) -> tuple[int, int, int]: ...
    @property
    def distance(self, /) -> float: ...
