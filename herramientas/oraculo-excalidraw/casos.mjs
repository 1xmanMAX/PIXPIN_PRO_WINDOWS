// Trazos de prueba. Deterministas: nada de Math.random, para que regenerar
// dé el mismo fichero byte a byte si el original no cambió.
const rango = (n, f) => Array.from({ length: n }, (_, i) => f(i));

const trazos = {
  recta: rango(40, (i) => [i * 5, 0]),
  curva: rango(60, (i) => [i * 4, Math.sin(i / 6) * 40]),
  espiral: rango(120, (i) => {
    const a = i / 8;
    const r = 4 + i * 0.9;
    return [Math.cos(a) * r, Math.sin(a) * r];
  }),
  // Un ocho: el contorno se cruza consigo mismo (el caso de los agujeros).
  bucle: rango(80, (i) => {
    const a = (i / 80) * 2 * Math.PI * 1.2;
    return [Math.sin(a * 2) * 60, Math.sin(a) * 60];
  }),
  punto: [[10, 10]],
  dos_puntos: [[0, 0], [30, 12]],
  zigzag: rango(30, (i) => [i * 25, i % 2 ? 40 : 0]),
  lento_rapido: rango(50, (i) => [i * i * 0.4, i * 2]),
};

const presionCreciente = (n) => rango(n, (i) => i / (n - 1));

export const casos = [
  ...Object.entries(trazos).flatMap(([nombre, puntos]) => [
    { nombre: `${nombre}_variable_medio`, puntos, presiones: [], grosor: 1, variabilidad: "variable", streamline: 0.5 },
    { nombre: `${nombre}_constante_medio`, puntos, presiones: [], grosor: 1, variabilidad: "constant", streamline: 0.5 },
  ]),
  { nombre: "recta_variable_fino", puntos: trazos.recta, presiones: [], grosor: 0.5, variabilidad: "variable", streamline: 0.5 },
  { nombre: "recta_variable_grueso", puntos: trazos.recta, presiones: [], grosor: 2, variabilidad: "variable", streamline: 0.5 },
  { nombre: "curva_lapiz_presion", puntos: trazos.curva, presiones: presionCreciente(60), grosor: 1, variabilidad: "variable", streamline: 0.2 },
  { nombre: "espiral_lapiz_presion", puntos: trazos.espiral, presiones: presionCreciente(120), grosor: 2, variabilidad: "variable", streamline: 0.2 },
  { nombre: "zigzag_constante_lapiz", puntos: trazos.zigzag, presiones: [], grosor: 2, variabilidad: "constant", streamline: 0.2 },
];
