"""无框图标：去白底、裁紧图形边界、放大到占满画布"""
from PIL import Image

src = Image.open(r"D:\SkillLink2\app\ui\7a87db98-dc9c-4a7a-a219-686e09fc85b7.png").convert("RGBA")
w, h = src.size

# 1. 白底/浅灰边框转透明：明度偏高的像素（min 通道 > 200）一律透明
px = src.load()
for y in range(h):
    for x in range(w):
        r, g, b, a = px[x, y]
        if min(r, g, b) > 200:
            px[x, y] = (r, g, b, 0)

# 2. 不做任何裁剪：整张图按比例缩放，保证头/左/底不被剪掉
cw, ch = src.size

# 3. 缩放到 240px（留 8px 边距），保持比例
side = max(cw, ch)
scale = 240.0 / side
new_w, new_h = max(1, int(cw * scale)), max(1, int(ch * scale))
resized = src.resize((new_w, new_h), Image.LANCZOS)

# 4. 居中到 256x256
canvas = Image.new("RGBA", (256, 256), (0, 0, 0, 0))
canvas.paste(resized, ((256 - new_w) // 2, (256 - new_h) // 2))
canvas.save(r"D:\SkillLink2\app\ui\icon.png")
canvas.save(
    r"D:\SkillLink2\app\icon.ico",
    sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
)
print("frameless icon ok")
