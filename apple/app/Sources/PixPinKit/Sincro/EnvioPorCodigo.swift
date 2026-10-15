// «Enviar y recibir por Wi-Fi» con un codigo de seis cifras, sin grupo:
// `sincronizar/enviar_wifi.rs` y `recibir_wifi.rs` de Windows (y
// `EnviarActivity`/`RecibirActivity` de Android). Los dos sentidos:
//
// - **Enviar y esperar**: se eligen archivos, se abre una puerta con un
//   codigo (y su QR `pixpin-envio:1:…`), se anuncia `_pixpinenvio._tcp` y
//   cada aparato que llega con el codigo espera a que se le apruebe.
// - **Esperar para recibir**: un codigo (y su QR `pixpin-recibe:1:…`) y se
//   anuncia `_pixpinrecibe._tcp`; quien envia lo teclea y manda.
// - **Tengo un codigo**: el del otro (o el texto de su QR): si es de quien
//   envia, se recibe; si es de quien espera, se le mandan archivos.
//
// La red y el protocolo son de Rust (`PuertaEnvio`, `PuertaRecibir`,
// `recibirEnvio`, `mandarAQuienEspera`); aqui solo la pantalla, los hilos
// que esperan en las puertas y el anuncio. Al cerrar la ventana el codigo
// deja de existir.

import AppKit
import PixPinCore
import SwiftUI

@MainActor
public final class ModeloEnvio: ObservableObject {
    /// Un aparato que llamo a la puerta de envio.
    public struct Llamada: Identifiable, Equatable {
        public var id: UInt64
        public var nombre: String
        public var esperaAprobacion: Bool
        public var hechos: UInt64 = 0
        public var total: UInt64 = 0
        public var fin: String?
    }

    /// Lo abierto ahora.
    public enum Abierta: Equatable {
        case nada
        case enviando(codigo: String, archivos: [String])
        case esperando(codigo: String)
    }

    @Published public private(set) var abierta: Abierta = .nada
    @Published public private(set) var qr: NSImage?
    @Published public private(set) var textoDelQr: String?
    @Published public private(set) var llamadas: [Llamada] = []
    /// Lo que se recibio (en esta ventana).
    @Published public private(set) var recibido: [RecibidoEnvio] = []
    @Published public private(set) var fase: String?
    @Published public private(set) var aviso: String?
    @Published public private(set) var error: String?

    public let nucleo: Nucleo
    private var puertaEnvio: PuertaEnvio?
    private var puertaRecibir: PuertaRecibir?
    private let anuncio = AnuncioDePuerta()
    private let nombre: String

    public init(nucleo: Nucleo, nombre: String = Host.current().localizedName ?? "Mac") {
        self.nucleo = nucleo
        self.nombre = nombre
    }

    // MARK: - Enviar y esperar

    /// Abre la puerta para enviar `urls`.
    public func enviar(_ urls: [URL]) async {
        cerrar()
        error = nil
        let nucleo = self.nucleo
        let rutas = urls.map(\.path)
        do {
            let p = try await Task.detached(priority: .utility) { try nucleo.abrirEnvio(rutas: rutas) }.value
            puertaEnvio = p
            abierta = .enviando(codigo: p.codigo(), archivos: urls.map(\.lastPathComponent))
            ponerQR(p.textoDelQr(host: ipv4Local()))
            let nombre = self.nombre
            anuncio.anunciar({ p.anuncio(nombre: nombre) }, puerto: p.puerto())
            atenderEnHilo(nombre: "pixpin-envio-puerta", { try p.atender() },
                          esFin: { $0 == .cerrada || $0 == .quemado }) { _ in }
            escucharAvisos(de: p)
        } catch {
            self.error = textoDeError(error)
        }
    }

