// Anotar encima de la zona sin abrir otra ventana (v2-captura de Windows,
// `captura2/anotar.rs`): lapiz, flecha, rectangulo, texto y mosaico (teclas
// 1-5), cinco colores, tres grosores y deshacer (⌘Z). Lo dibujado entra en
// la imagen que sale (copiar, guardar, pinear, al chat...).
//
// Implementacion ligera y nativa, no el `EditorLienzo` del puente: ese
// editor vive sobre un `.excalidraw` en disco y no tiene ni mosaico ni
// colores fijos; para cinco herramientas sobre una foto basta un modelo
// pequeno de figuras con su `CGPath`.
//
// Rendimiento (§4bis del plan):
// - cada figura es un `CGPath` vectorial: en pantalla lo pinta una
//   `CAShapeLayer` (la compone la tarjeta grafica, sin mapa de bits), y al
//   salir el MISMO camino se pinta con Core Graphics sobre el recorte;
// - el mosaico es `CIPixellate` de Core Image (en la GPU) solo sobre el
//   trozo que tapa, nunca sobre la foto entera;
// - mientras se arrastra un mosaico se ve su banda maciza (tapa igual y no
//   cuesta rehacer el pixelado en cada movimiento), como en Windows.
//
// Las figuras van en PUNTOS relativos a la esquina de ABAJO a la izquierda
// de la zona (y hacia arriba, como AppKit y como el contexto de un mapa de
// bits de Core Graphics): pintar sobre el recorte es solo escalar.

import AppKit
import CoreImage
import CoreText

/// Las cinco herramientas, en el orden de sus teclas (1-5).
public enum HerramientaAnotar: Int, CaseIterable, Sendable {
    case lapiz = 1, flecha, rectangulo, texto, mosaico

    public var titulo: String {
        switch self {
        case .lapiz: "Lápiz"
        case .flecha: "Flecha"
        case .rectangulo: "Rectángulo"
        case .texto: "Texto"
        case .mosaico: "Mosaico"
        }
    }

    public var simbolo: String {
        switch self {
        case .lapiz: "pencil.tip"
        case .flecha: "arrow.up.right"
        case .rectangulo: "rectangle"
        case .texto: "textformat"
        case .mosaico: "checkerboard.rectangle"
        }
    }

    /// La herramienta de una tecla (`1`...`5`).
    public static func deTecla(_ c: String) -> HerramientaAnotar? {
        guard let n = Int(c) else { return nil }
        return HerramientaAnotar(rawValue: n)
    }
}

/// Una figura dibujada.
public struct FiguraAnotada: Equatable {
    public enum Clase: Equatable {
        /// Lapiz: los puntos del trazo.
        case trazo([CGPoint])
        case flecha(desde: CGPoint, hasta: CGPoint)
        case rectangulo(CGRect)
        /// Texto de una linea con su punto de apoyo (abajo a la izquierda).
        case texto(String, en: CGPoint)
        /// La zona que se pixela.
        case mosaico(CGRect)
    }

    public var clase: Clase
    /// Indice en `AnotacionCaptura.colores`.
    public var color: Int
    /// Indice en `AnotacionCaptura.grosores`.
    public var grosor: Int

    public init(clase: Clase, color: Int, grosor: Int) {
        self.clase = clase
        self.color = color
        self.grosor = grosor
    }
}

/// El modelo de lo anotado sobre una zona: las figuras, la que se esta
/// haciendo y deshacer/rehacer. Sin pantalla: lo prueban las pruebas.
public struct AnotacionCaptura {
    /// Los cinco colores de la maqueta (`COLORES` de Windows): rojo,
    /// amarillo, verde, azul y negro.
    public static let colores: [(UInt8, UInt8, UInt8)] = [
        (0xFF, 0x3B, 0x30), (0xFF, 0xD6, 0x0A), (0x30, 0xD1, 0x58), (0x0A, 0x84, 0xFF), (0x1C, 0x1C, 0x1E),
    ]
    /// Grosor del trazo en puntos: fino, medio y grueso.
    public static let grosores: [CGFloat] = [2, 4, 8]
    /// Tamano de la letra por grosor (16/20/28, como Windows).
    public static let letras: [CGFloat] = [16, 20, 28]
    /// Lado del cuadro del mosaico en puntos por grosor.
    public static let bloques: [CGFloat] = [6, 10, 16]

