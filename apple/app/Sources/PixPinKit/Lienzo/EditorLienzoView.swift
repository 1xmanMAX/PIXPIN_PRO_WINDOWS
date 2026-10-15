// El lienzo EDITABLE en Mac: una `NSView` volteada (Y hacia abajo, como el
// mundo del lienzo) que manda el raton, el lapiz y el teclado al
// `EditorLienzo` de Rust (`pixpin-nucleo/src/editor.rs`, el `Gesto` y la
// `Escena` de `pixpin-motor2d`, los mismos de Windows) y pinta lo que le
// devuelve con el lector y el pintor de siempre (`LectorLienzo.swift`,
// `PintorLienzo.swift`).
//
// Rendimiento (§4bis del plan):
// - una llamada al puente por aviso del raton (AppKit ya los agrupa), y la
//   respuesta trae la version: si no cambio, no se repinta;
// - pintar pide a Rust SOLO lo que cae en pantalla (`planoDeVista`), una vez
//   por fotograma y solo si cambio la version, la camara o el tamano; Rust
//   guarda la geometria de lo que no cambio;
// - mover y acercar la vista no toca el documento;
// - guardar va solo, 1 s despues del ultimo cambio, en una cola de fondo
//   (`.utility`): nada de escribir JSON en el hilo principal.

import AppKit
import PixPinCore
import SwiftUI

public extension Notification.Name {
    /// Un lienzo del chat se guardo: `object` es la referencia del dibujo.
    /// La burbuja que lo ensena se vuelve a leer.
    static let pixpinLienzoGuardado = Notification.Name("PixPinLienzoGuardado")
}

/// Las herramientas de la barra: nombre del motor, titulo y simbolo.
public struct HerramientaLienzo: Identifiable, Equatable {
    public let id: String
    public let titulo: String
    public let simbolo: String
    /// La tecla que la elige (sin modificadores, fuera de un texto).
    public let tecla: Character

    public static let todas: [HerramientaLienzo] = [
        .init(id: "seleccion", titulo: "Seleccionar", simbolo: "cursorarrow", tecla: "v"),
        .init(id: "lapiz", titulo: "Lápiz", simbolo: "pencil.tip", tecla: "p"),
        .init(id: "resaltador", titulo: "Resaltador", simbolo: "highlighter", tecla: "h"),
        .init(id: "rectangulo", titulo: "Rectángulo", simbolo: "rectangle", tecla: "r"),
        .init(id: "elipse", titulo: "Elipse", simbolo: "circle", tecla: "o"),
        .init(id: "rombo", titulo: "Rombo", simbolo: "diamond", tecla: "d"),
        .init(id: "flecha", titulo: "Flecha", simbolo: "arrow.up.right", tecla: "a"),
        .init(id: "linea", titulo: "Línea", simbolo: "line.diagonal", tecla: "l"),
        .init(id: "texto", titulo: "Texto", simbolo: "textformat", tecla: "t"),
        .init(id: "borrador", titulo: "Goma", simbolo: "eraser", tecla: "e"),
    ]
}

/// El estado del editor que ve SwiftUI (la barra) y la vista (el lienzo).
@MainActor
public final class ModeloEditorLienzo: ObservableObject {
    public let editor: EditorLienzo
    /// La referencia del dibujo en el chat (para avisar al guardar), si viene
    /// de un chat.
    public let referencia: String?
    /// La foto de cada `id_objeto`, con su fichero.
    public let fotos: [UInt64: URL]

    @Published public private(set) var herramienta: String
    @Published public private(set) var puedeDeshacer = false
    @Published public private(set) var error: String?

    /// La vista que pinta (la pone ella al nacer).
    weak var vista: VistaEditorLienzoNS?

    /// Guardar, uno detras de otro y fuera del hilo principal.
    private let colaGuardar = DispatchQueue(label: "pixpin.lienzo.guardar", qos: .utility)
    private var guardadoPendiente: DispatchWorkItem?
    private var cambiosVistos: UInt64

    public init(editor: EditorLienzo, referencia: String? = nil, fotos: [UInt64: URL] = [:]) {
        self.editor = editor
        self.referencia = referencia
        self.fotos = fotos
        herramienta = editor.herramientaActual()
        cambiosVistos = editor.cambios()
        puedeDeshacer = editor.puedeDeshacer()
    }

