#!/usr/bin/env python3
"""Las fotos inventadas de los prototipos `mejor_foto`, `recortar_pizarra`
y `elegir_ocr` (29-oct-2026, docs/probar-ideas-sin-interfaz.md, «Más
casos»). Todo dibujado aqui con Pillow: ninguna foto de nadie.

    python3 hacer_fotos.py [carpeta]      # por omision, junto a este fichero

Escribe las fotos y, en `verdad.txt`, la caja de verdad de cada pizarra u
hoja (top bottom left right, en fracciones) para los guiones de
`recortar_pizarra`. Las fotos del repositorio se hicieron con Pillow 13.0;
con otra version pueden salir otros bytes (y las respuestas grabadas ya no
casarian: hay que grabarlas otra vez).
"""

import math
import random
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageEnhance, ImageFilter, ImageFont

FUENTES = "/usr/share/fonts/truetype"
SANS = f"{FUENTES}/liberation/LiberationSans-Regular.ttf"
SANS_B = f"{FUENTES}/liberation/LiberationSans-Bold.ttf"
SERIF = f"{FUENTES}/liberation/LiberationSerif-Regular.ttf"
OBLICUA = f"{FUENTES}/freefont/FreeSansOblique.ttf"
MONO = f"{FUENTES}/liberation/LiberationMono-Regular.ttf"

W, H = 512, 384
SALIDA = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).parent
VERDAD = []


def fuente(ruta, t):
    return ImageFont.truetype(ruta, t)


def guardar(img, nombre):
    p = SALIDA / nombre
    if nombre.endswith(".png"):
        img.convert("RGB").save(p, "PNG", optimize=True)
    else:
        img.convert("RGB").save(p, "JPEG", quality=80)


# ------------------------------------------------------------ retoques


def movida(img, n=14, dx=1.5):
    """Movimiento de la camara: la media de copias corridas."""
    w, h = img.size
    m = int(n * dx) + 4
    grande = img.resize((w + 2 * m, h + 2 * m), Image.BICUBIC)
    trozo = lambda i: grande.crop((m + int(i * dx), m + int(i * dx / 4), m + int(i * dx) + w, m + int(i * dx / 4) + h))
    acc = trozo(0)
    for i in range(1, n):
        acc = Image.blend(acc, trozo(i), 1.0 / (i + 1))
    return acc


def desenfocada(img, r=5):
    return img.filter(ImageFilter.GaussianBlur(r))


def oscura(img, f=0.22):
    return ImageEnhance.Brightness(img).enhance(f)


def quemada(img, f=2.4):
    return ImageEnhance.Brightness(img).enhance(f)


def torcida(img, grados=13):
    """Girada y recortada al centro (sin esquinas vacias)."""
    g = img.rotate(grados, resample=Image.BICUBIC, expand=False)
    z = 1.0 + abs(math.sin(math.radians(grados))) * 1.1
    w, h = img.size
    cw, ch = int(w / z), int(h / z)
    x, y = (w - cw) // 2, (h - ch) // 2
    return g.crop((x, y, x + cw, y + ch)).resize((w, h), Image.BICUBIC)


# ------------------------------------------------------------ fondos


def pared(color=(214, 205, 188), semilla=1):
    rnd = random.Random(semilla)
    img = Image.new("RGB", (W, H), color)
    d = ImageDraw.Draw(img)
    for y in range(H):
        f = 0.92 + 0.08 * y / H
        d.line([(0, y), (W, y)], fill=tuple(int(c * f) for c in color))
    for _ in range(900):
        x, y = rnd.randrange(W), rnd.randrange(H)
        k = rnd.randint(-10, 10)
        d.point((x, y), fill=tuple(max(0, min(255, c + k)) for c in color))
    return img