    public private(set) var figuras: [FiguraAnotada] = []
    private var deshechas: [FiguraAnotada] = []
    /// La que se esta dibujando (arrastrando o escribiendo).
    public private(set) var enCurso: FiguraAnotada?
    /// Donde se pulso (la esquina fija del rectangulo o del mosaico).
    private var ancla: CGPoint = .zero
    public var herramienta: HerramientaAnotar?
    public var color = 0
    public var grosor = 1

    public init() {}

    public var vacia: Bool { figuras.isEmpty && enCurso == nil }
    public var escribiendo: Bool {
        if case .texto = enCurso?.clase { return true }
        return false
    }

    /// Coge una herramienta; la misma otra vez la suelta (como Windows).
    public mutating func tomar(_ h: HerramientaAnotar) {
        cerrarTexto()
        herramienta = herramienta == h ? nil : h
    }

    public mutating func ponerColor(_ i: Int) {
        color = min(max(i, 0), AnotacionCaptura.colores.count - 1)
        if enCurso != nil { enCurso?.color = color }
    }

    public mutating func ponerGrosor(_ i: Int) {
        grosor = min(max(i, 0), AnotacionCaptura.grosores.count - 1)
        if enCurso != nil { enCurso?.grosor = grosor }
    }

    // MARK: Puntero (puntos relativos a la zona)

    public mutating func pulsar(_ p: CGPoint) {
        guard let herramienta else { return }
        cerrarTexto()
        ancla = p
        let clase: FiguraAnotada.Clase
        switch herramienta {
        case .lapiz: clase = .trazo([p])
        case .flecha: clase = .flecha(desde: p, hasta: p)
        case .rectangulo: clase = .rectangulo(CGRect(origin: p, size: .zero))
        case .texto: clase = .texto("", en: p)
        case .mosaico: clase = .mosaico(CGRect(origin: p, size: .zero))
        }
        enCurso = FiguraAnotada(clase: clase, color: color, grosor: grosor)
    }

    public mutating func mover(_ p: CGPoint) {
        guard var f = enCurso else { return }
        switch f.clase {
        case var .trazo(puntos):
            // Sin puntos repetidos: un raton quieto no engorda el trazo.
            if let u = puntos.last, abs(u.x - p.x) < 0.5, abs(u.y - p.y) < 0.5 { return }
            puntos.append(p)
            f.clase = .trazo(puntos)
        case let .flecha(desde, _):
            f.clase = .flecha(desde: desde, hasta: p)
        case .rectangulo:
            f.clase = .rectangulo(GeometriaCaptura.rectEntre(ancla, p))
        case .mosaico:
            f.clase = .mosaico(GeometriaCaptura.rectEntre(ancla, p))
        case .texto:
            return
        }
        enCurso = f
    }

    /// Suelta el boton: la figura queda (si no es un clic sin tamano). El
    /// texto sigue abierto para escribir.
    public mutating func soltar(_ p: CGPoint) {
        mover(p)
        guard let f = enCurso else { return }
        if case .texto = f.clase { return }
        enCurso = nil
        if AnotacionCaptura.tieneTamano(f) {
            figuras.append(f)
            deshechas.removeAll()
        }
    }

    // MARK: Texto

    public mutating func escribir(_ s: String) {
        guard case let .texto(t, p) = enCurso?.clase else { return }
        let limpio = s.filter { !$0.isNewline && $0 != "\u{7F}" }
        guard !limpio.isEmpty else { return }
        enCurso?.clase = .texto(t + limpio, en: p)
    }

    public mutating func borrarLetra() {
        guard case var .texto(t, p) = enCurso?.clase, !t.isEmpty else { return }
        t.removeLast()
        enCurso?.clase = .texto(t, en: p)
    }

    /// Cierra el texto que se escribe (vacio, no queda). Devuelve si habia.
    @discardableResult
    public mutating func cerrarTexto() -> Bool {
        guard let f = enCurso, case let .texto(t, _) = f.clase else { return false }
        enCurso = nil
        if !t.trimmingCharacters(in: .whitespaces).isEmpty {
            figuras.append(f)
            deshechas.removeAll()
        }
        return true
    }

    // MARK: Deshacer

    /// Deshace la ultima figura (o lo que se escribia). `false` si no habia.
    @discardableResult
    public mutating func deshacer() -> Bool {
        if enCurso != nil {
            enCurso = nil
            return true
        }
        guard let f = figuras.popLast() else { return false }
        deshechas.append(f)
        return true
    }