    public func elegir(_ nombre: String) {
        do {
            try editor.herramienta(nombre: nombre)
            herramienta = nombre
            vista?.trasCambio()
        } catch {
            self.error = "\(error)"
        }
    }

    public func deshacer() {
        _ = editor.deshacer()
        vista?.trasCambio()
    }

    public func rehacer() {
        _ = editor.rehacer()
        vista?.trasCambio()
    }

    public func borrarSeleccion() {
        if editor.borrarSeleccion() { vista?.trasCambio() }
    }

    /// Lo llama la vista tras cada cosa que puede cambiar el documento: si
    /// cambio, se programa el guardado (1 s despues del ULTIMO cambio).
    func revisarCambios() {
        let cambios = editor.cambios()
        guard cambios != cambiosVistos else { return }
        cambiosVistos = cambios
        let puede = editor.puedeDeshacer()
        if puede != puedeDeshacer { puedeDeshacer = puede }
        programarGuardado(dentroDe: 1.0)
    }

    private func programarGuardado(dentroDe segundos: Double) {
        guardadoPendiente?.cancel()
        let tarea = DispatchWorkItem { [editor, referencia] in
            ModeloEditorLienzo.guardarYa(editor, referencia: referencia)
        }
        guardadoPendiente = tarea
        colaGuardar.asyncAfter(deadline: .now() + segundos, execute: tarea)
    }

    /// Guarda ahora (al cerrar la ventana): termina el texto que se estaba
    /// escribiendo y escribe en la cola de fondo. El editor vive lo que dure
    /// la tarea aunque la ventana ya no este.
    public func guardarAlCerrar() {
        guardadoPendiente?.cancel()
        guardadoPendiente = nil
        _ = editor.terminarTexto()
        let editor = self.editor
        let referencia = self.referencia
        colaGuardar.async { ModeloEditorLienzo.guardarYa(editor, referencia: referencia) }
    }

    /// En la cola de fondo: escribe y, si escribio, avisa en el principal.
    private nonisolated static func guardarYa(_ editor: EditorLienzo, referencia: String?) {
        let escrito: Bool
        do {
            escrito = try editor.guardar()
        } catch {
            let texto = "\(error)"
            DispatchQueue.main.async {
                NSLog("PixPin: no se pudo guardar el lienzo: %@", texto)
            }
            return
        }
        guard escrito else { return }
        DispatchQueue.main.async {
            NotificationCenter.default.post(name: .pixpinLienzoGuardado, object: referencia)
            // La sincronizacion sola cuenta sus 30 s de calma desde aqui.
            NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
        }
    }
}

/// La vista que edita. Camara: `origen` es el punto del mundo en la esquina
/// de arriba a la izquierda y `zoom` los puntos de pantalla por unidad.
public final class VistaEditorLienzoNS: NSView, NSTextInputClient {
    let modelo: ModeloEditorLienzo
    private var editor: EditorLienzo { modelo.editor }

    public private(set) var origen = CGPoint.zero
    public private(set) var zoom: CGFloat = 1
    static let zoomMinimo: CGFloat = 0.05
    static let zoomMaximo: CGFloat = 30

    /// Lo ultimo pedido a Rust y con que se pidio: si nada cambio, se pinta
    /// esto sin cruzar el puente.
    private var leido: LienzoLeido?
    private var clave: (version: UInt64, x: CGFloat, y: CGFloat, zoom: CGFloat, w: CGFloat, h: CGFloat)?
    private var versionVista: UInt64 = 0
    private var encajada = false
    /// La presion del Force Touch del ultimo `pressureChange` (0...1).
    private var presionTrackpad: Float?
    /// Lo que el metodo de entrada esta componiendo (una tilde muerta): no
    /// se escribe hasta que llega el `insertText`.
    private var marcado = ""

