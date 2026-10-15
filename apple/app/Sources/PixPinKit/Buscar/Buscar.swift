// Buscar en todo: chats, mensajes (notas, ficheros, transcripciones),
// tareas y momentos del timeline, con el buscador del nucleo de Rust
// (`buscar.rs`: sin mayusculas ni tildes y cada palabra en cualquier orden).
//
// Rendimiento (§4bis del plan):
// - se espera a que se deje de escribir (200 ms) antes de buscar, y cada
//   letra nueva CANCELA la busqueda anterior: la `Task` de Swift y, en Rust,
//   el `Busqueda` que mira en cada linea si tiene que parar;
// - los resultados llegan por paginas (`tamPagina`), la siguiente al llegar
//   al final de la lista;
// - un mensaje encontrado se ensena con su pagina del chat alrededor (solo
//   esa pagina), ya desplazada hasta el; «Abrir en Proyectos» abre el chat
//   en esa misma pagina (`SaltoAMensaje`), no en la ultima.

import AppKit
import Foundation
import PixPinCore
import SwiftUI

/// Que la ventana de Proyectos abra un chat (lo pide Buscar). Es un valor y
/// no un aviso suelto: si la ventana aun no estaba abierta, lo lee al
/// aparecer.
@MainActor
public final class PeticionDeChat: ObservableObject {
    public static let compartida = PeticionDeChat()
    @Published public var chat: String?
    /// El mensaje al que saltar dentro del chat (lo toma la vista del chat al
    /// aparecer; ver `Chat/SaltoAMensaje.swift`). Se pone ANTES que `chat`.
    @Published public var salto: SaltoAMensaje?
}

@MainActor
public final class ModeloBuscar: ObservableObject {
    public nonisolated static let tamPagina: UInt32 = 50
    /// Lo que se espera desde la ultima letra antes de buscar.
    public nonisolated static let espera: Duration = .milliseconds(200)

    @Published public private(set) var consulta = ""
    @Published public private(set) var resultados: [ResultadoBusqueda] = []
    @Published public private(set) var hayMas = false
    @Published public private(set) var recortada = false
    @Published public private(set) var buscando = false
    @Published public private(set) var error: String?

    let almacen: Almacen
    /// La busqueda en curso (la espera y la llamada a Rust).
    private(set) var tarea: Task<Void, Never>?
    private var enRust: Busqueda?
    private var cargandoMas = false
    /// La consulta de los resultados que se ven (para pedir mas).
    private var buscada = ""

    public init(almacen: Almacen) {
        self.almacen = almacen
    }

    /// Lo escrito cambio: cancela lo anterior y busca tras la espera.
    public func cambiar(_ texto: String) {
        consulta = texto
        cancelar()
        let limpio = texto.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !limpio.isEmpty else {
            resultados = []
            buscada = ""
            hayMas = false
            recortada = false
            buscando = false
            return
        }
        buscando = true
        tarea = Task { [weak self] in
            try? await Task.sleep(for: ModeloBuscar.espera)
            guard !Task.isCancelled else { return }
            await self?.buscarAhora(limpio)
        }
    }

    /// Para lo que este en marcha (tambien la busqueda en Rust).
    public func cancelar() {
        tarea?.cancel()
        tarea = nil
        enRust?.cancelar()
        enRust = nil
    }

    /// Busca ya, sin esperar (la primera pagina).
    public func buscarAhora(_ texto: String) async {
        let b = Busqueda.nueva()
        enRust = b
        let pagina = await pedir(texto, desde: 0, busqueda: b)
        // Si mientras tanto se escribio otra cosa, esto ya no vale.
        guard enRust === b, !Task.isCancelled else { return }
        buscando = false
        enRust = nil
        guard let pagina, !pagina.cancelada else { return }
        buscada = texto
        resultados = pagina.resultados
        hayMas = pagina.hayMas
        recortada = pagina.recortada
    }

