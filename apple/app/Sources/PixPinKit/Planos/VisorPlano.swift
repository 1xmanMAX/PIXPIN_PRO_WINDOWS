// El visor de planos DWG y DXF (`ventana.rs` de Windows con Metal):
// moverse con el trackpad (dos dedos mueven, pellizcar acerca, ⌘ + rueda o
// la rueda del raton acercan donde esta el puntero), la regla con
// enganche, los marcos para imprimir y las capas.
//
// **Solo se pinta cuando algo cambia** (§4bis): la `MTKView` esta parada
// (`isPaused`) y pinta al pedirselo (`enableSetNeedsDisplay`). Lo de encima
// (cotas, la mira, lo anotado con el lapiz) va en una vista transparente con
// Core Graphics, que tambien se pinta solo al cambiar.

import AppKit
import Metal
import MetalKit
import PixPinCore
import SwiftUI

/// Que hace el raton en el visor.
public enum ModoVisorPlano: String, CaseIterable, Sendable {
    /// Arrastrar mueve el plano.
    case mover
    /// Cada clic pone un punto de la regla (con enganche); doble clic o
    /// Intro la termina y la guarda como cota.
    case medir
    /// Arrastrar dibuja un marco para imprimir.
    case marco
}

/// **El estado del visor de un plano**: lo leido, las capas, lo anotado y
/// lo que se esta midiendo. La vista de Metal lo pinta.
@MainActor
public final class VisorPlano: ObservableObject {
    @Published public private(set) var datos: PlanoEnTarjeta?
    @Published public private(set) var error: String?
    @Published public private(set) var capas: [CapaPlano] = []
    @Published public private(set) var anotado = AnotadoPlano(cotas: [], marcos: [])
    @Published public var modo: ModoVisorPlano = .mover {
        didSet { vista?.cambioDeModo() }
    }
    /// La medida de la regla en curso (o de la ultima), para la barra.
    @Published public private(set) var medida = ""
    public let titulo: String
    public let ruta: String

    /// Los grupos que se ven.
    private(set) var visibles: Set<UInt32> = []
    /// Lo anotado con el lapiz (la capa del motor del lienzo), solo para verlo.
    private(set) var tinta: LienzoLeido?
    private(set) var encaje: EncajePlano?
    /// Los textos de cada tramo de cada cota (se hacen al cambiar lo anotado).
    private(set) var etiquetas: [[String]] = []
    weak var vista: VistaPlanoMetal?

    public init(ruta: String, titulo: String) {
        self.ruta = ruta
        self.titulo = titulo
    }

    /// Lee el plano fuera del hilo principal y lo sube a la tarjeta.
    public func cargar(nucleo: Nucleo) {
        guard datos == nil, error == nil else { return }
        guard let motor = MotorMetal.compartido else {
            error = "Este Mac no tiene una tarjeta gráfica con Metal."
            return
        }
        let ruta = self.ruta
        Task {
            let hecho = await Task.detached(priority: .userInitiated) { () -> Result<PlanoEnTarjeta, Error> in
                Result { try PlanoEnTarjeta.cargar(ruta: ruta, nucleo: nucleo, motor: motor) }
            }.value
            switch hecho {
            case let .success(d):
                self.poner(d)
            case let .failure(e):
                self.error = "No se pudo abrir el plano: \(e)"
            }
        }
    }

    private func poner(_ d: PlanoEnTarjeta) {
        datos = d
        visibles = Set(d.plano.gruposVisibles())
        capas = d.plano.capas()
        anotado = d.plano.anotado()
        rehacerEtiquetas()
        leerTinta()
        vista?.alLlegarElPlano()
    }

    /// Lo anotado con el lapiz (Windows y Android) se ve encima, tal cual.
    private func leerTinta() {
        guard let d = datos else { return }
        let ruta = d.plano.rutaDeLaCapa()
        encaje = d.plano.encaje()
        guard FileManager.default.fileExists(atPath: ruta) else { return }
        Task {
            let leida = await Task.detached(priority: .utility) { () -> LienzoLeido? in
                guard let l = try? Lienzo.deFichero(ruta: ruta) else { return nil }
                return try? leerLienzo(l.plano())
            }.value
            self.tinta = leida
            self.vista?.repintar()
        }
    }

