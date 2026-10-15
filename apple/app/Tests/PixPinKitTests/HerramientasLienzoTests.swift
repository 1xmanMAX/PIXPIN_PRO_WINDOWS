// Las herramientas nuevas del lienzo: cambiar de herramienta desde la barra,
// el reloj del gesto de pararse (con un programador de mentira: sin esperar
// de verdad), la barra de estilo, los cajetines de medir y pegar una imagen
// por el puente de verdad.

import AppKit
import Foundation
import PixPinCore
@testable import PixPinKit
import XCTest

/// Un programador que no espera: apunta lo programado y lo dispara a mano.
private final class ProgramadorDePrueba {
    struct Pendiente {
        let segundos: Double
        let accion: () -> Void
        var cancelada = false
    }

    var pendientes: [Pendiente] = []

    var programar: RelojFormaRapida.Programar {
        { [unowned self] segundos, accion in
            self.pendientes.append(Pendiente(segundos: segundos, accion: accion))
            let i = self.pendientes.count - 1
            return { self.pendientes[i].cancelada = true }
        }
    }

    var vivas: [Pendiente] { pendientes.filter { !$0.cancelada } }

    /// Dispara la ultima que no se cancelo.
    func vencer() {
        guard let i = pendientes.lastIndex(where: { !$0.cancelada }) else { return }
        pendientes[i].cancelada = true
        pendientes[i].accion()
    }
}

final class RelojFormaRapidaTests: XCTestCase {
    func testUnaSolaEsperaQueSeReprogramaAlMoverseYSeCancelaAlSoltar() {
        let p = ProgramadorDePrueba()
        let reloj = RelojFormaRapida(temblorPx: 8, programar: p.programar)
        var paradas = 0
        reloj.alPararse = { paradas += 1 }

        reloj.pulsar(en: CGPoint(x: 0, y: 0), esperaMs: 550)
        XCTAssertEqual(p.vivas.count, 1)
        XCTAssertEqual(p.vivas.first?.segundos ?? 0, 0.55, accuracy: 1e-9)
        XCTAssertTrue(reloj.armado)

        // El temblor de la mano no cuenta como moverse.
        reloj.mover(a: CGPoint(x: 3, y: 4))
        reloj.mover(a: CGPoint(x: 5, y: 5))
        XCTAssertEqual(reloj.programadas, 1)

        // Alejarse reprograma: la vieja se cancela y queda UNA viva.
        reloj.mover(a: CGPoint(x: 40, y: 0))
        XCTAssertEqual(reloj.programadas, 2)
        XCTAssertEqual(p.vivas.count, 1)

        // Vence: avisa una vez y se desarma.
        p.vencer()
        XCTAssertEqual(paradas, 1)
        XCTAssertFalse(reloj.armado)
        p.vencer()
        XCTAssertEqual(paradas, 1, "nada mas que vencer")

        // El siguiente movimiento la vuelve a armar; soltar la cancela.
        reloj.mover(a: CGPoint(x: 41, y: 0))
        XCTAssertTrue(reloj.armado)
        reloj.soltar()
        XCTAssertFalse(reloj.armado)
        XCTAssertTrue(p.vivas.isEmpty)
        p.vencer()
        XCTAssertEqual(paradas, 1, "lo cancelado al soltar no avisa")
    }

    func testSinTrazoAManoNoSeProgramaNada() {
        let p = ProgramadorDePrueba()
        let reloj = RelojFormaRapida(temblorPx: 8, programar: p.programar)
        reloj.pulsar(en: .zero, esperaMs: nil)
        reloj.mover(a: CGPoint(x: 100, y: 100))
        XCTAssertTrue(p.pendientes.isEmpty)
        XCTAssertFalse(reloj.armado)
    }

    func testElGrafitoEsperaElSegundoQueDiceElMotor() {
        let p = ProgramadorDePrueba()
        let reloj = RelojFormaRapida(temblorPx: 8, programar: p.programar)
        reloj.pulsar(en: .zero, esperaMs: 1000)
        XCTAssertEqual(p.vivas.first?.segundos ?? 0, 1.0, accuracy: 1e-9)
    }

