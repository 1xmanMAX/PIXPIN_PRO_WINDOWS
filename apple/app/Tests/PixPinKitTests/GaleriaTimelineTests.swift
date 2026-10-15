// La galeria de capturas, el timeline, las tareas y buscar en todo por sus
// modelos de la app, cruzando el puente de verdad (el nucleo de Rust
// enlazado) sobre carpetas de datos temporales.

import Foundation
import PixPinCore
@testable import PixPinKit
import XCTest

@MainActor
final class GaleriaTimelineTests: XCTestCase {
    private func almacenTemporal(_ nombre: String) throws -> (Almacen, URL) {
        let raiz = try carpetaTemporal(nombre)
        let almacen = try Almacen(raiz: raiz.appendingPathComponent("datos"), nombreDelAparato: "Mac de prueba")
        return (almacen, raiz)
    }

    // MARK: Galeria

    func testLaGaleriaConservaAlargaBorraYDeshace() async throws {
        let (almacen, raiz) = try almacenTemporal("galeria")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let capturas = URL(fileURLWithPath: almacen.nucleo.carpetaCapturas(), isDirectory: true)
        try FileManager.default.createDirectory(at: capturas, withIntermediateDirectories: true)
        for n in ["captura-a.png", "captura-b.png", "captura-c.png"] {
            try escribirPNG(capturas.appendingPathComponent(n), ancho: 64, alto: 48)
        }
        try Data("no".utf8).write(to: capturas.appendingPathComponent("notas.txt"))

        let modelo = ModeloGaleria(almacen: almacen)
        modelo.dias = { 7 }
        await modelo.recargar()
        XCTAssertNil(modelo.error)
        XCTAssertEqual(modelo.total, 3, "el txt no es una captura")
        XCTAssertEqual(Set(modelo.capturas.map(\.nombre)), ["captura-a.png", "captura-b.png", "captura-c.png"])
        XCTAssertTrue(modelo.capturas.allSatisfy { $0.diasQueQuedan == 7 && !$0.conservada })
        // La miniatura de la rejilla sale de la cache comun, reducida.
        let primera = try XCTUnwrap(modelo.capturas.first)
        let mini = await Miniaturas.compartidas.cargar(modelo.url(de: primera), lado: MiniaturaCaptura.lado)
        XCTAssertNotNil(mini)

        // Conservar: a «Mensajes guardados» y deja de caducar.
        await modelo.conservar(["captura-a.png"])
        XCTAssertNil(modelo.error)
        let a = try XCTUnwrap(modelo.capturas.first { $0.nombre == "captura-a.png" })
        XCTAssertTrue(a.conservada)
        XCTAssertNil(a.seVa)
        let guardados = try almacen.nucleo.chatGuardados()
        let pagina = try almacen.nucleo.ultimosMensajes(chat: guardados.id, cuantos: 10)
        XCTAssertEqual(pagina.mensajes.map(\.nombre), ["captura-a.png"])
        // Dos veces no la manda dos veces.
        await modelo.conservar(["captura-a.png"])
        XCTAssertEqual(modelo.aviso, "Ya estaban conservadas")
        XCTAssertEqual(try almacen.nucleo.ultimosMensajes(chat: guardados.id, cuantos: 10).total, 1)

        // «7 dias mas».
        let antes = try XCTUnwrap(modelo.capturas.first { $0.nombre == "captura-b.png" }?.seVa)
        await modelo.prorrogar(["captura-b.png"])
        let despues = try XCTUnwrap(modelo.capturas.first { $0.nombre == "captura-b.png" }?.seVa)
        XCTAssertEqual(despues, antes + 7 * 86_400_000)

        // Borrar va a la papelera de PixPin y se deshace.
        await modelo.borrar(["captura-b.png", "captura-c.png"])
        XCTAssertEqual(modelo.total, 1)
        XCTAssertEqual(modelo.ultimoBorrado.count, 2)
        let papelera = raiz.appendingPathComponent("datos/papelera/capturas/captura-b.png")
        XCTAssertTrue(FileManager.default.fileExists(atPath: papelera.path))
        await modelo.deshacer()
        XCTAssertEqual(modelo.total, 3)
        XCTAssertTrue(modelo.ultimoBorrado.isEmpty)
        // Vuelve con su prorroga.
        XCTAssertEqual(modelo.capturas.first { $0.nombre == "captura-b.png" }?.seVa, despues)
    }

