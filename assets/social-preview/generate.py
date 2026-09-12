import os, sys
from PIL import Image, ImageDraw, ImageFont

S = os.path.dirname(os.path.abspath(__file__))
OUT = sys.argv[1] if len(sys.argv) > 1 else os.path.join(S, "out")
os.makedirs(OUT, exist_ok=True)

W, H = 1280, 640
BG, SURFACE, BORDER = "#0f1117", "#161922", "#1e2130"
TEXT, MUTED, ACCENT = "#e4e6ed", "#6b7280", "#00d4ff"
MONO = "/System/Library/Fonts/SFNSMono.ttf"
GOTHIC = "/System/Library/Fonts/AppleSDGothicNeo.ttc"

def font(path, size, index=0):
    return ImageFont.truetype(path, size, index=index)

f_word = font(GOTHIC, 30, 6)      # Apple SD Gothic Neo Bold-ish
f_url = font(MONO, 22)
f_desc = font(GOTHIC, 30, 0)      # Regular
logo = Image.open(os.path.join(S, "logo.png")).convert("RGB").crop((152, 418, 877, 585))
logo = logo.resize((int(725 * 40 / 167), 40), Image.LANCZOS)

def wrap(draw, text, f, maxw, maxlines=3):
    words = text.split()
    lines, cur = [], ""
    for w in words:
        t = (cur + " " + w).strip()
        if draw.textlength(t, font=f) <= maxw:
            cur = t
        else:
            if cur: lines.append(cur)
            cur = w
    if cur: lines.append(cur)
    if len(lines) > maxlines:
        lines = lines[:maxlines]
        last = lines[-1]
        while draw.textlength(last + "…", font=f) > maxw and last:
            last = last[:-1].rstrip()
        lines[-1] = last + "…"
    return lines

def render(name, desc):
    im = Image.new("RGB", (W, H), BG)
    d = ImageDraw.Draw(im)
    M = 80
    # header
    im.paste(logo, (M, 72))
    # accent rule
    d.rectangle([M, 176, M + 64, 180], fill=ACCENT)
    # repo name: shrink to fit
    size = 88
    while True:
        f_name = font(MONO, size)
        if d.textlength(name, font=f_name) <= W - 2 * M or size <= 40:
            break
        size -= 4
    d.text((M - 4, 212), name, font=f_name, fill=TEXT)
    name_h = f_name.getbbox("Hg")[3]
    # description
    y = 212 + name_h + 44
    lines = wrap(d, desc or "", f_desc, W - 2 * M)
    for ln in lines:
        d.text((M, y), ln, font=f_desc, fill=MUTED)
        y += 44
    # footer
    d.line([M, H - 96, W - M, H - 96], fill=BORDER, width=2)
    d.text((W - M, H - 56), f"github.com/Open330/{name}", font=f_url, fill=ACCENT, anchor="rm")
    im.save(os.path.join(OUT, f"{name}.png"), optimize=True)
    return y

with open(os.path.join(S, "repos.tsv")) as fh:
    for line in fh:
        line = line.rstrip("\n")
        if not line.strip(): continue
        name, _, desc = line.partition("\t")
        name = name.strip()
        ybot = render(name, desc.strip())
        print(f"{name:24s} desc_bottom={ybot}")