    /// Con el editor de verdad: el reloj pide la espera al motor y, al
    /// vencer, la raya quieta se vuelve una linea.
    func testAlVencerElTrazoQuietoSeVuelveFigura() throws {
        let carpeta = try carpetaTemporal("forma-rapida")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        let ruta = carpeta.appendingPathComponent("f.excalidraw").path
        let ed = try EditorLienzo.deFichero(ruta: ruta, crear: true)
        let p = ProgramadorDePrueba()
        let reloj = RelojFormaRapida(temblorPx: CGFloat(formaRapidaTemblorPx()), programar: p.programar)
        var convertido = false
        reloj.alPararse = { convertido = ed.formaRapida() }

        _ = ed.puntero(fase: .pulsar, x: 0, y: 0, presion: 0.5, modificadores: 0, zoom: 1)
        reloj.pulsar(en: .zero, esperaMs: ed.esperaFormaMs())
        for i in 1...40 {
            let x = Float(i) * 5
            _ = ed.puntero(fase: .mover, x: x, y: 0, presion: 0.5, modificadores: 0, zoom: 1)
            reloj.mover(a: CGPoint(x: CGFloat(x), y: 0))
        }
        p.vencer()
        XCTAssertTrue(convertido)
        reloj.soltar()
        _ = ed.puntero(fase: .soltar, x: 200, y: 0, presion: nil, modificadores: 0, zoom: 1)
        XCTAssertTrue(try ed.guardar())
        let json = try String(contentsOfFile: ruta, encoding: .utf8)
        XCTAssertTrue(json.contains("\"line\""))
        XCTAssertFalse(json.contains("\"freedraw\""))
    }
}

final class HerramientasLienzoTests: XCTestCase {
    @MainActor
    func testCadaHerramientaDeLaBarraLlegaAlMotorConSuNombre() throws {
        let carpeta = try carpetaTemporal("herramientas")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        let ed = try EditorLienzo.deFichero(ruta: carpeta.appendingPathComponent("h.excalidraw").path, crear: true)
        let modelo = ModeloEditorLienzo(editor: ed)
        for h in HerramientaLienzo.todas {
            modelo.elegir(h.id)
            XCTAssertNil(modelo.error, h.id)
            XCTAssertEqual(modelo.herramienta, h.id)
            XCTAssertEqual(ed.herramientaActual(), h.id)
        }
        // Las nuevas estan, cada una en su grupo, y las teclas no se pisan.
        let ids = Set(HerramientaLienzo.todas.map(\.id))
        for id in ["lazo", "cota", "escalar", "escala_grafica", "relleno", "recortar", "extender", "punto"] {
            XCTAssertTrue(ids.contains(id), id)
        }
        XCTAssertEqual(HerramientaLienzo.todas.filter { $0.grupo == .construir }.count, 4)
        XCTAssertEqual(HerramientaLienzo.todas.filter { $0.grupo == .medir }.count, 3)
        let teclas = HerramientaLienzo.todas.map(\.tecla)
        XCTAssertEqual(Set(teclas).count, teclas.count)
        // Caso negativo: un nombre que el motor no conoce deja el error.
        modelo.elegir("pincel_magico")
        XCTAssertNotNil(modelo.error)
        XCTAssertEqual(modelo.herramienta, HerramientaLienzo.todas.last?.id)
    }

    @MainActor
    func testElLazoEligeYLaBarraDeEstiloCambiaLoElegido() throws {
        let carpeta = try carpetaTemporal("lazo-estilo")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        let ruta = carpeta.appendingPathComponent("l.excalidraw").path
        let ed = try EditorLienzo.deFichero(ruta: ruta, crear: true)
        let modelo = ModeloEditorLienzo(editor: ed)
        modelo.elegir("rectangulo")
        _ = ed.puntero(fase: .pulsar, x: 10, y: 10, presion: nil, modificadores: 0, zoom: 1)
        _ = ed.puntero(fase: .mover, x: 60, y: 60, presion: nil, modificadores: 0, zoom: 1)
        _ = ed.puntero(fase: .soltar, x: 60, y: 60, presion: nil, modificadores: 0, zoom: 1)
        modelo.elegir("lazo")
        let contorno: [(Float, Float)] = [(0, 0), (100, 0), (100, 100), (0, 100), (0, 1)]
        _ = ed.puntero(fase: .pulsar, x: 0, y: 0, presion: nil, modificadores: 0, zoom: 1)
        for (x, y) in contorno.dropFirst() {
            _ = ed.puntero(fase: .mover, x: x, y: y, presion: nil, modificadores: 0, zoom: 1)
        }
        _ = ed.puntero(fase: .soltar, x: 0, y: 1, presion: nil, modificadores: 0, zoom: 1)
        XCTAssertEqual(ed.cuantosElegidos(), 1)

        let rojo = OpcionesEstiloLienzo.trazos[1].valor
        modelo.cambiarEstilo(.trazo(color: rojo))
        XCTAssertTrue(OpcionesEstiloLienzo.igual(modelo.estilo.trazo, rojo))
        modelo.cambiarEstilo(.grosor(grosor: .grueso))
        XCTAssertEqual(OpcionesEstiloLienzo.nombre(modelo.estilo.grosor), "Grueso")
        modelo.cambiarEstilo(.linea(linea: .discontinua))
        modelo.cambiarEstilo(.fondo(color: OpcionesEstiloLienzo.fondos[1].valor))
        modelo.cambiarEstilo(.relleno(relleno: .solido))
        XCTAssertTrue(try ed.guardar())
        let json = try String(contentsOfFile: ruta, encoding: .utf8)
        XCTAssertTrue(json.contains("#e03131"), "el trazo rojo de la barra")
        XCTAssertTrue(json.contains("\"dashed\""))
        XCTAssertTrue(json.contains("#ffc9c9"))
        XCTAssertTrue(json.contains("\"fillStyle\" : \"solid\"") || json.contains("\"fillStyle\": \"solid\""))
    }