def mesa(semilla=2):
    rnd = random.Random(semilla)
    img = Image.new("RGB", (W, H), (142, 96, 58))
    d = ImageDraw.Draw(img)
    for y in range(0, H, 3):
        k = rnd.randint(-14, 14)
        d.line([(0, y), (W, y + rnd.randint(-2, 2))], fill=(142 + k, 96 + k, 58 + k // 2), width=2)
    return img


# ------------------------------------------------------------ personas


def persona(d, cx, cy, r, ojos="abiertos", piel=(236, 196, 160), pelo=(70, 45, 30), ropa=(40, 90, 160)):
    d.ellipse([cx - r * 1.6, cy + r * 1.0, cx + r * 1.6, cy + r * 4.0], fill=ropa)
    d.rectangle([cx - r * 0.3, cy + r * 0.7, cx + r * 0.3, cy + r * 1.2], fill=piel)
    d.ellipse([cx - r * 1.05, cy - r * 1.25, cx + r * 1.05, cy + r * 0.55], fill=pelo)
    d.ellipse([cx - r, cy - r, cx + r, cy + r * 1.05], fill=piel)
    d.ellipse([cx - r * 1.12, cy - r * 0.1, cx - r * 0.9, cy + r * 0.3], fill=piel)
    d.ellipse([cx + r * 0.9, cy - r * 0.1, cx + r * 1.12, cy + r * 0.3], fill=piel)
    d.chord([cx - r, cy - r * 1.05, cx + r, cy - r * 0.15], 180, 360, fill=pelo)
    for s in (-1, 1):
        ex, ey = cx + s * r * 0.4, cy - r * 0.05
        d.line([ex - r * 0.25, ey - r * 0.32, ex + r * 0.25, ey - r * 0.36], fill=pelo, width=max(2, int(r * 0.08)))
        if ojos == "abiertos":
            d.ellipse([ex - r * 0.22, ey - r * 0.13, ex + r * 0.22, ey + r * 0.13], fill=(250, 250, 250), outline=(60, 40, 30))
            d.ellipse([ex - r * 0.11, ey - r * 0.11, ex + r * 0.11, ey + r * 0.11], fill=(80, 55, 30))
            d.ellipse([ex - r * 0.05, ey - r * 0.05, ex + r * 0.05, ey + r * 0.05], fill=(10, 10, 10))
        else:
            d.arc([ex - r * 0.22, ey - r * 0.12, ex + r * 0.22, ey + r * 0.12], 10, 170, fill=(60, 40, 30), width=max(2, int(r * 0.06)))
    d.line([cx, cy + r * 0.05, cx - r * 0.08, cy + r * 0.38, cx + r * 0.06, cy + r * 0.4], fill=(190, 140, 110), width=2)
    d.arc([cx - r * 0.35, cy + r * 0.35, cx + r * 0.35, cy + r * 0.75], 20, 160, fill=(170, 60, 60), width=max(2, int(r * 0.07)))


def retrato(ojos=("abiertos",), semilla=3):
    img = pared((196, 214, 222), semilla)
    d = ImageDraw.Draw(img)
    d.rectangle([0, int(H * 0.78), W, H], fill=(120, 130, 140))
    d.rectangle([W * 0.72, H * 0.1, W * 0.95, H * 0.45], fill=(235, 235, 225), outline=(90, 70, 50), width=6)
    d.ellipse([W * 0.78, H * 0.18, W * 0.89, H * 0.32], fill=(230, 160, 60))
    n = len(ojos)
    for i, o in enumerate(ojos):
        cx = W * (i + 1) / (n + 1)
        ropa = [(40, 90, 160), (160, 60, 60), (60, 130, 80)][i % 3]
        pelo = [(70, 45, 30), (30, 25, 20), (150, 100, 50)][i % 3]
        persona(d, cx, H * 0.38, 46 if n == 1 else 34, o, ropa=ropa, pelo=pelo)
    return img


def paisaje(semilla=4):
    img = Image.new("RGB", (W, H), (120, 170, 230))
    d = ImageDraw.Draw(img)
    for y in range(int(H * 0.6)):
        f = y / (H * 0.6)
        d.line([(0, y), (W, y)], fill=(int(110 + 80 * f), int(160 + 60 * f), int(230 + 15 * f)))
    d.ellipse([W * 0.7, H * 0.08, W * 0.82, H * 0.24], fill=(255, 236, 160))
    d.polygon([(0, H * 0.62), (W * 0.25, H * 0.25), (W * 0.48, H * 0.62)], fill=(110, 110, 130))
    d.polygon([(W * 0.18, H * 0.38), (W * 0.25, H * 0.25), (W * 0.31, H * 0.36)], fill=(245, 245, 250))
    d.polygon([(W * 0.3, H * 0.62), (W * 0.62, H * 0.18), (W * 0.95, H * 0.62)], fill=(95, 100, 120))
    d.polygon([(W * 0.54, H * 0.29), (W * 0.62, H * 0.18), (W * 0.7, H * 0.29)], fill=(245, 245, 250))
    d.rectangle([0, H * 0.6, W, H], fill=(80, 140, 70))
    rnd = random.Random(semilla)
    for _ in range(14):
        x, y = rnd.randrange(W), rnd.randint(int(H * 0.65), H - 10)
        d.polygon([(x, y - 40), (x - 14, y), (x + 14, y)], fill=(40, 90, 45))
        d.rectangle([x - 2, y, x + 2, y + 8], fill=(90, 60, 30))
    return img


# ------------------------------------------------------------ pizarras y hojas


def capa_pizarra(w, h, tipo="blanca"):
    """Una pizarra (o una hoja) suelta, en RGBA, con lo escrito."""
    c = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    d = ImageDraw.Draw(c)
    if tipo == "blanca":
        d.rectangle([0, 0, w - 1, h - 1], fill=(246, 247, 245, 255), outline=(150, 152, 156, 255), width=7)
        tinta = [(20, 60, 170, 255), (190, 30, 30, 255), (20, 20, 20, 255)]
        lineas = ["Derivadas", "f'(x) = lim (f(x+h) - f(x)) / h", "Regla: (x^n)' = n x^(n-1)"]
    elif tipo == "verde":
        d.rectangle([0, 0, w - 1, h - 1], fill=(38, 74, 56, 255), outline=(120, 85, 50, 255), width=9)
        tinta = [(235, 235, 230, 255)] * 3
        lineas = ["Fotosíntesis", "6CO2 + 6H2O -> C6H12O6 + 6O2", "luz + clorofila"]
    else:
        d.rectangle([0, 0, w - 1, h - 1], fill=(250, 250, 246, 255))
        for y in range(int(h * 0.12), h, max(8, h // 14)):
            d.line([(4, y), (w - 4, y)], fill=(170, 200, 230, 255), width=1)
        d.line([(int(w * 0.12), 0), (int(w * 0.12), h)], fill=(230, 150, 150, 255), width=1)
        tinta = [(30, 50, 140, 255)] * 3
        lineas = ["Lista de la obra", "- revisar planos baño", "- pedir azulejos lunes"]
    t = max(10, h // 9)
    x = int(w * 0.08) + (int(w * 0.06) if tipo == "hoja" else 0)
    # Lo escrito cabe en la pizarra.
    while t > 8 and max(d.textlength(l, font=fuente(OBLICUA, t)) for l in lineas) > w - x - int(w * 0.06):
        t -= 1
    y = int(h * 0.12)
    for i, l in enumerate(lineas):
        d.text((x, y), l, font=fuente(OBLICUA, t if i else int(t * 1.2)), fill=tinta[i % len(tinta)])
        y += int(t * 1.7)
    if tipo != "hoja":
        x0, y0 = int(w * 0.62), int(h * 0.92)
        d.line([(x0, y0), (x0, int(h * 0.55))], fill=tinta[-1], width=2)
        d.line([(x0, y0), (int(w * 0.95), y0)], fill=tinta[-1], width=2)
        pts = [(x0 + i * 4, y0 - int(((i - 15) ** 2) * 0.12) - 8) for i in range(31)]
        d.line(pts, fill=tinta[0], width=3)
    return c


def pegar(fondo, capa, x, y, grados, nombre):
    g = capa.rotate(grados, resample=Image.BICUBIC, expand=True)
    fondo = fondo.convert("RGBA")
    sombra = Image.new("RGBA", g.size, (0, 0, 0, 0))
    sombra.putalpha(g.getchannel("A").point(lambda a: a // 3))
    fondo.alpha_composite(sombra.filter(ImageFilter.GaussianBlur(4)), (x + 5, y + 6)) if 0 <= x and 0 <= y else None
    lienzo = Image.new("RGBA", fondo.size, (0, 0, 0, 0))
    lienzo.paste(g, (x, y), g)
    fondo.alpha_composite(lienzo)
    l, t, r, b = lienzo.getchannel("A").point(lambda a: 255 if a > 40 else 0).getbbox()
    VERDAD.append(f"{nombre}: {t / H:.3f} {b / H:.3f} {l / W:.3f} {r / W:.3f}")
    return fondo.convert("RGB")


def cuadro(fondo):
    """Un cuadro de colores en la pared: no es una pizarra."""
    d = ImageDraw.Draw(fondo)
    d.rectangle([W * 0.3, H * 0.15, W * 0.7, H * 0.65], fill=(240, 230, 200), outline=(110, 75, 40), width=12)
    d.rectangle([W * 0.34, H * 0.2, W * 0.66, H * 0.6], fill=(60, 120, 190))
    d.ellipse([W * 0.4, H * 0.25, W * 0.52, H * 0.4], fill=(240, 200, 60))
    d.polygon([(W * 0.34, H * 0.6), (W * 0.48, H * 0.38), (W * 0.66, H * 0.6)], fill=(60, 140, 70))
    return fondo


def ventana(fondo):
    d = ImageDraw.Draw(fondo)
    d.rectangle([W * 0.2, H * 0.1, W * 0.8, H * 0.7], fill=(150, 200, 240), outline=(245, 245, 245), width=14)
    d.line([(W * 0.5, H * 0.1), (W * 0.5, H * 0.7)], fill=(245, 245, 245), width=10)
    for y in range(int(H * 0.14), int(H * 0.66), 18):
        d.line([(W * 0.22, y), (W * 0.78, y)], fill=(225, 225, 220), width=6)
    return fondo


# ------------------------------------------------------------ OCR


def pantalla_chat():
    img = Image.new("RGB", (640, 420), (245, 246, 248))
    d = ImageDraw.Draw(img)
    d.rectangle([0, 0, 640, 34], fill=(40, 44, 52))
    d.text((14, 8), "Chat - Obra Calle Mayor", font=fuente(SANS_B, 16), fill=(240, 240, 240))
    for i, c in enumerate([(255, 95, 86), (255, 189, 46), (39, 201, 63)]):
        d.ellipse([560 + i * 24, 10, 574 + i * 24, 24], fill=c)
    msgs = [
        ("Lucía", "He subido los planos del baño, revisadlos.", False),
        ("Yo", "Vale, los miro esta tarde.", True),
        ("Pedro", "El fontanero viene el jueves a las 9.", False),
        ("Yo", "Perfecto, le abro yo.", True),
    ]
    y = 52
    f = fuente(SANS, 15)
    for quien, t, mio in msgs:
        w = int(d.textlength(t, font=f)) + 28
        x = 640 - w - 16 if mio else 16
        d.rounded_rectangle([x, y, x + w, y + 54], 10, fill=(214, 238, 255) if mio else (255, 255, 255), outline=(220, 222, 226))
        d.text((x + 14, y + 6), quien, font=fuente(SANS_B, 13), fill=(90, 90, 110))
        d.text((x + 14, y + 26), t, font=f, fill=(20, 20, 20))
        y += 70
    d.rectangle([0, 380, 640, 420], fill=(255, 255, 255))
    d.text((16, 392), "Escribe un mensaje...", font=fuente(SANS, 14), fill=(150, 150, 150))
    return img


def pantalla_ajustes():
    img = Image.new("RGB", (600, 400), (250, 250, 250))
    d = ImageDraw.Draw(img)
    d.rectangle([0, 0, 600, 32], fill=(230, 232, 236))
    d.text((12, 7), "Ajustes - Captura", font=fuente(SANS, 15), fill=(30, 30, 30))
    filas = [
        ("Formato de imagen", "PNG"),
        ("Carpeta de capturas", "Documentos\\Capturas"),
        ("Reconocer texto (OCR)", "Windows"),
        ("Copiar al portapapeles", "Sí"),
        ("Sonido al capturar", "No"),
    ]
    y = 56
    for k, v in filas:
        d.text((24, y), k, font=fuente(SANS, 15), fill=(20, 20, 20))
        d.rounded_rectangle([360, y - 4, 576, y + 22], 4, fill=(255, 255, 255), outline=(190, 190, 195))
        d.text((370, y), v, font=fuente(SANS, 14), fill=(40, 40, 40))
        y += 56
    return img


def hoja_impresa(texto, semilla=5, grados=-2.0, titulo=None):
    papel = Image.new("RGBA", (440, 330), (252, 251, 245, 255))
    d = ImageDraw.Draw(papel)
    y = 24
    if titulo:
        d.text((28, y), titulo, font=fuente(SANS_B, 20), fill=(20, 20, 20, 255))
        y += 38
    f = fuente(SERIF, 15)
    for linea in texto:
        d.text((28, y), linea, font=f, fill=(25, 25, 25, 255))
        y += 22
    fondo = mesa(semilla)
    g = papel.rotate(grados, resample=Image.BICUBIC, expand=True)
    fondo = fondo.convert("RGBA")
    fondo.alpha_composite(g, ((W - g.width) // 2, (H - g.height) // 2))
    return fondo.convert("RGB")


def tabla_impresa(filas, semilla=6, grados=1.5):
    papel = Image.new("RGBA", (440, 300), (252, 252, 250, 255))
    d = ImageDraw.Draw(papel)
    cols = [24, 200, 300, 416]
    y = 24
    for i, fila in enumerate(filas):
        f = fuente(SANS_B if i == 0 else SANS, 15)
        for j, celda in enumerate(fila):
            d.text((cols[j] + 6, y + 6), celda, font=f, fill=(20, 20, 20, 255))
        d.line([(cols[0], y), (cols[-1], y)], fill=(60, 60, 60, 255), width=2 if i < 2 else 1)
        y += 34
    d.line([(cols[0], y), (cols[-1], y)], fill=(60, 60, 60, 255), width=2)
    for x in cols:
        d.line([(x, 24), (x, y)], fill=(60, 60, 60, 255), width=1)
    fondo = mesa(semilla)
    g = papel.rotate(grados, resample=Image.BICUBIC, expand=True)
    fondo = fondo.convert("RGBA")
    fondo.alpha_composite(g, ((W - g.width) // 2, (H - g.height) // 2))
    return fondo.convert("RGB")


def tabla_pantalla(filas):
    img = Image.new("RGB", (600, 360), (255, 255, 255))
    d = ImageDraw.Draw(img)
    d.rectangle([0, 0, 600, 30], fill=(33, 115, 70))
    d.text((10, 6), "Presupuesto.xlsx", font=fuente(SANS_B, 15), fill=(255, 255, 255))
    cols = [40, 250, 380, 580]
    y = 50
    for i, fila in enumerate(filas):
        if i == 0:
            d.rectangle([cols[0], y, cols[-1], y + 30], fill=(230, 240, 232))
        for j, c in enumerate(fila):
            d.text((cols[j] + 8, y + 7), c, font=fuente(SANS_B if i == 0 else SANS, 14), fill=(20, 20, 20))
        d.line([(cols[0], y), (cols[-1], y)], fill=(200, 200, 200))
        y += 30
    d.line([(cols[0], y), (cols[-1], y)], fill=(200, 200, 200))
    for x in cols:
        d.line([(x, 50), (x, y)], fill=(200, 200, 200))
    for i in range(len(filas)):
        d.text((14, 57 + i * 30), str(i + 1), font=fuente(SANS, 12), fill=(130, 130, 130))
    return img


def manuscrito(lineas, semilla=7, tinta=(25, 45, 150)):
    """Letra a mano fingida: cada letra con su giro, su tamano y su altura,
    en una hoja con renglones."""
    rnd = random.Random(semilla)
    img = Image.new("RGB", (W, H), (250, 249, 240))
    d = ImageDraw.Draw(img)
    for y in range(40, H, 34):
        d.line([(0, y), (W, y)], fill=(175, 205, 235), width=1)
    d.line([(46, 0), (46, H)], fill=(235, 150, 150), width=2)
    y = 40 - 30
    for linea in lineas:
        x = 58 + rnd.randint(-4, 6)
        for ch in linea:
            t = rnd.randint(21, 26)
            f = fuente(OBLICUA, t)
            c = Image.new("RGBA", (t * 2, t * 2), (0, 0, 0, 0))
            cd = ImageDraw.Draw(c)
            cd.text((t // 2, t // 3), ch, font=f, fill=tinta + (255,))
            cd.text((t // 2 + 1, t // 3), ch, font=f, fill=tinta + (255,))
            c = c.rotate(rnd.uniform(-9, 9), resample=Image.BICUBIC)
            img.paste(c, (int(x) - t // 2, y + rnd.randint(-2, 2)), c)
            x += d.textlength(ch, font=f) * rnd.uniform(0.92, 1.08) + (2 if ch == " " else 0.5)
        y += 34
    return img.filter(ImageFilter.GaussianBlur(0.6))


# ------------------------------------------------------------ todo


def main():
    SALIDA.mkdir(parents=True, exist_ok=True)
    # Rafagas (mejor_foto).
    a = retrato(("abiertos",), 10)
    guardar(movida(a), "r01_1.jpg")
    guardar(a, "r01_2.jpg")
    guardar(retrato(("cerrados",), 10), "r01_3.jpg")
    b = retrato(("abiertos", "abiertos"), 11)
    guardar(retrato(("abiertos", "cerrados"), 11), "r02_1.jpg")
    guardar(oscura(b), "r02_2.jpg")
    guardar(b, "r02_3.jpg")
    pz = pegar(pared(semilla=12), capa_pizarra(380, 250), 66, 60, 0, "r03")
    guardar(pz, "r03_1.jpg")
    guardar(torcida(pz, 14), "r03_2.jpg")
    guardar(desenfocada(pz, 5), "r03_3.jpg")
    p = paisaje(13)
    guardar(quemada(p), "r04_1.jpg")
    guardar(p, "r04_2.jpg")
    guardar(movida(p, 16, 2), "r04_3.jpg")
    guardar(oscura(p), "r04_4.jpg")
    c = retrato(("abiertos",), 14)
    guardar(desenfocada(c, 6), "r05_1.jpg")
    guardar(oscura(movida(c), 0.25), "r05_2.jpg")
    # Rafagas de control.
    e = retrato(("abiertos",), 20)
    guardar(retrato(("cerrados",), 20), "rc1_1.jpg")
    guardar(e, "rc1_2.jpg")
    h = hoja_impresa(["Acta de la reunión de vecinos", "1. Arreglo del portal: aprobado.", "2. Ascensor: se piden tres", "presupuestos antes de enero.", "3. Limpieza: sin cambios."], 21, 0)
    guardar(h, "rc2_1.jpg")
    guardar(desenfocada(h, 5), "rc2_2.jpg")
    guardar(torcida(h, 15), "rc2_3.jpg")
    g = retrato(("abiertos", "abiertos", "abiertos"), 22)
    guardar(g, "rc3_1.jpg")
    guardar(retrato(("abiertos", "abiertos", "cerrados"), 22), "rc3_2.jpg")
    guardar(movida(g, 14, 2), "rc3_3.jpg")
    # Pizarras y hojas (recortar_pizarra).
    guardar(pegar(pared(semilla=30), capa_pizarra(360, 240), 76, 64, 0, "p01"), "p01.jpg")
    guardar(pegar(pared(semilla=31), capa_pizarra(300, 200), 170, 40, 8, "p02"), "p02.jpg")
    guardar(pegar(pared((200, 196, 190), 32), capa_pizarra(440, 300, "verde"), 36, 30, 0, "p03"), "p03.jpg")
    guardar(pegar(mesa(33), capa_pizarra(240, 300, "hoja"), 150, 50, 15, "p04"), "p04.jpg")
    guardar(pegar(pared(semilla=34), capa_pizarra(420, 260), 200, 70, 0, "p05"), "p05.jpg")
    guardar(cuadro(pared(semilla=35)), "p06.jpg")
    # Pizarras de control.
    guardar(pegar(mesa(40), capa_pizarra(150, 190, "hoja"), 330, 170, -10, "pc1"), "pc1.jpg")
    guardar(pegar(pared(semilla=41), capa_pizarra(340, 220, "verde"), 40, 100, -6, "pc2"), "pc2.jpg")
    guardar(pegar(pared(semilla=42), capa_pizarra(250, 170), 20, 20, 3, "pc3"), "pc3.jpg")
    guardar(ventana(pared(semilla=43)), "pc4.jpg")
    # OCR.
    guardar(pantalla_chat(), "o01.png")
    guardar(hoja_impresa(["La fotosíntesis es el proceso por el que", "las plantas convierten la luz en energía", "química. Ocurre en los cloroplastos y", "libera oxígeno como producto."], 50, -2, "Tema 4. La fotosíntesis"), "o02.jpg")
    guardar(tabla_impresa([("Material", "Cant.", "Precio"), ("Azulejos", "24", "312,00"), ("Cemento cola", "6", "58,20"), ("Grifo lavabo", "1", "89,90"), ("Silicona", "3", "14,85")], 51), "o03.jpg")
    guardar(manuscrito(["Comprar:", "pan y leche", "pilas para el mando", "llamar al dentista"], 52), "o04.jpg")
    guardar(paisaje(53), "o05.jpg")
    # OCR de control.
    guardar(manuscrito(["Martes:", "examen de física", "repasar tema 3", "llevar calculadora"], 60, (20, 20, 20)), "oc1.jpg")
    guardar(pantalla_ajustes(), "oc2.png")
    guardar(tabla_pantalla([("Partida", "Unidades", "Total"), ("Fontanería", "1", "1.250,00"), ("Electricidad", "1", "980,00"), ("Pintura", "85 m2", "722,50"), ("Carpintería", "3", "1.410,00")]), "oc3.png")
    guardar(hoja_impresa(["Querida Marta:", "te escribo para contarte que por fin", "terminamos la reforma de la cocina.", "Ven a verla cuando quieras.", "Un abrazo, Elena"], 61, 1.5), "oc4.jpg")
    (SALIDA / "verdad.txt").write_text("\n".join(VERDAD) + "\n")
    print("\n".join(VERDAD))


if __name__ == "__main__":
    main()
