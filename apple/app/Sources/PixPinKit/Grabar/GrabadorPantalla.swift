// Grabar la pantalla (o una zona) a MP4: ScreenCaptureKit (`SCStream`)
// entrega los fotogramas ya en YUV 4:2:0, la forma que come el codificador,
// y `AVAssetWriter` los codifica en H.264 con el codificador POR HARDWARE
// (VideoToolbox). Es lo que en Windows hace `pixpin-record` con Media
// Foundation; aqui sin copiar ni convertir un solo pixel en la CPU.
//
// - 30 fotogramas por segundo (`minimumFrameInterval`); si la pantalla no
//   cambia, ScreenCaptureKit no manda nada y no se codifica nada.
// - El sonido del sistema es opcional (`conSonido`), en AAC, sin el de
//   PixPin.
// - Se escribe en una carpeta temporal y al acabar se mueve a `capturas/`
//   con el nombre de Windows (`captura-AAAAMMDD-HHMMSS.mp4`): nunca hay un
//   MP4 a medias en la galeria.
// - H.264 y no HEVC: lo abren Windows y Android sin extensiones. Por eso una
//   zona mayor que 4096x2304 pixeles se reduce en la propia captura.

import AppKit
import AVFoundation
import CoreMedia
import ScreenCaptureKit
import VideoToolbox

@MainActor
public final class GrabadorPantalla: ObservableObject {
    public static let compartido = GrabadorPantalla()

    @Published public private(set) var grabando = false
    /// Cuando empezo la grabacion en marcha.
    @Published public private(set) var desde: Date?

    /// Donde se guardan las grabaciones: la galeria (`capturas/`).
    public var carpetaDeCapturas = carpetaDeDatos().appendingPathComponent("capturas", isDirectory: true)
    /// Avisa de una grabacion guardada (para ensenarla).
    public var alGuardar: ((URL) -> Void)?

    private static let claveSonido = "grabar.sonido"
    /// Grabar tambien el sonido del sistema (apagado de fabrica).
    public var conSonido: Bool {
        get { UserDefaults.standard.bool(forKey: GrabadorPantalla.claveSonido) }
        set {
            UserDefaults.standard.set(newValue, forKey: GrabadorPantalla.claveSonido)
            objectWillChange.send()
        }
    }

    private var flujo: SCStream?
    private var salida: SalidaGrabacion?
    private var empezando = false

    public init() {}

    /// Empieza a grabar `zona`. Si ya graba, no hace nada.
    public func empezar(_ zona: ZonaDeGrabacion) async {
        guard !grabando, !empezando, flujo == nil else { return }
        empezando = true
        defer { empezando = false }
        do {
            let contenido = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
            guard let pantalla = contenido.displays.first(where: { $0.displayID == zona.pantalla }) else {
                throw ErrorGrabacion.sinPantalla
            }
            let yo = ProcessInfo.processInfo.processIdentifier
            let propias = contenido.applications.filter { $0.processID == yo }
            let filtro = SCContentFilter(display: pantalla, excludingApplications: propias, exceptingWindows: [])

            let limites = CGRect(x: 0, y: 0, width: pantalla.width, height: pantalla.height)
            let rect = zona.rect.standardized.intersection(limites)
            guard !rect.isNull, rect.width >= 2, rect.height >= 2 else { throw ErrorGrabacion.zonaVacia }
            let tam = GeometriaGrabacion.tamSalida(puntos: rect.size, escala: zona.escala)
            let sonido = conSonido

            let config = SCStreamConfiguration()
            config.sourceRect = rect
            config.width = tam.ancho
            config.height = tam.alto
            config.scalesToFit = true
            config.minimumFrameInterval = CMTime(value: 1, timescale: GeometriaGrabacion.porSegundo)
            config.pixelFormat = kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange
            config.queueDepth = 6
            config.showsCursor = true
            config.capturesAudio = sonido
            if sonido {
                config.sampleRate = 48_000
                config.channelCount = 2
                config.excludesCurrentProcessAudio = true
            }

            let temporal = FileManager.default.temporaryDirectory
                .appendingPathComponent("pixpin-grabacion-\(UUID().uuidString).mp4")
            let salida = try SalidaGrabacion(url: temporal, ancho: tam.ancho, alto: tam.alto, conSonido: sonido)
            salida.alPararse = { [weak self] in
                Task { @MainActor in await self?.parar() }
            }
            let flujo = SCStream(filter: filtro, configuration: config, delegate: salida)
            try flujo.addStreamOutput(salida, type: .screen, sampleHandlerQueue: salida.cola)
            if sonido {
                try flujo.addStreamOutput(salida, type: .audio, sampleHandlerQueue: salida.cola)
            }
            try await flujo.startCapture()
            self.flujo = flujo
            self.salida = salida
            desde = Date()
            grabando = true
        } catch {
            PermisoCaptura.avisarDenegado(detalle: "No se pudo grabar la pantalla: \(error.localizedDescription)")
        }
    }

