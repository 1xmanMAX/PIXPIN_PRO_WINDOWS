// La superposicion de la captura: un panel sin bordes por pantalla, encima
// de todo, con la foto congelada, oscurecida salvo la zona elegida.
//
// Rendimiento (§4bis del plan):
// - la vista aloja sus capas (`layer` propio): la foto es el `contents` de
//   una capa y la compone la tarjeta grafica; mover el raton solo cambia un
//   camino (el velo), un borde y una etiqueta, nunca se redibuja la foto;
// - la lupa es otra capa con la MISMA foto y un `contentsRect`: cero copias;
// - nada de temporizadores: todo va por los eventos del raton y el teclado;
// - lo anotado encima son capas vectoriales (`CAShapeLayer`, `CATextLayer`)
//   dentro de una capa recortada a la zona; el mosaico, el trozo pixelado
//   por Core Image (`AnotacionCaptura`). La foto nunca se redibuja.

import AppKit
import CoreText
import QuartzCore

/// Que se elige al arrastrar o hacer clic (las letras de Windows).
enum ModoCaptura {
    /// `Z`: arrastrar una zona; un clic elige la ventana bajo el raton.
    case zona
    /// `V`: solo ventanas (resalta la de debajo; un clic la elige).
    case ventana
    /// `P`: la pantalla entera.
    case pantalla
    /// `T`: como zona, y al soltar saca el texto (OCR).
    case texto
    /// `S`: como zona, y al soltar la recorre bajando (captura con scroll).
    case scroll
    /// `G`: como zona, y al soltar la graba en GIF.
    case gif
    /// `L`: como zona, y al soltar saca un pin en vivo.
    case pinEnVivo

    /// Los de la barra de abajo, con su letra (las de Windows).
    static let enBarra: [(ModoCaptura, String)] = [
        (.zona, "Zona Z"), (.ventana, "Ventana V"), (.pantalla, "Pantalla P"), (.scroll, "Scroll S"),
        (.gif, "GIF G"), (.pinEnVivo, "En vivo L"), (.texto, "Texto T"),
    ]

    /// Si al elegir la zona se hace algo en el acto (sin barra de acciones).
    var actuaAlElegir: Bool {
        switch self {
        case .texto, .scroll, .gif, .pinEnVivo: true
        case .zona, .ventana, .pantalla: false
        }
    }
}

/// Lo que se hace con la zona elegida (la barra y sus atajos).
enum AccionCaptura: Int, CaseIterable {
    case copiar, pinear, alChat, texto, guardar, grabar, cerrar

    var titulo: String {
        switch self {
        case .copiar: "Copiar"
        case .pinear: "Pinear"
        case .alChat: "Al chat"
        case .texto: "Texto"
        case .guardar: "Guardar"
        case .grabar: "Grabar"
        case .cerrar: "Cerrar"
        }
    }

    var simbolo: String {
        switch self {
        case .copiar: "doc.on.doc"
        case .pinear: "pin"
        case .alChat: "bubble.left"
        case .texto: "text.viewfinder"
        case .guardar: "square.and.arrow.down"
        case .grabar: "record.circle"
        case .cerrar: "xmark"
        }
    }

    var atajo: String {
        switch self {
        case .copiar: "Intro o ⌘C"
        case .pinear: "⌃P o ⌘P"
        case .alChat: "⌃M o ⌘M"
        case .texto: "⌃T o ⌘T"
        case .guardar: "⌃S o ⌘S"
        case .grabar: "⌃G o ⌘G"
        case .cerrar: "Esc"
        }
    }
}

/// El panel de cada pantalla. Sin bordes no podria recibir el teclado: se
/// le deja ser ventana clave (sin activar la app, para no robar el foco a
/// la app de debajo al cerrar).
final class PanelCaptura: NSPanel {
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }

    /// Un panel que cubre `foto.pantalla` con su vista y su barra.
    @MainActor
    static func abrir(foto: FotoDePantalla, ventanas: [VentanaVisible], capturador: Capturador) -> PanelCaptura {
        let marco = foto.pantalla.frame
        let panel = PanelCaptura(contentRect: marco, styleMask: [.borderless, .nonactivatingPanel],
                                 backing: .buffered, defer: false)
        panel.setFrame(marco, display: false)
        panel.level = .screenSaver
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]
        panel.isOpaque = true
        panel.backgroundColor = .black
        panel.hasShadow = false
        panel.isReleasedWhenClosed = false
        panel.hidesOnDeactivate = false
        panel.isMovable = false
        panel.acceptsMouseMovedEvents = true
        panel.animationBehavior = .none

        // Las ventanas que tocan esta pantalla, en sus coordenadas.
        let locales = ventanas.compactMap { v -> VentanaVisible? in
            let r = GeometriaCaptura.enPantalla(v.marco, origenPantalla: marco.origin)
            let dentro = GeometriaCaptura.ajustar(r, a: CGRect(origin: .zero, size: marco.size))
            return dentro.isEmpty ? nil : VentanaVisible(marco: dentro, nombre: v.nombre)
        }
        let limites = NSRect(origin: .zero, size: marco.size)
        let contenedor = NSView(frame: limites)
        let vista = VistaCaptura(frame: limites, foto: foto.imagen,
                                 escalaPantalla: foto.pantalla.backingScaleFactor, ventanas: locales)
        vista.capturador = capturador
        vista.autoresizingMask = [.width, .height]
        contenedor.addSubview(vista)
        let barra = VistaCaptura.hacerBarra(destino: vista, disponible: { capturador.disponible($0) })
        contenedor.addSubview(barra)
        vista.barra = barra
        let modos = VistaCaptura.hacerBarraModos(destino: vista)
        contenedor.addSubview(modos)
        vista.barraModos = modos
        panel.contentView = contenedor
        vista.colocarBarraModos()
        panel.initialFirstResponder = vista
        panel.makeFirstResponder(vista)
        return panel
    }
}