    public init(modelo: ModeloEditorLienzo) {
        self.modelo = modelo
        super.init(frame: NSRect(x: 0, y: 0, width: 800, height: 600))
        modelo.vista = self
        wantsLayer = true
        layerContentsRedrawPolicy = .onSetNeedsDisplay
        // Las fotos del lienzo: se cargan reducidas y se repinta una vez.
        let fotos = modelo.fotos
        if !fotos.isEmpty {
            Task { [weak self] in
                var alguna = false
                for url in Set(fotos.values) {
                    if await Miniaturas.compartidas.cargar(url, lado: 2048) != nil { alguna = true }
                }
                if alguna { self?.clave = nil; self?.needsDisplay = true }
            }
        }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) no se usa") }

    override public var isFlipped: Bool { true }
    override public var acceptsFirstResponder: Bool { true }
    override public var isOpaque: Bool { true }
    override public func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    override public func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        window?.makeFirstResponder(self)
    }

    override public func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        if !encajada, newSize.width > 10, newSize.height > 10 {
            encajada = true
            encajar()
        }
    }

    /// Lo dibujado entero en la vista (o el origen al 100 % si esta vacio).
    public func encajar() {
        guard let caja = editor.caja() else {
            origen = .zero
            zoom = 1
            needsDisplay = true
            return
        }
        let margen: CGFloat = 40
        let ancho = CGFloat(caja.x1 - caja.x0), alto = CGFloat(caja.y1 - caja.y0)
        let z = min((bounds.width - margen * 2) / max(ancho, 1),
                    (bounds.height - margen * 2) / max(alto, 1), 1)
        zoom = max(VistaEditorLienzoNS.zoomMinimo, z)
        origen = CGPoint(x: CGFloat(caja.x0) - (bounds.width / zoom - ancho) / 2,
                         y: CGFloat(caja.y0) - (bounds.height / zoom - alto) / 2)
        needsDisplay = true
    }

    // MARK: Pintar

    override public func draw(_ dirtyRect: NSRect) {
        guard let ctx = NSGraphicsContext.current?.cgContext else { return }
        let version = editor.version()
        let w = bounds.width, h = bounds.height
        if let c = clave, c.version == version, c.x == origen.x, c.y == origen.y,
           c.zoom == zoom, c.w == w, c.h == h {
            // Nada cambio: lo de antes.
        } else {
            let plano = editor.planoDeVista(x: Float(origen.x), y: Float(origen.y), zoom: Float(zoom),
                                            anchoPx: Float(w), altoPx: Float(h))
            leido = try? leerLienzo(plano)
            clave = (version, origen.x, origen.y, zoom, w, h)
        }
        versionVista = version
        let fondo = leido?.fondo.cg ?? CGColor(gray: 1, alpha: 1)
        ctx.setFillColor(fondo)
        ctx.fill(dirtyRect)
        guard let leido else { return }
        ctx.saveGState()
        ctx.scaleBy(x: zoom, y: zoom)
        ctx.translateBy(x: -origen.x, y: -origen.y)
        let visible = CGRect(x: origen.x, y: origen.y, width: w / zoom, height: h / zoom)
        let fotos = modelo.fotos
        PintorLienzo(lienzo: leido) { id in
            fotos[id].flatMap { Miniaturas.compartidas.enCache($0, lado: 2048) }
        }.pintar(en: ctx, limites: visible)
        ctx.restoreGState()
    }

    /// Tras algo que pudo cambiar el dibujo: repinta si cambio la version y
    /// avisa al modelo (guardar, la barra).
    func trasCambio(version: UInt64? = nil) {
        let v = version ?? editor.version()
        if v != versionVista { needsDisplay = true }
        window?.invalidateCursorRects(for: self)
        modelo.revisarCambios()
    }

    // MARK: Raton y lapiz

    private func alMundo(_ evento: NSEvent) -> CGPoint {
        let p = convert(evento.locationInWindow, from: nil)
        return CGPoint(x: origen.x + p.x / zoom, y: origen.y + p.y / zoom)
    }

    private func modificadores(_ evento: NSEvent) -> UInt32 {
        var m: UInt32 = 0
        if evento.modifierFlags.contains(.shift) { m |= 1 }
        if evento.modifierFlags.contains(.option) { m |= 2 }
        return m
    }

    /// La presion: la de la tableta si es una muestra de lapiz, la del Force
    /// Touch si el trackpad la esta dando, y si no ninguna (raton).
    private func presion(_ evento: NSEvent) -> Float? {
        if evento.subtype == .tabletPoint { return Float(evento.pressure) }
        return presionTrackpad
    }

    private func mandar(_ fase: FasePuntero, _ evento: NSEvent) {
        let p = alMundo(evento)
        let v = editor.puntero(fase: fase, x: Float(p.x), y: Float(p.y), presion: presion(evento),
                               modificadores: modificadores(evento), zoom: Float(zoom))
        if fase == .mover {
            if v != versionVista { needsDisplay = true }
        } else {
            trasCambio(version: v)
        }
    }

    override public func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
        presionTrackpad = nil
        mandar(.pulsar, event)
    }

    override public func mouseDragged(with event: NSEvent) { mandar(.mover, event) }

    override public func mouseUp(with event: NSEvent) {
        mandar(.soltar, event)
        presionTrackpad = nil
    }

    override public func pressureChange(with event: NSEvent) {
        // Solo mientras se aprieta de verdad (fase 1 en adelante).
        presionTrackpad = event.stage >= 1 ? Float(max(0, min(1, event.pressure))) : nil
    }

    override public func tabletPoint(with event: NSEvent) {
        // Las muestras de la tableta sin boton pulsado no dibujan.
    }

    // Arrastrar con el boton del medio: mover la vista.
    override public func otherMouseDragged(with event: NSEvent) {
        desplazar(dx: event.deltaX, dy: event.deltaY)
    }

    // MARK: Mover y acercar la vista

    private func desplazar(dx: CGFloat, dy: CGFloat) {
        origen.x -= dx / zoom
        origen.y -= dy / zoom
        needsDisplay = true
    }

    /// Acerca `factor` veces dejando quieto el punto de la vista `foco`.
    private func acercar(_ factor: CGFloat, en foco: CGPoint) {
        let nuevo = max(VistaEditorLienzoNS.zoomMinimo, min(VistaEditorLienzoNS.zoomMaximo, zoom * factor))
        guard nuevo != zoom else { return }
        let mundo = CGPoint(x: origen.x + foco.x / zoom, y: origen.y + foco.y / zoom)
        zoom = nuevo
        origen = CGPoint(x: mundo.x - foco.x / zoom, y: mundo.y - foco.y / zoom)
        needsDisplay = true
    }

    override public func scrollWheel(with event: NSEvent) {
        let foco = convert(event.locationInWindow, from: nil)
        if event.modifierFlags.contains(.command) || event.modifierFlags.contains(.control) {
            // Cmd o Ctrl + rueda: acercar, como en Windows.
            let paso = event.hasPreciseScrollingDeltas ? event.scrollingDeltaY / 200 : event.scrollingDeltaY / 20
            acercar(CGFloat(exp(Double(paso))), en: foco)
        } else if event.hasPreciseScrollingDeltas {
            // Dos dedos en el trackpad: mover la vista.
            desplazar(dx: event.scrollingDeltaX, dy: event.scrollingDeltaY)
        } else {
            // La rueda del raton: lineas, no puntos.
            desplazar(dx: event.scrollingDeltaX * 12, dy: event.scrollingDeltaY * 12)
        }
    }

    override public func magnify(with event: NSEvent) {
        acercar(1 + event.magnification, en: convert(event.locationInWindow, from: nil))
    }

    override public func smartMagnify(with event: NSEvent) {
        // Doble toque con dos dedos: encajar el dibujo o volver al 100 %.
        if abs(zoom - 1) < 0.01 {
            encajar()
        } else {
            acercar(1 / zoom, en: convert(event.locationInWindow, from: nil))
        }
    }

    // MARK: Teclado

    override public func keyDown(with event: NSEvent) {
        let teclas = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
            .subtracting([.capsLock, .numericPad, .function])
        if editor.escribiendo() {
            // El texto: el metodo de entrada compone (tildes, emoji) y llega
            // por `insertText`/`doCommand`.
            interpretKeyEvents([event])
            return
        }
        switch event.keyCode {
        case 51, 117: // borrar, suprimir
            modelo.borrarSeleccion()
            return
        case 53: // Esc: suelta lo elegido / lo que iba a medias
            let v = editor.puntero(fase: .cancelar, x: 0, y: 0, presion: nil, modificadores: 0,
                                   zoom: Float(zoom))
            trasCambio(version: v)
            return
        default:
            break
        }
        if teclas.isEmpty, let letra = event.charactersIgnoringModifiers?.lowercased().first,
           let h = HerramientaLienzo.todas.first(where: { $0.tecla == letra }) {
            modelo.elegir(h.id)
            return
        }
        if teclas == .command, event.charactersIgnoringModifiers == "a" {
            editor.seleccionarTodo()
            trasCambio()
            return
        }
        super.keyDown(with: event)
    }

    // NSTextInputClient: solo lo que hace falta para escribir un texto del
    // lienzo con cualquier teclado. No hay texto que seleccionar aqui.

    public func insertText(_ string: Any, replacementRange: NSRange) {
        marcado = ""
        let texto: String
        if let a = string as? NSAttributedString { texto = a.string } else { texto = (string as? String) ?? "" }
        guard !texto.isEmpty else { return }
        if editor.teclear(texto: texto) { trasCambio() }
    }

    override public func doCommand(by selector: Selector) {
        let nombre: String?
        switch selector {
        case #selector(NSResponder.deleteBackward(_:)): nombre = "retroceso"
        case #selector(NSResponder.deleteForward(_:)): nombre = "suprimir"
        case #selector(NSResponder.moveLeft(_:)): nombre = "izquierda"
        case #selector(NSResponder.moveRight(_:)): nombre = "derecha"
        case #selector(NSResponder.moveToBeginningOfLine(_:)): nombre = "inicio"
        case #selector(NSResponder.moveToEndOfLine(_:)): nombre = "fin"
        case #selector(NSResponder.insertNewline(_:)): nombre = "entrar"
        case #selector(NSResponder.cancelOperation(_:)): nombre = "escape"
        default: nombre = nil
        }
        if let nombre, editor.tecla(nombre: nombre) { trasCambio() }
    }

    public func setMarkedText(_ string: Any, selectedRange: NSRange, replacementRange: NSRange) {
        if let a = string as? NSAttributedString { marcado = a.string } else { marcado = (string as? String) ?? "" }
    }

    public func unmarkText() { marcado = "" }

    public func selectedRange() -> NSRange { NSRange(location: NSNotFound, length: 0) }

    public func markedRange() -> NSRange {
        marcado.isEmpty ? NSRange(location: NSNotFound, length: 0)
            : NSRange(location: 0, length: (marcado as NSString).length)
    }

    public func hasMarkedText() -> Bool { !marcado.isEmpty }

    public func attributedSubstring(forProposedRange range: NSRange,
                                    actualRange: NSRangePointer?) -> NSAttributedString? { nil }

    public func validAttributesForMarkedText() -> [NSAttributedString.Key] { [] }

    public func firstRect(forCharacterRange range: NSRange, actualRange: NSRangePointer?) -> NSRect {
        // Donde se escribe: el centro de la vista basta para colocar la
        // ventanita del metodo de entrada.
        let enVentana = convert(NSRect(x: bounds.midX, y: bounds.midY, width: 1, height: 20), to: nil)
        return window?.convertToScreen(enVentana) ?? .zero
    }

    public func characterIndex(for point: NSPoint) -> Int { NSNotFound }

    // MARK: Cursor

    override public func resetCursorRects() {
        addCursorRect(bounds, cursor: modelo.herramienta == "seleccion" ? .arrow : .crosshair)
    }
}