    private func rehacerEtiquetas() {
        guard let p = datos?.plano else { etiquetas = []; return }
        etiquetas = anotado.cotas.map { c in
            zip(c.puntos, c.puntos.dropFirst()).map { a, b in p.textoDeMedida(distancia: hypot(b.x - a.x, b.y - a.y)) }
        }
    }

    // MARK: Capas

    public func ponerCapa(_ nombre: String, encendida: Bool) {
        guard let p = datos?.plano, p.ponerCapa(nombre: nombre, encendida: encendida) else { return }
        visibles = Set(p.gruposVisibles())
        capas = p.capas()
        vista?.repintar()
    }

    public func todasLasCapas(_ encendidas: Bool) {
        guard let p = datos?.plano else { return }
        for c in capas { _ = p.ponerCapa(nombre: c.nombre, encendida: encendidas) }
        visibles = Set(p.gruposVisibles())
        capas = p.capas()
        vista?.repintar()
    }

    // MARK: Regla, cotas y marcos

    /// El texto de una medida (con la unidad del plano).
    func texto(_ d: Double) -> String { datos?.plano.textoDeMedida(distancia: d) ?? String(format: "%.3f", d) }

    func ponerMedida(_ t: String) {
        if medida != t { medida = t }
    }

    /// Guarda en la capa (la que viaja) una cota nueva.
    func anadirCota(_ puntos: [PuntoPlano]) {
        guard puntos.count >= 2 else { return }
        var a = anotado
        a.cotas.append(CotaPlano(puntos: puntos))
        guardar(a)
    }

    func anadirMarco(_ c: CajaPlano) {
        var a = anotado
        a.marcos.append(c)
        guardar(a)
    }

    public func borrarCotas() {
        var a = anotado
        a.cotas = []
        guardar(a)
    }

    public func borrarMarcos() {
        var a = anotado
        a.marcos = []
        guardar(a)
    }

    /// Lo anotado se ve al momento y se escribe fuera del hilo principal.
    private func guardar(_ a: AnotadoPlano) {
        guard let p = datos?.plano else { return }
        anotado = a
        rehacerEtiquetas()
        vista?.repintar()
        Task.detached(priority: .utility) {
            do {
                _ = try p.guardarAnotado(anotado: a)
            } catch {
                await MainActor.run { NSSound.beep() }
            }
        }
    }

    // MARK: Ordenes de la barra

    public func encuadrar() { vista?.encuadrar() }

    public func imprimir() {
        guard let d = datos, let v = vista else { return }
        ImpresionPlano.imprimir(d, visibles: visibles, region: anotado.marcos.last ?? v.cajaVisible(),
                                titulo: titulo, ventana: v.window)
    }
}

// MARK: - La vista de Metal

/// **El plano en Metal.** Una `MTKView` parada que pinta solo al cambiar
/// algo; los gestos mueven la camara (`CamaraPlano`).
public final class VistaPlanoMetal: MTKView {
    let visor: VisorPlano
    private let motor: MotorMetal
    private let pintor: PintorPlano
    private(set) var camara = CamaraPlano(centroX: 0, centroY: 0, px: 1)
    private var encuadrado = false
    private let encima = CapaEncimaPlano()
    private var seguimiento: NSTrackingArea?

    // La regla y el marco.
    private(set) var cadena: [PuntoPlano] = []
    private(set) var mira: Enganche?
    private(set) var marcoEnCurso: (PuntoPlano, PuntoPlano)?
    private var arrastre: CGPoint?

    init(visor: VisorPlano, motor: MotorMetal) {
        self.visor = visor
        self.motor = motor
        let muestras = motor.muestras
        pintor = PintorPlano(motor: motor, color: .bgra8Unorm, muestras: muestras)
        super.init(frame: NSRect(x: 0, y: 0, width: 800, height: 600), device: motor.dispositivo)
        colorPixelFormat = .bgra8Unorm
        depthStencilPixelFormat = .invalid
        sampleCount = muestras
        isPaused = true
        enableSetNeedsDisplay = true
        autoResizeDrawable = true
        encima.vista = self
        encima.frame = bounds
        encima.autoresizingMask = [.width, .height]
        addSubview(encima)
        visor.vista = self
        try? pintor.preparar()
    }

    @available(*, unavailable)
    required init(coder: NSCoder) { fatalError("init(coder:) no se usa") }

