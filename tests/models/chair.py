from voxels import Color, Dimensions, Model, Palette, Pivot, Scene

palette = Palette()
wood_light = palette.add_material(Color(133, 94, 66), roughness=0.65)
wood_dark = palette.add_material(Color(92, 61, 40), roughness=0.65)
metal = palette.add_material(Color(180, 180, 190), roughness=0.25, metallic=1.0)
cushion = palette.add_material(Color(150, 30, 30), roughness=0.9)

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

scene = Scene()
root = scene.create_root_node("root")
root.add_model("chair", model)

scene.export_glb(".local/models/chair.glb")
