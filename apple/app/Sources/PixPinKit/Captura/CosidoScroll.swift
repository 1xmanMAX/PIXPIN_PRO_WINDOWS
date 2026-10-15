// La matematica del cosido de la captura con scroll: pura, sin pantalla.
//
// Es la MISMA receta que Windows y Android (`pixpin-codec/src/cosido.rs`,
// puerto de `ScrollMatcher`/`ScrollPlan`): cada fila del fotograma se
// resume en una firma (la suma de las luminancias de uno de cada 5
// pixeles) y el desplazamiento entre dos fotogramas se busca sobre ese
// vector, rechazando lo dudoso (bandas lisas, encajes ambiguos). Las
// franjas fijas (una barra que no se mueve) se detectan con el primer par
// que se desplaza: la cabecera entra una vez y el pie se anade al final.
//
// Se reescribe en Swift (y no se cruza el puente) porque son unas decenas
// de lineas y el cruce copiaria cada fotograma; las constantes son las de
// Rust para que los tres sistemas cosan igual.
//
// Rendimiento: de cada fotograma solo se guardan las FILAS NUEVAS (copiadas
// a un `Data` propio); el fotograma se suelta en cuanto se ha mirado. La
// imagen final se monta una vez al terminar, con tope de alto.

import CoreGraphics
import Foundation

public enum CosidoScroll {
    public static let sinEncaje = -1
    public static let filasCola = 48
    public static let maxFilasCola = 384
    public static let pasoMuestreo = 5
    public static let tolerancia: Int64 = 40
    public static let variacionMinima: Int64 = 300
    static let separacionMinima = 4
    static let factorAmbiguedad: Int64 = 2
    static let fraccionFijaMaxima: Double = 0.4
    /// Alto maximo de la imagen cosida (D76 de Windows).
    public static let altoMaximo = 20_000

    /// Firma de una fila en BGRA (el orden de los mapas de bits de Mac:
    /// `byteOrder32Little` + `premultipliedFirst`), muestreando uno de cada
    /// `paso` pixeles. Misma cuenta entera que `firma_de_fila` de Rust.
    public static func firmaDeFila(_ fila: UnsafePointer<UInt8>, ancho: Int, paso: Int = pasoMuestreo) -> Int64 {
        let paso = max(paso, 1)
        var suma: Int64 = 0
        var x = 0
        while x < ancho {
            let b = Int64(fila[x * 4])
            let g = Int64(fila[x * 4 + 1])
            let r = Int64(fila[x * 4 + 2])
            suma += (r * 77 + g * 151 + b * 28) >> 8
            x += paso
        }
        return suma
    }

    /// Las firmas de todas las filas de un mapa de bits BGRA.
    public static func firmas(_ base: UnsafePointer<UInt8>, ancho: Int, alto: Int, bytesPorFila: Int) -> [Int64] {
        (0 ..< max(alto, 0)).map { firmaDeFila(base + $0 * bytesPorFila, ancho: ancho) }
    }

    /// Donde vuelven a aparecer, dentro de `marco`, las filas de `cola`.
    /// `sinEncaje` si no encaja con confianza o encaja en varios sitios.
    public static func encontrarDesplazamiento(cola: [Int64], marco: [Int64], tolerancia: Int64 = tolerancia) -> Int {
        guard !cola.isEmpty, marco.count >= cola.count else { return sinEncaje }
        let n = marco.count - cola.count + 1
        var puntuaciones = [Int64](repeating: 0, count: n)
        var mejor = sinEncaje
        var mejorPuntuacion = Int64.max
        for d in 0 ..< n {
            var p: Int64 = 0
            for k in 0 ..< cola.count {
                p += abs(cola[k] - marco[d + k])
            }
            puntuaciones[d] = p
            if p < mejorPuntuacion {
                mejorPuntuacion = p
                mejor = d
            }
        }
        guard mejor != sinEncaje else { return sinEncaje }
        let mediaMejor = mejorPuntuacion / Int64(cola.count)
        guard mediaMejor <= tolerancia else { return sinEncaje }
        var segunda = Int64.max
        for (d, p) in puntuaciones.enumerated() where abs(d - mejor) >= separacionMinima && p < segunda {
            segunda = p
        }
        guard segunda != Int64.max else { return mejor }
        let mediaSegunda = segunda / Int64(cola.count)
        return mediaSegunda >= mediaMejor * factorAmbiguedad + tolerancia / 2 ? mejor : sinEncaje
    }

