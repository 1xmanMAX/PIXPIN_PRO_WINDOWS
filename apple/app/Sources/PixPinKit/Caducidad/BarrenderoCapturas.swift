// Las capturas caducadas, a la Papelera del Mac: lo que hace en Windows
// `caducidad_capturas.rs` (barre al arrancar y luego cada hora, y lo
// caducado va a la papelera de reciclaje, de donde se puede restaurar).
//
// Lo que se va lo decide el nucleo de Rust con las reglas de siempre
// (`Nucleo.capturasCaducadas`: conservadas, fecha acordada con el grupo,
// el plazo del ajuste y «7 dias mas»); Swift solo las mueve con
// `FileManager.trashItem` y luego le pide al nucleo que olvide las
// conservadas y prorrogadas que ya no estan (`olvidarCapturasAusentes`).
//
// Rendimiento (§4bis.6 del plan): nada de temporizadores propios. Un solo
// `NSBackgroundActivityScheduler` con mucha tolerancia: el sistema elige
// el momento (con el Mac despierto, sin bateria baja, junto a otros
// trabajos) y lo corre con prioridad `.utility`. Cada barrido es una
// lectura de `capturas/` (de la cache si no cambio) y del registro.

import Foundation
import os
import PixPinCore

public final class BarrenderoCapturas: @unchecked Sendable {
    /// El id de la actividad para el sistema (uno por app).
    public static let identificador = "io.github.1xmanmax.pixpin.caducidad-capturas"
    /// Cada cuanto, como mucho (Windows: `CADA`, una hora).
    public static let cada: TimeInterval = 60 * 60
    /// Cuanto puede retrasarlo el sistema para juntarlo con otros trabajos.
    public static let tolerancia: TimeInterval = 20 * 60
    /// La espera tras arrancar, para no competir con el arranque.
    public static let esperaAlArrancar: TimeInterval = 15

    /// Lo que hizo un barrido.
    public struct Barrido: Equatable, Sendable {
        /// Los nombres de las que se fueron a la Papelera.
        public var idas: [String] = []
        /// Las que no se pudieron mover (con el motivo).
        public var fallos: [String] = []
        /// Cuantos nombres se olvidaron del registro.
        public var olvidadas: UInt32 = 0
    }

    let nucleo: Nucleo
    /// Los dias del ajuste (`[capturas] dias_caducidad` de `pixpinmax.toml`,
    /// el que cambia Ajustes y vale tambien en Windows). Si el fichero no se
    /// entiende, cero: mejor no borrar nada que borrar con un plazo inventado.
    var dias: @Sendable () -> Int64
    /// Llevar un fichero a la Papelera (las pruebas lo cambian).
    var tirar: @Sendable (URL) throws -> Void = { url in
        try FileManager.default.trashItem(at: url, resultingItemURL: nil)
    }

    private let cerrojo = NSLock()
    private var programador: NSBackgroundActivityScheduler?
    private static let registro = Logger(subsystem: "io.github.1xmanmax.pixpin", category: "caducidad")

    public init(nucleo: Nucleo) {
        self.nucleo = nucleo
        dias = { Int64((try? nucleo.ajustes().diasCaducidad) ?? 0) }
    }

    deinit {
        programador?.invalidate()
    }

    /// **Barre una vez**: lo caducado a la Papelera. Bloqueante (lee el
    /// disco y mueve ficheros): fuera del hilo principal. Dos barridos a la
    /// vez no se pisan.
    @discardableResult
    public func barrer() -> Barrido {
        cerrojo.lock()
        defer { cerrojo.unlock() }
        var hecho = Barrido()
        let plazo = dias()
        if plazo > 0 {
            for c in nucleo.capturasCaducadas(dias: plazo) {
                do {
                    try tirar(URL(fileURLWithPath: c.ruta))
                    hecho.idas.append(c.nombre)
                } catch {
                    hecho.fallos.append("\(c.nombre): \(error.localizedDescription)")
                }
            }
        }
        do {
            hecho.olvidadas = try nucleo.olvidarCapturasAusentes()
        } catch {
            BarrenderoCapturas.registro.error("no se pudo limpiar el registro de caducidad: \(String(describing: error), privacy: .public)")
        }
        if !hecho.idas.isEmpty {
            BarrenderoCapturas.registro.info("capturas caducadas a la Papelera: \(hecho.idas.count)")
        }
        for f in hecho.fallos {
            BarrenderoCapturas.registro.error("no se pudo llevar a la Papelera: \(f, privacy: .public)")
        }
        return hecho
    }

    /// **Empieza**: un barrido poco despues de arrancar y luego como mucho
    /// cada hora, cuando el sistema vea bien. Llamarlo otra vez no hace nada.
    public func arrancar() {
        guard programador == nil else { return }
        DispatchQueue.global(qos: .utility).asyncAfter(deadline: .now() + BarrenderoCapturas.esperaAlArrancar) { [weak self] in
            self?.barrer()
        }
        let p = NSBackgroundActivityScheduler(identifier: BarrenderoCapturas.identificador)
        p.repeats = true
        p.interval = BarrenderoCapturas.cada
        p.tolerance = BarrenderoCapturas.tolerancia
        p.qualityOfService = .utility
        p.schedule { [weak self, weak p] completar in
            guard let self else {
                completar(.finished)
                return
            }
            if p?.shouldDefer == true {
                completar(.deferred)
                return
            }
            self.barrer()
            completar(.finished)
        }
        programador = p
    }

    /// Deja de barrer (al salir).
    public func parar() {
        programador?.invalidate()
        programador = nil
    }
}
