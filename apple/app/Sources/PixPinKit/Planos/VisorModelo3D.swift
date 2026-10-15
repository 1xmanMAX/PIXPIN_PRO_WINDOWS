// El visor 3D (`ventana3d.rs` + `gpu3d.rs` de Windows con Metal): IFC,
// Revit, LandXML y puntos de Civil 3D, y un DWG/DXF visto en 3D.
//
// - Girar arrastrando (alrededor del punto tocado), mover con dos dedos o
//   con Mayusculas/el boton derecho, acercar pellizcando o con la rueda
//   (hacia lo que hay bajo el puntero). La camara es la de Windows
//   (`camara*` de Rust: `Orbita`).
// - Clic: que elemento es (un rayo de la camara contra los triangulos, en
//   Rust), resaltado en azul, con lo que mide; «Aislar» deja solo ese.
// - Niveles y categorias se encienden y apagan: solo se rehacen los indices.
// - Caja de seccion con sus tapas, como Revit.
// - El terreno de Civil 3D ya trae el color por cota y las curvas de nivel.
//
// Como el visor de planos, la vista esta parada y pinta solo al cambiar.

import AppKit
import Metal
import MetalKit
import PixPinCore
import SwiftUI

/// El modelo en la tarjeta: los vertices (una vez) y los indices de lo que
/// se ve (se rehacen al filtrar).
public final class ModeloEnTarjeta: @unchecked Sendable {
    public let modelo: Modelo3dApple
    public let resumen: ResumenModelo3d
    let vertices: MTLBuffer?
    private(set) var indices: [MTLBuffer?] = [nil, nil, nil, nil, nil]
    private(set) var cuantos: [Int] = [0, 0, 0, 0, 0]
    let motor: MotorMetal

    enum Clase: Int { case opacos, transparentes, aristas, lineas, puntos }

    init(modelo: Modelo3dApple, motor: MotorMetal) {
        self.modelo = modelo
        self.motor = motor
        resumen = modelo.resumen()
        vertices = motor.bufer(modelo.vertices(), nombre: "vertices 3d")
        poner(modelo.indices())
    }

    /// Sube unos indices nuevos (al filtrar).
    func poner(_ i: Indices3d) {
        let todos = [i.opacos, i.transparentes, i.aristas, i.lineas, i.puntos]
        indices = todos.map { motor.bufer($0, nombre: "indices 3d") }
        cuantos = todos.map { $0.count / 4 }
    }

    func bufer(_ c: Clase) -> (MTLBuffer, Int)? {
        guard let b = indices[c.rawValue], cuantos[c.rawValue] > 0 else { return nil }
        return (b, cuantos[c.rawValue])
    }

    /// Lee y sube (bloquea: desde una `Task` aparte).
    public static func cargar(ruta: String, motor: MotorMetal) throws -> ModeloEnTarjeta {
        ModeloEnTarjeta(modelo: try Modelo3dApple.abrir(ruta: ruta), motor: motor)
    }
}

/// **El estado del visor 3D**: el modelo, el filtro, lo elegido y la caja de
/// seccion (en fracciones de la caja del modelo, de 0 a 1).
@MainActor
public final class VisorModelo3D: ObservableObject {
    @Published public private(set) var datos: ModeloEnTarjeta?
    @Published public private(set) var error: String?
    @Published public private(set) var niveles: [Nivel3d] = []
    @Published public private(set) var categorias: [Categoria3d] = []
    @Published public private(set) var filtro = Filtro3d(nivelesOcultos: [], tiposOcultos: [], aislado: nil)
    @Published public private(set) var elegido: InfoElemento3d?
    @Published public var aristas = true { didSet { vista?.repintar() } }
    @Published public var conCorte = false { didSet { vista?.repintar() } }
    /// La caja de seccion: minimo y maximo de cada eje, de 0 a 1.
    @Published public var corte: [Double] = [0, 0, 0, 1, 1, 1] { didSet { vista?.repintar() } }
    public let titulo: String
    public let ruta: String
    weak var vista: VistaModeloMetal?

    public init(ruta: String, titulo: String) {
        self.ruta = ruta
        self.titulo = titulo
    }

