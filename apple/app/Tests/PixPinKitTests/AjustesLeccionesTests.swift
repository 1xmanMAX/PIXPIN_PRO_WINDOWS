// Ajustes, lecciones y asistente con el puente de verdad (el nucleo de Rust
// enlazado), cada prueba en su carpeta temporal.

import Carbon.HIToolbox
import Foundation
import PixPinCore
@testable import PixPinKit
import XCTest

final class AjustesTests: XCTestCase {
    func testPixpinmaxTomlDeIdaYVueltaConservandoLoDeWindows() throws {
        let dir = try carpetaTemporal("ajustes")
        let toml = dir.appendingPathComponent("pixpinmax.toml")
        try "# de Windows\narranque_con_windows = false\n\n[capturas]\ndias_caducidad = 7\n"
            .write(to: toml, atomically: true, encoding: .utf8)
        let nucleo = try Nucleo.abrir(raiz: dir.path, nombreDelAparato: "Mac de prueba")
        let a = try nucleo.ajustes()
        XCTAssertEqual(a.diasCaducidad, 7)
        try nucleo.guardarAjustes(nuevos: AjustesMac(diasCaducidad: 21, sincroAutomatica: false, sincroPresencia: true))
        let b = try nucleo.ajustes()
        XCTAssertEqual(b.diasCaducidad, 21)
        XCTAssertFalse(b.sincroAutomatica)
        let texto = try String(contentsOf: toml, encoding: .utf8)
        XCTAssertTrue(texto.contains("# de Windows"))
        XCTAssertTrue(texto.contains("arranque_con_windows = false"))
        XCTAssertEqual(nucleo.rutaAjustes(), toml.path)
    }

    func testUnTomlRotoNoSePisa() throws {
        let dir = try carpetaTemporal("ajustes-roto")
        let toml = dir.appendingPathComponent("pixpinmax.toml")
        try "esto = = roto".write(to: toml, atomically: true, encoding: .utf8)
        let nucleo = try Nucleo.abrir(raiz: dir.path, nombreDelAparato: "Mac de prueba")
        XCTAssertThrowsError(try nucleo.guardarAjustes(nuevos: AjustesMac(diasCaducidad: 1, sincroAutomatica: true, sincroPresencia: true)))
        XCTAssertEqual(try String(contentsOf: toml, encoding: .utf8), "esto = = roto")
    }

    @MainActor
    func testElModeloLeeYGuardaFueraDelHiloPrincipal() async throws {
        let dir = try carpetaTemporal("ajustes-modelo")
        let nucleo = try Nucleo.abrir(raiz: dir.path, nombreDelAparato: "Mac de prueba")
        let defaults = try XCTUnwrap(UserDefaults(suiteName: "pixpin-pruebas-\(UUID().uuidString)"))
        let modelo = ModeloAjustes(nucleo: nucleo, defaults: defaults)
        await modelo.cargar()
        XCTAssertTrue(modelo.cargado)
        XCTAssertEqual(modelo.diasCaducidad, 7)
        XCTAssertEqual(modelo.tema, .sistema)
        XCTAssertEqual(modelo.carpetaDatos.standardizedFileURL.path, dir.standardizedFileURL.path)
        modelo.fijar(diasCaducidad: 3)
        await modelo.guardar()
        XCTAssertNil(modelo.error)
        XCTAssertEqual(try nucleo.ajustes().diasCaducidad, 3)
        modelo.tema = .oscuro
        XCTAssertEqual(Tema.guardado(defaults), .oscuro)
    }

    func testElAtajoSeEnsenaYSeValida() {
        XCTAssertEqual(AtajoGlobal.Combinacion.capturar.descripcion, "⌃⌥X")
        let m = AtajoGlobal.Combinacion.modificadores(de: [.command, .shift])
        XCTAssertEqual(m, UInt32(cmdKey | shiftKey))
        XCTAssertEqual(AtajoGlobal.Combinacion(tecla: UInt32(kVK_ANSI_5), modificadores: m).descripcion, "⇧⌘5")
        XCTAssertTrue(AtajoDeCaptura.vale(.capturar))
        XCTAssertFalse(AtajoDeCaptura.vale(AtajoGlobal.Combinacion(tecla: UInt32(kVK_ANSI_A), modificadores: UInt32(shiftKey))))
    }

