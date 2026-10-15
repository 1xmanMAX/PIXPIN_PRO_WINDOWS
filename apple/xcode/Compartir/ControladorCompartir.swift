// La extension de Compartir: mandar a un proyecto de PixPin desde cualquier
// app (el `compartir.rs` de Windows, §3 del plan). Se elige el proyecto y
// lo compartido entra como en Windows al soltarlo en el chat: los ficheros
// y las imagenes se COPIAN dentro del proyecto con su mensaje (`IMAGEN`,
// `VOZ` o `ARCHIVO`, por `Nucleo.adjuntar`), y el texto y los enlaces van
// como una nota.
//
// La extension vive en su caja de arena: la carpeta de datos de PixPin
// (`~/Library/Application Support/PixPinMax`) se abre por la excepcion de
// `Soporte/Compartir.entitlements`, con la ruta de la casa de verdad (la
// de `FileManager` seria la del contenedor de la extension).
//
// Todo lo que toca el disco va fuera del hilo principal (§4bis.3).

import AppKit
import PixPinCore
import UniformTypeIdentifiers

@objc(ControladorCompartir)
final class ControladorCompartir: NSViewController {
    private let lista = NSPopUpButton(frame: .zero, pullsDown: false)
    private let estado = NSTextField(wrappingLabelWithString: "Leyendo los proyectos…")
    private let botonEnviar = NSButton(title: "Enviar", target: nil, action: nil)
    private var nucleo: Nucleo?
    private var ids: [String] = []

    override func loadView() {
        let titulo = NSTextField(labelWithString: "Mandar a un proyecto de PixPin")
        titulo.font = .boldSystemFont(ofSize: 13)
        estado.font = .systemFont(ofSize: 11)
        estado.textColor = .secondaryLabelColor
        lista.isEnabled = false
        botonEnviar.target = self
        botonEnviar.action = #selector(enviar(_:))
        botonEnviar.keyEquivalent = "\r"
        botonEnviar.isEnabled = false
        let botonCancelar = NSButton(title: "Cancelar", target: self, action: #selector(cancelar(_:)))
        botonCancelar.keyEquivalent = "\u{1b}"
        let hueco = NSView()
        hueco.setContentHuggingPriority(.defaultLow, for: .horizontal)
        let botones = NSStackView(views: [hueco, botonCancelar, botonEnviar])
        botones.orientation = .horizontal
        let pila = NSStackView(views: [titulo, lista, estado, botones])
        pila.orientation = .vertical
        pila.alignment = .leading
        pila.spacing = 10
        pila.edgeInsets = NSEdgeInsets(top: 16, left: 16, bottom: 16, right: 16)
        lista.widthAnchor.constraint(equalTo: pila.widthAnchor, constant: -32).isActive = true
        botones.widthAnchor.constraint(equalTo: pila.widthAnchor, constant: -32).isActive = true
        estado.widthAnchor.constraint(equalTo: pila.widthAnchor, constant: -32).isActive = true
        pila.frame = NSRect(x: 0, y: 0, width: 380, height: 150)
        view = pila
        preferredContentSize = NSSize(width: 380, height: 150)
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        cargarProyectos()
    }

    /// La carpeta de datos de PixPin, en la casa de verdad del usuario.
    static func carpetaDeDatos() -> URL {
        var casa = NSHomeDirectory()
        if let pw = getpwuid(getuid()), let dir = pw.pointee.pw_dir {
            casa = String(cString: dir)
        }
        return URL(fileURLWithPath: casa, isDirectory: true)
            .appendingPathComponent("Library/Application Support/PixPinMax", isDirectory: true)
    }

    private func cargarProyectos() {
        let raiz = ControladorCompartir.carpetaDeDatos()
        guard FileManager.default.fileExists(atPath: raiz.path) else {
            estado.stringValue = "Abre PixPin una vez en este Mac antes de compartir."
            return
        }
        let nombre = Host.current().localizedName ?? "Mac"
        Task.detached(priority: .userInitiated) { [weak self] in
            let leido = Result { () throws -> (Nucleo, [FichaChat]) in
                let n = try Nucleo.abrir(raiz: raiz.path, nombreDelAparato: nombre)
                // «Mensajes guardados» siempre esta (y va primero).
                _ = try n.chatGuardados()
                return (n, try n.chats().filter { !$0.archivado })
            }
            await MainActor.run { self?.mostrar(leido) }
        }
    }

    private func mostrar(_ leido: Result<(Nucleo, [FichaChat]), Error>) {
        switch leido {
        case let .success((n, chats)):
            nucleo = n
            ids = chats.map(\.id)
            lista.removeAllItems()
            lista.addItems(withTitles: chats.map(\.nombre))
            lista.isEnabled = !chats.isEmpty
            botonEnviar.isEnabled = !chats.isEmpty
            estado.stringValue = chats.isEmpty ? "No hay proyectos." : ""
        case let .failure(e):
            estado.stringValue = "No se pudo abrir la carpeta de PixPin: \(e.localizedDescription)"
        }
    }

    @objc private func enviar(_ sender: Any?) {
        let i = lista.indexOfSelectedItem
        guard let nucleo, i >= 0, i < ids.count else { return }
        let chat = ids[i]
        let items = (extensionContext?.inputItems as? [NSExtensionItem]) ?? []
        botonEnviar.isEnabled = false
        estado.stringValue = "Enviando…"
        Task {
            let fallos = await Entrega.entregar(items, a: chat, nucleo: nucleo)
            if fallos.isEmpty {
                extensionContext?.completeRequest(returningItems: nil, completionHandler: nil)
            } else {
                estado.stringValue = fallos.joined(separator: "\n")
                botonEnviar.isEnabled = true
            }
        }
    }

    @objc private func cancelar(_ sender: Any?) {
        extensionContext?.cancelRequest(withError: NSError(domain: NSCocoaErrorDomain, code: NSUserCancelledError))
    }
}

/// Lo compartido, dentro del proyecto.
enum Entrega {
    /// Entrega todo; devuelve lo que fallo (vacio si fue bien).
    static func entregar(_ items: [NSExtensionItem], a chat: String, nucleo: Nucleo) async -> [String] {
        var textos: [String] = []
        var fallos: [String] = []
        for item in items {
            // Lo que se escribio en la hoja de compartir.
            if let t = item.attributedContentText?.string,
               !t.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                textos.append(t)
            }
            for proveedor in item.attachments ?? [] {
                do {
                    if let texto = try await entregar(proveedor, a: chat, nucleo: nucleo) {
                        textos.append(texto)
                    }
                } catch {
                    fallos.append(error.localizedDescription)
                }
            }
        }
        if !textos.isEmpty {
            let nota = textos.joined(separator: "\n")
            do {
                _ = try await Task.detached(priority: .userInitiated) {
                    try nucleo.escribirNota(chat: chat, texto: nota, respondeA: nil)
                }.value
            } catch {
                fallos.append(error.localizedDescription)
            }
        }
        return fallos
    }