    func testLaGaleriaLlegaPorPaginas() async throws {
        let (almacen, raiz) = try almacenTemporal("galeria-paginas")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let capturas = URL(fileURLWithPath: almacen.nucleo.carpetaCapturas(), isDirectory: true)
        try FileManager.default.createDirectory(at: capturas, withIntermediateDirectories: true)
        let cuantas = Int(ModeloGaleria.tamPagina) + 5
        for i in 0..<cuantas {
            try Data([0x89, 0x50, 0x4e, 0x47]).write(to: capturas.appendingPathComponent(String(format: "c-%04d.png", i)))
        }
        let modelo = ModeloGaleria(almacen: almacen)
        await modelo.recargar()
        XCTAssertEqual(modelo.capturas.count, Int(ModeloGaleria.tamPagina))
        XCTAssertTrue(modelo.quedanMas)
        await modelo.cargarMas()
        XCTAssertEqual(modelo.capturas.count, cuantas)
        XCTAssertFalse(modelo.quedanMas)
        XCTAssertEqual(Set(modelo.capturas.map(\.nombre)).count, cuantas, "sin repetidas")
    }

    // MARK: Timeline

    func testElTimelineDaLosDiasYSusMomentos() async throws {
        let (almacen, raiz) = try almacenTemporal("timeline")
        defer { try? FileManager.default.removeItem(at: raiz) }
        // `timeline/momentos.jsonl`, una linea por momento, como Windows.
        let carpeta = raiz.appendingPathComponent("datos/timeline", isDirectory: true)
        try FileManager.default.createDirectory(at: carpeta, withIntermediateDirectories: true)
        let ahora = Int64(Date().timeIntervalSince1970 * 1000)
        let hace10Dias = ahora - 10 * 86_400_000
        let lineas = [
            #"{"id":"tl-1-0","cuando":\#(ahora - 60_000),"titulo":"Llegó el yeso","descripcion":"diez sacos"}"#,
            #"{"id":"tl-2-0","cuando":\#(ahora - 30_000),"titulo":"Reunión","enMomentos":true}"#,
            #"{"id":"tl-3-0","cuando":\#(hace10Dias),"titulo":"Antiguo","fotos":["tl-3-0-1.png"]}"#,
            #"{"id":"roto"#,
        ]
        try (lineas.joined(separator: "\n") + "\n").write(to: carpeta.appendingPathComponent("momentos.jsonl"),
                                                          atomically: true, encoding: .utf8)

        let modelo = ModeloTimeline(nucleo: almacen.nucleo)
        await modelo.recargar()
        XCTAssertNil(modelo.error)
        // «Hoy»: las ultimas 24 horas, la mas vieja primero.
        XCTAssertEqual(modelo.eleccion, .hoy)
        XCTAssertEqual(modelo.momentos.map(\.titulo), ["Llegó el yeso", "Reunión"])
        XCTAssertEqual(modelo.dias.count, 2)
        // El dia viejo, el ultimo de la lista (el mas nuevo arriba).
        let viejo = try XCTUnwrap(modelo.dias.last)
        await modelo.elegir(.dia(viejo.iso))
        XCTAssertEqual(modelo.momentos.map(\.titulo), ["Antiguo"])
        XCTAssertTrue(try XCTUnwrap(modelo.momentos.first?.fotos.first).hasSuffix("timeline/medios/tl-3-0-1.png"))
        // Solo «Momentos».
        modelo.filtro = .momentos
        await modelo.recargar()
        XCTAssertEqual(modelo.dias.count, 1)
        XCTAssertEqual(ModeloTimeline.partes("2026-10-15")?.1, 10)
        XCTAssertNil(ModeloTimeline.partes("ayer"))
    }

    // MARK: Tareas

