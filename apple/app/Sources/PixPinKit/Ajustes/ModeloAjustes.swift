// Lo que se guarda de los Ajustes, cada cosa en su sitio:
//
// - En `pixpinmax.toml` (el MISMO fichero de Windows, por el nucleo de Rust
//   `Nucleo.ajustes`/`guardarAjustes`), solo lo que vale en los dos
//   sistemas: los dias que aguanta una captura y la sincronizacion sola.
// - En este Mac: el tema (UserDefaults), el atajo (`AtajoDeCaptura`) y
//   «Abrir al iniciar sesion» (`SMAppService.mainApp`, que es la verdad: si
//   el usuario lo quita en Ajustes del Sistema, aqui se ve quitado).
//
// Leer y escribir el TOML va fuera del hilo principal (§4bis.3).

import AppKit
import Foundation
import PixPinCore
import ServiceManagement

/// El aspecto de la app. De fabrica sigue al sistema.
public enum Tema: String, CaseIterable, Identifiable, Sendable {
    case sistema
    case claro
    case oscuro

    public var id: String { rawValue }

    public var nombre: String {
        switch self {
        case .sistema: return "Como el sistema"
        case .claro: return "Claro"
        case .oscuro: return "Oscuro"
        }
    }

    static let clave = "tema"

    public static func guardado(_ d: UserDefaults = .standard) -> Tema {
        d.string(forKey: clave).flatMap(Tema.init(rawValue:)) ?? .sistema
    }

    public func guardar(_ d: UserDefaults = .standard) {
        if self == .sistema { d.removeObject(forKey: Tema.clave) } else { d.set(rawValue, forKey: Tema.clave) }
    }

    /// `nil` sigue al sistema (y cambia solo con el).
    public var apariencia: NSAppearance? {
        switch self {
        case .sistema: return nil
        case .claro: return NSAppearance(named: .aqua)
        case .oscuro: return NSAppearance(named: .darkAqua)
        }
    }

    @MainActor
    public func aplicar() {
        NSApplication.shared.appearance = apariencia
    }
}

@MainActor
public final class ModeloAjustes: ObservableObject {
    /// Dias que aguanta una captura antes de irse sola (0 = nunca).
    @Published public private(set) var diasCaducidad: Int = 7
    @Published public private(set) var sincroAutomatica = true
    @Published public private(set) var abrirAlIniciar = false
    /// «Abrir al iniciar sesion» espera el visto bueno en Ajustes del Sistema.
    @Published public private(set) var esperaAprobacion = false
    @Published public var tema: Tema {
        didSet {
            tema.guardar(defaults)
            tema.aplicar()
        }
    }
    @Published public private(set) var error: String?
    @Published public private(set) var cargado = false

    public let nucleo: Nucleo
    /// La sincronizacion de la app, para encender o apagar la automatica.
    public weak var sincro: Sincronizador?
    private let defaults: UserDefaults
    /// Lo leido del TOML (para no pisar `presencia`, que aqui no se toca).
    private var leidos: AjustesMac?

    public init(nucleo: Nucleo, sincro: Sincronizador? = nil, defaults: UserDefaults = .standard) {
        self.nucleo = nucleo
        self.sincro = sincro
        self.defaults = defaults
        tema = Tema.guardado(defaults)
        leerAbrirAlIniciar()
    }

    /// La carpeta de datos (la de `proyectos/`, `sincro/`, `pixpinmax.toml`).
    public var carpetaDatos: URL { URL(fileURLWithPath: nucleo.raiz(), isDirectory: true) }

    /// `pixpinmax.toml`.
    public var ficheroAjustes: String { nucleo.rutaAjustes() }

    /// Lee el TOML y pasa la sincronizacion sola al `Sincronizador`.
    public func cargar() async {
        let nucleo = self.nucleo
        let r = await Task.detached(priority: .utility) { () -> Result<AjustesMac, Error> in
            do { return .success(try nucleo.ajustes()) } catch { return .failure(error) }
        }.value
        switch r {
        case let .success(a):
            leidos = a
            diasCaducidad = Int(a.diasCaducidad)
            sincroAutomatica = a.sincroAutomatica
            sincro?.automatica = a.sincroAutomatica
            error = nil
        case let .failure(e):
            error = "No se pudo leer pixpinmax.toml: \(ModeloAjustes.texto(de: e))"
        }
        leerAbrirAlIniciar()
        cargado = true
    }

    public func fijar(diasCaducidad dias: Int) {
        let d = max(0, min(400, dias))
        guard d != diasCaducidad else { return }
        diasCaducidad = d
        programarGuardado()
    }

    public func fijar(sincroAutomatica s: Bool) {
        guard s != sincroAutomatica else { return }
        sincroAutomatica = s
        sincro?.automatica = s
        programarGuardado()
    }

    /// El ultimo guardado en marcha: los guardados van **uno detras de
    /// otro** (dos escrituras a la vez de `pixpinmax.toml` se pisaban, y
    /// quien leyera en medio veia el fichero a medias).
    private var ultimoGuardado: Task<Void, Never>?

    /// Pone un guardado en la cola, ya mismo (en orden con los cambios).
    @discardableResult
    private func programarGuardado() -> Task<Void, Never> {
        let previo = ultimoGuardado
        let t = Task { [weak self] in
            await previo?.value
            await self?.guardarAhora()
        }
        ultimoGuardado = t
        return t
    }

    /// Escribe lo de este modelo en `pixpinmax.toml`, conservando lo demas
    /// (despues de los guardados que ya esperaban).
    public func guardar() async {
        await programarGuardado().value
    }

    private func guardarAhora() async {
        let nuevos = AjustesMac(diasCaducidad: UInt32(max(0, diasCaducidad)),
                                sincroAutomatica: sincroAutomatica,
                                sincroPresencia: leidos?.sincroPresencia ?? true)
        let nucleo = self.nucleo
        let fallo = await Task.detached(priority: .utility) { () -> String? in
            do {
                try nucleo.guardarAjustes(nuevos: nuevos)
                return nil
            } catch {
                return ModeloAjustes.texto(de: error)
            }
        }.value
        if let fallo {
            error = "No se pudo guardar: \(fallo)"
        } else {
            leidos = nuevos
            error = nil
        }
    }

    // MARK: Abrir al iniciar sesion

    private func leerAbrirAlIniciar() {
        let estado = SMAppService.mainApp.status
        abrirAlIniciar = estado == .enabled
        esperaAprobacion = estado == .requiresApproval
    }

    public func fijar(abrirAlIniciar quiere: Bool) {
        do {
            if quiere {
                try SMAppService.mainApp.register()
            } else {
                try SMAppService.mainApp.unregister()
            }
            error = nil
        } catch {
            self.error = "No se pudo cambiar «Abrir al iniciar sesión»: \(error.localizedDescription)"
        }
        leerAbrirAlIniciar()
    }

    /// Abre Ajustes del Sistema > General > Elementos de inicio.
    public func abrirElementosDeInicio() {
        SMAppService.openSystemSettingsLoginItems()
    }

    public func mostrarCarpetaEnFinder() {
        NSWorkspace.shared.activateFileViewerSelecting([carpetaDatos])
    }

    nonisolated static func texto(de error: Error) -> String {
        if let e = error as? ErrorNucleo {
            switch e {
            case let .Disco(d), let .Lienzo(d), let .Sincro(d), let .Argumento(d), let .Ajustes(d), let .Asistente(d):
                return d
            case let .NoExiste(que):
                return "no existe: \(que)"
            }
        }
        return error.localizedDescription
    }
}
