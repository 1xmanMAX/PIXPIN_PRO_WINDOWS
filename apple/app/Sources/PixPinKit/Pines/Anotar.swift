// Anotar encima de un pin: lapiz, flecha, rectangulo, texto y marcador
// (las teclas 1-5, como en la captura de Windows).
//
// Lo dibujado vive como una capa encima de la foto mientras se quiera (se
// puede seguir moviendo y agrandando el pin: los puntos se guardan
// relativos, de 0 a 1) y «Fijar en la imagen» lo quema en la foto con Core
// Graphics, una sola vez y al tamano de la foto.
//
// No usa `EditorLienzo` del puente: ese edita un `.excalidraw` de un chat
// (su escena, su historial y su guardado), y un pin es una foto suelta que no
// esta en ningun chat. Cinco figuras con `CGPath` no necesitan el motor.
//
// Rendimiento: la capa solo se redibuja al cambiar un trazo (`needsDisplay`)
// y nada corre si no se esta anotando.

import AppKit
import CoreText

/// Con que se anota.
enum HerramientaAnotacion: String, CaseIterable, Equatable {
    case lapiz, flecha, rectangulo, texto, marcador

    var titulo: String {
        switch self {
        case .lapiz: "Lápiz"
        case .flecha: "Flecha"
        case .rectangulo: "Rectángulo"
        case .texto: "Texto"
        case .marcador: "Marcador"
        }
    }

    var simbolo: String {
        switch self {
        case .lapiz: "pencil.tip"
        case .flecha: "arrow.up.right"
        case .rectangulo: "rectangle"
        case .texto: "textformat"
        case .marcador: "highlighter"
        }
    }

    /// La tecla que la elige mientras se anota.
    var tecla: String {
        switch self {
        case .lapiz: "1"
        case .flecha: "2"
        case .rectangulo: "3"
        case .texto: "4"
        case .marcador: "5"
        }
    }
}

/// Un trazo: sus puntos van de 0 a 1 desde la esquina de ABAJO a la
/// izquierda del pin (asi siguen en su sitio al agrandarlo).
struct TrazoAnotacion: Equatable {
    var herramienta: HerramientaAnotacion
    /// El color, por su numero en `ReglasAnotacion.colores`.
    var color: Int
    var puntos: [CGPoint]
    var texto: String = ""
}

enum ReglasAnotacion {
    /// Rojo, amarillo, azul, verde y blanco.
    static let colores: [(r: CGFloat, g: CGFloat, b: CGFloat)] = [
        (1, 59 / 255, 48 / 255),
        (245 / 255, 194 / 255, 24 / 255),
        (10 / 255, 132 / 255, 1),
        (63 / 255, 200 / 255, 140 / 255),
        (1, 1, 1),
    ]
    static let nombresColores = ["Rojo", "Amarillo", "Azul", "Verde", "Blanco"]

    static func color(_ i: Int, alfa: CGFloat = 1) -> CGColor {
        let c = colores[min(max(i, 0), colores.count - 1)]
        return CGColor(srgbRed: c.r, green: c.g, blue: c.b, alpha: alfa)
    }

    static func normalizar(_ p: CGPoint, en tam: CGSize) -> CGPoint {
        guard tam.width > 0, tam.height > 0 else { return .zero }
        return CGPoint(x: ReglasPin.limitar(p.x / tam.width, 0, 1), y: ReglasPin.limitar(p.y / tam.height, 0, 1))
    }

    static func desnormalizar(_ p: CGPoint, en tam: CGSize) -> CGPoint {
        CGPoint(x: p.x * tam.width, y: p.y * tam.height)
    }

    /// El grosor de cada herramienta, relativo al lado corto de lo que se
    /// pinta: el mismo trazo se ve igual en el pin y en la foto quemada.
    static func grosor(_ h: HerramientaAnotacion, lado: CGFloat) -> CGFloat {
        switch h {
        case .marcador: max(8, lado * 0.035)
        default: max(2, lado * 0.008)
        }
    }

    static func tamLetra(lado: CGFloat) -> CGFloat { max(11, lado * 0.06) }

