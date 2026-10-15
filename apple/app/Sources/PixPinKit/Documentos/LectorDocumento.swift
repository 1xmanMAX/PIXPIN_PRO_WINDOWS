// **El lector de documentos como un pin**: un PDF (o un Word o un PowerPoint
// paginados como Office) con tinta, resaltador, goma, marcadores y espacios
// para anotar a cada lado. Es `lector_pdf.rs` de Windows con las piezas de
// Apple:
//
// - Las hojas las pinta **PDFKit** (`PDFView`): desplazar, acercar, buscar y
//   elegir texto son suyos. Un Word o un PowerPoint se maquetan en Rust
//   (`DocumentoOffice`) y cada hoja se pinta una vez en un PDF de una pagina
//   (`PintorOffice`), que se va anadiendo mientras se maqueta el resto.
// - La tinta de cada hoja es un `EditorLienzo` de Rust abierto con
//   `Nucleo.anotarHoja`: el mismo `Gesto` y la misma escena que el lienzo, en
//   las unidades de la hoja (1400 de ancho), y se guarda donde la guardan
//   Windows y Android (`anot-<uid>-p<n>` y su marco) para que viaje.
// - **La tinta no tiembla**: la capa es una vista DENTRO de la del documento
//   de PDFKit, asi que se desplaza con las hojas en el mismo fotograma.
//
// Rendimiento (§4bis del plan):
// - solo se abren las tintas de las hojas que se ven (las que tienen fichero,
//   `hojasConTinta`, o la que se empieza a anotar), con tope; lo que se pinta
//   de cada una se pide a Rust una vez por cambio (`plano`), no por fotograma;
// - guardar va solo, 1 s despues del ultimo trazo, en una cola `.utility`;
// - leyendo (sin herramienta) la capa no recibe el raton: PDFKit elige texto.

import AppKit
import PDFKit
import PixPinCore
import SwiftUI

public extension Notification.Name {
    /// Se guardo lo anotado de un documento: `object` es su ruta.
    static let pixpinDocumentoAnotado = Notification.Name("PixPinDocumentoAnotado")
}

/// Los colores de la tinta del lector (la paleta de Excalidraw, como el
/// movil y Windows).
public struct ColorTinta: Identifiable, Equatable, Sendable {
    public let id: String
    public let r: Float, g: Float, b: Float

    public static let todos: [ColorTinta] = [
        .init(id: "negro", r: 0.118, g: 0.118, b: 0.118),
        .init(id: "rojo", r: 0.878, g: 0.192, b: 0.192),
        .init(id: "azul", r: 0.098, g: 0.443, b: 0.761),
        .init(id: "verde", r: 0.184, g: 0.620, b: 0.267),
        .init(id: "amarillo", r: 1.0, g: 0.831, b: 0.231),
    ]

    var color: Color { Color(red: Double(r), green: Double(g), blue: Double(b)) }
}

/// Lo que ve la barra del lector.
@MainActor
public final class ModeloLector: ObservableObject {
    /// `leer`, `lapiz`, `resaltador` o `borrador`.
    @Published public var herramienta = "leer"
    @Published public var color = ColorTinta.todos[0]
    @Published public var espacios: UInt8 = 0
    @Published public var marcas: [MarcaDocumento] = []
    @Published public var pagina = 0
    @Published public var paginas = 0
    @Published public var encima = false
    @Published public var maquetando = false
    @Published public var donde: DondeAnota = .juntoAlDocumento
    @Published public var aviso: String?

    weak var controlador: ControladorLector?

    public var anotando: Bool { herramienta != "leer" }
}

/// La barra de arriba del lector.
struct BarraLector: View {
    @ObservedObject var modelo: ModeloLector