    override public var acceptsFirstResponder: Bool { true }
    override public var isOpaque: Bool { true }

    /// Pinta otra vez (y lo de encima).
    func repintar() {
        needsDisplay = true
        encima.needsDisplay = true
    }

    /// Pixeles por punto de la pantalla.
    var escala: Double {
        bounds.width > 0 ? Double(drawableSize.width) / Double(bounds.width) : Double(window?.backingScaleFactor ?? 2)
    }

    var anchoPx: Double { Double(drawableSize.width) }
    var altoPx: Double { Double(drawableSize.height) }

    private var oscuro: Bool { effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua }

    /// El punto del dibujo bajo un punto de la vista.
    func aDibujo(_ p: CGPoint) -> PuntoPlano {
        camara.aDibujo(x: Double(p.x) * escala, y: Double(p.y) * escala, ancho: anchoPx, alto: altoPx)
    }

    /// El punto de la vista de un punto del dibujo.
    func aVista(_ p: PuntoPlano) -> CGPoint {
        let q = camara.aPixel(p, ancho: anchoPx, alto: altoPx)
        let e = escala
        return CGPoint(x: q.x / e, y: q.y / e)
    }

    /// Lo que se ve, en el dibujo.
    func cajaVisible() -> CajaPlano { camara.caja(ancho: anchoPx, alto: altoPx) }

    func alLlegarElPlano() {
        encuadrado = false
        encuadrar()
    }

    func encuadrar() {
        guard let d = visor.datos, anchoPx > 0, altoPx > 0 else { return }
        camara = CamaraPlano.encuadrar(d.resumen.cajaUtil, ancho: anchoPx, alto: altoPx)
        encuadrado = true
        repintar()
    }

    func cambioDeModo() {
        cadena = []
        mira = nil
        marcoEnCurso = nil
        visor.ponerMedida("")
        window?.invalidateCursorRects(for: self)
        repintar()
    }

