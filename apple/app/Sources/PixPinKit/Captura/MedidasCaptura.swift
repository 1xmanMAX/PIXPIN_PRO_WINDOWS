// Las cuentas de la barra de abajo de la captura (v2-captura de Windows):
// medidas exactas tecleadas, proporciones 16:9 · 4:3 · 1:1, «Repetir la
// ultima zona» y el color de la lupa en HEX y RGB. Puras a proposito: las
// prueban `CapturaMasTests` sin abrir ventanas.
//
// Las zonas van en PUNTOS de la vista de la captura (origen abajo a la
// izquierda, como AppKit); las medidas que se teclean y se ensenan, en
// PIXELES de la foto, como en Windows. La escala (pixeles por punto) la da
// la propia foto.

import CoreGraphics
import Foundation

/// Las proporciones de la barra (`Proporcion` de `captura2/disposicion.rs`).
public enum ProporcionCaptura: Int, CaseIterable, Sendable {
    case libre, p16x9, p4x3, p1x1

    /// `(ancho, alto)`, o `nil` si es libre.
    public var par: (Int, Int)? {
        switch self {
        case .libre: nil
        case .p16x9: (16, 9)
        case .p4x3: (4, 3)
        case .p1x1: (1, 1)
        }
    }

    public var rotulo: String {
        switch self {
        case .libre: "Libre"
        case .p16x9: "16:9"
        case .p4x3: "4:3"
        case .p1x1: "1:1"
        }
    }
}

public enum MedidasCaptura {
    /// Cinco cifras como mucho por lado (`Cifras` de Windows).
    public static let cifrasMaximas = 5

    // MARK: - Proporciones

    /// El rectangulo desde `ancla` hacia `p` atado a la proporcion: manda el
    /// lado que mas se arrastro (`atado` de `pixpin-ui/src/overlay.rs`).
    /// Libre: el rectangulo entre los dos puntos, sin mas.
    public static func atado(desde ancla: CGPoint, hasta p: CGPoint, proporcion: ProporcionCaptura) -> CGRect {
        guard let par = proporcion.par else { return GeometriaCaptura.rectEntre(ancla, p) }
        let (pa, pb) = par
        let dx = p.x - ancla.x
        let dy = p.y - ancla.y
        let ax = abs(dx)
        let ay = abs(dy)
        let ancho: CGFloat
        let alto: CGFloat
        if ax * CGFloat(pb) >= ay * CGFloat(pa) {
            ancho = ax
            alto = ax * CGFloat(pb) / CGFloat(pa)
        } else {
            alto = ay
            ancho = ay * CGFloat(pa) / CGFloat(pb)
        }
        let x = dx < 0 ? ancla.x - ancho : ancla.x
        let y = dy < 0 ? ancla.y - alto : ancla.y
        return CGRect(x: x, y: y, width: ancho, height: alto)
    }

    /// Ajusta una zona ya elegida a la proporcion, en el sitio: se conserva
    /// la esquina de ARRIBA a la izquierda y el ancho, y se calcula el alto;
    /// si no cabe hacia abajo en `limites`, manda el alto que cabe y se
    /// calcula el ancho (`poner_proporcion` de Windows). Libre: igual.
    public static func ajustar(_ r: CGRect, a proporcion: ProporcionCaptura, limites: CGRect) -> CGRect {
        guard let par = proporcion.par else { return r }
        let (pa, pb) = par
        let r = r.standardized
        var ancho = max(r.width, 1)
        var alto = max(ancho * CGFloat(pb) / CGFloat(pa), 1)
        // Origen abajo: «hacia abajo» es hacia `minY` de los limites.
        let cabeAlto = max(r.maxY - limites.minY, 1)
        if alto > cabeAlto {
            alto = cabeAlto
            ancho = max(alto * CGFloat(pa) / CGFloat(pb), 1)
        }
        let nuevo = CGRect(x: r.minX, y: r.maxY - alto, width: ancho, height: alto)
        return GeometriaCaptura.ajustar(nuevo, a: limites)
    }

    // MARK: - Medidas tecleadas