    /// Los avisos de la puerta (quien llega, cuanto va), en un hilo que
    /// duerme en `siguienteAviso()` hasta que hay uno.
    private func escucharAvisos(de p: PuertaEnvio) {
        let hilo = Thread { [weak self] in
            while true {
                let a = p.siguienteAviso()
                DispatchQueue.main.async {
                    MainActor.assumeIsolated { self?.recibirAviso(a) }
                }
                if a == .cerrada { return }
            }
        }
        hilo.name = "pixpin-envio-avisos"
        hilo.qualityOfService = .utility
        hilo.start()
    }

    private func recibirAviso(_ a: AvisoEnvio) {
        switch a {
        case let .llega(llamada, nombre):
            llamadas.append(Llamada(id: llamada, nombre: nombre, esperaAprobacion: true))
        case let .avance(llamada, hechos, total):
            if let i = llamadas.firstIndex(where: { $0.id == llamada }) {
                llamadas[i].hechos = hechos
                llamadas[i].total = total
                llamadas[i].esperaAprobacion = false
            }
        case let .fin(llamada, fin):
            let texto = textoDeFin(fin)
            if let i = llamadas.firstIndex(where: { $0.id == llamada }) {
                llamadas[i].fin = texto
                llamadas[i].esperaAprobacion = false
            } else if case .codigoMalo = fin {
                aviso = texto
            } else if case .quemado = fin {
                aviso = texto
            }
        case .cerrada:
            if case .enviando = abierta { abierta = .nada }
        }
    }

    public func aprobar(_ llamada: UInt64, _ si: Bool) {
        puertaEnvio?.aprobar(llamada: llamada, si: si)
        if let i = llamadas.firstIndex(where: { $0.id == llamada }) {
            llamadas[i].esperaAprobacion = false
            if !si { llamadas[i].fin = "No aprobado" }
        }
    }

    public func aprobarTodos() {
        puertaEnvio?.aprobarTodos()
        for i in llamadas.indices { llamadas[i].esperaAprobacion = false }
    }

    // MARK: - Esperar para recibir

    public func esperarParaRecibir() async {
        cerrar()
        error = nil
        let nucleo = self.nucleo
        do {
            let p = try await Task.detached(priority: .utility) { try nucleo.esperarEnvios() }.value
            puertaRecibir = p
            abierta = .esperando(codigo: p.codigo())
            ponerQR(p.textoDelQr(host: ipv4Local()))
            let nombre = self.nombre
            anuncio.anunciar({ p.anuncio(nombre: nombre) }, puerto: p.puerto())
            atenderEnHilo(nombre: "pixpin-recibir-puerta", { try p.atender() },
                          esFin: { $0 == .cerrada || $0 == .quemado }) { [weak self] r in
                DispatchQueue.main.async {
                    MainActor.assumeIsolated { self?.recibio(r) }
                }
            }
        } catch {
            self.error = textoDeError(error)
        }
    }

    private func recibio(_ r: Result<FinRecibir, Error>) {
        switch r {
        case let .success(.recibido(_, cosas)):
            recibido.append(contentsOf: cosas)
            aviso = textoDeRecibido(cosas)
            NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
        case let .success(.codigoMalo(intentos)):
            aviso = "Alguien probó otro código (\(intentos) de 5)."
        case .success(.quemado):
            aviso = "Demasiados códigos equivocados: se cerró."
            abierta = .nada
        case let .success(.fallo(detalle)):
            aviso = detalle
        case .success(.cerrada):
            break
        case let .failure(e):
            aviso = textoDeError(e)
        }
    }

    // MARK: - Tengo un codigo

