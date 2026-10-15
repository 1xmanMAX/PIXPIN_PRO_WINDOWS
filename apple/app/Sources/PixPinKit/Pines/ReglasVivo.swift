// Las cuentas del pin en vivo, sin ventanas ni ScreenCaptureKit (lo que en
// Windows es `pixpin-pin/src/remoto.rs` y `pixpin/src/pin_vivo.rs`). Todo
// puro para probarlo en la CI (`PinesVivoTests`).
//
// - El ritmo: 30 fotogramas por segundo mientras la zona cambia y 2 cuando
//   lleva un segundo quieta (lo dice ScreenCaptureKit con `SCFrameStatus`).
// - Donde nace: AL LADO de su zona, no encima (encima taparia justo lo que
//   se quiere seguir usando).
// - Manejar a distancia: llevar un punto del pin (agrandado, encogido) al de
//   la zona que se ve ahi, y decidir si una pulsacion fue un clic o el
//   principio de un arrastre.

import CoreGraphics
import Foundation

/// Que trajo un fotograma de ScreenCaptureKit (`SCFrameStatus`, sin
/// importar el framework en las cuentas).
enum EstadoFotograma: Equatable {
    /// `.complete`: hay pixeles nuevos.
    case nuevo
    /// `.idle`: nada cambio en la zona.
    case quieto
    /// `.blank`, `.suspended`, `.started`, `.stopped`: no cuentan.
    case otro
}

/// El ritmo de la captura: rapido mientras hay cambios, lento si esta
/// quieta. Cambiar el ritmo es una `updateConfiguration` del flujo: solo se
/// pide cuando de verdad cambia.
struct PoliticaFotogramas: Equatable {
    static let rapido = 30
    static let lento = 2
    /// Cuanto tiene que llevar quieta la zona para bajar el ritmo.
    static let esperaParaBajar: TimeInterval = 1

    private(set) var porSegundo = PoliticaFotogramas.rapido
    private var ultimoNuevo: TimeInterval?

    /// Apunta un fotograma. Devuelve el ritmo nuevo si hay que cambiarlo.
    mutating func registrar(_ estado: EstadoFotograma, ahora: TimeInterval) -> Int? {
        switch estado {
        case .nuevo:
            ultimoNuevo = ahora
            guard porSegundo != Self.rapido else { return nil }
            porSegundo = Self.rapido
            return porSegundo
        case .quieto:
            let desde = ultimoNuevo ?? ahora
            if ultimoNuevo == nil { ultimoNuevo = ahora }
            guard porSegundo == Self.rapido, ahora - desde >= Self.esperaParaBajar else { return nil }
            porSegundo = Self.lento
            return porSegundo
        case .otro:
            return nil
        }
    }

    /// Al reanudar se vuelve a empezar rapido.
    mutating func reiniciar() {
        porSegundo = Self.rapido
        ultimoNuevo = nil
    }
}

/// La vista interior del pin (zoom dentro del pin y su desplazamiento). Lo
/// que se ve en `v` del contenido es el punto `(v - d) / escala`.
struct VistaInterior: Equatable {
    var escala: CGFloat
    var dx: CGFloat
    var dy: CGFloat

    static let neutra = VistaInterior(escala: 1, dx: 0, dy: 0)
}

enum ReglasVivo {
    /// Cuanto puede moverse el cursor entre pulsar y soltar, en puntos, para
    /// que siga siendo un clic. Por encima es un arrastre.
    static let holguraClic: CGFloat = 5
    /// Separacion entre la zona y el pin al nacer: que la sombra del pin no
    /// pise el borde de lo que se esta mirando.
    static let hueco: CGFloat = 16

    /// Si entre `pulsado` y `soltado` hubo un clic y no un arrastre.
    static func fueClic(_ pulsado: CGPoint, _ soltado: CGPoint) -> Bool {
        abs(soltado.x - pulsado.x) <= holguraClic && abs(soltado.y - pulsado.y) <= holguraClic
    }

    /// El punto de la ZONA (desde su esquina, en puntos) que se ve en `p`
    /// (desde la misma esquina del contenido del pin). `nil` si cae fuera o
    /// con medidas a cero: un clic asi no se manda a ningun sitio.
    static func puntoEnLaZona(_ p: CGPoint, contenido: CGSize, vista: VistaInterior = .neutra,
                              zona: CGSize) -> CGPoint? {
        guard contenido.width > 0, contenido.height > 0, zona.width > 0, zona.height > 0 else { return nil }
        let escala = vista.escala > 0 && vista.escala.isFinite ? vista.escala : 1
        // Primero se deshace la vista interior, luego el tamano del pin.
        let x = (p.x - vista.dx) / escala * zona.width / contenido.width
        let y = (p.y - vista.dy) / escala * zona.height / contenido.height
        guard x >= 0, y >= 0, x < zona.width, y < zona.height else { return nil }
        return CGPoint(x: x, y: y)
    }

    /// Donde nace el pin en vivo (coordenadas de AppKit, y hacia arriba): a
    /// la derecha de la zona, si no a la izquierda, debajo o encima, dentro
    /// de `trabajo` (la zona visible de la pantalla). Si no cabe en ninguno,
    /// encima de la zona, desde donde el usuario lo arrastra.
    static func sitioJuntoA(_ zona: CGRect, trabajo: CGRect) -> CGRect {
        let w = zona.width, h = zona.height
        let candidatos: [(CGFloat, CGFloat, Bool)] = [
            (zona.maxX + hueco, zona.minY, false), // derecha
            (zona.minX - w - hueco, zona.minY, false), // izquierda
            (zona.minX, zona.minY - h - hueco, true), // debajo
            (zona.minX, zona.maxY + hueco, true), // encima
        ]
        for (x, y, vertical) in candidatos {
            // Se arrima al borde por el eje que NO separa de la zona.
            var r = CGRect(x: x, y: y, width: w, height: h)
            if vertical {
                r.origin.x = ReglasPin.limitar(x, trabajo.minX, max(trabajo.minX, trabajo.maxX - w))
            } else {
                r.origin.y = ReglasPin.limitar(y, trabajo.minY, max(trabajo.minY, trabajo.maxY - h))
            }
            if trabajo.contains(r), !r.intersects(zona) { return r }
        }
        return zona
    }

    /// La zona en coordenadas de su pantalla con el origen ARRIBA a la
    /// izquierda (lo que pide `SCStreamConfiguration.sourceRect`), desde una
    /// zona de AppKit (global, y hacia arriba) y el marco de esa pantalla.
    static func zonaEnPantalla(_ zona: CGRect, pantalla: CGRect) -> CGRect {
        CGRect(x: zona.minX - pantalla.minX, y: pantalla.maxY - zona.maxY,
               width: zona.width, height: zona.height)
    }

    /// Un punto de AppKit (global, y hacia arriba desde la pantalla
    /// principal) en el espacio de Quartz de los `CGEvent` (y hacia abajo
    /// desde arriba de la pantalla principal).
    static func aQuartz(_ p: CGPoint, altoPrincipal: CGFloat) -> CGPoint {
        CGPoint(x: p.x, y: altoPrincipal - p.y)
    }

    /// Los pixeles de la captura de una zona: sus puntos por la escala de la
    /// pantalla, al menos 2x2 (ScreenCaptureKit no acepta menos).
    static func pixeles(_ puntos: CGSize, escala: CGFloat) -> (ancho: Int, alto: Int) {
        let s = max(escala, 1)
        return (max(2, Int((puntos.width * s).rounded())), max(2, Int((puntos.height * s).rounded())))
    }
}
