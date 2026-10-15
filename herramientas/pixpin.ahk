; pixpin.ahk — PixPin Max desde AutoHotkey v2.
;
;   #Include "C:\Users\<tu>\AppData\Local\Programs\PixPinMax\pixpin.ahk"
;
;   ^!t:: PixPinTarea(InputBox("¿Que hay que hacer?", "PixPin").Value)
;   ^!c:: PixPinCapturar()
;   ^!o:: MsgBox(PixPinTexto("C:\capturas\factura.png"))
;   ^!i:: PixPinAsistente("resume mis tareas de hoy")
;
; Todo va por la linea de ordenes de pixpinmax.exe (docs/integraciones.md),
; que manda el pedido a la PixPin abierta (y la abre si no lo esta). Cada
; funcion devuelve el codigo de salida: 0 aceptado, 1 fallo aqui, 2 version,
; 3 no se entiende, 4 PixPin no esta abierta, 5 no contesto, 6 rechazado.
;
; El ejecutable: la variable de entorno PIXPIN_EXE, o el que esta junto a
; este fichero, o el instalado en %LOCALAPPDATA%\Programs\PixPinMax.

#Requires AutoHotkey v2.0

PixPinExe() {
    exe := EnvGet("PIXPIN_EXE")
    if (exe != "" && FileExist(exe))
        return exe
    SplitPath(A_LineFile, , &dir)
    if FileExist(dir "\pixpinmax.exe")
        return dir "\pixpinmax.exe"
    return EnvGet("LOCALAPPDATA") "\Programs\PixPinMax\pixpinmax.exe"
}

; La linea de ordenes, con cada argumento entre comillas como lo lee Windows
; (CommandLineToArgvW): un texto con comillas o barras llega entero.
PixPinLinea(args*) {
    linea := '"' PixPinExe() '"'
    for a in args
        linea .= " " PixPinArgumento(a)
    return linea
}

PixPinArgumento(a) {
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
            s .= PixPin_Repetir("\", barras * 2 + 1) '"'
            barras := 0
            continue
        }
        s .= PixPin_Repetir("\", barras) c
        barras := 0
    }
    return s PixPin_Repetir("\", barras * 2) '"'
}

PixPin_Repetir(t, n) {
    r := ""
    Loop n
        r .= t
    return r
}

; Corre una orden y espera a que acabe: PixPin("capturar"), PixPin("tarea", "pan").
PixPin(args*) => RunWait(PixPinLinea(args*), , "Hide")

; Un pedido del protocolo tal cual (docs/protocolo-pedidos.md).
PixPinPedido(json) => PixPin("--pedido", json)

; Un enlace pixpin:// (con su lista blanca, como si se pulsara).
PixPinEnlace(url) => PixPin("--url", url)

PixPinConProyecto(verbo, texto, proyecto) {
    if (proyecto = "")
        return PixPin(verbo, texto)
    return PixPin(verbo, texto, "--proyecto", proyecto)
}

PixPinTarea(texto, proyecto := "") => PixPinConProyecto("tarea", texto, proyecto)
PixPinNota(texto := "", proyecto := "") => PixPinConProyecto("nota", texto, proyecto)
PixPinChat(texto, proyecto := "") => PixPinConProyecto("chat", texto, proyecto)
PixPinAsistente(pedido, proyecto := "") => PixPinConProyecto("asistente", pedido, proyecto)
PixPinCapturar() => PixPin("capturar")
PixPinPinear(ruta) => PixPin("pinear", ruta)
PixPinPinearUltima() => PixPin("pinear-ultima")
PixPinGrabar(nombre := "Nota de voz") => PixPin("grabar", nombre)
PixPinAlMovil(ruta) => PixPin("al-movil", ruta)

PixPinAbrir(ruta, pagina := 0) {
    if (pagina > 0)
        return PixPin("abrir", ruta, "--pagina", pagina)
    return PixPin("abrir", ruta)
}

; El texto de una imagen (OCR de Windows), sin abrir PixPin. Pasa por el
; portapapeles (`ocr --copiar`), que se deja como estaba.
PixPinTexto(ruta) {
    antes := ClipboardAll()
    A_Clipboard := ""
    codigo := PixPin("ocr", ruta, "--copiar")
    texto := codigo = 0 ? A_Clipboard : ""
    A_Clipboard := antes
    return texto
}