    /// Con el codigo (o el texto del QR) de otro: recibe de quien envia o,
    /// si es de quien espera para recibir, le manda `archivos` (que hay que
    /// elegir antes).
    public func usarCodigo(_ texto: String, archivos: [URL] = []) async {
        error = nil
        guard let c = leerCodigoDeEnvio(texto: texto) else {
            error = "Eso no es un código de envío: son seis cifras (o el texto de su QR)."
            return
        }
        if c.esperaRecibir && archivos.isEmpty {
            error = "Ese código es de quien espera para recibir: elige qué mandarle."
            return
        }
        var destino: Direccion?
        if let host = c.host, c.puerto > 0 {
            destino = Direccion(host: host, puerto: c.puerto)
        } else {
            fase = "Buscando a quien tiene ese código en esta Wi-Fi…"
            let codigo = c.codigo
            let etiqueta = await Task.detached(priority: .utility) { etiquetaDeEnvio(codigo: codigo) }.value
            let tipo = c.esperaRecibir ? tipoDeServicioRecibir() : tipoDeServicioEnvio()
            destino = await buscarPuerta(tipo: tipo, etiqueta: etiqueta)
        }
        guard let destino else {
            fase = nil
            error = "No apareció nadie con ese código. ¿Tiene PixPin abierto en la misma Wi-Fi?"
            return
        }
        let nucleo = self.nucleo
        do {
            if c.esperaRecibir {
                fase = "Enviando…"
                let rutas = archivos.map(\.path)
                let fin = try await Task.detached(priority: .utility) {
                    try nucleo.mandarAQuienEspera(host: destino.host, puerto: destino.puerto,
                                                  codigo: c.codigo, rutas: rutas)
                }.value
                aviso = textoDeFin(fin)
            } else {
                fase = "Esperando a que te aprueben y recibiendo…"
                let cosas = try await Task.detached(priority: .utility) {
                    try nucleo.recibirEnvio(host: destino.host, puerto: destino.puerto, codigo: c.codigo)
                }.value
                recibido.append(contentsOf: cosas)
                aviso = textoDeRecibido(cosas)
                NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
            }
        } catch {
            self.error = textoDeError(error)
        }
        fase = nil
    }

    // MARK: - Cerrar

    /// Cierra lo abierto: el codigo deja de valer y se deja de anunciar.
    public func cerrar() {
        puertaEnvio?.cerrar()
        puertaEnvio = nil
        puertaRecibir?.cerrar()
        puertaRecibir = nil
        anuncio.retirar()
        abierta = .nada
        qr = nil
        textoDelQr = nil
        llamadas = []
    }

    private func ponerQR(_ texto: String) {
        textoDelQr = texto
        qr = imagenQR(texto)
    }
}

/// Como acabo un envio, en palabras.
public func textoDeFin(_ fin: FinEnvio) -> String {
    switch fin {
    case .cerrada: "Cerrado."
    case let .enviado(nombre): "Enviado a \(nombre)."
    case let .rechazado(nombre): "\(nombre) dijo que no."
    case let .sinAprobar(nombre): "No se aprobó a \(nombre)."
    case let .codigoMalo(intentos): "Alguien probó otro código (\(intentos) de 5)."
    case .quemado: "Demasiados códigos equivocados: se cerró."
    case let .fallo(detalle): detalle
    }
}

/// Lo recibido, en palabras.
public func textoDeRecibido(_ cosas: [RecibidoEnvio]) -> String {
    guard !cosas.isEmpty else { return "No llegó nada." }
    let proyectos = cosas.filter { $0.donde.hasPrefix("proyecto") }.count
    let sueltos = cosas.count - proyectos
    var partes: [String] = []
    if proyectos > 0 { partes.append(proyectos == 1 ? "1 proyecto en la lista" : "\(proyectos) proyectos en la lista") }
    if sueltos > 0 { partes.append(sueltos == 1 ? "1 archivo en Mensajes guardados" : "\(sueltos) archivos en Mensajes guardados") }
    return "Recibido: " + partes.joined(separator: " y ") + "."
}

/// La ventana «Enviar y recibir por Wi-Fi».
public struct VistaEnvio: View {
    @StateObject private var modelo: ModeloEnvio
    @State private var codigo = ""

    public init(nucleo: Nucleo) {
        _modelo = StateObject(wrappedValue: ModeloEnvio(nucleo: nucleo))
    }

