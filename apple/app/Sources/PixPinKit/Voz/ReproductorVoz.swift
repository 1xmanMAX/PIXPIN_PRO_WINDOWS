// El reproductor de las notas de voz del chat: UNO para toda la app (como
// `audio.rs` de Windows y el `Reproductor` del movil). Darle a otra nota
// para la que sonaba y suelta su `AVAudioPlayer`.
//
// Solo publica lo que cambia poco (que nota y si suena): el avance lo lee la
// burbuja que suena con un `TimelineView` mientras suena, asi las demas
// burbujas no se repintan diez veces por segundo y no hay temporizadores
// cuando no suena nada.

import AVFoundation
import Foundation

@MainActor
public final class ReproductorVoz: NSObject, ObservableObject {
    public static let compartido = ReproductorVoz()

    /// El id del mensaje cargado (sonando o en pausa).
    @Published public private(set) var actual: String?
    @Published public private(set) var sonando = false

    private var reproductor: AVAudioPlayer?

    override public init() { super.init() }

    /// Por donde va, de 0 a 1 (0 si no hay nada cargado).
    public var fraccion: Double {
        guard let r = reproductor, r.duration > 0 else { return 0 }
        return min(max(r.currentTime / r.duration, 0), 1)
    }

    /// Por donde va, en milisegundos.
    public var posicionMs: Int64 {
        guard let r = reproductor else { return 0 }
        return Int64(r.currentTime * 1000)
    }

    /// Suena o se pausa la nota `id` (el fichero `url`). Si sonaba otra, se
    /// para y se suelta.
    public func alternar(_ id: String, url: URL) {
        if actual == id, let r = reproductor {
            if r.isPlaying {
                r.pause()
                sonando = false
            } else {
                sonando = r.play()
            }
            return
        }
        parar()
        do {
            let r = try AVAudioPlayer(contentsOf: url)
            r.delegate = self
            r.prepareToPlay()
            reproductor = r
            actual = id
            sonando = r.play()
        } catch {
            parar()
        }
    }

    /// Salta a una fraccion (0...1) de la nota que esta cargada.
    public func saltar(_ id: String, a fraccion: Double) {
        guard actual == id, let r = reproductor, r.duration > 0 else { return }
        r.currentTime = min(max(fraccion, 0), 1) * r.duration
    }

    /// Para y suelta el reproductor.
    public func parar() {
        reproductor?.stop()
        reproductor?.delegate = nil
        reproductor = nil
        actual = nil
        sonando = false
    }

    private func acabo() {
        // Se queda cargada al principio, como el movil: otro toque la repite.
        reproductor?.currentTime = 0
        sonando = false
    }
}

extension ReproductorVoz: AVAudioPlayerDelegate {
    nonisolated public func audioPlayerDidFinishPlaying(_ player: AVAudioPlayer, successfully flag: Bool) {
        Task { @MainActor in ReproductorVoz.compartido.acabo() }
    }

    nonisolated public func audioPlayerDecodeErrorDidOccur(_ player: AVAudioPlayer, error: Error?) {
        Task { @MainActor in ReproductorVoz.compartido.parar() }
    }
}