    /// Una banda sin textura encaja en cualquier sitio: no sirve.
    public static func esLisa(_ cola: ArraySlice<Int64>, variacionMinima: Int64 = variacionMinima) -> Bool {
        guard cola.count >= 2, let mn = cola.min(), let mx = cola.max() else { return true }
        return mx - mn < variacionMinima
    }

    /// Filas iguales arriba (cabecera) y abajo (pie) entre dos fotogramas.
    /// Si todo es igual (no se movio): `(0, 0)`.
    public static func franjasFijas(_ anterior: [Int64], _ actual: [Int64], tolerancia: Int64 = tolerancia) -> (Int, Int) {
        let n = min(anterior.count, actual.count)
        guard n > 0 else { return (0, 0) }
        let tope = Int(Double(n) * fraccionFijaMaxima)
        var cabecera = 0
        while cabecera < tope, abs(anterior[cabecera] - actual[cabecera]) <= tolerancia { cabecera += 1 }
        let la = anterior.count
        let lb = actual.count
        var pie = 0
        while pie < tope, abs(anterior[la - 1 - pie] - actual[lb - 1 - pie]) <= tolerancia { pie += 1 }
        if cabecera >= tope, pie >= tope { return (0, 0) }
        return (cabecera, pie)
    }

    /// La banda de referencia del final; si es lisa se agranda.
    static func elegirCola(_ firmas: ArraySlice<Int64>) -> [Int64] {
        var filas = filasCola
        while filas < maxFilasCola, filas < firmas.count {
            let candidata = firmas.suffix(filas)
            if !esLisa(candidata) { return Array(candidata) }
            filas *= 2
        }
        return Array(firmas.suffix(min(filas, firmas.count)))
    }

    public enum Resultado: Equatable, Sendable {
        case primero, anadido, sinMovimiento, incierto, lleno
    }

    /// Que franja del fotograma hay que quedarse.
    public struct Orden: Equatable, Sendable {
        public var resultado: Resultado
        public var desde: Int
        public var filas: Int
        static func solo(_ r: Resultado) -> Orden { Orden(resultado: r, desde: 0, filas: 0) }
    }

    /// Que hacer con cada fotograma, sin tocar un pixel (`Plan` de Rust).
    public struct Plan {
        public let altoMaximo: Int
        public private(set) var alto = 0
        private var cola: [Int64] = []
        private var anterior: [Int64] = []
        private var franjas: (Int, Int)?

        public init(altoMaximo: Int = CosidoScroll.altoMaximo) {
            self.altoMaximo = altoMaximo
        }

        public var vacio: Bool { alto == 0 }
        public var pie: Int { franjas?.1 ?? 0 }

        public mutating func plan(_ firmas: [Int64]) -> Orden {
            let filas = firmas.count
            guard filas > CosidoScroll.filasCola else { return .solo(.incierto) }
            if vacio {
                guard filas <= altoMaximo else { return .solo(.lleno) }
                alto = filas
                anterior = firmas
                cola = CosidoScroll.elegirCola(firmas[...])
                return Orden(resultado: .primero, desde: 0, filas: filas)
            }
            var fijas = (0, 0)
            if let f = franjas {
                fijas = f
            } else {
                fijas = CosidoScroll.franjasFijas(anterior, firmas)
                if fijas.0 != 0 || fijas.1 != 0 {
                    franjas = fijas
                    let fin = max(anterior.count - fijas.1, 0)
                    cola = CosidoScroll.elegirCola(anterior[min(fijas.0, fin) ..< fin])
                }
            }
            let cabecera = fijas.0
            let pie = fijas.1
            let ini = min(cabecera, filas)
            let fin = max(filas - pie, ini)
            let contenido = Array(firmas[ini ..< fin])
            let d = CosidoScroll.encontrarDesplazamiento(cola: cola, marco: contenido)
            guard d != CosidoScroll.sinEncaje else { return .solo(.incierto) }
            let nuevoDesde = d + cola.count
            guard nuevoDesde < contenido.count else { return .solo(.sinMovimiento) }
            let nuevas = contenido.count - nuevoDesde
            guard alto + nuevas <= altoMaximo else { return .solo(.lleno) }
            alto += nuevas
            anterior = firmas
            cola = CosidoScroll.elegirCola(contenido[...])
            return Orden(resultado: .anadido, desde: cabecera + nuevoDesde, filas: nuevas)
        }
    }