    override public func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        if !encuadrado { encuadrar() }
        repintar()
    }

    override public func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        repintar()
    }

    override public func resetCursorRects() {
        addCursorRect(bounds, cursor: visor.modo == .mover ? .openHand : .crosshair)
    }

    // MARK: Pintar

    override public func draw(_ dirtyRect: NSRect) {
        let oscuro = self.oscuro
        clearColor = oscuro ? MTLClearColor(red: 0.13, green: 0.15, blue: 0.19, alpha: 1)
            : MTLClearColor(red: 0.99, green: 0.988, blue: 0.976, alpha: 1)
        guard let pasada = currentRenderPassDescriptor, let dibujable = currentDrawable,
              let cb = motor.cola.makeCommandBuffer(),
              let enc = cb.makeRenderCommandEncoder(descriptor: pasada) else { return }
        if let d = visor.datos {
            do {
                try pintor.pintar(d, visibles: visor.visibles, camara: camara, ancho: anchoPx, alto: altoPx,
                                  oscuro: oscuro, en: enc)
            } catch {
                NSLog("PixPin planos: \(error)")
            }
        }
        enc.endEncoding()
        cb.present(dibujable)
        cb.commit()
    }

    // MARK: Gestos

    override public func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let s = seguimiento { removeTrackingArea(s) }
        let s = NSTrackingArea(rect: bounds, options: [.mouseMoved, .activeInKeyWindow, .inVisibleRect],
                               owner: self, userInfo: nil)
        addTrackingArea(s)
        seguimiento = s
    }

    private func punto(_ e: NSEvent) -> CGPoint { convert(e.locationInWindow, from: nil) }

    /// Acercar `factor` dejando quieto el punto `p` de la vista.
    private func zoom(_ factor: Double, en p: CGPoint) {
        camara = camara.zoom(factor, x: Double(p.x) * escala, y: Double(p.y) * escala, ancho: anchoPx, alto: altoPx)
        repintar()
    }

    override public func scrollWheel(with e: NSEvent) {
        let p = punto(e)
        if e.hasPreciseScrollingDeltas && !e.modifierFlags.contains(.command) && !e.modifierFlags.contains(.control) {
            // Dos dedos en el trackpad: el plano sigue a los dedos.
            camara = camara.mover(dx: Double(e.scrollingDeltaX) * escala, dy: -Double(e.scrollingDeltaY) * escala)
            repintar()
        } else {
            // La rueda del raton (o ⌘ + dos dedos): acercar donde esta el puntero.
            let paso = e.hasPreciseScrollingDeltas ? Double(e.scrollingDeltaY) / 40 : Double(e.scrollingDeltaY)
            zoom(pow(1.2, paso), en: p)
        }
    }

    override public func magnify(with e: NSEvent) {
        zoom(1 + Double(e.magnification), en: punto(e))
    }

    override public func smartMagnify(with e: NSEvent) {
        encuadrar()
    }

    /// El enganche de la regla bajo el puntero (a 8 puntos de la pantalla).
    private func enganche(_ p: CGPoint) -> Enganche {
        let d = aDibujo(p)
        guard let plano = visor.datos?.plano else { return Enganche(x: d.x, y: d.y, tipo: .libre) }
        let radio = 8 * escala * camara.px
        return plano.enganchar(x: d.x, y: d.y, radio: radio, desde: cadena.last)
    }

    private func actualizarMedida() {
        var puntos = cadena
        if let m = mira { puntos.append(PuntoPlano(x: m.x, y: m.y)) }
        guard puntos.count >= 2 else {
            visor.ponerMedida(cadena.isEmpty ? "" : "Clic: otro punto · doble clic o Intro: guardar · Esc: soltar")
            return
        }
        var total = 0.0
        for (a, b) in zip(puntos, puntos.dropFirst()) { total += hypot(b.x - a.x, b.y - a.y) }
        let ultimo = hypot(puntos[puntos.count - 1].x - puntos[puntos.count - 2].x,
                           puntos[puntos.count - 1].y - puntos[puntos.count - 2].y)
        visor.ponerMedida(puntos.count > 2 ? "\(visor.texto(ultimo)) · total \(visor.texto(total))" : visor.texto(ultimo))
    }

    override public func mouseMoved(with e: NSEvent) {
        guard visor.modo != .mover, visor.datos != nil else { return }
        mira = enganche(punto(e))
        if visor.modo == .medir { actualizarMedida() }
        encima.needsDisplay = true
    }

    override public func mouseDown(with e: NSEvent) {
        window?.makeFirstResponder(self)
        let p = punto(e)
        switch visor.modo {
        case .mover:
            arrastre = p
            NSCursor.closedHand.push()
        case .medir:
            let m = enganche(p)
            let q = PuntoPlano(x: m.x, y: m.y)
            if e.clickCount >= 2 {
                terminarCadena()
            } else if cadena.last != q {
                cadena.append(q)
            }
            actualizarMedida()
            encima.needsDisplay = true
        case .marco:
            let m = enganche(p)
            let q = PuntoPlano(x: m.x, y: m.y)
            marcoEnCurso = (q, q)
            encima.needsDisplay = true
        }
    }

    override public func mouseDragged(with e: NSEvent) {
        let p = punto(e)
        switch visor.modo {
        case .mover:
            guard let a = arrastre else { return }
            camara = camara.mover(dx: Double(p.x - a.x) * escala, dy: Double(p.y - a.y) * escala)
            arrastre = p
            repintar()
        case .medir:
            mira = enganche(p)
            actualizarMedida()
            encima.needsDisplay = true
        case .marco:
            guard let a = marcoEnCurso?.0 else { return }
            let m = enganche(p)
            mira = m
            marcoEnCurso = (a, PuntoPlano(x: m.x, y: m.y))
            encima.needsDisplay = true
        }
    }

    override public func mouseUp(with e: NSEvent) {
        if arrastre != nil {
            arrastre = nil
            NSCursor.pop()
        }
        if visor.modo == .marco, let enCurso = marcoEnCurso {
            let (a, b) = enCurso
            marcoEnCurso = nil
            let c = CajaPlano(x0: min(a.x, b.x), y0: min(a.y, b.y), x1: max(a.x, b.x), y1: max(a.y, b.y))
            // Un clic sin arrastrar no es un marco.
            if (c.x1 - c.x0) / camara.px > 4 && (c.y1 - c.y0) / camara.px > 4 { visor.anadirMarco(c) }
            encima.needsDisplay = true
        }
    }

    // El boton derecho siempre mueve el plano, en cualquier modo.
    override public func rightMouseDown(with e: NSEvent) { arrastre = punto(e) }

    override public func rightMouseDragged(with e: NSEvent) {
        guard let a = arrastre else { return }
        let p = punto(e)
        camara = camara.mover(dx: Double(p.x - a.x) * escala, dy: Double(p.y - a.y) * escala)
        arrastre = p
        repintar()
    }

    override public func rightMouseUp(with e: NSEvent) { arrastre = nil }

    private func terminarCadena() {
        let puntos = cadena
        cadena = []
        if puntos.count >= 2 { visor.anadirCota(puntos) }
        actualizarMedida()
        encima.needsDisplay = true
    }

    override public func keyDown(with e: NSEvent) {
        switch e.keyCode {
        case 53: // Esc
            if !cadena.isEmpty || marcoEnCurso != nil {
                cadena = []
                marcoEnCurso = nil
                actualizarMedida()
                encima.needsDisplay = true
            } else if visor.modo != .mover {
                visor.modo = .mover
            }
        case 36, 76: // Intro
            terminarCadena()
        case 51, 117: // Borrar: quita el ultimo punto de la regla
            if !cadena.isEmpty {
                cadena.removeLast()
                actualizarMedida()
                encima.needsDisplay = true
            }
        default:
            let letra = e.charactersIgnoringModifiers?.lowercased() ?? ""
            switch letra {
            case "f": encuadrar()
            case "m": visor.modo = visor.modo == .medir ? .mover : .medir
            case "+", "=": zoom(1.4, en: CGPoint(x: bounds.midX, y: bounds.midY))
            case "-": zoom(1 / 1.4, en: CGPoint(x: bounds.midX, y: bounds.midY))
            default: super.keyDown(with: e)
            }
        }
    }
}

