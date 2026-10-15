// Pruebas de las cuentas de la captura (`GeometriaCaptura`): sin pantallas
// ni permisos, solo numeros.

import CoreGraphics
import Foundation
@testable import PixPinKit
import XCTest

final class CapturaTests: XCTestCase {
    // MARK: - Elegir la zona

    func testLaZonaEsLaMismaSeArrastreHaciaDondeSeArrastre() {
        let esperada = CGRect(x: 10, y: 20, width: 90, height: 30)
        XCTAssertEqual(GeometriaCaptura.rectEntre(CGPoint(x: 10, y: 20), CGPoint(x: 100, y: 50)), esperada)
        XCTAssertEqual(GeometriaCaptura.rectEntre(CGPoint(x: 100, y: 50), CGPoint(x: 10, y: 20)), esperada)
        XCTAssertEqual(GeometriaCaptura.rectEntre(CGPoint(x: 10, y: 50), CGPoint(x: 100, y: 20)), esperada)
        XCTAssertEqual(GeometriaCaptura.rectEntre(CGPoint(x: 100, y: 20), CGPoint(x: 10, y: 50)), esperada)
    }

    func testLaZonaNoSaleDeLaPantalla() {
        let pantalla = CGRect(x: 0, y: 0, width: 100, height: 100)
        XCTAssertEqual(GeometriaCaptura.ajustar(CGRect(x: -10, y: 90, width: 30, height: 30), a: pantalla),
                       CGRect(x: 0, y: 90, width: 20, height: 10))
        XCTAssertEqual(GeometriaCaptura.ajustar(CGRect(x: 200, y: 200, width: 5, height: 5), a: pantalla), .zero)
        // Un rectangulo con ancho negativo se normaliza antes.
        XCTAssertEqual(GeometriaCaptura.ajustar(CGRect(x: 50, y: 50, width: -20, height: -10), a: pantalla),
                       CGRect(x: 30, y: 40, width: 20, height: 10))
    }

    func testUnTemblorNoEsArrastre() {
        XCTAssertFalse(GeometriaCaptura.esArrastre(desde: .zero, hasta: CGPoint(x: 2, y: -2)))
        XCTAssertTrue(GeometriaCaptura.esArrastre(desde: .zero, hasta: CGPoint(x: 0, y: 3)))
    }

    // MARK: - Puntos de AppKit a pixeles de la foto

    func testEnRetinaCadaPuntoSonDosPixelesYLaYSeDaLaVuelta() {
        // Pantalla de 1440 x 900 puntos, foto de 2880 x 1800.
        let vista = CGSize(width: 1440, height: 900)
        // Una zona abajo a la izquierda en AppKit esta abajo en la foto.
        let r = GeometriaCaptura.rectEnPixeles(CGRect(x: 0, y: 0, width: 100, height: 50),
                                               tamVista: vista, ancho: 2880, alto: 1800)
        XCTAssertEqual(r, CGRect(x: 0, y: 1700, width: 200, height: 100))
        // Y arriba a la derecha, arriba en la foto (y = 0).
        let arriba = GeometriaCaptura.rectEnPixeles(CGRect(x: 1340, y: 850, width: 100, height: 50),
                                                    tamVista: vista, ancho: 2880, alto: 1800)
        XCTAssertEqual(arriba, CGRect(x: 2680, y: 0, width: 200, height: 100))
    }

    func testSinRetinaEsUnoAUno() {
        let r = GeometriaCaptura.rectEnPixeles(CGRect(x: 10, y: 20, width: 30, height: 40),
                                               tamVista: CGSize(width: 800, height: 600), ancho: 800, alto: 600)
        XCTAssertEqual(r, CGRect(x: 10, y: 540, width: 30, height: 40))
    }

