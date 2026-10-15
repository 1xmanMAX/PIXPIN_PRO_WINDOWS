// Ficheros en el chat: adjuntar por el `Almacen` (que cruza el puente de
// verdad) y las miniaturas de ImageIO.

import CoreGraphics
import Foundation
import ImageIO
import PixPinCore
@testable import PixPinKit
import UniformTypeIdentifiers
import XCTest

/// Un PNG de `ancho` x `alto` con algo pintado, escrito en `url`.
func escribirPNG(_ url: URL, ancho: Int, alto: Int) throws {
    let ctx = try XCTUnwrap(CGContext(data: nil, width: ancho, height: alto, bitsPerComponent: 8,
                                      bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!,
                                      bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
    ctx.setFillColor(CGColor(srgbRed: 0.9, green: 0.3, blue: 0.2, alpha: 1))
    ctx.fill(CGRect(x: 0, y: 0, width: ancho, height: alto))
    ctx.setFillColor(CGColor(srgbRed: 0.1, green: 0.4, blue: 0.9, alpha: 1))
    ctx.fill(CGRect(x: ancho / 4, y: alto / 4, width: ancho / 2, height: alto / 2))
    let imagen = try XCTUnwrap(ctx.makeImage())
    let destino = try XCTUnwrap(CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil))
    CGImageDestinationAddImage(destino, imagen, nil)
    XCTAssertTrue(CGImageDestinationFinalize(destino))
}

@MainActor
final class AdjuntosTests: XCTestCase {
    func testAdjuntarUnaFotoLaCopiaYSuMiniaturaNoPasaDelTamanoPedido() async throws {
        let raiz = try carpetaTemporal("adjuntos")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let almacen = try Almacen(raiz: raiz.appendingPathComponent("datos"), nombreDelAparato: "Mac de prueba")
        await almacen.crearChat(nombre: "Obra")
        let chat = try XCTUnwrap(almacen.chats.first { $0.nombre == "Obra" }).id
        await almacen.abrir(chat: chat)

        let fuera = raiz.appendingPathComponent("fuera", isDirectory: true)
        try FileManager.default.createDirectory(at: fuera, withIntermediateDirectories: true)
        let png = fuera.appendingPathComponent("fachada.png")
        try escribirPNG(png, ancho: 1200, alto: 800)
        let texto = fuera.appendingPathComponent("medidas.txt")
        try Data("3,20 x 2,10".utf8).write(to: texto)

        let hechos = await almacen.adjuntar(urls: [png, texto], en: chat)
        XCTAssertNil(almacen.error)
        XCTAssertEqual(hechos.count, 2)
        let foto = hechos[0]
        XCTAssertEqual(foto.clase, "IMAGEN")
        XCTAssertEqual(foto.nombre, "fachada.png")
        XCTAssertEqual(foto.ruta, "archivos/fachada.png")
        XCTAssertGreaterThan(foto.bytes, 0)
        XCTAssertEqual(hechos[1].clase, "ARCHIVO")
        XCTAssertEqual(hechos[1].ruta, "archivos/medidas.txt")
        // Se anaden al final de lo cargado.
        XCTAssertEqual(almacen.mensajes[chat]?.suffix(2).map(\.id), hechos.map(\.id))

        // La ruta del mensaje lleva a la copia, dentro del proyecto.
        let rutaFoto = try XCTUnwrap(foto.ruta)
        let url = await almacen.url(deRuta: rutaFoto, en: chat)
        let copia = try XCTUnwrap(url)
        XCTAssertTrue(copia.path.hasPrefix(raiz.path))
        XCTAssertNotEqual(copia.standardizedFileURL, png.standardizedFileURL)
        XCTAssertEqual(try Data(contentsOf: copia), try Data(contentsOf: png))
        XCTAssertEqual(almacen.urlResuelta(deRuta: "archivos/fachada.png", en: chat), copia)

        // Al volver a abrir el chat, siguen ahi (estan en el cuaderno).
        await almacen.abrir(chat: chat)
        XCTAssertEqual(almacen.mensajes[chat]?.last?.ruta, "archivos/medidas.txt")

        // La miniatura: el lado mayor como mucho el pedido, con la forma de
        // la foto, y en la cache despues.
        let miniaturas = Miniaturas(topeBytes: 8 << 20)
        let cargada = await miniaturas.cargar(copia, lado: 128)
        let mini = try XCTUnwrap(cargada)
        XCTAssertLessThanOrEqual(max(mini.width, mini.height), 128)
        XCTAssertEqual(Double(mini.width) / Double(mini.height), 1.5, accuracy: 0.05)
        XCTAssertNotNil(miniaturas.enCache(copia, lado: 128))
        XCTAssertNil(miniaturas.enCache(copia, lado: 64), "cada tamano es su entrada")
        let sincrona = try XCTUnwrap(Miniaturas.leer(copia, lado: 64))
        XCTAssertLessThanOrEqual(max(sincrona.width, sincrona.height), 64)
        // Lo que no es una imagen no da miniatura (ni revienta).
        XCTAssertNil(Miniaturas.leer(texto, lado: 64))
        let nada = await miniaturas.cargar(fuera.appendingPathComponent("no-esta.png"), lado: 64)
        XCTAssertNil(nada)
        miniaturas.vaciar()
        XCTAssertNil(miniaturas.enCache(copia, lado: 128))
    }

    func testUnaImagenPegadaEnTiffSeGuardaComoPng() async throws {
        let raiz = try carpetaTemporal("pegado")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let almacen = try Almacen(raiz: raiz, nombreDelAparato: "Mac de prueba")
        await almacen.crearChat(nombre: "Obra")
        let chat = try XCTUnwrap(almacen.chats.first).id

        // Un TIFF en memoria, como el que deja el portapapeles del Mac.
        let datos = NSMutableData()
        let ctx = try XCTUnwrap(CGContext(data: nil, width: 40, height: 20, bitsPerComponent: 8,
                                          bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!,
                                          bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        ctx.setFillColor(gray: 0.5, alpha: 1)
        ctx.fill(CGRect(x: 0, y: 0, width: 40, height: 20))
        let destino = try XCTUnwrap(CGImageDestinationCreateWithData(datos, UTType.tiff.identifier as CFString, 1, nil))
        CGImageDestinationAddImage(destino, try XCTUnwrap(ctx.makeImage()), nil)
        XCTAssertTrue(CGImageDestinationFinalize(destino))

        let pegado = await almacen.adjuntar(datos: datos as Data, formato: "tiff", en: chat)
        let m = try XCTUnwrap(pegado)
        XCTAssertEqual(m.clase, "IMAGEN")
        XCTAssertTrue(m.nombre.hasPrefix("pegado-") && m.nombre.hasSuffix(".png"), m.nombre)
        let rutaPegada = try XCTUnwrap(m.ruta)
        let resuelta = await almacen.url(deRuta: rutaPegada, en: chat)
        let url = try XCTUnwrap(resuelta)
        let leida = try XCTUnwrap(Miniaturas.leer(url, lado: 400))
        // Mas pequena que lo pedido: sale con su tamano, sin agrandar.
        XCTAssertEqual(leida.width, 40)
        XCTAssertEqual(leida.height, 20)
    }

    func testUnaRutaDeOtroAparatoNoSeResuelve() async throws {
        let raiz = try carpetaTemporal("rutas")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let almacen = try Almacen(raiz: raiz, nombreDelAparato: "Mac de prueba")
        await almacen.crearChat(nombre: "Obra")
        let chat = try XCTUnwrap(almacen.chats.first).id
        let ajena = await almacen.url(deRuta: "/storage/emulated/0/foto.jpg", en: chat)
        XCTAssertNil(ajena)
        let fuera = await almacen.url(deRuta: "../../secreto", en: chat)
        XCTAssertNil(fuera)
    }
}
