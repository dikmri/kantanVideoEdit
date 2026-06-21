from PIL import Image, ImageDraw
import math

SIZE = 1024
img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
d = ImageDraw.Draw(img)

# Rounded-square background gradient
def lerp(a, b, t):
    return tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(3))

c1 = (26, 27, 31)      # #1a1b1f
c2 = (79, 140, 255)    # #4f8cff

# draw gradient by stacking horizontal lines, masked by rounded rect
radius = 220
mask = Image.new("L", (SIZE, SIZE), 0)
md = ImageDraw.Draw(mask)
md.rounded_rectangle([0, 0, SIZE, SIZE], radius=radius, fill=255)

grad = Image.new("RGBA", (SIZE, SIZE), (0,0,0,0))
gd = ImageDraw.Draw(grad)
for y in range(SIZE):
    t = y / SIZE
    # diagonal gradient
    gd.line([(0, y), (SIZE, y)], fill=lerp(c2, c1, t) + (255,))

img.paste(grad, (0,0), mask)

# Film perforations (left & right vertical bars)
d2 = ImageDraw.Draw(img)
bar_w = 90
holes = 8
hole_w = 56
hole_h = 70
margin_y = (SIZE - holes * hole_h - (holes-1)* ( (SIZE - 2*margin_top) )) if False else None
top = 70
gap = (SIZE - 2*top - holes*hole_h) / (holes-1)
# left bar
d2.rounded_rectangle([40, 40, 40+bar_w, SIZE-40], radius=20, fill=(255,255,255,40))
d2.rounded_rectangle([SIZE-40-bar_w, 40, SIZE-40, SIZE-40], radius=20, fill=(255,255,255,40))
for i in range(holes):
    y = top + i*(hole_h+gap)
    d2.rounded_rectangle([40+ (bar_w-hole_w)/2, y, 40+ (bar_w-hole_w)/2 + hole_w, y+hole_h], radius=12, fill=(0,0,0,160))
    d2.rounded_rectangle([SIZE-40-bar_w + (bar_w-hole_w)/2, y, SIZE-40-bar_w + (bar_w-hole_w)/2 + hole_w, y+hole_h], radius=12, fill=(0,0,0,160))

# Play triangle in center (white)
cx, cy = SIZE/2, SIZE/2
tri = 300
pts = [
    (cx - tri*0.32, cy - tri*0.6),
    (cx - tri*0.32, cy + tri*0.6),
    (cx + tri*0.55, cy),
]
d2.polygon(pts, fill=(255,255,255,255))

img.save("H:/soft/kantanVideoEdit/tools/icon_source.png")
print("icon saved")
