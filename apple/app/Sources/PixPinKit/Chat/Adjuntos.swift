// Las burbujas con fichero (foto, archivo, dibujo) y lo que entra al chat
// soltando o pegando.
//
// Cada burbuja resuelve su fichero UNA vez (`Almacen.url(deRuta:en:)`,
// memorizado) y pide su miniatura a la cache compartida: ni decodifica la
// foto entera ni toca el disco en el hilo principal (§4bis del plan).

import AppKit
import PixPinCore
import QuickLook
import SwiftUI
import UniformTypeIdentifiers

/// El lado mayor de una foto o un dibujo en su burbuja, en puntos.
let ladoDeBurbuja: CGFloat = 260

// MARK: - Foto

struct BurbujaImagen: View {
    let mensaje: MensajeChat
    let chat: String
    let almacen: Almacen
    @State private var url: URL?

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            if let url {
                MiniaturaVista(url: url, lado: ladoDeBurbuja)
                    .accionesDeFichero(url: url, nombre: mensaje.nombre)
            } else {
                Label(mensaje.nombre, systemImage: "photo")
            }
            // El pie de la foto (el `texto` de un IMAGEN, como en el movil).
            if !mensaje.texto.isEmpty {
                Text(mensaje.texto).textSelection(.enabled)
            }
        }
        .task(id: mensaje.id) { url = await resolver(mensaje, chat, almacen) }
    }
}

// MARK: - Archivo

struct BurbujaArchivo: View {
    let mensaje: MensajeChat
    let chat: String
    let almacen: Almacen
    @State private var url: URL?
    @State private var icono: NSImage?

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 8) {
                Group {
                    if let icono {
                        Image(nsImage: icono).resizable()
                    } else {
                        Image(systemName: Burbuja.icono(mensaje.clase)).resizable().scaledToFit()
                            .foregroundStyle(.secondary).padding(4)
                    }
                }
                .frame(width: 32, height: 32)
                VStack(alignment: .leading, spacing: 1) {
                    Text(mensaje.nombre.isEmpty ? mensaje.resumen : mensaje.nombre)
                        .lineLimit(2).truncationMode(.middle)
                    if mensaje.bytes > 0 {
                        Text(ByteCountFormatter.string(fromByteCount: mensaje.bytes, countStyle: .file))
                            .font(.caption).foregroundStyle(.secondary)
                    }
                }
            }
            .frame(maxWidth: 320, alignment: .leading)
            .contentShape(Rectangle())
            .modifier(AccionesSiHay(url: url, nombre: mensaje.nombre, nucleo: almacen.nucleo))
            // Un PDF que se esta aligerando (o como acabo).
            if let url, TiposDeDocumento.clase(de: url) == .pdf {
                EstadoAligerado(url: url)
            }
            // El pie, o lo que se dijo en una nota de voz.
            if !mensaje.texto.isEmpty {
                Text(mensaje.texto).textSelection(.enabled)
            } else if mensaje.resumen != mensaje.nombre && !mensaje.resumen.isEmpty {
                Text(mensaje.resumen).foregroundStyle(.secondary).textSelection(.enabled)
            }
        }
        .task(id: mensaje.id) {
            guard let u = await resolver(mensaje, chat, almacen) else { return }
            url = u
            // El icono del Finder del propio fichero (el de su app, o la
            // vista previa que dibuje el sistema). Si no esta en el disco,
            // el de su tipo.
            if FileManager.default.fileExists(atPath: u.path) {
                icono = NSWorkspace.shared.icon(forFile: u.path)
            } else {
                icono = NSWorkspace.shared.icon(for: UTType(filenameExtension: u.pathExtension) ?? .data)
            }
        }
    }
}

// MARK: - Dibujo

struct BurbujaDibujo: View {
    let mensaje: MensajeChat
    let chat: String
    let almacen: Almacen
    @State private var dibujo: DibujoDelChat?
    @State private var falta = false
    /// Sube cuando llegan las fotos del dibujo, para volver a pintarlo.
    @State private var fotosListas = 0
    /// Sube cuando el editor guarda este dibujo, para volver a leerlo.
    @State private var recargas = 0

