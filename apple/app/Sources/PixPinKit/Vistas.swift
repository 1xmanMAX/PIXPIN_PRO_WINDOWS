// La ventana de Proyectos: la lista de chats a la izquierda y el chat a la
// derecha, como en Windows y en el movil.
//
// El chat es una `LazyVStack`: solo existen las filas que se ven, y la
// pagina anterior se pide al llegar arriba (§4bis.2 del plan).

import AppKit
import PixPinCore
import SwiftUI

public struct VentanaProyectos: View {
    @ObservedObject var almacen: Almacen
    @State private var elegido: String?
    @State private var nuevo = ""

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
                            Burbuja(mensaje: m).id(m.id)
                        }
                    }
                    .padding()
                }
                .defaultScrollAnchor(.bottom)
                .onChange(of: almacen.mensajes[chat]?.last?.id) { _, ultimo in
                    if let ultimo { lector.scrollTo(ultimo, anchor: .bottom) }
                }
            }
            Divider()
            HStack(alignment: .bottom) {
                TextField("Escribe una nota", text: $borrador, axis: .vertical)
                    .textFieldStyle(.plain)
                    .lineLimit(1...8)
                    .onSubmit(enviar)
                Button("Enviar", action: enviar)
                    .keyboardShortcut(.return, modifiers: .command)
                    .disabled(borrador.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
            .padding(10)
        }
        .task { await almacen.abrir(chat: chat) }
    }

    private func enviar() {
        let texto = borrador
        borrador = ""
        Task { await almacen.escribir(nota: texto, en: chat) }
    }
}

struct Burbuja: View {
    let mensaje: MensajeChat
    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            switch mensaje.clase {
            case "NOTA":
                Text(mensaje.texto).textSelection(.enabled)
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

    override public var isFlipped: Bool { true }

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
    public init(lienzo: LienzoLeido) { self.lienzo = lienzo }
    public func makeNSView(context: Context) -> VistaLienzoNS {
        let v = VistaLienzoNS()
        v.lienzo = lienzo
        return v
    }
    public func updateNSView(_ v: VistaLienzoNS, context: Context) { v.lienzo = lienzo }
}
