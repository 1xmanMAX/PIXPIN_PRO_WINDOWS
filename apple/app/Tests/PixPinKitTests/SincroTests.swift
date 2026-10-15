// Pruebas de sincronizar: lo puro (TXT de Bonjour, quien es del grupo, la
// espera de «quieto» con un reloj de mentira) y una vuelta de verdad entre
// dos nucleos en carpetas temporales por 127.0.0.1, como hace Windows.

import Foundation
import Network
import PixPinCore
@testable import PixPinKit
import XCTest

final class TXTBonjourTests: XCTestCase {
    func testElTXTSeEscribeEnOrdenYSeLeeIgual() {
        let pares = [("g", "0123456789abcdef"), ("id", "abc"), ("l", "c"), ("n", "Mac de Max ñ")]
        let datos = codificarTXT(pares)
        // Cada entrada lleva delante su largo en un byte.
        XCTAssertEqual(Int(datos[0]), "g=0123456789abcdef".utf8.count)
        XCTAssertEqual(String(decoding: datos[1 ... 2], as: UTF8.self), "g=")
        let leido = leerTXT(datos)
        XCTAssertEqual(leido, ["g": "0123456789abcdef", "id": "abc", "l": "c", "n": "Mac de Max ñ"])
    }

    func testUnTXTVacioEsUnByteCeroYLoQueNoCabeSeDejaFuera() {
        XCTAssertEqual(codificarTXT([]), Data([0]))
        let largo = String(repeating: "x", count: 300)
        XCTAssertEqual(leerTXT(codificarTXT([("n", largo), ("id", "a")])), ["id": "a"])
    }

    func testLasClavesNoDistinguenMayusculas() {
        let d = DatosVecino(txt: ["G": "etq", "ID": "otro", "L": "b", "N": "PC"])
        XCTAssertEqual(d, DatosVecino(etiqueta: "etq", id: "otro", letra: "b", nombre: "PC"))
    }

    func testSoloEsDelGrupoQuienAnunciaMiEtiquetaYNoSoyYo() {
        let otro = DatosVecino(txt: ["g": "etq", "id": "pc"])
        XCTAssertTrue(otro.esDelGrupo(etiqueta: "etq", yo: "mac"))
        XCTAssertFalse(otro.esDelGrupo(etiqueta: "otra", yo: "mac"))
        // Mi propio anuncio tambien sale al buscar.
        XCTAssertFalse(otro.esDelGrupo(etiqueta: "etq", yo: "pc"))
        // Sin `g` (sin grupo) no es de ninguno, ni con una etiqueta vacia.
        let suelto = DatosVecino(txt: ["g": "", "id": "x"])
        XCTAssertNil(suelto.etiqueta)
        XCTAssertFalse(suelto.esDelGrupo(etiqueta: "", yo: "mac"))
    }

    func testElNombreSaleDelTXTODeLaInstancia() {
        XCTAssertEqual(DatosVecino(txt: ["n": "Movil"]).nombreParaVer(instancia: "PixPin Telefono"), "Movil")
        XCTAssertEqual(DatosVecino(txt: [:]).nombreParaVer(instancia: "PixPin Telefono"), "Telefono")
        XCTAssertEqual(DatosVecino(txt: [:]).nombreParaVer(instancia: "Otro"), "Otro")
    }

    func testLasDireccionesSeEscribenComoEnElMovil() {
        XCTAssertEqual(Direccion(texto: "192.168.1.20:47474"), Direccion(host: "192.168.1.20", puerto: 47474))
        XCTAssertEqual(Direccion(texto: " 10.0.0.8 "), Direccion(host: "10.0.0.8", puerto: puertoPreferido()))
        XCTAssertEqual(Direccion(texto: "[fe80::1%en0]:5000"), Direccion(host: "fe80::1%en0", puerto: 5000))
        XCTAssertEqual(Direccion(texto: "fe80::1"), Direccion(host: "fe80::1", puerto: puertoPreferido()))
        XCTAssertNil(Direccion(texto: ""))
        XCTAssertNil(Direccion(texto: "pc:nope"))
        XCTAssertNil(Direccion(texto: "pc:0"))
    }