// MARK: - Lo de encima

/// **Lo que va encima del plano**, con Core Graphics: lo anotado con el
/// lapiz, las cotas (naranja, como Windows y Android), los marcos para
/// imprimir (azul), la regla en curso y la mira del enganche. No recibe el
/// raton: todo lo atiende la vista de Metal.
final class CapaEncimaPlano: NSView {
    weak var vista: VistaPlanoMetal?

    override var isOpaque: Bool { false }
    override func hitTest(_ point: NSPoint) -> NSView? { nil }

    static let naranja = NSColor(srgbRed: 1, green: 0x8a / 255.0, blue: 0, alpha: 1)
    static let azul = NSColor(srgbRed: 0x3d / 255.0, green: 0x8b / 255.0, blue: 1, alpha: 1)

    override func draw(_ dirtyRect: NSRect) {
        guard let v = vista, let ctx = NSGraphicsContext.current?.cgContext else { return }
        let visor = v.visor
        pintarTinta(v, visor, ctx)
        // Los marcos para imprimir.
        ctx.setLineWidth(1.2)
        ctx.setStrokeColor(CapaEncimaPlano.azul.cgColor)
        ctx.setLineDash(phase: 0, lengths: [6, 4])
        for m in visor.anotado.marcos {
            ctx.stroke(rect(v, m))
        }
        if let m = v.marcoEnCurso {
            let (a, b) = m
            ctx.stroke(rect(v, CajaPlano(x0: min(a.x, b.x), y0: min(a.y, b.y), x1: max(a.x, b.x), y1: max(a.y, b.y))))
        }
        ctx.setLineDash(phase: 0, lengths: [])
        // Las cotas guardadas y la regla en curso.
        for (k, c) in visor.anotado.cotas.enumerated() {
            let textos = k < visor.etiquetas.count ? visor.etiquetas[k] : []
            cadena(v, c.puntos, textos: textos, ctx: ctx)
        }
        if !v.cadena.isEmpty {
            var puntos = v.cadena
            if let m = v.mira { puntos.append(PuntoPlano(x: m.x, y: m.y)) }
            let textos = zip(puntos, puntos.dropFirst()).map { a, b in visor.texto(hypot(b.x - a.x, b.y - a.y)) }
            cadena(v, puntos, textos: textos, ctx: ctx)
        }
        if let m = v.mira, visor.modo != .mover { pintarMira(v.aVista(PuntoPlano(x: m.x, y: m.y)), m.tipo, ctx) }
    }

    private func rect(_ v: VistaPlanoMetal, _ c: CajaPlano) -> CGRect {
        let a = v.aVista(PuntoPlano(x: c.x0, y: c.y0)), b = v.aVista(PuntoPlano(x: c.x1, y: c.y1))
        return CGRect(x: min(a.x, b.x), y: min(a.y, b.y), width: abs(b.x - a.x), height: abs(b.y - a.y))
    }

