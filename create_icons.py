import os
try:
    from PIL import Image, ImageDraw
except ImportError:
    os.system("pip install Pillow")
    from PIL import Image, ImageDraw

def create_icon(filename, glow_color, line_color):
    size = (512, 512)
    img = Image.new('RGBA', size, (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    # Monitor Frame
    d.rectangle([40, 60, 472, 420], fill=(30, 30, 30, 255), outline=(80, 80, 80, 255), width=4)
    d.rectangle([60, 80, 452, 380], fill=(10, 15, 25, 255))

    # Heartbeat Line
    line_pts = [(80, 230), (140, 230), (160, 150), (190, 310), (220, 200), (250, 260), (280, 90), (310, 350), (340, 230), (430, 230)]
    d.line(line_pts, fill=line_color, width=14, joint="curve")

    # LED
    d.ellipse([251, 395, 261, 405], fill=glow_color)

    # Save
    icon_path = os.path.join("esp_sender_rust", filename)
    img.save(icon_path, format='ICO', sizes=[(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
    print(f"✅ Created {filename}")

if __name__ == "__main__":
    create_icon("icon_online.ico", (0, 255, 100, 255), (0, 255, 255, 255))  # Green LED, Cyan Line
    create_icon("icon_offline.ico", (255, 0, 0, 255), (100, 100, 100, 255)) # Red LED, Grey Line
