// La galeria de capturas: las de `capturas/`, la mas nueva primero, con los
// dias que les quedan, y conservar, «7 dias mas», borrar (con deshacer) y
// Vista Rapida. Lo que decide (caducidad, papelera, mandar al chat) es del
// nucleo de Rust, con los mismos ficheros que Windows (`galeria.rs`).
//
// Rendimiento (§4bis del plan):
// - la lista llega por paginas (`tamPagina`) y la rejilla es perezosa
//   (`LazyVGrid`): solo existen las celdas que se ven, y la pagina
//   siguiente se pide al llegar al final;
// - las miniaturas son de ImageIO, reducidas al leer y en la cache con tope
//   de toda la app (`Miniaturas.compartidas`), fuera del hilo principal;
// - la carpeta la vigila el sistema (`VigiaCarpeta`): sin sondear.

import AppKit
import Foundation
import PixPinCore
import QuickLook
import SwiftUI

/// Los dias que aguanta una captura (el ajuste «dias hasta borrar» de
/// Windows, `[capturas] dias_caducidad`). 0 = nunca.
public enum AjusteCaducidad {
    public static let clave = "capturas.diasCaducidad"
    public static let porOmision = 7

    public static var dias: Int64 {
        let d = UserDefaults.standard.object(forKey: clave) as? Int ?? porOmision
        return Int64(max(0, d))
    }
}

@MainActor
public final class ModeloGaleria: ObservableObject {
    /// Cuantas capturas trae cada pagina: unas cuantas pantallas de rejilla.
    public nonisolated static let tamPagina: UInt32 = 120

    @Published public private(set) var capturas: [CapturaGaleria] = []
    @Published public private(set) var total: UInt64 = 0
    @Published public private(set) var error: String?
    /// Lo ultimo que se borro, para «Deshacer».
    @Published public private(set) var ultimoBorrado: [CapturaBorrada] = []
    @Published public private(set) var aviso: String?

    public let almacen: Almacen
    private var cargando = false
    private var vigia: VigiaCarpeta?
    /// Los dias del ajuste; las pruebas lo fijan.
    var dias: () -> Int64 = { AjusteCaducidad.dias }

    public init(almacen: Almacen) {
        self.almacen = almacen
    }

    private var nucleo: Nucleo { almacen.nucleo }

    public var quedanMas: Bool { UInt64(capturas.count) < total }

    /// Empieza a vigilar `capturas/` (al abrir la ventana).
    public func vigilar() {
        guard vigia == nil else { return }
        let carpeta = URL(fileURLWithPath: nucleo.carpetaCapturas(), isDirectory: true)
        try? FileManager.default.createDirectory(at: carpeta, withIntermediateDirectories: true)
        vigia = VigiaCarpeta(carpeta) { [weak self] in
            Task { await self?.recargar() }
        }
    }

    public func dejarDeVigilar() {
        vigia = nil
    }

    /// Relee desde el principio lo que ya estaba cargado (al menos una
    /// pagina): tras borrar o al cambiar la carpeta.
    public func recargar() async {
        let cuantos = UInt32(min(max(capturas.count, Int(ModeloGaleria.tamPagina)), 2000))
        await cargar(desde: 0, cuantos: cuantos, sustituir: true)
    }

    /// La pagina siguiente a lo cargado (al llegar al final de la rejilla).
    public func cargarMas() async {
        guard quedanMas else { return }
        await cargar(desde: UInt64(capturas.count), cuantos: ModeloGaleria.tamPagina, sustituir: false)
    }

    private func cargar(desde: UInt64, cuantos: UInt32, sustituir: Bool) async {
        guard !cargando else { return }
        cargando = true
        defer { cargando = false }
        let nucleo = self.nucleo
        let dias = self.dias()
        let desfase = desfaseLocalMs()
        do {
            let pagina = try await Task.detached(priority: .userInitiated) {
                try nucleo.galeria(desde: desde, cuantos: cuantos, dias: dias, desfaseMs: desfase)
            }.value
            if sustituir {
                capturas = pagina.capturas
            } else {
                capturas.append(contentsOf: pagina.capturas)
            }
            total = pagina.total
            error = nil
        } catch {
            self.error = "\(error)"
        }
    }

