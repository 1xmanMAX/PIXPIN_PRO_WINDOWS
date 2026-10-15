// Pruebas de lo nuevo de los pines: guias e iman, el ritmo y las cuentas del
// pin en vivo, la barra comun, el panel de pines abiertos, anotar y las
// mini-apps por el puente (documento entra, documento sale) con un chat de
// verdad en una carpeta temporal. Nada abre ventanas ni pide permisos.

import CoreGraphics
import Foundation
import PixPinCore
@testable import PixPinKit
import XCTest

final class GuiasTests: XCTestCase {
    func testATiroDelBordeIzquierdoDeOtroSaleSuGuiaYSePega() {
        let video = CGRect(x: 440, y: 128, width: 600, height: 300)
        let recorte = CGRect(x: 445, y: 460, width: 230, height: 86)
        let a = ReglasGuias.alinear(recorte, otros: [video])
        XCTAssertEqual(a.rect.minX, 440, "izquierda con izquierda")
        let g = try? XCTUnwrap(a.guias.first { $0.vertical })
        XCTAssertEqual(g?.pos, 440)
        XCTAssertLessThanOrEqual(g?.desde ?? .infinity, 128)
        XCTAssertGreaterThanOrEqual(g?.hasta ?? 0, a.rect.maxY, "cubre a los dos")
    }

    func testSeOfreceLaSeparacionDeDieciseis() {
        let video = CGRect(x: 440, y: 128, width: 600, height: 300)
        // Su borde a 19 puntos del otro: a 3 de los 16.
        let recorte = CGRect(x: 700, y: 447, width: 230, height: 86)
        let a = ReglasGuias.alinear(recorte, otros: [video])
        XCTAssertEqual(a.rect.minY, 428 + 16)
        XCTAssertTrue(a.guias.contains { !$0.vertical && $0.pos == 428 })
    }

    func testLejosDeTodoNoHayGuiaNiSeMueve() {
        let otro = CGRect(x: 0, y: 0, width: 100, height: 100)
        let lejos = CGRect(x: 500, y: 500, width: 100, height: 100)
        let a = ReglasGuias.alinear(lejos, otros: [otro])
        XCTAssertEqual(a.rect, lejos)
        XCTAssertTrue(a.guias.isEmpty)
        XCTAssertTrue(ReglasGuias.alinear(lejos, otros: []).guias.isEmpty)
    }

    func testGanaElBordeMasCercano() {
        let aLado = CGRect(x: 100, y: 0, width: 100, height: 100)
        let otro = CGRect(x: 103, y: 300, width: 100, height: 100)
        let movido = CGRect(x: 105, y: 600, width: 50, height: 50)
        XCTAssertEqual(ReglasGuias.alinear(movido, otros: [aLado, otro]).rect.minX, 103)
    }

    func testUnaGuiaPorEjeComoMucho() {
        let otro = CGRect(x: 100, y: 100, width: 200, height: 200)
        let movido = CGRect(x: 103, y: 104, width: 200, height: 200)
        let a = ReglasGuias.alinear(movido, otros: [otro])
        XCTAssertEqual(a.rect, otro)
        XCTAssertEqual(a.guias.count, 2)
        XCTAssertEqual(a.guias.filter(\.vertical).count, 1)
    }

    func testElMarcoDeUnaGuiaEsUnaRayaDeSuGrosor() {
        let v = ReglasGuias.marco(de: GuiaPin(vertical: true, pos: 50, desde: 10, hasta: 110), grosor: 2)
        XCTAssertEqual(v, CGRect(x: 49, y: 10, width: 2, height: 100))
        let h = ReglasGuias.marco(de: GuiaPin(vertical: false, pos: 50, desde: 10, hasta: 10), grosor: 2)
        XCTAssertEqual(h.width, 1, "nunca de largo cero")
    }
}

final class PinVivoTests: XCTestCase {
    func testElRitmoBajaTrasUnSegundoQuietoYSubeAlCambiar() {
        var p = PoliticaFotogramas()
        XCTAssertEqual(p.porSegundo, 30)
        XCTAssertNil(p.registrar(.quieto, ahora: 0))
        XCTAssertNil(p.registrar(.quieto, ahora: 0.5))
        XCTAssertEqual(p.registrar(.quieto, ahora: 1.0), 2)
        XCTAssertNil(p.registrar(.quieto, ahora: 3), "ya esta lento: no se pide otra vez")
        XCTAssertNil(p.registrar(.otro, ahora: 4))
        XCTAssertEqual(p.registrar(.nuevo, ahora: 4.2), 30)
        XCTAssertNil(p.registrar(.nuevo, ahora: 4.3))
        // Un cambio reciente aplaza la bajada.
        XCTAssertNil(p.registrar(.quieto, ahora: 5.0))
        XCTAssertEqual(p.registrar(.quieto, ahora: 5.4), 2)
        p.reiniciar()
        XCTAssertEqual(p.porSegundo, 30)
    }

