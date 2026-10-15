// Las guias azules para alinear un pin con los demas mientras se arrastra
// (lo que en Windows es `pixpin-pin/src/guias.rs`).
//
// Mientras se mueve un pin, si uno de sus bordes queda a tiro del de otro
// pin (el mismo borde, o pegado a el con la separacion de 16 puntos) sale
// una linea azul que lo dice. Al SOLTAR se pega a esa guia (el iman suave):
// durante el arrastre no se pega, porque pegarse a media pasada pelea con el
// raton y se siente como un tiron. Opcion mientras se arrastra = sin guias
// ni iman (el Alt de Windows).
//
// La cuenta es pura y se prueba aparte (`GuiasTests`); las lineas son dos
// paneles macizos de 2 puntos que no recogen el raton y no salen en las
// capturas. Se crean la primera vez y se esconden al acabar.

import AppKit

/// Una linea de guia, en coordenadas de pantalla.
struct GuiaPin: Equatable {
    /// Vertical (alinea en x) u horizontal (alinea en y).
    var vertical: Bool
    /// La x de una vertical o la y de una horizontal.
    var pos: CGFloat
    /// De donde a donde va, en el otro eje.
    var desde: CGFloat
    var hasta: CGFloat
}

/// Donde quedaria el pin pegado a sus guias, y las guias.
struct AjusteGuias: Equatable {
    var rect: CGRect
    var guias: [GuiaPin]
}

enum ReglasGuias {
    /// A cuanto se nota una guia, en puntos.
    static let umbral: CGFloat = 8
    /// La separacion entre pines que se ofrece pegar, en puntos.
    static let separacion: CGFloat = 16

    /// El mejor candidato de un eje: el que menos mueve, si mueve menos que
    /// `umbral`. Con un empate gana el primero (como `min_by_key` en Rust).
    private static func mejor(_ candidatos: [(CGFloat, GuiaPin)], umbral: CGFloat) -> (CGFloat, GuiaPin)? {
        var elegido: (CGFloat, GuiaPin)?
        for c in candidatos where abs(c.0) <= umbral {
            if let e = elegido, abs(e.0) <= abs(c.0) { continue }
            elegido = c
        }
        return elegido
    }

    /// Busca las guias de `rect` frente a `otros`: en cada eje, el borde mas
    /// cercano a menos de `umbral`. Dos clases de guia: el mismo borde
    /// (izquierda con izquierda, arriba con arriba...) o pegado al de al lado
    /// con `separacion` en medio.
    static func alinear(_ rect: CGRect, otros: [CGRect],
                        umbral: CGFloat = umbral, separacion: CGFloat = separacion) -> AjusteGuias {
        let r = rect.standardized
        var enX: [(CGFloat, GuiaPin)] = []
        var enY: [(CGFloat, GuiaPin)] = []
        for o in otros.map(\.standardized) {
            // La linea vertical cubre a los dos pines.
            let dv = min(r.minY, o.minY), hv = max(r.maxY, o.maxY)
            func v(_ pos: CGFloat) -> GuiaPin { GuiaPin(vertical: true, pos: pos, desde: dv, hasta: hv) }
            enX += [
                (o.minX - r.minX, v(o.minX)),
                (o.maxX - r.maxX, v(o.maxX)),
                (o.minX - r.maxX, v(o.minX)),
                (o.maxX - r.minX, v(o.maxX)),
                // Separados: la guia marca el borde del otro.
                (o.maxX + separacion - r.minX, v(o.maxX)),
                (o.minX - separacion - r.maxX, v(o.minX)),
            ]
            let dh = min(r.minX, o.minX), hh = max(r.maxX, o.maxX)
            func h(_ pos: CGFloat) -> GuiaPin { GuiaPin(vertical: false, pos: pos, desde: dh, hasta: hh) }
            enY += [
                (o.minY - r.minY, h(o.minY)),
                (o.maxY - r.maxY, h(o.maxY)),
                (o.minY - r.maxY, h(o.minY)),
                (o.maxY - r.minY, h(o.maxY)),
                (o.maxY + separacion - r.minY, h(o.maxY)),
                (o.minY - separacion - r.maxY, h(o.minY)),
            ]
        }
        var ajuste = AjusteGuias(rect: r, guias: [])
        if case let (dx, g)? = mejor(enX, umbral: umbral) {
            ajuste.rect.origin.x += dx
            ajuste.guias.append(g)
        }
        if case let (dy, g)? = mejor(enY, umbral: umbral) {
            ajuste.rect.origin.y += dy
            ajuste.guias.append(g)
        }
        // La guia tiene que cubrir tambien el pin YA pegado.
        let pegado = ajuste.rect
        for i in ajuste.guias.indices {
            if ajuste.guias[i].vertical {
                ajuste.guias[i].desde = min(ajuste.guias[i].desde, pegado.minY)
                ajuste.guias[i].hasta = max(ajuste.guias[i].hasta, pegado.maxY)
            } else {
                ajuste.guias[i].desde = min(ajuste.guias[i].desde, pegado.minX)
                ajuste.guias[i].hasta = max(ajuste.guias[i].hasta, pegado.maxX)
            }
        }
        return ajuste
    }

    /// El rectangulo de la ventanita que dibuja una guia de `grosor`.
    static func marco(de g: GuiaPin, grosor: CGFloat) -> CGRect {
        let largo = max(1, g.hasta - g.desde)
        if g.vertical {
            return CGRect(x: g.pos - grosor / 2, y: g.desde, width: grosor, height: largo)
        }
        return CGRect(x: g.desde, y: g.pos - grosor / 2, width: largo, height: grosor)
    }
}

/// Las dos lineas en pantalla (vertical y horizontal).
@MainActor
final class LineasGuia {
    static let compartidas = LineasGuia()
    /// El azul de acento de Windows (#0A84FF).
    static let azul = NSColor(srgbRed: 10 / 255, green: 132 / 255, blue: 1, alpha: 0.92)
    private var lineas: [NSPanel?] = [nil, nil]

    private func linea(_ i: Int) -> NSPanel {
        if let l = lineas[i] { return l }
        let l = NSPanel(contentRect: CGRect(x: 0, y: 0, width: 2, height: 2),
                        styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: true)
        l.isReleasedWhenClosed = false
        l.ignoresMouseEvents = true
        l.hasShadow = false
        l.isOpaque = true
        l.backgroundColor = LineasGuia.azul
        l.level = .floating
        l.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .transient, .ignoresCycle]
        // No sale en ninguna captura ni en el pin en vivo.
        l.sharingType = .none
        lineas[i] = l
        return l
    }

    /// Ensena estas guias (y esconde las que sobren).
    func ensenar(_ guias: [GuiaPin]) {
        for i in 0..<2 {
            if let g = guias.first(where: { $0.vertical == (i == 0) }) {
                let l = linea(i)
                l.setFrame(ReglasGuias.marco(de: g, grosor: 2), display: false)
                l.orderFrontRegardless()
            } else {
                lineas[i]?.orderOut(nil)
            }
        }
    }

    func esconder() { ensenar([]) }
}
