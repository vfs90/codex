"""Render captured native terminal cells; Pillow is a review-only dependency."""
import json
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

review = Path(__file__).resolve().parent.parent
font = ImageFont.truetype('/usr/share/fonts/redhat-vf/RedHatMono[wght].ttf', 15)
caption_font = ImageFont.truetype('/usr/share/fonts/redhat-vf/RedHatText[wght].ttf', 16)
cell_width, cell_height = 9, 21
palette = dict(black='#151515', red='#cd5555', green='#68b868', yellow='#d3bd66',
               blue='#668bc9', magenta='#bd7fcb', cyan='#64b9c5', white='#eeeeee',
               brown='#d3bd66', brightblack='#888888', brightred='#ff8888',
               brightgreen='#99ee99', brightyellow='#eeee99', brightblue='#99bbff',
               brightmagenta='#ee99ee', brightcyan='#99eeee', brightwhite='#ffffff')


def color(value, default):
    if value == 'default':
        return default
    if value in palette:
        return palette[value]
    if len(value) == 6 and all(ch in '0123456789abcdefABCDEF' for ch in value):
        return '#' + value
    return default


def render(frame, padded=False):
    cols = 160 if padded else frame['width']
    picture = Image.new('RGB', (cols * cell_width + 32, 30 * cell_height + 68), '#26292e')
    draw = ImageDraw.Draw(picture)
    draw.text((16, 12), f"Native Codex CLI · {frame['width']} × {frame['height']}", fill='#dddddd', font=caption_font)
    draw.text((16, 36), 'Synthetic usage; quotas unavailable', fill='#aaaaaa', font=caption_font)
    draw.rectangle((16, 64, frame['width'] * cell_width + 15, 30 * cell_height + 63), fill='#181818')
    for y, row in enumerate(frame['rows']):
        for x, cell in enumerate(row):
            px, py = 16 + x * cell_width, 64 + y * cell_height
            background = color(cell['bg'], '#181818')
            if background != '#181818':
                draw.rectangle((px, py, px + cell_width - 1, py + cell_height - 1), fill=background)
            foreground = color(cell['fg'], '#dddddd')
            # RedHatMono lacks these block glyphs. Draw the captured terminal
            # symbols explicitly rather than showing identical missing glyphs.
            if cell['text'] == '█':
                draw.rectangle((px, py + 3, px + cell_width - 1, py + cell_height - 2), fill=foreground)
            elif cell['text'] == '░':
                for dot_y in range(py + 3, py + cell_height - 1, 3):
                    for dot_x in range(px + (dot_y % 2), px + cell_width, 3):
                        draw.point((dot_x, dot_y), fill=foreground)
            else:
                draw.text((px, py), cell['text'], font=font, fill=foreground)
    return picture


frames = json.loads((review / 'color/frames.json').read_text())
render(frames[0]).save(review / 'infobar-full.png')
for width, name in [(48, 'compact'), (40, 'wrapped')]:
    frame = next(frame for frame in frames if frame['width'] == width)
    render(frame).save(review / f'infobar-{name}.png')
animation = [render(frame, padded=True) for frame in frames]
animation[0].save(review / 'infobar-resize.gif', save_all=True, append_images=animation[1:],
                  duration=round(20000 / len(animation)), loop=0, disposal=2)
print(f'Rendered {len(frames)} native terminal captures as three PNGs and a 20-second GIF.')