    /// Las dos alas de la punta de una flecha que va de `desde` a `hasta`,
    /// a 30 grados de la linea.
    static func puntaFlecha(desde: CGPoint, hasta: CGPoint, largo: CGFloat) -> (CGPoint, CGPoint) {
        let angulo = atan2(hasta.y - desde.y, hasta.x - desde.x)
        let abertura = CGFloat.pi / 6
        let a = CGPoint(x: hasta.x - largo * cos(angulo - abertura), y: hasta.y - largo * sin(angulo - abertura))
        let b = CGPoint(x: hasta.x - largo * cos(angulo + abertura), y: hasta.y - largo * sin(angulo + abertura))
        return (a, b)
    }

    /// El rectangulo entre dos esquinas cualesquiera.
    static func rectangulo(_ a: CGPoint, _ b: CGPoint) -> CGRect {
        CGRect(x: min(a.x, b.x), y: min(a.y, b.y), width: abs(a.x - b.x), height: abs(a.y - b.y))
    }

    /// Pinta los trazos en `ctx` (y hacia arriba) sobre algo de `tam`.
    static func dibujar(_ trazos: [TrazoAnotacion], en ctx: CGContext, tam: CGSize) {
        let lado = min(tam.width, tam.height)
        for t in trazos {
            let pts = t.puntos.map { desnormalizar($0, en: tam) }
            guard let primero = pts.first else { continue }
            let ancho = grosor(t.herramienta, lado: lado)
            ctx.saveGState()
            ctx.setLineCap(.round)
            ctx.setLineJoin(.round)
            ctx.setLineWidth(ancho)
            switch t.herramienta {
            case .lapiz, .marcador:
                ctx.setStrokeColor(color(t.color, alfa: t.herramienta == .marcador ? 0.4 : 1))
                if t.herramienta == .marcador { ctx.setLineCap(.butt) }
                ctx.beginPath()
                ctx.move(to: primero)
                if pts.count == 1 {
                    ctx.addLine(to: CGPoint(x: primero.x + 0.1, y: primero.y))
                } else {
                    for p in pts.dropFirst() { ctx.addLine(to: p) }
                }
                ctx.strokePath()
            case .flecha:
                guard let ultimo = pts.last, pts.count >= 2 else { break }
                ctx.setStrokeColor(color(t.color))
                ctx.beginPath()
                ctx.move(to: primero)
                ctx.addLine(to: ultimo)
                let (a, b) = puntaFlecha(desde: primero, hasta: ultimo, largo: max(10, ancho * 4))
                ctx.move(to: a)
                ctx.addLine(to: ultimo)
                ctx.addLine(to: b)
                ctx.strokePath()
            case .rectangulo:
                guard let ultimo = pts.last, pts.count >= 2 else { break }
                ctx.setStrokeColor(color(t.color))
                ctx.stroke(rectangulo(primero, ultimo))
            case .texto:
                guard !t.texto.isEmpty else { break }
                let letra = NSFont.boldSystemFont(ofSize: tamLetra(lado: lado))
                let atributos: [NSAttributedString.Key: Any] = [
                    .font: letra,
                    NSAttributedString.Key(kCTForegroundColorAttributeName as String): color(t.color),
                ]
                let linea = CTLineCreateWithAttributedString(NSAttributedString(string: t.texto, attributes: atributos))
                ctx.textMatrix = .identity
                ctx.textPosition = primero
                CTLineDraw(linea, ctx)
            }
            ctx.restoreGState()
        }
    }

    /// La foto con los trazos quemados, a su tamano. `nil` si no se pudo.
    static func quemar(_ trazos: [TrazoAnotacion], en imagen: CGImage) -> CGImage? {
        let tam = CGSize(width: imagen.width, height: imagen.height)
        let espacio = CGColorSpace(name: CGColorSpace.sRGB) ?? CGColorSpaceCreateDeviceRGB()
        guard let ctx = CGContext(data: nil, width: imagen.width, height: imagen.height, bitsPerComponent: 8,
                                  bytesPerRow: 0, space: espacio,
                                  bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue
                                      | CGBitmapInfo.byteOrder32Little.rawValue)
        else { return nil }
        ctx.draw(imagen, in: CGRect(origin: .zero, size: tam))
        dibujar(trazos, en: ctx, tam: tam)
        return ctx.makeImage()
    }
}

