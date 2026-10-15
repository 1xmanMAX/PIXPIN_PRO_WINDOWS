// Las capturas caducadas a la Papelera, saltar a un mensaje desde Buscar,
// las claves en el Llavero, el asistente en vivo (SSE de un llama-server
// falso) y las acciones de Atajos. Cruzan el puente de verdad sobre
// carpetas temporales; nada pide permisos de pantalla ni de micro.

import Foundation
import Network
import PixPinCore
@testable import PixPinKit
import XCTest

// MARK: - Capturas caducadas

final class BarrenderoCapturasTests: XCTestCase {
    private func viejo(_ url: URL, dias: Double) throws {
        let fecha = Date().addingTimeInterval(-dias * 86_400)
        try FileManager.default.setAttributes([.modificationDate: fecha], ofItemAtPath: url.path)
    }

    /// Una carpeta de datos con tres capturas: una vieja, una vieja pero
    /// conservada y una de hoy. El registro dice que la caducidad se
    /// estreno hace mucho (como un Windows que ya la usaba).
    private func preparar(_ nombre: String) throws -> (Nucleo, URL, URL) {
        let raiz = try carpetaTemporal(nombre)
        let datos = raiz.appendingPathComponent("datos", isDirectory: true)
        let nucleo = try Nucleo.abrir(raiz: datos.path, nombreDelAparato: "Mac de prueba")
        let capturas = URL(fileURLWithPath: nucleo.carpetaCapturas(), isDirectory: true)
        try FileManager.default.createDirectory(at: capturas, withIntermediateDirectories: true)
        for n in ["vieja.png", "guardada.png", "nueva.png"] {
            try Data("png".utf8).write(to: capturas.appendingPathComponent(n))
        }
        try viejo(capturas.appendingPathComponent("vieja.png"), dias: 8)
        try viejo(capturas.appendingPathComponent("guardada.png"), dias: 30)
        // El mismo fichero que Windows y Android (`capturas-caducidad.json`).
        let registro = #"{"desde": 0, "conservadas": ["guardada.png", "borrada-a-mano.png"]}"#
        try Data(registro.utf8).write(to: datos.appendingPathComponent("capturas-caducidad.json"))
        return (nucleo, raiz, capturas)
    }

    func testLasCaducadasVanALaPapeleraYLasConservadasSeQuedan() throws {
        let (nucleo, raiz, capturas) = try preparar("barrer")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let papelera = raiz.appendingPathComponent("papelera-falsa", isDirectory: true)
        try FileManager.default.createDirectory(at: papelera, withIntermediateDirectories: true)

        let barrendero = BarrenderoCapturas(nucleo: nucleo)
        barrendero.dias = { 7 }
        barrendero.tirar = { url in
            try FileManager.default.moveItem(at: url, to: papelera.appendingPathComponent(url.lastPathComponent))
        }
        let hecho = barrendero.barrer()
        XCTAssertEqual(hecho.idas, ["vieja.png"])
        XCTAssertTrue(hecho.fallos.isEmpty)
        XCTAssertEqual(hecho.olvidadas, 1, "la conservada borrada a mano se olvida")
        XCTAssertFalse(FileManager.default.fileExists(atPath: capturas.appendingPathComponent("vieja.png").path))
        XCTAssertTrue(FileManager.default.fileExists(atPath: papelera.appendingPathComponent("vieja.png").path))
        XCTAssertTrue(FileManager.default.fileExists(atPath: capturas.appendingPathComponent("guardada.png").path))
        XCTAssertTrue(FileManager.default.fileExists(atPath: capturas.appendingPathComponent("nueva.png").path))
        // La galeria ya no la ensena.
        let pagina = try nucleo.galeria(desde: 0, cuantos: 10, dias: 7, desfaseMs: 0)
        XCTAssertEqual(Set(pagina.capturas.map(\.nombre)), ["guardada.png", "nueva.png"])

        // Otra vez: no queda nada que barrer.
        XCTAssertEqual(barrendero.barrer(), BarrenderoCapturas.Barrido())
    }

