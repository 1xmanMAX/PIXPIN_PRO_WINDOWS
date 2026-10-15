// Como entran los documentos en la app: desde una burbuja del chat (doble
// clic y menu), desde el Finder («Abrir con PixPin») y los ajustes de
// aligerar PDF. Lo demas esta en sus ficheros: el lector
// (`LectorDocumento.swift`), el pin de Vista Rapida (`PinOffice.swift`), las
// notas (`EditorNota.swift`), las hojas (`HojaCalculo.swift`) y aligerar
// (`AligerarPdf.swift`).

import AppKit
import PixPinCore
import SwiftUI

@MainActor
public enum AbrirDocumento {
    /// **Abre un documento como toca**: un PDF en el lector con tinta; un
    /// Word, Excel, PowerPoint, ODF, RTF, CSV o EPUB al instante en un pin de
    /// Vista Rapida. `false` si no es un documento de estos.
    @discardableResult
    public static func abrir(_ url: URL, nucleo: Nucleo, titulo: String? = nil) -> Bool {
        switch TiposDeDocumento.clase(de: url) {
        case .pdf:
            LectoresDeDocumentos.compartidos.abrir(url, nucleo: nucleo, titulo: titulo)
            return true
        case .officePaginado, .vistaRapida:
            PinesDeOffice.compartidos.abrir(url, nucleo: nucleo)
            return true
        case .otro:
            return false
        }
    }

    /// Para anotar un Word o un PowerPoint: el lector paginado como Office.
    public static func leerYAnotar(_ url: URL, nucleo: Nucleo, titulo: String? = nil) {
        LectoresDeDocumentos.compartidos.abrir(url, nucleo: nucleo, titulo: titulo)
    }
}

/// Lo del menu de una burbuja con un documento.
struct MenuDeDocumento: View {
    let url: URL
    let nucleo: Nucleo

    var body: some View {
        switch TiposDeDocumento.clase(de: url) {
        case .pdf:
            Button("Leer y anotar") { AbrirDocumento.abrir(url, nucleo: nucleo) }
            Button("Aligerar el PDF") { ColaAligerar.compartida.aMano(url, nucleo: nucleo) }
        case .officePaginado:
            Button("Abrir en un pin") { PinesDeOffice.compartidos.abrir(url, nucleo: nucleo) }
            Button("Leer y anotar") { AbrirDocumento.leerYAnotar(url, nucleo: nucleo) }
        case .vistaRapida:
            Button("Abrir en un pin") { PinesDeOffice.compartidos.abrir(url, nucleo: nucleo) }
        case .otro:
            EmptyView()
        }
    }
}

/// Bajo la burbuja de un PDF: por donde va al aligerarse y como acabo.
struct EstadoAligerado: View {
    let url: URL
    @ObservedObject private var cola = ColaAligerar.compartida

    var body: some View {
        if let p = cola.progreso[url.path] {
            HStack(spacing: 6) {
                ProgressView(value: Double(p), total: 100).frame(width: 120)
                Text("Aligerando…").font(.caption).foregroundStyle(.secondary)
                Button { cola.cancelar(url) } label: { Image(systemName: "xmark.circle") }
                    .buttonStyle(.borderless)
                    .help("Cancelar")
            }
        } else if let r = cola.resultado[url.path] {
            Text(r).font(.caption).foregroundStyle(.secondary)
        }
    }
}

/// **La burbuja de una hoja de calculo** (`MINIAPP` tabla del PC o `TABLA`
/// del movil): su nombre y abrirla.
struct BurbujaTabla: View {
    let mensaje: MensajeChat
    let chat: String
    let almacen: Almacen

    /// Si un mensaje es una tabla: `TABLA` del movil o la mini-app `tabla`
    /// del PC (su texto es el JSON de `TablaDeCalculo`, con `celdas`). Sin
    /// cruzar el puente: lo pinta cada burbuja.
    static func esTabla(_ m: MensajeChat) -> Bool {
        if m.clase == "TABLA" { return m.referencia?.isEmpty == false }
        guard m.clase == "MINIAPP" else { return false }
        let t = m.texto.trimmingCharacters(in: .whitespacesAndNewlines)
        return t.hasPrefix("{") && t.contains("\"celdas\"")
    }

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "tablecells").font(.title2).foregroundStyle(.green)
            VStack(alignment: .leading, spacing: 1) {
                Text(mensaje.nombre.isEmpty ? "Tabla" : mensaje.nombre).lineLimit(1)
                Text("Hoja de cálculo").font(.caption).foregroundStyle(.secondary)
            }
            Spacer(minLength: 12)
            Button("Abrir") { abrir() }
        }
        .frame(maxWidth: 320, alignment: .leading)
        .contentShape(Rectangle())
        .onTapGesture(count: 2, perform: abrir)
    }

    private func abrir() {
        HojasDeCalculo.compartidas.abrir(mensaje, en: chat, almacen: almacen)
    }
}

