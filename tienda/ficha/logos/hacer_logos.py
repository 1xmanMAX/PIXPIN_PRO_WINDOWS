#!/usr/bin/env python3
"""Logos de la ficha de la Store a partir del icono de la app.

El icono (`apps/pindesk/recursos/pindesk.ico`) es un aro azul #2A7AE2 con
el centro gris muy claro #F0F0F5: radio exterior 108.5/256 e interior 56/256,
centrado. Aqui se vuelve a dibujar a la medida (con 4x de sobremuestreo) en
vez de agrandar el PNG de 256 px, para que salga nitido.

Deja en esta carpeta:
- `logo-300x300.png`: «Store logos → 1:1 app icon (300×300)», transparente.
- `logo-1080x1080.png` y `logo-2160x2160.png`: «1:1 box art», fondo azul suave.
- `poster-720x1080.png` y `poster-1440x2160.png`: «2:3 poster art».

Uso: `python tienda/ficha/logos/hacer_logos.py` (necesita Pillow).
"""

from pathlib import Path

from PIL import Image, ImageDraw

AQUI = Path(__file__).resolve().parent
AZUL = (42, 122, 226, 255)
CENTRO = (240, 240, 245, 255)
FONDO = (232, 240, 252, 255)
R_FUERA = 108.5 / 256
R_DENTRO = 56 / 256
SOBRE = 4


def aro(lado: int) -> Image.Image:
    """El icono, transparente, ocupando el cuadrado de lado `lado`."""
    g = lado * SOBRE
    img = Image.new("RGBA", (g, g), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    c = g / 2
    for r, color in ((R_FUERA * g, AZUL), (R_DENTRO * g, CENTRO)):
        d.ellipse((c - r, c - r, c + r, c + r), fill=color)
    return img.resize((lado, lado), Image.LANCZOS)


def sobre_fondo(ancho: int, alto: int, fraccion: float) -> Image.Image:
    """El icono centrado sobre el fondo, de `fraccion` del lado menor."""
    img = Image.new("RGBA", (ancho, alto), FONDO)
    lado = round(min(ancho, alto) * fraccion)
    img.alpha_composite(aro(lado), ((ancho - lado) // 2, (alto - lado) // 2))
    return img.convert("RGB")


def main() -> None:
    aro(300).save(AQUI / "logo-300x300.png", optimize=True)
    for lado in (1080, 2160):
        sobre_fondo(lado, lado, 0.62).save(AQUI / f"logo-{lado}x{lado}.png", optimize=True)
    for ancho, alto in ((720, 1080), (1440, 2160)):
        sobre_fondo(ancho, alto, 0.7).save(AQUI / f"poster-{ancho}x{alto}.png", optimize=True)
    for p in sorted(AQUI.glob("*.png")):
        with Image.open(p) as im:
            print(f"{p.name}: {im.width}x{im.height} {im.mode}")


if __name__ == "__main__":
    main()