    func testConElAjusteACeroNoSeVaNinguna() throws {
        let (nucleo, raiz, capturas) = try preparar("barrer-cero")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let barrendero = BarrenderoCapturas(nucleo: nucleo)
        barrendero.dias = { 0 }
        barrendero.tirar = { _ in XCTFail("no tenia que tirar nada") }
        XCTAssertTrue(barrendero.barrer().idas.isEmpty)
        XCTAssertTrue(FileManager.default.fileExists(atPath: capturas.appendingPathComponent("vieja.png").path))
        // Y con un plazo largo, tampoco la de hace 8 dias.
        barrendero.dias = { 30 }
        XCTAssertTrue(barrendero.barrer().idas.isEmpty)
    }

    func testElPlazoSaleDePixpinmaxToml() throws {
        let (nucleo, raiz, _) = try preparar("barrer-toml")
        defer { try? FileManager.default.removeItem(at: raiz) }
        try nucleo.guardarAjustes(nuevos: AjustesMac(diasCaducidad: 3, sincroAutomatica: true, sincroPresencia: true))
        XCTAssertEqual(BarrenderoCapturas(nucleo: nucleo).dias(), 3)
    }

    func testDeVerdadALaPapeleraDelMac() throws {
        let (nucleo, raiz, capturas) = try preparar("barrer-papelera")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let barrendero = BarrenderoCapturas(nucleo: nucleo)
        barrendero.dias = { 7 }
        let hecho = barrendero.barrer()
        if !hecho.fallos.isEmpty {
            throw XCTSkip("Esta maquina no tiene Papelera para la carpeta temporal: \(hecho.fallos)")
        }
        XCTAssertEqual(hecho.idas, ["vieja.png"])
        XCTAssertFalse(FileManager.default.fileExists(atPath: capturas.appendingPathComponent("vieja.png").path))
    }
}

// MARK: - Saltar a un mensaje desde Buscar

@MainActor
final class SaltoAMensajeTests: XCTestCase {
    func testLaVentanaContieneElMensajeYNoSeSaleDelChat() {
        let n: UInt64 = 60
        XCTAssertEqual(ventanaDeSalto(posicion: 500, total: 1000, tam: n).desde, 470)
        XCTAssertEqual(ventanaDeSalto(posicion: 500, total: 1000, tam: n).cuantos, 60)
        // Al principio y al final se corre hacia dentro.
        XCTAssertEqual(ventanaDeSalto(posicion: 0, total: 1000, tam: n).desde, 0)
        XCTAssertEqual(ventanaDeSalto(posicion: 10, total: 1000, tam: n).desde, 0)
        XCTAssertEqual(ventanaDeSalto(posicion: 999, total: 1000, tam: n).desde, 940)
        XCTAssertEqual(ventanaDeSalto(posicion: 990, total: 1000, tam: n).cuantos, 60)
        // Un chat corto entra entero.
        XCTAssertEqual(ventanaDeSalto(posicion: 5, total: 10, tam: n).desde, 0)
        XCTAssertEqual(ventanaDeSalto(posicion: 5, total: 10, tam: n).cuantos, 10)
        // Casos negativos: chat vacio, posicion de mas, pagina de cero.
        XCTAssertEqual(ventanaDeSalto(posicion: 3, total: 0, tam: n).cuantos, 0)
        XCTAssertEqual(ventanaDeSalto(posicion: 5000, total: 1000, tam: n).desde, 940)
        XCTAssertEqual(ventanaDeSalto(posicion: 7, total: 1000, tam: 0).cuantos, 1)
        // Siempre contiene la posicion pedida.
        for p in stride(from: UInt64(0), to: 1000, by: 37) {
            let v = ventanaDeSalto(posicion: p, total: 1000, tam: n)
            XCTAssertTrue(v.desde <= p && p < v.desde + UInt64(v.cuantos), "posicion \(p)")
        }
    }

