// Pruebas de lo que se anadio a la captura para igualar a Windows: medidas
// y proporciones, repetir la zona, el color de la lupa, anotar y quemar,
// el cosido con scroll, el GIF (deduplicar y escribir), la pila, el
// selector de proyecto y los gestos con Option. Sin pantallas, permisos ni
// eventos: solo cuentas e imagenes sinteticas.

import CoreGraphics
import Foundation
import ImageIO
@testable import PixPinKit
import XCTest

final class CapturaMasTests: XCTestCase {
    // MARK: - Proporciones

    func testArrastrarConProporcionMandaElLadoMasArrastrado() {
        let a = MedidasCaptura.atado(desde: .zero, hasta: CGPoint(x: 160, y: 10), proporcion: .p16x9)
        XCTAssertEqual(a, CGRect(x: 0, y: 0, width: 160, height: 90))
        let b = MedidasCaptura.atado(desde: .zero, hasta: CGPoint(x: 10, y: 90), proporcion: .p16x9)
        XCTAssertEqual(b, CGRect(x: 0, y: 0, width: 160, height: 90))
        // Hacia abajo a la izquierda: el ancla es la esquina opuesta.
        let c = MedidasCaptura.atado(desde: CGPoint(x: 100, y: 100), hasta: CGPoint(x: 0, y: 95), proporcion: .p1x1)
        XCTAssertEqual(c, CGRect(x: 0, y: 0, width: 100, height: 100))
        // Caso negativo: libre no ata nada.
        XCTAssertEqual(MedidasCaptura.atado(desde: .zero, hasta: CGPoint(x: 30, y: 7), proporcion: .libre),
                       CGRect(x: 0, y: 0, width: 30, height: 7))
    }

    func testPonerProporcionAjustaEnElSitioSinSalirse() {
        let limites = CGRect(x: 0, y: 0, width: 500, height: 500)
        // Se conserva la esquina de arriba a la izquierda y el ancho.
        let r = MedidasCaptura.ajustar(CGRect(x: 10, y: 10, width: 100, height: 100), a: .p4x3, limites: limites)
        XCTAssertEqual(r.minX, 10)
        XCTAssertEqual(r.maxY, 110)
        XCTAssertEqual(r.width, 100)
        XCTAssertEqual(r.height, 75, accuracy: 0.001)
        // No cabe hacia abajo: manda el alto que cabe.
        let s = MedidasCaptura.ajustar(CGRect(x: 0, y: 0, width: 320, height: 50), a: .p16x9, limites: limites)
        XCTAssertEqual(s.height, 50, accuracy: 0.001)
        XCTAssertEqual(s.width, 50 * 16 / 9, accuracy: 0.001)
        XCTAssertEqual(MedidasCaptura.ajustar(s, a: .libre, limites: limites), s)
    }

    // MARK: - Medidas tecleadas

    func testLasMedidasSeLeenComoLasTecleaLaGente() {
        for texto in ["1280x720", "1280 × 720", "1280*720", "1280,720", " 1280 720 ", "1280X720"] {
            let m = MedidasCaptura.leer(texto)
            XCTAssertEqual(m?.ancho, 1280, texto)
            XCTAssertEqual(m?.alto, 720, texto)
        }
        // Casos negativos: cero, seis cifras, un lado, letras, tres lados, signo.
        for texto in ["0x10", "123456x10", "12x", "abc", "10x10x10", "-5x5", "", "1e3x2"] {
            XCTAssertNil(MedidasCaptura.leer(texto), texto)
        }
    }

