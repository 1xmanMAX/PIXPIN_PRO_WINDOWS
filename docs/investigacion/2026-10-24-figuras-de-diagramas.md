# Figuras de diagramas: qué traen las demás y qué añadimos (24-oct-2026)

Pregunta del usuario: «investiga qué otras figuras hacen falta para hacer
diagramas y añádelas, para que de verdad le sirva a la gente» (estudiantes,
profesionales, docentes). Antes ya teníamos: diagrama de flujo ISO 5807 (26),
electrónica (45), marcas y sellos (19), una hoja de mapa mental en
«Académico», una línea de tiempo jurídica en «Derecho», flechas finas de
storyboard y las de ciencias.

## Qué traen las otras herramientas

| Herramienta | Bibliotecas de formas para diagramas |
|---|---|
| draw.io / diagrams.net | «General» (formas básicas, texto, conectores: basta para flujo y mapas mentales), UML y UML 2.5 (clases por compartimentos, notas, relaciones de herencia, realización, dependencia, asociación, agregación y composición), ERD (`mxgraph.er.*`), BPMN 2.0 (en «Business»; la norma recomienda unas 270 formas), redes y nube (genéricas, Cisco, AWS, Azure). [1][2][3] |
| Lucidchart | Por defecto: Standard, Flowchart, Containers y Shapes. Además: mapas mentales, maquetas, UML, ER, redes, organigramas, BPMN y Venn. [4] |
| Miro | Para todos: formas básicas, diagrama de flujo y conectores. En planes de pago: BPMN, UML, ERD, Data Flow, Value Stream y nube (AWS, Azure, GCP, Kubernetes, Cisco). [5] |
| FigJam | Formas básicas y de flujo (cilindro, documento, proceso predefinido) en todos los planes; «avanzadas» en los de pago; nube y redes en «More shapes». Sus usuarios se quejan de que no llega para BPMN. [6] |
| Visio | Abre con «Basic Shapes» y «Arrow Shapes»; «Callouts» en Visio Extras; «Business Matrix» con 2×2, 3×3, DAFO, Ansoff, BCG; marcos de negocio y líneas de tiempo. [7] |
| Excalidraw (bibliotecas de la comunidad) | Las más usadas: arquitectura de software, componentes de diseño de sistemas, iconos de AWS y diagramas de red (routers, switches, firewalls); UML de clases, secuencia y actividad. [8] |
| Whimsical | Biblioteca «limitada»: flujo, mapas mentales y wireframes. [8] |
| Canva | Plantillas de diagrama de flujo, mapa mental, organigrama, Venn (de 2, 3 y 4), DAFO, procesos y mapas de sitio; formas y conectores desde «Elementos». [9] |

## Qué se usa más

- **Diagrama de flujo**, de largo: en Miro la plantilla de flujo es la más
  usada (≈3,1 mil usos), seguida de mapeo de flujo de valor y «sí/no»; el de
  funciones cruzadas (carriles) ronda los 2 mil; el **organigrama** y el
  **mapa mental**, unos 1,4 mil cada uno. [10]
- **UML**: en las encuestas, el de **clases** es el primero (73 % lo usa en
  dos tercios o más de sus proyectos; 98 % lo ha usado alguna vez), luego
  **casos de uso** (87 %), **secuencia** (82 %) y **actividad** (75 %);
  tiempos, perfiles y colaboración, casi nada. [11][12]
- **BPMN**: menos del 20 % del vocabulario se usa con regularidad; lo de
  siempre es flujo de secuencia, tarea, evento de inicio y de fin y pool, y
  después compuertas, carriles y objetos de datos. [13]
- **Negocio y docencia**: DAFO/FODA, Venn, espina de pescado, línea de
  tiempo, embudo y pirámide son de lo más repetido en las bibliotecas de
  plantillas (Canva, Venngage, Visio). [7][9][14]

## Qué añadimos y por qué

Siete familias, 123 figuras, cada una por partes con nombre y textos de
ejemplo en español, con el trazo azul y relleno suave del diagrama de flujo.