    public var body: some View {
        Form {
            switch modelo.abierta {
            case .nada:
                Section {
                    Button("Elegir archivos y enviar…") {
                        let urls = elegirArchivos()
                        if !urls.isEmpty { Task { await modelo.enviar(urls) } }
                    }
                    Button("Esperar para recibir") { Task { await modelo.esperarParaRecibir() } }
                } header: {
                    Text("Sin grupo, con un código de seis cifras")
                }
                Section {
                    TextField("Código (482 913) o texto del QR", text: $codigo)
                        .font(.system(.body, design: .monospaced))
                    HStack {
                        Button("Recibir") { Task { await modelo.usarCodigo(codigo) } }
                        Button("Enviarle archivos…") {
                            let urls = elegirArchivos()
                            if !urls.isEmpty { Task { await modelo.usarCodigo(codigo, archivos: urls) } }
                        }
                    }
                    .disabled(leerCodigoDeEnvio(texto: codigo) == nil || modelo.fase != nil)
                } header: {
                    Text("Tengo el código de otro aparato")
                }
            case let .enviando(c, archivos):
                abierta(codigo: c, titulo: "Enviando \(archivos.count == 1 ? archivos[0] : "\(archivos.count) archivos")")
                Section("Quien llega") {
                    if modelo.llamadas.isEmpty {
                        Text("Esperando a que alguien teclee el código…").foregroundStyle(.secondary)
                    }
                    ForEach(modelo.llamadas) { l in
                        FilaLlamada(llamada: l, aprobar: { modelo.aprobar(l.id, $0) })
                    }
                    if modelo.llamadas.contains(where: \.esperaAprobacion) {
                        Button("Aprobar a todos") { modelo.aprobarTodos() }
                    }
                }
            case let .esperando(c):
                abierta(codigo: c, titulo: "Esperando para recibir")
            }

            if modelo.fase != nil || modelo.aviso != nil || modelo.error != nil {
                Section("Actividad") {
                    if let f = modelo.fase { HStack { ProgressView().controlSize(.small); Text(f) } }
                    if let a = modelo.aviso { Text(a) }
                    if let e = modelo.error {
                        Label(e, systemImage: "exclamationmark.triangle").foregroundStyle(.red)
                    }
                }
            }
        }
        .formStyle(.grouped)
        .frame(minWidth: 420, minHeight: 460)
        .onDisappear { modelo.cerrar() }
    }

    @ViewBuilder private func abierta(codigo: String, titulo: String) -> some View {
        Section {
            VStack(spacing: 10) {
                Text(codigoDeEnvioLegible(codigo: codigo))
                    .font(.system(size: 34, weight: .semibold, design: .monospaced))
                    .textSelection(.enabled)
                if let qr = modelo.qr {
                    Image(nsImage: qr).interpolation(.none).resizable().frame(width: 200, height: 200)
                }
                Button("Cerrar") { modelo.cerrar() }
            }
            .frame(maxWidth: .infinity)
        } header: {
            Text(titulo)
        } footer: {
            Text("En el otro aparato: Enviar o Recibir por Wi-Fi, con este código o escaneando el QR. Vale mientras esta ventana esté abierta.")
                .font(.caption).foregroundStyle(.secondary)
        }
    }

    private func elegirArchivos() -> [URL] {
        let panel = NSOpenPanel()
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = true
        panel.prompt = "Enviar"
        return panel.runModal() == .OK ? panel.urls : []
    }
}

struct FilaLlamada: View {
    let llamada: ModeloEnvio.Llamada
    let aprobar: (Bool) -> Void

    var body: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text(llamada.nombre).font(.headline)
                if let fin = llamada.fin {
                    Text(fin).font(.caption).foregroundStyle(.secondary)
                } else if llamada.total > 0 {
                    ProgressView(value: Double(llamada.hechos), total: Double(max(llamada.total, 1)))
                }
            }
            Spacer()
            if llamada.esperaAprobacion {
                Button("No") { aprobar(false) }
                Button("Aprobar") { aprobar(true) }.keyboardShortcut(.defaultAction)
            }
        }
    }
}
