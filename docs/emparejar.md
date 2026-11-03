# Emparejar con el código, el QR que se lee con cualquier cámara y sincronizar en segundo plano

> 3-nov-2026. El usuario: «cuando escribo el código de sincronización **no aparece el aparato**:
> tengo que escribir su dirección (IP). Antes no pasaba: en la misma red todo se conectaba solo con
> el código, y rápido. Además: esa pantalla debería usar los enlaces externos de la app, para que
> enseñe un **QR que se pueda leer con CUALQUIER cámara** y lleve directo a la app y empareje, sin
> abrirla antes. Y que la sincronización también vaya **en segundo plano siempre que se pueda**, para
> tenerlo todo al día».

Código: `crates/pindesk-sincro/src/emparejar.rs` (lo puro: la pregunta por la baliza, buscar sin
rendirse, el enlace), `crates/pindesk-shell/src/cortafuegos.rs` (el permiso de red),
`apps/pindesk/src/sincronizar.rs` (`buscar_para_unirse`, el QR), `sincronizar/baliza.rs`
(`buscar_para_unirse`), `sincronizar/permiso_red.rs` y `sincronizar/automatica.rs`
(`Reloj::desperto`).

## 1. La causa

Unirse con un código («Unirme con un código», en el PC y en Android) **solo buscaba por mDNS**: el
anuncio `_pindesk._tcp` con la etiqueta del grupo en el TXT `g`. Si mDNS no traía al otro en 25 s,
salía «No apareció ningún aparato con ese código… También puedes escribir su dirección». Con la
dirección no se busca nada (se llama directamente), y por eso «con la IP sí va».

Lo que cambió con PinDesk (1-nov-2026) y lo dejó sin margen:

1. **mDNS por partida doble.** Desde el cambio de nombre (`94331d6`, Android `041a448`) cada
   aparato anuncia, busca y resuelve **dos tipos** (`_pindesk._tcp` y `_pixpin._tcp`), y lo mismo
   para envío, recepción y chats compartidos. Son el doble de registros, consultas y resoluciones
   en las dos pilas mDNS (la de Windows, `DnsService*`, y la de Android, `NsdManager`, que resuelve
   **de una en una**: `Red.buscar` encola cada servicio visto, ahora el doble, propios incluidos, y
   una resolución que Android no contesta deja la cola parada). El protocolo en sí no cambió: las
   magias, la sal, la etiqueta `g`, la baliza `PXB1` y los TXT son los mismos en el PC y en Android
   (comprobado byte a byte; ver las pruebas de `codigo.rs`, `baliza.rs` y `emparejar.rs`).
2. **El ejecutable estrena ruta, y el cortafuegos no lo conoce.** La regla de entrada que Windows
   creó al pulsar «Permitir» era para `Programs\PixPinMax\pixpinmax.exe`. `pindesk.exe` vive en
   `Programs\PinDesk`: Windows vuelve a preguntar la primera vez que escucha (al arrancar, detrás de
   la consola de `Instalar.cmd` o al iniciar sesión). **Si ese aviso se cierra sin «Permitir»,
   Windows crea una regla de BLOQUEO para el programa**, que gana a cualquier permiso, y no vuelve a
   preguntar. `Instalar.cmd` no puede poner reglas (no es administrador) y nunca las puso. Sin
   permiso, el móvil no llega al PC (TCP 47474) y el PC no oye las balizas (UDP 47475).
3. **El PC se rendía al primer fallo.** `buscar_del_grupo` hacía
   `mdns::buscar(...).ok()?`: si la primera búsqueda mDNS devolvía error (Windows ocupado, la red
   aún sin asentar), se acababan los 25 s al momento. Reproducido en
   `emparejar::pruebas::un_fallo_de_una_busqueda_no_acaba_la_busqueda`.

Lo que **no** se pudo comprobar aquí (no hay Windows ni móvil en este equipo de desarrollo): cuál de
las tres pesó en el caso del usuario. Si vuelve a pasar, el registro (`pindesk.log`) dice ahora por
dónde se encontró al otro (`unirse: encontrado … por=baliza|mDNS`), si mDNS falló
(`unirse: mDNS fallo`) y qué dice el cortafuegos (`cortafuegos: permiso de red de PinDesk`).

## 2. Lo que hace ahora el PC

