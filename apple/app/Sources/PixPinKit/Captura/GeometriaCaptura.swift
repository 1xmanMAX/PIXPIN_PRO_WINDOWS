// Las cuentas de la captura que no tocan la pantalla: elegir la zona, pasar
// de puntos de AppKit (origen abajo a la izquierda) a pixeles de la foto
// (origen arriba a la izquierda), donde poner la barra y el nombre del
// fichero. Puras a proposito: las prueba `CapturaTests` sin abrir ventanas.

import CoreGraphics
import Foundation

public enum GeometriaCaptura {
    /// Por debajo de esto (en puntos) un arrastre cuenta como clic.
    public static let arrastreMinimo: CGFloat = 3

    /// La zona entre dos puntos, se arrastre hacia donde se arrastre.
    public static func rectEntre(_ a: CGPoint, _ b: CGPoint) -> CGRect {
        CGRect(x: min(a.x, b.x), y: min(a.y, b.y), width: abs(a.x - b.x), height: abs(a.y - b.y))
    }

    /// `r` dentro de `limites`; `.zero` si queda fuera del todo.
    public static func ajustar(_ r: CGRect, a limites: CGRect) -> CGRect {
        let dentro = r.standardized.intersection(limites)
        return dentro.isNull ? .zero : dentro
    }

    /// Si el raton se ha movido lo bastante para que sea un arrastre.
    public static func esArrastre(desde a: CGPoint, hasta b: CGPoint) -> Bool {
        abs(a.x - b.x) >= arrastreMinimo || abs(a.y - b.y) >= arrastreMinimo
    }

    /// Una zona de la vista (puntos, origen abajo a la izquierda, del tamano
    /// `tamVista`) en pixeles de la foto (`ancho` x `alto`, origen arriba a la
    /// izquierda), redondeada hacia fuera para no perder el borde y recortada
    /// a la foto. La escala sale de la propia foto y no de
    /// `backingScaleFactor`: asi vale aunque el sistema la haya entregado a
    /// otra resolucion.
    public static func rectEnPixeles(_ r: CGRect, tamVista: CGSize, ancho: Int, alto: Int) -> CGRect {
        guard tamVista.width > 0, tamVista.height > 0, ancho > 0, alto > 0 else { return .zero }
        let r = r.standardized
        let sx = CGFloat(ancho) / tamVista.width
        let sy = CGFloat(alto) / tamVista.height
        // Un poco de holgura: 100.0000001 * 2 no debe dar 201.
        let abajo = { (v: CGFloat) in (v + 1e-6).rounded(.down) }
        let arriba = { (v: CGFloat) in (v - 1e-6).rounded(.up) }
        let x0 = abajo(r.minX * sx)
        let x1 = arriba(r.maxX * sx)
        let y0 = abajo((tamVista.height - r.maxY) * sy)
        let y1 = arriba((tamVista.height - r.minY) * sy)
        let px = CGRect(x: x0, y: y0, width: x1 - x0, height: y1 - y0)
        return ajustar(px, a: CGRect(x: 0, y: 0, width: ancho, height: alto))
    }

    /// Un marco en coordenadas de Core Graphics (las de `CGWindowList` y
    /// ScreenCaptureKit: origen arriba a la izquierda de la pantalla
    /// principal) en coordenadas globales de AppKit (origen abajo a la
    /// izquierda de la pantalla principal).
    public static func desdeCoordenadasCG(_ r: CGRect, altoPantallaPrincipal: CGFloat) -> CGRect {
        CGRect(x: r.minX, y: altoPantallaPrincipal - r.maxY, width: r.width, height: r.height)
    }

    /// Un marco global de AppKit en las coordenadas de la vista que cubre
    /// la pantalla cuyo marco empieza en `origenPantalla`.
    public static func enPantalla(_ r: CGRect, origenPantalla: CGPoint) -> CGRect {
        r.offsetBy(dx: -origenPantalla.x, dy: -origenPantalla.y)
    }

    /// La primera ventana (de delante a atras) que tiene el punto.
    public static func ventanaBajo(_ p: CGPoint, en marcos: [CGRect]) -> CGRect? {
        marcos.first { $0.contains(p) }
    }

