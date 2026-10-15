// Pruebas de la app de Mac que corren en la CI de macOS (`swift test`):
// cruzan el puente de verdad (el nucleo de Rust enlazado), no un simulacro.

import CoreGraphics
import Foundation
import PixPinCore
@testable import PixPinKit
import XCTest

/// El mismo lienzo que `nucleo/pixpin-nucleo/tests/api.rs`.
let lienzoDePrueba = #"""
{
  "type": "excalidraw", "version": 2, "source": "pixpin-android",
  "elements": [
    {"id":"a1","type":"rectangle","x":10,"y":20,"width":100,"height":50,
     "strokeColor":"#1e1e1e","backgroundColor":"#ffc9c9","fillStyle":"solid","strokeWidth":2,
     "strokeStyle":"dashed","roughness":1,"opacity":100,"seed":12345},
    {"id":"t","type":"freedraw","x":100,"y":200,"width":10,"height":10,
     "points":[[0,0],[5,5],[10,10],[20,4]]},
    {"id":"x","type":"text","x":0,"y":300,"width":50,"height":20,"text":"Cota ñ",
     "fontSize":20,"fontFamily":5},
    {"id":"i1","type":"image","x":5000,"y":5000,"width":160,"height":110,
     "fileId":"7xJoKC","strokeColor":"#1e1e1e"}
  ],
  "appState": {"viewBackgroundColor": "#fafafa"},
  "files": {"7xJoKC":{"id":"7xJoKC","mimeType":"image/png","path":"imagenes/7xJoKC"}}
}
"""#

func carpetaTemporal(_ nombre: String) throws -> URL {
    let url = FileManager.default.temporaryDirectory
        .appendingPathComponent("pixpin-mac-\(nombre)-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
    return url
}

final class LectorLienzoTests: XCTestCase {
    func testElBuferDeRustSeLeeEnteroYConSusFiguras() throws {
        let lienzo = try Lienzo.deJson(texto: lienzoDePrueba)
        let leido = try leerLienzo(lienzo.plano())
        XCTAssertFalse(leido.figuras.isEmpty)
        XCTAssertNotNil(leido.caja)
        // El papel #fafafa.
        XCTAssertEqual(leido.fondo.r, 0xfa / 255, accuracy: 0.01)
        // El texto con su ñ intacta y la imagen por su id.
        XCTAssertTrue(leido.figuras.contains { if case let .texto(t, _, _, _, _, _, _, _) = $0 { t == "Cota ñ" } else { false } })
        let ids = lienzo.imagenes().map(\.idObjeto)
        XCTAssertTrue(leido.figuras.contains { if case let .imagen(id, _, _, _, _, _) = $0 { ids.contains(id) } else { false } })
    }

    func testUnaOrdenDesconocidaSeSaltaSinRomperNada() throws {
        // Cabecera + una orden de codigo 99 de largo 3 + un poligono.
        var f: [Float] = [1, 12, 2, 1, 1, 1, 1, 0, 0, 10, 10, 0]
        let desconocida: [Float] = [99, 3, 42]
        let poligono: [Float] = [1, 13, 0, 0, 0, 1, 3, 0, 0, 10, 0, 0, 10]
        f += desconocida
        f += poligono
        f[11] = Float(f.count)
        let datos = f.withUnsafeBytes { Data($0) }
        let leido = try leerLienzo(LienzoPlano(datos: datos, textos: [], imagenes: []))
        XCTAssertEqual(leido.figuras.count, 1)
        guard case let .relleno(path, _) = leido.figuras[0] else { return XCTFail("no es un relleno") }
        XCTAssertEqual(path.boundingBox, CGRect(x: 0, y: 0, width: 10, height: 10))
    }

    func testUnaVersionQueNoSeConoceNoSeLee() {
        var f: [Float] = [2, 12, 0, 1, 1, 1, 1, .nan, .nan, .nan, .nan, 12]
        let datos = f.withUnsafeMutableBytes { Data($0) }
        XCTAssertThrowsError(try leerLienzo(LienzoPlano(datos: datos, textos: [], imagenes: []))) {
            XCTAssertEqual($0 as? ErrorLectorLienzo, .version(2))
        }
    }

    func testPintarElLienzoEnUnBitmapNoRevientaYDejaTinta() throws {
        let leido = try leerLienzo(try Lienzo.deJson(texto: lienzoDePrueba).plano())
        let ctx = try XCTUnwrap(CGContext(data: nil, width: 400, height: 400, bitsPerComponent: 8,
                                          bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!,
                                          bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        ctx.translateBy(x: 0, y: 400)
        ctx.scaleBy(x: 1, y: -1)
        PintorLienzo(lienzo: leido).pintar(en: ctx, limites: CGRect(x: 0, y: 0, width: 400, height: 400))
        let bytes = try XCTUnwrap(ctx.data).assumingMemoryBound(to: UInt8.self)
        let pintados = (0..<(400 * 400)).reduce(0) { $0 + (bytes[$1 * 4 + 3] > 0 ? 1 : 0) }
        XCTAssertGreaterThan(pintados, 100, "el rectangulo y la tinta tienen que dejar pixeles")
    }
}

@MainActor
final class AlmacenTests: XCTestCase {
    func testCrearUnProyectoEscribirYLeerPorPaginas() async throws {
        let raiz = try carpetaTemporal("almacen")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let almacen = try Almacen(raiz: raiz, nombreDelAparato: "Mac de prueba")
        await almacen.crearChat(nombre: "Obra ñandú")
        XCTAssertNil(almacen.error)
        let chat = try XCTUnwrap(almacen.chats.first { $0.nombre == "Obra ñandú" })

        for k in 0..<(Int(Almacen.tamPagina) + 5) {
            await almacen.escribir(nota: "nota \(k)", en: chat.id)
        }
        // Al abrir solo llega la ultima pagina, no el chat entero.
        await almacen.abrir(chat: chat.id)
        XCTAssertEqual(almacen.mensajes[chat.id]?.count, Int(Almacen.tamPagina))
        XCTAssertEqual(almacen.mensajes[chat.id]?.last?.texto, "nota \(Int(Almacen.tamPagina) + 4)")
        XCTAssertTrue(almacen.quedanAnteriores(chat: chat.id))

        await almacen.cargarAnteriores(chat: chat.id)
        XCTAssertEqual(almacen.mensajes[chat.id]?.count, Int(Almacen.tamPagina) + 5)
        XCTAssertEqual(almacen.mensajes[chat.id]?.first?.texto, "nota 0")
        XCTAssertFalse(almacen.quedanAnteriores(chat: chat.id))

        // Una nota vacia no se escribe.
        let nada = await almacen.escribir(nota: "   ", en: chat.id)
        XCTAssertNil(nada)

        // Aligerar suelta lo que no se ve.
        almacen.aligerar(visible: nil)
        XCTAssertNil(almacen.mensajes[chat.id])
    }
}