    /// Las fotos de dentro del dibujo se piden al tamano de la burbuja (a
    /// 2x): no hace falta mas para verlas ahi.
    private var pixeles: Int { Int(ladoDeBurbuja * 2) }

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Group {
                if let dibujo {
                    let fotos = dibujo.fotos
                    let pixeles = self.pixeles
                    VistaLienzo(lienzo: dibujo.lienzo, version: fotosListas) { id in
                        fotos[id].flatMap { Miniaturas.compartidas.enCache($0, lado: pixeles) }
                    }
                    .alDobleClic(editar)
                } else if falta {
                    Label(mensaje.nombre.isEmpty ? "Dibujo" : mensaje.nombre,
                          systemImage: "scribble.variable")
                } else {
                    ProgressView()
                }
            }
            .frame(width: ladoDeBurbuja, height: ladoDeBurbuja * 0.7)
            .clipShape(RoundedRectangle(cornerRadius: 8))
            // Doble clic: editarlo en su ventana. Al guardarse alli, se
            // vuelve a leer aqui.
            .contentShape(Rectangle())
            .onTapGesture(count: 2, perform: editar)
            if !mensaje.texto.isEmpty {
                Text(mensaje.texto).textSelection(.enabled)
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .pixpinLienzoGuardado)) { aviso in
            guard let referencia = aviso.object as? String, referencia == mensaje.referencia else { return }
            recargas += 1
        }
        .task(id: "\(mensaje.id)#\(recargas)") {
            guard let referencia = mensaje.referencia, !referencia.isEmpty,
                  let leido = await almacen.dibujo(referencia, en: chat)
            else {
                falta = true
                return
            }
            dibujo = leido
            // Las fotos del dibujo, por la misma cache que las del chat;
            // al tenerlas, se vuelve a pintar una vez.
            var alguna = false
            for url in Set(leido.fotos.values) {
                if await Miniaturas.compartidas.cargar(url, lado: pixeles) != nil { alguna = true }
            }
            if alguna { fotosListas += 1 }
        }
    }

    private func editar() {
        guard let referencia = mensaje.referencia, !referencia.isEmpty else { return }
        VentanasDeLienzo.compartidas.abrir(referencia: referencia, en: chat,
                                           titulo: mensaje.nombre.isEmpty ? "Dibujo" : mensaje.nombre,
                                           almacen: almacen)
    }
}

// MARK: - Abrir, ver y arrastrar un fichero

/// El fichero de un mensaje en este Mac, o `nil`.
@MainActor
private func resolver(_ m: MensajeChat, _ chat: String, _ almacen: Almacen) async -> URL? {
    guard let ruta = m.ruta, !ruta.isEmpty else { return nil }
    if let hecha = almacen.urlResuelta(deRuta: ruta, en: chat) { return hecha }
    return await almacen.url(deRuta: ruta, en: chat)
}

/// Doble clic: Vista Rapida (Quick Look). Menu: abrir con su app o
/// ensenarlo en el Finder. Y se puede arrastrar fuera (al Finder, a Mail...)
/// como una copia del fichero.
struct AccionesDeFichero: ViewModifier {
    let url: URL
    let nombre: String
    /// Con el nucleo, los documentos se abren en el lector o en un pin
    /// (`Documentos/`); sin el, con Vista Rapida.
    var nucleo: Nucleo?
    @State private var vistaRapida: URL?

    func body(content: Content) -> some View {
        content
            .onTapGesture(count: 2) { abrir() }
            .contextMenu {
                if let nucleo { MenuDeDocumento(url: url, nucleo: nucleo) }
                Button("Vista rápida") { vistaRapida = url }
                Button("Abrir") { NSWorkspace.shared.open(url) }
                Button("Mostrar en el Finder") { NSWorkspace.shared.activateFileViewerSelecting([url]) }
            }
            .onDrag {
                // Un proveedor con el FICHERO (no su URL): quien lo recibe
                // hace una copia, y el del proyecto se queda donde esta.
                let p = NSItemProvider(contentsOf: url) ?? NSItemProvider()
                p.suggestedName = nombre.isEmpty ? url.lastPathComponent : nombre
                return p
            }
            .quickLookPreview($vistaRapida)
    }

