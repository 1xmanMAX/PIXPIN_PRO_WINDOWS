// El estado de la app: el nucleo de Rust abierto sobre la carpeta de datos,
// la lista de chats y las paginas de mensajes que se ven.
//
// Rendimiento (§4bis del plan):
// - todo lo que toca el disco va FUERA del hilo principal (`Task.detached`
//   con prioridad `.utility`), y solo el resultado vuelve al principal;
// - los mensajes se piden por paginas (`pagina`), nunca el chat entero;
// - ante presion de memoria se sueltan las paginas que no se ven y se
//   vacian las caches del nucleo.

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
    public static let tamPagina: UInt32 = 60

    @Published public private(set) var chats: [FichaChat] = []
    @Published public private(set) var error: String?
    /// Los mensajes cargados de cada chat abierto (solo los de las paginas
    /// pedidas), del mas antiguo al mas nuevo.
    @Published public private(set) var mensajes: [String: [MensajeChat]] = [:]
    /// Cuantos tiene cada chat en total (para saber si quedan mas antiguos).
    @Published public private(set) var totales: [String: UInt64] = [:]

    public let nucleo: Nucleo
    private var desde: [String: UInt64] = [:]
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
        do {
            let pagina = try await Task.detached(priority: .userInitiated) {
                try nucleo.ultimosMensajes(chat: chat, cuantos: Almacen.tamPagina)
            }.value
            mensajes[chat] = pagina.mensajes
            totales[chat] = pagina.total
            desde[chat] = pagina.desde
        } catch {
            self.error = "\(error)"
        }
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
            mensajes[chat, default: []].append(m)
            totales[chat, default: 0] += 1
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
            await cargarChats()
        } catch {
            self.error = "\(error)"
        }
    }

    /// Suelta lo que no se ve: deja solo la ultima pagina del chat
    /// `visible` y vacia las caches del nucleo.
    public func aligerar(visible: String?) {
        for (chat, lista) in mensajes where chat != visible {
            _ = lista
            mensajes[chat] = nil
            desde[chat] = nil
        }
        nucleo.vaciarCaches()
    }

    /// El aviso del sistema de que falta memoria: se aligera sin esperar.
    private func vigilarMemoria() {
        let fuente = DispatchSource.makeMemoryPressureSource(eventMask: [.warning, .critical], queue: .main)
        fuente.setEventHandler { [weak self] in
            MainActor.assumeIsolated { self?.aligerar(visible: nil) }
        }
        fuente.resume()
        vigiaMemoria = fuente
    }
}