    @MainActor
    func testElAtajoSeGuardaEnUserDefaults() throws {
        let defaults = try XCTUnwrap(UserDefaults(suiteName: "pixpin-pruebas-\(UUID().uuidString)"))
        let atajo = AtajoDeCaptura(defaults: defaults, registrar: false) {}
        XCTAssertEqual(atajo.combinacion, .capturar)
        let nueva = AtajoGlobal.Combinacion(tecla: UInt32(kVK_ANSI_S), modificadores: UInt32(cmdKey | shiftKey))
        XCTAssertTrue(atajo.cambiar(a: nueva))
        XCTAssertEqual(AtajoDeCaptura(defaults: defaults, registrar: false) {}.combinacion, nueva)
        XCTAssertTrue(atajo.restablecer())
        XCTAssertEqual(AtajoDeCaptura(defaults: defaults, registrar: false) {}.combinacion, .capturar)
    }
}

final class LeccionesTests: XCTestCase {
    func borrador(_ titulo: String) -> LeccionEditada {
        var b = BorradorLeccion()
        b.titulo = titulo
        return b.editada
    }

    func testSeGuardaComoEnWindowsYAndroid() throws {
        let dir = try carpetaTemporal("lecciones")
        let nucleo = try Nucleo.abrir(raiz: dir.path, nombreDelAparato: "Mac de prueba")
        var b = BorradorLeccion()
        b.titulo = "Revisar el encofrado antes de vaciar"
        b.etiquetasEscritas = "obra, #encofrado"
        let f = try nucleo.guardarLeccion(editada: b.editada)
        XCTAssertEqual(f.etiquetas, ["obra", "encofrado"])
        let guardados = try nucleo.chatGuardados()
        XCTAssertEqual(f.chat, guardados.id)
        let archivo = dir.appendingPathComponent("proyectos/\(guardados.id)/android/guardados/lecciones/\(f.id).leccion")
        XCTAssertTrue(FileManager.default.fileExists(atPath: archivo.path))
        let json = try JSONSerialization.jsonObject(with: Data(contentsOf: archivo)) as? [String: Any]
        XCTAssertEqual(json?["titulo"] as? String, "Revisar el encofrado antes de vaciar")
        XCTAssertNotNil(json?["quePaso"])
        let pagina = try nucleo.ultimosMensajes(chat: guardados.id, cuantos: 10)
        let m = try XCTUnwrap(pagina.mensajes.first { $0.id == "lec-\(f.id)" })
        XCTAssertEqual(m.clase, "ARCHIVO")
        XCTAssertEqual(m.ruta, "pixpin:files/guardados/lecciones/\(f.id).leccion")
    }

    func testBuscarFiltrarRepasarYBorrar() throws {
        let dir = try carpetaTemporal("lecciones-lista")
        let nucleo = try Nucleo.abrir(raiz: dir.path, nombreDelAparato: "Mac de prueba")
        for i in 0 ..< 5 {
            var b = BorradorLeccion()
            b.titulo = "Lección número \(i)"
            if i < 2 { b.etiquetas = ["casa"] }
            _ = try nucleo.guardarLeccion(editada: b.editada)
        }
        let p = try nucleo.guardarLeccion(editada: borrador("Llevar paraguas si llueve"))
        let todas = try nucleo.lecciones(filtro: FiltroLecciones(texto: "", etiqueta: nil, soloParaRepasar: false), desde: 0, cuantas: 4)
        XCTAssertEqual(todas.total, 6)
        XCTAssertEqual(todas.lecciones.count, 4)
        XCTAssertEqual(todas.etiquetas.first?.etiqueta, "casa")
        let casa = try nucleo.lecciones(filtro: FiltroLecciones(texto: "", etiqueta: "casa", soloParaRepasar: false), desde: 0, cuantas: 50)
        XCTAssertEqual(casa.total, 2)
        let busca = try nucleo.lecciones(filtro: FiltroLecciones(texto: "paraguas", etiqueta: nil, soloParaRepasar: false), desde: 0, cuantas: 50)
        XCTAssertEqual(busca.lecciones.first?.id, p.id)
        let r = try nucleo.repasarLeccion(id: p.id, nota: .recordaba)
        XCTAssertEqual(r.caja, 1)
        let otra = try nucleo.repasarLeccion(id: p.id, nota: .repetida)
        XCTAssertEqual(otra.veces, 2)
        try nucleo.borrarLeccion(id: p.id)
        XCTAssertThrowsError(try nucleo.leccion(id: p.id))
        XCTAssertEqual(try nucleo.lecciones(filtro: FiltroLecciones(texto: "", etiqueta: nil, soloParaRepasar: false), desde: 0, cuantas: 50).total, 5)
    }

