# Distribución: código privado, descargas públicas (7-nov-2026)

El código de PinDesk (Windows, Android, Mac, iPad) pasa a ser **privado y
cerrado** (`LICENSE`: licencia propietaria). Lo público vive en otro
repositorio, **1xmanMAX/PinDesk**, que solo tiene:

- la guía de la app (qué hace y cómo se usa), los planes Gratis/Pro, la
  política de privacidad y los términos;
- las **Releases** con los instaladores.

La web enlaza ahí:

| Para | Enlace |
|---|---|
| Descarga directa de Windows (siempre la última) | `https://github.com/1xmanMAX/PinDesk/releases/latest/download/PinDesk-Windows-x64.zip` |
| Página de la última versión | `https://github.com/1xmanMAX/PinDesk/releases/latest` |
| Política de privacidad (la app y las tiendas) | `https://github.com/1xmanMAX/PinDesk/blob/main/docs/privacidad.md` |

## Cómo llega una versión al repositorio público

`entrega.yml` (Actions → Entrega → Run workflow, con `version`) compila,
publica la versión en las Releases de este repositorio privado (para uso
interno) y, si existe el secreto **`PINDESK_PUBLICO_TOKEN`**, la publica
también en `1xmanMAX/PinDesk` con el nombre fijo `PinDesk-Windows-x64.zip`
(etiqueta `vX.Y.Z`; si ya existe, reemplaza el archivo). Sin el secreto, ese
paso se salta y nada falla.

## Lo que hace el dueño (una vez)

1. **Crear el repositorio público** `1xmanMAX/PinDesk` (vacío, público).
2. **Poner privados** los repositorios de código: en cada uno, Settings →
   General → Danger Zone → Change repository visibility → Private.
   - `PIXPIN_PRO_WINDOWS`, `PIXPIN_PRO_ANDROID`, `PIXPIN_PRO_MAC`,
     `PIXPIN_PRO_iPAD`, `CAD-pixpin-`.
   - `PIXPIN_PRO_PLUGIN_FLOWLAUNCHER`: el catálogo de Flow Launcher descarga
     el plugin desde las Releases de su repositorio; si se pone privado, el
     plugin deja de instalarse desde Flow. O se deja público, o sus
     versiones se publican también en `1xmanMAX/PinDesk` y se cambia la URL en
     el catálogo de Flow.
3. **Crear el token** para publicar: GitHub → Settings → Developer settings →
   Fine-grained tokens → Generate new token. Repository access: *Only select
   repositories* → `1xmanMAX/PinDesk`. Permissions → Repository → **Contents:
   Read and write**. Caducidad: la más larga que permita (y apuntar cuándo
   renovarlo).
4. **Guardarlo como secreto** en `PIXPIN_PRO_WINDOWS` (y luego en los de
   Android/Mac/iPad cuando publiquen ahí): Settings → Secrets and variables →
   Actions → New repository secret → nombre `PINDESK_PUBLICO_TOKEN`.
5. **Cambiar la URL de privacidad** en Partner Center (Microsoft Store) y en
   Google Play Console a la del repositorio público (tabla de arriba): la
   antigua (`PIXPIN_PRO_WINDOWS/blob/main/docs/privacidad.md`) deja de verse
   al poner el repositorio privado.

## A tener en cuenta

- **Minutos de Actions.** En repositorios privados, el plan gratis de GitHub da
  2000 minutos al mes, y los minutos de Windows cuentan doble (los de macOS,
  diez veces). Una entrega de Windows tarda unos 30–40 min → ~70 minutos
  facturables; la CI de cada push también gasta. Conviene lanzar la CI pesada
  solo en `main` o a mano, o pasar a GitHub Pro / Team.
- **Lo ya publicado con MIT** sigue siendo MIT para quien lo copió antes del
  cambio; la licencia propietaria rige desde ahora.
- **Releases antiguas** de este repositorio dejan de ser públicas al ponerlo
  privado: todos los enlaces viejos (`PIXPIN_PRO_WINDOWS/releases/...`) dejan
  de funcionar para otros.