    func testLosHostsDeNetworkSeEscribenParaRust() {
        XCTAssertEqual(textoIPv4(Data([192, 168, 1, 5])), "192.168.1.5")
        XCTAssertNil(textoIPv4(Data([1, 2, 3])))
        XCTAssertEqual(Vecinos.textoDeHost(.ipv4(IPv4Address("10.0.0.9")!)), "10.0.0.9")
        XCTAssertEqual(Vecinos.textoDeHost(.ipv6(IPv6Address("2001:db8::1")!)), "2001:db8::1")
        XCTAssertEqual(Vecinos.textoDeHost(.name("pc.local", nil)), "pc.local")
    }

    func testElTipoYElPuertoSonLosDeWindows() {
        XCTAssertEqual(tipoDeServicio(), "_pixpin._tcp")
        XCTAssertEqual(puertoPreferido(), 47474)
    }
}

final class CalmaTests: XCTestCase {
    /// Un reloj que solo avanza cuando la prueba quiere.
    final class Reloj {
        var ahora = Date(timeIntervalSince1970: 1_000_000)
        func avanzar(_ s: TimeInterval) { ahora = ahora.addingTimeInterval(s) }
    }

    func testSinCambiosNoHayNadaPendiente() {
        let calma = Calma()
        XCTAssertNil(calma.falta(en: Date()))
        XCTAssertFalse(calma.toca(en: Date()))
    }

    func testTocaTreintaSegundosDespuesDelUltimoCambio() {
        let reloj = Reloj()
        var calma = Calma()
        calma.cambio(en: reloj.ahora)
        XCTAssertEqual(calma.falta(en: reloj.ahora), 30)
        reloj.avanzar(20)
        XCTAssertEqual(calma.falta(en: reloj.ahora), 10)
        // Otro cambio vuelve a empezar la cuenta.
        calma.cambio(en: reloj.ahora)
        reloj.avanzar(29)
        XCTAssertFalse(calma.toca(en: reloj.ahora))
        XCTAssertEqual(calma.falta(en: reloj.ahora) ?? -1, 1, accuracy: 0.001)
        reloj.avanzar(1)
        XCTAssertTrue(calma.toca(en: reloj.ahora))
        reloj.avanzar(100)
        XCTAssertEqual(calma.falta(en: reloj.ahora), 0, "pasado el plazo, toca ya")
        calma.hecha()
        XCTAssertNil(calma.falta(en: reloj.ahora))
    }

    func testLoAutomaticoRespetaElBajoConsumo() {
        XCTAssertTrue(sePuedeSincronizar(.manual, automatica: false, bajoConsumo: true))
        XCTAssertTrue(sePuedeSincronizar(.quieto, automatica: true, bajoConsumo: false))
        XCTAssertFalse(sePuedeSincronizar(.quieto, automatica: true, bajoConsumo: true))
        XCTAssertFalse(sePuedeSincronizar(.aparecio, automatica: true, bajoConsumo: true))
        XCTAssertFalse(sePuedeSincronizar(.aparecio, automatica: false, bajoConsumo: false))
    }

    func testUnAparatoQueVaYVieneNoLanzaUnaVueltaCadaVez() {
        let reloj = Reloj()
        XCTAssertTrue(tocaPorAparecer(ultimaVuelta: nil, ahora: reloj.ahora))
        let vuelta = reloj.ahora
        reloj.avanzar(10)
        XCTAssertFalse(tocaPorAparecer(ultimaVuelta: vuelta, ahora: reloj.ahora))
        reloj.avanzar(entreVueltasPorAparecer)
        XCTAssertTrue(tocaPorAparecer(ultimaVuelta: vuelta, ahora: reloj.ahora))
    }
}

