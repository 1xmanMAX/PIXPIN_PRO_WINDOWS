/**
 * La nube de referencia (repositorio agente-2.0-constrution) arrancada en
 * este equipo para `nube::de_verdad` de pindesk-sincro. Lee la carpeta del
 * repositorio de `PINDESK_NUBE_AGENTE`, da de alta dos aparatos y escribe en
 * una linea {puerto, max, ana}. Se cierra al cerrarse su entrada.
 */
import { createServer } from 'node:http'
import { mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

const raiz = process.env.PINDESK_NUBE_AGENTE
if (!raiz) throw new Error('falta PINDESK_NUBE_AGENTE')
const nubeDir = join(raiz, 'src/herramientas/nube')
const { AlmacenNube } = await import(pathToFileURL(join(nubeDir, 'almacen.ts')).href)
const { atenderApi } = await import(pathToFileURL(join(nubeDir, 'servir.ts')).href)

const dir = mkdtempSync(join(tmpdir(), 'pindesk-nube-'))
const nube = new AlmacenNube(join(dir, 'nube'))
const max = nube.altaDeAparato('PC de Max', 'max', 'R')
const ana = nube.altaDeAparato('PC de Ana', 'ana', 'R')
const servidor = createServer((req, res) => {
  atenderApi(req, res, new URL(req.url ?? '/', 'http://x'), nube).catch((e: unknown) => {
    res.writeHead(500)
    res.end(String(e))
  })
})
servidor.listen(0, '127.0.0.1', () => {
  const a = servidor.address()
  const puerto = typeof a === 'object' && a ? a.port : 0
  process.stdout.write(JSON.stringify({
    puerto,
    max: { llave: max.llave, id: max.aparato.id },
    ana: { llave: ana.llave, id: ana.aparato.id },
  }) + '\n')
})
process.stdin.resume()
process.stdin.on('end', () => {
  servidor.close()
  rmSync(dir, { recursive: true, force: true })
  process.exit(0)
})