    func testAlCienPorCienElPuntoDelPinEsElDeLaZona() {
        let m = CGSize(width: 400, height: 300)
        XCTAssertEqual(ReglasVivo.puntoEnLaZona(CGPoint(x: 120, y: 45), contenido: m, zona: m), CGPoint(x: 120, y: 45))
        // Pin al doble.
        XCTAssertEqual(ReglasVivo.puntoEnLaZona(CGPoint(x: 600, y: 300), contenido: CGSize(width: 800, height: 600), zona: m),
                       CGPoint(x: 300, y: 150))
        // Pin encogido.
        XCTAssertEqual(ReglasVivo.puntoEnLaZona(CGPoint(x: 100, y: 50), contenido: CGSize(width: 200, height: 150), zona: m),
                       CGPoint(x: 200, y: 100))
    }

    func testConZoomInteriorSeDeshaceLaVistaAntesQueElTamano() {
        let m = CGSize(width: 400, height: 300)
        let vista = VistaInterior(escala: 2, dx: -200, dy: -150)
        XCTAssertEqual(ReglasVivo.puntoEnLaZona(CGPoint(x: 200, y: 150), contenido: m, vista: vista, zona: m),
                       CGPoint(x: 200, y: 150))
        XCTAssertEqual(ReglasVivo.puntoEnLaZona(.zero, contenido: m, vista: vista, zona: m), CGPoint(x: 100, y: 75))
        // Una escala rota se toma por neutra.
        XCTAssertEqual(ReglasVivo.puntoEnLaZona(CGPoint(x: 10, y: 10), contenido: m,
                                                vista: VistaInterior(escala: 0, dx: 0, dy: 0), zona: m),
                       CGPoint(x: 10, y: 10))
    }

    func testFueraDelContenidoOConMedidasACeroNoSeManda() {
        let m = CGSize(width: 400, height: 300)
        XCTAssertNil(ReglasVivo.puntoEnLaZona(CGPoint(x: -1, y: 10), contenido: m, zona: m))
        XCTAssertNil(ReglasVivo.puntoEnLaZona(CGPoint(x: 10, y: 300), contenido: m, zona: m))
        XCTAssertNil(ReglasVivo.puntoEnLaZona(CGPoint(x: 400, y: 10), contenido: m, zona: m))
        XCTAssertNotNil(ReglasVivo.puntoEnLaZona(CGPoint(x: 399.9, y: 299.9), contenido: m, zona: m))
        XCTAssertNil(ReglasVivo.puntoEnLaZona(CGPoint(x: 1, y: 1), contenido: CGSize(width: 0, height: 300), zona: m))
        XCTAssertNil(ReglasVivo.puntoEnLaZona(CGPoint(x: 1, y: 1), contenido: m, zona: CGSize(width: 400, height: 0)))
    }

    func testUnClicTiemblaYUnArrastreNoEsUnClic() {
        XCTAssertTrue(ReglasVivo.fueClic(CGPoint(x: 500, y: 500), CGPoint(x: 504, y: 496)))
        XCTAssertFalse(ReglasVivo.fueClic(CGPoint(x: 500, y: 500), CGPoint(x: 506, y: 500)))
        XCTAssertFalse(ReglasVivo.fueClic(CGPoint(x: 500, y: 500), CGPoint(x: 500, y: 380)))
    }

    private let trabajo = CGRect(x: 0, y: 0, width: 1920, height: 1040)

    func testNaceALaDerechaDeLaZonaALaMismaAltura() {
        let zona = CGRect(x: 100, y: 200, width: 400, height: 300)
        let r = ReglasVivo.sitioJuntoA(zona, trabajo: trabajo)
        XCTAssertEqual(r, CGRect(x: 100 + 400 + ReglasVivo.hueco, y: 200, width: 400, height: 300))
    }

    func testPegadaALaDerechaLoPoneALaIzquierda() {
        let zona = CGRect(x: 1500, y: 200, width: 400, height: 300)
        XCTAssertEqual(ReglasVivo.sitioJuntoA(zona, trabajo: trabajo).minX, 1500 - 400 - ReglasVivo.hueco)
    }

