#!/usr/bin/env python3
"""Los catalogos de idioma de PixPin Max: que falta, que sobra y la copia de Apple.

Uso (desde la raiz del repositorio; solo la biblioteca estandar de Python 3):

    python3 herramientas/idiomas.py                 # estado de cada idioma (PinDesk y Plantillas)
    python3 herramientas/idiomas.py faltan fr-FR    # las claves que le faltan
    python3 herramientas/idiomas.py faltan fr-FR --ftl > faltan.ftl
                                                    # lo que falta, en ingles, listo para traducir
    python3 herramientas/idiomas.py apple           # rehace los catalogos de apple/
    python3 herramientas/idiomas.py apple --comprobar
                                                    # falla si la copia de apple/ no esta al dia
    python3 herramientas/idiomas.py --plantillas faltan fr-FR
                                                    # lo mismo con los de PinDesk Plantillas

Hay dos juegos de catalogos: los de PinDesk (`crates/pindesk-store/i18n`) y
los de PinDesk Plantillas (`crates/pindesk-plantillas/i18n`, claves `plt-…`;
van aparte para que la app se lleve sus textos el dia que se separe, ver
`docs/plantillas.md`). `estado` ensena los dos.

La referencia es en-US: cada idioma puede tener menos claves (lo que falta se
ve en ingles), pero no claves que en-US no tenga ("sobran", se quedaron de
una version vieja) ni otras variables (`{ $n }`) que las de en-US. Lo mismo
comprueban los tests de `crates/pindesk-store/src/idioma.rs`; esta herramienta
lo dice con detalle. Ver `docs/idiomas.md`.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
CRATES = RAIZ / "crates" / "pindesk-store" / "i18n"
APPLE = RAIZ / "apple" / "nucleo" / "pindesk-store" / "i18n"
PLANTILLAS = RAIZ / "crates" / "pindesk-plantillas" / "i18n"

# El mismo orden que `Idioma::TODOS` y `build.rs`.
ETIQUETAS = [
    "es-ES", "en-US", "zh-CN", "hi-IN", "ar-SA", "pt-BR", "fr-FR",
    "de-DE", "ja-JP", "ru-RU", "ko-KR", "it-IT", "id-ID", "tr-TR",
]
BASES = ("es-ES", "en-US")

CABECERA = re.compile(r"^(-?[a-zA-Z][a-zA-Z0-9_-]*)\s*=")
VARIABLE = re.compile(r"\$([a-zA-Z][a-zA-Z0-9_-]*)")


class Entrada:
    """Un mensaje o termino: su clave, su texto entero (con las lineas de
    continuacion) y los comentarios `#` que lo preceden."""

    def __init__(self, clave: str, lineas: list[str], comentario: list[str]):
        self.clave = clave
        self.lineas = lineas
        self.comentario = comentario

    @property
    def texto(self) -> str:
        return "".join(self.lineas)

    @property
    def variables(self) -> frozenset[str]:
        return frozenset(VARIABLE.findall(self.texto))


def leer(ruta: Path) -> tuple[dict[str, Entrada], list[tuple[str, str]]]:
    """Las entradas de un .ftl, y el orden del fichero como una lista de
    `("clave", clave)` y `("grupo", linea)` (los comentarios `##`)."""
    entradas: dict[str, Entrada] = {}
    orden: list[tuple[str, str]] = []
    if not ruta.is_file():
        return entradas, orden
    comentario: list[str] = []
    actual: Entrada | None = None
    for linea in ruta.read_text(encoding="utf-8").splitlines(keepends=True):
        # Continua el mensaje lo sangrado y tambien un `}` en la columna 0,
        # que cierra un selector (Fluent lo admite).
        if actual is not None and linea.strip() and (linea[:1] in (" ", "\t", "}")):
            actual.lineas.append(linea)
            continue
        actual = None
        m = CABECERA.match(linea)
        if m:
            actual = Entrada(m.group(1), [linea], comentario)
            comentario = []
            entradas[actual.clave] = actual
            orden.append(("clave", actual.clave))
        elif linea.startswith("##"):
            orden.append(("grupo", linea))
            comentario = []
        elif linea.startswith("#"):
            comentario.append(linea)
        else:
            comentario = []
    return entradas, orden


def catalogo(raiz: Path, etiqueta: str) -> Path:
    return raiz / etiqueta / "main.ftl"


def revisar(referencia: dict[str, Entrada], propio: dict[str, Entrada]):
    faltan = [c for c in referencia if c not in propio]
    sobran = [c for c in propio if c not in referencia]
    variables = [
        (c, sorted(propio[c].variables), sorted(referencia[c].variables))
        for c in propio
        if c in referencia and propio[c].variables != referencia[c].variables
    ]
    return faltan, sobran, variables


def orden_estado(args) -> int:
    if args.apple:
        juegos = [("Apple", APPLE)]
    elif args.plantillas:
        juegos = [("PinDesk Plantillas", PLANTILLAS)]
    else:
        juegos = [("PinDesk", CRATES), ("PinDesk Plantillas", PLANTILLAS)]
    malos = 0
    for i, (nombre, raiz) in enumerate(juegos):
        if len(juegos) > 1:
            print(("\n" if i else "") + f"## {nombre} ({raiz.relative_to(RAIZ).as_posix()})")
        malos += estado_de(raiz)
    if args.estricto and malos:
        return 1
    return 0


def estado_de(raiz: Path) -> int:
    en, _ = leer(catalogo(raiz, "en-US"))
    malos = 0
    print(f"{'idioma':8} {'hechas':>7} {'faltan':>7} {'sobran':>7} {'vars':>5}  cobertura")
    for etiqueta in ETIQUETAS:
        ruta = catalogo(raiz, etiqueta)
        if not ruta.is_file():
            print(f"{etiqueta:8} {'-':>7} {len(en):>7} {'-':>7} {'-':>5}  sin fichero (todo en ingles)")
            continue
        propio, _ = leer(ruta)
        faltan, sobran, variables = revisar(en, propio)
        hechas = len(propio) - len(sobran)
        malos += len(sobran) + len(variables)
        print(
            f"{etiqueta:8} {hechas:>7} {len(faltan):>7} {len(sobran):>7} {len(variables):>5}"
            f"  {100 * hechas / max(len(en), 1):5.1f} %"
        )
        for c in sobran:
            print(f"    sobra «{c}» (no esta en en-US)")
        for c, propias, de_en in variables:
            print(f"    «{c}» usa {propias} y en-US {de_en}")
    return malos


def orden_faltan(args) -> int:
    raiz = APPLE if args.apple else PLANTILLAS if args.plantillas else CRATES
    en, orden = leer(catalogo(raiz, "en-US"))
    propio, _ = leer(catalogo(raiz, args.etiqueta))
    faltan, sobran, _ = revisar(en, propio)
    if args.ftl:
        print(f"# Lo que falta en {args.etiqueta}, en ingles: traducir y anadir a")
        print(f"# {catalogo(raiz, args.etiqueta).relative_to(RAIZ).as_posix()}.")
        print("# Reglas: docs/idiomas.md (marcas, `{ $variables }`, plurales de CLDR).\n")
        que = set(faltan)
        for tipo, valor in orden:
            if tipo == "clave" and valor in que:
                e = en[valor]
                sys.stdout.write("".join(e.comentario) + e.texto)
        return 0
    for c in faltan:
        print(c)
    for c in sobran:
        print(f"# sobra: {c}")
    return 0


def generar_apple(etiqueta: str) -> tuple[str | None, dict[str, int]]:
    """El catalogo de Apple de un idioma: las claves de Apple (las de su
    en-US, en su orden) con la traduccion de `crates/`. Una clave que solo
    tiene Apple, o cuyas variables en Apple no son las de la traduccion, se
    queda sin traducir (se ve en ingles)."""
    apple_en, orden = leer(catalogo(APPLE, "en-US"))
    traducido, _ = leer(catalogo(CRATES, etiqueta))
    cuenta = {"copiadas": 0, "sin_traducir": 0, "solo_apple": 0, "otras_variables": 0}
    if not traducido:
        return None, cuenta
    partes = [
        f"# {etiqueta}: generado por `herramientas/idiomas.py apple` a partir de\n",
        f"# crates/pindesk-store/i18n/{etiqueta}/main.ftl, con las claves de Apple.\n",
        "# No se edita a mano: se traduce en crates/ y se vuelve a generar.\n",
    ]
    grupo_pendiente: str | None = None
    for tipo, valor in orden:
        if tipo == "grupo":
            grupo_pendiente = valor
            continue
        e = traducido.get(valor)
        if e is None:
            cuenta["solo_apple" if valor not in leer_crates_en() else "sin_traducir"] += 1
            continue
        if e.variables != apple_en[valor].variables:
            cuenta["otras_variables"] += 1
            continue
        if grupo_pendiente is not None:
            partes.append("\n" + grupo_pendiente)
            grupo_pendiente = None
        partes.append(e.texto)
        cuenta["copiadas"] += 1
    return "".join(partes), cuenta


_CRATES_EN: dict[str, Entrada] | None = None


def leer_crates_en() -> dict[str, Entrada]:
    global _CRATES_EN
    if _CRATES_EN is None:
        _CRATES_EN, _ = leer(catalogo(CRATES, "en-US"))
    return _CRATES_EN


def orden_apple(args) -> int:
    apple_en, _ = leer(catalogo(APPLE, "en-US"))
    solo_apple = [c for c in apple_en if c not in leer_crates_en()]
    if solo_apple:
        print(
            f"{len(solo_apple)} claves solo estan en Apple (se quedan con su es/en y "
            "en los demas idiomas salen en ingles):"
        )
        for c in solo_apple:
            print(f"    {c}")
    distinto = False
    for etiqueta in ETIQUETAS:
        if etiqueta in BASES:
            continue
        texto, cuenta = generar_apple(etiqueta)
        destino = catalogo(APPLE, etiqueta)
        if texto is None:
            if destino.is_file():
                print(f"{etiqueta}: sin catalogo en crates/, pero si en apple/")
                distinto = True
            continue
        actual = destino.read_text(encoding="utf-8") if destino.is_file() else None
        if actual == texto:
            estado = "al dia"
        elif args.comprobar:
            estado = "DESACTUALIZADO"
            distinto = True
        else:
            destino.parent.mkdir(parents=True, exist_ok=True)
            destino.write_text(texto, encoding="utf-8")
            estado = "escrito"
        print(
            f"{etiqueta}: {estado}; {cuenta['copiadas']} copiadas, "
            f"{cuenta['sin_traducir']} sin traducir, {cuenta['solo_apple']} solo de Apple, "
            f"{cuenta['otras_variables']} con otras variables en Apple"
        )
    return 1 if (args.comprobar and distinto) else 0


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--apple", action="store_true", help="mirar los catalogos de apple/ en vez de crates/")
    p.add_argument("--plantillas", action="store_true", help="solo los de PinDesk Plantillas")
    sub = p.add_subparsers(dest="orden")
    e = sub.add_parser("estado", help="cuanto tiene cada idioma (lo de por defecto)")
    e.add_argument("--estricto", action="store_true", help="salir con 1 si algo sobra o cambia de variables")
    f = sub.add_parser("faltan", help="las claves que le faltan a un idioma")
    f.add_argument("etiqueta", choices=[x for x in ETIQUETAS if x != "en-US"])
    f.add_argument("--ftl", action="store_true", help="sacarlas como .ftl en ingles, para traducir")
    a = sub.add_parser("apple", help="rehacer los catalogos de apple/ desde crates/")
    a.add_argument("--comprobar", action="store_true", help="no escribir; salir con 1 si algo cambiaria")
    args = p.parse_args()
    if args.orden == "faltan":
        return orden_faltan(args)
    if args.orden == "apple":
        return orden_apple(args)
    if args.orden is None:
        args.estricto = False
    return orden_estado(args)


if __name__ == "__main__":
    sys.exit(main())
