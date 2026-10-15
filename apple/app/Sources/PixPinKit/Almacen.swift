// El estado de la app: el nucleo de Rust abierto sobre la carpeta de datos,
// la lista de chats y las paginas de mensajes que se ven.
//
// Rendimiento (§4bis del plan):
// - todo lo que toca el disco va FUERA del hilo principal (`Task.detached`
//   con prioridad `.utility`), y solo el resultado vuelve al principal;
// - los mensajes se piden por paginas (`pagina`), nunca el chat entero;
// - ante presion de memoria se sueltan las paginas que no se ven y se
//   vacian las caches del nucleo.

import AppKit
import Foundation
import PixPinCore

/// Donde viven los datos de PixPin en este Mac: la misma estructura que
/// `%APPDATA%\PixPinMax` en Windows (`proyectos/`, `sincro/`...).
public func carpetaDeDatos() -> URL {
    let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
    return base.appendingPathComponent("PixPinMax", isDirectory: true)
}

@MainActor
public final class Almacen: ObservableObject {
    /// Cuantos mensajes trae cada pagina: los que caben en una ventana alta
    /// con margen para desplazarse sin esperar.
    public nonisolated static let tamPagina: UInt32 = 60

    @Published public private(set) var chats: [FichaChat] = []
    @Published public private(set) var error: String?
    /// Los mensajes cargados de cada chat abierto (solo los de las paginas
    /// pedidas), del mas antiguo al mas nuevo.
    @Published public private(set) var mensajes: [String: [MensajeChat]] = [:]
    /// Cuantos tiene cada chat en total (para saber si quedan mas antiguos).
    @Published public private(set) var totales: [String: UInt64] = [:]

    public let nucleo: Nucleo
    private var desde: [String: UInt64] = [:]
    /// Cada vez que se carga un chat desde cero (abrirlo o saltar a un
    /// mensaje) sube su turno: lo que llegue de una carga anterior se tira.
    private var turnos: [String: Int] = [:]
    /// Chats pidiendo ya lo de despues (para no pedirlo dos veces).
    private var cargandoPosteriores: Set<String> = []
    /// Rutas ya resueltas (`chat` + `ruta` del mensaje -> fichero). Pocas
    /// y pequenas; se sueltan al aligerar.
    private var rutas: [String: URL] = [:]
    private var vigiaMemoria: DispatchSourceMemoryPressure?

    public init(raiz: URL = carpetaDeDatos(), nombreDelAparato: String = Host.current().localizedName ?? "Mac") throws {
        try FileManager.default.createDirectory(at: raiz, withIntermediateDirectories: true)
        nucleo = try Nucleo.abrir(raiz: raiz.path, nombreDelAparato: nombreDelAparato)
        vigilarMemoria()
    }

    /// Relee la lista de chats.
    public func cargarChats() async {
        let nucleo = self.nucleo
        do {
            let lista = try await Task.detached(priority: .utility) { try nucleo.chats() }.value
            chats = lista
            error = nil
        } catch {
            self.error = "\(error)"
        }
    }

    /// La ultima pagina de un chat (lo mas nuevo), sustituyendo lo cargado.
    public func abrir(chat: String) async {
        let nucleo = self.nucleo
        // Lo que llegue suelto de otro aparato («Enviar a…») va a este chat.
        if let ficha = chats.first(where: { $0.id == chat }) {
            nucleo.ponerAlFrente(frente: .chat(id: ficha.id, nombre: ficha.nombre))
        }
        let turno = nuevoTurno(chat)
        do {
            let pagina = try await Task.detached(priority: .userInitiated) {
                try nucleo.ultimosMensajes(chat: chat, cuantos: Almacen.tamPagina)
            }.value
            guard turno == turnos[chat] else { return }
            mensajes[chat] = pagina.mensajes
            totales[chat] = pagina.total
            desde[chat] = pagina.desde
        } catch {
            self.error = "\(error)"
        }
    }

    /// Ya no hay ningun chat delante (se cerro la ventana): lo que llegue
    /// suelto va a «Mensajes guardados».
    public func nadaAlFrente() {
        nucleo.ponerAlFrente(frente: .nada)
    }

    /// Si quedan mensajes mas antiguos que los cargados.
    public func quedanAnteriores(chat: String) -> Bool {
        (desde[chat] ?? 0) > 0
    }