/// La vista de AppKit dentro de SwiftUI.
struct LienzoEditable: NSViewRepresentable {
    let modelo: ModeloEditorLienzo

    func makeNSView(context: Context) -> VistaEditorLienzoNS {
        VistaEditorLienzoNS(modelo: modelo)
    }

    func updateNSView(_ vista: VistaEditorLienzoNS, context: Context) {
        vista.window?.invalidateCursorRects(for: vista)
    }
}

/// **El editor del lienzo**: la barra de herramientas arriba y el lienzo.
public struct EditorLienzoView: View {
    @ObservedObject var modelo: ModeloEditorLienzo

    public init(modelo: ModeloEditorLienzo) { self.modelo = modelo }

    public var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 2) {
                ForEach(HerramientaLienzo.todas) { h in
                    Button {
                        modelo.elegir(h.id)
                    } label: {
                        Image(systemName: h.simbolo)
                            .frame(width: 28, height: 24)
                            .background(modelo.herramienta == h.id ? Color.accentColor.opacity(0.25) : Color.clear,
                                        in: RoundedRectangle(cornerRadius: 5))
                    }
                    .buttonStyle(.plain)
                    .help("\(h.titulo) (\(String(h.tecla).uppercased()))")
                }
                Divider().frame(height: 20).padding(.horizontal, 6)
                Button(action: modelo.deshacer) { Image(systemName: "arrow.uturn.backward") }
                    .keyboardShortcut("z", modifiers: .command)
                    .help("Deshacer (⌘Z)")
                Button(action: modelo.rehacer) { Image(systemName: "arrow.uturn.forward") }
                    .keyboardShortcut("z", modifiers: [.command, .shift])
                    .help("Rehacer (⇧⌘Z)")
                Spacer()
                if let error = modelo.error {
                    Text(error).font(.caption).foregroundStyle(.red).lineLimit(1)
                }
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 5)
            .background(.bar)
            Divider()
            LienzoEditable(modelo: modelo)
        }
    }
}