    var body: some View {
        HStack(spacing: 10) {
            Picker("", selection: Binding(get: { modelo.herramienta },
                                          set: { modelo.controlador?.elegir($0) })) {
                Image(systemName: "book").help("Leer (elegir texto)").tag("leer")
                Image(systemName: "pencil.tip").help("Lápiz (P)").tag("lapiz")
                Image(systemName: "highlighter").help("Resaltador (H)").tag("resaltador")
                Image(systemName: "eraser").help("Goma (E)").tag("borrador")
            }
            .pickerStyle(.segmented)
            .frame(width: 180)
            HStack(spacing: 4) {
                ForEach(ColorTinta.todos) { c in
                    Circle().fill(c.color).frame(width: 16, height: 16)
                        .overlay(Circle().stroke(Color.primary, lineWidth: modelo.color == c ? 2 : 0))
                        .onTapGesture { modelo.controlador?.ponerColor(c) }
                        .help(c.id)
                }
            }
            Divider().frame(height: 18)
            Button { modelo.controlador?.alternarEspacio(1) } label: {
                Image(systemName: modelo.espacios & 1 != 0 ? "rectangle.lefthalf.inset.filled" : "rectangle.lefthalf.inset.filled.arrow.left")
            }
            .help("Espacio para anotar a la izquierda (⌘[)")
            Button { modelo.controlador?.alternarEspacio(2) } label: {
                Image(systemName: modelo.espacios & 2 != 0 ? "rectangle.righthalf.inset.filled" : "rectangle.righthalf.inset.filled.arrow.right")
            }
            .help("Espacio para anotar a la derecha (⌘])")
            Menu {
                Section("Poner aquí") {
                    ForEach(emojisDeMarcas(), id: \.self) { e in
                        Button(e) { modelo.controlador?.ponerMarca(emoji: e) }
                    }
                }
                if !modelo.marcas.isEmpty {
                    Section("Ir a") {
                        ForEach(modelo.marcas, id: \.id) { m in
                            Button("\(m.emoji)  pág. \(Int(m.y) + 1)") { modelo.controlador?.ir(a: m) }
                        }
                    }
                    Button("Quitar todos", role: .destructive) { modelo.controlador?.quitarMarcas() }
                }
            } label: {
                Image(systemName: "bookmark")
            }
            .menuStyle(.button)
            .buttonStyle(.borderless)
            .frame(width: 34)
            .help("Marcadores (M)")
            Button { modelo.controlador?.deshacer() } label: { Image(systemName: "arrow.uturn.backward") }
                .help("Deshacer (⌘Z)")
            Spacer()
            if modelo.maquetando { ProgressView().controlSize(.small) }
            Text(modelo.paginas > 0 ? "\(modelo.pagina + 1) / \(modelo.paginas)" : "")
                .font(.caption.monospacedDigit()).foregroundStyle(.secondary)
            if modelo.donde == .chat {
                Image(systemName: "arrow.triangle.2.circlepath").foregroundStyle(.secondary)
                    .help("Lo anotado viaja con el mensaje a tus otros aparatos")
            }
            Toggle(isOn: Binding(get: { modelo.encima }, set: { modelo.controlador?.ponerEncima($0) })) {
                Image(systemName: "pin")
            }
            .toggleStyle(.button)
            .help("Siempre encima, como un pin")
            Button { modelo.controlador?.pantallaCompleta() } label: {
                Image(systemName: "arrow.up.left.and.arrow.down.right")
            }
            .help("Pantalla completa (⌃⌘F)")
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .overlay(alignment: .bottom) {
            if let aviso = modelo.aviso {
                Text(aviso).font(.caption).padding(4)
                    .background(.red.opacity(0.15), in: RoundedRectangle(cornerRadius: 4))
                    .offset(y: 26)
            }
        }
    }
}

/// La ventana del lector: recoge las teclas del lector antes que nadie.
final class VentanaLectorNS: NSWindow {
    weak var controlador: ControladorLector?

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        if let c = controlador, c.tecla(event) { return true }
        return super.performKeyEquivalent(with: event)
    }

    override func keyDown(with event: NSEvent) {
        if let c = controlador, c.tecla(event) { return }
        super.keyDown(with: event)
    }

    override func cancelOperation(_ sender: Any?) {
        if styleMask.contains(.fullScreen) {
            toggleFullScreen(nil)
        } else if let c = controlador, c.modelo.anotando {
            c.elegir("leer")
        } else {
            super.cancelOperation(sender)
        }
    }
}

/// **La capa de la tinta**, dentro de la vista del documento de PDFKit.
final class CapaTintaNS: NSView {
    weak var controlador: ControladorLector?

    override var isFlipped: Bool { true }
    override var isOpaque: Bool { false }

