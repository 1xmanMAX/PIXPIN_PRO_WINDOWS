// Notas de voz y grabar la pantalla, sin microfono ni permiso de grabacion:
// las cuentas puras, escribir un `.m4a` con sonido sintetizado y guardarlo
// en un chat de verdad (cruzando el puente) con su texto.

import AVFoundation
import Foundation
import PixPinCore
@testable import PixPinKit
import XCTest

/// Un bloque de seno de `segundos` a `muestreo` con `canales` canales
/// (Float32 sin entrelazar, como lo da el microfono) y amplitud `amplitud`.
func senoDePrueba(segundos: Double, muestreo: Double, canales: AVAudioChannelCount = 2,
                  amplitud: Float = 0.5) throws -> AVAudioPCMBuffer {
    let formato = try XCTUnwrap(AVAudioFormat(standardFormatWithSampleRate: muestreo, channels: canales))
    let n = AVAudioFrameCount(segundos * muestreo)
    let b = try XCTUnwrap(AVAudioPCMBuffer(pcmFormat: formato, frameCapacity: n))
    b.frameLength = n
    let datos = try XCTUnwrap(b.floatChannelData)
    for c in 0..<Int(canales) {
        for i in 0..<Int(n) {
            datos[c][i] = amplitud * sin(2 * .pi * 440 * Float(i) / Float(muestreo))
        }
    }
    return b
}

final class FormatoVozTests: XCTestCase {
    func testLaMarcaDeTiempoEsLaDelMovil() {
        XCTAssertEqual(FormatoVoz.marca(ms: 0), "0:00")
        XCTAssertEqual(FormatoVoz.marca(ms: 83_000), "1:23")
        XCTAssertEqual(FormatoVoz.marca(ms: 725_999), "12:05")
        XCTAssertEqual(FormatoVoz.marca(ms: 3_729_000), "1:02:09")
        XCTAssertEqual(FormatoVoz.marca(ms: -5), "0:00")
    }

    func testElMuestreoYLaDuracionSonLosDeWindows() {
        XCTAssertEqual(FormatoVoz.muestreo(deLaTarjeta: 44_100), 44_100)
        XCTAssertEqual(FormatoVoz.muestreo(deLaTarjeta: 48_000), 48_000)
        XCTAssertEqual(FormatoVoz.muestreo(deLaTarjeta: 96_000), 48_000)
        XCTAssertEqual(FormatoVoz.duracionMs(muestras: 48_000 * 7 - 1, muestreo: 48_000), 6_999)
        XCTAssertEqual(FormatoVoz.duracionMs(muestras: 10, muestreo: 0), 0)
        let a = FormatoVoz.ajustes(muestreo: 48_000)
        XCTAssertEqual(a[AVFormatIDKey] as? AudioFormatID, kAudioFormatMPEG4AAC)
        XCTAssertEqual(a[AVNumberOfChannelsKey] as? Int, 1)
        XCTAssertEqual(a[AVEncoderBitRateKey] as? Int, 32_000)
        XCTAssertNil(FormatoVoz.ajustes(muestreo: 48_000, conBitrate: false)[AVEncoderBitRateKey])
    }

    func testLaTranscripcionSeLeeEnTrozosSinSusMinutos() {
        let t = FormatoVoz.trozos("[0:00] llamar al aparejador\n\n[1:23] por los niveles\r\n\r\n  \nsuelto\notra linea")
        XCTAssertEqual(t.count, 3)
        XCTAssertEqual(t[0].ms, 0)
        XCTAssertEqual(t[0].texto, "llamar al aparejador")
        XCTAssertEqual(t[1].ms, 83_000)
        XCTAssertEqual(t[1].texto, "por los niveles")
        XCTAssertNil(t[2].ms)
        XCTAssertEqual(t[2].texto, "suelto\notra linea")
        XCTAssertTrue(FormatoVoz.trozos("  \n\n ").isEmpty)
        XCTAssertEqual(FormatoVoz.tiempo(de: "[1:02:09] x")?.0, 3_729_000)
        XCTAssertNil(FormatoVoz.tiempo(de: "[1:2] x"), "dos cifras en los segundos")
        XCTAssertNil(FormatoVoz.tiempo(de: "[0:61] x"))
        XCTAssertNil(FormatoVoz.tiempo(de: "sin marca"))
    }

    func testLosPicosSonUnoCadaCincuentaMsDeCeroA32767() throws {
        var r = RepartoPicos(muestreo: 1000)
        // 1 s a 1 kHz: 20 picos de 50 muestras; el ultimo trozo a medias cuenta.
        let muestras = (0..<1010).map { i -> Float in i == 75 ? -1.5 : 0.25 }
        muestras.withUnsafeBufferPointer { r.empuja($0) }
        let picos = r.terminar()
        XCTAssertEqual(picos.count, 21)
        XCTAssertEqual(picos[1], 32767, "el valor absoluto, acotado")
        XCTAssertEqual(picos[0], FormatoVoz.pico(0.25))
        XCTAssertTrue(picos.allSatisfy { (0...32767).contains($0) })
    }

