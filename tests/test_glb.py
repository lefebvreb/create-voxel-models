import json
import struct
import tempfile
from pathlib import Path

from voxels import Interpolation, Model, Palette, Scene, Vec3

palette = Palette()
red = palette.add_color((255, 0, 0), emissive=2.0)
glass = palette.add_color((0, 128, 255), ior=1.33, transmission=0.8)

model = Model((2, 2, 2), palette)
model.put((0, 0, 0), red)
model.put((1, 0, 0), glass)

scene = Scene()
root = scene.create_root_node("root")
child = root.create_child_node("child", translation=Vec3(1, 2, 3))
child.add_model("cube", model)

anim = scene.create_anim("wiggle")
anim.add_translation(child, [0.0, 1.0], [Vec3.ZERO, Vec3(0, 1, 0)], interpolation=Interpolation.Step)

with tempfile.TemporaryDirectory() as tmp:
    path = Path(tmp) / "scene.glb"
    scene.export_glb(path)

    data = path.read_bytes()
    assert data[0:4] == b"glTF", "GLB magic header missing"
    version, total_len = struct.unpack_from("<II", data, 4)
    assert version == 2
    assert total_len == len(data)

    json_chunk_len, json_chunk_type = struct.unpack_from("<I4s", data, 12)
    assert json_chunk_type == b"JSON"
    json_bytes = data[20 : 20 + json_chunk_len]
    root_json = json.loads(json_bytes)

    for key in ("asset", "scenes", "nodes", "meshes", "materials", "accessors", "bufferViews", "animations"):
        assert key in root_json, f"missing top-level key: {key}"

    assert root_json["asset"]["version"] == "2.0"
    assert len(root_json["nodes"]) == 2
    assert len(root_json["meshes"]) == 1
    assert len(root_json["animations"]) == 1
    assert "KHR_materials_transmission" in root_json["extensionsUsed"]
    assert "KHR_materials_ior" in root_json["extensionsUsed"]
    assert "KHR_materials_emissive_strength" in root_json["extensionsUsed"]

    bin_chunk_offset = 20 + json_chunk_len
    bin_chunk_len, bin_chunk_type = struct.unpack_from("<I4s", data, bin_chunk_offset)
    assert bin_chunk_type == b"BIN\0"
    assert bin_chunk_len > 0

print("glb export ok")
