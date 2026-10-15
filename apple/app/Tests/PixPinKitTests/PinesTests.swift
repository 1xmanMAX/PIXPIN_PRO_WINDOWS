// Pruebas de los pines: solo las cuentas (sin abrir ventanas) y el OCR de
// Vision sobre una imagen con texto pintada aqui con Core Text.

import CoreGraphics
import CoreText
import Foundation
import ImageIO
@testable import PixPinKit
import UniformTypeIdentifiers
import XCTest

final class ReglasPinTests: XCTestCase {
    func testElZoomNoPasaDeOchoVecesNiBajaDelLadoMinimo() {
        let base = CGSize(width: 400, height: 200)
        XCTAssertEqual(ReglasPin.escalaLimitada(1.5, base: base), 1.5)
        XCTAssertEqual(ReglasPin.escalaLimitada(100, base: base), ReglasPin.escalaMaxima)
        // El lado corto (200) no baja de 32 puntos.
        XCTAssertEqual(ReglasPin.escalaLimitada(0.01, base: base), 32 / 200, accuracy: 1e-9)
        // Un pin que nacio mas pequeno que el minimo no encoge mas.
        XCTAssertEqual(ReglasPin.escalaLimitada(0.5, base: CGSize(width: 20, height: 10)), 1)
        XCTAssertEqual(ReglasPin.escalaLimitada(.nan, base: base), 1)
    }

    func testLaRuedaSubeYBajaLoMismo() {
        let arriba = ReglasPin.factorZoom(deltaY: 3, precisa: false)
        let abajo = ReglasPin.factorZoom(deltaY: -3, precisa: false)
        XCTAssertGreaterThan(arriba, 1)
        XCTAssertEqual(arriba * abajo, 1, accuracy: 1e-9)
        // Un tiron enorme no salta mas de e^0.5 de una vez.
        XCTAssertEqual(ReglasPin.factorZoom(deltaY: 10_000, precisa: true), exp(0.5), accuracy: 1e-9)
    }

    func testElZoomDejaQuietoElPuntoBajoElRaton() {
        let marco = CGRect(x: 100, y: 100, width: 200, height: 100)
        let ancla = CGPoint(x: 150, y: 125) // a 1/4 por cada lado
        let nuevo = ReglasPin.marcoTrasZoom(marco, ancla: ancla, tamano: CGSize(width: 400, height: 200))
        XCTAssertEqual(nuevo, CGRect(x: 50, y: 75, width: 400, height: 200))
        XCTAssertEqual((ancla.x - nuevo.minX) / nuevo.width, 0.25, accuracy: 1e-9)
    }

    func testLaOpacidadVaDeVeinteACienAPasosDeCinco() {
        XCTAssertEqual(ReglasPin.opacidadTrasRueda(1, deltaY: 1, precisa: false), 1)
        XCTAssertEqual(ReglasPin.opacidadTrasRueda(1, deltaY: -1, precisa: false), 0.95, accuracy: 1e-9)
        XCTAssertEqual(ReglasPin.opacidadTrasRueda(0.22, deltaY: -1, precisa: false), 0.2, accuracy: 1e-9)
        XCTAssertEqual(ReglasPin.opacidadTrasRueda(0.5, deltaY: 0, precisa: false), 0.5, accuracy: 1e-9)
        XCTAssertEqual(ReglasPin.opacidadTrasRueda(0.5, deltaY: 20, precisa: true), 0.6, accuracy: 1e-9)
        XCTAssertEqual(ReglasPin.limitarOpacidad(0.01), 0.2)
        XCTAssertEqual(ReglasPin.porcentaje(0.7), 70)
        XCTAssertEqual(ReglasPin.opacidades, [100, 90, 80, 70, 60, 50, 40, 30])
        // Todas las del menu caben en el intervalo permitido.
        for p in ReglasPin.opacidades {
            XCTAssertEqual(ReglasPin.porcentaje(CGFloat(p) / 100), p)
        }
    }

    func testLaMiniaturaSeLeeAlTamanoDeLaPantallaEnPixeles() {
        XCTAssertEqual(ReglasPin.ladoMiniatura(pantalla: CGSize(width: 1512, height: 982), escala: 2), 3024)
        XCTAssertEqual(ReglasPin.ladoMiniatura(pantalla: CGSize(width: 1920, height: 1080), escala: 1), 1920)
        XCTAssertEqual(ReglasPin.ladoMiniatura(pantalla: .zero, escala: 2), ReglasPin.miniaturaMinima)
    }

    func testElPinNaceASuTamanoSinPasarDelOchentaPorCiento() {
        let visible = CGSize(width: 1000, height: 800)
        // 600x400 pixeles en Retina: 300x200 puntos.
        XCTAssertEqual(ReglasPin.tamanoInicial(pixeles: CGSize(width: 600, height: 400), escala: 2, visible: visible),
                       CGSize(width: 300, height: 200))
        // Una foto enorme cabe en 800x640 conservando 2:1.
        let grande = ReglasPin.tamanoInicial(pixeles: CGSize(width: 8000, height: 4000), escala: 1, visible: visible)
        XCTAssertEqual(grande, CGSize(width: 800, height: 400))
        // Una diminuta crece hasta el lado minimo.
        let diminuta = ReglasPin.tamanoInicial(pixeles: CGSize(width: 20, height: 10), escala: 1, visible: visible)
        XCTAssertEqual(diminuta, CGSize(width: 64, height: 32))
    }