final class VistaCaptura: NSView {
    weak var capturador: Capturador?
    weak var barra: NSView?
    weak var barraModos: NSView?

    private var foto: CGImage?
    private var ventanas: [VentanaVisible]
    private let escalaPantalla: CGFloat

    /// La zona elegida o la que se esta arrastrando (puntos de la vista).
    private(set) var seleccion: CGRect = .zero
    /// Si ya hay zona y se esperan acciones.
    private(set) var elegida = false
    private var resaltada: VentanaVisible?
    private var inicio: CGPoint?
    private var arrastrando = false
    private var seguimiento: NSTrackingArea?

    /// Lo anotado encima de la zona (puntos relativos a su esquina de
    /// abajo a la izquierda).
    private(set) var anotacion = AnotacionCaptura()
    /// Si el raton esta dibujando (no eligiendo zona).
    private var anotando = false
    /// El color bajo el raton (para `C`).
    private(set) var colorBajoRaton: (UInt8, UInt8, UInt8)?

    private let raiz = CALayer()
    private let capaFoto = CALayer()
    private let capaVelo = CAShapeLayer()
    private let capaBorde = CAShapeLayer()
    private let capaEtiqueta = CATextLayer()
    private let capaLupa = CALayer()
    private let capaColor = CATextLayer()
    /// Lo dibujado: una capa por figura, recortada a la zona.
    private let capaDibujo = CALayer()
    private var capasFiguras: [CALayer] = []
    private var capaEnCurso: CALayer?
    private let fuente = NSFont.monospacedDigitSystemFont(ofSize: 12, weight: .medium)

    /// La lupa: ensena `ladoLupa` puntos de la foto en `tamLupa` puntos.
    private let ladoLupa: CGFloat = 16
    private let tamLupa: CGFloat = 112

    // Los controles de las barras que cambian de estado.
    var botonesHerramienta: [NSButton] = []
    var botonesColor: [NSButton] = []
    var botonesGrosor: [NSButton] = []
    var selectorModo: NSSegmentedControl?
    var selectorProporcion: NSSegmentedControl?
    var campoMedidas: NSTextField?
    var botonRepetir: NSButton?

    init(frame: NSRect, foto: CGImage, escalaPantalla: CGFloat, ventanas: [VentanaVisible]) {
        self.foto = foto
        self.ventanas = ventanas
        self.escalaPantalla = escalaPantalla
        super.init(frame: frame)
        // Vista que aloja capas: la capa es nuestra y no se redibuja sola.
        layer = raiz
        wantsLayer = true
        raiz.backgroundColor = NSColor.black.cgColor

        capaFoto.contents = foto
        capaFoto.contentsGravity = .resize

        capaVelo.fillColor = NSColor(white: 0, alpha: 0.45).cgColor
        capaVelo.fillRule = .evenOdd

        capaBorde.fillColor = nil
        capaBorde.strokeColor = NSColor.controlAccentColor.cgColor
        capaBorde.lineWidth = 1.5

        capaEtiqueta.font = fuente
        capaEtiqueta.fontSize = fuente.pointSize
        capaEtiqueta.foregroundColor = NSColor.white.cgColor
        capaEtiqueta.backgroundColor = NSColor(white: 0, alpha: 0.72).cgColor
        capaEtiqueta.cornerRadius = 4
        capaEtiqueta.alignmentMode = .center
        capaEtiqueta.isHidden = true

        capaLupa.contents = foto
        capaLupa.contentsGravity = .resize
        capaLupa.magnificationFilter = .nearest
        capaLupa.borderColor = NSColor.white.cgColor
        capaLupa.borderWidth = 1.5
        capaLupa.cornerRadius = 8
        capaLupa.masksToBounds = true
        capaLupa.isHidden = true

        capaColor.font = fuente
        capaColor.fontSize = 11
        capaColor.foregroundColor = NSColor.white.cgColor
        capaColor.backgroundColor = NSColor(white: 0, alpha: 0.78).cgColor
        capaColor.cornerRadius = 4
        capaColor.alignmentMode = .center
        capaColor.isHidden = true

        capaDibujo.masksToBounds = true

        for capa in [capaFoto, capaDibujo, capaVelo, capaBorde, capaEtiqueta, capaLupa, capaColor] {
            capa.contentsScale = escalaPantalla
            raiz.addSublayer(capa)
        }
        colocarCapas()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("no se crea desde un nib") }

    // MARK: - Capas

    override func setFrameSize(_ nuevo: NSSize) {
        super.setFrameSize(nuevo)
        colocarCapas()
    }

    private func colocarCapas() {
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        for capa in [capaFoto, capaVelo, capaBorde] { capa.frame = bounds }
        CATransaction.commit()
        redibujar()
    }