    /// Va guardando las filas nuevas de cada fotograma (BGRA, `ancho * 4`
    /// bytes por fila, sin relleno) y al terminar las junta en una imagen.
    public struct Cosedor {
        public let ancho: Int
        public private(set) var plan: Plan
        private var tiras: [Data] = []
        private var ultimoPie = Data()

        public init(ancho: Int, altoMaximo: Int = CosidoScroll.altoMaximo) {
            self.ancho = ancho
            plan = Plan(altoMaximo: altoMaximo)
        }

        public var vacio: Bool { tiras.isEmpty }
        public var alto: Int { plan.alto }

        /// Mira un fotograma (BGRA) y se queda con sus filas nuevas. El
        /// fotograma se puede soltar en cuanto vuelve.
        public mutating func anadir(_ base: UnsafePointer<UInt8>, ancho: Int, alto: Int, bytesPorFila: Int) -> Resultado {
            guard ancho == self.ancho, alto > 0 else { return .incierto }
            let f = CosidoScroll.firmas(base, ancho: ancho, alto: alto, bytesPorFila: bytesPorFila)
            let orden = plan.plan(f)
            let fila = ancho * 4
            func filas(_ desde: Int, _ cuantas: Int) -> Data {
                var d = Data(count: cuantas * fila)
                d.withUnsafeMutableBytes { destino in
                    guard let p = destino.baseAddress else { return }
                    for i in 0 ..< cuantas {
                        memcpy(p + i * fila, base + (desde + i) * bytesPorFila, fila)
                    }
                }
                return d
            }
            if orden.filas > 0 { tiras.append(filas(orden.desde, orden.filas)) }
            if orden.resultado == .primero || orden.resultado == .anadido {
                let pie = plan.pie
                ultimoPie = pie > 0 && pie < alto ? filas(alto - pie, pie) : Data()
            }
            return orden.resultado
        }

        /// Los bytes de la imagen final y su alto; `nil` si no hay nada.
        public func terminarBytes() -> (bytes: Data, alto: Int)? {
            guard !tiras.isEmpty else { return nil }
            let fila = ancho * 4
            let pie = plan.pie
            var total = Data()
            total.reserveCapacity(tiras.reduce(0) { $0 + $1.count } + ultimoPie.count)
            for (i, t) in tiras.enumerated() {
                // La primera tira entro con el pie (aun no se conocia): fuera.
                if i == 0, pie > 0, tiras.count > 1, t.count > pie * fila {
                    total.append(t.prefix(t.count - pie * fila))
                } else {
                    total.append(t)
                }
            }
            if tiras.count > 1 { total.append(ultimoPie) }
            return (total, total.count / max(fila, 1))
        }

        /// La imagen final (BGRA premultiplicada, sRGB).
        public func terminar() -> CGImage? {
            guard let hecho = terminarBytes(), hecho.alto > 0,
                  let proveedor = CGDataProvider(data: hecho.bytes as CFData) else { return nil }
            let info = CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedFirst.rawValue
                | CGBitmapInfo.byteOrder32Little.rawValue)
            return CGImage(width: ancho, height: hecho.alto, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: ancho * 4,
                           space: CGColorSpace(name: CGColorSpace.sRGB) ?? CGColorSpaceCreateDeviceRGB(),
                           bitmapInfo: info, provider: proveedor, decode: nil, shouldInterpolate: false,
                           intent: .defaultIntent)
        }
    }
}
