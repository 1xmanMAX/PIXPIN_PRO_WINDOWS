# Sincronizar en Mac: lo que hace y como

Estado del 15-oct-2026. Es el mismo protocolo (`pixpin-sincro`, version 4) y el mismo
formato en disco que Windows y Android: el Mac es un aparato mas del grupo. Aqui va lo que
el Mac anade por encima (transporte, hilos, Swift) y lo que se decidio.

## Lo que lleva cada conexion

| Quien llama | Los 4 primeros bytes | Que se atiende | Codigo |
|---|---|---|---|
| La presencia de otro aparato | `PING` | La sonda: `PONG` y nada mas | `Escucha::atender` |
| Un aparato del grupo | `PXS1` | Unirse, una vuelta (chats + **galeria de capturas**), **archivos sueltos** | `Respondedor::atender_con_galeria` con `CapturasDelMac` y `RecibeAlFrente` |
| Una persona de un chat compartido | `PXC1` | Solo los chats que compartimos, **con la valla** | `Respondedor::atender_persona` |
| Quien entra con una invitacion | `PXI1` | Solo en la puerta de la invitacion (`Invitacion`) | `Respondedor::atender_invitacion` |

Y fuera del puerto de siempre, cada una en su puerto propio mientras su ventana esta abierta:
`PuertaEnvio` (`_pixpinenvio._tcp`), `PuertaRecibir` (`_pixpinrecibe._tcp`) e `Invitacion`
(`_pixpinchat._tcp`).

### Galeria de capturas

`sincronizar_con` hace `Sesion::con_galeria(&CapturasDelMac)` y la escucha contesta las cuatro
peticiones (`galeria`, `galeriajunta`, `damecaptura`, `poncaptura`) igual que `una_vuelta` y
`responder` de Windows. `CapturasDelMac` (`pixpin-nucleo/src/galeria_sincro.rs`) es
`CapturasDelPc` de Windows linea a linea: `capturas/` con la fecha de modificacion, el
registro `capturas-caducidad.json` (con el cerrojo de `galeria.rs`), lo quitado en otro
aparato a `papelera/capturas/` y «dias hasta borrar» de `pixpinmax.toml`. El estado
compartido es `galeria-compartida.json`, el de siempre.

### «Enviar a…» (archivo suelto)

`Nucleo::enviar_a_aparato(host, puerto, rutas)` es `al_lienzo::Conexion` (canal del grupo,
sin codigos). Al recibir, lo que esta delante lo dice Swift con `Nucleo::poner_al_frente`
(`FrenteMac::Chat` al abrir un chat en Proyectos, `Nada` al cerrar la ventana):

- chat abierto: cualquier archivo, como un adjunto (`Nucleo::adjuntar`);
- lienzo delante (`FrenteMac::Lienzo`): solo fotos; la foto queda en `sincro/al-lienzo/` y
  se avisa con `Notification.Name.pixpinFotoAlLienzo`. **El editor del lienzo del Mac aun no
  sabe meter fotos**, asi que la app no pone `Lienzo` todavia: lo que llega va al chat;
- nada: «Mensajes guardados».

### Envio con codigo de seis cifras

`PuertaEnvio` (quien envia espera y aprueba a cada uno; avisos por `siguiente_aviso`, que
bloquea sin sondear), `PuertaRecibir` (quien recibe espera), `recibir_envio` y
`mandar_a_quien_espera` (quien tiene el codigo del otro). Cinco codigos malos queman la
puerta. Lo recibido va como en Windows (`recibir.rs`): un proyecto a la lista (encima del
mismo, con `copias::hacer` antes, o nuevo con `importar_paquete`) y lo demas a «Mensajes
guardados» con `nombre_por_contenido`. Falta respecto a Windows: un lienzo suelto del movil
va como fichero a «Mensajes guardados» (no a su proyecto), no se pregunta «actualizar o
crear como nuevo», no se reconoce un archivo que vuelve (los tres codigos) y desde el Mac
solo se mandan archivos (no proyectos enteros).

### Chats compartidos con personas

`pixpin-nucleo/src/compartidos.rs` sobre `pixpin_sincro::compartidos` e `invitacion`:
compartir, invitar (codigo de ocho signos + QR `pixpin-chat:1:<codigo>:<ip>:<puerto>`,
pintado en Swift con `CIQRCodeGenerator`), entrar (`invitacion::entrar` + la primera
`vuelta_con_persona`), sincronizar con cada persona que este en la Wi-Fi (por su `id` en el
TXT de `_pixpin._tcp`), quitar y salir. La valla (`pixpin_sincro::valla` y
`AccesoDePersona`) la aplica el crate: ningun fichero de otro chat sale ni entra.

