// Quien escucha la respuesta del `llama-server` mientras llega: el
// `OyenteIa` del nucleo de Rust (un interfaz que implementa Swift). Rust lo
// llama desde el hilo del turno, en tandas (cada ~60 ms como mucho); aqui
// solo se pasa cada tanda a quien la pinta, que la lleva al hilo principal.

import Foundation
import PixPinCore

final class OyenteDelAsistente: OyenteIa, @unchecked Sendable {
    let alTexto: @Sendable (String) -> Void
    let alHerramienta: @Sendable (HerramientaUsada) -> Void

    init(alTexto: @escaping @Sendable (String) -> Void,
         alHerramienta: @escaping @Sendable (HerramientaUsada) -> Void = { _ in }) {
        self.alTexto = alTexto
        self.alHerramienta = alHerramienta
    }

    func texto(trozo: String) {
        alTexto(trozo)
    }

    func herramienta(usada: HerramientaUsada) {
        alHerramienta(usada)
    }
}