    func testElPinNaceDentroDeLaPantalla() {
        let visible = CGRect(x: 0, y: 0, width: 1000, height: 800)
        let marco = ReglasPin.marcoInicial(tamano: CGSize(width: 200, height: 100),
                                           centro: CGPoint(x: 990, y: 10), visible: visible)
        XCTAssertEqual(marco, CGRect(x: 800, y: 0, width: 200, height: 100))
        let centrado = ReglasPin.marcoInicial(tamano: CGSize(width: 200, height: 100),
                                              centro: CGPoint(x: 500, y: 400), visible: visible)
        XCTAssertEqual(centrado, CGRect(x: 400, y: 350, width: 200, height: 100))
    }

    func testElTipoDeContenidoSaleDeLaExtension() {
        let carpeta = URL(fileURLWithPath: "/no/existe")
        XCTAssertEqual(TipoContenido.de(carpeta.appendingPathComponent("a.png")), .imagen)
        XCTAssertEqual(TipoContenido.de(carpeta.appendingPathComponent("a.heic")), .imagen)
        XCTAssertEqual(TipoContenido.de(carpeta.appendingPathComponent("a.pdf")), .pdf)
        XCTAssertEqual(TipoContenido.de(carpeta.appendingPathComponent("a.mov")), .video)
        XCTAssertEqual(TipoContenido.de(carpeta.appendingPathComponent("a.mp4")), .video)
        XCTAssertEqual(TipoContenido.de(carpeta.appendingPathComponent("a.mp3")), .audio)
        XCTAssertNil(TipoContenido.de(carpeta.appendingPathComponent("a.txt")))
    }

    func testLaMiniaturaNoDecodificaLaFotoEntera() throws {
        // Un PNG de 3000x1000 leido con lado maximo 600 sale de 600x200.
        let carpeta = try carpetaTemporal("miniatura")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        let url = carpeta.appendingPathComponent("grande.png")
        let imagen = try XCTUnwrap(lienzoBlanco(ancho: 3000, alto: 1000)?.makeImage())
        let destino = try XCTUnwrap(CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil))
        CGImageDestinationAddImage(destino, imagen, nil)
        XCTAssertTrue(CGImageDestinationFinalize(destino))

        let mini = try XCTUnwrap(Miniatura.de(url: url, ladoMaximo: 600))
        XCTAssertEqual(mini.width, 600)
        XCTAssertEqual(mini.height, 200)
        // Una que ya cabe no se toca.
        XCTAssertTrue(Miniatura.reducir(imagen, ladoMaximo: 5000) === imagen)
        let reducida = Miniatura.reducir(imagen, ladoMaximo: 300)
        XCTAssertEqual(reducida.width, 300)
        XCTAssertEqual(reducida.height, 100)
    }
}

final class SacarTextoTests: XCTestCase {
    func testVisionLeeElTextoPintadoConCoreText() async throws {
        let ctx = try XCTUnwrap(lienzoBlanco(ancho: 1200, alto: 240))
        let fuente = CTFontCreateWithName("Helvetica" as CFString, 72, nil)
        let atributos: [NSAttributedString.Key: Any] = [
            NSAttributedString.Key(kCTFontAttributeName as String): fuente,
            NSAttributedString.Key(kCTForegroundColorAttributeName as String): CGColor(gray: 0, alpha: 1),
        ]
        let linea = CTLineCreateWithAttributedString(
            NSAttributedString(string: "Hola mundo PixPin", attributes: atributos) as CFAttributedString)
        ctx.textPosition = CGPoint(x: 60, y: 90)
        CTLineDraw(linea, ctx)
        let imagen = try XCTUnwrap(ctx.makeImage())

        let texto = try await sacarTexto(de: imagen)
        XCTAssertTrue(texto.lowercased().contains("mundo"), "Vision leyo: \(texto)")
    }

    func testUnaImagenSinTextoDaCadenaVacia() async throws {
        let imagen = try XCTUnwrap(lienzoBlanco(ancho: 200, alto: 200)?.makeImage())
        let texto = try await sacarTexto(de: imagen)
        XCTAssertTrue(texto.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
    }
}

/// Un bitmap blanco RGB de 8 bits.
private func lienzoBlanco(ancho: Int, alto: Int) -> CGContext? {
    guard let ctx = CGContext(data: nil, width: ancho, height: alto, bitsPerComponent: 8, bytesPerRow: 0,
                              space: CGColorSpaceCreateDeviceRGB(),
                              bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
    else { return nil }
    ctx.setFillColor(CGColor(gray: 1, alpha: 1))
    ctx.fill(CGRect(x: 0, y: 0, width: ancho, height: alto))
    return ctx
}