    func testLasMedidasSalenDeLaEsquinaOCentradas() {
        let limites = CGRect(x: 0, y: 0, width: 1000, height: 800)
        // 200 x 100 pixeles a 2x son 100 x 50 puntos.
        XCTAssertEqual(MedidasCaptura.zona(ancho: 200, alto: 100, escala: 2, seleccion: nil, limites: limites),
                       CGRect(x: 450, y: 375, width: 100, height: 50))
        let sel = CGRect(x: 10, y: 700, width: 300, height: 90)
        XCTAssertEqual(MedidasCaptura.zona(ancho: 200, alto: 100, escala: 2, seleccion: sel, limites: limites),
                       CGRect(x: 10, y: 740, width: 100, height: 50))
        // Mas grande que la pantalla: se recorta a ella.
        let enorme = MedidasCaptura.zona(ancho: 99999, alto: 99999, escala: 1, seleccion: nil, limites: limites)
        XCTAssertEqual(enorme, limites)
    }

    // MARK: - Color de la lupa

    func testElColorSeEscribeComoEnWindows() {
        XCTAssertEqual(MedidasCaptura.hex(255, 87, 51), "#FF5733")
        XCTAssertEqual(MedidasCaptura.hex(0, 0, 0), "#000000")
        XCTAssertEqual(MedidasCaptura.rgb(255, 87, 51), "RGB 255 87 51")
    }

    func testElPixelBajoElRatonTieneLaYDadaLaVuelta() {
        let vista = CGSize(width: 100, height: 50)
        let a = MedidasCaptura.pixel(en: CGPoint(x: 0.5, y: 49.5), tamVista: vista, ancho: 200, alto: 100)
        XCTAssertEqual(a?.x, 1)
        XCTAssertEqual(a?.y, 1)
        let b = MedidasCaptura.pixel(en: CGPoint(x: 99.9, y: 0.1), tamVista: vista, ancho: 200, alto: 100)
        XCTAssertEqual(b?.x, 199)
        XCTAssertEqual(b?.y, 99)
        XCTAssertNil(MedidasCaptura.pixel(en: CGPoint(x: -1, y: 10), tamVista: vista, ancho: 200, alto: 100))
    }

    func testElColorDeLaFotoSeLeeDeUnSoloPixel() throws {
        let foto = try XCTUnwrap(imagen(ancho: 4, alto: 4) { x, y in x == 3 && y == 0 ? (255, 87, 51) : (10, 20, 30) })
        // Arriba a la derecha en la foto es arriba a la derecha en la vista.
        let c = try XCTUnwrap(VistaCaptura.color(de: foto, en: CGPoint(x: 3.5, y: 3.5), tamVista: CGSize(width: 4, height: 4)))
        XCTAssertEqual(c.0, 255)
        XCTAssertEqual(c.1, 87)
        XCTAssertEqual(c.2, 51)
    }

    // MARK: - Repetir la ultima zona