    /// Lo que se ilumina ahora: la zona elegida o arrastrada, o la ventana
    /// bajo el raton.
    private var zonaVisible: CGRect {
        if elegida || arrastrando { return seleccion }
        return resaltada?.marco ?? .zero
    }

    private func redibujar() {
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        let zona = zonaVisible
        let velo = CGMutablePath()
        velo.addRect(bounds)
        if !zona.isEmpty { velo.addRect(zona) }
        capaVelo.path = velo
        capaBorde.path = zona.isEmpty ? nil : CGPath(rect: zona, transform: nil)
        capaDibujo.frame = elegida ? seleccion : .zero

        guard !zona.isEmpty, let foto else {
            capaEtiqueta.isHidden = true
            return
        }
        let px = GeometriaCaptura.rectEnPixeles(zona, tamVista: bounds.size, ancho: foto.width, alto: foto.height)
        var texto = "\(Int(px.width)) × \(Int(px.height))"
        if !elegida, !arrastrando, let nombre = resaltada?.nombre, !nombre.isEmpty {
            texto = "\(nombre)  ·  \(texto)"
        }
        if let par = capturador?.proporcion.par {
            texto += "  ·  \(par.0):\(par.1)"
        }
        capaEtiqueta.string = texto
        let tam = (texto as NSString).size(withAttributes: [.font: fuente])
        let ancho = min(tam.width.rounded(.up) + 14, bounds.width)
        let alto = tam.height.rounded(.up) + 4
        var origen = CGPoint(x: zona.minX, y: zona.maxY + 4)
        if origen.y + alto > bounds.maxY { origen.y = zona.maxY - alto - 4 }
        origen.x = max(bounds.minX, min(origen.x, bounds.maxX - ancho))
        origen.y = max(bounds.minY, origen.y)
        capaEtiqueta.frame = CGRect(origin: origen, size: CGSize(width: ancho, height: alto))
        capaEtiqueta.isHidden = false
    }

    private func moverLupa(a p: CGPoint) {
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        guard !elegida, let foto, bounds.contains(p) else {
            capaLupa.isHidden = true
            capaColor.isHidden = true
            colorBajoRaton = nil
            return
        }
        var origen = CGPoint(x: p.x + 20, y: p.y - 20 - tamLupa)
        if origen.x + tamLupa > bounds.maxX { origen.x = p.x - 20 - tamLupa }
        if origen.y - 36 < bounds.minY { origen.y = p.y + 20 + 36 }
        capaLupa.frame = CGRect(origin: origen, size: CGSize(width: tamLupa, height: tamLupa))
        capaLupa.contentsRect = GeometriaCaptura.rectDeLupa(en: p, lado: ladoLupa, tamVista: bounds.size)
        capaLupa.isHidden = false

        // El color del pixel: un recorte de 1x1 (sin copiar la foto) pintado
        // en un mapa de bits de 4 bytes.
        colorBajoRaton = VistaCaptura.color(de: foto, en: p, tamVista: bounds.size)
        if let c = colorBajoRaton {
            capaColor.string = "\(MedidasCaptura.hex(c.0, c.1, c.2))   C copia\n\(MedidasCaptura.rgb(c.0, c.1, c.2))"
            capaColor.frame = CGRect(x: origen.x, y: origen.y - 34, width: tamLupa, height: 32)
            capaColor.isHidden = false
        } else {
            capaColor.isHidden = true
        }
    }

    /// El color (sRGB) del pixel de la foto bajo el punto `p` de la vista.
    nonisolated static func color(de foto: CGImage, en p: CGPoint, tamVista: CGSize) -> (UInt8, UInt8, UInt8)? {
        guard let px = MedidasCaptura.pixel(en: p, tamVista: tamVista, ancho: foto.width, alto: foto.height),
              let uno = foto.cropping(to: CGRect(x: px.x, y: px.y, width: 1, height: 1)) else { return nil }
        var bytes = [UInt8](repeating: 0, count: 4)
        let info = CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.byteOrder32Big.rawValue
        let hecho = bytes.withUnsafeMutableBytes { buf -> Bool in
            guard let ctx = CGContext(data: buf.baseAddress, width: 1, height: 1, bitsPerComponent: 8, bytesPerRow: 4,
                                      space: CGColorSpace(name: CGColorSpace.sRGB) ?? CGColorSpaceCreateDeviceRGB(),
                                      bitmapInfo: info) else { return false }
            ctx.interpolationQuality = .none
            ctx.draw(uno, in: CGRect(x: 0, y: 0, width: 1, height: 1))
            return true
        }
        return hecho ? (bytes[0], bytes[1], bytes[2]) : nil
    }

    private func colocarBarra() {
        guard let barra else { return }
        let o = GeometriaCaptura.posicionBarra(seleccion: seleccion, tamBarra: barra.frame.size, limites: bounds)
        barra.setFrameOrigin(o)
        barra.isHidden = false
        colocarBarraModos()
    }

    /// La barra de los modos, abajo en el centro; arriba si la zona o la
    /// barra de acciones la tapan.
    func colocarBarraModos() {
        guard let barraModos else { return }
        let t = barraModos.frame.size
        var o = CGPoint(x: (bounds.midX - t.width / 2).rounded(), y: bounds.minY + 24)
        let abajo = CGRect(origin: o, size: t)
        let tapaBarra = barra.map { !$0.isHidden && abajo.intersects($0.frame) } ?? false
        if elegida, abajo.intersects(seleccion) || tapaBarra {
            o.y = bounds.maxY - t.height - 36
        }
        barraModos.setFrameOrigin(o)
        actualizarBarraModos()
    }