    func testSaltarCargaSuPaginaLasDeAlrededorYVuelveAlFinalAlEscribir() async throws {
        let raiz = try carpetaTemporal("salto")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let almacen = try Almacen(raiz: raiz.appendingPathComponent("datos"), nombreDelAparato: "Mac de prueba")
        let chat = try almacen.nucleo.crearChat(nombre: "Largo").id
        for i in 0 ..< 200 {
            _ = try almacen.nucleo.escribirNota(chat: chat, texto: "nota \(i)", respondeA: nil)
        }
        await almacen.abrir(chat: chat, en: 100)
        let pagina = try XCTUnwrap(almacen.mensajes[chat])
        XCTAssertEqual(pagina.count, Int(Almacen.tamPagina))
        XCTAssertTrue(pagina.contains { $0.texto == "nota 100" })
        XCTAssertTrue(almacen.quedanAnteriores(chat: chat))
        XCTAssertTrue(almacen.quedanPosteriores(chat: chat))

        // Bajar: la pagina siguiente se pega detras, sin huecos.
        await almacen.cargarPosteriores(chat: chat)
        let bajado = try XCTUnwrap(almacen.mensajes[chat])
        XCTAssertEqual(bajado.count, 2 * Int(Almacen.tamPagina))
        XCTAssertEqual(bajado.map(\.texto), (70 ..< 190).map { "nota \($0)" })
        // Subir: la anterior delante.
        await almacen.cargarAnteriores(chat: chat)
        XCTAssertEqual(almacen.mensajes[chat]?.first?.texto, "nota 10")

        // Escribir con un hueco al final: se vuelve a la ultima pagina.
        let m = await almacen.escribir(nota: "la nueva", en: chat)
        XCTAssertNotNil(m)
        XCTAssertFalse(almacen.quedanPosteriores(chat: chat))
        XCTAssertEqual(almacen.mensajes[chat]?.last?.texto, "la nueva")
        XCTAssertEqual(almacen.mensajes[chat]?.count, Int(Almacen.tamPagina))

        // Caso negativo: con todo cargado no hay nada despues.
        await almacen.cargarPosteriores(chat: chat)
        XCTAssertEqual(almacen.mensajes[chat]?.last?.texto, "la nueva")
    }

    func testBuscarPideElSaltoConLaPosicionDelMensaje() async throws {
        let raiz = try carpetaTemporal("salto-buscar")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let almacen = try Almacen(raiz: raiz.appendingPathComponent("datos"), nombreDelAparato: "Mac de prueba")
        let chat = try almacen.nucleo.crearChat(nombre: "Notas").id
        for i in 0 ..< 90 {
            _ = try almacen.nucleo.escribirNota(chat: chat, texto: i == 12 ? "la aguja" : "paja \(i)", respondeA: nil)
        }
        let r = try XCTUnwrap(almacen.nucleo.buscar(consulta: "aguja", desde: 0, cuantos: 10, busqueda: nil)
            .resultados.first { $0.clase == .mensaje })
        let posicion = try XCTUnwrap(r.posicion)
        XCTAssertEqual(posicion, 12)
        await almacen.abrir(chat: chat, en: posicion)
        XCTAssertTrue(almacen.mensajes[chat]?.contains { $0.id == r.mensaje } == true)
        // Cerca del principio la pagina empieza en el primero.
        XCTAssertEqual(almacen.mensajes[chat]?.first?.texto, "paja 0")
    }
}

// MARK: - Llavero

final class LlaveroDelSistemaTests: XCTestCase {
    func testIdaYVueltaPorElLlaveroYPorElPuente() throws {
        // Un servicio propio de la prueba: las claves de verdad no se tocan.
        let llavero = LlaveroDelSistema(sufijo: ".pruebas-\(UUID().uuidString)")
        do {
            try llavero.guardar(servicio: "PixPinMax.asistente", cuenta: "prueba", clave: "uno")
        } catch {
            throw XCTSkip("No hay Llavero de inicio de sesion utilizable aqui: \(error)")
        }
        XCTAssertEqual(llavero.leer(servicio: "PixPinMax.asistente", cuenta: "prueba"), "uno")
        // Guardar otra vez la sustituye.
        try llavero.guardar(servicio: "PixPinMax.asistente", cuenta: "prueba", clave: "dos ñ")
        XCTAssertEqual(llavero.leer(servicio: "PixPinMax.asistente", cuenta: "prueba"), "dos ñ")
        try llavero.borrar(servicio: "PixPinMax.asistente", cuenta: "prueba")
        // Casos negativos: borrada no se lee, y borrar lo que no esta vale.
        XCTAssertNil(llavero.leer(servicio: "PixPinMax.asistente", cuenta: "prueba"))
        XCTAssertNoThrow(try llavero.borrar(servicio: "PixPinMax.asistente", cuenta: "prueba"))

        // Por el puente: Rust guarda y lee la clave en este llavero.
        XCTAssertTrue(LlaveroDelSistema.instalar(llavero))
        try iaGuardarClave(proveedorId: "claude", clave: "  sk-ant-api03-PRUEBA1234567890abcd  ")
        XCTAssertTrue(iaHayClave(proveedorId: "claude"))
        XCTAssertEqual(iaClaveEnmascarada(proveedorId: "claude"), "sk-ant-…abcd")
        XCTAssertEqual(llavero.leer(servicio: "PixPinMax.asistente", cuenta: "claude"),
                       "sk-ant-api03-PRUEBA1234567890abcd", "recortada, en su servicio y cuenta")
        try iaBorrarClave(proveedorId: "claude")
        XCTAssertFalse(iaHayClave(proveedorId: "claude"))
        XCTAssertThrowsError(try iaGuardarClave(proveedorId: "nadie", clave: "x"))
    }
}