Sin grupo pero dentro de algun chat compartido, el Mac escucha y se anuncia igual
(`Nucleo::anuncio_de_presencia`: el anuncio sin `g`), como la presencia de Windows.

**Las claves.** El par P-256 vive en `<datos>/sincro/persona.json` (publica X.509 y
privada PKCS#8 en base64), como en Windows y Android. Windows **no** usa DPAPI para este
fichero (DPAPI es solo para las claves de la nube del asistente, `pixpin_ia::nube::claves`),
asi que el Mac tampoco usa el Llavero: mismo fichero, mismas reglas, y una carpeta de datos
copiada de un aparato a otro sigue funcionando. Si algun dia Windows protege
`persona.json`, en Mac se haria con el mismo patron que `pixpin_ia::nube::claves::Llavero`:
un trait que implementa Swift con Keychain (`kSecClassGenericPassword`, servicio
`PixPin`, cuenta `persona`), instalado al arrancar.

## Varias conexiones a la vez

Swift deja `hilos_de_escucha()` (4) hilos esperando en la misma `Escucha::atender`: `accept`
es seguro desde varios hilos y cada uno atiende la conexion que le toca. Asi una sonda o un
archivo suelto no esperan a que acabe una vuelta (Windows abre un hilo por conexion; aqui el
tope es fijo y cada hilo dormido en `accept` no gasta nada). Lo que escribe sigue siendo
seguro:

- una sola vuelta por carpeta de datos (`protocolo::ocupar`, por raiz): la segunda oye
  «ocupado», tambien si este Mac esta dirigiendo una;
- cada cuaderno se escribe con el cerrojo de `pixpin_proyecto::cuaderno`;
- `compartidos.json`, `capturas-caducidad.json` y `galeria-compartida.json` tienen su cerrojo.

`Escucha::cerrar` despierta a todos los hilos que esperan (una conexion a si misma por hilo).
Las puertas de envio e invitacion ignoran las conexiones que cuelgan sin hablar (la que abre
Swift para saber la direccion de un servicio de Bonjour).

## IPv6

`pixpin-nucleo/src/red.rs`: el `host` puede ser IPv4, IPv6 (`::1`, `[2001:db8::1]`), IPv6 de
enlace local con su interfaz (`fe80::1%en0` o `%4`) o un nombre. Una IP escrita no pasa por el
DNS; la interfaz por nombre la resuelve `getaddrinfo` del sistema. Con varias direcciones se
prueban **antes las IPv4** (Windows y Android escuchan en `0.0.0.0`). Todo escucha en `::`
(con IPv4 dentro) y, si no hay IPv6, en `0.0.0.0`. Swift da el host tal como lo resuelve
Network.framework (`Vecinos.textoDeHost`, con `%interfaz` en las de enlace local).

Los QR solo llevan IPv4: se parten por `:` en Windows y Android y una IPv6 los romperia. Sin
IPv4 el host va vacio y el otro busca en Bonjour por la etiqueta del codigo.

## Negociar la version del protocolo (preparado, apagado)

Hoy el `hola` lleva `version: 4` y Windows y Android exigen exactamente 4. **No se cambia el
cable.** El diseño, compatible hacia atras, esta en `pixpin-nucleo/src/version.rs` detras de
`NEGOCIAR = false`:

1. `hola.version` sigue siendo 4 mientras quede algun aparato de antes.
2. Dos campos nuevos y opcionales en `Hola`: `versionMin` y `versionMax` (no se escriben si
   valen lo mismo que `version`). Android los ignora (`ignoreUnknownKeys`) y Rust tambien
   (`serde` sin `deny_unknown_fields`).
3. Quien los entiende elige la **mayor comun** (`version_comun`); quien no los trae solo sabe
   su `version` (`rango(v, None, None) == (v, v)`).
4. Quien responde devuelve en su `hola` la elegida en `version` y su rango; desde ahi los dos
   hablan la elegida.
5. Para pasar a 5: los aparatos nuevos saben 4 y 5; con uno de antes eligen 4, entre nuevos 5.
   Cuando ya no quede ninguno de antes, `version` sube a 5 en el cable.

Encenderlo exige cambiar a la vez `pixpin_sincro::mensajes::Hola` aqui y en `crates/`, y
`Hola` de Android, y las comprobaciones `hola.version == VERSION` de `Sesion`,
`al_lienzo::Conexion` y del PC.