    // MARK: - Lo que pide el capturador

    /// Vuelve a elegir desde cero (otra pantalla empezo una zona, o clic
    /// derecho). Lo anotado se va con la zona.
    func reiniciar() {
        seleccion = .zero
        elegida = false
        inicio = nil
        arrastrando = false
        anotando = false
        anotacion.vaciar()
        sincronizarDibujo()
        actualizarBarraAnotar()
        barra?.isHidden = true
        redibujar()
        colocarBarraModos()
    }

    /// Elige `r` como zona y ensena la barra (o hace lo del modo).
    func elegir(_ r: CGRect) {
        let zona = GeometriaCaptura.ajustar(r, a: bounds)
        guard !zona.isEmpty else { return }
        if zona != seleccion {
            anotacion.vaciar()
            sincronizarDibujo()
        }
        seleccion = zona
        elegida = true
        resaltada = nil
        moverLupa(a: CGPoint(x: -1, y: -1))
        redibujar()
        sincronizarDibujo()
        colocarBarra()
        capturador?.alElegir(en: self)
    }

    /// Elige la pantalla entera (la tecla `P`).
    func elegirPantalla() { elegir(bounds) }

    /// La pantalla de esta vista (para recordar su ultima zona).
    var numeroDePantalla: CGDirectDisplayID? { window?.screen?.numeroDePantalla }

    /// El trozo elegido de la foto, en pixeles, con lo anotado dentro. Sin
    /// anotar, `cropping(to:)` no copia ni recodifica: comparte los pixeles
    /// de la foto; anotado, es un mapa de bits nuevo del tamano del recorte.
    func recorte() -> CGImage? {
        guard elegida, let foto else { return nil }
        let px = GeometriaCaptura.rectEnPixeles(seleccion, tamVista: bounds.size, ancho: foto.width, alto: foto.height)
        guard !px.isEmpty, let trozo = foto.cropping(to: px) else { return nil }
        anotacion.cerrarTexto()
        return AnotacionCaptura.quemar(anotacion.figuras, en: trozo, escala: escalaFoto)
    }

    /// Si hay algo anotado (entonces el recorte ya es una copia aparte).
    var anotada: Bool { !anotacion.vacia }

    /// La zona elegida para grabarla en video (`Grabar/`): su pantalla y la
    /// zona en puntos con el origen arriba, como la pide ScreenCaptureKit.
    func zonaDeGrabacion() -> ZonaDeGrabacion? {
        guard elegida, let numero = window?.screen?.numeroDePantalla else { return nil }
        let rect = GeometriaGrabacion.desdeVista(seleccion, altoPantalla: bounds.height)
        return ZonaDeGrabacion(pantalla: numero, rect: rect, escala: escalaFoto)
    }

    /// La zona elegida en coordenadas globales de AppKit (para el pin en
    /// vivo).
    func zonaGlobal() -> CGRect? {
        guard elegida, let w = window else { return nil }
        return seleccion.offsetBy(dx: w.frame.minX, dy: w.frame.minY)
    }

    /// Pixeles por punto de la foto (para el PPP del PNG).
    var escalaFoto: CGFloat {
        guard let foto, bounds.width > 0 else { return escalaPantalla }
        return CGFloat(foto.width) / bounds.width
    }

    /// El tamano de la foto entera, en pixeles.
    var tamFoto: CGSize {
        guard let foto else { return .zero }
        return CGSize(width: foto.width, height: foto.height)
    }

    /// Suelta la foto (al cerrar): las capas dejan de retenerla.
    func soltar() {
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        capaFoto.contents = nil
        capaLupa.contents = nil
        for c in capasFiguras { c.removeFromSuperlayer() }
        capaEnCurso?.removeFromSuperlayer()
        CATransaction.commit()
        capasFiguras = []
        capaEnCurso = nil
        anotacion.vaciar()
        foto = nil
        ventanas = []
        resaltada = nil
    }

    // MARK: - Medidas, proporcion y repetir

    /// Aplica la proporcion a la zona ya elegida (en el sitio).
    func aplicarProporcion(_ p: ProporcionCaptura) {
        actualizarBarraModos()
        guard elegida else {
            redibujar()
            return
        }
        elegir(MedidasCaptura.ajustar(seleccion, a: p, limites: bounds))
    }

    /// Las medidas tecleadas (en pixeles): desde la esquina de la zona o
    /// centradas. `false` si no se entienden.
    @discardableResult
    func aplicarMedidas(_ texto: String) -> Bool {
        guard let m = MedidasCaptura.leer(texto) else { return false }
        let r = MedidasCaptura.zona(ancho: m.ancho, alto: m.alto, escala: escalaFoto,
                                    seleccion: elegida ? seleccion : nil, limites: bounds)
        guard !r.isEmpty else { return false }
        capturador?.empezo(en: self)
        elegir(r)
        return true
    }