/// Abre los lienzos en ventanas propias, una por dibujo: abrir otra vez el
/// mismo la trae al frente.
@MainActor
public final class VentanasDeLienzo: NSObject, NSWindowDelegate {
    public static let compartidas = VentanasDeLienzo()

    private struct Abierta {
        let ventana: NSWindow
        let modelo: ModeloEditorLienzo
    }

    private var abiertas: [String: Abierta] = [:]

    /// Abre el dibujo `referencia` del chat. El fichero se lee fuera del
    /// hilo principal.
    public func abrir(referencia: String, en chat: String, titulo: String, almacen: Almacen) {
        let clave = chat + "\u{0}" + referencia
        if let abierta = abiertas[clave] {
            abierta.ventana.makeKeyAndOrderFront(nil)
            NSApp.activate()
            return
        }
        let nucleo = almacen.nucleo
        Task {
            let hecho = await Task.detached(priority: .userInitiated) { () -> Result<(EditorLienzo, [UInt64: URL]), Error> in
                do {
                    let editor = try nucleo.editarLienzo(chat: chat, dibujo: referencia)
                    var fotos: [UInt64: URL] = [:]
                    for f in editor.imagenes() { fotos[f.idObjeto] = URL(fileURLWithPath: f.ruta) }
                    return .success((editor, fotos))
                } catch {
                    return .failure(error)
                }
            }.value
            switch hecho {
            case let .success((editor, fotos)):
                self.mostrar(ModeloEditorLienzo(editor: editor, referencia: referencia, fotos: fotos),
                             clave: clave, titulo: titulo)
            case .failure:
                NSSound.beep()
            }
        }
    }