    /// Lee «ancho × alto» como lo teclea la gente: `800x600`, `800 × 600`,
    /// `800*600`, `800,600` o `800 600`. Solo cifras, de 1 a 5 por lado y
    /// mayores que cero; cualquier otra cosa es `nil`.
    public static func leer(_ texto: String) -> (ancho: Int, alto: Int)? {
        let separadores = CharacterSet(charactersIn: "xX×*,;: ")
        let trozos = texto.trimmingCharacters(in: .whitespacesAndNewlines)
            .components(separatedBy: separadores)
            .filter { !$0.isEmpty }
        guard trozos.count == 2 else { return nil }
        func numero(_ s: String) -> Int? {
            guard !s.isEmpty, s.count <= cifrasMaximas,
                  s.unicodeScalars.allSatisfy({ $0.value >= 48 && $0.value <= 57 }),
                  let v = Int(s), v > 0 else { return nil }
            return v
        }
        guard let a = numero(trozos[0]), let b = numero(trozos[1]) else { return nil }
        return (a, b)
    }

    /// La zona (puntos) que piden unas medidas en PIXELES (`zona_de_medidas`
    /// de Windows): con zona elegida, desde su esquina de arriba a la
    /// izquierda; sin ella, centrada. Siempre dentro de `limites`.
    public static func zona(ancho: Int, alto: Int, escala: CGFloat, seleccion: CGRect?, limites: CGRect) -> CGRect {
        let e = max(escala, 0.01)
        let w = CGFloat(max(ancho, 1)) / e
        let h = CGFloat(max(alto, 1)) / e
        let r: CGRect
        if let s = seleccion?.standardized, !s.isEmpty {
            r = CGRect(x: s.minX, y: s.maxY - h, width: w, height: h)
        } else {
            r = CGRect(x: limites.midX - w / 2, y: limites.midY - h / 2, width: w, height: h)
        }
        return GeometriaCaptura.ajustar(r, a: limites)
    }

    // MARK: - Color de la lupa

    /// `#RRGGBB`, como `texto_color(Hex)` de `pixpin-ui/src/lupa.rs`.
    public static func hex(_ r: UInt8, _ g: UInt8, _ b: UInt8) -> String {
        String(format: "#%02X%02X%02X", Int(r), Int(g), Int(b))
    }

    /// `RGB r g b`, la otra linea de la lupa de Windows.
    public static func rgb(_ r: UInt8, _ g: UInt8, _ b: UInt8) -> String {
        "RGB \(r) \(g) \(b)"
    }

    /// El punto `p` (puntos de la vista) en pixeles de la foto, origen
    /// arriba; `nil` si cae fuera.
    public static func pixel(en p: CGPoint, tamVista: CGSize, ancho: Int, alto: Int) -> (x: Int, y: Int)? {
        guard tamVista.width > 0, tamVista.height > 0, ancho > 0, alto > 0 else { return nil }
        let x = Int((p.x * CGFloat(ancho) / tamVista.width).rounded(.down))
        let y = Int(((tamVista.height - p.y) * CGFloat(alto) / tamVista.height).rounded(.down))
        guard x >= 0, y >= 0, x < ancho, y < alto else { return nil }
        return (x, y)
    }
}

// MARK: - Repetir la ultima zona

/// La ultima zona elegida en cada pantalla, en `UserDefaults` (la clave
/// lleva el numero de la pantalla): `R` la vuelve a elegir aunque la app se
/// haya cerrado. Va en puntos de la vista de esa pantalla.
public struct UltimaZona {
    public let defaults: UserDefaults
    public static let prefijo = "captura.ultimaZona."

    public init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
    }

    public func guardar(_ r: CGRect, pantalla: UInt32) {
        let r = r.standardized
        guard r.width >= 1, r.height >= 1 else { return }
        defaults.set([Double(r.minX), Double(r.minY), Double(r.width), Double(r.height)],
                     forKey: UltimaZona.prefijo + String(pantalla))
    }

    public func leer(pantalla: UInt32) -> CGRect? {
        guard let v = defaults.array(forKey: UltimaZona.prefijo + String(pantalla)) as? [Double], v.count == 4,
              v[2] >= 1, v[3] >= 1 else { return nil }
        return CGRect(x: v[0], y: v[1], width: v[2], height: v[3])
    }
}
