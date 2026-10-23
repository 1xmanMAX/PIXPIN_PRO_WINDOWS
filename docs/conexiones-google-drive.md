# Conectar Google Drive y Google Calendar

> **Google Calendar** (18-oct-2026) va con el **mismo ID de cliente**: en el paso 2 habilita también la *Google Calendar API* y, en Ajustes, pulsa **Conectar** en la fila **Google Calendar**. Ver [Google Calendar](#google-calendar) abajo.

Con Google Drive conectado, el asistente de PixPin Max (con Claude o DeepSeek) puede, siempre por encargo tuyo:

- **buscar** archivos en tu Drive, por el nombre o por lo que dicen dentro;
- **leer** el texto de un Documento, una Presentación o una Hoja de cálculo de Google, o de un PDF, Word o texto que tengas en Drive (para resumirlo o contestarte sobre él sin bajarlo);
- **bajar** un archivo al chat (un Doc de Google llega como Word, una Hoja como Excel);
- **subir** un archivo del chat a tu Drive. Antes de subir nada **te pregunta** («¿Subo «Memoria.pdf» (2,1 MB) a tu Google Drive?»), y sin tu «Sí» no sale ni un byte.

PixPin no puede borrar ni cambiar lo que ya tienes en Drive: solo pide leer (`drive.readonly`) y tocar lo que él mismo sube (`drive.file`).

Google no deja que una app de escritorio se conecte a Drive con un permiso compartido entre todos sus usuarios. Por eso, **una sola vez**, creas tu propio «ID de cliente» en Google Cloud (es gratis y tarda unos 10 minutos) y lo pegas en PixPin.

---

## 1. Crear el proyecto en Google Cloud

1. Entra en <https://console.cloud.google.com/> con la cuenta de Google cuyo Drive quieres conectar.
2. Arriba a la izquierda, en el selector de proyectos, pulsa **Seleccionar un proyecto → Proyecto nuevo**.
3. Nombre del proyecto: `PixPin Max` (el que quieras). Organización: déjalo como esté («Sin organización» en una cuenta personal).
4. Pulsa **Crear** y, cuando termine, comprueba que arriba aparece seleccionado **PixPin Max**.

## 2. Habilitar la API de Google Drive

1. Menú ☰ → **APIs y servicios → Biblioteca**.
2. Busca `Google Drive API` y ábrela.
3. Pulsa **Habilitar**.
4. Para **Google Calendar**: busca también `Google Calendar API` y pulsa **Habilitar**.

## 3. Pantalla de consentimiento (Google Auth Platform)

1. Menú ☰ → **APIs y servicios → Pantalla de consentimiento de OAuth** (en la consola nueva se llama **Google Auth Platform**). Si te sale **Comenzar**, púlsalo.
2. **Información de la app**: nombre de la app `PixPin Max`; correo de asistencia: el tuyo. **Siguiente**.
3. **Público**: elige **Externo**. **Siguiente**.
4. **Información de contacto**: tu correo. **Siguiente**.
5. Acepta la política de datos de usuario de las API de Google y pulsa **Crear**.
6. Ve a **Público** (en el menú de la izquierda de Google Auth Platform) → **Usuarios de prueba → Agregar usuarios** y añade **tu propio correo de Gmail**. Guarda.
7. (Opcional pero recomendable) **Acceso a los datos → Agregar o quitar permisos** y marca estos dos (puedes pegarlos en «Agregar permisos manualmente»):
   - `https://www.googleapis.com/auth/drive.readonly`
   - `https://www.googleapis.com/auth/drive.file`
   - `https://www.googleapis.com/auth/calendar.readonly` (para Google Calendar)

   Pulsa **Actualizar** y luego **Guardar**. PixPin los pide de todas formas al conectar; aquí solo quedan apuntados.

> **Prueba o producción.** Mientras la app está «En prueba», Google hace caducar el permiso **a los 7 días** y tendrías que pulsar *Conectar* otra vez cada semana. Para que no caduque, en **Público** pulsa **Publicar app → Confirmar** («En producción»). No hace falta que Google la verifique para usarla tú: al conectar verás el aviso «Google no ha verificado esta app»; pulsa **Configuración avanzada → Ir a PixPin Max (no seguro)**. Es tu propia app, con tu propio ID.

## 4. Crear el ID de cliente «App de escritorio»

1. En Google Auth Platform → **Clientes → Crear cliente** (o **APIs y servicios → Credenciales → Crear credenciales → ID de cliente de OAuth**).
2. **Tipo de aplicación: App de escritorio**. ⚠️ Tiene que ser *App de escritorio*: con «Aplicación web» Google rechaza la vuelta a PixPin (`redirect_uri_mismatch`).
3. Nombre: `PixPin Max PC`. Pulsa **Crear**.
4. Google enseña el **ID de cliente** (`1234567890-abc…apps.googleusercontent.com`) y el **Secreto del cliente** (`GOCSPX-…`). Pulsa **Descargar JSON** o copia los dos.

No hace falta escribir ninguna «URI de redireccionamiento»: una app de escritorio vuelve a `http://127.0.0.1:<puerto>` y Google lo acepta solo.

## 5. Pegarlo en PixPin

1. PixPin Max → **Ajustes → Asistente**, baja hasta **Conexiones del asistente**.
2. En **ID de cliente de Google Drive** pega **una** de estas tres cosas y pulsa **Guardar** (o Intro):
   - el ID y el secreto separados por un espacio: `1234567890-abc.apps.googleusercontent.com GOCSPX-xxxx`;
   - o el contenido entero del JSON descargado (ábrelo con el Bloc de notas, Ctrl+A, Ctrl+C);
   - o solo el ID (si Google pidiera el secreto al conectar, te lo dirá: vuelve a pegarlo con el secreto).
3. Aparece la fila **Google Drive** con el botón **Conectar**. Púlsalo.
4. Se abre tu navegador en la página de Google: elige tu cuenta, (si sale) pasa el aviso de app no verificada como se explica arriba y pulsa **Continuar / Permitir**.
5. El navegador dice **«Google Drive conectado. Ya puedes cerrar esta pestaña y volver a PixPin»**, y en Ajustes la fila pasa a **✓ Conectado · tu-correo@gmail.com**.

Tienes 5 minutos para terminar en el navegador; si no, vuelve a pulsar **Conectar**.

## 6. Usarlo

Con Claude o DeepSeek como motor del asistente (la IA local no usa las conexiones), en el chat **Asistente**:

- «Busca en mi Drive el contrato de la obra Tahuantinsuyo.»
- «Lee el documento *Informe mensual* de mi Drive y resúmelo en 5 puntos.»
- «¿Cuánto suma la columna Costo de la hoja *Presupuesto 2026* que tengo en Drive?»
- «Baja *Plano de señalización.pdf* de Drive al chat y resalta JR. ANTISUYO.»
- «Sube este PDF a mi Drive, a la carpeta *Obra*.» → te pregunta antes.

## 7. Desconectar

Ajustes → Asistente → Conexiones → **Desconectar**. Los tokens se borran de este PC al momento y PixPin le pide a Google que retire el permiso. El ID de cliente se queda guardado por si vuelves a conectar. También puedes retirar el permiso desde tu cuenta: <https://myaccount.google.com/permissions>.

## Google Calendar

Solo para **ver**: PixPin pide `calendar.readonly` y no puede crear, cambiar ni borrar nada de tu calendario.

1. Con el ID de cliente ya pegado (el de Drive vale: se copia solo), en **Ajustes → Asistente → Conexiones** aparece la fila **Google Calendar** con **Conectar**. Púlsalo y da permiso en el navegador, como con Drive.
2. PixPin lee tus calendarios visibles (los que tienes marcados en Google) cada **15 minutos**, de ayer a tres semanas, y lo guarda en el PC: todo sale al instante y sin internet.
3. **En el horario semanal** (bandeja → «Horario de clases…») los eventos salen en la rejilla, translúcidos y con la raya del color de su calendario; los de todo el día, como una tira arriba. Pulsar uno lo abre en Google Calendar.
4. **Avisos**: antes de cada evento (con la misma antelación que las clases) sale la **llamada** en pantalla, con su hora, su lugar y su calendario.
5. **El asistente**: «¿qué tengo hoy?», «¿tengo algo el jueves?», «mi agenda de esta semana».

## Seguridad

- El ID de cliente, su secreto y los tokens se guardan **cifrados con DPAPI** de Windows (solo tu cuenta de Windows en este PC puede descifrarlos) en `%LOCALAPPDATA%\PixPinMax\claves\conexion-google_drive.cifrado`. Nunca van al fichero de ajustes, ni a la carpeta que se sincroniza, ni a los registros.
- La vuelta del navegador llega a `127.0.0.1` (solo este PC) y se comprueba con PKCE (S256) y un `state` aleatorio: otra app no puede colarse a medias.
- Lo que el asistente lee de tu Drive se le manda al modelo de la nube que hayas elegido (Claude o DeepSeek), igual que un archivo que adjuntas al chat.

## Si algo falla

| Lo que dice | Qué pasa y qué hacer |
|---|---|
| «eso no parece un ID de cliente» | Pegaste otra cosa. Copia el ID (termina en `.apps.googleusercontent.com`) o el JSON entero. |
| Google: «Error 403: access_denied» / «no ha completado el proceso de verificación» | Tu correo no está en **Usuarios de prueba** (paso 3.6), o publica la app (aviso del paso 3). |
| Google: «Error 400: redirect_uri_mismatch» | El cliente no es **App de escritorio**. Crea otro de ese tipo (paso 4) y pégalo. |
| «el ID de cliente no vale (invalid_client)» | ID o secreto mal copiados, o el cliente se borró. Vuelve a pegarlos. |
| «el permiso caducó o se retiró; hay que volver a conectar» | Pasaron 7 días con la app «En prueba», cambiaste la contraseña o retiraste el permiso. Pulsa **Conectar**. |
| «Google Drive no está conectado» (en el chat) | Falta el paso 5. |

---

## Para quien programa: añadir otro servicio (Dropbox, OneDrive…)

Todo está en `crates/pixpin-ia/src/conexiones/` y es un solo componente con variables:

1. Una constante `Servicio` (como `drive::GOOGLE_DRIVE`) con sus direcciones de OAuth y de la API, alcances y parámetros extra, añadida a `SERVICIOS`.
2. Un `impl Almacen` (buscar, archivo, leer, bajar, subir, cuenta) y su línea en `Conexion::almacen`.
3. Su id en el `enum` de `servicio` de `conexiones/acciones.rs` (las cuatro herramientas son las mismas) y, si hace falta, el texto de sus descripciones.

OAuth (PKCE + vuelta a 127.0.0.1), el guardado cifrado, la renovación del token, las herramientas y la fila de Ajustes salen solos. Las pruebas (`conexiones/pruebas.rs`) levantan un servidor falso en 127.0.0.1 que imita a Google; un servicio nuevo copia ese patrón con sus respuestas.