    func testNuncaNaceEncimaDeSuZonaSiHayOtroSitio() {
        // Ancha y arriba del todo: ni a los lados cabe; va debajo.
        let zona = CGRect(x: 200, y: 740, width: 1500, height: 300)
        let r = ReglasVivo.sitioJuntoA(zona, trabajo: trabajo)
        XCTAssertFalse(r.intersects(zona))
        XCTAssertEqual(r.minY, 740 - 300 - ReglasVivo.hueco)
    }

    func testCercaDelBordeSeArrimaSinSalirse() {
        let zona = CGRect(x: 100, y: -160, width: 300, height: 300)
        let r = ReglasVivo.sitioJuntoA(zona, trabajo: trabajo)
        XCTAssertEqual(r.minX, 100 + 300 + ReglasVivo.hueco)
        XCTAssertEqual(r.minY, 0, "subido lo justo para caber")
        XCTAssertTrue(trabajo.contains(r))
    }

    func testUnaZonaQueLlenaLaPantallaSeQuedaDondeEsta() {
        XCTAssertEqual(ReglasVivo.sitioJuntoA(trabajo, trabajo: trabajo), trabajo)
    }

    func testLaZonaPasaALaPantallaConElOrigenArriba() {
        // Pantalla secundaria a la derecha y algo mas baja.
        let pantalla = CGRect(x: 1440, y: -180, width: 1920, height: 1080)
        let zona = CGRect(x: 1540, y: 600, width: 200, height: 100)
        XCTAssertEqual(ReglasVivo.zonaEnPantalla(zona, pantalla: pantalla),
                       CGRect(x: 100, y: 1080 - 180 - 700, width: 200, height: 100))
        XCTAssertEqual(ReglasVivo.aQuartz(CGPoint(x: 10, y: 890), altoPrincipal: 900), CGPoint(x: 10, y: 10))
        XCTAssertEqual(ReglasVivo.pixeles(CGSize(width: 200, height: 100.4), escala: 2).ancho, 400)
        XCTAssertEqual(ReglasVivo.pixeles(CGSize(width: 0.2, height: 0.2), escala: 1).alto, 2, "al menos 2x2")
    }
}

final class BarraPinTests: XCTestCase {
    func testLaMismaBarraEnTodosConSuControlPropioPrimero() {
        let foto = ReglasBarra.piezas(TipoBarraPin(propio: .zoom, copiable: true, anotable: true))
        XCTAssertEqual(foto, [.boton(.alejar), .rotulo(.zoom), .boton(.acercar), .separador,
                              .boton(.copiar), .boton(.anotar), .boton(.mas), .separador, .boton(.cerrar)])
        let vivo = ReglasBarra.piezas(TipoBarraPin(propio: .vivo, copiable: true, anotable: false))
        XCTAssertEqual(Array(vivo.prefix(4)), [.rotulo(.enVivo), .boton(.reproducir), .boton(.manejar), .boton(.congelar)])
        let video = ReglasBarra.piezas(TipoBarraPin(propio: .video, copiable: true, anotable: false))
        XCTAssertEqual(Array(video.prefix(5)), [.boton(.reproducir), .rotulo(.tiempo), .recorrido,
                                                .rotulo(.duracion), .boton(.sonido)])
        // Una herramienta no tiene control propio ni separador delante.
        let herramienta = ReglasBarra.piezas(TipoBarraPin(propio: .ninguno, copiable: true, anotable: false))
        XCTAssertEqual(herramienta, [.boton(.copiar), .boton(.mas), .separador, .boton(.cerrar)])
        // Un PDF de una pagina no lleva paso de pagina.
        XCTAssertEqual(ReglasBarra.piezas(TipoBarraPin(propio: .paginas(conPaso: false), copiable: true, anotable: false)).first,
                       .boton(.copiar))
        // Cerrar siempre el ultimo.
        for t in [foto, vivo, video, herramienta] { XCTAssertEqual(t.last, .boton(.cerrar)) }
    }

    func testAnotandoLaBarraLlevaLasHerramientas() {
        let p = ReglasBarra.piezas(TipoBarraPin(propio: .zoom, copiable: true, anotable: true, anotando: true))
        XCTAssertEqual(Array(p.prefix(5)), HerramientaAnotacion.allCases.map { PiezaBarra.boton(.herramienta($0)) })
        XCTAssertTrue(p.contains(.boton(.fijar)))
        XCTAssertEqual(p.last, .boton(.listo))
        XCTAssertFalse(p.contains(.boton(.cerrar)))
    }