    /// Conserva las capturas: van a «Mensajes guardados» y dejan de caducar.
    public func conservar(_ nombres: [String]) async {
        let nucleo = self.nucleo
        let pendientes = nombres.filter { n in capturas.first { $0.nombre == n }?.conservada != true }
        guard !pendientes.isEmpty else {
            if !nombres.isEmpty { aviso = "Ya estaban conservadas" }
            return
        }
        let fallo = await Task.detached(priority: .userInitiated) { () -> String? in
            var fallo: String?
            for n in pendientes {
                do { _ = try nucleo.conservarCaptura(nombre: n) } catch { fallo = "\(n): \(error)" }
            }
            return fallo
        }.value
        error = fallo
        aviso = fallo == nil ? (pendientes.count == 1 ? "Conservada en Mensajes guardados"
            : "\(pendientes.count) conservadas en Mensajes guardados") : nil
        NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
        await recargar()
    }

    /// «Dar 7 dias mas».
    public func prorrogar(_ nombres: [String]) async {
        let nucleo = self.nucleo
        let dias = self.dias()
        let fallo = await Task.detached(priority: .userInitiated) { () -> String? in
            var fallo: String?
            for n in nombres {
                do { _ = try nucleo.prorrogarCaptura(nombre: n, dias: dias) } catch { fallo = "\(n): \(error)" }
            }
            return fallo
        }.value
        error = fallo
        if fallo == nil { aviso = "7 días más" }
        await recargar()
    }

    /// Borra sin preguntar (a la papelera de PixPin): se deshace con
    /// `deshacer()`.
    public func borrar(_ nombres: [String]) async {
        guard !nombres.isEmpty else { return }
        let nucleo = self.nucleo
        let resultado = await Task.detached(priority: .userInitiated) { () -> Result<[CapturaBorrada], Error> in
            Result(catching: { try nucleo.borrarCapturas(nombres: nombres) })
        }.value
        switch resultado {
        case let .success(borradas):
            ultimoBorrado = borradas
            error = nil
            aviso = borradas.count == 1 ? "Borrada" : "\(borradas.count) borradas"
        case let .failure(e):
            error = "\(e)"
        }
        await recargar()
    }

    /// Devuelve lo ultimo borrado a su sitio, con lo que tenia apuntado.
    public func deshacer() async {
        let borradas = ultimoBorrado
        guard !borradas.isEmpty else { return }
        ultimoBorrado = []
        let nucleo = self.nucleo
        let vueltas = await Task.detached(priority: .userInitiated) {
            (try? nucleo.devolverCapturas(borradas: borradas)) ?? 0
        }.value
        aviso = vueltas == 0 ? "No se pudo deshacer" : nil
        await recargar()
    }

    public func url(de c: CapturaGaleria) -> URL {
        URL(fileURLWithPath: c.ruta)
    }
}

// MARK: - La ventana

public struct VentanaGaleria: View {
    @StateObject private var modelo: ModeloGaleria
    @State private var elegidas: Set<String> = []
    @State private var vistaRapida: URL?

    private let columnas = [GridItem(.adaptive(minimum: 170, maximum: 260), spacing: 10)]

    public init(almacen: Almacen) {
        _modelo = StateObject(wrappedValue: ModeloGaleria(almacen: almacen))
    }