/// El menu y el doble clic de una nota (`NOTA`): editarla.
struct AccionesDeNota: ViewModifier {
    let mensaje: MensajeChat
    let chat: String
    let almacen: Almacen

    func body(content: Content) -> some View {
        content
            .contextMenu {
                Button("Editar nota") { EditoresDeNotas.compartidos.abrir(mensaje, en: chat, almacen: almacen) }
            }
            .simultaneousGesture(TapGesture(count: 2).onEnded {
                EditoresDeNotas.compartidos.abrir(mensaje, en: chat, almacen: almacen)
            })
    }
}

/// **Los ajustes de aligerar PDF** (`[pdf]` de `pixpinmax.toml`, los de
/// Windows): una seccion para la ventana de Ajustes.
public struct SeccionAjustesPdf: View {
    let nucleo: Nucleo
    @State private var ajustes: AjustesPdf?
    @State private var error: String?

    public init(nucleo: Nucleo) { self.nucleo = nucleo }

    public var body: some View {
        Section("PDF") {
            if let a = ajustes {
                Toggle("Aligerar los PDF al meterlos en un chat", isOn: Binding(
                    get: { a.aligerarAlEntrar },
                    set: { guardar(AjustesPdf(aligerarAlEntrar: $0, nivel: a.nivel)) }))
                Picker("Nivel", selection: Binding(get: { a.nivel },
                                                   set: { guardar(AjustesPdf(aligerarAlEntrar: a.aligerarAlEntrar, nivel: $0)) })) {
                    Text("Exacto (sin pérdida)").tag(NivelAligerar.sinPerdida)
                    Text("Medio (no se nota)").tag(NivelAligerar.equilibrado)
                    Text("Chico (para pantalla)").tag(NivelAligerar.pequeno)
                    Text("Máximo").tag(NivelAligerar.extremo)
                }
                Text("Al entrar al chat solo se usan Exacto y Medio; Chico y Máximo, con «Aligerar el PDF» del menu del mensaje. Se guarda en pixpinmax.toml, igual que en Windows.")
                    .font(.caption).foregroundStyle(.secondary)
            }
            if let error { Text(error).font(.caption).foregroundStyle(.red) }
        }
        .task { await cargar() }
    }

    private func cargar() async {
        let nucleo = self.nucleo
        let r = await Task.detached(priority: .utility) { Result { try nucleo.ajustesPdf() } }.value
        switch r {
        case let .success(a): ajustes = a
        case let .failure(e): error = "\(e)"
        }
    }

    private func guardar(_ a: AjustesPdf) {
        ajustes = a
        let nucleo = self.nucleo
        Task.detached(priority: .utility) { try? nucleo.guardarAjustesPdf(nuevos: a) }
    }
}

/// **«Abrir con PixPin» desde el Finder** (lo de documentos): un PDF va al
/// lector; Word, Excel, PowerPoint y compania, a un pin de Vista Rapida.
///
/// No instala su propio manejador de Apple Events: lo llama
/// `DelegadoAbrirFicheros` (el delegado de la app), que reparte cada fichero
/// entre los planos y los documentos. Dos manejadores se pisarian.
@MainActor
public final class AbrirDesdeFinder: NSObject {
    public static let compartido = AbrirDesdeFinder()
    private var nucleo: Nucleo?

    /// Se llama una vez al arrancar, con el almacen ya abierto.
    public func instalar(nucleo: Nucleo) {
        self.nucleo = nucleo
    }

    /// Abre `url` si es un documento que PixPin sabe leer.
    public func abrir(_ url: URL) -> Bool {
        guard let nucleo else { return false }
        return AbrirDocumento.abrir(url, nucleo: nucleo)
    }
}
