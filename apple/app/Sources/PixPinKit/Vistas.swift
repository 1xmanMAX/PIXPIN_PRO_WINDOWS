// La ventana de Proyectos: la lista de chats a la izquierda y el chat a la
// derecha, como en Windows y en el movil.
//
// El chat es una `LazyVStack`: solo existen las filas que se ven, y la
// pagina anterior se pide al llegar arriba (§4bis.2 del plan). Las burbujas
// con fichero (fotos por su miniatura, archivos, dibujos) y lo que entra al
// soltar o pegar estan en `Chat/Adjuntos.swift`.

import AppKit
import PixPinCore
import SwiftUI

public struct VentanaProyectos: View {
    @ObservedObject var almacen: Almacen
    @State private var elegido: String?
    @State private var nuevo = ""
    /// Un chat que pide abrir Buscar (ver `Buscar/Buscar.swift`).
    @ObservedObject private var peticion = PeticionDeChat.compartida
    @Environment(\.openWindow) private var abrirVentana

    public init(almacen: Almacen) { self.almacen = almacen }

    public var body: some View {
        NavigationSplitView {
            List(selection: $elegido) {
                ForEach(almacen.chats, id: \.id) { chat in
                    FilaChat(chat: chat).tag(chat.id)
                }
            }
            .navigationSplitViewColumnWidth(min: 220, ideal: 260)
            .safeAreaInset(edge: .bottom) {
                HStack {
                    TextField("Proyecto nuevo", text: $nuevo)
                        .textFieldStyle(.roundedBorder)
                        .onSubmit(crear)
                    Button(action: crear) { Image(systemName: "plus") }
                        .disabled(nuevo.trimmingCharacters(in: .whitespaces).isEmpty)
                }
                .padding(8)
            }
        } detail: {
            if let chat = elegido {
                VistaChat(almacen: almacen, chat: chat).id(chat)
            } else {
                ContentUnavailableView("Elige un proyecto", systemImage: "bubble.left.and.bubble.right")
            }
        }
        .task { await almacen.cargarChats() }
        .onChange(of: elegido) { _, visible in almacen.aligerar(visible: visible) }
        .onDisappear { almacen.nadaAlFrente() }
        // ⌘F: buscar en todo (chats, notas, tareas, momentos).
        .toolbar {
            Button { abrirVentana(id: IdVentana.buscar) } label: {
                Label("Buscar", systemImage: "magnifyingglass")
            }
            .keyboardShortcut("f")
        }
        .onReceive(peticion.$chat) { pedido in
            guard let pedido else { return }
            elegido = pedido
            peticion.chat = nil
        }
        .overlay(alignment: .bottom) {
            if let error = almacen.error {
                Text(error).font(.caption).padding(6)
                    .background(.red.opacity(0.15), in: RoundedRectangle(cornerRadius: 6))
                    .padding()
            }
        }
    }

    private func crear() {
        let nombre = nuevo.trimmingCharacters(in: .whitespaces)
        guard !nombre.isEmpty else { return }
        nuevo = ""
        Task { await almacen.crearChat(nombre: nombre) }
    }
}

struct FilaChat: View {
    let chat: FichaChat
    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack {
                Text(chat.nombre).font(.headline).lineLimit(1)
                if chat.sinLeer > 0 {
                    Text("\(chat.sinLeer)").font(.caption2).padding(.horizontal, 5)
                        .background(.tint, in: Capsule()).foregroundStyle(.white)
                }
            }
            Text(chat.resumen).font(.caption).foregroundStyle(.secondary).lineLimit(1)
        }
    }
}

public struct VistaChat: View {
    @ObservedObject var almacen: Almacen
    let chat: String
    @State private var borrador = ""
    /// Algo arrastrado encima del chat (para marcarlo).
    @State private var soltando = false
    /// El vigia de ⌘V mientras el chat esta en pantalla.
    @State private var vigiaPegar: Any?
    /// Un salto a un mensaje que pide Buscar (`Chat/SaltoAMensaje.swift`).
    @ObservedObject private var peticion = PeticionDeChat.compartida
    /// El mensaje marcado tras un salto, y al que desplazarse.
    @State private var destacado: String?
    @State private var irA: String?
    /// Sube al llegar la pagina del salto: entonces se desplaza.
    @State private var saltos = 0
    /// Se esta viendo una pagina del medio: no se baja solo al final.
    @State private var enSalto = false