    public var body: some View {
        ScrollView {
            LazyVGrid(columns: columnas, spacing: 10) {
                ForEach(modelo.capturas, id: \.nombre) { c in
                    CeldaCaptura(captura: c, elegida: elegidas.contains(c.nombre))
                        .onTapGesture(count: 2) { vistaRapida = modelo.url(de: c) }
                        .onTapGesture { elegir(c.nombre) }
                        .contextMenu { menu(para: c) }
                        .onDrag {
                            NSItemProvider(contentsOf: modelo.url(de: c)) ?? NSItemProvider()
                        }
                }
                if modelo.quedanMas {
                    ProgressView()
                        .frame(maxWidth: .infinity, minHeight: 60)
                        .task { await modelo.cargarMas() }
                }
            }
            .padding(12)
        }
        .overlay {
            if modelo.capturas.isEmpty, modelo.total == 0 {
                ContentUnavailableView("Sin capturas",
                                       systemImage: "photo.on.rectangle.angled",
                                       description: Text(verbatim: "Las capturas guardadas aparecen aquí y se borran solas a los \(AjusteCaducidad.dias) días, salvo las que conservas."))
            }
        }
        .overlay(alignment: .bottom) { pie }
        .quickLookPreview($vistaRapida, in: modelo.capturas.map { modelo.url(de: $0) })
        .onDeleteCommand { borrar(Array(elegidas)) }
        .toolbar { barra }
        .navigationTitle("Galería de capturas")
        .navigationSubtitle(subtitulo)
        .task {
            await modelo.recargar()
            modelo.vigilar()
        }
        .onDisappear { modelo.dejarDeVigilar() }
        .frame(minWidth: 560, minHeight: 420)
    }

    private var subtitulo: String {
        modelo.total == 1 ? "1 captura" : "\(modelo.total) capturas"
    }

    @ToolbarContentBuilder
    private var barra: some ToolbarContent {
        ToolbarItemGroup {
            Button {
                if let n = elegidas.first, let c = modelo.capturas.first(where: { $0.nombre == n }) {
                    vistaRapida = modelo.url(de: c)
                }
            } label: { Label("Vista rápida", systemImage: "eye") }
                .disabled(elegidas.count != 1)
                .keyboardShortcut("y")
            Button { conservar(Array(elegidas)) } label: { Label("Conservar", systemImage: "pin") }
                .disabled(elegidas.isEmpty)
            Button { prorrogar(Array(elegidas)) } label: { Label("7 días más", systemImage: "clock.arrow.circlepath") }
                .disabled(elegidas.isEmpty)
            Button { borrar(Array(elegidas)) } label: { Label("Borrar", systemImage: "trash") }
                .disabled(elegidas.isEmpty)
            Button { Task { await modelo.deshacer() } } label: { Label("Deshacer", systemImage: "arrow.uturn.backward") }
                .disabled(modelo.ultimoBorrado.isEmpty)
                .keyboardShortcut("z")
            Button {
                NSWorkspace.shared.open(URL(fileURLWithPath: modelo.almacen.nucleo.carpetaCapturas(), isDirectory: true))
            } label: { Label("Abrir la carpeta", systemImage: "folder") }
        }
    }

    @ViewBuilder
    private var pie: some View {
        if let texto = modelo.error ?? modelo.aviso {
            Text(texto)
                .font(.callout)
                .padding(.horizontal, 12).padding(.vertical, 6)
                .background(modelo.error != nil ? AnyShapeStyle(.red.opacity(0.2)) : AnyShapeStyle(.regularMaterial),
                            in: Capsule())
                .padding()
        }
    }

    @ViewBuilder
    private func menu(para c: CapturaGaleria) -> some View {
        let nombres = elegidas.contains(c.nombre) ? Array(elegidas) : [c.nombre]
        Button("Vista rápida") { vistaRapida = modelo.url(de: c) }
        Button("Abrir") { NSWorkspace.shared.open(modelo.url(de: c)) }
        Button("Mostrar en el Finder") { NSWorkspace.shared.activateFileViewerSelecting([modelo.url(de: c)]) }
        Button("Copiar") { copiar(c) }
        Button("Pinear") {
            if !GestorPines.compartido.pinear(fichero: modelo.url(de: c)) { NSSound.beep() }
        }
        Divider()
        Button("Conservar") { conservar(nombres) }
            .disabled(nombres.count == 1 && c.conservada)
        Button("Dar 7 días más") { prorrogar(nombres) }
            .disabled(nombres.count == 1 && c.seVa == nil)
        Divider()
        Button("Borrar", role: .destructive) { borrar(nombres) }
    }

    private func elegir(_ nombre: String) {
        if NSEvent.modifierFlags.contains(.command) {
            if elegidas.contains(nombre) { elegidas.remove(nombre) } else { elegidas.insert(nombre) }
        } else {
            elegidas = [nombre]
        }
    }

