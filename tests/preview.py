import voxels
from models.chair import scene

scene.export_glb(".local/models/chair.glb")

voxels.main([
    ".local/models/chair.glb",
    "--angle", "0,0",
    "--angle", "90,30,1.5",
])