    /// Una cadena de cotas: la raya, sus puntos y cada medida en su tramo.
    private func cadena(_ v: VistaPlanoMetal, _ puntos: [PuntoPlano], textos: [String], ctx: CGContext) {
        let q = puntos.map { v.aVista($0) }
        guard q.count >= 2 else {
            if let p = q.first { marca(p, ctx) }
            return
        }
        ctx.setStrokeColor(CapaEncimaPlano.naranja.cgColor)
        ctx.setLineWidth(1.5)
        ctx.addLines(between: q)
        ctx.strokePath()
        for p in q { marca(p, ctx) }
        let atributos: [NSAttributedString.Key: Any] = [
            .font: NSFont.monospacedDigitSystemFont(ofSize: 11, weight: .semibold),
            .foregroundColor: NSColor.white,
            .backgroundColor: CapaEncimaPlano.naranja.withAlphaComponent(0.9),
        ]
        for (i, t) in textos.enumerated() where i + 1 < q.count {
            let m = CGPoint(x: (q[i].x + q[i + 1].x) / 2, y: (q[i].y + q[i + 1].y) / 2)
            let s = NSAttributedString(string: " \(t) ", attributes: atributos)
            let tam = s.size()
            s.draw(at: CGPoint(x: m.x - tam.width / 2, y: m.y + 3))
        }
    }

    private func marca(_ p: CGPoint, _ ctx: CGContext) {
        ctx.setFillColor(CapaEncimaPlano.naranja.cgColor)
        ctx.fillEllipse(in: CGRect(x: p.x - 3, y: p.y - 3, width: 6, height: 6))
    }

    /// La mira: un cuadrado en un punto, una escuadra en la perpendicular,
    /// una cruz en la raya y un circulo libre (las marcas de AutoCAD).
    private func pintarMira(_ p: CGPoint, _ tipo: TipoEnganche, _ ctx: CGContext) {
        let r: CGFloat = 6
        ctx.setLineWidth(1.5)
        ctx.setStrokeColor(NSColor.systemGreen.cgColor)
        switch tipo {
        case .punto:
            ctx.stroke(CGRect(x: p.x - r, y: p.y - r, width: 2 * r, height: 2 * r))
        case .perpendicular:
            ctx.addLines(between: [CGPoint(x: p.x - r, y: p.y + r), CGPoint(x: p.x - r, y: p.y - r), CGPoint(x: p.x + r, y: p.y - r)])
            ctx.addLines(between: [CGPoint(x: p.x - r, y: p.y), CGPoint(x: p.x, y: p.y), CGPoint(x: p.x, y: p.y - r)])
            ctx.strokePath()
        case .enLaRaya:
            ctx.addLines(between: [CGPoint(x: p.x - r, y: p.y - r), CGPoint(x: p.x + r, y: p.y + r)])
            ctx.addLines(between: [CGPoint(x: p.x - r, y: p.y + r), CGPoint(x: p.x + r, y: p.y - r)])
            ctx.strokePath()
        case .libre:
            ctx.setStrokeColor(NSColor.secondaryLabelColor.cgColor)
            ctx.strokeEllipse(in: CGRect(x: p.x - r / 2, y: p.y - r / 2, width: r, height: r))
        }
    }

    /// Lo anotado con el lapiz, en su sitio: `x = cero_x + x_capa * u`,
    /// `y = cero_y - y_capa * u` (la y de la capa baja).
    private func pintarTinta(_ v: VistaPlanoMetal, _ visor: VisorPlano, _ ctx: CGContext) {
        guard let tinta = visor.tinta, let e = visor.encaje, e.u > 0 else { return }
        let c = v.camara
        let s = c.px * v.escala // unidades del dibujo por punto
        let a = e.u / s
        let t = CGAffineTransform(a: a, b: 0, c: 0, d: -a,
                                  tx: (e.ceroX - c.centroX) / s + Double(v.bounds.midX),
                                  ty: (e.ceroY - c.centroY) / s + Double(v.bounds.midY))
        ctx.saveGState()
        ctx.concatenate(t)
        let limites = v.bounds.applying(t.inverted())
        PintorLienzo(lienzo: tinta).pintar(en: ctx, limites: limites)
        ctx.restoreGState()
    }
}