final class SincroDeVerdadTests: XCTestCase {
    /// Dos aparatos en dos carpetas: el «PC» crea el grupo y escucha en su
    /// `Puerta` (el hilo de la app), el «Mac» se une y sincroniza una nota,
    /// todo por la API de Rust y 127.0.0.1.
    func testDosNucleosSeUnenYSincronizanPorLocalhost() throws {
        let raiz = try carpetaTemporal("sincro")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let pc = try Nucleo.abrir(raiz: raiz.appendingPathComponent("pc").path, nombreDelAparato: "PC")
        let mac = try Nucleo.abrir(raiz: raiz.appendingPathComponent("mac").path, nombreDelAparato: "Mac")

        // Sin grupo, no hay anuncio.
        XCTAssertNil(try pc.anuncio())
        let codigo = try pc.crearGrupo()
        XCTAssertTrue(codigoValido(codigo: codigo))
        let anuncio = try XCTUnwrap(try pc.anuncio())
        XCTAssertEqual(anuncio.tipo, tipoDeServicio())
        XCTAssertEqual(anuncio.instancia, "PixPin PC")

        // Lo que anuncia el PC, leido como lo leeria otro Mac por Bonjour,
        // es del grupo de quien tenga su codigo.
        let visto = DatosVecino(txt: leerTXT(codificarTXT(anuncio.txt.map { ($0.clave, $0.valor) })))
        XCTAssertTrue(visto.esDelGrupo(etiqueta: etiquetaDeGrupo(codigo: codigo), yo: try mac.aparato().id))
        XCTAssertEqual(visto.nombre, "PC")
        XCTAssertEqual(visto.letra, "a")

        let puerta = try Puerta(nucleo: pc)
        defer { puerta.cerrar() }
        let llegoElMac = expectation(description: "el Mac saludo dos veces (union y vuelta)")
        llegoElMac.expectedFulfillmentCount = 2
        puerta.arrancar { resultado in
            if case let .success(.aparato(_, nombre)) = resultado, nombre == "Mac" {
                llegoElMac.fulfill()
            }
        }

        let yo = try mac.unirseAGrupo(host: "127.0.0.1", puerto: puerta.puerto,
                                      codigo: codigoLegible(codigo: codigo).lowercased())
        XCTAssertEqual(yo.grupo, codigo)
        XCTAssertNotNil(yo.letra)
        // Ya en el grupo, el Mac anuncia la misma etiqueta que el PC.
        XCTAssertEqual(try mac.anuncio()?.etiqueta, anuncio.etiqueta)

        let obra = try mac.crearChat(nombre: "Obra")
        _ = try mac.escribirNota(chat: obra.id, texto: "desde el Mac", respondeA: nil)
        let r = try mac.sincronizarCon(host: "127.0.0.1", puerto: puerta.puerto)
        XCTAssertEqual(r.otroNombre, "PC")
        XCTAssertGreaterThanOrEqual(r.enviados, 1)
        XCTAssertFalse(r.texto.isEmpty)
        wait(for: [llegoElMac], timeout: 10)

        let suya = try XCTUnwrap(try pc.chats().first { $0.nombre == "Obra" })
        let pagina = try pc.ultimosMensajes(chat: suya.id, cuantos: 10)
        XCTAssertEqual(pagina.mensajes.map(\.texto), ["desde el Mac"])
    }

    /// La sonda de la app (`NWConnection`, PING → PONG) contra la puerta de
    /// Rust: contesta y da la direccion que necesita `sincronizarCon`.
    func testLaSondaDeNetworkContestaYDaLaDireccion() async throws {
        let raiz = try carpetaTemporal("sonda")
        defer { try? FileManager.default.removeItem(at: raiz) }
        let pc = try Nucleo.abrir(raiz: raiz.path, nombreDelAparato: "PC")
        let puerta = try Puerta(nucleo: pc)
        defer { puerta.cerrar() }
        puerta.arrancar { _ in }
        let punto = NWEndpoint.hostPort(host: "127.0.0.1", port: NWEndpoint.Port(rawValue: puerta.puerto)!)
        let d = await Vecinos.sondear(punto)
        XCTAssertEqual(d, Direccion(host: "127.0.0.1", puerto: puerta.puerto))
    }
}
