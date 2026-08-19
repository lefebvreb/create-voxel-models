from voxels import Color, Dimensions, Interpolation, Model, Palette, Pivot, Quat, Scene, Volume

palette = Palette()
red = palette.add_material(Color(255, 0, 0), emissive=10.0)
glass = palette.add_material(Color(0, 128, 255), ior=1.0, transmission=0.95, roughness=0.05, volume=Volume(Color(0, 128, 255), 2.0))
gold = palette.add_material(Color(239, 191, 4), roughness=0.0, metallic=1.0)

model = Model(Dimensions(3, 1, 1), palette, Pivot.Center)
model.put(red, (0, 0, 0))
model.put(glass, (1, 0, 0))
model.put(gold, (2, 0, 0))

scene = Scene()
root = scene.create_root_node("root")
controller = root.create_child_node("controller")
controller.add_model("test_model", model)

anim = scene.create_anim("wiggle")
anim.add_rotation(controller, [0.0, 1.0, 2.0], [Quat.IDENTITY, Quat.from_rotation_y(180), Quat.IDENTITY], interpolation=Interpolation.Linear)
