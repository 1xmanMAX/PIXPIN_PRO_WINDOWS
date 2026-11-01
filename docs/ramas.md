# Las dos ramas principales (1-nov-2026)

PixPin Max se mantiene en **dos ramas principales**, para que las
restricciones de la Microsoft Store no toquen el codigo general:

| Rama | Que lleva | Como se entrega |
|---|---|---|
| `main` | **Todo el codigo general, sin la Store.** Aqui se desarrolla todo lo nuevo. | El ZIP con `Instalar.cmd` (`.github/workflows/entrega.yml`, Releases `vX.Y.Z`). |
| `microsoft-store` | `main` **mas** lo que pide la Store y nada mas: el paquete MSIX y su manifiesto, la tarea de inicio del paquete en vez del registro, sin el actualizador propio, los permisos (capabilities) y lo que la certificacion obligue a cambiar. | El `.msix` que se sube a Partner Center (Releases `vX.Y.Z-store`). |

## Reglas

1. **Todo se desarrolla en `main`** (o en ramas que se unen a `main`). Nunca
   se programa una funcion nueva directamente en `microsoft-store`.
2. **`main` fluye hacia `microsoft-store`, nunca al reves**: cada vez que
   `main` avanza, se une (`git merge main`) en `microsoft-store`. Lo
   propio de la Store no vuelve a `main`.
3. En `microsoft-store` solo cambia **lo minimo** que exige la Store, y en
   ficheros propios cuando se pueda (`tienda/`, el manifiesto, un modulo
   `tienda.rs`), para que unir `main` no choque.
4. Si una restriccion de la Store obliga a quitar o cambiar algo, se quita
   **solo en `microsoft-store`**; en `main` sigue como esta.
5. Los agentes de los otros aparatos (Android, Mac, iPad) siguen leyendo
   `main`: la sincronizacion y los formatos son los mismos en las dos ramas.

## Los flujos de la Store tambien estan en `main`

GitHub solo deja lanzar a mano (`workflow_dispatch`) un flujo que exista en la
rama por defecto. Por eso `.github/workflows/tienda.yml` y
`capturas-tienda.yml` estan tambien en `main`, pero se lanzan con
`ref: microsoft-store`: compilan y empaquetan lo de esa rama. En `main` no
cambian nada de la app.