    func testLasOpcionesDeLaBarraYSusColores() {
        XCTAssertEqual(OpcionesEstiloLienzo.trazos.count, 5)
        XCTAssertNil(OpcionesEstiloLienzo.fondos[0].valor, "el primero es transparente")
        let c = OpcionesEstiloLienzo.hex(0x1971c2)
        XCTAssertEqual(c.r, Float(0x19) / 255, accuracy: 1e-6)
        XCTAssertEqual(c.b, Float(0xc2) / 255, accuracy: 1e-6)
        let ida = OpcionesEstiloLienzo.estilo(de: OpcionesEstiloLienzo.color(c))
        XCTAssertTrue(OpcionesEstiloLienzo.igual(ida, c))
        XCTAssertFalse(OpcionesEstiloLienzo.igual(c, nil))
        XCTAssertEqual(Set(OpcionesEstiloLienzo.rellenos.map(\.id)).count, 5)
    }

    func testLosNumerosDeLosCajetines() {
        XCTAssertEqual(CajetinMedida.numero("2,5"), 2.5)
        XCTAssertEqual(CajetinMedida.numero(" 3.25 "), 3.25)
        XCTAssertNil(CajetinMedida.numero("dos"))
        XCTAssertNil(CajetinMedida.numero(""))
    }

    func testLaCotaYCalibrarPorElPuente() throws {
        let carpeta = try carpetaTemporal("cota")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        let ed = try EditorLienzo.deFichero(ruta: carpeta.appendingPathComponent("c.excalidraw").path, crear: true)
        try ed.herramienta(nombre: "escalar")
        _ = ed.puntero(fase: .pulsar, x: 0, y: 0, presion: nil, modificadores: 0, zoom: 1)
        _ = ed.puntero(fase: .mover, x: 100, y: 0, presion: nil, modificadores: 0, zoom: 1)
        _ = ed.puntero(fase: .soltar, x: 100, y: 0, presion: nil, modificadores: 0, zoom: 1)
        guard case let .calibrar(largoPx)? = ed.tomarPeticion() else {
            return XCTFail("tenia que pedir la medida")
        }
        XCTAssertTrue(ed.calibrar(largoPx: largoPx, valor: 50, unidad: "cm"))
        try ed.herramienta(nombre: "cota")
        _ = ed.puntero(fase: .pulsar, x: 0, y: 50, presion: nil, modificadores: 0, zoom: 1)
        _ = ed.puntero(fase: .mover, x: 200, y: 50, presion: nil, modificadores: 0, zoom: 1)
        _ = ed.puntero(fase: .soltar, x: 200, y: 50, presion: nil, modificadores: 0, zoom: 1)
        guard case let .dictarCota(id, largo, _, unidad)? = ed.tomarPeticion() else {
            return XCTFail("la cota tenia que pedir su medida")
        }
        XCTAssertEqual(unidad, "cm")
        XCTAssertEqual(largo, 100, accuracy: 0.01)
        XCTAssertTrue(ed.dictarCota(id: id, largo: 25, grados: 0))
        XCTAssertEqual(ed.cuantosElementos(), 1)
    }