    /// La pagina anterior a lo cargado (al subir en el chat).
    public func cargarAnteriores(chat: String) async {
        guard let inicio = desde[chat], inicio > 0 else { return }
        let nucleo = self.nucleo
        let cuantos = UInt32(min(UInt64(Almacen.tamPagina), inicio))
        let pedir = inicio - UInt64(cuantos)
        do {
            let pagina = try await Task.detached(priority: .userInitiated) {
                try nucleo.mensajes(chat: chat, desde: pedir, cuantos: cuantos)
            }.value
            // Si mientras tanto se cargo otra cosa (un salto), no se pega.
            guard desde[chat] == inicio else { return }
            mensajes[chat] = pagina.mensajes + (mensajes[chat] ?? [])
            desde[chat] = pagina.desde
            totales[chat] = pagina.total
        } catch {
            self.error = "\(error)"
        }
    }

    /// Escribe una nota y la anade al final de lo cargado.
    @discardableResult
    public func escribir(nota: String, en chat: String) async -> MensajeChat? {
        let texto = nota.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !texto.isEmpty else { return nil }
        let nucleo = self.nucleo
        do {
            let m = try await Task.detached(priority: .userInitiated) {
                try nucleo.escribirNota(chat: chat, texto: texto, respondeA: nil)
            }.value
            if quedanPosteriores(chat: chat) {
                // Se veia una pagina del medio (un salto): a la ultima.
                await abrir(chat: chat)
                NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
            } else {
                anadir([m], en: chat)
            }
            return m
        } catch {
            self.error = "\(error)"
            return nil
        }
    }

    /// Crea un proyecto (un chat nuevo).
    public func crearChat(nombre: String) async {
        let nucleo = self.nucleo
        do {
            _ = try await Task.detached(priority: .userInitiated) { try nucleo.crearChat(nombre: nombre) }.value
            NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
            await cargarChats()
        } catch {
            self.error = "\(error)"
        }
    }

    /// Un lienzo nuevo en el chat (su `.excalidraw` vacio y su mensaje
    /// `DIBUJO`, como en Windows), anadido al final de lo cargado.
    @discardableResult
    public func crearLienzo(en chat: String) async -> MensajeChat? {
        let nucleo = self.nucleo
        do {
            let m = try await Task.detached(priority: .userInitiated) {
                try nucleo.crearLienzo(chat: chat, nombre: nil)
            }.value
            anadir([m], en: chat)
            return m
        } catch {
            self.error = "\(error)"
            return nil
        }
    }

    // MARK: Ficheros en el chat

    /// Adjunta ficheros (soltados o pegados) a un chat: cada uno se COPIA
    /// dentro del proyecto con su mensaje (`IMAGEN`, `VOZ` o `ARCHIVO`, como
    /// en Windows) y se anade al final de lo cargado. La copia va fuera del
    /// hilo principal. Uno que falle no se lleva los demas: se dice en
    /// `error` y siguen. Las carpetas no se adjuntan.
    @discardableResult
    public func adjuntar(urls: [URL], en chat: String) async -> [MensajeChat] {
        let origenes = urls.compactMap { u -> String? in
            // Un `file:///.file/id=...` (referencia) pasa a ruta de verdad.
            let url = (u as NSURL).filePathURL ?? u
            return url.isFileURL ? url.path : nil
        }
        guard !origenes.isEmpty else { return [] }
        let nucleo = self.nucleo
        let (hechos, fallo) = await Task.detached(priority: .userInitiated) { () -> ([MensajeChat], String?) in
            var hechos: [MensajeChat] = []
            var fallo: String?
            for ruta in origenes {
                do {
                    hechos.append(try nucleo.adjuntar(chat: chat, rutaOrigen: ruta, nombre: nil))
                } catch {
                    fallo = "\((ruta as NSString).lastPathComponent): \(error)"
                }
            }
            return (hechos, fallo)
        }.value
        anadir(hechos, en: chat)
        error = fallo
        // Los PDF se aligeran despues, en segundo plano (si esta encendido).
        for m in hechos where (m.nombre as NSString).pathExtension.lowercased() == "pdf" {
            if let r = m.ruta, let u = await url(deRuta: r, en: chat) {
                ColaAligerar.compartida.alEntrar(u, nucleo: nucleo)
            }
        }
        return hechos
    }