    func testUnaNotaLargaNoPasaDelTopeDePicos() {
        var r = RepartoPicos(muestreo: 1000, tope: 8)
        let muestras = [Float](repeating: 0.5, count: 50 * 100)
        muestras.withUnsafeBufferPointer { r.empuja($0) }
        let picos = r.terminar()
        XCTAssertLessThanOrEqual(picos.count, 8)
        XCTAssertGreaterThan(picos.count, 2)
        XCTAssertEqual(RepartoPicos.aMitad([2, 4, 7]), [3, 7])
    }

    func testLosSegmentosDelReconocedorPasanAMilisegundos() {
        let s = TranscriptorVoz.segmentos([(desde: 0, texto: "llamar"), (desde: 1.25, texto: " al "),
                                          (desde: 2, texto: "  ")])
        XCTAssertEqual(s, [SegmentoVoz(desdeMs: 0, texto: "llamar"), SegmentoVoz(desdeMs: 1250, texto: "al")])
    }
}

final class EscritorVozTests: XCTestCase {
    func testSeEscribeUnM4aAACMonoConSuDuracionYSusPicos() throws {
        let carpeta = try carpetaTemporal("voz-escribir")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        let url = carpeta.appendingPathComponent("nota.m4a")
        let escritor = try EscritorVoz(url: url, muestreoDeEntrada: 48_000)
        // En dos bloques, como llegan del microfono.
        try escritor.escribir(try senoDePrueba(segundos: 0.75, muestreo: 48_000))
        try escritor.escribir(try senoDePrueba(segundos: 0.75, muestreo: 48_000))
        XCTAssertEqual(escritor.llevadoMs, 1500)
        let nota = escritor.terminar()
        XCTAssertEqual(nota.duracionMs, 1500)
        XCTAssertEqual(nota.picos.count, 30)
        // Seno de amplitud 0,5: los picos rondan la mitad de 32767.
        for p in nota.picos { XCTAssertEqual(Double(p), 16_383, accuracy: 600) }

        // El fichero es AAC mono en MPEG-4 y se puede leer de vuelta.
        let leido = try AVAudioFile(forReading: url)
        XCTAssertEqual(leido.fileFormat.streamDescription.pointee.mFormatID, kAudioFormatMPEG4AAC)
        XCTAssertEqual(leido.fileFormat.channelCount, 1)
        XCTAssertEqual(leido.fileFormat.sampleRate, 48_000)
        XCTAssertEqual(Double(leido.length) / 48_000, 1.5, accuracy: 0.1)
    }

    func testOtroMuestreoSeConvierteAlDelFichero() throws {
        let carpeta = try carpetaTemporal("voz-convertir")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        let url = carpeta.appendingPathComponent("nota.m4a")
        let escritor = try EscritorVoz(url: url, muestreoDeEntrada: 22_050)
        XCTAssertEqual(escritor.muestreo, 48_000)
        try escritor.escribir(try senoDePrueba(segundos: 1, muestreo: 22_050, canales: 1))
        let nota = escritor.terminar()
        XCTAssertGreaterThan(nota.duracionMs, 900)
        XCTAssertLessThanOrEqual(nota.duracionMs, 1000)
        XCTAssertFalse(nota.picos.isEmpty)
        XCTAssertEqual(try AVAudioFile(forReading: url).fileFormat.channelCount, 1)
    }
}