    /// `R`: la ultima zona de esta pantalla. `false` si no hay.
    @discardableResult
    func repetirUltima() -> Bool {
        guard let n = numeroDePantalla, let r = UltimaZona().leer(pantalla: n) else { return false }
        capturador?.empezo(en: self)
        elegir(r)
        return true
    }

    // MARK: - Anotar

    /// Coge una herramienta de anotar (la misma otra vez la suelta).
    func tomar(_ h: HerramientaAnotar) {
        guard elegida else { return }
        anotacion.tomar(h)
        sincronizarDibujo()
        actualizarBarraAnotar()
        window?.invalidateCursorRects(for: self)
    }

    func ponerColor(_ i: Int) {
        anotacion.ponerColor(i)
        sincronizarDibujo()
        actualizarBarraAnotar()
    }

    func ponerGrosor(_ i: Int) {
        anotacion.ponerGrosor(i)
        sincronizarDibujo()
        actualizarBarraAnotar()
    }

    @discardableResult
    func deshacer() -> Bool {
        let hecho = anotacion.deshacer()
        sincronizarDibujo()
        return hecho
    }

    @discardableResult
    func rehacer() -> Bool {
        let hecho = anotacion.rehacer()
        sincronizarDibujo()
        return hecho
    }

    private func relativo(_ p: CGPoint) -> CGPoint {
        CGPoint(x: p.x - seleccion.minX, y: p.y - seleccion.minY)
    }

    /// Las capas de lo dibujado al dia con el modelo: si cambio el numero de
    /// figuras se rehacen (son pocas), y la de en curso se cambia sola.
    private func sincronizarDibujo() {
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        if capasFiguras.count != anotacion.figuras.count {
            for c in capasFiguras { c.removeFromSuperlayer() }
            capasFiguras = anotacion.figuras.map { capa(de: $0, enCurso: false) }
            for c in capasFiguras { capaDibujo.addSublayer(c) }
        }
        capaEnCurso?.removeFromSuperlayer()
        capaEnCurso = nil
        if let f = anotacion.enCurso {
            let c = capa(de: f, enCurso: true)
            capaDibujo.addSublayer(c)
            capaEnCurso = c
        }
    }

    /// Una capa para una figura: vectorial (trazos y texto) o el trozo
    /// pixelado (mosaico terminado). Mientras se arrastra un mosaico, banda
    /// maciza.
    private func capa(de f: FiguraAnotada, enCurso: Bool) -> CALayer {
        switch f.clase {
        case let .mosaico(r):
            let c = CALayer()
            c.frame = r
            c.magnificationFilter = .nearest
            if !enCurso, let foto {
                let enVista = r.offsetBy(dx: seleccion.minX, dy: seleccion.minY)
                let px = GeometriaCaptura.rectEnPixeles(enVista, tamVista: bounds.size, ancho: foto.width, alto: foto.height)
                let bloque = AnotacionCaptura.bloques[min(max(f.grosor, 0), AnotacionCaptura.bloques.count - 1)]
                c.contents = AnotacionCaptura.pixelado(foto, rectPx: px, bloquePx: bloque * escalaFoto)
            }
            if c.contents == nil { c.backgroundColor = CGColor(gray: 0.55, alpha: 1) }
            return c
        case let .texto(t, p):
            let c = CATextLayer()
            let linea = AnotacionCaptura.lineaDeTexto(t.isEmpty ? " " : t, figura: f)
            var ascenso: CGFloat = 0
            var descenso: CGFloat = 0
            let ancho = CGFloat(CTLineGetTypographicBounds(linea, &ascenso, &descenso, nil))
            let letra = AnotacionCaptura.fuente(f.grosor)
            c.font = letra
            c.fontSize = CTFontGetSize(letra)
            c.foregroundColor = AnotacionCaptura.cgColor(f.color)
            c.string = enCurso ? t + "▏" : t
            c.contentsScale = escalaPantalla
            c.frame = CGRect(x: p.x, y: p.y - descenso, width: ancho + c.fontSize, height: ascenso + descenso)
            if enCurso {
                c.borderWidth = 1
                c.borderColor = NSColor.white.withAlphaComponent(0.6).cgColor
            }
            return c
        default:
            let c = CAShapeLayer()
            c.frame = CGRect(origin: .zero, size: seleccion.size)
            c.path = AnotacionCaptura.camino(de: f)
            c.fillColor = nil
            c.strokeColor = AnotacionCaptura.cgColor(f.color)
            c.lineWidth = AnotacionCaptura.grosores[min(max(f.grosor, 0), AnotacionCaptura.grosores.count - 1)]
            c.lineCap = .round
            c.lineJoin = .round
            c.contentsScale = escalaPantalla
            return c
        }
    }

    func actualizarBarraAnotar() {
        for b in botonesHerramienta { b.state = b.tag == anotacion.herramienta?.rawValue ? .on : .off }
        for b in botonesColor { b.layer?.borderWidth = b.tag == anotacion.color ? 2.5 : 0 }
        for b in botonesGrosor { b.state = b.tag == anotacion.grosor ? .on : .off }
    }