    func testLasChapasLlevanSuAtajo() {
        XCTAssertEqual(ReglasBarra.chapa(.copiar), "Copiar  ⌘C")
        XCTAssertEqual(ReglasBarra.chapa(.anotar), "Anotar  A")
        XCTAssertEqual(ReglasBarra.chapa(.congelar), "Congelar")
        XCTAssertEqual(ReglasBarra.chapa(.herramienta(.flecha)), "Flecha  2")
    }

    func testLaBarraVaEncimaYSiNoCabeDebajo() {
        let barra = CGSize(width: 300, height: 40)
        let visible = CGRect(x: 0, y: 0, width: 1000, height: 800)
        XCTAssertEqual(ReglasBarra.marco(barra: barra, pin: CGRect(x: 100, y: 100, width: 200, height: 100), visible: visible),
                       CGRect(x: 50, y: 206, width: 300, height: 40))
        let arriba = ReglasBarra.marco(barra: barra, pin: CGRect(x: 100, y: 700, width: 200, height: 100), visible: visible)
        XCTAssertEqual(arriba.minY, 700 - 6 - 40)
        let derecha = ReglasBarra.marco(barra: barra, pin: CGRect(x: 900, y: 100, width: 200, height: 100), visible: visible)
        XCTAssertEqual(derecha.maxX, 1000, "no se sale por el lado")
    }

    func testLosRotulos() {
        XCTAssertEqual(ReglasBarra.textoZoom(150), "150 %")
        XCTAssertEqual(ReglasBarra.textoTiempo(65), "1:05")
        XCTAssertEqual(ReglasBarra.textoTiempo(3725), "1:02:05")
        XCTAssertEqual(ReglasBarra.textoTiempo(.nan), "0:00")
    }

    func testMayusculasMasRuedaUsaElEjeQueTraigaAlgo() {
        XCTAssertEqual(ReglasPin.deltaDeRueda(x: 0, y: -3), -3)
        // Con Mayusculas, la rueda de un raton llega en X.
        XCTAssertEqual(ReglasPin.deltaDeRueda(x: 2, y: 0), 2)
        XCTAssertEqual(ReglasPin.deltaDeRueda(x: .nan, y: 0), 0)
    }
}

final class PanelAbiertosTests: XCTestCase {
    private func f(_ id: Int, _ grupo: Int?, _ nombre: String = "Foto") -> FilaPin {
        FilaPin(id: id, nombre: nombre, detalle: "Foto", tipo: .foto, grupo: grupo, oculto: false)
    }

    func testSeAgrupaPorColorEnElOrdenDeLaPaletaYSinGrupoAlFinal() {
        let g = ReglasPanel.agrupar([f(1, nil), f(2, 2), f(3, 0), f(4, 2), f(5, nil)])
        XCTAssertEqual(g.map(\.grupo), [0, 2, nil])
        XCTAssertEqual(g[1].filas.map(\.id), [2, 4], "en el orden de llegada")
        XCTAssertEqual(g[2].filas.map(\.id), [1, 5])
        XCTAssertTrue(ReglasPanel.agrupar([]).isEmpty)
    }

    func testLaBusquedaNoMiraMayusculasNiTildes() {
        let filas = [f(1, nil, "Cámara.png"), f(2, nil, "plano.pdf")]
        XCTAssertEqual(ReglasPanel.filtrar(filas, busqueda: "CAMARA").map(\.id), [1])
        XCTAssertEqual(ReglasPanel.filtrar(filas, busqueda: "  ").count, 2)
        XCTAssertTrue(ReglasPanel.filtrar(filas, busqueda: "xyz").isEmpty)
        XCTAssertEqual(ColoresGrupo.nombres.count, ColoresGrupo.rgb.count)
    }
}

final class AnotarTests: XCTestCase {
    func testLaPuntaDeLaFlechaEsSimetrica() {
        let (a, b) = ReglasAnotacion.puntaFlecha(desde: .zero, hasta: CGPoint(x: 10, y: 0), largo: 4)
        XCTAssertEqual(a.x, b.x, accuracy: 1e-9)
        XCTAssertEqual(a.y, -b.y, accuracy: 1e-9)
        XCTAssertEqual(a.x, 10 - 4 * cos(CGFloat.pi / 6), accuracy: 1e-9)
        XCTAssertEqual(abs(a.y), 2, accuracy: 1e-9)
    }

