class Color:
    @property
    def rgb(self) -> tuple[int, int, int]: ...
    @property
    def roughness(self) -> float: ...
    @property
    def metallic(self) -> float: ...
    @property
    def ior(self) -> float: ...
    @property
    def transmission(self) -> float: ...
    @property
    def emissive(self) -> float: ...

class Palette:
    def add_color(
        self,
        rgb: tuple[int, int, int],
        *,
        roughness: float = 1.0,
        metallic: float = 0.0,
        ior: float = 1.5,
        transmission: float = 0.0,
        emissive: float = 0.0,
    ) -> Color: ...

class Scene:
    def __init__(self) -> None: ...
