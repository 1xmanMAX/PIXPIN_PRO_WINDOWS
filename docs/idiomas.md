# Los idiomas de PixPin Max

PixPin Max habla catorce idiomas (1-nov-2026). Español e inglés son los de siempre y están
completos; los otros doce pueden estar enteros, a medias o sin empezar: **lo que le falta a un
idioma se ve en inglés** (y si tampoco está en inglés, en español). Nunca se ve una clave.

| Idioma | Carpeta (`i18n/`) | En los ajustes (`idioma = …`) | Nombre en el selector |
|---|---|---|---|
| Español | `es-ES` | `es` | Español |
| Inglés | `en-US` | `en` | English |
| Chino simplificado | `zh-CN` | `zh-CN` | 中文 (简体) |
| Hindi | `hi-IN` | `hi` | हिन्दी |
| Árabe | `ar-SA` | `ar` | العربية |
| Portugués de Brasil | `pt-BR` | `pt-BR` | Português (Brasil) |
| Francés | `fr-FR` | `fr` | Français |
| Alemán | `de-DE` | `de` | Deutsch |
| Japonés | `ja-JP` | `ja` | 日本語 |
| Ruso | `ru-RU` | `ru` | Русский |
| Coreano | `ko-KR` | `ko` | 한국어 |
| Italiano | `it-IT` | `it` | Italiano |
| Indonesio | `id-ID` | `id` | Bahasa Indonesia |
| Turco | `tr-TR` | `tr` | Türkçe |

En `ajustes.toml` también vale la etiqueta entera (`fr-FR`, `es-ES`); los ficheros de antes
(`es`, `en`, `sistema`) se leen igual.

## Cómo funciona

