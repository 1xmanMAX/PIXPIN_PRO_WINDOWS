// El editor del lienzo por el puente de verdad: dibujar con el puntero,
// deshacer, guardar y volver a abrir; y la vista que lo pinta.

import AppKit
import Foundation
import PixPinCore
@testable import PixPinKit
import XCTest

/// Un trazo a mano de `pasos` avisos desde `(x, y)`.
private func trazo(_ ed: EditorLienzo, x: Float, y: Float, pasos: Int = 20) {
    _ = ed.puntero(fase: .pulsar, x: x, y: y, presion: 0.5, modificadores: 0, zoom: 1)
    for i in 1...pasos {
        let t = Float(i)
        _ = ed.puntero(fase: .mover, x: x + t * 5, y: y + sin(t * 0.6) * 15, presion: 0.6,
                       modificadores: 0, zoom: 1)
    }
    _ = ed.puntero(fase: .soltar, x: x + Float(pasos) * 5, y: y, presion: nil, modificadores: 0, zoom: 1)
}

final class EditorLienzoTests: XCTestCase {
    func testDibujarDeshacerGuardarYVolverAAbrir() throws {
        let carpeta = try carpetaTemporal("editor")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        let ruta = carpeta.appendingPathComponent("a.excalidraw").path
        let ed = try EditorLienzo.deFichero(ruta: ruta, crear: true)
        XCTAssertTrue(FileManager.default.fileExists(atPath: ruta))
        XCTAssertEqual(ed.cuantosElementos(), 0)

        let v0 = ed.version()
        trazo(ed, x: 10, y: 10)
        XCTAssertGreaterThan(ed.version(), v0)
        try ed.herramienta(nombre: "rectangulo")
        _ = ed.puntero(fase: .pulsar, x: 100, y: 100, presion: nil, modificadores: 0, zoom: 1)
        _ = ed.puntero(fase: .mover, x: 200, y: 160, presion: nil, modificadores: 0, zoom: 1)
        _ = ed.puntero(fase: .soltar, x: 200, y: 160, presion: nil, modificadores: 0, zoom: 1)
        XCTAssertEqual(ed.cuantosElementos(), 2)

        // Deshacer quita el rectangulo; rehacer lo devuelve; deshacer otra vez.
        XCTAssertTrue(ed.deshacer())
        XCTAssertEqual(ed.cuantosElementos(), 1)
        XCTAssertTrue(ed.rehacer())
        XCTAssertEqual(ed.cuantosElementos(), 2)
        XCTAssertTrue(ed.deshacer())

        // Lo que se ve lo lee el lector de siempre.
        let leido = try leerLienzo(ed.planoDeVista(x: 0, y: 0, zoom: 1, anchoPx: 800, altoPx: 600))
        XCTAssertFalse(leido.figuras.isEmpty)

        XCTAssertTrue(ed.sinGuardar())
        XCTAssertTrue(try ed.guardar())
        XCTAssertFalse(try ed.guardar(), "sin cambios no se vuelve a escribir")

        let otra = try EditorLienzo.deFichero(ruta: ruta, crear: false)
        XCTAssertEqual(otra.cuantosElementos(), 1)
        let json = try String(contentsOfFile: ruta, encoding: .utf8)
        XCTAssertTrue(json.contains("\"freedraw\""))
        XCTAssertFalse(json.contains("\"rectangle\""))
    }

    func testUnLienzoNuevoEnElChatSeEditaYSeGuardaEnSuSitio() throws {
        let raiz = try carpetaTemporal("editor-chat")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let nucleo = try Nucleo.abrir(raiz: raiz.path, nombreDelAparato: "Mac")
        let chat = try nucleo.crearChat(nombre: "Obra")
        let m = try nucleo.crearLienzo(chat: chat.id, nombre: nil)
        XCTAssertEqual(m.clase, "DIBUJO")
        let referencia = try XCTUnwrap(m.referencia)
        let ed = try nucleo.editarLienzo(chat: chat.id, dibujo: referencia)
        try ed.herramienta(nombre: "elipse")
        _ = ed.puntero(fase: .pulsar, x: 0, y: 0, presion: nil, modificadores: 0, zoom: 2)
        _ = ed.puntero(fase: .mover, x: 80, y: 50, presion: nil, modificadores: 0, zoom: 2)
        _ = ed.puntero(fase: .soltar, x: 80, y: 50, presion: nil, modificadores: 0, zoom: 2)
        XCTAssertTrue(try ed.guardar())
        // El de solo mirar lo lee igual.
        XCTAssertEqual(try nucleo.abrirLienzo(chat: chat.id, dibujo: referencia).cuantosElementos(), 1)
    }

    @MainActor
    func testLaVistaPintaLoQueDibujaElEditor() throws {
        let carpeta = try carpetaTemporal("editor-vista")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        let ed = try EditorLienzo.deFichero(ruta: carpeta.appendingPathComponent("v.excalidraw").path,
                                            crear: true)
        trazo(ed, x: 20, y: 20, pasos: 30)
        let modelo = ModeloEditorLienzo(editor: ed)
        XCTAssertEqual(modelo.herramienta, "lapiz")
        modelo.elegir("seleccion")
        XCTAssertEqual(modelo.herramienta, "seleccion")
        XCTAssertEqual(ed.herramientaActual(), "seleccion")

        let vista = VistaEditorLienzoNS(modelo: modelo)
        vista.frame = NSRect(x: 0, y: 0, width: 400, height: 300)
        let rep = try XCTUnwrap(vista.bitmapImageRepForCachingDisplay(in: vista.bounds))
        vista.cacheDisplay(in: vista.bounds, to: rep)
        // Algo que no es el papel blanco: la tinta.
        var oscuros = 0
        for y in stride(from: 0, to: rep.pixelsHigh, by: 2) {
            for x in stride(from: 0, to: rep.pixelsWide, by: 2) {
                if let c = rep.colorAt(x: x, y: y)?.usingColorSpace(.deviceRGB),
                   c.alphaComponent > 0.5, c.brightnessComponent < 0.5 { oscuros += 1 }
            }
        }
        XCTAssertGreaterThan(oscuros, 5, "el trazo tiene que verse")
    }
}