    @discardableResult
    public mutating func rehacer() -> Bool {
        guard let f = deshechas.popLast() else { return false }
        figuras.append(f)
        return true
    }

    public mutating func vaciar() {
        figuras.removeAll()
        deshechas.removeAll()
        enCurso = nil
    }

    /// Todo lo que hay que pintar: las figuras y la que esta a medias.
    public var todas: [FiguraAnotada] {
        enCurso.map { figuras + [$0] } ?? figuras
    }

    // MARK: Geometria

    static func tieneTamano(_ f: FiguraAnotada) -> Bool {
        switch f.clase {
        case let .trazo(p): return !p.isEmpty
        case let .flecha(a, b): return hypot(b.x - a.x, b.y - a.y) >= 3
        case let .rectangulo(r), let .mosaico(r): return r.width >= 2 && r.height >= 2
        case let .texto(t, _): return !t.isEmpty
        }
    }

    /// La punta de la flecha: dos alas de `largo` a ±28° del trazo.
    public static func alas(desde a: CGPoint, hasta b: CGPoint, grosor: CGFloat) -> (CGPoint, CGPoint) {
        let angulo = atan2(b.y - a.y, b.x - a.x)
        let largo = max(10, grosor * 4)
        let abertura: CGFloat = .pi * 28 / 180
        let i = CGPoint(x: b.x - largo * cos(angulo - abertura), y: b.y - largo * sin(angulo - abertura))
        let d = CGPoint(x: b.x - largo * cos(angulo + abertura), y: b.y - largo * sin(angulo + abertura))
        return (i, d)
    }

    /// El camino de una figura de trazo (lapiz, flecha, rectangulo). `nil`
    /// para el texto y el mosaico, que no son trazos.
    public static func camino(de f: FiguraAnotada) -> CGPath? {
        let g = grosores[min(max(f.grosor, 0), grosores.count - 1)]
        let c = CGMutablePath()
        switch f.clase {
        case let .trazo(puntos):
            guard let primero = puntos.first else { return nil }
            c.move(to: primero)
            if puntos.count == 1 {
                // Un clic deja un punto: una linea de largo cero con
                // extremo redondo.
                c.addLine(to: primero)
            } else {
                c.addLines(between: puntos)
            }
        case let .flecha(a, b):
            c.move(to: a)
            c.addLine(to: b)
            let (i, d) = alas(desde: a, hasta: b, grosor: g)
            c.move(to: i)
            c.addLine(to: b)
            c.addLine(to: d)
        case let .rectangulo(r):
            c.addRect(r)
        case .texto, .mosaico:
            return nil
        }
        return c
    }

    public static func cgColor(_ i: Int) -> CGColor {
        let (r, g, b) = colores[min(max(i, 0), colores.count - 1)]
        return CGColor(srgbRed: CGFloat(r) / 255, green: CGFloat(g) / 255, blue: CGFloat(b) / 255, alpha: 1)
    }

    public static func fuente(_ grosor: Int) -> CTFont {
        CTFontCreateUIFontForLanguage(.system, letras[min(max(grosor, 0), letras.count - 1)], nil)
            ?? CTFontCreateWithName("Helvetica-Bold" as CFString, letras[1], nil)
    }

    // MARK: Pintar

    /// Pinta las figuras sobre `contexto` (ya escalado a puntos de la zona,
    /// origen abajo a la izquierda). El mosaico se pinta con `pixelar`, que
    /// recibe la zona en puntos y devuelve su trozo pixelado; sin el, banda
    /// maciza gris (lo que se ve mientras se arrastra).
    public static func pintar(_ figuras: [FiguraAnotada], en contexto: CGContext,
                              pixelar: ((CGRect, CGFloat) -> CGImage?)? = nil) {
        // Primero los mosaicos: tapan la foto, no lo dibujado encima.
        for f in figuras {
            guard case let .mosaico(r) = f.clase, r.width >= 1, r.height >= 1 else { continue }
            let bloque = bloques[min(max(f.grosor, 0), bloques.count - 1)]
            if let img = pixelar?(r, bloque) {
                contexto.saveGState()
                contexto.interpolationQuality = .none
                contexto.draw(img, in: r)
                contexto.restoreGState()
            } else {
                contexto.setFillColor(CGColor(gray: 0.55, alpha: 1))
                contexto.fill(r)
            }
        }
        for f in figuras {
            switch f.clase {
            case .mosaico:
                continue
            case let .texto(t, p):
                pintarTexto(t, en: p, figura: f, contexto: contexto)
            default:
                guard let c = camino(de: f) else { continue }
                contexto.saveGState()
                contexto.setStrokeColor(cgColor(f.color))
                contexto.setLineWidth(grosores[min(max(f.grosor, 0), grosores.count - 1)])
                contexto.setLineCap(.round)
                contexto.setLineJoin(.round)
                contexto.addPath(c)
                contexto.strokePath()
                contexto.restoreGState()
            }
        }
    }