    /// Donde va la barra de acciones (su esquina abajo a la izquierda):
    /// debajo de la zona y alineada a su derecha; si no cabe, encima; si
    /// tampoco, dentro, abajo. Nunca fuera de `limites`.
    public static func posicionBarra(seleccion s: CGRect, tamBarra t: CGSize, limites l: CGRect,
                                     margen: CGFloat = 8) -> CGPoint {
        var y = s.minY - margen - t.height
        if y < l.minY + margen {
            y = s.maxY + margen
            if y + t.height > l.maxY - margen { y = max(s.minY + margen, l.minY + margen) }
        }
        var x = s.maxX - t.width
        x = min(x, l.maxX - margen - t.width)
        x = max(x, l.minX + margen)
        return CGPoint(x: x, y: y)
    }

    /// Lo que ensena la lupa: un cuadrado de `lado` puntos alrededor de `p`,
    /// en la unidad de `CALayer.contentsRect` (0...1, origen abajo a la
    /// izquierda como la capa en macOS).
    public static func rectDeLupa(en p: CGPoint, lado: CGFloat, tamVista: CGSize) -> CGRect {
        guard tamVista.width > 0, tamVista.height > 0 else { return CGRect(x: 0, y: 0, width: 1, height: 1) }
        return CGRect(x: (p.x - lado / 2) / tamVista.width, y: (p.y - lado / 2) / tamVista.height,
                      width: lado / tamVista.width, height: lado / tamVista.height)
    }

    /// Si un recorte es tan pequeno frente a la foto entera que conviene
    /// copiarlo aparte: `CGImage.cropping(to:)` no copia, pero retiene la
    /// foto entera (decenas de MB en una pantalla 5K) mientras viva el
    /// recorte. Para un pin que se queda horas, mejor copiar los pocos
    /// pixeles que se ven y soltar el resto.
    public static func convieneCopiarAparte(recorte: CGSize, entera: CGSize) -> Bool {
        let a = recorte.width * recorte.height
        let b = entera.width * entera.height
        return b > 0 && a < b / 2
    }

    // MARK: - Nombre del fichero

    /// Las extensiones de la galeria: un nombre ocupado con cualquiera de
    /// ellas no se reutiliza (como `galeria_capturas::EXTENSIONES` de Windows).
    public static let extensionesDeGaleria = ["png", "jpg", "jpeg", "bmp", "gif", "webp", "mp4"]

    /// `captura-AAAAMMDD-HHMMSS` en la hora local, el mismo nombre que pone
    /// Windows (`galeria_capturas::nombre_por_hora`): el nombre es la clave
    /// de cada captura en todos los aparatos.
    public static func nombreDeCaptura(fecha: Date, zona: TimeZone = .current) -> String {
        var calendario = Calendar(identifier: .gregorian)
        calendario.timeZone = zona
        let c = calendario.dateComponents([.year, .month, .day, .hour, .minute, .second], from: fecha)
        return String(format: "captura-%04d%02d%02d-%02d%02d%02d",
                      c.year ?? 0, c.month ?? 0, c.day ?? 0, c.hour ?? 0, c.minute ?? 0, c.second ?? 0)
    }

    /// La ruta de una captura nueva en `carpeta`: el nombre por la hora y
    /// `-2`, `-3`... si en ese segundo ya hay otra (con cualquier extension
    /// de la galeria). `existe` se puede cambiar en las pruebas.
    public static func rutaNueva(en carpeta: URL, fecha: Date, zona: TimeZone = .current,
                                 existe: (URL) -> Bool = { FileManager.default.fileExists(atPath: $0.path) }) -> URL {
        let base = nombreDeCaptura(fecha: fecha, zona: zona)
        func ocupado(_ nombre: String) -> Bool {
            extensionesDeGaleria.contains { existe(carpeta.appendingPathComponent("\(nombre).\($0)")) }
        }
        var nombre = base
        var n = 2
        while ocupado(nombre) {
            nombre = "\(base)-\(n)"
            n += 1
        }
        return carpeta.appendingPathComponent("\(nombre).png")
    }
}