    public func cargar() {
        guard datos == nil, error == nil else { return }
        guard let motor = MotorMetal.compartido else {
            error = "Este Mac no tiene una tarjeta gráfica con Metal."
            return
        }
        let ruta = self.ruta
        Task {
            let hecho = await Task.detached(priority: .userInitiated) { () -> Result<ModeloEnTarjeta, Error> in
                Result { try ModeloEnTarjeta.cargar(ruta: ruta, motor: motor) }
            }.value
            switch hecho {
            case let .success(d):
                self.datos = d
                self.niveles = d.modelo.niveles()
                self.categorias = d.modelo.categorias()
                self.vista?.alLlegarElModelo()
            case let .failure(e):
                self.error = "No se pudo abrir el modelo: \(e)"
            }
        }
    }

    /// La caja de seccion en coordenadas del modelo (o `nil` sin corte).
    var cajaDeCorte: Caja3d? {
        guard conCorte, let c = datos?.resumen.caja else { return nil }
        func eje(_ a: Float, _ b: Float, _ t: Double) -> Float { a + (b - a) * Float(t) }
        return Caja3d(x0: eje(c.x0, c.x1, min(corte[0], corte[3])), y0: eje(c.y0, c.y1, min(corte[1], corte[4])),
                      z0: eje(c.z0, c.z1, min(corte[2], corte[5])), x1: eje(c.x0, c.x1, max(corte[0], corte[3])),
                      y1: eje(c.y0, c.y1, max(corte[1], corte[4])), z1: eje(c.z0, c.z1, max(corte[2], corte[5])))
    }

    // MARK: Filtros

    public func verNivel(_ i: UInt32, _ ver: Bool) {
        var f = filtro
        f.nivelesOcultos.removeAll { $0 == i }
        if !ver { f.nivelesOcultos.append(i) }
        filtrar(f)
    }

    public func verCategoria(_ tipo: String, _ ver: Bool) {
        var f = filtro
        f.tiposOcultos.removeAll { $0 == tipo }
        if !ver { f.tiposOcultos.append(tipo) }
        filtrar(f)
    }

    public func aislar() {
        var f = filtro
        f.aislado = f.aislado == nil ? elegido?.id : nil
        filtrar(f)
    }

    public func verTodo() { filtrar(Filtro3d(nivelesOcultos: [], tiposOcultos: [], aislado: nil)) }

    /// Rehace los indices fuera del hilo principal; los vertices se quedan.
    private func filtrar(_ f: Filtro3d) {
        guard let d = datos else { return }
        filtro = f
        Task {
            let i = await Task.detached(priority: .userInitiated) { d.modelo.visibles(filtro: f) }.value
            guard self.filtro == f else { return } // ya se pidio otro
            d.poner(i)
            self.vista?.repintar()
        }
    }

    // MARK: Elegir

    func elegir(_ e: Eleccion3d?) {
        elegido = e.flatMap { datos?.modelo.elemento(id: $0.elemento) }
        vista?.repintar()
    }

    public func encuadrar() { vista?.encuadrar() }
}

// MARK: - La vista de Metal

public final class VistaModeloMetal: MTKView {
    let visor: VisorModelo3D
    private let motor: MotorMetal
    private let muestras: Int
    private(set) var camara = Camara3d(objetivo: Punto3d(x: 0, y: 0, z: 0), rumbo: -0.8, altura: 0.5, dist: 10)
    private var radio = 10.0
    private var encuadrado = false
    /// El arrastre: donde empezo, la camara de entonces, el pivote y si giraba.
    private var arrastre: (p: CGPoint, camara: Camara3d, pivote: Punto3d, girar: Bool, movido: Bool)?
    /// El ultimo punto tocado por la rueda: se reusa mientras se sigue girando.
    private var rueda: (cuando: Date, p: CGPoint, fijo: Punto3d)?

    init(visor: VisorModelo3D, motor: MotorMetal) {
        self.visor = visor
        self.motor = motor
        muestras = motor.muestras
        super.init(frame: NSRect(x: 0, y: 0, width: 800, height: 600), device: motor.dispositivo)
        colorPixelFormat = .bgra8Unorm
        depthStencilPixelFormat = .depth32Float
        clearDepth = 1
        sampleCount = muestras
        isPaused = true
        enableSetNeedsDisplay = true
        visor.vista = self
    }