/// La capa de anotar encima del contenido del pin.
final class VistaAnotacion: NSView {
    var trazos: [TrazoAnotacion] = [] { didSet { needsDisplay = true } }
    var herramienta: HerramientaAnotacion = .lapiz
    var color = 0
    /// Si se esta anotando (si no, la capa solo ensena y deja pasar el raton).
    var editable = false {
        didSet {
            if !editable { terminarTexto() }
            window?.invalidateCursorRects(for: self)
        }
    }
    var alCambiar: (() -> Void)?

    private var actual: TrazoAnotacion?
    private var campo: NSTextField?
    private var puntoCampo: CGPoint = .zero

    override var isOpaque: Bool { false }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    override func hitTest(_ point: NSPoint) -> NSView? {
        editable ? super.hitTest(point) : nil
    }

    override func resetCursorRects() {
        if editable { addCursorRect(bounds, cursor: herramienta == .texto ? .iBeam : .crosshair) }
    }

    override func draw(_ dirtyRect: NSRect) {
        guard let ctx = NSGraphicsContext.current?.cgContext else { return }
        var todos = trazos
        if let actual { todos.append(actual) }
        ReglasAnotacion.dibujar(todos, en: ctx, tam: bounds.size)
    }

    override func mouseDown(with event: NSEvent) {
        guard editable else { return super.mouseDown(with: event) }
        let p = convert(event.locationInWindow, from: nil)
        terminarTexto()
        if herramienta == .texto {
            empezarTexto(en: p)
            return
        }
        actual = TrazoAnotacion(herramienta: herramienta, color: color,
                                puntos: [ReglasAnotacion.normalizar(p, en: bounds.size)])
        needsDisplay = true
    }

    override func mouseDragged(with event: NSEvent) {
        guard editable, var t = actual else { return }
        let p = ReglasAnotacion.normalizar(convert(event.locationInWindow, from: nil), en: bounds.size)
        switch t.herramienta {
        case .lapiz, .marcador:
            t.puntos.append(p)
        default:
            // Flecha y rectangulo: el primer punto y el de ahora.
            t.puntos = [t.puntos[0], p]
        }
        actual = t
        needsDisplay = true
    }

    override func mouseUp(with event: NSEvent) {
        guard editable, let t = actual else { return }
        actual = nil
        // Una flecha o un rectangulo sin arrastrar no son nada.
        if t.herramienta == .flecha || t.herramienta == .rectangulo, t.puntos.count < 2 {
            needsDisplay = true
            return
        }
        trazos.append(t)
        alCambiar?()
    }

    func deshacer() {
        guard !trazos.isEmpty else { return NSSound.beep() }
        trazos.removeLast()
        alCambiar?()
    }

    // MARK: Texto

    private func empezarTexto(en p: CGPoint) {
        let lado = min(bounds.width, bounds.height)
        let tam = ReglasAnotacion.tamLetra(lado: lado)
        let campo = NSTextField(frame: CGRect(x: p.x, y: p.y - tam * 0.3, width: max(120, bounds.width - p.x - 4),
                                              height: tam * 1.5))
        campo.font = .boldSystemFont(ofSize: tam)
        campo.textColor = NSColor(cgColor: ReglasAnotacion.color(color)) ?? .systemRed
        campo.isBordered = false
        campo.drawsBackground = false
        campo.focusRingType = .none
        campo.placeholderString = "Texto"
        campo.target = self
        campo.action = #selector(textoListo(_:))
        addSubview(campo)
        window?.makeFirstResponder(campo)
        self.campo = campo
        puntoCampo = p
    }

    @objc private func textoListo(_ sender: NSTextField) {
        terminarTexto()
    }

    /// Pasa lo escrito a un trazo y quita el campo.
    func terminarTexto() {
        guard let campo else { return }
        self.campo = nil
        let texto = campo.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        campo.removeFromSuperview()
        guard !texto.isEmpty else { return }
        trazos.append(TrazoAnotacion(herramienta: .texto, color: color,
                                     puntos: [ReglasAnotacion.normalizar(puntoCampo, en: bounds.size)],
                                     texto: texto))
        alCambiar?()
    }

    /// Si se esta escribiendo un texto (las teclas son suyas).
    var escribiendo: Bool { campo != nil }
}