    override func hitTest(_ point: NSPoint) -> NSView? {
        // Leyendo, el raton es de PDFKit (elegir texto, enlaces).
        guard let c = controlador, c.modelo.anotando else { return nil }
        return super.hitTest(point)
    }

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    override func draw(_ dirtyRect: NSRect) {
        guard let ctx = NSGraphicsContext.current?.cgContext else { return }
        controlador?.pintarCapa(en: ctx, sucio: dirtyRect, capa: self)
    }

    override func mouseDown(with event: NSEvent) { controlador?.puntero(.pulsar, event, capa: self) }
    override func mouseDragged(with event: NSEvent) { controlador?.puntero(.mover, event, capa: self) }
    override func mouseUp(with event: NSEvent) { controlador?.puntero(.soltar, event, capa: self) }

    override func rightMouseDown(with event: NSEvent) {
        if controlador?.quitarMarca(en: convert(event.locationInWindow, from: nil), capa: self) != true {
            super.rightMouseDown(with: event)
        }
    }

    override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        needsDisplay = true
    }
}

/// **Un documento abierto en el lector.**
@MainActor
public final class ControladorLector: NSObject, NSWindowDelegate {
    let url: URL
    let nucleo: Nucleo
    let modelo = ModeloLector()
    let ventana: VentanaLectorNS
    let vistaPDF = PDFView()
    private let capa = CapaTintaNS()

    /// La tinta de cada hoja abierta, y lo ultimo leido de ella.
    private var editores: [Int: EditorLienzo] = [:]
    private var leidos: [Int: (version: UInt64, lienzo: LienzoLeido)] = [:]
    private var cargando: Set<Int> = []
    /// Las hojas con fichero de tinta (las demas no se abren hasta anotarlas).
    private var conTinta: Set<Int> = []
    /// Por orden de uso, para soltar las mas viejas.
    private var usadas: [Int] = []
    private static let topeDeHojas = 16

    /// La hoja del trazo en curso y la ultima anotada (deshacer).
    private var hojaDelTrazo: Int?
    private var ultimaAnotada: Int?
    private var presionTrackpad: Float?

    private let colaGuardar = DispatchQueue(label: "pixpin.lector.guardar", qos: .utility)
    private var guardadoPendiente: DispatchWorkItem?
    private var tareaOffice: Task<Void, Never>?
    private var vigias: [NSObjectProtocol] = []
    var alCerrar: (@MainActor (ControladorLector) -> Void)?

    init(url: URL, nucleo: Nucleo, titulo: String) {
        self.url = url
        self.nucleo = nucleo
        ventana = VentanaLectorNS(contentRect: NSRect(x: 0, y: 0, width: 900, height: 1000),
                                  styleMask: [.titled, .closable, .miniaturizable, .resizable],
                                  backing: .buffered, defer: false)
        super.init()
        modelo.controlador = self
        ventana.controlador = self
        ventana.title = titulo
        ventana.isReleasedWhenClosed = false
        ventana.collectionBehavior.insert(.fullScreenPrimary)
        ventana.delegate = self
        montar()
    }