    /// La pagina siguiente (al llegar al final de la lista).
    public func cargarMas() async {
        guard hayMas, !cargandoMas, enRust == nil else { return }
        cargandoMas = true
        defer { cargandoMas = false }
        let texto = buscada
        let b = Busqueda.nueva()
        guard let pagina = await pedir(texto, desde: UInt32(resultados.count), busqueda: b),
              texto == buscada, enRust == nil, !pagina.cancelada else { return }
        resultados.append(contentsOf: pagina.resultados)
        hayMas = pagina.hayMas
        recortada = pagina.recortada
    }

    private func pedir(_ texto: String, desde: UInt32, busqueda: Busqueda) async -> PaginaBusqueda? {
        let nucleo = almacen.nucleo
        let hecho = await withTaskCancellationHandler {
            await Task.detached(priority: .userInitiated) { () -> Result<PaginaBusqueda, Error> in
                Result(catching: {
                    try nucleo.buscar(consulta: texto, desde: desde, cuantos: ModeloBuscar.tamPagina,
                                      busqueda: busqueda)
                })
            }.value
        } onCancel: {
            busqueda.cancelar()
        }
        switch hecho {
        case let .success(p):
            error = nil
            return p
        case let .failure(e):
            error = "\(e)"
            return nil
        }
    }
}

// MARK: - La ventana

public struct VentanaBuscar: View {
    @StateObject private var modelo: ModeloBuscar
    @State private var texto = ""
    @State private var elegido: ResultadoBusqueda?
    @FocusState private var enLaCaja: Bool

    public init(almacen: Almacen) {
        _modelo = StateObject(wrappedValue: ModeloBuscar(almacen: almacen))
    }

    public var body: some View {
        NavigationSplitView {
            List(selection: $elegido) {
                ForEach(modelo.resultados, id: \.self) { r in
                    FilaResultado(r: r).tag(r)
                }
                if modelo.hayMas {
                    ProgressView().frame(maxWidth: .infinity)
                        .task { await modelo.cargarMas() }
                }
                if modelo.recortada {
                    Text("Hay más resultados: afina la búsqueda.")
                        .font(.caption).foregroundStyle(.secondary)
                }
            }
            .navigationSplitViewColumnWidth(min: 280, ideal: 340)
            .overlay {
                if modelo.resultados.isEmpty {
                    if modelo.buscando {
                        ProgressView()
                    } else if !modelo.consulta.trimmingCharacters(in: .whitespaces).isEmpty {
                        ContentUnavailableView.search(text: modelo.consulta)
                    } else {
                        ContentUnavailableView("Buscar en PixPin", systemImage: "magnifyingglass",
                                               description: Text("Chats, notas, archivos, tareas y momentos."))
                    }
                }
            }
        } detail: {
            if let r = elegido {
                DetalleResultado(r: r, almacen: modelo.almacen).id(r)
            } else {
                ContentUnavailableView("Elige un resultado", systemImage: "text.magnifyingglass")
            }
        }
        .searchable(text: $texto, placement: .toolbar, prompt: "Buscar en chats, tareas y momentos")
        .onChange(of: texto) { _, nuevo in
            elegido = nil
            modelo.cambiar(nuevo)
        }
        .searchFocused($enLaCaja)
        .onAppear { enLaCaja = true }
        .onDisappear { modelo.cancelar() }
        .navigationTitle("Buscar")
        .frame(minWidth: 640, minHeight: 420)
    }
}

struct FilaResultado: View {
    let r: ResultadoBusqueda

    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            Image(systemName: FilaResultado.icono(r))
                .foregroundStyle(.secondary)
                .frame(width: 18)
            VStack(alignment: .leading, spacing: 2) {
                Text(r.titulo.isEmpty ? r.nombreChat : r.titulo)
                    .lineLimit(1)
                    .strikethrough(r.clase == .tarea && r.hecha)
                if !r.fragmento.isEmpty, r.fragmento != r.titulo {
                    Text(r.fragmento).font(.caption).foregroundStyle(.secondary).lineLimit(2)
                }
                HStack(spacing: 4) {
                    if !r.nombreChat.isEmpty, r.clase != .chat {
                        Text(r.nombreChat)
                    }
                    if r.cuando > 0 {
                        Text(fechaDe(ms: r.cuando), format: .dateTime.day().month().year())
                    }
                }
                .font(.caption2).foregroundStyle(.tertiary)
            }
        }
    }

    static func icono(_ r: ResultadoBusqueda) -> String {
        switch r.clase {
        case .chat: return "bubble.left.and.bubble.right"
        case .tarea: return r.hecha ? "checkmark.square" : "square"
        case .momento: return "clock"
        case .mensaje: return r.tipo == "NOTA" ? "text.bubble" : Burbuja.icono(r.tipo)
        }
    }
}