    public var body: some View {
        VStack(spacing: 0) {
            ScrollViewReader { lector in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 6) {
                        if almacen.quedanAnteriores(chat: chat) {
                            ProgressView().frame(maxWidth: .infinity)
                                .task { await almacen.cargarAnteriores(chat: chat) }
                        }
                        ForEach(almacen.mensajes[chat] ?? [], id: \.id) { m in
                            Burbuja(mensaje: m, chat: chat, almacen: almacen)
                                .overlay {
                                    if m.id == destacado {
                                        RoundedRectangle(cornerRadius: 10)
                                            .strokeBorder(Color.accentColor, lineWidth: 2)
                                    }
                                }
                                .id(m.id)
                        }
                        if almacen.quedanPosteriores(chat: chat) {
                            ProgressView().frame(maxWidth: .infinity)
                                .task { await almacen.cargarPosteriores(chat: chat) }
                        }
                    }
                    .padding()
                }
                .defaultScrollAnchor(.bottom)
                .onChange(of: almacen.mensajes[chat]?.last?.id) { _, ultimo in
                    if let ultimo, !enSalto { lector.scrollTo(ultimo, anchor: .bottom) }
                }
                .onChange(of: saltos) { _, _ in
                    guard let id = irA else { return }
                    // Una vuelta despues: que la pila perezosa ya tenga la fila.
                    DispatchQueue.main.async { lector.scrollTo(id, anchor: .center) }
                }
            }
            Divider()
            HStack(alignment: .bottom) {
                TextField("Escribe una nota", text: $borrador, axis: .vertical)
                    .textFieldStyle(.plain)
                    .lineLimit(1...8)
                    .onSubmit(enviar)
                BotonVoz(chat: chat, almacen: almacen)
                Button(action: nuevoLienzo) { Image(systemName: "scribble.variable") }
                    .help("Lienzo nuevo")
                Button("Enviar", action: enviar)
                    .keyboardShortcut(.return, modifiers: .command)
                    .disabled(borrador.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
            .padding(10)
        }
        .task {
            // Si se vino por un salto, la carga es la suya.
            if atenderSalto() || enSalto { return }
            await almacen.abrir(chat: chat)
        }
        .onReceive(peticion.$salto) { _ in _ = atenderSalto() }
        // Soltar ficheros o fotos en el chat los adjunta (se copian).
        .onDrop(of: EntradaDeFicheros.tipos, isTargeted: $soltando) { proveedores in
            MainActor.assumeIsolated {
                EntradaDeFicheros.soltar(proveedores, en: chat, almacen: almacen)
            }
            return true
        }
        .overlay {
            if soltando {
                RoundedRectangle(cornerRadius: 10)
                    .strokeBorder(Color.accentColor, style: StrokeStyle(lineWidth: 3, dash: [8, 5]))
                    .padding(4)
                    .allowsHitTesting(false)
            }
        }
        .onAppear(perform: vigilarPegar)
        .onDisappear(perform: dejarDeVigilarPegar)
    }

    /// ⌘V con ficheros o una imagen en el portapapeles los adjunta, aunque
    /// la caja de escribir tenga el foco (ella solo sabe pegar texto). Con
    /// texto, el ⌘V sigue su camino de siempre.
    private func vigilarPegar() {
        guard vigiaPegar == nil else { return }
        let chat = self.chat
        let almacen = self.almacen
        vigiaPegar = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { evento in
            let teclas = evento.modifierFlags.intersection(.deviceIndependentFlagsMask)
            guard teclas == .command, evento.charactersIgnoringModifiers == "v",
                  evento.window != nil, !(evento.window is NSPanel) else { return evento }
            let tomado = MainActor.assumeIsolated {
                EntradaDeFicheros.pegar(en: chat, almacen: almacen)
            }
            return tomado ? nil : evento
        }
    }

    private func dejarDeVigilarPegar() {
        if let vigia = vigiaPegar { NSEvent.removeMonitor(vigia) }
        vigiaPegar = nil
    }

    /// Toma el salto pedido si es de este chat: carga su pagina, marca el
    /// mensaje y se desplaza hasta el.
    private func atenderSalto() -> Bool {
        guard let s = peticion.salto, s.chat == chat else { return false }
        peticion.salto = nil
        enSalto = true
        destacado = s.mensaje
        irA = s.mensaje
        let almacen = self.almacen
        Task {
            await almacen.abrir(chat: s.chat, en: s.posicion)
            saltos += 1
        }
        return true
    }

    /// Un lienzo nuevo en este chat, abierto ya para dibujar.
    private func nuevoLienzo() {
        enSalto = false
        let chat = self.chat
        let almacen = self.almacen
        Task {
            guard let m = await almacen.crearLienzo(en: chat), let referencia = m.referencia else { return }
            VentanasDeLienzo.compartidas.abrir(referencia: referencia, en: chat,
                                               titulo: "Lienzo nuevo", almacen: almacen)
        }
    }

    private func enviar() {
        enSalto = false
        destacado = nil
        let texto = borrador
        borrador = ""
        Task { await almacen.escribir(nota: texto, en: chat) }
    }
}

