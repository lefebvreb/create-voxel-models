import shutil
import struct
import tempfile
from pathlib import Path
from typing import Callable

from voxels import CameraAngle, Interpolation, Model, Palette, Quat, RenderOutput, Scene, Vec3

def assert_raises_value_error(f: Callable[[], RenderOutput]):
    try:
        f()
    except ValueError:
        return
    raise AssertionError("expected a ValueError")

palette = Palette()
red = palette.add_color((255, 0, 0), emissive=2.0)
glass = palette.add_color((0, 128, 255), ior=1.33, transmission=0.5)
gold = palette.add_color((239, 191, 4), roughness=0, metallic=1.0)

model = Model((3, 1, 1), palette)
model.put((0, 0, 0), red)
model.put((1, 0, 0), glass)
model.put((2, 0, 0), gold)

scene = Scene()
root = scene.create_root_node("root")
child = root.create_child_node("child")
grandchild = child.create_child_node("grandchild", translation=Vec3(-1.5, -0.5, -0.5))
grandchild.add_model("cube", model)

anim = scene.create_anim("wiggle")
anim.add_rotation(child, [0.0, 1.0, 2.0], [Quat.IDENTITY, Quat.from_rotation_y(180), Quat.IDENTITY], interpolation=Interpolation.Linear)

def assert_valid_png(path: Path, expected_size: int = 512):
    data = path.read_bytes()
    assert data[:8] == b"\x89PNG\r\n\x1a\n", f"not a PNG: {path}"
    width, height = struct.unpack_from(">II", data, 16)
    assert (width, height) == (expected_size, expected_size)

angles = [CameraAngle(0.0, 0.0), CameraAngle(90.0, 30.0, zoom=1.5)]

# --- Scene.render: static (no animation) ---
out = scene.render(angles)
try:
    assert len(out.files) == len(angles)
    for f in out.files:
        assert_valid_png(Path(out.dir) / f)
finally:
    shutil.rmtree(out.dir)

# --- Scene.render: named animation across multiple times ---
times = [0.0, 1.0, 2.0]
out = scene.render(angles, times=times, animation="wiggle")
try:
    assert len(out.files) == len(times) * len(angles)
    for f in out.files:
        assert_valid_png(Path(out.dir) / f)
finally:
    shutil.rmtree(out.dir)

# --- Scene.render: include/exclude filtering, explicit output_dir ---
with tempfile.TemporaryDirectory() as tmp:
    out = scene.render(angles[:1], exclude=["cube"], output_dir=tmp)
    assert str(out.dir) == tmp
    assert len(out.files) == 1
    assert_valid_png(Path(out.dir) / out.files[0])

# --- Model.render ---
out = model.render(angles)
try:
    assert len(out.files) == len(angles)
    for f in out.files:
        assert_valid_png(Path(out.dir) / f)
finally:
    shutil.rmtree(out.dir)

# --- Error cases ---
assert_raises_value_error(lambda: scene.render([]))
assert_raises_value_error(lambda: scene.render(angles, animation="does-not-exist"))

print("render ok")