| Familia (categoría) | Figuras | Por qué |
|---|---|---|
| **UML** (26) | clase, interfaz, enumeración, objeto, actor, caso de uso, límite del sistema, línea de vida con activación, nota, paquete, componente, nodo, acción, estado, estado inicial y final, decisión, bifurcación; relaciones de herencia, realización, asociación, dependencia, agregación y composición; mensaje síncrono y de respuesta | Cubre los cuatro diagramas más usados (clases, casos de uso, secuencia, actividad/estados) y lo básico de componentes y despliegue. Las relaciones son flechas de verdad con su punta UML. |
| **BPMN** (21) | eventos de inicio, inicio de mensaje, intermedio, temporizador, fin y fin de mensaje; tarea, de usuario, de servicio, subproceso; compuertas exclusiva, paralela, inclusiva y basada en eventos; pool con carriles; objeto de datos; anotación; grupo; flujos de secuencia, de mensaje y asociación | El 20 % de BPMN que sale en casi todos los modelos reales. |
| **Entidad-relación** (15) | entidad, entidad débil, atributo, clave, multivaluado, derivado, relación, relación identificadora, especialización, tabla con PK/FK, cardinalidades de pata de gallo (1..1, 0..1, 1..*, 0..*) y relación uno a muchos | Lo que se enseña en bases de datos (Chen) y lo que se usa en el modelo lógico (pata de gallo). |
| **Redes y TI** (16) | servidor, base de datos, nube, router, switch, cortafuegos, balanceador, wifi, PC, portátil, móvil, tableta, impresora, usuario, grupo de usuarios, API | Los genéricos de red y arquitectura de sistemas (lo más bajado en Excalidraw), sin marcas de fabricante. |
| **Mapas mentales y organigramas** (8) | tema central, idea principal, subtema, rama curva, mapa de cuatro ramas, caja de cargo con foto, caja de cargo, organigrama de ejemplo | Mapas y organigramas están entre los más usados; ya había la hoja entera de mapa mental, faltaban las piezas. |
| **Estrategia y negocio** (14) | DAFO/FODA, matriz 2×2, Eisenhower, embudo, pirámide, ciclo PDCA, Venn de 2 y de 3, Ishikawa, línea de tiempo, Kanban, lienzo de modelo de negocio, chevrones, pros y contras | Lo que más se repite en plantillas de empresa y en clase. |
| **Formas y llamadas** (23) | bocadillos (diálogo, ovalado, pensamiento), llamadas con guía y con flecha, nota adhesiva, llave, flechas gruesas (derecha, doble, curva, en U, en cruz), chevron, estrella, estallido, cinta, contenedor con título, tabla 3×3, iconos de correcto, incorrecto, advertencia, información y pregunta | Lo que traen todas por defecto (Basic Shapes, Arrow Shapes, Callouts, Containers) y no teníamos. |

Lo que se deja fuera: los cientos de iconos de fabricante (AWS, Azure,
Cisco: cambian cada año y tienen marca), los diagramas UML poco usados
(tiempos, perfiles, colaboración) y el resto de BPMN (coreografías,
compensación, escalado). Lo que ya existía no se repite: el cilindro y el
rombo de decisión grande siguen en «Diagrama de flujo», el cronograma en
«Planificación» y la hoja de mapa mental en «Académico».

## Fuentes

1. draw.io, «How many shapes do you need to draw technical diagrams?» — https://www.drawio.com/blog/technical-diagramming-shapes
2. draw.io, «UML class diagrams» — https://www.drawio.com/blog/uml-class-diagrams
3. draw.io, «Network and technical diagram shapes» — https://drawio-app.com/blog/network-and-technical-diagram-shapes-in-draw-io/
4. Lucid, «Shape libraries in Lucidchart» — https://help.lucid.co/hc/en-us/articles/14931750819476
5. Miro, diagramación y paquetes de formas — https://miro.com/online-diagram-and-map-maker/ y https://community.miro.com/product-news-31/uml-shape-pack-for-software-modeling-ease-7286
6. Figma, ayuda de formas de FigJam y foro — https://help.figma.com/hc/en-us/articles/1500004414382 y https://forum.figma.com/suggest-a-feature-11/feature-request-allow-company-org-shape-kits-in-figjam-more-shapes-sidebar-43262
7. Microsoft, Visio para la web: marcos de negocio y líneas de tiempo — https://insider.microsoft365.com/blog/create-business-frameworks-and-infographic-timelines-in-visio-for-the-web y https://support.microsoft.com/office/9ca66c90-04f1-42c8-8176-1687ffb35d46
8. Bibliotecas de Excalidraw y comparativa con Miro y Whimsical — https://aptify.atlassian.net/wiki/spaces/EDLFC/pages/17596426/Excalidraw+Diagrams+Libraries+for+Confluence+User+Guide y https://kx.cloudingenium.com/excalidraw-self-hosted-whiteboard-diagrams-wireframes-collaboration
9. Canva, plantillas de pizarra y Venn — https://www.canva.com/online-whiteboard/templates/team/ y https://www.canva.com/graphs/venn-diagrams/
10. Miro, plantillas de diagramas con sus usos — https://miro.com/templates/diagrams/?page=1 y https://miro.com/templates/diagrams/?page=2
11. Dobing y Parsons, «How UML is used», CACM 2006 — https://cacm.acm.org/?p=1967
12. UML Forum, resultados de la encuesta — https://groups.google.com/g/umlforum/c/HhYMd_UsRCI
13. zur Muehlen y Recker, «How Much Language Is Enough?» (BPMN) — https://researchwith.stevens.edu/en/publications/how-much-language-is-enough-theoretical-and-practical-use-of-the-
14. Venngage, plantillas de diagramas — https://venngage.com/templates/diagrams
