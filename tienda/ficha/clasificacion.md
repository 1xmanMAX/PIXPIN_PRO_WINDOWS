# Clasificación por edades (cuestionario IARC) de PinDesk

Partner Center → la entrega → **Age ratings**. Es el cuestionario de IARC
(International Age Rating Coalition): sus respuestas dan a la vez la
clasificación de ESRB, PEGI, USK, ClassInd, ACB y las demás, y el
certificado llega por correo a la dirección de la cuenta. Las preguntas
pueden cambiar de redacción; aquí va el sentido de cada una y la respuesta
recomendada **para lo que PinDesk hace de verdad** (comprobado en el código).

> Regla de IARC: se responde por **lo que la app contiene o permite**, no por
> lo que pensamos que hará el usuario. Ante la duda, la respuesta más
> conservadora (el «Sí» con su nota) es la que no da problemas en
> certificación. Los avisos de interacción («Users Interact», «Unrestricted
> Internet», «Shares Info») **no suben la edad**: solo se muestran en la
> ficha.

## 0. Datos de contacto

- **Email address**: el de la cuenta de Partner Center (allí llega el
  certificado de IARC). El de soporte de la app es
  `max.antony.8.5.1@gmail.com`.

## 1. Categoría

**What category best describes your product?**
→ **Utility, Productivity, Communication, or Other** (Utilidades,
productividad, comunicación u otros).

No es un juego, ni una red social, ni un navegador, ni una tienda. La
mensajería con otras personas (chats compartidos) es secundaria y solo por la
red local, así que no es «Social networking».

## 2. Contenido de la propia app

Para la categoría de utilidades IARC pregunta, sobre todo, si la app
**contiene** (no si el usuario puede crear) estas cosas:

| Pregunta (sentido) | Respuesta | Nota |
|---|---|---|
| Violencia, sangre, miedo | **No** | No hay contenido así en la app. |
| Contenido sexual, desnudez | **No** | |
| Lenguaje soez, humor grosero | **No** | |
| Drogas, alcohol, tabaco | **No** | |
| Juegos de azar, simulación de apuestas | **No** | Hay una herramienta «ruleta» para elegir al azar (sorteos, turnos): no hay apuestas, ni dinero, ni premios. |
| Discriminación, odio | **No** | |
| Miedo / horror | **No** | |

## 3. Elementos interactivos

| Pregunta (sentido) | Respuesta | Nota honesta |
|---|---|---|
| **¿Los usuarios pueden interactuar o intercambiar contenido con otros usuarios** (texto, imágenes, audio, voz)? | **Sí** | Los **chats compartidos** dejan mandar mensajes, fotos y audios a otras personas, pero solo entre aparatos emparejados a mano (código o QR) en la **red local**, cifrado, sin servidor nuestro y sin buscar desconocidos. También «compartir por enlace» sube un archivo que el usuario elige a un servicio público temporal. Si IARC pregunta además si el contenido es **público** o con **desconocidos**: **No**. Si pregunta si hay **moderación**: no hay servidor que moderar; se emparejan solo personas que el usuario elige. |
| **¿La app comparte la ubicación física del usuario con otros usuarios?** | **No** | PinDesk no lee la ubicación. |
| **¿La app permite comprar bienes digitales?** | **No** | Gratis, sin compras dentro de la app ni anuncios. Las claves de API (Anthropic, DeepSeek, TypeSafe) las paga el usuario fuera de la app, si quiere; la app no vende nada. |
| **¿La app da acceso sin restricciones a internet** (navegador o buscador)? | **Sí** | El **asistente** puede buscar en la web (DuckDuckGo/Brave, o la búsqueda de Claude) y leer páginas para contestar, y los enlaces se abren en el navegador. No es un navegador completo, pero lo que trae de la web no se filtra, así que lo honesto es «Sí». |
| **¿La app comparte información personal que da el usuario con terceros?** | **Sí** (recomendado) | Solo si el usuario la activa con su clave: lo que escribe al asistente va a Anthropic o DeepSeek; frases de una reunión a TypeSafe (Jev); archivos a Google Drive si lo conecta; texto a las voces de Edge. Todo está en `docs/privacidad.md`. Si se prefiere «No» (porque es el propio usuario quien lo manda a su propia cuenta), hay que poder defenderlo; «Sí» no cambia la edad y evita discusiones. |
| **¿La app muestra contenido generado por IA** / tiene IA generativa? (si sale la pregunta) | **Sí** | El asistente usa modelos de terceros (Claude, DeepSeek) **solo con la clave del usuario**. Cada respuesta tiene «Reportar respuesta…» (menú del mensaje › Más), que guarda el reporte y abre el correo hacia `max.antony.8.5.1@gmail.com`. Los proveedores aplican sus propios filtros de seguridad. |
| ¿Es principalmente un navegador web o un buscador? | **No** | |
| ¿Recoge datos de menores / está dirigida a niños? | **No** | La política dice que no está dirigida a menores de 13 años. |

## 4. Resultado esperado

Con estas respuestas lo normal es la clasificación más baja en todas las
regiones (**IARC 3+, PEGI 3, ESRB Everyone, USK 0**), con los avisos
**«Users Interact»**, **«Unrestricted Internet»** y **«Shares Info»**
(y, si existe la casilla, el de IA generativa). Si sale algo más alto, revisar
la respuesta de la sección 2: algo se marcó como «Sí» por error.

## 5. Dónde depende de lo que quiera el usuario

- **Shares Info**: «Sí» es lo conservador. «No» solo si se quiere sostener que
  el usuario envía sus datos a su propia cuenta de un tercero, no PinDesk.
- **Users Interact**: si un día se quitan los chats compartidos y compartir
  por enlace en la rama de la Store, pasaría a «No».
- **Unrestricted Internet**: si se apagara la búsqueda web del asistente en
  la rama de la Store, podría pasar a «No».
- Si en el futuro se publicara también en la Xbox o se añadieran compras,
  hay que repetir el cuestionario.
