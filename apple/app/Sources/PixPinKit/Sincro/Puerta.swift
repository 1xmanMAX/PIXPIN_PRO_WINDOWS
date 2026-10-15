// La puerta por la que los demas aparatos llaman a este Mac: la `Escucha` de
// Rust (un `TcpListener`) atendida en un hilo propio.
//
// Por que un `Thread` y no una `Task`: `Escucha.atender()` bloquea hasta que
// llega alguien (sin gastar nada: es un `accept`), y una `Task` bloqueada
// ocuparia uno de los pocos hilos del pool cooperativo de Swift todo el rato
// que la app vive. Un hilo dormido en `accept` no cuesta CPU ni energia.
//
// El puerto es el de siempre (47474, `puertoPreferido()`) o, si esta tomado,
// el que de el sistema, como Windows y Android: se anuncia el que salga.

import Foundation
import PixPinCore

public final class Puerta: @unchecked Sendable {
    public let puerto: UInt16
    private let escucha: Escucha

    /// Abre la escucha en 47474 o, si no se puede, en cualquiera.
    public init(nucleo: Nucleo) throws {
        let e: Escucha
        do {
            e = try nucleo.escuchar(puerto: puertoPreferido())
        } catch {
            e = try nucleo.escuchar(puerto: 0)
        }
        escucha = e
        puerto = e.puerto()
    }

    /// Arranca el hilo que atiende a quien llame, uno detras de otro.
    /// `alAtender` se llama en ese hilo con cada conexion atendida (o su
    /// error); el hilo termina con `cerrar()`.
    public func arrancar(alAtender: @escaping @Sendable (Result<Atendido, Error>) -> Void) {
        let escucha = self.escucha
        let hilo = Thread {
            var fallosSeguidos = 0
            while true {
                do {
                    let atendido = try escucha.atender()
                    if atendido == .cerrada { return }
                    fallosSeguidos = 0
                    alAtender(.success(atendido))
                } catch {
                    // Un aparato con otro codigo o una red que se corta es
                    // un fallo de esa conexion: se sigue escuchando. Si
                    // fallan muchas seguidas (sin descriptores...), se
                    // espera un poco para no girar en vacio.
                    fallosSeguidos += 1
                    alAtender(.failure(error))
                    if fallosSeguidos >= 3 {
                        Thread.sleep(forTimeInterval: min(30, Double(fallosSeguidos)))
                    }
                }
            }
        }
        hilo.name = "pixpin-sincro-escucha"
        hilo.qualityOfService = .utility
        hilo.start()
    }

    /// Cierra la puerta: el hilo sale de su espera y termina.
    public func cerrar() {
        escucha.cerrar()
    }
}