    /// Adjunta una imagen que no es un fichero (pegada o soltada desde otra
    /// app). Se llama `pegado-<hora>.<ext>` como en Windows; un TIFF (lo
    /// que suele dejar el portapapeles del Mac) se guarda como PNG, que es
    /// lo que entienden Windows y Android. Todo fuera del hilo principal.
    @discardableResult
    public func adjuntar(datos: Data, formato: String, en chat: String) async -> MensajeChat? {
        let nucleo = self.nucleo
        let resultado = await Task.detached(priority: .userInitiated) { () -> Result<MensajeChat, Error> in
            var bytes = datos
            var ext = formato.lowercased()
            if ext == "tif" || ext == "tiff" || ext.isEmpty,
               let png = NSBitmapImageRep(data: bytes)?.representation(using: .png, properties: [:]) {
                bytes = png
                ext = "png"
            }
            let cuando = Int64(Date().timeIntervalSince1970 * 1000)
            let carpeta = FileManager.default.temporaryDirectory
                .appendingPathComponent("pixpin-pegado-\(UUID().uuidString)", isDirectory: true)
            defer { try? FileManager.default.removeItem(at: carpeta) }
            do {
                try FileManager.default.createDirectory(at: carpeta, withIntermediateDirectories: true)
                let fichero = carpeta.appendingPathComponent("pegado-\(cuando).\(ext)")
                try bytes.write(to: fichero)
                return .success(try nucleo.adjuntar(chat: chat, rutaOrigen: fichero.path, nombre: nil))
            } catch {
                return .failure(error)
            }
        }.value
        switch resultado {
        case let .success(m):
            anadir([m], en: chat)
            error = nil
            return m
        case let .failure(e):
            error = "\(e)"
            return nil
        }
    }

    /// Un mensaje que cambio (el texto de una nota de voz) en lo cargado.
    func sustituir(_ m: MensajeChat, en chat: String) {
        guard let i = mensajes[chat]?.firstIndex(where: { $0.id == m.id }) else { return }
        mensajes[chat]?[i] = m
        NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
    }

    /// Para las extensiones de otros ficheros (voz): dice o borra el error.
    func ponerError(_ texto: String?) {
        error = texto
    }

    func anadir(_ nuevos: [MensajeChat], en chat: String) {
        guard !nuevos.isEmpty else { return }
        if quedanPosteriores(chat: chat) {
            // Se ve una pagina del medio (un salto): lo nuevo va al final
            // del chat, asi que se vuelve a la ultima pagina.
            Task { await abrir(chat: chat) }
        } else {
            mensajes[chat, default: []].append(contentsOf: nuevos)
            totales[chat, default: 0] += UInt64(nuevos.count)
        }
        // La sincronizacion sola cuenta sus 30 s de calma desde aqui.
        NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
    }

    // MARK: Saltar a un mensaje

    private func nuevoTurno(_ chat: String) -> Int {
        let t = (turnos[chat] ?? 0) &+ 1
        turnos[chat] = t
        return t
    }

    /// Si quedan mensajes mas nuevos que los cargados (tras un salto).
    public func quedanPosteriores(chat: String) -> Bool {
        guard let inicio = desde[chat], let total = totales[chat] else { return false }
        return inicio + UInt64(mensajes[chat]?.count ?? 0) < total
    }

    /// **Carga la pagina que contiene el mensaje de `posicion`** (0 el mas
    /// antiguo), con el en medio, sustituyendo lo cargado. Dos cruces del
    /// puente: el total (la ultima pagina de un mensaje, del indice en
    /// cache) y la pagina.
    public func abrir(chat: String, en posicion: UInt64) async {
        let nucleo = self.nucleo
        if let ficha = chats.first(where: { $0.id == chat }) {
            nucleo.ponerAlFrente(frente: .chat(id: ficha.id, nombre: ficha.nombre))
        }
        let turno = nuevoTurno(chat)
        do {
            let pagina = try await Task.detached(priority: .userInitiated) { () throws -> PaginaMensajes in
                let total = try nucleo.ultimosMensajes(chat: chat, cuantos: 1).total
                let v = ventanaDeSalto(posicion: posicion, total: total, tam: UInt64(Almacen.tamPagina))
                return try nucleo.mensajes(chat: chat, desde: v.desde, cuantos: v.cuantos)
            }.value
            guard turno == turnos[chat] else { return }
            mensajes[chat] = pagina.mensajes
            totales[chat] = pagina.total
            desde[chat] = pagina.desde
        } catch {
            self.error = "\(error)"
        }
    }

