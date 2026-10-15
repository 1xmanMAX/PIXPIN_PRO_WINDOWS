// Las vistas de las notas de voz: la burbuja `VOZ` (reproducir, la onda con
// su avance, la duracion y lo que se dijo) y el boton del microfono de la
// caja de escribir.

import AppKit
import PixPinCore
import SwiftUI

struct BurbujaVoz: View {
    let mensaje: MensajeChat
    let chat: String
    let almacen: Almacen
    @ObservedObject private var reproductor = ReproductorVoz.compartido
    @State private var url: URL?
    @State private var transcribiendo = false

    private var esLaActual: Bool { reproductor.actual == mensaje.id }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 10) {
                Button(action: alternar) {
                    Image(systemName: esLaActual && reproductor.sonando ? "pause.circle.fill" : "play.circle.fill")
                        .font(.system(size: 28))
                }
                .buttonStyle(.plain)
                .disabled(url == nil)
                .help(esLaActual && reproductor.sonando ? "Pausa" : "Escuchar")

                Group {
                    if esLaActual && reproductor.sonando {
                        // Solo la que suena se repinta mientras suena.
                        TimelineView(.animation(minimumInterval: 0.1, paused: false)) { _ in
                            onda(avance: reproductor.fraccion)
                        }
                    } else {
                        onda(avance: esLaActual ? reproductor.fraccion : 0)
                    }
                }
                .frame(width: 180, height: 28)

                Text(FormatoVoz.marca(ms: mensaje.duracionMs))
                    .font(.caption.monospacedDigit()).foregroundStyle(.secondary)
            }
            texto
        }
        .frame(maxWidth: 360, alignment: .leading)
        .contextMenu {
            Button("Pasar a texto") { transcribir() }
                .disabled(transcribiendo)
            if let url {
                Button("Mostrar en el Finder") { NSWorkspace.shared.activateFileViewerSelecting([url]) }
            }
        }
        .task(id: mensaje.id) {
            guard let ruta = mensaje.ruta else { return }
            if let u = await almacen.url(deRuta: ruta, en: chat), FileManager.default.fileExists(atPath: u.path) {
                url = u
            }
        }
    }

    /// Lo que se dijo, en parrafos y sin los `[m:ss]` dentro del texto.
    @ViewBuilder private var texto: some View {
        let trozos = FormatoVoz.trozos(mensaje.transcripcion ?? "")
        if !trozos.isEmpty {
            Text(trozos.map { $0.texto }.joined(separator: "\n\n"))
                .textSelection(.enabled)
        } else if transcribiendo {
            ProgressView().controlSize(.small)
        } else if !mensaje.texto.isEmpty {
            Text(mensaje.texto).textSelection(.enabled)
        }
    }

    /// La onda de los picos (normalizada contra el maximo de la nota, como
    /// `barras_de_onda` de Windows) con lo ya escuchado en color.
    private func onda(avance: Double) -> some View {
        let picos = mensaje.picos
        return Canvas { ctx, tam in
            let barras = max(min(picos.count, Int(tam.width / 3)), 1)
            let maximo = max(Double(picos.max() ?? 0), 1)
            let ancho = tam.width / CGFloat(barras)
            for i in 0..<barras {
                let valor: Double
                if picos.isEmpty {
                    valor = 0.15
                } else {
                    // Cada barra, el maximo de su tramo de picos.
                    let a = i * picos.count / barras
                    let b = max((i + 1) * picos.count / barras, a + 1)
                    valor = Double(picos[a..<min(b, picos.count)].max() ?? 0) / maximo
                }
                let alto = max(CGFloat(valor) * tam.height, 2)
                let rect = CGRect(x: CGFloat(i) * ancho, y: (tam.height - alto) / 2,
                                  width: max(ancho - 1, 1), height: alto)
                let hecho = (Double(i) + 0.5) / Double(barras) <= avance
                ctx.fill(Path(roundedRect: rect, cornerRadius: 1),
                         with: .color(hecho ? Color.accentColor : Color.secondary.opacity(0.5)))
            }
        }
        .contentShape(Rectangle())
        .gesture(SpatialTapGesture().onEnded { toque in
            guard esLaActual else { return }
            reproductor.saltar(mensaje.id, a: Double(toque.location.x / 180))
        })
    }

    private func alternar() {
        guard let url else { return NSSound.beep() }
        reproductor.alternar(mensaje.id, url: url)
    }

    private func transcribir() {
        guard !transcribiendo else { return }
        transcribiendo = true
        Task {
            await almacen.transcribir(mensaje, en: chat)
            transcribiendo = false
        }
    }
}

/// El boton del microfono de la caja de escribir: un toque graba, otro para
/// y la nota va a este chat. Mientras graba, rojo y con el tiempo.
struct BotonVoz: View {
    let chat: String
    let almacen: Almacen
    @ObservedObject private var grabadora = GrabadoraVoz.compartida

    var body: some View {
        HStack(spacing: 4) {
            if case let .grabando(c, desde) = grabadora.estado, c == chat {
                TimelineView(.periodic(from: desde, by: 1)) { contexto in
                    Text(FormatoVoz.marca(ms: Int64(contexto.date.timeIntervalSince(desde) * 1000)))
                        .font(.caption.monospacedDigit()).foregroundStyle(.red)
                }
            } else if grabadora.estado == .guardando || grabadora.estado == .transcribiendo {
                ProgressView().controlSize(.small)
            }
            Button {
                grabadora.alternar(en: chat, almacen: almacen)
            } label: {
                Image(systemName: grabadora.grabando(en: chat) ? "stop.circle.fill" : "mic")
                    .foregroundStyle(grabadora.grabando(en: chat) ? Color.red : Color.primary)
            }
            .buttonStyle(.borderless)
            .help(grabadora.grabando(en: chat) ? "Parar y mandar la nota de voz" : "Grabar una nota de voz")
            .disabled(ocupadaEnOtro)
        }
    }

    /// Grabando en otro chat, o guardando: este boton espera.
    private var ocupadaEnOtro: Bool {
        switch grabadora.estado {
        case .quieta: false
        case let .grabando(c, _): c != chat
        case .guardando, .transcribiendo: true
        }
    }
}