### Unirse con el código: dos caminos a la vez

`buscar_para_unirse` (sincronizar.rs) busca 25 s por **mDNS** y por **la baliza** a la vez; gana
el primero.

- **La baliza** (`emparejar::pregunta`): con la clave que sale del código tecleado (la misma
  PBKDF2 de siempre), el PC manda cada segundo una baliza `PXB1` con la bandera `PREGUNTA` a la
  difusión general, a la de cada red y a la multidifusión `239.255.74.74`, puerto 47475; a los 4 s,
  una vez, a cada dirección de cada red (Wi-Fi que filtra la difusión). **Los aparatos del grupo ya
  contestan** a cualquier pregunta firmada con la clave del grupo aunque quien pregunta no esté en
  su lista (el PC desde el 28-oct-2026 y Android igual: `BalizaDelMovil.oir`, que contesta antes de
  mirar si es miembro): mandan su baliza, directa, con su puerto TCP. Con eso se llama por TCP y
  sigue el saludo de siempre. **No hace falta cambiar nada en Android** para que el PC encuentre al
  móvil así; basta con que el móvil tenga PinDesk a la vista (su baliza vive mientras).
- **Sale del puerto de la baliza** (`Oyente::mandar`): la respuesta del grupo va siempre a
  `IP:47475`, y el cortafuegos de Windows deja entrar la respuesta directa a una difusión **si va
  al puerto del que salió** (unos 3 s), aunque PinDesk no tenga regla.
- **mDNS** igual que antes, pero sin rendirse (`emparejar::buscar_mientras`): un error espera 1 s
  y se vuelve a buscar.
- Si nadie aparece, el aviso de siempre y, si el cortafuegos bloquea, una línea más: «El
  cortafuegos de Windows no deja entrar a PinDesk: usa «Dar permiso de red» en Sincronizar».

Probado con sockets de verdad en `baliza_de_verdad::quien_se_une_con_el_codigo_encuentra_al_grupo_
por_la_baliza_sin_mdns` (del código a la conexión TCP en milisegundos) y su caso negativo.

### El permiso de red

- Al abrir Sincronizar se mira, en un hilo, si el cortafuegos deja entrar a `pindesk.exe` en el
  puerto TCP de la presencia y en UDP 47475 (`INetFwMgr::IsPortAllowed`, con el perfil de red de
  ahora; cuenta reglas de programa y de puerto, bloqueos y si está apagado).
- Si bloquea, arriba de todo: **«PinDesk no tiene permiso de red»**, qué pasa («tus otros aparatos
  no aparecen al unirse con el código») y **«Dar permiso de red»**, que avisa de que Windows pedirá
  confirmar como administrador.
- El botón lanza PowerShell como administrador (`runas`, con `-EncodedCommand` para que la ruta
  llegue entera): quita las reglas de **bloqueo** de entrada de ese programa, quita las de
  «PinDesk - Sincronizar» y «PixPin Max - Sincronizar» de antes y pone TCP (cualquier puerto del
  programa) y UDP 47475, en redes **privadas y de dominio**. Lo mismo hace
  `herramientas\cortafuegos-sincro.ps1`.
- Resultado: «Listo…», «No se dio el permiso…» (UAC cancelado), «Windows no dejó…» (directiva) o,
  si tras ponerlo sigue bloqueado, «esta Wi‑Fi está marcada como Pública: márcala como privada».
  La red pública **no** se abre a propósito.
- La escucha TCP y la baliza se abren al arrancar la app (`presencia::instalar`), así que el aviso
  de Windows sale cuanto antes; los dos están atados a `0.0.0.0`, que es lo que lo provoca.

### El QR de emparejar

Al tocar el código del grupo (Sincronizar → «Añadir otro aparato»), debajo sale su QR. Lleva
`pindesk://emparejar?…` (§3). Se oculta solo a los **10 minutos** (`VIGENCIA_QR_MS`), al volver a
tocar el código, al salir del grupo o al cerrar la ventana: el código es la llave del grupo y no se
deja a la vista más de lo que dura emparejar. El enlace lleva además su caducidad (`e`).

## 3. El enlace `pindesk://emparejar`

```
pindesk://emparejar?v=1&c=K7Q2M9XMPA&h=192.168.1.20,10.0.0.5&p=47474&n=PinDesk%20Casa&e=1793180000000
```