    @available(*, unavailable)
    required init(coder: NSCoder) { fatalError("init(coder:) no se usa") }

    override public var acceptsFirstResponder: Bool { true }

    func repintar() { needsDisplay = true }

    private var escala: Double {
        bounds.width > 0 ? Double(drawableSize.width) / Double(bounds.width) : 2
    }

    private var ancho: UInt32 { UInt32(max(drawableSize.width, 1)) }
    private var alto: UInt32 { UInt32(max(drawableSize.height, 1)) }
    private var claro: Bool { effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) != .darkAqua }

    func alLlegarElModelo() {
        guard let d = visor.datos else { return }
        let c = d.resumen.caja
        radio = max(Double(hypot(hypot(c.x1 - c.x0, c.y1 - c.y0), c.z1 - c.z0)) / 2, 1e-3)
        encuadrado = false
        encuadrar()
    }

    func encuadrar() {
        guard let d = visor.datos, drawableSize.width > 0 else { return }
        camara = camaraEncuadrar(caja: d.resumen.cajaUtil, ancho: ancho, alto: alto)
        encuadrado = true
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

    // MARK: Pintar

    /// Lo que el sombreador 3D sabe de la vista (`Vista3d`, 176 bytes).
    static func vista(_ c: Camara3d, ancho: UInt32, alto: UInt32, radio: Double, corte: Caja3d?,
                      elegido: UInt32?, aislado: UInt32?, claro: Bool, radioPuntos: Float) -> PalabrasUniforme {
        var u = PalabrasUniforme()
        for x in camaraMatriz(camara: c, ancho: ancho, alto: alto, radio: radio) { u.f(x) }
        let l = luzDeCamara(c)
        u.f4(l.x, l.y, l.z, 0)
        let o = camaraOjo(camara: c)
        u.f4(Float(o.x), Float(o.y), Float(o.z), 1)
        u.f4(corte == nil ? 0 : 1, 0, radioPuntos, 0)
        u.f4(corte?.x0 ?? 0, corte?.y0 ?? 0, corte?.z0 ?? 0, 0)
        u.f4(corte?.x1 ?? 0, corte?.y1 ?? 0, corte?.z1 ?? 0, 0)
        u.u(elegido.map { $0 + 1 } ?? 0)
        u.f(Float(claro ? 1 : 0))
        u.f(Float(ancho))
        u.f(Float(alto))
        u.u(aislado.map { $0 + 1 } ?? 0)
        u.f(Float(0))
        u.f(Float(0))
        u.f(Float(0))
        return u
    }

    private func tuberia(_ v: String, _ f: String, mezcla: Bool) throws -> MTLRenderPipelineState {
        try motor.tuberia(fuente: SombreadoresCAD.modelo, claveFuente: "modelo", vertice: v, fragmento: f,
                          color: colorPixelFormat, profundidad: depthStencilPixelFormat, muestras: muestras, mezcla: mezcla)
    }

    override public func draw(_ dirtyRect: NSRect) {
        let claro = self.claro
        clearColor = claro ? MTLClearColor(red: 0.955, green: 0.958, blue: 0.965, alpha: 1)
            : MTLClearColor(red: 0.13, green: 0.15, blue: 0.19, alpha: 1)
        guard let pasada = currentRenderPassDescriptor, let dibujable = currentDrawable,
              let cb = motor.cola.makeCommandBuffer(),
              let enc = cb.makeRenderCommandEncoder(descriptor: pasada) else { return }
        if let d = visor.datos, let vb = d.vertices {
            do {
                try pintar(d, vb, enc, claro: claro)
            } catch {
                NSLog("PixPin 3D: \(error)")
            }
        }
        enc.endEncoding()
        cb.present(dibujable)
        cb.commit()
    }

    private func pintar(_ d: ModeloEnTarjeta, _ vb: MTLBuffer, _ enc: MTLRenderCommandEncoder, claro: Bool) throws {
        let corte = visor.cajaDeCorte
        let u = VistaModeloMetal.vista(camara, ancho: ancho, alto: alto, radio: radio, corte: corte,
                                       elegido: visor.elegido?.id, aislado: visor.filtro.aislado, claro: claro,
                                       radioPuntos: Float(4.5 * escala))
        let largo = u.bytes
        u.palabras.withUnsafeBytes { b in
            if let base = b.baseAddress {
                enc.setVertexBytes(base, length: largo, index: 1)
                enc.setFragmentBytes(base, length: largo, index: 0)
            }
        }
        enc.setCullMode(.none)
        enc.setVertexBuffer(vb, offset: 0, index: 0)
        let escribe = motor.profundidad(escribe: true), lee = motor.profundidad(escribe: false)
        func dibujar(_ c: ModeloEnTarjeta.Clase, _ tipo: MTLPrimitiveType) {
            guard let par = d.bufer(c) else { return }
            let (ib, n) = par
            enc.drawIndexedPrimitives(type: tipo, indexCount: n, indexType: .uint32, indexBuffer: ib, indexBufferOffset: 0)
        }
        // 1. Lo opaco, un poco hacia atras para que las aristas queden encima.
        enc.setRenderPipelineState(try tuberia("vs_3d", "ps_3d", mezcla: false))
        enc.setDepthStencilState(escribe)
        enc.setDepthBias(800, slopeScale: 1.5, clamp: 0)
        dibujar(.opacos, .triangle)
        // 2. Las aristas, encima.
        enc.setDepthBias(0, slopeScale: 0, clamp: 0)
        if visor.aristas {
            enc.setRenderPipelineState(try tuberia("vs_3d", "ps_arista", mezcla: true))
            enc.setDepthStencilState(lee)
            dibujar(.aristas, .line)
        }
        // 2a. Las tapas de la caja de seccion.
        if corte != nil {
            enc.setRenderPipelineState(try tuberia("vs_3d", "ps_tapa", mezcla: false))
            enc.setDepthStencilState(escribe)
            enc.setDepthBias(800, slopeScale: 1.5, clamp: 0)
            dibujar(.opacos, .triangle)
            enc.setDepthBias(0, slopeScale: 0, clamp: 0)
        }
        // 2b. Las rayas sueltas (curvas de nivel, ejes) y los puntos.
        enc.setRenderPipelineState(try tuberia("vs_3d", "ps_linea", mezcla: true))
        enc.setDepthStencilState(escribe)
        dibujar(.lineas, .line)
        enc.setRenderPipelineState(try tuberia("vs_punto", "ps_punto", mezcla: false))
        dibujar(.puntos, .triangle)
        // 3. Los vidrios, al final y sin tapar.
        enc.setRenderPipelineState(try tuberia("vs_3d", "ps_vidrio", mezcla: true))
        enc.setDepthStencilState(lee)
        dibujar(.transparentes, .triangle)
    }

    // MARK: Gestos

    private func punto(_ e: NSEvent) -> CGPoint { convert(e.locationInWindow, from: nil) }

    /// El pixel (desde arriba a la izquierda, como `camara_rayo`).
    private func pixel(_ p: CGPoint) -> (Double, Double) {
        (Double(p.x) * escala, Double(bounds.height - p.y) * escala)
    }

    /// Lo que hay bajo un punto de la vista.
    private func elegir(_ p: CGPoint) -> Eleccion3d? {
        guard let d = visor.datos else { return nil }
        let (x, y) = pixel(p)
        let r = camaraRayo(camara: camara, x: x, y: y, ancho: ancho, alto: alto)
        return d.modelo.elegir(origen: camaraOjo(camara: camara), direccion: r, filtro: visor.filtro, corte: visor.cajaDeCorte)
    }

    /// El punto bajo el puntero o, sin nada, el del rayo a la distancia del objetivo.
    private func puntoBajo(_ p: CGPoint) -> Punto3d {
        if let e = elegir(p) { return e.punto }
        let (x, y) = pixel(p)
        let r = camaraRayo(camara: camara, x: x, y: y, ancho: ancho, alto: alto)
        let o = camaraOjo(camara: camara)
        return Punto3d(x: o.x + r.x * camara.dist, y: o.y + r.y * camara.dist, z: o.z + r.z * camara.dist)
    }

    private func zoom(_ factor: Double, en p: CGPoint) {
        let fijo: Punto3d
        if let r = rueda, Date().timeIntervalSince(r.cuando) < 0.4, abs(r.p.x - p.x) + abs(r.p.y - p.y) < 4 {
            fijo = r.fijo
        } else {
            fijo = puntoBajo(p)
        }
        rueda = (Date(), p, fijo)
        camara = camaraZoom(camara: camara, factor: factor, fijo: fijo)
        repintar()
    }

    override public func scrollWheel(with e: NSEvent) {
        let p = punto(e)
        if e.hasPreciseScrollingDeltas && !e.modifierFlags.contains(.command) && !e.modifierFlags.contains(.control) {
            // Dos dedos: mover (el modelo sigue a los dedos).
            camara = camaraMover(camara: camara, dx: Double(e.scrollingDeltaX) * escala,
                                 dy: Double(e.scrollingDeltaY) * escala, alto: alto)
            repintar()
        } else {
            let paso = e.hasPreciseScrollingDeltas ? Double(e.scrollingDeltaY) / 40 : Double(e.scrollingDeltaY)
            zoom(pow(1.2, -paso), en: p)
        }
    }

    override public func magnify(with e: NSEvent) {
        zoom(1 / max(1 + Double(e.magnification), 0.1), en: punto(e))
    }

    override public func smartMagnify(with e: NSEvent) { encuadrar() }

    override public func mouseDown(with e: NSEvent) {
        window?.makeFirstResponder(self)
        let p = punto(e)
        let girar = !e.modifierFlags.contains(.shift)
        arrastre = (p, camara, girar ? puntoBajo(p) : camara.objetivo, girar, false)
    }

    override public func rightMouseDown(with e: NSEvent) {
        arrastre = (punto(e), camara, camara.objetivo, false, false)
    }

    private func arrastrar(_ e: NSEvent) {
        guard var a = arrastre else { return }
        let p = punto(e)
        let dx = Double(p.x - a.p.x) * escala, dy = -Double(p.y - a.p.y) * escala
        if !a.movido && abs(dx) + abs(dy) <= 3 { return }
        a.movido = true
        arrastre = a
        if a.girar {
            // Media vuelta por el lado corto de la ventana, sea del tamano que sea.
            let k = Double.pi / Double(max(min(ancho, alto), 1))
            camara = camaraGirar(camara: a.camara, dRumbo: -dx * k, dAltura: dy * k, pivote: a.pivote)
        } else {
            camara = camaraMover(camara: a.camara, dx: dx, dy: dy, alto: alto)
        }
        repintar()
    }

    override public func mouseDragged(with e: NSEvent) { arrastrar(e) }
    override public func rightMouseDragged(with e: NSEvent) { arrastrar(e) }
    override public func rightMouseUp(with e: NSEvent) { arrastre = nil }

    override public func mouseUp(with e: NSEvent) {
        defer { arrastre = nil }
        // Un clic sin arrastrar: que es.
        if let a = arrastre, !a.movido { visor.elegir(elegir(punto(e))) }
    }

    override public func keyDown(with e: NSEvent) {
        let letra = e.charactersIgnoringModifiers?.lowercased() ?? ""
        switch e.keyCode {
        case 53: visor.elegir(nil)
        case 123: girarTeclado(0.26, 0)
        case 124: girarTeclado(-0.26, 0)
        case 126: girarTeclado(0, 0.2)
        case 125: girarTeclado(0, -0.2)
        default:
            switch letra {
            case "f": encuadrar()
            case "+", "=": camara = camaraZoom(camara: camara, factor: 1 / 1.4, fijo: camara.objetivo); repintar()
            case "-": camara = camaraZoom(camara: camara, factor: 1.4, fijo: camara.objetivo); repintar()
            default: super.keyDown(with: e)
            }
        }
    }

    private func girarTeclado(_ r: Double, _ a: Double) {
        camara = camaraGirar(camara: camara, dRumbo: r, dAltura: a, pivote: camara.objetivo)
        repintar()
    }
}