    private func montar() {
        let barra = NSHostingView(rootView: BarraLector(modelo: modelo))
        barra.translatesAutoresizingMaskIntoConstraints = false
        vistaPDF.translatesAutoresizingMaskIntoConstraints = false
        vistaPDF.displayMode = .singlePageContinuous
        vistaPDF.displaysPageBreaks = true
        vistaPDF.autoScales = true
        vistaPDF.backgroundColor = NSColor.windowBackgroundColor
        let raiz = NSView()
        raiz.addSubview(barra)
        raiz.addSubview(vistaPDF)
        NSLayoutConstraint.activate([
            barra.topAnchor.constraint(equalTo: raiz.topAnchor),
            barra.leadingAnchor.constraint(equalTo: raiz.leadingAnchor),
            barra.trailingAnchor.constraint(equalTo: raiz.trailingAnchor),
            vistaPDF.topAnchor.constraint(equalTo: barra.bottomAnchor),
            vistaPDF.leadingAnchor.constraint(equalTo: raiz.leadingAnchor),
            vistaPDF.trailingAnchor.constraint(equalTo: raiz.trailingAnchor),
            vistaPDF.bottomAnchor.constraint(equalTo: raiz.bottomAnchor),
        ])
        ventana.contentView = raiz
        ventana.setContentSize(NSSize(width: 900, height: 1000))
        ventana.center()
        _ = ventana.setFrameAutosaveName("PixPinLector")
        capa.controlador = self
        let centro = NotificationCenter.default
        for nombre in [Notification.Name.PDFViewScaleChanged, .PDFViewPageChanged, .PDFViewDocumentChanged] {
            vigias.append(centro.addObserver(forName: nombre, object: vistaPDF, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.alMoverse() }
            })
        }
    }

    // MARK: Abrir

    /// Lee lo recordado y pone el documento. Un Word o un PowerPoint se
    /// maquetan en segundo plano, hoja a hoja.
    func abrir() {
        ventana.makeKeyAndOrderFront(nil)
        NSApp.activate()
        let ruta = url.path
        let nucleo = self.nucleo
        Task {
            // Lo recordado: unos ficheros pequenos, pero mira tambien si es
            // el PDF de algun proyecto. Fuera del hilo principal.
            let lectura = await Task.detached(priority: .userInitiated) {
                nucleo.lecturaDeDocumento(documento: ruta)
            }.value
            self.modelo.espacios = lectura.espacios
            self.modelo.marcas = lectura.marcas
            self.modelo.donde = lectura.donde
            switch TiposDeDocumento.clase(de: self.url) {
            case .officePaginado:
                self.abrirOffice(lectura)
            default:
                guard let doc = PDFDocument(url: self.url) else {
                    self.modelo.aviso = "No se puede abrir este PDF."
                    return
                }
                self.poner(doc, lectura: lectura)
            }
        }
    }

    private func poner(_ doc: PDFDocument, lectura: LecturaDocumento) {
        vistaPDF.document = doc
        modelo.paginas = doc.pageCount
        engancharCapa()
        ponerMargenes()
        let ruta = url.path
        let n = UInt32(doc.pageCount)
        let nucleo = self.nucleo
        Task {
            let hojas = await Task.detached(priority: .userInitiated) {
                nucleo.hojasConTinta(documento: ruta, paginas: n)
            }.value
            self.conTinta = Set(hojas.map { Int($0) })
            self.cargarVisibles()
        }
        // Por donde se iba y a que aumento.
        if lectura.zoom.isFinite, abs(lectura.zoom - 1) > 0.01 {
            let ajuste = vistaPDF.scaleFactorForSizeToFit
            if ajuste > 0 {
                vistaPDF.autoScales = false
                vistaPDF.scaleFactor = ajuste * CGFloat(lectura.zoom)
            }
        }
        let i = Int(lectura.pagina.rounded(.down))
        if i > 0, i < doc.pageCount, let p = doc.page(at: i) {
            vistaPDF.go(to: p)
        }
    }

    private func abrirOffice(_ lectura: LecturaDocumento) {
        modelo.maquetando = true
        let ruta = url.path
        let nucleo = self.nucleo
        tareaOffice = Task { [weak self] in
            let abierto = await Task.detached(priority: .userInitiated) { () -> DocumentoOffice? in
                try? DocumentoOffice.abrir(ruta: ruta, medidor: MedidorCoreText())
            }.value
            guard let office = abierto else {
                self?.modelo.maquetando = false
                self?.modelo.aviso = "No se puede abrir este documento."
                return
            }
            let doc = PDFDocument()
            var n: UInt32 = 0
            while !Task.isCancelled {
                let k = n
                let pagina = await Task.detached(priority: .userInitiated) { () -> Data? in
                    guard let p = office.pagina(n: k),
                          let pintor = try? PintorOffice(pagina: p, imagen: { office.imagen(id: $0) })
                    else { return nil }
                    return pintor.pdf()
                }.value
                guard let self, let datos = pagina, let hoja = PDFDocument(data: datos)?.page(at: 0) else { break }
                doc.insert(hoja, at: doc.pageCount)
                if n == 0 {
                    self.poner(doc, lectura: lectura)
                } else {
                    self.modelo.paginas = doc.pageCount
                    if Int(lectura.pagina) == Int(n) { self.vistaPDF.go(to: hoja) }
                }
                n += 1
            }
            self?.modelo.maquetando = false
            if let self, doc.pageCount > 0 {
                let total = UInt32(doc.pageCount)
                let hojas = await Task.detached(priority: .utility) {
                    nucleo.hojasConTinta(documento: ruta, paginas: total)
                }.value
                self.conTinta = Set(hojas.map { Int($0) })
                self.cargarVisibles()
            }
        }
    }

    /// La capa va dentro de la vista del documento de PDFKit: se desplaza con
    /// las hojas en el mismo fotograma.
    private func engancharCapa() {
        guard let doc = vistaPDF.documentView else { return }
        if capa.superview !== doc {
            capa.removeFromSuperview()
            capa.frame = doc.bounds
            capa.autoresizingMask = [.width, .height]
            doc.addSubview(capa)
        }
        if let clip = doc.enclosingScrollView?.contentView {
            clip.postsBoundsChangedNotifications = true
            vigias.append(NotificationCenter.default.addObserver(
                forName: NSView.boundsDidChangeNotification, object: clip, queue: .main) { [weak self] _ in
                    MainActor.assumeIsolated { self?.alMoverse() }
                })
        }
    }

    // MARK: Hojas y geometria

    /// La hoja `i` en la capa (Y hacia abajo).
    private func geometria(_ pagina: PDFPage) -> GeometriaHoja {
        let enVista = vistaPDF.convert(pagina.bounds(for: vistaPDF.displayBox), from: pagina)
        return GeometriaHoja(rect: capa.convert(enVista, from: vistaPDF))
    }

    /// El alto de una hoja en unidades de la tinta (tal como se ve, girada).
    private func alto(_ pagina: PDFPage) -> Float {
        let b = pagina.bounds(for: vistaPDF.displayBox)
        let girada = pagina.rotation % 180 != 0
        return GeometriaHoja.altoDeHoja(ancho: girada ? b.height : b.width, alto: girada ? b.width : b.height)
    }

    private func indice(_ p: PDFPage) -> Int {
        vistaPDF.document?.index(for: p) ?? 0
    }

    private func alMoverse() {
        if let p = vistaPDF.currentPage { modelo.pagina = indice(p) }
        cargarVisibles()
        capa.needsDisplay = true
    }

    /// Abre (fuera del hilo principal) la tinta de las hojas que se ven y
    /// tienen fichero.
    private func cargarVisibles() {
        for p in vistaPDF.visiblePages {
            let i = indice(p)
            guard conTinta.contains(i), editores[i] == nil, !cargando.contains(i) else { continue }
            cargando.insert(i)
            let ruta = url.path, h = alto(p), nucleo = self.nucleo
            Task {
                let ed = await Task.detached(priority: .userInitiated) {
                    try? nucleo.anotarHoja(documento: ruta, pagina: UInt32(i), alto: h)
                }.value
                self.cargando.remove(i)
                if let ed, self.editores[i] == nil {
                    self.guardarEditor(ed, en: i)
                    self.capa.needsDisplay = true
                }
            }
        }
    }

    private func guardarEditor(_ ed: EditorLienzo, en i: Int) {
        editores[i] = ed
        aplicarHerramienta(ed)
        usar(i)
    }

    /// Apunta que la hoja `i` se uso; si hay demasiadas abiertas, se guarda y
    /// se suelta la mas vieja que no se ve.
    private func usar(_ i: Int) {
        usadas.removeAll { $0 == i }
        usadas.append(i)
        let visibles = Set(vistaPDF.visiblePages.map(indice))
        while usadas.count > ControladorLector.topeDeHojas,
              let vieja = usadas.first(where: { !visibles.contains($0) && $0 != hojaDelTrazo }) {
            usadas.removeAll { $0 == vieja }
            if let ed = editores.removeValue(forKey: vieja) {
                colaGuardar.async { _ = try? ed.guardar() }
            }
            leidos[vieja] = nil
        }
    }

    /// La tinta de la hoja `i`, abriendola ya si hace falta (al empezar a
    /// anotar en ella).
    private func editor(_ i: Int, pagina: PDFPage) -> EditorLienzo? {
        if let ed = editores[i] { return ed }
        do {
            let ed = try nucleo.anotarHoja(documento: url.path, pagina: UInt32(i), alto: alto(pagina))
            guardarEditor(ed, en: i)
            return ed
        } catch {
            modelo.aviso = "No se puede anotar aquí: \(error)"
            return nil
        }
    }

    // MARK: Pintar la capa

    func pintarCapa(en ctx: CGContext, sucio: NSRect, capa: NSView) {
        for p in vistaPDF.visiblePages {
            let i = indice(p)
            let g = geometria(p)
            // El papel de los espacios para anotar.
            let (izq, der) = g.espacios(modelo.espacios)
            ctx.setFillColor(NSColor.textBackgroundColor.withAlphaComponent(0.92).cgColor)
            for r in [izq, der].compactMap({ $0 }) where r.intersects(sucio) {
                ctx.fill(r)
            }
            if let ed = editores[i] {
                let v = ed.version()
                if leidos[i]?.version != v, let l = try? leerLienzo(ed.plano()) {
                    leidos[i] = (v, l)
                }
                if let l = leidos[i]?.lienzo, !l.figuras.isEmpty {
                    ctx.saveGState()
                    ctx.concatenate(g.transformacion)
                    let k = max(g.escala, 0.0001)
                    let visible = CGRect(x: (sucio.minX - g.rect.minX) / k, y: (sucio.minY - g.rect.minY) / k,
                                         width: sucio.width / k, height: sucio.height / k)
                    PintorLienzo(lienzo: l).pintar(en: ctx, limites: visible)
                    ctx.restoreGState()
                }
            }
            // Los marcadores de esta hoja, con su emoticono.
            for m in modelo.marcas {
                guard let punto = g.punto(de: m, pagina: i) else { continue }
                let lado: CGFloat = 20
                let r = CGRect(x: punto.x - lado / 2, y: punto.y - lado / 2, width: lado, height: lado)
                guard r.intersects(sucio) else { continue }
                NSGraphicsContext.saveGraphicsState()
                NSGraphicsContext.current = NSGraphicsContext(cgContext: ctx, flipped: true)
                (m.emoji as NSString).draw(in: r, withAttributes: [.font: NSFont.systemFont(ofSize: lado * 0.8)])
                NSGraphicsContext.restoreGraphicsState()
            }
        }
    }

    // MARK: Anotar

    private func aplicarHerramienta(_ ed: EditorLienzo) {
        let h = modelo.anotando ? modelo.herramienta : "lapiz"
        try? ed.herramienta(nombre: h)
        let c = modelo.color
        ed.tintaColor(r: c.r, g: c.g, b: c.b, a: 1)
    }

    func elegir(_ h: String) {
        modelo.herramienta = h
        if h == "resaltador", modelo.color == ColorTinta.todos[0] {
            modelo.color = ColorTinta.todos[4]
        }
        for ed in editores.values { aplicarHerramienta(ed) }
        ventana.invalidateCursorRects(for: capa)
        capa.needsDisplay = true
    }

    func ponerColor(_ c: ColorTinta) {
        modelo.color = c
        if !modelo.anotando { modelo.herramienta = "lapiz" }
        for ed in editores.values { aplicarHerramienta(ed) }
    }

    func puntero(_ fase: FasePuntero, _ evento: NSEvent, capa: NSView) {
        let p = capa.convert(evento.locationInWindow, from: nil)
        if fase == .pulsar {
            let enVista = vistaPDF.convert(p, from: capa)
            guard let pagina = vistaPDF.page(for: enVista, nearest: true) else { return }
            hojaDelTrazo = indice(pagina)
            presionTrackpad = nil
        }
        guard let i = hojaDelTrazo, let pagina = vistaPDF.document?.page(at: i),
              let ed = editor(i, pagina: pagina) else { return }
        let g = geometria(pagina)
        let q = g.aHoja(p)
        let presion: Float? = evento.subtype == .tabletPoint ? Float(evento.pressure) : presionTrackpad
        var mods: UInt32 = 0
        if evento.modifierFlags.contains(.shift) { mods |= 1 }
        if evento.modifierFlags.contains(.option) { mods |= 2 }
        _ = ed.puntero(fase: fase, x: Float(q.x), y: Float(q.y), presion: presion, modificadores: mods,
                       zoom: Float(g.escala))
        // Repintar solo la hoja (y sus margenes).
        capa.setNeedsDisplay(g.rect.insetBy(dx: -g.rect.width * 0.8, dy: -20))
        if fase == .soltar || fase == .cancelar {
            conTinta.insert(i)
            ultimaAnotada = i
            hojaDelTrazo = nil
            usar(i)
            programarGuardado()
        }
    }

    func deshacer() {
        guard let i = ultimaAnotada, let ed = editores[i] else { return }
        if ed.deshacer() {
            capa.needsDisplay = true
            programarGuardado()
        }
    }

    private func rehacer() {
        guard let i = ultimaAnotada, let ed = editores[i] else { return }
        if ed.rehacer() {
            capa.needsDisplay = true
            programarGuardado()
        }
    }

    // MARK: Guardar

    private func programarGuardado() {
        guardadoPendiente?.cancel()
        let editores = Array(self.editores.values)
        let ruta = url.path
        let tarea = DispatchWorkItem {
            ControladorLector.guardarTinta(editores, ruta: ruta)
        }
        guardadoPendiente = tarea
        colaGuardar.asyncAfter(deadline: .now() + 1.0, execute: tarea)
    }

    private nonisolated static func guardarTinta(_ editores: [EditorLienzo], ruta: String) {
        var alguno = false
        for ed in editores where ed.sinGuardar() {
            do {
                alguno = try ed.guardar() || alguno
            } catch {
                let texto = "\(error)"
                DispatchQueue.main.async { NSLog("PixPin: no se pudo guardar la tinta: %@", texto) }
            }
        }
        guard alguno else { return }
        DispatchQueue.main.async {
            NotificationCenter.default.post(name: .pixpinDocumentoAnotado, object: ruta)
            NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
        }
    }

    /// Lo recordado (aumento, por donde iba, espacios y marcadores), fuera
    /// del hilo principal. `altos`: los de todas las hojas, por si cambian
    /// los espacios (ver `guardar_lectura_de_documento`).
    private func guardarLectura() {
        guard let doc = vistaPDF.document, doc.pageCount > 0 else { return }
        let ajuste = vistaPDF.scaleFactorForSizeToFit
        let zoom = vistaPDF.autoScales || ajuste <= 0 ? 1 : Float(vistaPDF.scaleFactor / ajuste)
        var pagina = Double(modelo.pagina)
        if let p = vistaPDF.currentPage {
            let g = geometria(p)
            let arriba = capa.visibleRect.minY
            let f = (arriba - g.rect.minY) / max(g.rect.height, 1)
            pagina = Double(indice(p)) + Double(min(max(f, 0), 0.999))
        }
        let altos = (0..<doc.pageCount).compactMap { doc.page(at: $0).map(alto) }
        let lectura = LecturaDocumento(zoom: zoom, pagina: pagina, espacios: modelo.espacios,
                                       marcas: modelo.marcas, donde: modelo.donde)
        let ruta = url.path, nucleo = self.nucleo
        colaGuardar.async {
            try? nucleo.guardarLecturaDeDocumento(documento: ruta, lectura: lectura, altos: altos)
        }
    }

    // MARK: Espacios, marcadores, ventana

    private func ponerMargenes() {
        guard let p = vistaPDF.document?.page(at: 0) else { return }
        let ancho = p.bounds(for: vistaPDF.displayBox).width * 0.75
        let izq: CGFloat = modelo.espacios & 1 != 0 ? ancho : 4
        let der: CGFloat = modelo.espacios & 2 != 0 ? ancho : 4
        vistaPDF.pageBreakMargins = NSEdgeInsets(top: 4, left: izq, bottom: 4, right: der)
        vistaPDF.layoutDocumentView()
        capa.needsDisplay = true
    }

    func alternarEspacio(_ lado: UInt8) {
        // Antes de cambiarlos, Rust apunta el marco de la tinta de antes con
        // los espacios de ahora (`fijar_lo_viejo`): se guarda ya.
        guardarLectura()
        modelo.espacios = (modelo.espacios ^ lado) & 3
        ponerMargenes()
        guardarLectura()
    }

    func ponerMarca(emoji: String) {
        guard let p = vistaPDF.currentPage else { return }
        let g = geometria(p)
        let centro = CGPoint(x: g.rect.midX, y: max(capa.visibleRect.minY + 40, g.rect.minY))
        let (x, y) = g.sitio(de: centro, pagina: indice(p))
        modelo.marcas = conMarca(modelo.marcas, x: x, y: y, emoji: emoji,
                                 ahora: Int64(Date().timeIntervalSince1970 * 1000))
        capa.needsDisplay = true
        guardarLectura()
    }

    func ir(a m: MarcaDocumento) {
        guard let doc = vistaPDF.document, let p = doc.page(at: Int(m.y.rounded(.down))) else { return }
        let b = p.bounds(for: vistaPDF.displayBox)
        let f = CGFloat(m.y - m.y.rounded(.down))
        vistaPDF.go(to: CGRect(x: b.minX, y: b.maxY - f * b.height - 40, width: 1, height: 40), on: p)
    }

    func quitarMarcas() {
        modelo.marcas = []
        capa.needsDisplay = true
        guardarLectura()
    }

    /// Clic derecho sobre un marcador: se quita. `true` si habia uno.
    func quitarMarca(en p: CGPoint, capa: NSView) -> Bool {
        for pag in vistaPDF.visiblePages {
            let g = geometria(pag), i = indice(pag)
            let cerca: (MarcaDocumento) -> Bool = { marca in
                guard let q = g.punto(de: marca, pagina: i) else { return false }
                return hypot(q.x - p.x, q.y - p.y) < 14
            }
            if let m = modelo.marcas.first(where: cerca) {
                modelo.marcas.removeAll { $0.id == m.id }
                capa.needsDisplay = true
                guardarLectura()
                return true
            }
        }
        return false
    }

    func ponerEncima(_ si: Bool) {
        modelo.encima = si
        ventana.level = si ? .floating : .normal
    }

    func pantallaCompleta() {
        ventana.toggleFullScreen(nil)
    }

    /// Las teclas del lector. `true` si era suya.
    func tecla(_ e: NSEvent) -> Bool {
        let m = e.modifierFlags.intersection(.deviceIndependentFlagsMask).subtracting([.capsLock, .numericPad, .function])
        let c = e.charactersIgnoringModifiers?.lowercased() ?? ""
        if m == [.control, .command], c == "f" { pantallaCompleta(); return true }
        if e.keyCode == 103 { pantallaCompleta(); return true } // F11, como en Windows
        if m == .command, c == "z" { deshacer(); return true }
        if m == [.command, .shift], c == "z" { rehacer(); return true }
        if m == .command, c == "[" { alternarEspacio(1); return true }
        if m == .command, c == "]" { alternarEspacio(2); return true }
        // Sin modificadores, y solo si no se esta escribiendo en otra cosa.
        guard m.isEmpty, e.type == .keyDown, !(ventana.firstResponder is NSText) else { return false }
        switch c {
        case "a": elegir(modelo.anotando ? "leer" : "lapiz"); return true
        case "p": elegir("lapiz"); return true
        case "h": elegir("resaltador"); return true
        case "e": elegir("borrador"); return true
        case "m": ponerMarca(emoji: emojisDeMarcas().first ?? "🔖"); return true
        default: return false
        }
    }

    // MARK: NSWindowDelegate

    public func windowWillClose(_ notification: Notification) {
        tareaOffice?.cancel()
        guardadoPendiente?.cancel()
        guardarLectura()
        let editores = Array(self.editores.values)
        let ruta = url.path
        colaGuardar.async { ControladorLector.guardarTinta(editores, ruta: ruta) }
        for v in vigias { NotificationCenter.default.removeObserver(v) }
        vigias = []
        self.editores = [:]
        leidos = [:]
        capa.removeFromSuperview()
        vistaPDF.document = nil
        alCerrar?(self)
    }
}

/// **Los lectores abiertos**: uno por documento.
@MainActor
public final class LectoresDeDocumentos {
    public static let compartidos = LectoresDeDocumentos()
    private var abiertos: [String: ControladorLector] = [:]

    /// Abre `url` en el lector (o lo trae delante si ya estaba).
    public func abrir(_ url: URL, nucleo: Nucleo, titulo: String? = nil) {
        let clave = url.standardizedFileURL.path
        if let c = abiertos[clave] {
            c.ventana.makeKeyAndOrderFront(nil)
            NSApp.activate()
            return
        }
        let c = ControladorLector(url: url, nucleo: nucleo, titulo: titulo ?? url.lastPathComponent)
        c.alCerrar = { [weak self] _ in self?.abiertos[clave] = nil }
        abiertos[clave] = c
        c.abrir()
    }
}