| Campo | Qué | Obligatorio |
|---|---|---|
| `v` | Versión del formato (1). Mayor que la que se entiende: no se lee | no (1) |
| `c` | El código del grupo, limpio (10 signos de `23456789ABCDEFGHJKMNPQRSTUVWXYZ`). Se acepta con guion y en minúsculas | **sí** |
| `h` | Las IPv4 de quien lo enseña en la red local, la de la ruta preferida primero; como mucho 4, separadas por comas. Solo privadas (10/8, 172.16/12, 192.168/16) y 100.64/10 | no |
| `p` | Su puerto TCP de sincronizar | no (47474) |
| `n` | Su nombre, cifrado como URL (RFC 3986; `+` se lee como espacio) | no |
| `e` | Hasta cuándo vale, ms desde 1970. Se rechaza pasado `e` + 5 min (relojes) | no |

- Se lee también `pixpin://emparejar?…`, y el esquema y el anfitrión sin distinguir mayúsculas.
  Campos desconocidos se ignoran (para crecer sin romper).
- Lo implementan `pindesk_sincro::emparejar::Enlace::{texto, leer, vigente}` con sus pruebas
  (forma exacta, ida y vuelta, casos que no valen).
- **Por qué un esquema propio y no `https://`**: Android aún no tiene un dominio con App Links
  verificados, y el de la nube (`docs/nube.md`) lo pone cada usuario. La cámara de Google, Google
  Lens y la de Samsung ofrecen «Abrir» para un enlace con esquema que una app instalada declara. Si
  algún día hay dominio propio, la misma consulta irá en `https://<dominio>/emparejar#…` (en el
  fragmento, para que el código no llegue al servidor) con una página que salte a
  `intent://emparejar?…#Intent;scheme=pindesk;package=com.forge.pixpin;S.browser_fallback_url=…;end`.

**Seguridad.** El código **es** la llave del grupo: quien lo tenga puede unirse y sincronizar.
Por eso el QR solo se enseña a petición, en la pantalla de emparejar y diez minutos. El enlace no
abre nada por sí solo: quien lo lee pregunta antes («¿Unirte al grupo de PinDesk Casa?»), llama
con el saludo cifrado de siempre (la clave sale del código; un `h` falso solo hace que no
conteste nadie) y no se sincroniza nada hasta estar dentro.

## 4. En segundo plano

### En el PC (ya estaba; lo nuevo, marcado)

Con PinDesk abierto (vive en la bandeja), la sincronización automática (`automatica.rs`, apagable
con `[sincro] automatica = false`) sincroniza **sin bloquear nunca la interfaz** (cada vuelta en su
hilo, una a la vez) con los aparatos del grupo **que contestan**:

| Cuándo | Qué |
|---|---|
| Al **aparecer** uno: contesta a la sonda TCP (cada 4 s con Sincronizar delante, cada 60 s sin) o se oye su baliza (cada 15 s) | Vuelta entera, ya |
| Al **cambiar algo** aquí | Tras 30 s sin más cambios |
| **Tareas** tachadas o editadas | Solo esos chats, tras 0,6 s de calma (1,5 s como mucho) |
| **Periódica** | Cada 10 min mientras haya alguien |
| Tras un **fallo** | 5 s, 10, 20… hasta 5 min (±25 %); «ocupado», 1–4 s |
| Al **volver la red** (`NotifyAddrChange`) | Sondear ya; lo pendiente, ya |
| **Nuevo: al volver de la suspensión** (`PowerRegisterSuspendResumeNotification`, y si no avisa, un salto del reloj de pared mientras el hilo esperaba) | Sondear ya, y quien conteste cuenta como **recién aparecido**: vuelta entera enseguida. Antes seguía «visto» de antes de cerrar la tapa y esperaba al periodo |
| Mientras **alguien graba** | Nada salvo la cámara; al acabar, el chat de la grabación primero |

### Lo que tiene que hacer Android (especificación)

Android hoy solo sincroniza con la app a la vista (`Presencia`, `SincronizacionAutomatica`) o
cuando llama el PC. Para «siempre que se pueda»:

1. **Con la app a la vista** (como ahora): presencia, baliza, sonda cada 4 s y la misma tabla del
   PC (aparecer → ya; cambio → 30 s quieto; tareas → al instante; 10 min; reintentos).
2. **WorkManager periódico**: un `PeriodicWorkRequest` de 15 min (el mínimo de Android) con
   `NetworkType.UNMETERED` (o `CONNECTED` si el usuario lo elige) y `setRequiresBatteryNotLow(true)`.
   En cada ejecución: si hay grupo, sondear (TCP `PING`) las últimas direcciones de los miembros y
   preguntar por la baliza (5 s como mucho); con quien conteste, una vuelta **callada** con lo
   elegido la última vez (como `en_fondo::lanzar_callada` del PC). `ExistingPeriodicWorkPolicy.KEEP`
   con un nombre único, encolado al arrancar la app y tras `BOOT_COMPLETED`.
3. **Al tener red**: `ConnectivityManager.registerNetworkCallback` (Wi‑Fi) desde el `Application`;
   en `onAvailable`, un `OneTimeWorkRequest` **expedited** (`setExpedited`, con
   `OutOfQuotaPolicy.RUN_AS_NON_EXPEDITED_WORK_REQUEST`) con la misma lógica, con 10 s de calma
   para juntar avisos seguidos.
4. **Primer plano**: una vuelta larga (archivos de una grabación, muchos MB) va en un
   `ForegroundService` de tipo `dataSync` con su notificación de progreso, no en el worker (que
   Android corta a los 10 min).
5. **Prioridades** como el PC: mientras alguien graba, solo la cámara; al acabar, el chat de la
   grabación; las tareas antes que la vuelta entera.
6. **Sin ruido**: sin notificaciones si sale bien y no trae nada; una si trae algo o tras tres
   fallos seguidos (como `FALLOS_PARA_AVISAR`).
7. Un ajuste para apagarlo («Sincronizar en segundo plano», encendido por omisión), y respetar el
   ahorro de batería.

### Lo que tiene que hacer Android para el QR y unirse sin mDNS

- [ ] **Intent-filter** en una actividad exportada (por ejemplo `SincronizarActivity` o una
      `EmparejarActivity` translúcida con `taskAffinity` propia, como `AtajoActivity`):
      `ACTION_VIEW`, categorías `DEFAULT` y **`BROWSABLE`** (sin ella las cámaras y los navegadores
      no la abren), `<data android:scheme="pindesk" android:host="emparejar" />` y la misma con
      `pixpin`.
- [ ] **Leer el enlace** como `Enlace::leer` (§3, con sus casos de prueba). Si no vale o ha
      caducado (`e` + 5 min): «Este QR ya no vale: vuelve a enseñarlo en el PC».
- [ ] **Preguntar** «¿Unirte al grupo de `<n>`?» (y, si este aparato ya está en otro grupo, decir
      que saldrá de él, como al teclear el código).
- [ ] **Llamar sin buscar**: cada `h` en orden con `Red.sondear(h, p)` (1,2 s cada una); la primera
      que conteste → `unirseCon(c, "h:p")`. Si ninguna contesta, `unirseCon(c, null)` (buscar).
- [ ] **Unirse por la baliza también** (lo que hace ahora el PC): en `unirseCon` sin dirección,
      además de `red.buscar`, mandar cada segundo una baliza `PXB1` con `PREGUNTA`, firmada con
      `Grupo.clave(codigo)`, **desde el socket que oye en 47475** (`SocketsDeLaBaliza`), a la
      difusión, a la de la red y a `239.255.74.74`; a los 4 s, una vez, a cada IP de la red. La
      primera baliza válida de ese grupo con `id` distinto del propio da `IP de origen:puerto`.
      El que oye ya contesta bien a los que preguntan; no hay que tocarlo.
- [ ] El **lector de QR de la app** (`Qr.kt`) que acepte también `pindesk://emparejar` (además de
      `pixpin-envio/recibe/chat`).
- [ ] **No buscar dos veces lo mismo en mDNS** mientras dure la transición: `Red.buscar` encola
      para resolver cada servicio de los dos tipos; resolver solo los de `_pixpin._tcp` cuyo nombre
      no se haya visto ya con `_pindesk._tcp`, y poner un tope (5 s) a cada resolución para que una
      que Android no contesta no pare la cola.