    func actualizarBarraModos() {
        guard let capturador else { return }
        if let i = ModoCaptura.enBarra.firstIndex(where: { $0.0 == capturador.modo }) {
            selectorModo?.selectedSegment = i
        }
        selectorProporcion?.selectedSegment = capturador.proporcion.rawValue
        if let n = numeroDePantalla { botonRepetir?.isEnabled = UltimaZona().leer(pantalla: n) != nil }
        if let foto, elegida, let campo = campoMedidas {
            let px = GeometriaCaptura.rectEnPixeles(seleccion, tamVista: bounds.size, ancho: foto.width, alto: foto.height)
            campo.placeholderString = "\(Int(px.width)) × \(Int(px.height))"
        } else {
            campoMedidas?.placeholderString = "ancho × alto"
        }
    }

    // MARK: - Raton

    override var acceptsFirstResponder: Bool { true }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    override func updateTrackingAreas() {
        if let seguimiento { removeTrackingArea(seguimiento) }
        let area = NSTrackingArea(rect: .zero,
                                  options: [.mouseMoved, .mouseEnteredAndExited, .activeAlways, .inVisibleRect],
                                  owner: self, userInfo: nil)
        addTrackingArea(area)
        seguimiento = area
        super.updateTrackingAreas()
    }

    override func resetCursorRects() {
        addCursorRect(bounds, cursor: .crosshair)
        if elegida, anotacion.herramienta == .texto { addCursorRect(seleccion, cursor: .iBeam) }
    }

    private func punto(_ e: NSEvent) -> CGPoint {
        convert(e.locationInWindow, from: nil)
    }

    /// Resalta la ventana bajo `p` (si aun no hay zona).
    func seguirRaton(a p: CGPoint) {
        guard !elegida, inicio == nil else { return }
        let modo = capturador?.modo ?? .zona
        let nueva = modo == .pantalla ? nil : ventanas.first { $0.marco.contains(p) }
        if nueva?.marco != resaltada?.marco {
            resaltada = nueva
            redibujar()
        }
        moverLupa(a: p)
    }

    override func mouseEntered(with e: NSEvent) {
        window?.makeKey()
        capturador?.activar(self)
        NSCursor.crosshair.set()
    }

    override func mouseExited(with e: NSEvent) {
        moverLupa(a: CGPoint(x: -1, y: -1))
        if !elegida, resaltada != nil {
            resaltada = nil
            redibujar()
        }
    }

    override func mouseMoved(with e: NSEvent) {
        let p = punto(e)
        let escribir = elegida && anotacion.herramienta == .texto && seleccion.contains(p)
        (escribir ? NSCursor.iBeam : NSCursor.crosshair).set()
        seguirRaton(a: p)
    }

    override func mouseDown(with e: NSEvent) {
        let p = punto(e)
        window?.makeFirstResponder(self)
        // Con una herramienta cogida, dentro de la zona se dibuja (y fuera
        // no pasa nada: clic derecho vuelve a elegir).
        if elegida, anotacion.herramienta != nil {
            if seleccion.contains(p) {
                anotacion.pulsar(relativo(p))
                anotando = true
                sincronizarDibujo()
            }
            return
        }
        capturador?.empezo(en: self)
        if capturador?.modo == .pantalla {
            elegirPantalla()
            return
        }
        barra?.isHidden = true
        if elegida {
            anotacion.vaciar()
            sincronizarDibujo()
        }
        elegida = false
        seleccion = .zero
        arrastrando = false
        inicio = p
        colocarBarraModos()
    }

    override func mouseDragged(with e: NSEvent) {
        let p = punto(e)
        if anotando {
            let dentro = CGPoint(x: min(max(p.x, seleccion.minX), seleccion.maxX),
                                 y: min(max(p.y, seleccion.minY), seleccion.maxY))
            anotacion.mover(relativo(dentro))
            sincronizarDibujo()
            return
        }
        guard let inicio else { return }
        if !arrastrando, GeometriaCaptura.esArrastre(desde: inicio, hasta: p) { arrastrando = true }
        if arrastrando, capturador?.modo != .ventana {
            let r = MedidasCaptura.atado(desde: inicio, hasta: p, proporcion: capturador?.proporcion ?? .libre)
            seleccion = GeometriaCaptura.ajustar(r, a: bounds)
            redibujar()
        }
        moverLupa(a: p)
    }

    override func mouseUp(with e: NSEvent) {
        let p = punto(e)
        if anotando {
            anotando = false
            let dentro = CGPoint(x: min(max(p.x, seleccion.minX), seleccion.maxX),
                                 y: min(max(p.y, seleccion.minY), seleccion.maxY))
            anotacion.soltar(relativo(dentro))
            sincronizarDibujo()
            return
        }
        guard inicio != nil else { return }
        let zona = seleccion
        let fueArrastre = arrastrando && capturador?.modo != .ventana
        inicio = nil
        arrastrando = false
        if fueArrastre, zona.width >= 2, zona.height >= 2 {
            elegir(zona)
        } else {
            // Un clic: la ventana de debajo o, si no hay, la pantalla.
            elegir(ventanas.first { $0.marco.contains(p) }?.marco ?? bounds)
        }
    }

    override func rightMouseDown(with e: NSEvent) {
        if elegida || inicio != nil {
            reiniciar()
            seguirRaton(a: punto(e))
        } else {
            capturador?.cerrar()
        }
    }

    // MARK: - Teclado (lo decide el capturador, que sabe que pantalla manda)

    override func keyDown(with e: NSEvent) {
        if escribirTexto(e) { return }
        if capturador?.tecla(e) != true { super.keyDown(with: e) }
    }