@MainActor
final class NotaDeVozEnElChatTests: XCTestCase {
    func testUnaNotaGrabadaVaAlChatComoVOZYSuTextoSeGuardaComoEnWindows() async throws {
        let raiz = try carpetaTemporal("voz-chat")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let almacen = try Almacen(raiz: raiz.appendingPathComponent("datos"), nombreDelAparato: "Mac de prueba")
        await almacen.crearChat(nombre: "Obra")
        let chat = try XCTUnwrap(almacen.chats.first { $0.nombre == "Obra" }).id
        await almacen.abrir(chat: chat)

        let url = raiz.appendingPathComponent("grabando.m4a")
        let escritor = try EscritorVoz(url: url, muestreoDeEntrada: 44_100)
        try escritor.escribir(try senoDePrueba(segundos: 1.2, muestreo: 44_100, canales: 1))
        let nota = escritor.terminar()

        let hecho = await almacen.adjuntarVoz(nota, en: chat)
        let m = try XCTUnwrap(hecho, almacen.error ?? "")
        XCTAssertEqual(m.clase, "VOZ")
        XCTAssertTrue(m.nombre.hasPrefix("voz_") && m.nombre.hasSuffix(".m4a"), m.nombre)
        XCTAssertEqual(m.ruta, "archivos/\(m.nombre)")
        XCTAssertEqual(m.duracionMs, 1200)
        XCTAssertEqual(m.picos, nota.picos)
        XCTAssertNil(m.transcripcion)
        XCTAssertEqual(almacen.mensajes[chat]?.last?.id, m.id)

        // La copia dentro del proyecto suena (es el mismo .m4a).
        let ruta = try XCTUnwrap(m.ruta)
        let resuelta = await almacen.url(deRuta: ruta, en: chat)
        let copia = try XCTUnwrap(resuelta)
        XCTAssertEqual(try Data(contentsOf: copia), try Data(contentsOf: url))
        XCTAssertNoThrow(try AVAudioPlayer(contentsOf: copia))

        // El texto: con sus minutos, en el mensaje cargado y en el disco.
        let conTexto = await almacen.ponerTranscripcion(
            [SegmentoVoz(desdeMs: 0, texto: "llamar al"), SegmentoVoz(desdeMs: 500, texto: "aparejador")],
            en: m.id, chat: chat)
        XCTAssertEqual(conTexto?.transcripcion, "[0:00] llamar al aparejador")
        XCTAssertEqual(almacen.mensajes[chat]?.last?.transcripcion, "[0:00] llamar al aparejador")
        XCTAssertEqual(almacen.mensajes[chat]?.last?.resumen, "[0:00] llamar al aparejador")

        let cuaderno = raiz.appendingPathComponent("datos/proyectos/\(chat)/guardados.jsonl")
        let linea = try XCTUnwrap(String(contentsOf: cuaderno, encoding: .utf8)
            .split(separator: "\n").first { $0.contains(m.id) })
        let json = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(linea.utf8)) as? [String: Any])
        XCTAssertEqual(json["clase"] as? String, "VOZ")
        XCTAssertEqual(json["duracionMs"] as? Int, 1200)
        XCTAssertEqual(json["transcripcion"] as? String, "[0:00] llamar al aparejador")
        XCTAssertEqual(json["estadoDelTexto"] as? String, "bien")
        XCTAssertEqual((json["picos"] as? [Int])?.count, nota.picos.count)
    }
}

final class GeometriaGrabacionTests: XCTestCase {
    func testElVideoSaleConMedidasParesYCabeEnH264() {
        let a = GeometriaGrabacion.tamSalida(puntos: CGSize(width: 320.5, height: 201), escala: 2)
        XCTAssertEqual(a.ancho, 640)
        XCTAssertEqual(a.alto, 402)
        XCTAssertEqual(a.ancho % 2, 0)
        // Una 5K a 2x se reduce sin deformar hasta caber en 4096x2304.
        let b = GeometriaGrabacion.tamSalida(puntos: CGSize(width: 2560, height: 1440), escala: 2)
        XCTAssertLessThanOrEqual(b.ancho, 4096)
        XCTAssertLessThanOrEqual(b.alto, 2304)
        XCTAssertEqual(Double(b.ancho) / Double(b.alto), 16.0 / 9.0, accuracy: 0.01)
        XCTAssertEqual(b.ancho % 2 + b.alto % 2, 0)
        let c = GeometriaGrabacion.tamSalida(puntos: CGSize(width: 0, height: 0), escala: 1)
        XCTAssertEqual(c.ancho, 2)
        XCTAssertEqual(c.alto, 2)
    }

    func testLosBitsPorSegundoSonLosDeWindows() {
        XCTAssertEqual(GeometriaGrabacion.bitsPorSegundo(ancho: 1920, alto: 1080), 6_220_800)
        XCTAssertEqual(GeometriaGrabacion.bitsPorSegundo(ancho: 100, alto: 100), 500_000)
        XCTAssertEqual(GeometriaGrabacion.bitsPorSegundo(ancho: 7680, alto: 4320), 20_000_000)
    }

    func testLaZonaPasaAOrigenArribaYElNombreEsElDeLaGaleria() throws {
        let r = GeometriaGrabacion.desdeVista(CGRect(x: 10, y: 700, width: 200, height: 100), altoPantalla: 900)
        XCTAssertEqual(r, CGRect(x: 10, y: 100, width: 200, height: 100))

        let carpeta = URL(fileURLWithPath: "/tmp/galeria", isDirectory: true)
        let zona = try XCTUnwrap(TimeZone(identifier: "Europe/Madrid"))
        // 2026-10-09 14:25:07 en Madrid (12:25:07 UTC).
        let fecha = Date(timeIntervalSince1970: 1_791_548_707)
        let libre = GeometriaGrabacion.rutaNueva(en: carpeta, fecha: fecha, zona: zona, existe: { _ in false })
        XCTAssertEqual(libre.lastPathComponent, "captura-20261009-142507.mp4")
        // Ocupado por una captura PNG de ese segundo: la siguiente.
        let ocupada = GeometriaGrabacion.rutaNueva(en: carpeta, fecha: fecha, zona: zona,
                                                   existe: { $0.lastPathComponent == "captura-20261009-142507.png" })
        XCTAssertEqual(ocupada.lastPathComponent, "captura-20261009-142507-2.mp4")
    }
}
