import subprocess
import sys

from models.test_model import scene

scene.export_glb(".local/models/test.glb")

subprocess.run(
    [
        sys.executable,
        "-m", "voxels.preview",
        ".local/models/test.glb",
        "--angle", "0,0",
        "--angle", "90,30,1.5",
        "--time", "0", "--time", "1", "--time", "2",
        "--anim", "wiggle",
    ],
    check=True,
)