// MARK: - SwiftUI

/// La vista de Metal dentro de SwiftUI.
struct LienzoMetalPlano: NSViewRepresentable {
    let visor: VisorPlano
    let motor: MotorMetal

    func makeNSView(context _: Context) -> VistaPlanoMetal { VistaPlanoMetal(visor: visor, motor: motor) }
    func updateNSView(_ v: VistaPlanoMetal, context _: Context) { v.repintar() }
}

/// **La ventana de un plano**: la barra (modos, encuadrar, imprimir, 3D),
/// el plano y el panel de capas.
public struct VentanaPlanoView: View {
    @ObservedObject var visor: VisorPlano
    let alVerEn3D: () -> Void
    @State private var verCapas = true

    public init(visor: VisorPlano, alVerEn3D: @escaping () -> Void) {
        self.visor = visor
        self.alVerEn3D = alVerEn3D
    }

    public var body: some View {
        VStack(spacing: 0) {
            barra
            Divider()
            HStack(spacing: 0) {
                contenido
                if verCapas && visor.datos != nil {
                    Divider()
                    panelCapas.frame(width: 230)
                }
            }
        }
    }

    @ViewBuilder private var contenido: some View {
        if let error = visor.error {
            ContentUnavailableView("No se puede ver el plano", systemImage: "exclamationmark.triangle",
                                   description: Text(error))
        } else if let motor = MotorMetal.compartido {
            ZStack {
                LienzoMetalPlano(visor: visor, motor: motor)
                if visor.datos == nil { ProgressView("Leyendo el plano…") }
            }
        } else {
            ContentUnavailableView("Sin Metal", systemImage: "cpu", description: Text("Este Mac no tiene una tarjeta gráfica con Metal."))
        }
    }

    private var barra: some View {
        HStack(spacing: 10) {
            Picker("", selection: $visor.modo) {
                Label("Mover", systemImage: "hand.raised").tag(ModoVisorPlano.mover)
                Label("Medir", systemImage: "ruler").tag(ModoVisorPlano.medir)
                Label("Marco", systemImage: "rectangle.dashed").tag(ModoVisorPlano.marco)
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .frame(width: 260)
            Button { visor.encuadrar() } label: { Image(systemName: "arrow.up.left.and.arrow.down.right") }
                .help("Ver el plano entero (F)")
            Menu {
                Button("Borrar las cotas") { visor.borrarCotas() }
                    .disabled(visor.anotado.cotas.isEmpty)
                Button("Borrar los marcos") { visor.borrarMarcos() }
                    .disabled(visor.anotado.marcos.isEmpty)
            } label: { Image(systemName: "trash") }
                .menuIndicator(.hidden)
                .fixedSize()
            Text(visor.medida)
                .font(.callout.monospacedDigit())
                .foregroundStyle(.secondary)
                .lineLimit(1)
            Spacer()
            Button { alVerEn3D() } label: { Label("3D", systemImage: "cube") }
                .help("Ver el plano en 3D")
            Button { visor.imprimir() } label: { Image(systemName: "printer") }
                .help("Imprimir (el último marco, o lo que se ve)")
                .keyboardShortcut("p")
            Toggle(isOn: $verCapas) { Image(systemName: "square.3.layers.3d") }
                .toggleStyle(.button)
                .help("Capas")
        }
        .disabled(visor.datos == nil)
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(.bar)
    }

    private var panelCapas: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text("Capas").font(.headline)
                Spacer()
                Button("Todas") { visor.todasLasCapas(true) }.buttonStyle(.link)
                Button("Ninguna") { visor.todasLasCapas(false) }.buttonStyle(.link)
            }
            .padding(8)
            List(visor.capas, id: \.nombre) { c in
                Toggle(isOn: Binding(get: { c.encendida }, set: { visor.ponerCapa(c.nombre, encendida: $0) })) {
                    Text(c.nombre)
                        .foregroundStyle(c.enElPlano ? .primary : .secondary)
                        .lineLimit(1)
                        .help(c.enElPlano ? c.nombre : "\(c.nombre) (apagada en el plano)")
                }
                .toggleStyle(.checkbox)
            }
            .listStyle(.sidebar)
        }
    }
}
