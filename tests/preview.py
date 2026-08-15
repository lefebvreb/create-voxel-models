from pathlib import Path

from voxels import CameraAngle, Interpolation, Model, Palette, Quat, Scene, Vec3, Volume

# --- Create Scene ---

palette = Palette()
red = palette.add_color((255, 0, 0), emissive=2.0)
glass = palette.add_color(
    (0, 128, 255), ior=1.0, transmission=0.95, roughness=0.05, volume=Volume((0, 128, 255), 2.0)
)
gold = palette.add_color((239, 191, 4), roughness=0.0, metallic=1.0)

model = Model((3, 1, 1), palette)
model.put(red, (0, 0, 0))
model.put(glass, (1, 0, 0))
model.put(gold, (2, 0, 0))

scene = Scene()
root = scene.create_root_node("root")
child = root.create_child_node("child")
grandchild = child.create_child_node("grandchild", translation=Vec3(-1.5, -0.5, -0.5))
grandchild.add_model("cube", model)

anim = scene.create_anim("wiggle")
anim.add_rotation(child, [0.0, 1.0, 2.0], [Quat.IDENTITY, Quat.from_rotation_y(180), Quat.IDENTITY], interpolation=Interpolation.Linear)

# --- Render ---

scene.export_glb(".local/models/scene.glb")

angles = [CameraAngle(0.0, 0.0), CameraAngle(90.0, 30.0, zoom=1.5)]
times = [0.0, 1.0, 2.0]
out = scene.render(angles, times=times, animation="wiggle")
for f in out.files:
    print(Path(out.dir) / f)