    func testRectanguloYPuntosRelativos() {
        XCTAssertEqual(ReglasAnotacion.rectangulo(CGPoint(x: 10, y: 5), CGPoint(x: 2, y: 9)),
                       CGRect(x: 2, y: 5, width: 8, height: 4))
        let tam = CGSize(width: 200, height: 100)
        let p = ReglasAnotacion.normalizar(CGPoint(x: 50, y: 25), en: tam)
        XCTAssertEqual(p, CGPoint(x: 0.25, y: 0.25))
        XCTAssertEqual(ReglasAnotacion.desnormalizar(p, en: CGSize(width: 400, height: 200)), CGPoint(x: 100, y: 50))
        XCTAssertEqual(ReglasAnotacion.normalizar(CGPoint(x: -5, y: 500), en: tam), CGPoint(x: 0, y: 1), "dentro del pin")
    }

    func testQuemarPintaEnLaFotoYNoCambiaSuTamano() throws {
        let ancho = 40, alto = 40
        let espacio = CGColorSpace(name: CGColorSpace.sRGB)!
        let ctx = try XCTUnwrap(CGContext(data: nil, width: ancho, height: alto, bitsPerComponent: 8, bytesPerRow: 0,
                                          space: espacio, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        ctx.setFillColor(CGColor(srgbRed: 1, green: 1, blue: 1, alpha: 1))
        ctx.fill(CGRect(x: 0, y: 0, width: ancho, height: alto))
        let blanca = try XCTUnwrap(ctx.makeImage())
        // Una raya roja de lado a lado por la mitad.
        let raya = TrazoAnotacion(herramienta: .lapiz, color: 0,
                                  puntos: [CGPoint(x: 0, y: 0.5), CGPoint(x: 1, y: 0.5)])
        let quemada = try XCTUnwrap(ReglasAnotacion.quemar([raya], en: blanca))
        XCTAssertEqual(quemada.width, ancho)
        XCTAssertEqual(quemada.height, alto)
        // Se lee en RGBA para mirar el pixel del centro y uno de la esquina.
        let lector = try XCTUnwrap(CGContext(data: nil, width: ancho, height: alto, bitsPerComponent: 8,
                                             bytesPerRow: ancho * 4, space: espacio,
                                             bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        lector.draw(quemada, in: CGRect(x: 0, y: 0, width: ancho, height: alto))
        let datos = try XCTUnwrap(lector.data).assumingMemoryBound(to: UInt8.self)
        func pixel(_ x: Int, _ fila: Int) -> (UInt8, UInt8, UInt8) {
            let i = fila * ancho * 4 + x * 4
            return (datos[i], datos[i + 1], datos[i + 2])
        }
        let centro = pixel(20, alto / 2)
        XCTAssertGreaterThan(centro.0, 200)
        XCTAssertLessThan(centro.1, 120, "rojo")
        let esquina = pixel(2, 2)
        XCTAssertEqual(esquina.0, 255)
        XCTAssertEqual(esquina.1, 255, "lo demas sigue blanco")
        // Sin trazos, la misma foto.
        XCTAssertNotNil(ReglasAnotacion.quemar([], en: blanca))
    }
}

/// Las mini-apps por el puente: el documento es el de Windows y el movil.
final class MiniAppsTests: XCTestCase {
    func testTareasDeIdaYVuelta() throws {
        var d = try XCTUnwrap(miniDocumentoNuevo(miniapp: "tareas", titulo: "Compra", moneda: "EUR"))
        d = miniTareasAnadir(documento: d, texto: "pan", desfaseMs: 0)
        d = miniTareasAnadir(documento: d, texto: "leche", desfaseMs: 0)
        d = miniTareasAlternar(documento: d, indice: 0)
        XCTAssertTrue(d.hasPrefix("# Compra"))
        XCTAssertTrue(d.contains("- [x] pan"))
        let t = miniTareasLeer(documento: d, desfaseMs: 0)
        XCTAssertEqual(t.map(\.texto), ["pan", "leche"])
        XCTAssertEqual(t.map(\.hecha), [true, false])
        XCTAssertEqual(t[0].dias, 0)
        XCTAssertEqual(miniTitulo(documento: d), "Compra")
        XCTAssertEqual(miniResumen(miniapp: "tareas", documento: d, moneda: "EUR")?.hechas, 1)
        d = miniTareasQuitar(documento: d, indice: 0)
        XCTAssertEqual(miniTareasLeer(documento: d, desfaseMs: 0).map(\.texto), ["leche"])
        // Un indice que no existe deja el documento igual.
        XCTAssertEqual(miniTareasQuitar(documento: d, indice: 9), d)
    }

    func testContadorRuletaYGastos() throws {
        var c = try XCTUnwrap(miniDocumentoNuevo(miniapp: "contador", titulo: "Cajas", moneda: "EUR"))
        c = miniContadorCambiar(documento: c, accion: .paso(paso: 12))
        c = miniContadorCambiar(documento: c, accion: .mas)
        XCTAssertEqual(miniContadorLeer(documento: c), CuentaMini(valor: 12, paso: 12))
        c = miniContadorCambiar(documento: c, accion: .reiniciar)
        XCTAssertEqual(miniContadorLeer(documento: c).valor, 0)

        var r = try XCTUnwrap(miniDocumentoNuevo(miniapp: "ruleta", titulo: "", moneda: "EUR"))
        XCTAssertNil(miniRuletaGirar(documento: r, azar: 0.5))
        r = miniRuletaAnadir(documento: r, texto: "ana")
        r = miniRuletaAnadir(documento: r, texto: "luis")
        XCTAssertEqual(miniRuletaLeer(documento: r), ["ana", "luis"])
        let giro = try XCTUnwrap(miniRuletaGirar(documento: r, azar: 0.75))
        XCTAssertEqual(giro.nombre, "luis")
        XCTAssertEqual(giro.angulo, 4 * 360 - 270)

        var g = try XCTUnwrap(miniDocumentoNuevo(miniapp: "gastos", titulo: "Viaje", moneda: "EUR"))
        g = try miniGastosAnadir(documento: g, moneda: "EUR", concepto: "Taxi", importe: "12,5")
        let libro = miniGastosLeer(documento: g, moneda: "USD")
        XCTAssertEqual(libro.moneda, "EUR", "manda la del documento")
        XCTAssertEqual(libro.total, 1250)
        XCTAssertThrowsError(try miniGastosAnadir(documento: g, moneda: "EUR", concepto: "x", importe: "abc"))
        XCTAssertNil(miniDocumentoNuevo(miniapp: "otra", titulo: "", moneda: "EUR"))
    }

    func testUnaMiniappViveEnSuMensajeYNoPisaUnCambioAjeno() throws {
        let dir = try carpetaTemporal("miniapps")
        let nucleo = try Nucleo.abrir(raiz: dir.path, nombreDelAparato: "Mac de prueba")
        let chat = try nucleo.chatGuardados().id
        let m = try nucleo.crearMiniapp(chat: chat, miniapp: "contador", titulo: "Cajas", moneda: "EUR")
        XCTAssertEqual(m.clase, "MINIAPP")
        let d0 = try nucleo.documentoMiniapp(chat: chat, mensaje: m.id)
        let d1 = miniContadorCambiar(documento: d0, accion: .mas)
        XCTAssertTrue(try nucleo.guardarMiniapp(chat: chat, mensaje: m.id, anterior: d0, documento: d1))
        XCTAssertEqual(try nucleo.documentoMiniapp(chat: chat, mensaje: m.id), d1)
        // Sobre un documento viejo no se guarda.
        XCTAssertFalse(try nucleo.guardarMiniapp(chat: chat, mensaje: m.id, anterior: d0,
                                                 documento: miniContadorCambiar(documento: d0, accion: .menos)))
        XCTAssertEqual(miniContadorLeer(documento: try nucleo.documentoMiniapp(chat: chat, mensaje: m.id)).valor, 1)
    }

    @MainActor
    func testUnImporteRaroNoCambiaLosGastos() throws {
        let d = try XCTUnwrap(miniDocumentoNuevo(miniapp: "gastos", titulo: "", moneda: "EUR"))
        let doc = DocumentoMini(miniapp: "gastos", documento: d, nucleo: nil, chat: nil, mensaje: nil)
        XCTAssertFalse(doc.cambiar { try miniGastosAnadir(documento: $0, moneda: "EUR", concepto: "x", importe: "abc") })
        XCTAssertEqual(doc.documento, d)
        XCTAssertTrue(doc.cambiar { try miniGastosAnadir(documento: $0, moneda: "EUR", concepto: "Cafe", importe: "1,20") })
        XCTAssertEqual(miniGastosLeer(documento: doc.documento, moneda: "EUR").total, 120)
    }
}