    private func copiar(_ c: CapturaGaleria) {
        let url = modelo.url(de: c)
        let tablero = NSPasteboard.general
        tablero.clearContents()
        var objetos: [NSPasteboardWriting] = [url as NSURL]
        if !c.esVideo, let imagen = NSImage(contentsOf: url) {
            objetos.insert(imagen, at: 0)
        }
        tablero.writeObjects(objetos)
    }

    private func conservar(_ nombres: [String]) {
        Task { await modelo.conservar(nombres) }
    }

    private func prorrogar(_ nombres: [String]) {
        Task { await modelo.prorrogar(nombres) }
    }

    private func borrar(_ nombres: [String]) {
        guard !nombres.isEmpty else { return }
        elegidas.subtract(nombres)
        Task { await modelo.borrar(nombres) }
    }
}

/// Una captura en la rejilla: su miniatura, la chapita de los dias que le
/// quedan (roja) o de conservada (verde), y el marco si esta elegida.
struct CeldaCaptura: View {
    let captura: CapturaGaleria
    let elegida: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            MiniaturaCaptura(url: URL(fileURLWithPath: captura.ruta), esVideo: captura.esVideo)
                .frame(height: 130)
                .frame(maxWidth: .infinity)
                .clipShape(RoundedRectangle(cornerRadius: 8))
                .overlay(alignment: .topTrailing) { InsigniaCaducidad(captura: captura).padding(6) }
                .overlay {
                    RoundedRectangle(cornerRadius: 8)
                        .strokeBorder(elegida ? Color.accentColor : .clear, lineWidth: 3)
                }
            Text(fechaDe(ms: captura.cuando), format: .dateTime.day().month().hour().minute())
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .contentShape(Rectangle())
        .help(captura.nombre)
    }
}

/// Los dias que le quedan: «hoy», «1», «6»... en rojo; o un pin verde si
/// esta conservada. Nada si la caducidad esta apagada.
struct InsigniaCaducidad: View {
    let captura: CapturaGaleria

    var body: some View {
        if captura.conservada {
            Image(systemName: "pin.fill")
                .font(.caption2.bold())
                .padding(5)
                .background(.green, in: Circle())
                .foregroundStyle(.white)
                .help("Conservada: no se borra")
        } else if let d = captura.diasQueQuedan {
            Text(d <= 0 ? "hoy" : "\(d)")
                .font(.caption2.bold())
                .padding(.horizontal, 6).padding(.vertical, 2)
                .background(.red, in: Capsule())
                .foregroundStyle(.white)
                .help(InsigniaCaducidad.pista(dias: d))
        }
    }

    static func pista(dias d: Int64) -> String {
        switch d {
        case ..<1: return "Se borra hoy. Clic derecho para conservarla o darle 7 días más."
        case 1: return "Se borra mañana. Clic derecho para conservarla o darle 7 días más."
        default: return "Se borra en \(d) días. Clic derecho para conservarla o darle 7 días más."
        }
    }
}

/// La miniatura de una captura (ImageIO, reducida al leer, en la cache de
/// toda la app). Un video lleva un icono: no se decodifica.
struct MiniaturaCaptura: View {
    let url: URL
    let esVideo: Bool
    /// Lado mayor en pixeles: la celda mide hasta ~260 puntos a 2x.
    static let lado = 400
    @State private var imagen: CGImage?
    @State private var falta = false

    var body: some View {
        ZStack {
            Rectangle().fill(.quaternary)
            if let imagen {
                Image(decorative: imagen, scale: 2)
                    .resizable()
                    .interpolation(.medium)
                    .scaledToFill()
            } else if esVideo {
                Image(systemName: "film").font(.largeTitle).foregroundStyle(.secondary)
            } else if falta {
                Image(systemName: "photo.badge.exclamationmark").foregroundStyle(.secondary)
            } else {
                ProgressView().controlSize(.small)
            }
        }
        .clipped()
        .task(id: url) {
            guard !esVideo else { return }
            if let hecha = Miniaturas.compartidas.enCache(url, lado: MiniaturaCaptura.lado) {
                imagen = hecha
                return
            }
            let leida = await Miniaturas.compartidas.cargar(url, lado: MiniaturaCaptura.lado)
            imagen = leida
            falta = leida == nil
        }
    }
}