    /// Una imagen pequena de verdad, en TIFF (lo que deja una captura en el
    /// portapapeles).
    private func tiff(ancho: Int, alto: Int) throws -> Data {
        let rep = try XCTUnwrap(NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: ancho, pixelsHigh: alto,
                                                 bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true,
                                                 isPlanar: false, colorSpaceName: .deviceRGB,
                                                 bytesPerRow: 0, bitsPerPixel: 0))
        for y in 0..<alto {
            for x in 0..<ancho {
                rep.setColor(NSColor(red: CGFloat(x) / CGFloat(ancho), green: 0.5, blue: 0.2, alpha: 1), atX: x, y: y)
            }
        }
        return try XCTUnwrap(rep.tiffRepresentation)
    }

    func testUnaImagenPegadaSeGuardaEnElChatConSuEntradaEnFiles() throws {
        let raiz = try carpetaTemporal("pegar")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let nucleo = try Nucleo.abrir(raiz: raiz.path, nombreDelAparato: "Mac")
        let chat = try nucleo.crearChat(nombre: "Obra")
        let referencia = try XCTUnwrap(try nucleo.crearLienzo(chat: chat.id, nombre: nil).referencia)
        let ed = try nucleo.editarLienzo(chat: chat.id, dibujo: referencia)

        let png = try XCTUnwrap(ImagenPegadaLienzo.png(de: try tiff(ancho: 40, alto: 30)))
        XCTAssertEqual(Array(png.prefix(4)), [0x89, 0x50, 0x4e, 0x47], "sale un PNG")
        // Un PNG que ya cabe pasa tal cual, sin recodificar.
        XCTAssertEqual(ImagenPegadaLienzo.png(de: png), png)
        XCTAssertNil(ImagenPegadaLienzo.png(de: Data("no soy una foto".utf8)))

        let foto = try ed.pegarImagen(png: png, centroX: 100, centroY: 100, anchoVista: 800, altoVista: 600)
        XCTAssertTrue(FileManager.default.fileExists(atPath: foto.ruta))
        XCTAssertEqual(ed.herramientaActual(), "seleccion")
        XCTAssertTrue(try ed.guardar())

        let json = try JSONSerialization.jsonObject(with: Data(contentsOf: URL(fileURLWithPath: ed.ruta())))
            as? [String: Any]
        let elementos = try XCTUnwrap(json?["elements"] as? [[String: Any]])
        let imagen = try XCTUnwrap(elementos.first { $0["type"] as? String == "image" })
        let clave = try XCTUnwrap(imagen["fileId"] as? String)
        XCTAssertTrue(clave.hasPrefix("pc"))
        let files = try XCTUnwrap(json?["files"] as? [String: Any])
        let entrada = try XCTUnwrap(files[clave] as? [String: Any])
        XCTAssertEqual(entrada["mimeType"] as? String, "image/png")
        XCTAssertEqual(entrada["path"] as? String, "imagenes/\(clave)")

        // Al reabrir, la foto sale con su ruta en la carpeta del chat.
        let otra = try nucleo.editarLienzo(chat: chat.id, dibujo: referencia)
        let fotos = otra.imagenes()
        XCTAssertEqual(fotos.count, 1)
        XCTAssertEqual(fotos.first?.idObjeto, foto.idObjeto)
        XCTAssertTrue(fotos.first?.ruta.hasSuffix("imagenes/\(clave)") ?? false)
        XCTAssertEqual(try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(fotos.first?.ruta))), png)
    }

    @MainActor
    func testElCursorDelTextoColocaLaVentanitaDelMetodoDeEntrada() throws {
        let carpeta = try carpetaTemporal("ime")
        defer { try? FileManager.default.removeItem(at: carpeta) }
        let ed = try EditorLienzo.deFichero(ruta: carpeta.appendingPathComponent("t.excalidraw").path, crear: true)
        let modelo = ModeloEditorLienzo(editor: ed)
        let vista = VistaEditorLienzoNS(modelo: modelo)
        vista.frame = NSRect(x: 0, y: 0, width: 400, height: 300)
        XCTAssertNil(vista.rectanguloDelCursor())
        modelo.elegir("texto")
        _ = ed.puntero(fase: .pulsar, x: 150, y: 120, presion: nil, modificadores: 0, zoom: 1)
        _ = ed.puntero(fase: .soltar, x: 150, y: 120, presion: nil, modificadores: 0, zoom: 1)
        _ = ed.teclear(texto: "Hola")
        let r = try XCTUnwrap(vista.rectanguloDelCursor())
        // La vista sin encajar: origen 0 y zoom 1, el cursor detras de «Hola».
        XCTAssertGreaterThan(r.minX, 150)
        XCTAssertGreaterThanOrEqual(r.minY, 115)
        XCTAssertLessThan(r.minY, 160)
        XCTAssertGreaterThan(r.height, 5)
        // Y la cuenta del mundo a la vista con camara.
        let c = CajaLienzo(x0: 10, y0: 20, x1: 10, y1: 40)
        let v = VistaEditorLienzoNS.aVista(c, origen: CGPoint(x: 5, y: 10), zoom: 2)
        XCTAssertEqual(v, NSRect(x: 10, y: 20, width: 1, height: 40))
    }
}