    /// Para, cierra el MP4 y lo deja en la galeria.
    public func parar() async {
        guard let flujo, let salida else { return }
        self.flujo = nil
        self.salida = nil
        let empezo = desde ?? Date()
        grabando = false
        desde = nil
        try? await flujo.stopCapture()
        guard let hecho = await salida.terminar() else {
            NSSound.beep()
            return
        }
        let carpeta = carpetaDeCapturas
        let destino = await Task.detached(priority: .utility) { () -> URL? in
            do {
                try FileManager.default.createDirectory(at: carpeta, withIntermediateDirectories: true)
                let destino = GeometriaGrabacion.rutaNueva(en: carpeta, fecha: empezo)
                try FileManager.default.moveItem(at: hecho, to: destino)
                return destino
            } catch {
                try? FileManager.default.removeItem(at: hecho)
                return nil
            }
        }.value
        if let destino { alGuardar?(destino) } else { NSSound.beep() }
    }
}

enum ErrorGrabacion: LocalizedError {
    case sinPantalla
    case zonaVacia
    case escritor(String)

    var errorDescription: String? {
        switch self {
        case .sinPantalla: return "la pantalla ya no esta"
        case .zonaVacia: return "la zona elegida esta vacia"
        case let .escritor(d): return d
        }
    }
}

/// Recibe los fotogramas y el sonido de ScreenCaptureKit en su cola y los
/// pasa al `AVAssetWriter`. Todo lo suyo pasa en `cola`.
final class SalidaGrabacion: NSObject, SCStreamOutput, SCStreamDelegate, @unchecked Sendable {
    let cola = DispatchQueue(label: "PixPin grabar pantalla", qos: .userInitiated)
    /// El flujo se paro solo (la pantalla se desconecto, se quito el
    /// permiso...): quien graba tiene que cerrar.
    var alPararse: (() -> Void)?

    private let url: URL
    private let escritor: AVAssetWriter
    private let video: AVAssetWriterInput
    private let sonido: AVAssetWriterInput?
    private var empezado = false
    private var cerrado = false
    /// El ultimo fotograma escrito y cuando llego (reloj del sistema): al
    /// parar, el video dura hasta el momento de parar aunque la pantalla
    /// llevase un rato quieta.
    private var ultimo = CMTime.invalid
    private var ultimoReloj = CMTime.invalid

    init(url: URL, ancho: Int, alto: Int, conSonido: Bool) throws {
        try? FileManager.default.removeItem(at: url)
        let escritor = try AVAssetWriter(outputURL: url, fileType: .mp4)
        // El indice delante: el video se puede empezar a ver antes de
        // tenerlo entero (al mandarlo al movil o abrirlo desde la red).
        escritor.shouldOptimizeForNetworkUse = true

        let compresion: [String: Any] = [
            AVVideoAverageBitRateKey: GeometriaGrabacion.bitsPorSegundo(ancho: ancho, alto: alto),
            AVVideoExpectedSourceFrameRateKey: Int(GeometriaGrabacion.porSegundo),
            AVVideoMaxKeyFrameIntervalKey: Int(GeometriaGrabacion.porSegundo) * 2,
            AVVideoProfileLevelKey: AVVideoProfileLevelH264HighAutoLevel,
        ]
        let ajustes: [String: Any] = [
            AVVideoCodecKey: AVVideoCodecType.h264,
            AVVideoWidthKey: ancho,
            AVVideoHeightKey: alto,
            AVVideoCompressionPropertiesKey: compresion,
            AVVideoColorPropertiesKey: [
                AVVideoColorPrimariesKey: AVVideoColorPrimaries_ITU_R_709_2,
                AVVideoTransferFunctionKey: AVVideoTransferFunction_ITU_R_709_2,
                AVVideoYCbCrMatrixKey: AVVideoYCbCrMatrix_ITU_R_709_2,
            ],
            // El codificador de la tarjeta (VideoToolbox), no uno en software.
            AVVideoEncoderSpecificationKey: [
                kVTVideoEncoderSpecification_EnableHardwareAcceleratedVideoEncoder as String: true,
            ],
        ]
        let video = AVAssetWriterInput(mediaType: .video, outputSettings: ajustes)
        video.expectsMediaDataInRealTime = true
        guard escritor.canAdd(video) else { throw ErrorGrabacion.escritor("el MP4 no admite el video") }
        escritor.add(video)

        var sonido: AVAssetWriterInput?
        if conSonido {
            let s = AVAssetWriterInput(mediaType: .audio, outputSettings: [
                AVFormatIDKey: kAudioFormatMPEG4AAC,
                AVSampleRateKey: 48_000,
                AVNumberOfChannelsKey: 2,
                AVEncoderBitRateKey: 128_000,
            ])
            s.expectsMediaDataInRealTime = true
            if escritor.canAdd(s) {
                escritor.add(s)
                sonido = s
            }
        }
        self.url = url
        self.escritor = escritor
        self.video = video
        self.sonido = sonido
        super.init()
    }