    /// Mientras se escribe un texto anotado, las teclas son del texto
    /// (Intro y Esc lo cierran). Devuelve si se usaron.
    private func escribirTexto(_ e: NSEvent) -> Bool {
        guard anotacion.escribiendo else { return false }
        let mods = e.modifierFlags.intersection(.deviceIndependentFlagsMask)
        if mods.contains(.command) || mods.contains(.control) { return false }
        switch e.keyCode {
        case 53, 36, 76: // Esc, Intro
            anotacion.cerrarTexto()
        case 51: // borrar
            anotacion.borrarLetra()
        default:
            guard let c = e.characters, !c.isEmpty else { return true }
            anotacion.escribir(c)
        }
        sincronizarDibujo()
        return true
    }

    override func performKeyEquivalent(with e: NSEvent) -> Bool {
        let mods = e.modifierFlags.intersection(.deviceIndependentFlagsMask)
        let conMando = mods.contains(.command) || mods.contains(.control)
        // Sin ⌘ ni ⌃, la tecla es de quien escribe (el texto anotado o el
        // campo de las medidas), no un atajo.
        if !conMando, anotacion.escribiendo || window?.firstResponder !== self { return false }
        if capturador?.tecla(e) == true { return true }
        return super.performKeyEquivalent(with: e)
    }

    override func cancelOperation(_ sender: Any?) {
        if anotacion.escribiendo {
            anotacion.cerrarTexto()
            sincronizarDibujo()
            return
        }
        capturador?.cerrar()
    }

    // MARK: - Barras

    @objc func pulsado(_ boton: NSButton) {
        guard let accion = AccionCaptura(rawValue: boton.tag) else { return }
        capturador?.accion(accion, en: self)
    }

    @objc func pulsadaHerramienta(_ boton: NSButton) {
        guard let h = HerramientaAnotar(rawValue: boton.tag) else { return }
        tomar(h)
    }

    @objc func pulsadoColor(_ boton: NSButton) { ponerColor(boton.tag) }
    @objc func pulsadoGrosor(_ boton: NSButton) { ponerGrosor(boton.tag) }
    @objc func pulsadoDeshacer(_ boton: NSButton) { if !deshacer() { NSSound.beep() } }

    @objc func cambiadoModo(_ control: NSSegmentedControl) {
        let i = control.selectedSegment
        guard i >= 0, i < ModoCaptura.enBarra.count else { return }
        capturador?.cambiarModo(ModoCaptura.enBarra[i].0, desde: self)
    }

    @objc func cambiadaProporcion(_ control: NSSegmentedControl) {
        guard let p = ProporcionCaptura(rawValue: control.selectedSegment) else { return }
        capturador?.cambiarProporcion(p, desde: self)
    }

    @objc func tecleadasMedidas(_ campo: NSTextField) {
        let texto = campo.stringValue
        campo.stringValue = ""
        window?.makeFirstResponder(self)
        if !texto.isEmpty, !aplicarMedidas(texto) { NSSound.beep() }
    }

    @objc func pulsadoRepetir(_ boton: NSButton) {
        if !repetirUltima() { NSSound.beep() }
    }

    private static func fondoDeBarra(_ contenido: NSView) -> NSView {
        let fondo = NSVisualEffectView()
        fondo.material = .hudWindow
        fondo.blendingMode = .withinWindow
        fondo.state = .active
        fondo.wantsLayer = true
        fondo.layer?.cornerRadius = 10
        fondo.layer?.masksToBounds = true
        fondo.frame = NSRect(origin: .zero, size: contenido.fittingSize)
        contenido.frame = fondo.bounds
        contenido.autoresizingMask = [.width, .height]
        fondo.addSubview(contenido)
        return fondo
    }

    private static func boton(simbolo: String, titulo: String, destino: VistaCaptura, accion: Selector, tag: Int) -> NSButton {
        let imagen = NSImage(systemSymbolName: simbolo, accessibilityDescription: titulo) ?? NSImage()
        let b = NSButton(image: imagen, target: destino, action: accion)
        b.tag = tag
        b.toolTip = titulo
        b.refusesFirstResponder = true
        b.bezelStyle = .smallSquare
        return b
    }