    private func mostrar(_ modelo: ModeloEditorLienzo, clave: String, titulo: String) {
        if let abierta = abiertas[clave] {
            abierta.ventana.makeKeyAndOrderFront(nil)
            return
        }
        let ventana = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1000, height: 720),
                               styleMask: [.titled, .closable, .miniaturizable, .resizable],
                               backing: .buffered, defer: false)
        ventana.title = titulo
        ventana.isReleasedWhenClosed = false
        ventana.contentViewController = NSHostingController(rootView: EditorLienzoView(modelo: modelo))
        ventana.setContentSize(NSSize(width: 1000, height: 720))
        ventana.center()
        _ = ventana.setFrameAutosaveName("PixPinLienzo")
        ventana.delegate = self
        ventana.identifier = NSUserInterfaceItemIdentifier(clave)
        abiertas[clave] = Abierta(ventana: ventana, modelo: modelo)
        ventana.makeKeyAndOrderFront(nil)
        NSApp.activate()
    }

    public func windowWillClose(_ notification: Notification) {
        guard let ventana = notification.object as? NSWindow,
              let clave = ventana.identifier?.rawValue,
              let abierta = abiertas[clave] else { return }
        // El editor lo retiene la tarea de guardar hasta que termine; la
        // vista y el modelo se sueltan con la ventana.
        abierta.modelo.guardarAlCerrar()
        abiertas[clave] = nil
    }
}