    func testLosMediosPuntosSeRedondeanHaciaFueraYSinPasarse() {
        // Escala 1.5 (pantallas "mas espacio"): 10.5 * 1.5 = 15.75.
        let r = GeometriaCaptura.rectEnPixeles(CGRect(x: 10.5, y: 0, width: 10, height: 10),
                                               tamVista: CGSize(width: 100, height: 100), ancho: 150, alto: 150)
        XCTAssertEqual(r.minX, 15)
        XCTAssertEqual(r.maxX, 31) // 20.5 * 1.5 = 30.75 -> 31
        // Un error de coma flotante no anade una fila de mas.
        let exacta = GeometriaCaptura.rectEnPixeles(CGRect(x: 0, y: 0, width: 100.0000001, height: 10),
                                                    tamVista: CGSize(width: 200, height: 100), ancho: 400, alto: 200)
        XCTAssertEqual(exacta.width, 200)
        // Y nunca sale de la foto.
        let borde = GeometriaCaptura.rectEnPixeles(CGRect(x: 90, y: 90, width: 50, height: 50),
                                                   tamVista: CGSize(width: 100, height: 100), ancho: 200, alto: 200)
        XCTAssertEqual(borde, CGRect(x: 180, y: 0, width: 20, height: 20))
    }

    func testSinTamanoNoHayRecorte() {
        XCTAssertEqual(GeometriaCaptura.rectEnPixeles(CGRect(x: 0, y: 0, width: 10, height: 10),
                                                      tamVista: .zero, ancho: 100, alto: 100), .zero)
    }

    // MARK: - Coordenadas de Core Graphics y pantallas

    func testUnaVentanaDeCoreGraphicsPasaAAppKit() {
        // Pantalla principal de 900 de alto: una ventana arriba del todo en
        // Core Graphics (y = 0) esta arriba del todo en AppKit (maxY = 900).
        let r = GeometriaCaptura.desdeCoordenadasCG(CGRect(x: 100, y: 0, width: 300, height: 200),
                                                    altoPantallaPrincipal: 900)
        XCTAssertEqual(r, CGRect(x: 100, y: 700, width: 300, height: 200))
        // Una pantalla secundaria encima de la principal tiene y negativa en CG.
        let encima = GeometriaCaptura.desdeCoordenadasCG(CGRect(x: 0, y: -500, width: 100, height: 100),
                                                         altoPantallaPrincipal: 900)
        XCTAssertEqual(encima, CGRect(x: 0, y: 1300, width: 100, height: 100))
    }

    func testUnMarcoGlobalEnUnaPantallaSecundaria() {
        // Pantalla a la derecha de la principal, en x = 1440.
        let r = GeometriaCaptura.enPantalla(CGRect(x: 1500, y: 100, width: 10, height: 10),
                                            origenPantalla: CGPoint(x: 1440, y: 0))
        XCTAssertEqual(r, CGRect(x: 60, y: 100, width: 10, height: 10))
    }

    func testElImanEligeLaVentanaDeDelante() {
        let delante = CGRect(x: 50, y: 50, width: 100, height: 100)
        let detras = CGRect(x: 0, y: 0, width: 400, height: 400)
        XCTAssertEqual(GeometriaCaptura.ventanaBajo(CGPoint(x: 60, y: 60), en: [delante, detras]), delante)
        XCTAssertEqual(GeometriaCaptura.ventanaBajo(CGPoint(x: 10, y: 10), en: [delante, detras]), detras)
        XCTAssertNil(GeometriaCaptura.ventanaBajo(CGPoint(x: 500, y: 500), en: [delante, detras]))
    }

    // MARK: - Barra y lupa