    // MARK: SCStreamOutput

    func stream(_ stream: SCStream, didOutputSampleBuffer muestra: CMSampleBuffer, of tipo: SCStreamOutputType) {
        guard !cerrado, muestra.isValid else { return }
        if tipo == .screen {
            guard SalidaGrabacion.esFotogramaCompleto(muestra) else { return }
            let t = muestra.presentationTimeStamp
            if !empezado {
                guard escritor.startWriting() else {
                    cerrado = true
                    alPararse?()
                    return
                }
                escritor.startSession(atSourceTime: t)
                empezado = true
            }
            if video.isReadyForMoreMediaData, video.append(muestra) {
                ultimo = t
                ultimoReloj = CMClockGetTime(CMClockGetHostTimeClock())
            }
        } else if tipo == .audio {
            guard empezado, let sonido, sonido.isReadyForMoreMediaData else { return }
            sonido.append(muestra)
        }
    }

    /// Solo los fotogramas con imagen nueva: ScreenCaptureKit tambien manda
    /// avisos de «nada cambio» sin pixeles.
    static func esFotogramaCompleto(_ muestra: CMSampleBuffer) -> Bool {
        guard let adjuntos = CMSampleBufferGetSampleAttachmentsArray(muestra, createIfNecessary: false)
            as? [[SCStreamFrameInfo: Any]],
            let crudo = adjuntos.first?[.status] as? Int,
            let estado = SCFrameStatus(rawValue: crudo) else { return false }
        return estado == .complete
    }

    // MARK: SCStreamDelegate

    func stream(_ stream: SCStream, didStopWithError error: Error) {
        alPararse?()
    }

    // MARK: Cerrar

    /// Cierra el MP4. Devuelve su ruta, o `nil` si no llego a escribirse
    /// nada (o fallo).
    func terminar() async -> URL? {
        await withCheckedContinuation { (c: CheckedContinuation<URL?, Never>) in
            cola.async {
                self.cerrado = true
                guard self.empezado, self.escritor.status == .writing, self.ultimo.isValid else {
                    if self.escritor.status == .writing { self.escritor.cancelWriting() }
                    try? FileManager.default.removeItem(at: self.url)
                    c.resume(returning: nil)
                    return
                }
                self.video.markAsFinished()
                self.sonido?.markAsFinished()
                // Hasta ahora, no hasta el ultimo fotograma (con tope de una
                // hora por si el reloj dijera algo raro).
                let quieto = CMTimeSubtract(CMClockGetTime(CMClockGetHostTimeClock()), self.ultimoReloj)
                let extra = CMTimeMinimum(CMTimeMaximum(quieto, .zero), CMTime(value: 3600, timescale: 1))
                self.escritor.endSession(atSourceTime: CMTimeAdd(self.ultimo, extra))
                self.escritor.finishWriting {
                    let bien = self.escritor.status == .completed
                    if !bien { try? FileManager.default.removeItem(at: self.url) }
                    c.resume(returning: bien ? self.url : nil)
                }
            }
        }
    }
}