// MARK: - Asistente en vivo

/// Un `llama-server` de mentira: a cada peticion contesta lo mismo, un
/// flujo SSE como el de verdad (`data: {...}` y `data: [DONE]`).
final class LlamaFalso: @unchecked Sendable {
    let listener: NWListener
    let cola = DispatchQueue(label: "pixpin.pruebas.llama-falso")

    static let cuerpo: String = {
        func trozo(_ t: String) -> String {
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"\(t)\"}}]}\n\n"
        }
        return trozo("Hola") + trozo(", que tal")
            + "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n"
            + "data: [DONE]\n\n"
    }()

    init() throws {
        listener = try NWListener(using: .tcp, on: .any)
        let respuesta = Data(("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n"
            + "Content-Length: \(LlamaFalso.cuerpo.utf8.count)\r\nConnection: close\r\n\r\n"
            + LlamaFalso.cuerpo).utf8)
        let cola = self.cola
        listener.newConnectionHandler = { conexion in
            conexion.start(queue: cola)
            LlamaFalso.atender(conexion, leido: Data(), respuesta: respuesta)
        }
    }

    /// Lee la peticion entera (cabecera y cuerpo) y contesta.
    static func atender(_ c: NWConnection, leido: Data, respuesta: Data) {
        c.receive(minimumIncompleteLength: 1, maximumLength: 65536) { datos, _, completo, error in
            var todo = leido
            if let datos { todo.append(datos) }
            if completo || error != nil || peticionEntera(todo) {
                c.send(content: respuesta, contentContext: .finalMessage, isComplete: true,
                       completion: .contentProcessed { _ in c.cancel() })
            } else {
                atender(c, leido: todo, respuesta: respuesta)
            }
        }
    }

    static func peticionEntera(_ d: Data) -> Bool {
        guard let fin = d.range(of: Data("\r\n\r\n".utf8)) else { return false }
        let cabecera = String(decoding: d[..<fin.lowerBound], as: UTF8.self).lowercased()
        let largo = cabecera.split(separator: "\r\n")
            .first { $0.hasPrefix("content-length:") }
            .flatMap { Int($0.dropFirst("content-length:".count).trimmingCharacters(in: .whitespaces)) } ?? 0
        return d.count - fin.upperBound >= largo
    }

    /// Empieza a escuchar y da el puerto.
    func arrancar() async throws -> UInt16 {
        let listo = XCTestExpectation(description: "escuchando")
        listener.stateUpdateHandler = { estado in
            if case .ready = estado { listo.fulfill() }
        }
        listener.start(queue: cola)
        _ = await XCTWaiter().fulfillment(of: [listo], timeout: 5)
        guard let puerto = listener.port?.rawValue else { throw XCTSkip("no se pudo escuchar") }
        return puerto
    }

    deinit {
        listener.cancel()
    }
}

/// Lo que llega por el oyente, apuntado desde el hilo del turno.
final class Tandas: @unchecked Sendable {
    private let cerrojo = NSLock()
    private var lista: [String] = []
    func meter(_ t: String) {
        cerrojo.lock()
        lista.append(t)
        cerrojo.unlock()
    }

    var todas: [String] {
        cerrojo.lock()
        defer { cerrojo.unlock() }
        return lista
    }
}

