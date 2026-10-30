import cairosvg, io
from PIL import Image
BG,FG,BD='#000000','#EDEDED','#2A2A2A'
def tk(x0,y0,x1,y1,rc,n,cy):
    return (f"M{x0+rc} {y0}H{x1-rc}A{rc} {rc} 0 0 1 {x1} {y0+rc}V{cy-n}A{n} {n} 0 0 0 {x1} {cy+n}"
            f"V{y1-rc}A{rc} {rc} 0 0 1 {x1-rc} {y1}H{x0+rc}A{rc} {rc} 0 0 1 {x0} {y1-rc}"
            f"V{cy+n}A{n} {n} 0 0 0 {x0} {cy-n}V{y0+rc}A{rc} {rc} 0 0 1 {x0+rc} {y0}Z")
# 32-grid master glyph: ticket 4..28 x 8..24, notch r3, 2x2 perforation holes at x19 (evenodd)
G32=tk(4,8,28,24,2.5,3,16)+"".join(f"M19 {y}h2v2h-2Z" for y in (11,15,19))
# 16-grid hinted glyph: ticket 2..14 x 4..13 (9 rows), notch r1.5 at row 8.5, 1px holes at col 10 rows 5,7,9,11
G16=tk(2,4,14,13,1,1.5,8.5)+"".join(f"M10 {y}h1v1h-1Z" for y in (5,7,9,11))
def tile32(edge=True):  # 1px edge keeps the black tile visible in dark browser tabs; omitted at app-icon sizes
    rect=(f'<rect x=".5" y=".5" width="31" height="31" rx="7.5" fill="{BG}" stroke="{BD}"/>' if edge else f'<rect width="32" height="32" rx="7.5" fill="{BG}"/>')
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32">{rect}'
                      f'<path fill="{FG}" fill-rule="evenodd" d="{G32}"/></svg>')
def tile16(): return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><rect width="16" height="16" rx="3.5" fill="{BG}"/>'
                      f'<path fill="{FG}" fill-rule="evenodd" d="{G16}"/></svg>')
def full(scale_pct):  # full-bleed square, glyph scaled about centre (apple-touch / maskable)
    s=scale_pct/100; t=16*(1-s)
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32"><rect width="32" height="32" fill="{BG}"/>'
            f'<path transform="translate({t} {t}) scale({s})" fill="{FG}" fill-rule="evenodd" d="{G32}"/></svg>')
def pngb(svg,n): return cairosvg.svg2png(bytestring=svg.encode(),output_width=n,output_height=n)
open('favicon.svg','w').write(tile32()+"\n")
open('favicon-16.svg','w').write(tile16()+"\n")
open('favicon-32.png','wb').write(pngb(tile32(),32))
open('apple-touch-icon.png','wb').write(pngb(full(100),180))
open('icon-192.png','wb').write(pngb(tile32(False),192))
open('icon-512.png','wb').write(pngb(tile32(False),512))
open('icon-maskable-512.png','wb').write(pngb(full(80),512))
ims=[Image.open(io.BytesIO(pngb(tile16(),16))).convert('RGBA'),Image.open(io.BytesIO(pngb(tile32(),32))).convert('RGBA'),Image.open(io.BytesIO(pngb(tile32(),48))).convert('RGBA')]
ims[2].save('favicon.ico',sizes=[(16,16),(32,32),(48,48)],append_images=ims[:2])
# preview: 16 hinted vs 16 unhinted zoomed, on the dark background
pv=Image.new('RGB',(280,200),BG)
for j,svg in enumerate([tile16(),tile32()]):
    im=Image.open(io.BytesIO(pngb(svg,16))).convert('RGBA')
    tile=Image.new('RGBA',(16,16),BG); tile.alpha_composite(im)
    pv.paste(tile.resize((128,128),Image.NEAREST),(10+j*140,10))
    pv.paste(tile,(10+j*140,160))
pv.save('preview16.png')
print(G16)
