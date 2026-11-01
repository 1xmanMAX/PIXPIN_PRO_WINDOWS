; pindesk.ahk — PinDesk desde AutoHotkey v2.
;
;   #Include "C:\Users\<tu>\AppData\Local\Programs\PinDesk\pindesk.ahk"
;
;   ^!t:: PinDeskTarea(InputBox("¿Que hay que hacer?", "PinDesk").Value)
;   ^!c:: PinDeskCapturar()
;   ^!o:: MsgBox(PinDeskTexto("C:\capturas\factura.png"))
;   ^!i:: PinDeskAsistente("resume mis tareas de hoy")
;
; Todo va por la linea de ordenes de pindesk.exe (docs/integraciones.md),
; que manda el pedido a la PinDesk abierta (y la abre si no lo esta). Cada
; funcion devuelve el codigo de salida: 0 aceptado, 1 fallo aqui, 2 version,
; 3 no se entiende, 4 PinDesk no esta abierta, 5 no contesto, 6 rechazado.
;
; El ejecutable: la variable de entorno PINDESK_EXE (o la PIXPIN_EXE de antes), o el que esta junto a
; este fichero, o el instalado en %LOCALAPPDATA%\Programs\PinDesk (o el
; pixpinmax.exe de cuando se llamaba PixPin Max).

#Requires AutoHotkey v2.0

PinDeskExe() {
    for v in ["PINDESK_EXE", "PIXPIN_EXE"] {
        exe := EnvGet(v)
        if (exe != "" && FileExist(exe))
            return exe
    }
    SplitPath(A_LineFile, , &dir)
    for exe in [dir "\pindesk.exe", dir "\pixpinmax.exe"
            , EnvGet("LOCALAPPDATA") "\Programs\PinDesk\pindesk.exe"
            , EnvGet("LOCALAPPDATA") "\Programs\PixPinMax\pixpinmax.exe"] {
        if FileExist(exe)
            return exe
    }
    return EnvGet("LOCALAPPDATA") "\Programs\PinDesk\pindesk.exe"
}

; La linea de ordenes, con cada argumento entre comillas como lo lee Windows
; (CommandLineToArgvW): un texto con comillas o barras llega entero.
PinDeskLinea(args*) {
    linea := '"' PinDeskExe() '"'
    for a in args
        linea .= " " PinDeskArgumento(a)
    return linea
}

PinDeskArgumento(a) {
    a := String(a)
    if (a != "" && !RegExMatch(a, '[\s"]'))
        return a
    s := '"'
    barras := 0
    Loop Parse a {
        c := A_LoopField
        if (c = "\") {
            barras++
            continue
        }
        if (c = '"') {
            s .= PinDesk_Repetir("\", barras * 2 + 1) '"'
            barras := 0
            continue
        }
        s .= PinDesk_Repetir("\", barras) c
        barras := 0
    }
    return s PinDesk_Repetir("\", barras * 2) '"'
}

PinDesk_Repetir(t, n) {
    r := ""
    Loop n
        r .= t
    return r
}

; Corre una orden y espera a que acabe: PinDesk("capturar"), PinDesk("tarea", "pan").
PinDesk(args*) => RunWait(PinDeskLinea(args*), , "Hide")

; Un pedido del protocolo tal cual (docs/protocolo-pedidos.md).
PinDeskPedido(json) => PinDesk("--pedido", json)

; Un enlace pindesk:// (con su lista blanca, como si se pulsara).
PinDeskEnlace(url) => PinDesk("--url", url)

PinDeskConProyecto(verbo, texto, proyecto) {
    if (proyecto = "")
        return PinDesk(verbo, texto)
    return PinDesk(verbo, texto, "--proyecto", proyecto)
}

PinDeskTarea(texto, proyecto := "") => PinDeskConProyecto("tarea", texto, proyecto)
PinDeskNota(texto := "", proyecto := "") => PinDeskConProyecto("nota", texto, proyecto)
PinDeskChat(texto, proyecto := "") => PinDeskConProyecto("chat", texto, proyecto)
PinDeskAsistente(pedido, proyecto := "") => PinDeskConProyecto("asistente", pedido, proyecto)
PinDeskCapturar() => PinDesk("capturar")
PinDeskPinear(ruta) => PinDesk("pinear", ruta)
PinDeskPinearUltima() => PinDesk("pinear-ultima")
PinDeskGrabar(nombre := "Nota de voz") => PinDesk("grabar", nombre)
PinDeskAlMovil(ruta) => PinDesk("al-movil", ruta)

PinDeskAbrir(ruta, pagina := 0) {
    if (pagina > 0)
        return PinDesk("abrir", ruta, "--pagina", pagina)
    return PinDesk("abrir", ruta)
}

; El texto de una imagen (OCR de Windows), sin abrir PinDesk. Pasa por el
; portapapeles (`ocr --copiar`), que se deja como estaba.
PinDeskTexto(ruta) {
    antes := ClipboardAll()
    A_Clipboard := ""
    codigo := PinDesk("ocr", ruta, "--copiar")
    texto := codigo = 0 ? A_Clipboard : ""
    A_Clipboard := antes
    return texto
}

; Los nombres de antes de PinDesk (PixPinTarea…), para los scripts que ya los
; usan. Se pueden quitar cuando nadie los use (docs/formato-pindesk.md).
PixPin(args*) => PinDesk(args*)
PixPinAbrir(args*) => PinDeskAbrir(args*)
PixPinAlMovil(args*) => PinDeskAlMovil(args*)
PixPinAsistente(args*) => PinDeskAsistente(args*)
PixPinCapturar(args*) => PinDeskCapturar(args*)
PixPinChat(args*) => PinDeskChat(args*)
PixPinConProyecto(args*) => PinDeskConProyecto(args*)
PixPinEnlace(args*) => PinDeskEnlace(args*)
PixPinGrabar(args*) => PinDeskGrabar(args*)
PixPinNota(args*) => PinDeskNota(args*)
PixPinPedido(args*) => PinDeskPedido(args*)
PixPinPinear(args*) => PinDeskPinear(args*)
PixPinPinearUltima(args*) => PinDeskPinearUltima(args*)
PixPinTarea(args*) => PinDeskTarea(args*)
PixPinTexto(args*) => PinDeskTexto(args*)