    /// Un adjunto: un fichero o una imagen se adjuntan; un enlace o un texto
    /// se devuelven para la nota.
    static func entregar(_ p: NSItemProvider, a chat: String, nucleo: Nucleo) async throws -> String? {
        if p.hasItemConformingToTypeIdentifier(UTType.fileURL.identifier) {
            let item = try await cargar(p, UTType.fileURL)
            var url = item as? URL
            if url == nil, let datos = item as? Data {
                url = URL(dataRepresentation: datos, relativeTo: nil)
            }
            guard let url, url.isFileURL else { return nil }
            let ruta = url.path
            _ = try await Task.detached(priority: .userInitiated) {
                try nucleo.adjuntar(chat: chat, rutaOrigen: ruta, nombre: nil)
            }.value
            return nil
        }
        if p.hasItemConformingToTypeIdentifier(UTType.url.identifier) {
            let item = try await cargar(p, UTType.url)
            if let url = item as? URL { return url.absoluteString }
            if let datos = item as? Data, let url = URL(dataRepresentation: datos, relativeTo: nil) {
                return url.absoluteString
            }
            return nil
        }
        if p.hasItemConformingToTypeIdentifier(UTType.image.identifier) {
            let tipo = p.registeredTypeIdentifiers.compactMap { UTType($0) }
                .first { $0.conforms(to: .image) } ?? .png
            let bytes = try await datos(p, tipo)
            try await adjuntarImagen(bytes, tipo: tipo, a: chat, nucleo: nucleo)
            return nil
        }
        if p.hasItemConformingToTypeIdentifier(UTType.plainText.identifier) {
            let item = try await cargar(p, UTType.plainText)
            if let s = item as? String { return s }
            if let a = item as? NSAttributedString { return a.string }
            if let d = item as? Data { return String(data: d, encoding: .utf8) }
        }
        return nil
    }

    /// Una imagen sin fichero: `compartido-<hora>.<ext>` (un TIFF, como PNG,
    /// que es lo que entienden Windows y Android), adjuntada y borrada.
    static func adjuntarImagen(_ datos: Data, tipo: UTType, a chat: String, nucleo: Nucleo) async throws {
        try await Task.detached(priority: .userInitiated) {
            var bytes = datos
            var ext = tipo.preferredFilenameExtension ?? "png"
            if tipo.conforms(to: .tiff) || ext == "tif" || ext == "tiff",
               let png = NSBitmapImageRep(data: bytes)?.representation(using: .png, properties: [:]) {
                bytes = png
                ext = "png"
            }
            let carpeta = FileManager.default.temporaryDirectory
                .appendingPathComponent("pixpin-compartido-\(UUID().uuidString)", isDirectory: true)
            defer { try? FileManager.default.removeItem(at: carpeta) }
            try FileManager.default.createDirectory(at: carpeta, withIntermediateDirectories: true)
            let ms = Int64(Date().timeIntervalSince1970 * 1000)
            let fichero = carpeta.appendingPathComponent("compartido-\(ms).\(ext)")
            try bytes.write(to: fichero)
            _ = try nucleo.adjuntar(chat: chat, rutaOrigen: fichero.path, nombre: nil)
        }.value
    }

    static func cargar(_ p: NSItemProvider, _ tipo: UTType) async throws -> NSSecureCoding? {
        try await withCheckedThrowingContinuation { (c: CheckedContinuation<NSSecureCoding?, Error>) in
            p.loadItem(forTypeIdentifier: tipo.identifier, options: nil) { item, error in
                if let error {
                    c.resume(throwing: error)
                } else {
                    c.resume(returning: item)
                }
            }
        }
    }

    static func datos(_ p: NSItemProvider, _ tipo: UTType) async throws -> Data {
        try await withCheckedThrowingContinuation { (c: CheckedContinuation<Data, Error>) in
            _ = p.loadDataRepresentation(forTypeIdentifier: tipo.identifier) { datos, error in
                if let datos {
                    c.resume(returning: datos)
                } else {
                    c.resume(throwing: error ?? CocoaError(.fileReadUnknown))
                }
            }
        }
    }
}