struct Burbuja: View {
    let mensaje: MensajeChat
    let chat: String
    /// Sin observar: una burbuja no se repinta por cada cambio del almacen.
    let almacen: Almacen

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            switch mensaje.clase {
            case "NOTA":
                Text(mensaje.texto).textSelection(.enabled)
                    .modifier(AccionesDeNota(mensaje: mensaje, chat: chat, almacen: almacen))
            case "TABLA", "MINIAPP" where BurbujaTabla.esTabla(mensaje):
                BurbujaTabla(mensaje: mensaje, chat: chat, almacen: almacen)
            case "IMAGEN" where mensaje.ruta != nil:
                BurbujaImagen(mensaje: mensaje, chat: chat, almacen: almacen)
            case "VOZ" where mensaje.ruta != nil:
                BurbujaVoz(mensaje: mensaje, chat: chat, almacen: almacen)
            case "ARCHIVO" where mensaje.ruta != nil:
                BurbujaArchivo(mensaje: mensaje, chat: chat, almacen: almacen)
            case "DIBUJO" where mensaje.referencia != nil:
                BurbujaDibujo(mensaje: mensaje, chat: chat, almacen: almacen)
            default:
                Label(mensaje.resumen.isEmpty ? mensaje.nombre : mensaje.resumen,
                      systemImage: Burbuja.icono(mensaje.clase))
            }
            Text(Date(timeIntervalSince1970: TimeInterval(mensaje.cuando) / 1000),
                 format: .dateTime.hour().minute())
                .font(.caption2).foregroundStyle(.secondary)
        }
        .padding(8)
        .background(.quaternary.opacity(0.6), in: RoundedRectangle(cornerRadius: 10))
    }

    static func icono(_ clase: String) -> String {
        switch clase {
        case "IMAGEN": "photo"
        case "VOZ": "waveform"
        case "DIBUJO": "scribble.variable"
        case "PAGINA": "doc.richtext"
        case "MINIAPP": "checklist"
        default: "doc"
        }
    }
}

/// Un lienzo en una `NSView` volteada (Y hacia abajo, como el mundo del
/// lienzo). Ajusta lo dibujado a la vista; pintar es recorrer las figuras
/// ya hechas, sin crear trazados por fotograma.
public final class VistaLienzoNS: NSView {
    public var lienzo: LienzoLeido? { didSet { needsDisplay = true } }
    public var imagen: (UInt64) -> CGImage? = { _ in nil }
    /// El doble clic (abrir el dibujo para editarlo). Lo atiende la vista:
    /// un gesto de SwiftUI no siempre llega a traves de una `NSView`.
    public var alDobleClic: (() -> Void)?

    override public var isFlipped: Bool { true }

    override public func mouseDown(with event: NSEvent) {
        if event.clickCount == 2, let alDobleClic {
            alDobleClic()
        } else {
            super.mouseDown(with: event)
        }
    }

    override public func draw(_ dirtyRect: NSRect) {
        guard let lienzo, let ctx = NSGraphicsContext.current?.cgContext else { return }
        ctx.setFillColor(lienzo.fondo.cg)
        ctx.fill(bounds)
        let caja = lienzo.caja ?? bounds
        let margen: CGFloat = 16
        let escala = min((bounds.width - margen * 2) / max(caja.width, 1),
                         (bounds.height - margen * 2) / max(caja.height, 1), 4)
        ctx.saveGState()
        ctx.translateBy(x: margen - caja.minX * escala, y: margen - caja.minY * escala)
        ctx.scaleBy(x: escala, y: escala)
        let visible = CGRect(x: caja.minX - margen / escala, y: caja.minY - margen / escala,
                             width: bounds.width / escala, height: bounds.height / escala)
        PintorLienzo(lienzo: lienzo, imagen: imagen).pintar(en: ctx, limites: visible)
        ctx.restoreGState()
    }
}

public struct VistaLienzo: NSViewRepresentable {
    public let lienzo: LienzoLeido
    /// Cambiarla vuelve a pintar (p. ej. al llegar las fotos del lienzo).
    public var version: Int
    /// La foto de cada `id_objeto`, ya decodificada (una cache: nunca lee
    /// del disco al pintar).
    public var imagen: (UInt64) -> CGImage?

    public init(lienzo: LienzoLeido, version: Int = 0,
                imagen: @escaping (UInt64) -> CGImage? = { _ in nil }) {
        self.lienzo = lienzo
        self.version = version
        self.imagen = imagen
    }

    /// Lo que hace el doble clic (ver `VistaLienzoNS.alDobleClic`).
    var dobleClic: (() -> Void)?

    /// Con doble clic.
    public func alDobleClic(_ hacer: @escaping () -> Void) -> VistaLienzo {
        var copia = self
        copia.dobleClic = hacer
        return copia
    }

    public func makeNSView(context: Context) -> VistaLienzoNS {
        let v = VistaLienzoNS()
        v.imagen = imagen
        v.alDobleClic = dobleClic
        v.lienzo = lienzo
        return v
    }

    public func updateNSView(_ v: VistaLienzoNS, context: Context) {
        v.imagen = imagen
        v.alDobleClic = dobleClic
        v.lienzo = lienzo
    }
}