// MARK: - SwiftUI

struct LienzoMetalModelo: NSViewRepresentable {
    let visor: VisorModelo3D
    let motor: MotorMetal

    func makeNSView(context _: Context) -> VistaModeloMetal { VistaModeloMetal(visor: visor, motor: motor) }
    func updateNSView(_ v: VistaModeloMetal, context _: Context) { v.repintar() }
}

/// **La ventana de un modelo 3D**: la barra, el modelo y el panel (niveles,
/// categorias, caja de seccion y lo elegido).
public struct VentanaModeloView: View {
    @ObservedObject var visor: VisorModelo3D
    @State private var verPanel = true

    public init(visor: VisorModelo3D) { self.visor = visor }

    public var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 10) {
                Button { visor.encuadrar() } label: { Image(systemName: "arrow.up.left.and.arrow.down.right") }
                    .help("Ver el modelo entero (F)")
                Toggle("Aristas", isOn: $visor.aristas).toggleStyle(.button)
                Toggle("Caja de sección", isOn: $visor.conCorte).toggleStyle(.button)
                Button(visor.filtro.aislado == nil ? "Aislar" : "Ver todo") { visor.aislar() }
                    .disabled(visor.elegido == nil && visor.filtro.aislado == nil)
                Spacer()
                if let r = visor.datos?.resumen {
                    Text("\(r.elementos) elementos").foregroundStyle(.secondary).font(.callout)
                }
                Toggle(isOn: $verPanel) { Image(systemName: "sidebar.right") }.toggleStyle(.button)
            }
            .disabled(visor.datos == nil)
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(.bar)
            Divider()
            HStack(spacing: 0) {
                contenido
                if verPanel && visor.datos != nil {
                    Divider()
                    panel.frame(width: 260)
                }
            }
        }
    }

    @ViewBuilder private var contenido: some View {
        if let error = visor.error {
            ContentUnavailableView("No se puede ver el modelo", systemImage: "exclamationmark.triangle",
                                   description: Text(error))
        } else if let motor = MotorMetal.compartido {
            ZStack {
                LienzoMetalModelo(visor: visor, motor: motor)
                if visor.datos == nil { ProgressView("Leyendo el modelo…") }
            }
        } else {
            ContentUnavailableView("Sin Metal", systemImage: "cpu", description: Text("Este Mac no tiene una tarjeta gráfica con Metal."))
        }
    }

    private var panel: some View {
        List {
            if let e = visor.elegido {
                Section("Elegido") {
                    LabeledContent("Categoría", value: e.categoria)
                    if !e.nombre.isEmpty { LabeledContent("Nombre", value: e.nombre) }
                    if let n = e.nivel { LabeledContent("Nivel", value: n) }
                    LabeledContent("Clase", value: e.tipo)
                    if e.volumen > 0 {
                        LabeledContent(e.cerrado ? "Volumen" : "Volumen (aprox.)", value: String(format: "%.3f", e.volumen))
                    }
                    if e.planta > 0 { LabeledContent("Área en planta", value: String(format: "%.3f", e.planta)) }
                    if e.alzado > 0 { LabeledContent("Área en alzado", value: String(format: "%.3f", e.alzado)) }
                    LabeledContent("Superficie", value: String(format: "%.3f", e.superficie))
                }
            }
            if visor.conCorte {
                Section("Caja de sección") {
                    ForEach(0 ..< 3, id: \.self) { k in
                        let eje = ["X", "Y", "Z"][k]
                        Slider(value: $visor.corte[k], in: 0 ... 1) { Text("\(eje) desde") }
                        Slider(value: $visor.corte[k + 3], in: 0 ... 1) { Text("\(eje) hasta") }
                    }
                }
            }
            if !visor.niveles.isEmpty {
                Section("Niveles") {
                    ForEach(visor.niveles, id: \.indice) { n in
                        Toggle(isOn: Binding(get: { !visor.filtro.nivelesOcultos.contains(n.indice) },
                                             set: { visor.verNivel(n.indice, $0) })) {
                            Text("\(n.nombre) · \(String(format: "%.2f", n.cota)) m").lineLimit(1)
                        }
                        .toggleStyle(.checkbox)
                    }
                }
            }
            Section("Categorías") {
                ForEach(visor.categorias, id: \.tipo) { c in
                    Toggle(isOn: Binding(get: { !visor.filtro.tiposOcultos.contains(c.tipo) },
                                         set: { visor.verCategoria(c.tipo, $0) })) {
                        Text("\(c.nombre) (\(c.elementos))").lineLimit(1)
                    }
                    .toggleStyle(.checkbox)
                }
            }
        }
        .listStyle(.sidebar)
    }
}