    /// La pagina siguiente a lo cargado (al bajar tras un salto).
    public func cargarPosteriores(chat: String) async {
        guard quedanPosteriores(chat: chat), !cargandoPosteriores.contains(chat),
              let inicio = desde[chat], let total = totales[chat] else { return }
        cargandoPosteriores.insert(chat)
        defer { cargandoPosteriores.remove(chat) }
        let hasta = inicio + UInt64(mensajes[chat]?.count ?? 0)
        let cuantos = UInt32(min(UInt64(Almacen.tamPagina), total - hasta))
        let turno = turnos[chat]
        let nucleo = self.nucleo
        do {
            let pagina = try await Task.detached(priority: .userInitiated) {
                try nucleo.mensajes(chat: chat, desde: hasta, cuantos: cuantos)
            }.value
            // Solo si lo cargado sigue siendo lo mismo.
            guard turno == turnos[chat], desde[chat] == inicio,
                  inicio + UInt64(mensajes[chat]?.count ?? 0) == hasta else { return }
            mensajes[chat, default: []].append(contentsOf: pagina.mensajes)
            totales[chat] = pagina.total
        } catch {
            self.error = "\(error)"
        }
    }

    /// El fichero de una `ruta` de mensaje (o de una foto de un lienzo) si
    /// ya se resolvio antes. No cruza el puente.
    public func urlResuelta(deRuta ruta: String, en chat: String) -> URL? {
        rutas[chat + "\u{0}" + ruta]
    }

    /// El fichero de una `ruta` de mensaje en este Mac (`Nucleo.rutaAbsoluta`,
    /// fuera del hilo principal la primera vez). `nil` si no se puede
    /// resolver aqui (una ruta de otro aparato). No mira si existe.
    public func url(deRuta ruta: String, en chat: String) async -> URL? {
        let clave = chat + "\u{0}" + ruta
        if let hecha = rutas[clave] { return hecha }
        let nucleo = self.nucleo
        let absoluta = await Task.detached(priority: .userInitiated) {
            try? nucleo.rutaAbsoluta(chat: chat, rutaRelativa: ruta)
        }.value
        guard let absoluta else { return nil }
        let url = URL(fileURLWithPath: absoluta)
        rutas[clave] = url
        return url
    }

    /// Un dibujo del chat (la `referencia` de un `DIBUJO`) listo para pintar
    /// y sus fotos con su fichero: leer el JSON, el bufer y las rutas va
    /// todo junto fuera del hilo principal, en tres cruces del puente.
    public func dibujo(_ referencia: String, en chat: String) async -> DibujoDelChat? {
        let nucleo = self.nucleo
        let hecho = await Task.detached(priority: .userInitiated) { () -> Result<DibujoDelChat, Error> in
            do {
                let lienzo = try nucleo.abrirLienzo(chat: chat, dibujo: referencia)
                let leido = try leerLienzo(lienzo.plano())
                var fotos: [UInt64: URL] = [:]
                for f in try nucleo.imagenesDelLienzo(chat: chat, lienzo: lienzo) {
                    fotos[f.idObjeto] = URL(fileURLWithPath: f.ruta)
                }
                return .success(DibujoDelChat(lienzo: leido, fotos: fotos))
            } catch {
                return .failure(error)
            }
        }.value
        switch hecho {
        case let .success(d): return d
        case .failure: return nil
        }
    }

    /// Suelta lo que no se ve: deja solo la ultima pagina del chat
    /// `visible` y vacia las caches del nucleo.
    public func aligerar(visible: String?) {
        for (chat, lista) in mensajes where chat != visible {
            _ = lista
            mensajes[chat] = nil
            desde[chat] = nil
            turnos[chat] = nil
        }
        rutas = [:]
        nucleo.vaciarCaches()
    }

    /// El aviso del sistema de que falta memoria: se aligera sin esperar.
    private func vigilarMemoria() {
        let fuente = DispatchSource.makeMemoryPressureSource(eventMask: [.warning, .critical], queue: .main)
        fuente.setEventHandler { [weak self] in
            MainActor.assumeIsolated { self?.aligerar(visible: nil) }
            Miniaturas.compartidas.vaciar()
        }
        fuente.resume()
        vigiaMemoria = fuente
    }
}

/// Un dibujo del chat leido: el lienzo y el fichero de cada foto por su
/// `id_objeto` (el de las ordenes del bufer).
public struct DibujoDelChat {
    public var lienzo: LienzoLeido
    public var fotos: [UInt64: URL]
}