    static func lineaDeTexto(_ t: String, figura f: FiguraAnotada) -> CTLine {
        let atributos: [NSAttributedString.Key: Any] = [
            NSAttributedString.Key(kCTFontAttributeName as String): fuente(f.grosor),
            NSAttributedString.Key(kCTForegroundColorAttributeName as String): cgColor(f.color),
        ]
        return CTLineCreateWithAttributedString(NSAttributedString(string: t, attributes: atributos))
    }

    static func pintarTexto(_ t: String, en p: CGPoint, figura f: FiguraAnotada, contexto: CGContext) {
        guard !t.isEmpty else { return }
        let linea = lineaDeTexto(t, figura: f)
        contexto.saveGState()
        contexto.textMatrix = .identity
        contexto.textPosition = p
        CTLineDraw(linea, contexto)
        contexto.restoreGState()
    }

    // MARK: Quemar en la imagen

    /// El contexto de Core Image compartido (GPU). Uno para toda la app:
    /// crearlo cuesta y guarda sus caches.
    static let contextoCI = CIContext(options: [.cacheIntermediates: false])

    /// El trozo `rectPx` (pixeles, origen ARRIBA) de `imagen`, pixelado con
    /// cuadros de `bloquePx` pixeles. Solo se lee y se procesa ese trozo.
    public static func pixelado(_ imagen: CGImage, rectPx: CGRect, bloquePx: CGFloat) -> CGImage? {
        let limites = CGRect(x: 0, y: 0, width: imagen.width, height: imagen.height)
        let r = rectPx.integral.intersection(limites)
        guard !r.isNull, r.width >= 1, r.height >= 1, let trozo = imagen.cropping(to: r) else { return nil }
        let entrada = CIImage(cgImage: trozo)
        guard let filtro = CIFilter(name: "CIPixellate") else { return nil }
        filtro.setValue(entrada.clampedToExtent(), forKey: kCIInputImageKey)
        filtro.setValue(max(bloquePx, 2), forKey: kCIInputScaleKey)
        filtro.setValue(CIVector(x: 0, y: 0), forKey: kCIInputCenterKey)
        guard let salida = filtro.outputImage?.cropped(to: entrada.extent) else { return nil }
        return contextoCI.createCGImage(salida, from: entrada.extent)
    }

    /// El recorte con lo anotado dentro: un mapa de bits del tamano del
    /// recorte (una sola copia), el recorte, los mosaicos y encima los
    /// trazos. Sin figuras devuelve el recorte tal cual (cero copias).
    public static func quemar(_ figuras: [FiguraAnotada], en recorte: CGImage, escala: CGFloat) -> CGImage {
        guard !figuras.isEmpty else { return recorte }
        let w = recorte.width
        let h = recorte.height
        let espacio = recorte.colorSpace ?? CGColorSpace(name: CGColorSpace.sRGB) ?? CGColorSpaceCreateDeviceRGB()
        let info = CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue
        guard let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: 0,
                                  space: espacio, bitmapInfo: info) else { return recorte }
        ctx.interpolationQuality = .none
        ctx.draw(recorte, in: CGRect(x: 0, y: 0, width: w, height: h))
        let e = max(escala, 0.01)
        ctx.scaleBy(x: e, y: e)
        let altoPuntos = CGFloat(h) / e
        pintar(figuras, en: ctx) { r, bloque in
            // De puntos de la zona (origen abajo) a pixeles del recorte
            // (origen arriba).
            let px = CGRect(x: r.minX * e, y: (altoPuntos - r.maxY) * e, width: r.width * e, height: r.height * e)
            return pixelado(recorte, rectPx: px, bloquePx: bloque * e)
        }
        return ctx.makeImage() ?? recorte
    }
}