final class AsistenteEnVivoTests: XCTestCase {
    func testElTextoLlegaPorElOyenteMientrasSeEscribe() async throws {
        let llama = try LlamaFalso()
        let puerto = try await llama.arrancar()
        let tandas = Tandas()
        let oyente = OyenteDelAsistente(alTexto: { tandas.meter($0) })
        let corte = CorteIa.nuevo()
        let r = try await Task.detached {
            try llamaLocalPreguntarEnVivo(puerto: puerto, historial: [], pedido: "saluda",
                                          oyente: oyente, corte: corte)
        }.value
        XCTAssertEqual(r.texto, "Hola, que tal")
        XCTAssertFalse(tandas.todas.isEmpty)
        XCTAssertEqual(tandas.todas.joined(), "Hola, que tal")
        XCTAssertFalse(corte.cortado())
    }

    func testCortadoAntesDeEmpezarNoHablaConNadie() async throws {
        let corte = CorteIa.nuevo()
        corte.cancelar()
        let tandas = Tandas()
        // Un puerto donde no escucha nadie: si llamara, daria error.
        let r = try await Task.detached {
            try llamaLocalPreguntarEnVivo(puerto: 9, historial: [], pedido: "hola",
                                          oyente: OyenteDelAsistente(alTexto: { tandas.meter($0) }), corte: corte)
        }.value
        XCTAssertTrue(r.texto.isEmpty)
        XCTAssertTrue(tandas.todas.isEmpty)
        // Caso negativo: un pedido vacio es un error de argumento.
        XCTAssertThrowsError(try llamaLocalPreguntarEnVivo(puerto: 9, historial: [], pedido: "  ",
                                                           oyente: OyenteDelAsistente(alTexto: { _ in }),
                                                           corte: CorteIa.nuevo()))
    }

    @MainActor
    func testElModeloLlenaLaBurbujaEnVivoYAlFinalLaDejaEntera() async throws {
        let llama = try LlamaFalso()
        let puerto = try await llama.arrancar()
        let modelo = ModeloAsistente(almacen: nil, puertoLlama: puerto, usarApple: false)
        await modelo.enviar("saluda")
        XCTAssertEqual(modelo.motor, .llamaLocal(puerto))
        XCTAssertEqual(modelo.burbujas.count, 2)
        XCTAssertEqual(modelo.burbujas[1].texto, "Hola, que tal")
        XCTAssertFalse(modelo.burbujas[1].esAviso)
        XCTAssertFalse(modelo.ocupado)
        XCTAssertFalse(modelo.sePuedeParar)
        // Lo que llegue tarde por el oyente no se pega dos veces.
        modelo.pegar("de mas", en: modelo.burbujas[1].id)
        XCTAssertEqual(modelo.burbujas[1].texto, "Hola, que tal")
    }
}

// MARK: - Acciones de Atajos

@MainActor
final class AccionesAppTests: XCTestCase {
    func testNotaYBuscarComoDesdeAtajos() async throws {
        let raiz = try carpetaTemporal("acciones")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let almacen = try Almacen(raiz: raiz.appendingPathComponent("datos"), nombreDelAparato: "Mac de prueba")
        let chat = try almacen.nucleo.crearChat(nombre: "Compras").id
        let acciones = AccionesApp()
        var capturas = 0
        acciones.registrar(almacen: almacen, sincro: nil, capturar: { capturas += 1 })

        let proyectos = try await acciones.proyectos()
        XCTAssertTrue(proyectos.contains(AccionesApp.Proyecto(id: chat, nombre: "Compras")))
        try await acciones.escribirNota("  leche y pan  ", en: chat)
        let encontrados = try await acciones.buscar("leche")
        XCTAssertTrue(encontrados.contains { $0.detalle.contains("leche") }, "\(encontrados)")
        try acciones.capturarZona()
        XCTAssertEqual(capturas, 1)

        // Casos negativos: nota vacia, y sincronizar sin sincronizador.
        do {
            try await acciones.escribirNota("   ", en: chat)
            XCTFail("una nota vacia no se escribe")
        } catch let fallo as AccionesApp.Fallo {
            XCTAssertEqual(fallo, .vacio)
        }
        do {
            _ = try await acciones.sincronizarAhora()
            XCTFail("sin sincronizador no se sincroniza")
        } catch let fallo as AccionesApp.Fallo {
            XCTAssertEqual(fallo, .sinApp)
        }
    }
}
