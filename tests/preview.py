from voxels import CameraAngle
from models.test_model import scene

angles = [CameraAngle(0.0, 0.0), CameraAngle(90.0, 30.0, zoom=1.5)]
times = [0.0, 1.0, 2.0]
out = scene.render(angles, times=times, animation="wiggle")
for f in out.files: print(f)
