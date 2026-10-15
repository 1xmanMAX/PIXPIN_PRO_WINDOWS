// La superposicion de la captura: un panel sin bordes por pantalla, encima
// de todo, con la foto congelada, oscurecida salvo la zona elegida.
//
// Rendimiento (§4bis del plan):
// - la vista aloja sus capas (`layer` propio): la foto es el `contents` de
//   una capa y la compone la tarjeta grafica; mover el raton solo cambia un
//   camino (el velo), un borde y una etiqueta, nunca se redibuja la foto;
// - la lupa es otra capa con la MISMA foto y un `contentsRect`: cero copias;
// - nada de temporizadores: todo va por los eventos del raton y el teclado.

import AppKit
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
        panel.contentView = contenedor
        panel.initialFirstResponder = vista
        panel.makeFirstResponder(vista)
        return panel
    }
}

final class VistaCaptura: NSView {
    weak var capturador: Capturador?
    weak var barra: NSView?

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

    private let raiz = CALayer()
    private let capaFoto = CALayer()
    private let capaVelo = CAShapeLayer()
    private let capaBorde = CAShapeLayer()
    private let capaEtiqueta = CATextLayer()
    private let capaLupa = CALayer()
    private let fuente = NSFont.monospacedDigitSystemFont(ofSize: 12, weight: .medium)

    /// La lupa: ensena `ladoLupa` puntos de la foto en `tamLupa` puntos.
    private let ladoLupa: CGFloat = 16
    private let tamLupa: CGFloat = 112

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

        for capa in [capaFoto, capaVelo, capaBorde, capaEtiqueta, capaLupa] {
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

        guard !zona.isEmpty, let foto else {
            capaEtiqueta.isHidden = true
            return
        }
        let px = GeometriaCaptura.rectEnPixeles(zona, tamVista: bounds.size, ancho: foto.width, alto: foto.height)
        var texto = "\(Int(px.width)) × \(Int(px.height))"
        if !elegida, !arrastrando, let nombre = resaltada?.nombre, !nombre.isEmpty {
            texto = "\(nombre)  ·  \(texto)"
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
        guard !elegida, foto != nil, bounds.contains(p) else {
            capaLupa.isHidden = true
            return
        }
        var origen = CGPoint(x: p.x + 20, y: p.y - 20 - tamLupa)
        if origen.x + tamLupa > bounds.maxX { origen.x = p.x - 20 - tamLupa }
        if origen.y < bounds.minY { origen.y = p.y + 20 }
        capaLupa.frame = CGRect(origin: origen, size: CGSize(width: tamLupa, height: tamLupa))
        capaLupa.contentsRect = GeometriaCaptura.rectDeLupa(en: p, lado: ladoLupa, tamVista: bounds.size)
        capaLupa.isHidden = false
    }

    private func colocarBarra() {
        guard let barra else { return }
        let o = GeometriaCaptura.posicionBarra(seleccion: seleccion, tamBarra: barra.frame.size, limites: bounds)
        barra.setFrameOrigin(o)
        barra.isHidden = false
    }

    // MARK: - Lo que pide el capturador

    /// Vuelve a elegir desde cero (otra pantalla empezo una zona, o clic
    /// derecho).
    func reiniciar() {
        seleccion = .zero
        elegida = false
        inicio = nil
        arrastrando = false
        barra?.isHidden = true
        redibujar()
    }

    /// Elige `r` como zona y ensena la barra.
    func elegir(_ r: CGRect) {
        let zona = GeometriaCaptura.ajustar(r, a: bounds)
        guard !zona.isEmpty else { return }
        seleccion = zona
        elegida = true
        resaltada = nil
        moverLupa(a: CGPoint(x: -1, y: -1))
        redibujar()
        colocarBarra()
        if capturador?.modo == .texto { capturador?.accion(.texto, en: self) }
    }

    /// Elige la pantalla entera (la tecla `P`).
    func elegirPantalla() { elegir(bounds) }

    /// El trozo elegido de la foto, en pixeles. `cropping(to:)` no copia
    /// ni recodifica: comparte los pixeles de la foto.
    func recorte() -> CGImage? {
        guard elegida, let foto else { return nil }
        let px = GeometriaCaptura.rectEnPixeles(seleccion, tamVista: bounds.size, ancho: foto.width, alto: foto.height)
        guard !px.isEmpty else { return nil }
        return foto.cropping(to: px)
    }

    /// La zona elegida para grabarla en video (`Grabar/`): su pantalla y la
    /// zona en puntos con el origen arriba, como la pide ScreenCaptureKit.
    func zonaDeGrabacion() -> ZonaDeGrabacion? {
        guard elegida, let numero = window?.screen?.numeroDePantalla else { return nil }
        let rect = GeometriaGrabacion.desdeVista(seleccion, altoPantalla: bounds.height)
        return ZonaDeGrabacion(pantalla: numero, rect: rect, escala: escalaFoto)
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
        CATransaction.commit()
        foto = nil
        ventanas = []
        resaltada = nil
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
        NSCursor.crosshair.set()
        seguirRaton(a: punto(e))
    }

    override func mouseDown(with e: NSEvent) {
        capturador?.empezo(en: self)
        if capturador?.modo == .pantalla {
            elegirPantalla()
            return
        }
        barra?.isHidden = true
        elegida = false
        seleccion = .zero
        arrastrando = false
        inicio = punto(e)
    }

    override func mouseDragged(with e: NSEvent) {
        guard let inicio else { return }
        let p = punto(e)
        if !arrastrando, GeometriaCaptura.esArrastre(desde: inicio, hasta: p) { arrastrando = true }
        if arrastrando, capturador?.modo != .ventana {
            seleccion = GeometriaCaptura.ajustar(GeometriaCaptura.rectEntre(inicio, p), a: bounds)
            redibujar()
        }
        moverLupa(a: p)
    }

    override func mouseUp(with e: NSEvent) {
        guard inicio != nil else { return }
        let p = punto(e)
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
        if capturador?.tecla(e) != true { super.keyDown(with: e) }
    }

    override func performKeyEquivalent(with e: NSEvent) -> Bool {
        if capturador?.tecla(e) == true { return true }
        return super.performKeyEquivalent(with: e)
    }

    override func cancelOperation(_ sender: Any?) {
        capturador?.cerrar()
    }

    // MARK: - Barra de acciones

    @objc func pulsado(_ boton: NSButton) {
        guard let accion = AccionCaptura(rawValue: boton.tag) else { return }
        capturador?.accion(accion, en: self)
    }

    /// La barra flotante con un boton por accion (oculta hasta elegir).
    /// Las que no tienen a nadie detras (`disponible`) salen apagadas.
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
        let pila = NSStackView(views: botones)
        pila.orientation = .horizontal
        pila.spacing = 4
        pila.edgeInsets = NSEdgeInsets(top: 6, left: 8, bottom: 6, right: 8)

        let fondo = NSVisualEffectView()
        fondo.material = .hudWindow
        fondo.blendingMode = .withinWindow
        fondo.state = .active
        fondo.wantsLayer = true
        fondo.layer?.cornerRadius = 10
        fondo.layer?.masksToBounds = true
        fondo.frame = NSRect(origin: .zero, size: pila.fittingSize)
        pila.frame = fondo.bounds
        pila.autoresizingMask = [.width, .height]
        fondo.addSubview(pila)
        fondo.isHidden = true
        return fondo
    }
}