    func testLasTareasSeApuntanYSeMarcanEnSuMensaje() async throws {
        let (almacen, raiz) = try almacenTemporal("tareas")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let modelo = ModeloTareas(nucleo: almacen.nucleo)
        await modelo.cargar()
        XCTAssertTrue(modelo.listas.isEmpty)

        await modelo.apuntar("comprar yeso")
        await modelo.apuntar("llamar al fontanero")
        XCTAssertNil(modelo.error)
        XCTAssertEqual(modelo.listas.count, 1)
        let inbox = try XCTUnwrap(modelo.listas.first)
        XCTAssertTrue(inbox.esInbox)
        XCTAssertEqual(inbox.tareas.map(\.texto), ["comprar yeso", "llamar al fontanero"])
        // Recien creadas: grupo «Hoy».
        XCTAssertEqual(modelo.pendientesPorGrupo.map(\.grupo), [.hoy])
        XCTAssertEqual(modelo.pendientesPorGrupo.first?.tareas.count, 2)

        let yeso = TareaConLista(lista: inbox, tarea: inbox.tareas[0])
        await modelo.marcar(yeso, hecha: true)
        XCTAssertNil(modelo.error)
        XCTAssertEqual(modelo.hechas.map(\.tarea.texto), ["comprar yeso"])
        // El mensaje del chat lleva la casilla marcada (lo que viaja al movil).
        let guardados = try almacen.nucleo.chatGuardados()
        let lista = try XCTUnwrap(try almacen.nucleo.ultimosMensajes(chat: guardados.id, cuantos: 10)
            .mensajes.first { $0.id == inbox.mensaje })
        XCTAssertTrue(lista.texto.contains("- [x] comprar yeso"), lista.texto)

        // Con un texto que ya no es el de esa tarea, no se toca nada.
        let vieja = TareaConLista(lista: inbox, tarea: Tarea(indice: 1, crudo: "otra cosa", texto: "otra cosa",
                                                              hecha: false, creada: nil, dias: nil, imagenes: []))
        await modelo.marcar(vieja, hecha: true)
        XCTAssertNotNil(modelo.error)
        XCTAssertEqual(modelo.hechas.count, 1)
    }

    func testLosGruposDeTareasPorLoQueLlevan() {
        func t(_ dias: UInt32?) -> Tarea {
            Tarea(indice: 0, crudo: "", texto: "", hecha: false, creada: nil, dias: dias, imagenes: [])
        }
        XCTAssertEqual(GrupoDeTareas.de(t(nil)), .sinFecha)
        XCTAssertEqual(GrupoDeTareas.de(t(0)), .hoy)
        XCTAssertEqual(GrupoDeTareas.de(t(7)), .estaSemana)
        XCTAssertEqual(GrupoDeTareas.de(t(20)), .esteMes)
        XCTAssertEqual(GrupoDeTareas.de(t(90)), .masDeUnMes)
    }

    // MARK: Buscar

    func testBuscarEsperaCancelaLoAnteriorYDaLaPosicion() async throws {
        let (almacen, raiz) = try almacenTemporal("buscar")
        defer { try? FileManager.default.removeItem(at: raiz) }
        await almacen.crearChat(nombre: "Obra")
        let chat = try XCTUnwrap(almacen.chats.first { $0.nombre == "Obra" }).id
        for i in 0..<60 {
            _ = try almacen.nucleo.escribirNota(chat: chat, texto: "nota \(i)", respondeA: nil)
        }
        let nota = try almacen.nucleo.escribirNota(chat: chat, texto: "Falta el yeso de la cocina", respondeA: nil)

        let modelo = ModeloBuscar(almacen: almacen)
        // Dos letras seguidas: solo cuenta la ultima consulta.
        modelo.cambiar("cocinero")
        let primera = try XCTUnwrap(modelo.tarea)
        modelo.cambiar("yeso cocina")
        XCTAssertTrue(primera.isCancelled)
        _ = await modelo.tarea?.value
        XCTAssertEqual(modelo.consulta, "yeso cocina")
        XCTAssertFalse(modelo.buscando)
        let r = try XCTUnwrap(modelo.resultados.first { $0.clase == .mensaje })
        XCTAssertEqual(r.mensaje, nota.id)
        XCTAssertEqual(r.chat, chat)
        // La posicion lleva a la pagina donde esta.
        let pos = try XCTUnwrap(r.posicion)
        let pagina = try almacen.nucleo.mensajes(chat: chat, desde: pos, cuantos: 1)
        XCTAssertEqual(pagina.mensajes.first?.id, nota.id)

        // Por paginas: 60 notas, 50 en la primera.
        await modelo.buscarAhora("nota")
        XCTAssertEqual(modelo.resultados.count, Int(ModeloBuscar.tamPagina))
        XCTAssertTrue(modelo.hayMas)
        await modelo.cargarMas()
        XCTAssertEqual(modelo.resultados.count, 60)
        XCTAssertFalse(modelo.hayMas)
        XCTAssertEqual(Set(modelo.resultados.compactMap(\.mensaje)).count, 60, "sin repetidos")
        // Vaciar la caja vacia la lista sin buscar.
        modelo.cambiar("  ")
        XCTAssertTrue(modelo.resultados.isEmpty)
        XCTAssertNil(modelo.tarea)

        // Una busqueda cancelada en Rust lo dice.
        let b = Busqueda.nueva()
        b.cancelar()
        let cancelada = try almacen.nucleo.buscar(consulta: "nota", desde: 0, cuantos: 10, busqueda: b)
        XCTAssertTrue(cancelada.cancelada)
    }
}
