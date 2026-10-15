// «Enviar a…»: mandar archivos a lo que otro aparato del grupo tenga
// delante (su chat abierto, su lienzo si son fotos, o la conversacion
// general), como el `suelto` de Windows y Android
// (`pixpin_sincro::al_lienzo`). Va por el canal del grupo y el puerto de
// siempre: sin codigos. Y lo contrario: lo que llega aqui va a lo que este
// Mac tenga delante (`Nucleo.ponerAlFrente`).
//
// Aqui lo puro (los textos y el estado, probados en `EnvioTests`) y el menu
// de la barra con los aparatos del grupo y como va cada uno.

import AppKit
import PixPinCore
import SwiftUI

/// Como va «Enviar a…» con un aparato.
public enum EstadoEnvioSuelto: Equatable, Sendable {
    /// Mandando `n` archivos.
    case enviando(Int)
    /// Hecho: la frase de donde quedo.
    case hecho(String)
    case fallo(String)

    public var texto: String {
        switch self {
        case let .enviando(n): n == 1 ? "Enviando…" : "Enviando \(n) archivos…"
        case let .hecho(t): t
        case let .fallo(t): t
        }
    }
}

/// Donde quedo un archivo en el otro aparato, en palabras.
public func textoDeDonde(_ donde: DondeLlego, chat: String?) -> String {
    switch donde {
    case .lienzo: "en su lienzo"
    case .chatAbierto: chat.map { "en «\($0)»" } ?? "en su chat abierto"
    case .chat: "en Mensajes guardados"
    }
}

/// La frase de un «Enviar a…» terminado: donde quedo lo que llego y por que
/// no llego lo demas.
public func estadoDeEnvio(_ r: [EnvioSuelto], a nombre: String) -> EstadoEnvioSuelto {
    let bien = r.filter { $0.donde != nil }
    let mal = r.filter { $0.donde == nil }
    if bien.isEmpty {
        return .fallo(mal.first?.error ?? "No se envió nada a \(nombre).")
    }
    var partes: [String] = []
    if bien.count == 1, let uno = bien.first, let donde = uno.donde {
        partes.append("«\(uno.nombre)» llegó a \(nombre), \(textoDeDonde(donde, chat: uno.chat)).")
    } else if let donde = bien.first?.donde {
        partes.append("\(bien.count) archivos llegaron a \(nombre), \(textoDeDonde(donde, chat: bien.first?.chat)).")
    }
    for m in mal {
        partes.append("«\(m.nombre)»: \(m.error ?? "no se envió").")
    }
    return .hecho(partes.joined(separator: "\n"))
}

/// La frase de lo que llego suelto a este Mac.
public func textoDeLlegadas(_ l: [ArchivoLlegado], de nombre: String) -> String {
    guard let primero = l.first else { return "" }
    let donde: String
    switch primero.donde {
    case .lienzo: donde = "al lienzo"
    case .chatAbierto: donde = "al chat abierto"
    case .chat: donde = "a Mensajes guardados"
    }
    if l.count == 1 {
        return "\(nombre) envió «\(primero.nombre)» \(donde)."
    }
    return "\(nombre) envió \(l.count) archivos \(donde)."
}

/// **El menu «Enviar a…»** de la barra de menus: los aparatos del grupo que
/// hay en la Wi-Fi; al elegir uno se eligen los archivos y se mandan. Debajo
/// de cada uno, como fue lo ultimo.
public struct MenuEnviarA: View {
    @ObservedObject var sincro: Sincronizador

    public init(sincro: Sincronizador) {
        _sincro = ObservedObject(wrappedValue: sincro)
    }

    public var body: some View {
        Menu("Enviar a…") {
            if sincro.filas.isEmpty {
                Text(sincro.codigoLegible == nil ? "Sin grupo: únete en Sincronizar"
                    : "Ningún aparato del grupo en esta Wi-Fi")
            }
            ForEach(sincro.filas) { fila in
                Button(etiqueta(fila)) { elegirYEnviar(a: fila.id) }
                    .disabled(fila.direccion == nil && fila.estado == .noContesta)
            }
        }
    }

    private func etiqueta(_ fila: Sincronizador.Fila) -> String {
        guard let e = sincro.envios[fila.id] else { return fila.nombre }
        let linea = e.texto.split(separator: "\n").first.map(String.init) ?? ""
        return "\(fila.nombre) — \(linea)"
    }

    private func elegirYEnviar(a id: String) {
        let panel = NSOpenPanel()
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = true
        panel.prompt = "Enviar"
        NSApp.activate()
        guard panel.runModal() == .OK else { return }
        let urls = panel.urls
        Task { await sincro.enviar(urls, a: id) }
    }
}