- **Los catálogos** son ficheros [Fluent](https://projectfluent.org/) en
  `crates/pixpin-store/i18n/<etiqueta>/main.ftl`. `crates/pixpin-store/build.rs` mete en el
  binario los que existan: para añadir o actualizar una traducción **basta con dejar el fichero y
  compilar**; no hay que tocar código. Español e inglés son obligatorios.
- **La cadena de red** (`Catalogo`, `src/idioma.rs`): cada texto se busca en el idioma elegido,
  después en inglés y después en español. Cada idioma es su propio `FluentBundle` con su locale,
  así que los plurales siguen las reglas de CLDR del idioma (abajo). Un texto traducido que falla
  al formarse (una `{ $variable }` mal copiada) también cede al inglés.
- **El idioma del sistema** (`resolver_idioma`): con «El de Windows» se mira la parte de idioma del
  locale del usuario (`fr-CA` → francés). Chino: `zh-Hans`, `zh-CN`, `zh-SG` → simplificado; el
  tradicional (`zh-TW`, `zh-HK`, `zh-Hant`) cae también al simplificado mientras no tenga catálogo
  propio. `pt-PT` → portugués de Brasil. Un idioma sin catálogo → inglés.
- **Cambiar de idioma** en Ajustes → General vale desde el próximo arranque, como siempre. Como hay
  catorce, el selector es «−» / «+» con el nombre de cada idioma en su propia lengua, para que
  quien no entiende el idioma actual encuentre el suyo.

## Cómo traduce un traductor

1. Copia `crates/pixpin-store/i18n/en-US/main.ftl` a `crates/pixpin-store/i18n/<etiqueta>/main.ftl`
   (o parte de él: se puede ir por trozos) y traduce **solo lo que va detrás del `=`**.
2. Comprueba:

   ```sh
   cargo test -p pixpin-store          # se entiende, no inventa claves, mismas variables
   python3 herramientas/idiomas.py     # cuánto tiene cada idioma, qué sobra o no cuadra
   ```

3. Para seguir otro día, lo que falta, en inglés y listo para traducir:

   ```sh
   python3 herramientas/idiomas.py faltan fr-FR          # las claves
   python3 herramientas/idiomas.py faltan fr-FR --ftl    # el .ftl en inglés de lo que falta
   ```

4. Cuando cambie el inglés (claves nuevas), `faltan` dice qué traducir; una clave que ya no está en
   inglés sale como «sobra» y hay que quitarla.

### Las reglas

- **Las claves no se traducen** (`captura-guardada = …`: solo lo de la derecha).
- **Las marcas se quedan como están**: PixPin Max, Jev, TypeSafe, Anki, Excalidraw, Whisper, Vosk,
  Claude, DeepSeek, Google Drive, WeChat… y los nombres de teclas y atajos (`Ctrl+Alt+X`).
- **Las variables, iguales**: `{ $atajo }`, `{ $n }`, `{ $ruta }` van tal cual, con el mismo nombre,
  donde la frase las pida. Cada mensaje tiene que usar **las mismas** variables que en inglés (lo
  comprueba el test). Tampoco se traducen las referencias a otros mensajes o términos
  (`{ app-nombre }`, `{ -marca }`).
- **Los plurales con las categorías del idioma** (CLDR). En inglés hay `[one]` y `*[other]`; cada
  idioma pone las suyas, y la variante por defecto (`*`) es siempre `other`:

  | Idioma | Categorías |
  |---|---|
  | Árabe | `zero`, `one`, `two`, `few`, `many`, `other` |
  | Ruso | `one` (1, 21, 31…), `few` (2–4, 22–24…), `many` (0, 5–20, 25…), `other` (decimales) |
  | Francés, portugués | `one` (0 y 1 en francés; 0 y 1 en pt-BR), `many` (millones), `other` |
  | Español, italiano | `one`, `many` (millones), `other` |
  | Alemán, inglés, hindi, turco | `one`, `other` |
  | Chino, japonés, coreano, indonesio | solo `other`: basta `{ $n } 个` sin selector |

  Ejemplo en ruso:

  ```ftl
  pila-titulo-varias = { $cuantas ->
      [one] { $cuantas } снимок
      [few] { $cuantas } снимка
     *[many] { $cuantas } снимков
  }
  ```

  Un idioma sin plurales puede quitar el selector, pero la variable se queda
  (`pila-titulo-varias = { $cuantas } 张截图`).
- **Emojis, flechas y signos** (`🌐`, `→`, `…`, `«»`) se pueden cambiar por los propios del idioma
  (`「」`, `“”`), pero no quitar lo que dice algo.
- **Nada de marcas de dirección** (U+200E, U+202B…): la dirección del texto la pone el pintor.

## Apple

`apple/nucleo/pixpin-store` es una copia con un subconjunto de claves (las que usa la app de Apple)
y algunas propias. Sus catálogos de los doce idiomas **no se editan a mano**: salen de los de
`crates/` con

```sh
python3 herramientas/idiomas.py apple              # escribe los que cambien
python3 herramientas/idiomas.py apple --comprobar  # para CI: falla si no están al día
cd apple && cargo test -p pixpin-store
```

Se copian las claves de Apple que estén traducidas en `crates/` con las mismas variables que el
inglés de Apple; las que solo tiene Apple (hoy 42, del asistente local y de los ajustes del
sistema) se quedan en su español e inglés y en los demás idiomas salen en inglés.

## Escritura, letras y dirección

- **Letras** (`crates/pixpin-render/src/idioma.rs`): el ejecutable fija al arrancar el locale del
  idioma (`pixpin_render::idioma::fijar`) y todos los formatos de texto de la interfaz se crean con
  él. Con el locale, la reserva de letras del sistema de DirectWrite (automática en cada
  `IDWriteTextLayout` desde Windows 8.1) elige la letra de lo que Segoe UI no tiene: Microsoft
  YaHei (chino), Yu Gothic (japonés), Malgun Gothic (coreano), Nirmala UI (devanagari). Los
  ideogramas Han se dibujan con la forma del idioma (chino o japonés), no con la de otro.
- **Árabe**: un rótulo cuya primera letra es árabe, o cualquier rótulo con algo en árabe si la
  interfaz es árabe, se dispone de derecha a izquierda (`DWRITE_READING_DIRECTION_RIGHT_TO_LEFT`),
  con sus renglones alineados a la derecha. La caja se ciñe al texto, que sigue empezando donde lo
  pone quien pinta.
- **Partir renglones**: lo que dispone DirectWrite ya parte bien el chino y el japonés. Lo que se
  parte a mano en Rust (el PDF de un chat, el texto pegado en el lienzo:
  `pixpin_motor2d::cortes`) parte también entre ideogramas y kana, sin dejar `。` o `」` al
  principio de un renglón; y mide un ideograma como un em.
- **Fechas** (`crates/pixpin-timeline/src/lengua.rs`): los nombres de meses y días y cómo se
  escribe una fecha en cada idioma («2026年10月5日月曜日», «Понедельник, 5 октября 2026 г.»), con
  los datos de CLDR. Los «hace 5 min» y los meses del chat ya eran de Fluent.

## Añadir un idioma más

1. `crates/pixpin-store/src/idioma.rs`: la variante de `Idioma`, en `TODOS`, `nombre_propio`,
   `es_rtl` si va de derecha a izquierda y su caso en `idioma_del_locale`.
2. `crates/pixpin-store/build.rs` y `herramientas/idiomas.py`: la etiqueta en `ETIQUETAS`, en el
   mismo orden que `TODOS` (un test lo vigila).
3. `crates/pixpin-store/src/ajustes.rs`: la variante de `PreferenciaIdioma` (con su `rename`), y
   en `idioma()` y `de()`.
4. `crates/pixpin-timeline/src/lengua.rs`: su tabla de fechas.
5. Lo mismo en `apple/nucleo/pixpin-store` (es copia: `idioma.rs`, `ajustes.rs`, `build.rs`).
6. El catálogo, cuando llegue.

## Lo que falta

- **Interfaz en espejo**: con el árabe, los rótulos van de derecha a izquierda, pero la
  disposición de las ventanas no se refleja (la barra lateral sigue a la izquierda, los iconos
  delante del texto, las flechas «atrás» apuntando a la izquierda). Haría falta que cada ventana
  pinte con `x` reflejada.
- **Texto del lienzo y del lector**: los textos que escribe el usuario en el lienzo y los
  documentos del lector siguen de izquierda a derecha (tienen su propia alineación y su cursor);
  DirectWrite sí ordena bien las palabras árabes dentro del renglón.
- **Chino tradicional** (`zh-TW`) y **portugués de Portugal** (`pt-PT`): caen al simplificado y al
  de Brasil.
- **Locale del sistema**: «El de Windows» mira `GetUserDefaultLocaleName` (el formato regional), no
  el idioma de la interfaz de Windows; quien tiene Windows en inglés con formato francés ve
  francés. Es lo de siempre, y se dejó así para no cambiar lo de español e inglés.
- **Textos escritos aún en el código** (no en Fluent) en español e inglés: los demás idiomas ven
  el inglés (`Idioma::base`). Otro trabajo los está pasando a los catálogos.
- La copia de `pixpin-timeline` en `apple/` sigue con las fechas en español e inglés.
- **Frases del dictado del timeline** («con esto pasó…», «y así te lo cuento…»,
  `pixpin_timeline::frases`): solo en español; por eso `timeline-vacio` las deja en español en
  todos los catálogos. Para hacerlas por idioma haría falta una lista de frases por idioma.
- **Buscar tareas por día**: «hoy» y «ayer» se reconocen en los catorce idiomas a la vez (las
  palabras de la pista `tareas3-buscar`, en `tareas::tarjetas::dia_de`). «hier» es ayer en
  francés y también «aquí» en alemán: buscarla añade lo de ayer a lo que la contiene.
- **`mini-moneda`**: cada catálogo trae la moneda de su país (CNY, JPY, KRW, INR, BRL, EUR, USD);
  es solo la moneda por defecto de un gasto nuevo, y `Moneda::de_codigo` sabe los decimales de
  cada una (JPY y KRW sin decimales).
