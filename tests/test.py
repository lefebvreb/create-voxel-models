from voxels import Palette, Scene

p = Palette()
c = p.add_color((255, 0 ,0))
print(len(p))

s = Scene(p)
print(s)