    func testLaUltimaZonaSeRecuerdaPorPantalla() throws {
        let nombre = "pixpin-pruebas-\(UUID().uuidString)"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: nombre))
        defer { defaults.removePersistentDomain(forName: nombre) }
        let ultima = UltimaZona(defaults: defaults)
        XCTAssertNil(ultima.leer(pantalla: 1))
        ultima.guardar(CGRect(x: 10, y: 20, width: 300, height: 200), pantalla: 1)
        XCTAssertEqual(ultima.leer(pantalla: 1), CGRect(x: 10, y: 20, width: 300, height: 200))
        XCTAssertNil(ultima.leer(pantalla: 2), "cada pantalla la suya")
        // Caso negativo: una zona vacia no pisa la buena.
        ultima.guardar(.zero, pantalla: 1)
        XCTAssertEqual(ultima.leer(pantalla: 1), CGRect(x: 10, y: 20, width: 300, height: 200))
    }

    // MARK: - Anotar

    func testCogerLaMismaHerramientaDosVecesLaSuelta() {
        var a = AnotacionCaptura()
        a.tomar(.flecha)
        XCTAssertEqual(a.herramienta, .flecha)
        a.tomar(.flecha)
        XCTAssertNil(a.herramienta)
        XCTAssertEqual(HerramientaAnotar.deTecla("5"), .mosaico)
        XCTAssertNil(HerramientaAnotar.deTecla("6"))
    }

    func testUnRectanguloSeDibujaSeDeshaceYSeRehace() {
        var a = AnotacionCaptura()
        a.tomar(.rectangulo)
        a.ponerColor(3)
        a.pulsar(CGPoint(x: 50, y: 50))
        a.mover(CGPoint(x: 10, y: 80))
        a.soltar(CGPoint(x: 10, y: 80))
        XCTAssertEqual(a.figuras.count, 1)
        XCTAssertEqual(a.figuras[0].clase, .rectangulo(CGRect(x: 10, y: 50, width: 40, height: 30)))
        XCTAssertEqual(a.figuras[0].color, 3)
        XCTAssertTrue(a.deshacer())
        XCTAssertTrue(a.figuras.isEmpty)
        XCTAssertTrue(a.rehacer())
        XCTAssertEqual(a.figuras.count, 1)
        // Caso negativo: un clic sin arrastrar no deja un rectangulo.
        a.pulsar(CGPoint(x: 5, y: 5))
        a.soltar(CGPoint(x: 5, y: 5))
        XCTAssertEqual(a.figuras.count, 1)
        // Sin herramienta, el clic no dibuja.
        a.tomar(.rectangulo)
        a.pulsar(CGPoint(x: 1, y: 1))
        XCTAssertNil(a.enCurso)
    }

    func testElTextoSeEscribeBorraYCierra() {
        var a = AnotacionCaptura()
        a.tomar(.texto)
        a.pulsar(CGPoint(x: 4, y: 4))
        XCTAssertTrue(a.escribiendo)
        a.escribir("Hola\n")
        a.borrarLetra()
        XCTAssertEqual(a.enCurso?.clase, .texto("Hol", en: CGPoint(x: 4, y: 4)))
        XCTAssertTrue(a.cerrarTexto())
        XCTAssertEqual(a.figuras.count, 1)
        // Caso negativo: un texto vacio no queda.
        a.pulsar(CGPoint(x: 9, y: 9))
        XCTAssertTrue(a.cerrarTexto())
        XCTAssertEqual(a.figuras.count, 1)
    }

    func testLaFlechaTieneLasAlasDetrasDeLaPunta() {
        let (i, d) = AnotacionCaptura.alas(desde: .zero, hasta: CGPoint(x: 100, y: 0), grosor: 4)
        XCTAssertLessThan(i.x, 100)
        XCTAssertLessThan(d.x, 100)
        XCTAssertEqual(i.y, -d.y, accuracy: 0.001)
        XCTAssertNotNil(AnotacionCaptura.camino(de: FiguraAnotada(clase: .flecha(desde: .zero, hasta: CGPoint(x: 9, y: 9)),
                                                                 color: 0, grosor: 1)))
        XCTAssertNil(AnotacionCaptura.camino(de: FiguraAnotada(clase: .mosaico(.zero), color: 0, grosor: 1)))
    }

    func testLoDibujadoEntraEnLaImagenYSinDibujoNoSeCopia() throws {
        let foto = try XCTUnwrap(imagen(ancho: 40, alto: 40) { _, _ in (255, 255, 255) })
        XCTAssertTrue(AnotacionCaptura.quemar([], en: foto, escala: 2) === foto, "sin figuras, la misma imagen")
        // Un rectangulo rojo de 2 a 18 puntos (4 a 36 pixeles a 2x).
        let f = FiguraAnotada(clase: .rectangulo(CGRect(x: 2, y: 2, width: 16, height: 16)), color: 0, grosor: 1)
        let hecha = AnotacionCaptura.quemar([f], en: foto, escala: 2)
        XCTAssertEqual(hecha.width, 40)
        let px = try pixeles(hecha)
        let borde = px(4, 20)
        XCTAssertGreaterThan(borde.0, 200)
        XCTAssertLessThan(borde.1, 120, "el borde es rojo")
        let centro = px(20, 20)
        XCTAssertEqual(centro.0, 255)
        XCTAssertEqual(centro.1, 255, "dentro del rectangulo no se pinta")
    }

    func testElMosaicoTapaSuZonaYNoLaDeFuera() throws {
        // Un degradado: cada columna de otro gris.
        let foto = try XCTUnwrap(imagen(ancho: 64, alto: 64) { x, _ in (UInt8(x * 4), UInt8(x * 4), UInt8(x * 4)) })
        let f = FiguraAnotada(clase: .mosaico(CGRect(x: 0, y: 0, width: 32, height: 64)), color: 0, grosor: 2)
        let hecha = AnotacionCaptura.quemar([f], en: foto, escala: 1)
        let px = try pixeles(hecha)
        // Dentro, dos columnas vecinas del mismo cuadro (16 px) son iguales.
        XCTAssertEqual(px(1, 10).0, px(2, 10).0)
        // Fuera, el degradado sigue tal cual.
        XCTAssertEqual(px(40, 10).0, 160)
        XCTAssertEqual(px(41, 10).0, 164)
    }

    // MARK: - Cosido con scroll

    func testElDesplazamientoSeEncuentraYLoLisoNoEncaja() {
        var g = Generador(semilla: 7)
        let marco = (0 ..< 300).map { _ in Int64(g.siguiente() % 5000) }
        let cola = Array(marco[50 ..< 98])
        XCTAssertEqual(CosidoScroll.encontrarDesplazamiento(cola: cola, marco: marco), 50)
        // Casos negativos: una banda lisa no sirve y lo liso no encaja.
        let liso = [Int64](repeating: 1000, count: 300)
        XCTAssertTrue(CosidoScroll.esLisa(liso[0 ..< 48]))
        XCTAssertEqual(CosidoScroll.encontrarDesplazamiento(cola: Array(liso[0 ..< 48]), marco: liso),
                       CosidoScroll.sinEncaje)
    }

    func testLasFranjasFijasSeDetectanArribaYAbajo() {
        var g = Generador(semilla: 3)
        var a = (0 ..< 100).map { _ in Int64(g.siguiente() % 9000) }
        var b = (0 ..< 100).map { _ in Int64(g.siguiente() % 9000) }
        for i in 0 ..< 5 { b[i] = a[i] }
        for i in 0 ..< 3 { b[99 - i] = a[99 - i] }
        // Justo despues de las franjas, seguro que distintas.
        b[5] = a[5] + 5000
        b[96] = a[96] + 5000
        XCTAssertEqual(CosidoScroll.franjasFijas(a, b).0, 5)
        XCTAssertEqual(CosidoScroll.franjasFijas(a, b).1, 3)
        // Todo igual: no se movio, no hay franjas.
        a = b
        XCTAssertEqual(CosidoScroll.franjasFijas(a, b).0, 0)
        XCTAssertEqual(CosidoScroll.franjasFijas(a, b).1, 0)
    }

    func testTresFotogramasSeCosenEnLaPaginaEntera() {
        let ancho = 64
        let pagina = paginaAleatoria(ancho: ancho, alto: 500, semilla: 11)
        var c = CosidoScroll.Cosedor(ancho: ancho)
        for (desde, esperado) in [(0, CosidoScroll.Resultado.primero), (100, .anadido), (200, .anadido)] {
            let r = conFotograma(pagina, ancho: ancho, desde: desde, alto: 300) { base in
                c.anadir(base, ancho: ancho, alto: 300, bytesPorFila: ancho * 4)
            }
            XCTAssertEqual(r, esperado, "fotograma en \(desde)")
        }
        // El mismo otra vez: no se movio.
        let quieto = conFotograma(pagina, ancho: ancho, desde: 200, alto: 300) { base in
            c.anadir(base, ancho: ancho, alto: 300, bytesPorFila: ancho * 4)
        }
        XCTAssertEqual(quieto, .sinMovimiento)
        let hecho = c.terminarBytes()
        XCTAssertEqual(hecho?.alto, 500)
        XCTAssertEqual(hecho?.bytes, pagina, "la pagina cosida es la original, fila a fila")
        XCTAssertEqual(c.terminar()?.height, 500)
    }

    func testElCosidoRespetaElAltoMaximo() {
        let ancho = 32
        let pagina = paginaAleatoria(ancho: ancho, alto: 400, semilla: 5)
        var c = CosidoScroll.Cosedor(ancho: ancho, altoMaximo: 250)
        _ = conFotograma(pagina, ancho: ancho, desde: 0, alto: 200) { c.anadir($0, ancho: ancho, alto: 200, bytesPorFila: ancho * 4) }
        let r = conFotograma(pagina, ancho: ancho, desde: 100, alto: 200) {
            c.anadir($0, ancho: ancho, alto: 200, bytesPorFila: ancho * 4)
        }
        XCTAssertEqual(r, .lleno)
        XCTAssertEqual(c.alto, 200)
        // Caso negativo: un fotograma de otro ancho no se cose.
        let otro = conFotograma(pagina, ancho: ancho, desde: 0, alto: 200) {
            c.anadir($0, ancho: ancho - 1, alto: 200, bytesPorFila: ancho * 4)
        }
        XCTAssertEqual(otro, .incierto)
    }

    // MARK: - GIF

    func testLosFotogramasRepetidosAlarganElAnterior() {
        var d = DeduplicadorGif()
        XCTAssertEqual(d.anadir(firma: 1, tiempo: 10.0), .nuevo(retrasoDelAnterior: nil))
        XCTAssertEqual(d.anadir(firma: 1, tiempo: 10.1), .descartar)
        XCTAssertEqual(d.anadir(firma: 1, tiempo: 10.2), .descartar)
        XCTAssertEqual(d.anadir(firma: 2, tiempo: 10.3), .nuevo(retrasoDelAnterior: 30))
        XCTAssertEqual(d.distintos, 2)
        XCTAssertEqual(d.repetidos, 2)
        XCTAssertEqual(d.retrasoFinal(hasta: 10.31), DeduplicadorGif.minimoCentesimas)
        XCTAssertEqual(DeduplicadorGif.centesimas(desde: 0, hasta: 10_000), DeduplicadorGif.maximoCentesimas)
    }

    func testLaFirmaDistingueFotogramas() {
        var a = [UInt32](repeating: 0xFF10_2030, count: 16 * 8)
        let fa = a.withUnsafeBytes { DeduplicadorGif.firma($0.baseAddress!, ancho: 16, alto: 8, bytesPorFila: 64) }
        let fa2 = a.withUnsafeBytes { DeduplicadorGif.firma($0.baseAddress!, ancho: 16, alto: 8, bytesPorFila: 64) }
        XCTAssertEqual(fa, fa2)
        a[4] = 0xFF00_0000
        let fb = a.withUnsafeBytes { DeduplicadorGif.firma($0.baseAddress!, ancho: 16, alto: 8, bytesPorFila: 64) }
        XCTAssertNotEqual(fa, fb)
    }

    func testElGifSaleReducidoYSinDeformar() {
        let t = DeduplicadorGif.tamSalida(puntos: CGSize(width: 1920, height: 1080))
        XCTAssertEqual(t.ancho, 960)
        XCTAssertEqual(t.alto, 540)
        let p = DeduplicadorGif.tamSalida(puntos: CGSize(width: 100, height: 50))
        XCTAssertEqual(p.ancho, 100)
        XCTAssertEqual(p.alto, 50)
    }

    func testElGifSeEscribeConSusFotogramasYRetrasos() throws {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("pixpin-prueba-\(UUID().uuidString).gif")
        defer { try? FileManager.default.removeItem(at: url) }
        let rojo = try XCTUnwrap(imagen(ancho: 8, alto: 8) { _, _ in (255, 0, 0) })
        let azul = try XCTUnwrap(imagen(ancho: 8, alto: 8) { _, _ in (0, 0, 255) })
        XCTAssertTrue(SalidaGif.escribirGif([(rojo, 30), (azul, 10), (rojo, 50)], en: url))
        let fuente = try XCTUnwrap(CGImageSourceCreateWithURL(url as CFURL, nil))
        XCTAssertEqual(CGImageSourceGetCount(fuente), 3)
        let props = CGImageSourceCopyPropertiesAtIndex(fuente, 0, nil) as? [CFString: Any]
        let gif = props?[kCGImagePropertyGIFDictionary] as? [CFString: Any]
        let retraso = (gif?[kCGImagePropertyGIFUnclampedDelayTime] as? Double) ?? (gif?[kCGImagePropertyGIFDelayTime] as? Double)
        XCTAssertEqual(retraso ?? 0, 0.3, accuracy: 0.011)
        // Caso negativo: sin fotogramas no hay GIF.
        XCTAssertFalse(SalidaGif.escribirGif([], en: url.appendingPathExtension("vacio")))
    }

    // MARK: - Pila

    func testLaTandaApilaCierraYSeArma() {
        var t = TandaPila()
        let f = { (n: Int) in URL(fileURLWithPath: "/tmp/captura-\(n).png") }
        XCTAssertTrue(t.meter(f(1), en: 0))
        XCTAssertFalse(t.meter(f(2), en: 5), "dentro de los 10 s se apila")
        XCTAssertEqual(t.cuantas, 2)
        XCTAssertTrue(t.meter(f(3), en: 30), "pasado el plazo empieza otra")
        XCTAssertEqual(t.ficheros, [f(3)])
        t.armada = true
        XCTAssertFalse(t.meter(f(4), en: 1000), "armada no se cierra sola")
        for n in 0 ..< 40 { t.meter(f(100 + n), en: 1001) }
        XCTAssertEqual(t.cuantas, TandaPila.tope)
        XCTAssertEqual(t.ficheros.last, f(139), "la mas vieja sale por abajo")
        t.vaciar()
        XCTAssertFalse(t.abierta(en: 1001))
        XCTAssertEqual(t.cuantas, 0)
    }

    // MARK: - Selector de proyecto

    func testElUltimoProyectoSaleElPrimeroYSeBuscaSinTildes() {
        let v = [("a", "Guardados"), ("b", "Obra"), ("c", "Tésis")]
        let o = ListaDeProyectos.ordenar(v, id: { $0.0 }, ultimo: "c")
        XCTAssertEqual(o.map { $0.0 }, ["c", "a", "b"])
        XCTAssertEqual(ListaDeProyectos.ordenar(v, id: { $0.0 }, ultimo: nil).map { $0.0 }, ["a", "b", "c"])
        XCTAssertEqual(ListaDeProyectos.ordenar(v, id: { $0.0 }, ultimo: "zz").map { $0.0 }, ["a", "b", "c"])
        XCTAssertEqual(ListaDeProyectos.filtrar(v, nombre: { $0.1 }, texto: "TESIS").map { $0.0 }, ["c"])
        XCTAssertEqual(ListaDeProyectos.filtrar(v, nombre: { $0.1 }, texto: " ").count, 3)
        XCTAssertTrue(ListaDeProyectos.filtrar(v, nombre: { $0.1 }, texto: "xyz").isEmpty)
        XCTAssertEqual(ListaDeProyectos.nombreDeFichero(fecha: Date(timeIntervalSince1970: 1.2345)), "captura-1234.png")
    }

    // MARK: - Gestos y marcos

    func testLosBotonesDeLosGestos() {
        XCTAssertEqual(GeometriaGestos.boton(tipo: .leftMouseDragged, numero: 0), .izquierdo)
        XCTAssertEqual(GeometriaGestos.boton(tipo: .rightMouseUp, numero: 1), .derecho)
        XCTAssertEqual(GeometriaGestos.boton(tipo: .otherMouseDown, numero: 2), .central)
        XCTAssertNil(GeometriaGestos.boton(tipo: .otherMouseDown, numero: 3), "los botones laterales no")
        XCTAssertTrue(GeometriaGestos.soloOpcion(.maskAlternate))
        XCTAssertFalse(GeometriaGestos.soloOpcion([.maskAlternate, .maskShift]))
        XCTAssertFalse(GeometriaGestos.soloOpcion([]))
        XCTAssertTrue(GeometriaGestos.esGesto(desde: .zero, hasta: CGPoint(x: 10, y: 10)))
        XCTAssertFalse(GeometriaGestos.esGesto(desde: .zero, hasta: CGPoint(x: 10, y: 2)), "una raya no es una zona")
    }

    func testElMarcoDeLaZonaVaEnCoordenadasDeAppKit() {
        let r = MarcoDeZona.marcoGlobal(CGRect(x: 10, y: 20, width: 100, height: 50),
                                        pantalla: CGRect(x: 1440, y: 0, width: 1440, height: 900))
        XCTAssertEqual(r, CGRect(x: 1450, y: 830, width: 100, height: 50))
    }

    // MARK: - Ayudas

    /// Una imagen sRGB de `ancho` x `alto` con el color de cada pixel (x, y
    /// con y desde arriba).
    private func imagen(ancho: Int, alto: Int, color: (Int, Int) -> (UInt8, UInt8, UInt8)) -> CGImage? {
        var bytes = [UInt8](repeating: 255, count: ancho * alto * 4)
        for y in 0 ..< alto {
            for x in 0 ..< ancho {
                let (r, g, b) = color(x, y)
                let i = (y * ancho + x) * 4
                bytes[i] = r
                bytes[i + 1] = g
                bytes[i + 2] = b
            }
        }
        guard let proveedor = CGDataProvider(data: Data(bytes) as CFData) else { return nil }
        return CGImage(width: ancho, height: alto, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: ancho * 4,
                       space: CGColorSpace(name: CGColorSpace.sRGB)!,
                       bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.noneSkipLast.rawValue),
                       provider: proveedor, decode: nil, shouldInterpolate: false, intent: .defaultIntent)
    }

    /// Los pixeles de una imagen en RGBA, para mirar (x, y desde arriba).
    private func pixeles(_ img: CGImage) throws -> (Int, Int) -> (UInt8, UInt8, UInt8) {
        let w = img.width
        let h = img.height
        var bytes = [UInt8](repeating: 0, count: w * h * 4)
        let ok = bytes.withUnsafeMutableBytes { buf -> Bool in
            guard let ctx = CGContext(data: buf.baseAddress, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                                      space: CGColorSpace(name: CGColorSpace.sRGB)!,
                                      bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return false }
            ctx.draw(img, in: CGRect(x: 0, y: 0, width: w, height: h))
            return true
        }
        XCTAssertTrue(ok)
        let copia = bytes
        return { x, y in
            let i = (y * w + x) * 4
            return (copia[i], copia[i + 1], copia[i + 2])
        }
    }

    /// Una pagina BGRA con cada fila distinta (textura aleatoria).
    private func paginaAleatoria(ancho: Int, alto: Int, semilla: UInt64) -> Data {
        var g = Generador(semilla: semilla)
        var d = Data(count: ancho * alto * 4)
        for i in 0 ..< d.count { d[i] = (i % 4 == 3) ? 255 : UInt8(truncatingIfNeeded: g.siguiente()) }
        return d
    }

    /// Las filas `desde ..< desde + alto` de la pagina, como un fotograma.
    private func conFotograma<T>(_ pagina: Data, ancho: Int, desde: Int, alto: Int,
                                 _ hacer: (UnsafePointer<UInt8>) -> T) -> T {
        let trozo = pagina.subdata(in: (desde * ancho * 4) ..< ((desde + alto) * ancho * 4))
        return trozo.withUnsafeBytes { hacer($0.bindMemory(to: UInt8.self).baseAddress!) }
    }
}

/// Generador determinista (LCG), para que las pruebas no dependan del azar.
private struct Generador {
    var estado: UInt64
    init(semilla: UInt64) { estado = semilla &* 0x9E37_79B9_7F4A_7C15 | 1 }
    mutating func siguiente() -> UInt64 {
        estado = estado &* 6_364_136_223_846_793_005 &+ 1_442_695_040_888_963_407
        return estado >> 33
    }
}