    /// La barra flotante: arriba la de anotar (herramientas 1-5, colores,
    /// grosores y deshacer) y debajo un boton por accion (oculta hasta
    /// elegir). Las acciones que no tienen a nadie detras (`disponible`)
    /// salen apagadas.
    static func hacerBarra(destino: VistaCaptura, disponible: (AccionCaptura) -> Bool) -> NSView {
        let botones = AccionCaptura.allCases.map { accion -> NSButton in
            let imagen = NSImage(systemSymbolName: accion.simbolo, accessibilityDescription: accion.titulo) ?? NSImage()
            let boton = NSButton(title: accion.titulo, image: imagen, target: destino,
                                 action: #selector(VistaCaptura.pulsado(_:)))
            boton.imagePosition = .imageLeading
            boton.tag = accion.rawValue
            boton.toolTip = "\(accion.titulo) (\(accion.atajo))"
            boton.refusesFirstResponder = true
            boton.isEnabled = disponible(accion)
            if accion == (destino.capturador?.accionIntro ?? .copiar) { boton.bezelColor = .controlAccentColor }
            return boton
        }
        let acciones = NSStackView(views: botones)
        acciones.orientation = .horizontal
        acciones.spacing = 4

        // Anotar: herramientas, colores, grosores y deshacer.
        let herramientas = HerramientaAnotar.allCases.map { h -> NSButton in
            let b = boton(simbolo: h.simbolo, titulo: "\(h.titulo) (\(h.rawValue))", destino: destino,
                          accion: #selector(VistaCaptura.pulsadaHerramienta(_:)), tag: h.rawValue)
            b.setButtonType(.pushOnPushOff)
            return b
        }
        let colores = AnotacionCaptura.colores.indices.map { i -> NSButton in
            let b = NSButton(title: "", target: destino, action: #selector(VistaCaptura.pulsadoColor(_:)))
            b.tag = i
            b.isBordered = false
            b.wantsLayer = true
            b.layer?.backgroundColor = AnotacionCaptura.cgColor(i)
            b.layer?.cornerRadius = 9
            b.layer?.borderColor = NSColor.white.cgColor
            b.refusesFirstResponder = true
            b.translatesAutoresizingMaskIntoConstraints = false
            b.widthAnchor.constraint(equalToConstant: 18).isActive = true
            b.heightAnchor.constraint(equalToConstant: 18).isActive = true
            b.toolTip = "Color"
            return b
        }
        let nombresGrosor = ["Fino", "Medio", "Grueso"]
        let puntosGrosor: [CGFloat] = [6, 10, 14]
        let grosores = AnotacionCaptura.grosores.indices.map { i -> NSButton in
            let config = NSImage.SymbolConfiguration(pointSize: puntosGrosor[min(i, 2)], weight: .bold)
            let imagen = NSImage(systemSymbolName: "circle.fill", accessibilityDescription: "Grosor")?
                .withSymbolConfiguration(config) ?? NSImage()
            let b = NSButton(image: imagen, target: destino, action: #selector(VistaCaptura.pulsadoGrosor(_:)))
            b.tag = i
            b.setButtonType(.pushOnPushOff)
            b.bezelStyle = .smallSquare
            b.refusesFirstResponder = true
            b.toolTip = nombresGrosor[min(i, 2)]
            return b
        }
        let deshacer = boton(simbolo: "arrow.uturn.backward", titulo: "Deshacer (⌘Z)", destino: destino,
                             accion: #selector(VistaCaptura.pulsadoDeshacer(_:)), tag: 0)
        func separador() -> NSView {
            let v = NSBox()
            v.boxType = .separator
            return v
        }
        var vistasAnotar: [NSView] = herramientas
        vistasAnotar.append(separador())
        vistasAnotar.append(contentsOf: colores)
        vistasAnotar.append(separador())
        vistasAnotar.append(contentsOf: grosores)
        vistasAnotar.append(separador())
        vistasAnotar.append(deshacer)
        let anotar = NSStackView(views: vistasAnotar)
        anotar.orientation = .horizontal
        anotar.spacing = 4
        destino.botonesHerramienta = herramientas
        destino.botonesColor = colores
        destino.botonesGrosor = grosores

        let filas = NSStackView(views: [anotar, acciones])
        filas.orientation = .vertical
        filas.alignment = .trailing
        filas.spacing = 6
        filas.edgeInsets = NSEdgeInsets(top: 6, left: 8, bottom: 6, right: 8)
        let fondo = fondoDeBarra(filas)
        fondo.isHidden = true
        destino.actualizarBarraAnotar()
        return fondo
    }

    /// La barra de abajo: los modos con su letra, «ancho × alto», las
    /// proporciones y repetir la ultima zona.
    static func hacerBarraModos(destino: VistaCaptura) -> NSView {
        let modos = NSSegmentedControl(labels: ModoCaptura.enBarra.map { $0.1 }, trackingMode: .selectOne,
                                       target: destino, action: #selector(VistaCaptura.cambiadoModo(_:)))
        modos.refusesFirstResponder = true
        let campo = NSTextField(string: "")
        campo.placeholderString = "ancho × alto"
        campo.toolTip = "Medidas exactas en píxeles (p. ej. 1280x720) e Intro"
        campo.target = destino
        campo.action = #selector(VistaCaptura.tecleadasMedidas(_:))
        campo.translatesAutoresizingMaskIntoConstraints = false
        campo.widthAnchor.constraint(equalToConstant: 110).isActive = true
        let proporciones = NSSegmentedControl(labels: ProporcionCaptura.allCases.map { $0.rotulo },
                                              trackingMode: .selectOne, target: destino,
                                              action: #selector(VistaCaptura.cambiadaProporcion(_:)))
        proporciones.refusesFirstResponder = true
        let repetir = NSButton(title: "Repetir R", target: destino, action: #selector(VistaCaptura.pulsadoRepetir(_:)))
        repetir.refusesFirstResponder = true
        repetir.toolTip = "Repetir la última zona de esta pantalla (R)"
        let fila = NSStackView(views: [modos, campo, proporciones, repetir])
        fila.orientation = .horizontal
        fila.spacing = 8
        fila.edgeInsets = NSEdgeInsets(top: 6, left: 8, bottom: 6, right: 8)
        destino.selectorModo = modos
        destino.selectorProporcion = proporciones
        destino.campoMedidas = campo
        destino.botonRepetir = repetir
        return fondoDeBarra(fila)
    }
}