    @MainActor
    func testElModeloPaginaYGuarda() async throws {
        let dir = try carpetaTemporal("lecciones-modelo")
        let nucleo = try Nucleo.abrir(raiz: dir.path, nombreDelAparato: "Mac de prueba")
        let modelo = ModeloLecciones(nucleo: nucleo)
        await modelo.recargar()
        XCTAssertTrue(modelo.lecciones.isEmpty)
        for i in 0 ..< Int(ModeloLecciones.tamPagina) + 5 {
            _ = try nucleo.guardarLeccion(editada: borrador("Lección \(i)"))
        }
        await modelo.recargar()
        XCTAssertEqual(modelo.lecciones.count, Int(ModeloLecciones.tamPagina))
        XCTAssertTrue(modelo.quedanMas)
        await modelo.cargarMas()
        XCTAssertEqual(modelo.lecciones.count, Int(ModeloLecciones.tamPagina) + 5)
        XCTAssertFalse(modelo.quedanMas)
        let f = await modelo.guardar(borrador("Una más desde el modelo"))
        XCTAssertNotNil(f)
        XCTAssertEqual(modelo.total, UInt64(ModeloLecciones.tamPagina) + 6)
        let vacia = await modelo.guardar(borrador("   "))
        XCTAssertNil(vacia)
        XCTAssertNotNil(modelo.error)
    }

    func testElBorradorAceptaYQuitaPropuestas() {
        var b = BorradorLeccion()
        b.etiquetasAuto = ["obra", "prisa"]
        b.aceptar("obra")
        b.quitar("prisa")
        XCTAssertEqual(b.etiquetas, ["obra"])
        XCTAssertEqual(b.quitadas, ["prisa"])
        XCTAssertTrue(b.etiquetasAuto.isEmpty)
    }
}

final class AsistenteTests: XCTestCase {
    func testLasHerramientasCruzanComoJson() throws {
        let catalogo = try XCTUnwrap(iaCatalogoPara(pedido: "dibuja un círculo rojo", conImagen: false).data(using: .utf8))
        let lista = try XCTUnwrap(try JSONSerialization.jsonObject(with: catalogo) as? [[String: Any]])
        XCTAssertFalse(lista.isEmpty)
        let r = iaLlamar(nombre: "dibujar_en_lienzo",
                         argumentos: #"{"elementos":[{"tipo":"elipse","x":0,"y":0,"ancho":40,"alto":40,"color":"rojo"}]}"#,
                         origenX: 0, origenY: 0, semilla: 1)
        let rec = Recolector()
        XCTAssertTrue(rec.apuntar(nombre: "dibujar_en_lienzo", respuesta: r).hasPrefix("dibujadas"))
        let lienzo = try XCTUnwrap(rec.lienzo())
        XCTAssertNoThrow(try Lienzo.deJson(texto: lienzo))
        XCTAssertTrue(rec.apuntar(nombre: "x", respuesta: iaLlamar(nombre: "borrar_todo", argumentos: "{}", origenX: 0, origenY: 0, semilla: 2)).hasPrefix("ERROR"))
    }

    @MainActor
    func testSinMotorSeDiceClaro() async throws {
        // Un puerto que nadie escucha y sin Foundation Models.
        let modelo = ModeloAsistente(almacen: nil, puertoLlama: 9, usarApple: false)
        let motor = await modelo.comprobarMotor()
        XCTAssertEqual(motor, .ninguno)
        await modelo.enviar("hola")
        XCTAssertEqual(modelo.burbujas.count, 2)
        XCTAssertTrue(modelo.burbujas[1].esAviso)
        XCTAssertEqual(modelo.burbujas[1].texto, ModeloAsistente.textoSinMotor)
        XCTAssertFalse(llamaLocalActivo(puerto: 9))
    }

    func testElHistorialVaDelanteDelPedido() {
        XCTAssertEqual(textoConHistorial("hola", historial: []), "hola")
        let t = textoConHistorial("y ahora?", historial: [MensajeIa(delUsuario: true, texto: "dibuja"),
                                                         MensajeIa(delUsuario: false, texto: "hecho")])
        XCTAssertTrue(t.contains("Usuario: dibuja"))
        XCTAssertTrue(t.hasSuffix("Pedido nuevo: y ahora?"))
    }
}