    func testLaBarraVaDebajoEncimaODentro() {
        let pantalla = CGRect(x: 0, y: 0, width: 1000, height: 800)
        let barra = CGSize(width: 300, height: 40)
        // Hay sitio debajo: debajo y alineada a la derecha de la zona.
        let debajo = GeometriaCaptura.posicionBarra(seleccion: CGRect(x: 400, y: 300, width: 400, height: 200),
                                                    tamBarra: barra, limites: pantalla)
        XCTAssertEqual(debajo, CGPoint(x: 500, y: 252))
        // Zona pegada abajo: encima.
        let encima = GeometriaCaptura.posicionBarra(seleccion: CGRect(x: 400, y: 0, width: 400, height: 200),
                                                    tamBarra: barra, limites: pantalla)
        XCTAssertEqual(encima.y, 208)
        // La pantalla entera: dentro, abajo.
        let dentro = GeometriaCaptura.posicionBarra(seleccion: pantalla, tamBarra: barra, limites: pantalla)
        XCTAssertEqual(dentro, CGPoint(x: 692, y: 8))
        // Zona estrecha a la izquierda: la barra no se sale.
        let izquierda = GeometriaCaptura.posicionBarra(seleccion: CGRect(x: 0, y: 300, width: 50, height: 50),
                                                       tamBarra: barra, limites: pantalla)
        XCTAssertEqual(izquierda.x, 8)
    }

    func testLaLupaEnUnidadesDeLaCapa() {
        let r = GeometriaCaptura.rectDeLupa(en: CGPoint(x: 50, y: 25), lado: 10, tamVista: CGSize(width: 100, height: 50))
        XCTAssertEqual(r.minX, 0.45, accuracy: 1e-9)
        XCTAssertEqual(r.minY, 0.4, accuracy: 1e-9)
        XCTAssertEqual(r.width, 0.1, accuracy: 1e-9)
        XCTAssertEqual(r.height, 0.2, accuracy: 1e-9)
    }

    func testSoloSeCopiaAparteUnRecortePequeno() {
        let pantalla = CGSize(width: 2880, height: 1800)
        XCTAssertTrue(GeometriaCaptura.convieneCopiarAparte(recorte: CGSize(width: 400, height: 300), entera: pantalla))
        XCTAssertFalse(GeometriaCaptura.convieneCopiarAparte(recorte: pantalla, entera: pantalla))
        XCTAssertFalse(GeometriaCaptura.convieneCopiarAparte(recorte: CGSize(width: 10, height: 10), entera: .zero))
    }

    // MARK: - Nombre del fichero

    func testElNombreEsElDeWindows() throws {
        let utc = try XCTUnwrap(TimeZone(identifier: "UTC"))
        // 2026-10-15 14:03:09 UTC.
        let fecha = Date(timeIntervalSince1970: 1_792_072_989)
        XCTAssertEqual(GeometriaCaptura.nombreDeCaptura(fecha: fecha, zona: utc), "captura-20261015-140309")
        // En Madrid (UTC+2 en octubre) es la hora local.
        let madrid = try XCTUnwrap(TimeZone(identifier: "Europe/Madrid"))
        XCTAssertEqual(GeometriaCaptura.nombreDeCaptura(fecha: fecha, zona: madrid), "captura-20261015-160309")
    }

    func testSiElNombreEstaCogidoSeAnadeUnNumero() throws {
        let utc = try XCTUnwrap(TimeZone(identifier: "UTC"))
        let fecha = Date(timeIntervalSince1970: 1_792_072_989)
        let carpeta = URL(fileURLWithPath: "/tmp/capturas", isDirectory: true)
        let libre = GeometriaCaptura.rutaNueva(en: carpeta, fecha: fecha, zona: utc) { _ in false }
        XCTAssertEqual(libre.lastPathComponent, "captura-20261015-140309.png")
        // Ya hay un PNG y un GIF de ese segundo (un GIF tambien cuenta).
        let ocupados: Set<String> = ["captura-20261015-140309.png", "captura-20261015-140309-2.gif"]
        let siguiente = GeometriaCaptura.rutaNueva(en: carpeta, fecha: fecha, zona: utc) {
            ocupados.contains($0.lastPathComponent)
        }
        XCTAssertEqual(siguiente.lastPathComponent, "captura-20261015-140309-3.png")
        XCTAssertEqual(siguiente.deletingLastPathComponent().path, carpeta.path)
    }
}