    private func abrir() {
        // Un plano o un modelo 3D va a su visor (Metal).
        if FileManager.default.fileExists(atPath: url.path),
           VentanasDePlano.compartidas.abrirSiEsSuyo(url, titulo: nombre) {
            return
        }
        // Lo que Quick Look no sabe ensenar (un .dwg sin extension de vista
        // previa) lo abre su app.
        if FileManager.default.fileExists(atPath: url.path) {
            // Un PDF, al lector con tinta; Word, Excel, PowerPoint..., a un pin.
            if let nucleo, AbrirDocumento.abrir(url, nucleo: nucleo, titulo: nombre.isEmpty ? nil : nombre) {
                return
            }
            vistaRapida = url
        } else {
            NSSound.beep()
        }
    }
}

/// `AccionesDeFichero` solo cuando ya se sabe el fichero.
struct AccionesSiHay: ViewModifier {
    let url: URL?
    let nombre: String
    var nucleo: Nucleo?
    @ViewBuilder
    func body(content: Content) -> some View {
        if let url {
            content.modifier(AccionesDeFichero(url: url, nombre: nombre, nucleo: nucleo))
        } else {
            content
        }
    }
}

extension View {
    func accionesDeFichero(url: URL, nombre: String) -> some View {
        modifier(AccionesDeFichero(url: url, nombre: nombre))
    }
}

// MARK: - Soltar y pegar en el chat

/// Lo que entra al chat al soltar o pegar: ficheros (se copian) e imagenes
/// sueltas (de un navegador, de Fotos, del portapapeles).
enum EntradaDeFicheros {
    /// Los tipos que acepta el chat al soltar.
    static let tipos: [UTType] = [.fileURL, .image]

    /// Mete en el chat lo soltado. Leer los proveedores es asincrono y
    /// adjuntar va fuera del hilo principal (`Almacen`).
    @MainActor
    static func soltar(_ proveedores: [NSItemProvider], en chat: String, almacen: Almacen) {
        Task { @MainActor in
            var urls: [URL] = []
            var imagenes: [(Data, String)] = []
            for p in proveedores {
                if p.hasItemConformingToTypeIdentifier(UTType.fileURL.identifier) {
                    if let u = await url(de: p) { urls.append(u) }
                } else if let tipo = p.registeredTypeIdentifiers
                    .compactMap({ UTType($0) })
                    .first(where: { $0.conforms(to: .image) }),
                    let datos = await datos(de: p, tipo: tipo) {
                    imagenes.append((datos, tipo.preferredFilenameExtension ?? "png"))
                }
            }
            if !urls.isEmpty { await almacen.adjuntar(urls: urls, en: chat) }
            for (datos, ext) in imagenes {
                await almacen.adjuntar(datos: datos, formato: ext, en: chat)
            }
        }
    }

    /// ⌘V con ficheros o una imagen en el portapapeles: van al chat. Con
    /// texto, `false` (lo pega la caja de escribir, como siempre).
    @MainActor
    static func pegar(en chat: String, almacen: Almacen) -> Bool {
        let pb = NSPasteboard.general
        let opciones: [NSPasteboard.ReadingOptionKey: Any] = [.urlReadingFileURLsOnly: true]
        if let urls = pb.readObjects(forClasses: [NSURL.self], options: opciones) as? [URL], !urls.isEmpty {
            Task { await almacen.adjuntar(urls: urls, en: chat) }
            return true
        }
        // Una imagen sin texto (una captura copiada, una foto copiada de
        // Vista Previa). Si hay texto, gana el texto.
        guard pb.string(forType: .string) == nil else { return false }
        let candidatos: [(NSPasteboard.PasteboardType, String)] = [(.png, "png"), (.tiff, "tiff")]
        for (tipo, ext) in candidatos {
            if let datos = pb.data(forType: tipo) {
                Task { await almacen.adjuntar(datos: datos, formato: ext, en: chat) }
                return true
            }
        }
        return false
    }

    private static func url(de p: NSItemProvider) async -> URL? {
        await withCheckedContinuation { (c: CheckedContinuation<URL?, Never>) in
            _ = p.loadObject(ofClass: URL.self) { url, _ in c.resume(returning: url) }
        }
    }

    private static func datos(de p: NSItemProvider, tipo: UTType) async -> Data? {
        await withCheckedContinuation { (c: CheckedContinuation<Data?, Never>) in
            _ = p.loadDataRepresentation(forTypeIdentifier: tipo.identifier) { datos, _ in
                c.resume(returning: datos)
            }
        }
    }
}
