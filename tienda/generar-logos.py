"""Genera los logos del paquete de la Store (tienda/Assets/*.png) a partir del
icono de la app (el dibujo de 256x256 de apps/pindesk/recursos/pindesk.ico).

Uso (desde la raiz del repo, con Pillow instalado):
    python tienda/generar-logos.py apps/pindesk/recursos/pindesk.ico tienda/Assets

Los PNG van en el repo: solo hay que volver a generarlos si cambia el icono.
Cada logo sale sin calificador (escala 100, vale aunque no haya
resources.pri) y con sus escalas; Square44x44Logo, ademas, con los tamanos
exactos que piden la barra de tareas, Inicio y el Explorador (targetsize-*,
con y sin placa). El workflow (tienda.yml) junta las variantes con makepri.
"""

import os
import sys

from PIL import Image

ico, salida = sys.argv[1], sys.argv[2]
os.makedirs(salida, exist_ok=True)
base = Image.open(ico)
base.size = (256, 256)
base = base.convert("RGBA")
# Sin el margen transparente: asi la fraccion es la del dibujo.
caja = base.getbbox()
dibujo = base.crop(caja) if caja else base


def lienzo(w, h, fraccion):
    """El dibujo centrado en w x h, ocupando `fraccion` del lado menor."""
    c = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    lado = max(1, round(min(w, h) * fraccion))
    d = dibujo.copy()
    d.thumbnail((lado, lado), Image.LANCZOS)
    c.paste(d, ((w - d.width) // 2, (h - d.height) // 2), d)
    return c


def guardar(im, nombre):
    im.save(os.path.join(salida, nombre), optimize=True)


ESCALAS = [100, 125, 150, 200, 400]
# (nombre, ancho y alto a escala 100, fraccion del dibujo)
LOGOS = [
    ("Square44x44Logo", 44, 44, 1.0),
    ("Square71x71Logo", 71, 71, 0.6),
    ("Square150x150Logo", 150, 150, 0.6),
    ("Wide310x150Logo", 310, 150, 0.6),
    ("Square310x310Logo", 310, 310, 0.6),
    ("StoreLogo", 50, 50, 1.0),
    ("SplashScreen", 620, 300, 0.5),
]
for nombre, w, h, f in LOGOS:
    guardar(lienzo(w, h, f), f"{nombre}.png")
    for e in ESCALAS:
        guardar(lienzo(round(w * e / 100), round(h * e / 100), f), f"{nombre}.scale-{e}.png")
for t in [16, 20, 24, 30, 32, 36, 40, 48, 60, 64, 72, 80, 96, 256]:
    im = lienzo(t, t, 1.0)
    guardar(im, f"Square44x44Logo.targetsize-{t}.png")
    guardar(im, f"Square44x44Logo.targetsize-{t}_altform-unplated.png")
    guardar(im, f"Square44x44Logo.targetsize-{t}_altform-lightunplated.png")
print("logos:", len(os.listdir(salida)))