/// Lo elegido: un mensaje o una tarea con su pagina del chat alrededor (ya
/// desplazada hasta el y marcado), un chat, o un momento.
struct DetalleResultado: View {
    let r: ResultadoBusqueda
    let almacen: Almacen
    @State private var alrededor: [MensajeChat] = []
    @State private var fallo: String?
    @Environment(\.openWindow) private var abrirVentana

    /// Cuantos mensajes se ensenan antes y despues del encontrado.
    static let margen: UInt64 = 25

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                VStack(alignment: .leading) {
                    Text(r.clase == .momento ? "Timeline" : r.nombreChat).font(.headline)
                    if r.clase == .tarea {
                        Text("Tarea de «\(r.titulo)»").font(.caption).foregroundStyle(.secondary)
                    }
                }
                Spacer()
                if let chat = r.chat {
                    Button("Abrir en Proyectos") {
                        // Un mensaje o una tarea: el chat se abre en su pagina,
                        // desplazado hasta el y marcado.
                        PeticionDeChat.compartida.salto = r.posicion.map {
                            SaltoAMensaje(chat: chat, mensaje: r.mensaje, posicion: $0)
                        }
                        PeticionDeChat.compartida.chat = chat
                        abrirVentana(id: IdVentana.proyectos)
                        NSApp.activate()
                    }
                } else if r.clase == .momento {
                    Button("Abrir el timeline") {
                        abrirVentana(id: IdVentana.timeline)
                        NSApp.activate()
                    }
                }
            }
            .padding(10)
            Divider()
            contenido
        }
        .task { await cargar() }
    }

    @ViewBuilder
    private var contenido: some View {
        if r.clase == .momento || r.chat == nil {
            ScrollView {
                VStack(alignment: .leading, spacing: 8) {
                    Text(r.titulo).font(.title3)
                    Text(r.fragmento).textSelection(.enabled)
                    Text(fechaDe(ms: r.cuando), format: .dateTime).foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding()
            }
        } else if let fallo {
            ContentUnavailableView("No se pudo leer el chat", systemImage: "exclamationmark.triangle",
                                   description: Text(fallo))
        } else if let chat = r.chat {
            ScrollViewReader { lector in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 6) {
                        ForEach(alrededor, id: \.id) { m in
                            Burbuja(mensaje: m, chat: chat, almacen: almacen)
                                .overlay {
                                    if m.id == r.mensaje {
                                        RoundedRectangle(cornerRadius: 10)
                                            .strokeBorder(Color.accentColor, lineWidth: 2)
                                    }
                                }
                                .id(m.id)
                        }
                    }
                    .padding()
                }
                .onChange(of: alrededor.count) { _, _ in
                    if let id = r.mensaje { lector.scrollTo(id, anchor: .center) }
                }
            }
        }
    }

    /// La pagina del chat alrededor del mensaje (solo esa).
    private func cargar() async {
        guard let chat = r.chat, let posicion = r.posicion else { return }
        let nucleo = almacen.nucleo
        let desde = posicion > DetalleResultado.margen ? posicion - DetalleResultado.margen : 0
        let cuantos = UInt32(posicion - desde + DetalleResultado.margen + 1)
        do {
            let pagina = try await Task.detached(priority: .userInitiated) {
                try nucleo.mensajes(chat: chat, desde: desde, cuantos: cuantos)
            }.value
            alrededor = pagina.mensajes
        } catch {
            fallo = "\(error)"
        }
    }
}
