//! **La tinta y los marcos DENTRO de la pagina** (19-oct-2026).
//!
//! El usuario: «en algunos visores como HTML, cuando anoto y luego me
//! desplazo, la anotacion se mueve con retraso. La anotacion siempre tiene
//! que estar pegada al punto donde se hizo: al desplazar se tiene que mover
//! junto, sin retraso».
//!
//! Antes la tinta y los marcos iban solo en la ventana de encima de la
//! pagina (la de [`super::riel`]), que le preguntaba a la pagina por donde
//! iba cada 40 ms y se repintaba: siempre uno o dos fotogramas por detras
//! del desplazamiento. Ahora, mientras no se esta trazando, lo dibujado va
//! **dentro de la pagina**, como una capa con posicion absoluta en las
//! coordenadas del documento (pixeles CSS): el navegador la desplaza junto
//! con el texto, en el mismo fotograma, sea con la rueda, con la barra, con
//! el teclado o con el panel tactil. La ventana de encima solo pinta la
//! tinta mientras hay un gesto a medias (el trazo que se dibuja, el texto
//! que se escribe; editar la tinta es siempre cosa de [`Tinta`]) y en cuanto
//! la pagina confirma que tiene la capa nueva la suelta.
//!
//! La tinta sale con el mismo SVG que exporta el lienzo
//! ([`pixpin_motor2d::exportar_svg`]), escrito desde las mismas ordenes que
//! pinta la pantalla, y con la tinta adaptada al papel de noche como la
//! pinta el lector ([`crate::dibujo::tema`]): se ve igual que en la ventana.
//! Los marcos, con el azul, el grosor y el numero de los de
//! [`crate::marcos_doc`], en pixeles de la pantalla (no crecen con el zoom).
//!
//! El mismo guion pone en la pagina dos escuchas mas:
//!
//! - **El desplazamiento al momento**: con cada `scroll` (uno por
//!   fotograma) manda por donde va, sin que nadie pregunte. Lo que la ventana
//!   de encima pinta (la barra de la tinta, lo elegido) lo sigue sin sondeo.
//! - **Las teclas de los visores** (I, M, A, G, Ctrl+M, Ctrl+P con marcos):
//!   la pagina se queda el teclado y sin esto solo el PDF las entendia. Fuera
//!   de un campo de texto se mandan al visor; dentro (el editor de notas)
//!   son del texto (Ctrl+M es alli la formula).
//!
//! [`Tinta`]: crate::lector_tinta::Tinta

use pixpin_motor2d::elemento::ColorRgba;

use super::riel::Sitio;
use crate::lector_tinta::Capa;
use crate::marcos_doc::Marco;

/// **El guion que se instala una vez en la pagina** (`window.__pixpinRiel`):
/// donde se desplaza, la capa de la tinta y los marcos, y las dos escuchas.
/// Si la pagina se recarga se pierde y el siguiente guion lo vuelve a poner
/// (`nuevo` en la respuesta: hay que mandarle la capa otra vez).
pub const INSTALAR: &str = r#"(function(){
if(window.__pixpinRiel)return;
var R={nuevo:true,ctrlp:false,menu:false,capa:null,raf:0};window.__pixpinRiel=R;
function caja(e){if(!e||e===document.body||e===document.documentElement)return false;var o=getComputedStyle(e).overflowY;return(o==='auto'||o==='scroll'||o==='overlay')&&e.scrollHeight>e.clientHeight+1;}
R.donde=function(){var e=document.elementFromPoint(innerWidth/2,innerHeight/2);while(e&&!caja(e))e=e.parentElement;return e;};
function cual(){var e=R.donde();return{raiz:!e,s:e||document.scrollingElement||document.documentElement};}
function maximo(d){return d.raiz?Math.max(0,d.s.scrollHeight-innerHeight):Math.max(0,d.s.scrollHeight-d.s.clientHeight);}
function area(d){var s=d.s,x,y,ww,hh,bx=0,by=0;if(d.raiz){x=0;y=0;ww=document.documentElement.clientWidth||innerWidth;hh=document.documentElement.clientHeight||innerHeight;}else{var c=s.getBoundingClientRect();bx=c.left;by=c.top;x=c.left+s.clientLeft;y=c.top+s.clientTop;ww=s.clientWidth;hh=s.clientHeight;}
var x0=Math.max(0,x),y0=Math.max(0,y),x1=Math.min(innerWidth,x+ww),y1=Math.min(innerHeight,y+hh);return{bx:bx,by:by,ax:x0,ay:y0,aw:Math.max(0,x1-x0),ah:Math.max(0,y1-y0)};}
R.ir=function(modo,a,b){var d=cual(),max=maximo(d),w=d.raiz?window:d.s;
function ir(){var q=area(d);if(modo==='ir')w.scrollTo({top:a*max,behavior:'instant'});if(modo==='bajar')w.scrollBy({top:a,behavior:'instant'});if(modo==='colocar')w.scrollTo({left:a+q.bx-q.ax,top:b+q.by-q.ay,behavior:'instant'});}
if(modo==='mirar')return;ir();if(modo==='colocar'){requestAnimationFrame(ir);setTimeout(ir,80);}};
R.sitio=function(){var d=cual(),s=d.s,max=maximo(d),r;if(d.raiz){r={x:scrollX,y:scrollY,arriba:scrollY,maximo:max};}else{var c=s.getBoundingClientRect();r={x:s.scrollLeft-c.left,y:s.scrollTop-c.top,arriba:s.scrollTop,maximo:max};}
r.x-=R.desfase(s);var M=window.__pixpinMaqueta;if(M)r.maq=M.medir();
var q=area(d);r.ax=q.ax;r.ay=q.ay;r.aw=q.aw;r.ah=q.ah;var v=window.visualViewport;r.vx=v?v.offsetLeft:0;r.vy=v?v.offsetTop:0;r.escala=v?v.scale:1;r.dpr=devicePixelRatio||1;return r;};
function mandar(o){try{window.chrome.webview.postMessage(JSON.stringify(o));}catch(_){}}
R.desfase=function(s){var M=window.__pixpinMaqueta;return M&&M.desfase?M.desfase(s):0;};
R.colgar=function(){var c=R.capa;if(!c)return;var e=R.donde(),padre=e||document.documentElement;if(!padre)return;if(c.parentNode!==padre)padre.appendChild(c);var D=R.desfase(e||document.scrollingElement||document.documentElement);
if(e){if(getComputedStyle(e).position==='static')e.style.position='relative';c.style.left=(D-e.clientLeft)+'px';c.style.top=(-e.clientTop)+'px';}else{c.style.left=D+'px';c.style.top='0px';}};
R.poner=function(t,m,ver,ctrlp,menu){R.ctrlp=ctrlp;R.menu=menu;var c=R.capa;
if(!c){c=R.capa=document.createElement('div');c.id='pixpin-riel-capa';c.setAttribute('aria-hidden','true');c.style.cssText='position:absolute;left:0;top:0;width:0;height:0;margin:0;padding:0;border:0;overflow:visible;pointer-events:none;user-select:none;z-index:2147483647';c.appendChild(document.createElement('div'));c.appendChild(document.createElement('div'));}
R.colgar();var a=c.firstChild,b=c.lastChild;if(a.__t!==t){a.innerHTML=t;a.__t=t;}a.style.visibility=ver?'visible':'hidden';if(b.__t!==m){b.innerHTML=m;b.__t=m;}
return location.protocol==='about:'?0:1;};
function editable(n){return!!n&&(n.isContentEditable||/^(INPUT|TEXTAREA|SELECT)$/.test(n.tagName||''));}
addEventListener('keydown',function(ev){if(ev.isComposing||ev.altKey||ev.metaKey||ev.repeat)return;var k=(ev.key||'').toLowerCase(),ed=editable(ev.target)||editable(document.activeElement),vk=0;
if(ev.ctrlKey){if(k==='p'&&R.ctrlp)vk=80;else if(k==='m'&&!ed&&!ev.shiftKey)vk=77;}
else if(!ed&&!ev.shiftKey){if(k==='i')vk=73;else if(k==='m')vk=77;else if(k==='a')vk=65;else if(k==='g'&&R.menu)vk=71;}
if(!vk)return;ev.preventDefault();ev.stopPropagation();mandar({pixpin_riel:'tecla',vk:vk,ctrl:!!ev.ctrlKey});},true);
function aviso(){R.raf=0;if(R.capa)R.colgar();var r=R.sitio();r.pixpin_riel='sitio';mandar(r);}
function pronto(){if(!R.raf)R.raf=requestAnimationFrame(aviso);}
addEventListener('scroll',pronto,{capture:true,passive:true});addEventListener('resize',pronto,{passive:true});
if(window.visualViewport){visualViewport.addEventListener('scroll',pronto,{passive:true});visualViewport.addEventListener('resize',pronto,{passive:true});}
})();"#;

/// **Lo que tiene que haber en la pagina**: la tinta (SVG, vacio sin
/// tinta), si se ve (no mientras se traza: la pinta la ventana de encima),
/// los marcos y lo que tienen que mandar sus teclas.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Deseado {
    pub tinta: String,
    pub ver_tinta: bool,
    pub marcos: String,
    /// Ctrl+P es del visor (hay marcos y no lo coge ya el WebView2).
    pub ctrl_p: bool,
    /// La G abre el menu ⋮ del visor.
    pub menu: bool,
    /// **La maqueta de la tinta** de una nota (JSON de [`Maqueta`]): la
    /// medida de la columna con que se anoto. `None`: sin maqueta.
    pub maqueta: Option<String>,
    /// La columna puesta a esa medida (el marcador ✏️ o el lapiz).
    pub fija: bool,
}

/// El guion que deja `d` en la pagina (con [`INSTALAR`] delante). Devuelve
/// 1 si quedo puesto en una pagina de verdad, 0 si aun era la vacia del
/// principio (`about:blank`: la de verdad no ha llegado).
pub fn guion_poner(d: &Deseado) -> String {
    let js = |t: &str| serde_json::to_string(t).unwrap_or_else(|_| "\"\"".into());
    format!(
        // La maqueta antes que la capa: la capa se cuelga con su desfase.
        "{INSTALAR}\nif(window.__pixpinMaqueta)window.__pixpinMaqueta.poner({},{});\nwindow.__pixpinRiel.poner({},{},{},{},{})",
        d.maqueta.as_deref().unwrap_or("null"),
        d.fija,
        js(&d.tinta),
        js(&d.marcos),
        d.ver_tinta,
        d.ctrl_p,
        d.menu
    )
}

/// **La maqueta de la tinta de una nota** (20-oct-2026). El usuario: que las
/// anotaciones con el lapiz «no se muevan al sincronizar… como se hace en el
/// BIM: un marcador especial en el que al darle clic la disposicion del texto
/// se corrija a cierta escala». La tinta va en pixeles CSS del documento, y
/// el texto de una nota se recoloca con el ancho de la ventana (y del
/// movil): con otra medida, la tinta ya no cae sobre lo que se anoto. Por
/// eso se guarda la medida de la columna con que se anoto (`ancho`, su caja
/// en pixeles CSS; `tam`, la letra) y donde empezaba (`ox`, su borde
/// izquierdo dentro de la caja que se desplaza). Con la columna a esa
/// medida el texto cae igual en cualquier equipo; si la ventana es mas
/// ancha, la columna se centra y la tinta la sigue (el desfase de
/// `__pixpinMaqueta.desfase`, `ox` de ahora menos el guardado).
///
/// La pagina la mide (`Sitio::maq`, con la misma forma); se guarda en las
/// vistas del documento (`marcos_doc::Vistas`, clave [`CLAVE_MAQUETA`]), que
/// viajan con la sincro.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Maqueta {
    pub ancho: f32,
    pub tam: f32,
    #[serde(default)]
    pub ox: f32,
}

/// La clave de la maqueta en las vistas del documento.
pub const CLAVE_MAQUETA: &str = "maqueta";

impl Maqueta {
    /// Si el texto cae igual con la medida `m` de ahora (medio pixel de
    /// holgura: la caja se mide redondeada).
    pub fn coincide(&self, m: &Maqueta) -> bool {
        (self.ancho - m.ancho).abs() < 1.5 && (self.tam - m.tam).abs() < 0.25
    }

    pub fn a_texto(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn de_texto(t: &str) -> Option<Maqueta> {
        serde_json::from_str::<Maqueta>(t)
            .ok()
            .filter(|m| m.ancho > 50.0 && m.tam > 4.0)
    }
}

/// **Lo que manda la pagina por su cuenta** (`chrome.webview.postMessage`).
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
#[serde(tag = "pixpin_riel", rename_all = "snake_case")]
pub enum DelGuion {
    /// Se desplazo (o cambio el zoom): por donde va ahora.
    Sitio(Sitio),
    /// Una tecla de los visores pulsada fuera de un campo de texto.
    Tecla {
        vk: u32,
        #[serde(default)]
        ctrl: bool,
    },
}

/// El mensaje de la pagina, si es del riel (lo demas es de quien la lleva).
pub fn mensaje_de(json: &str) -> Option<DelGuion> {
    if !json.contains("pixpin_riel") {
        return None;
    }
    serde_json::from_str::<DelGuion>(json)
        .ok()
        .filter(|m| match m {
            DelGuion::Sitio(s) => super::riel::sitio_valido(s),
            DelGuion::Tecla { .. } => true,
        })
}

/// **La tinta de `capa` como SVG para meter en la pagina**, sobre `papel`:
/// colocada en el documento (pixeles CSS) con posicion absoluta. Vacio sin
/// tinta.
pub fn svg_de_la_tinta(capa: &Capa, papel: ColorRgba) -> String {
    use pixpin_motor2d::exportar::{self, Alcance};
    if capa.vacia() {
        return String::new();
    }
    // Como la pinta el lector sobre ese papel: de noche, la tinta oscura se
    // aclara (y el grafito se cuece con el color ya adaptado).
    let mut escena = capa.escena.clone();
    escena.fondo = papel;
    crate::dibujo::tema::grafito_como_en_el_lienzo(&mut escena);
    let Some(mut hoja) = exportar::hojas(&escena, Alcance::Todo, &[], None)
        .into_iter()
        .next()
    else {
        return String::new();
    };
    hoja.ordenes =
        crate::dibujo::tema::ordenes_como_en_el_lienzo(std::mem::take(&mut hoja.ordenes), papel);
    let svg = pixpin_motor2d::exportar_svg::svg(
        &hoja,
        pixpin_motor2d::exportar_svg::OpcionesSvg {
            fondo: None,
            marcos: false,
        },
        &|_| None,
    );
    let svg = pixpin_motor2d::exportar_html::documento::sin_cabecera(&svg);
    let (x0, y0, _, _) = hoja.caja;
    match svg.strip_prefix("<svg ") {
        Some(resto) => format!(
            "<svg style=\"position:absolute;left:{}px;top:{}px;overflow:visible\" {resto}",
            num(x0 as f64),
            num(y0 as f64)
        ),
        None => String::new(),
    }
}

/// **Los marcos numerados como SVG para la pagina**: la raya azul de 2 px y
/// el numero en su cuadrito, como los pinta [`crate::marcos_doc::Marcos`],
/// con `z` pixeles de pantalla por pixel CSS y la escala `e` del monitor
/// (para que midan en la pantalla lo mismo que los de la ventana).
pub fn svg_de_los_marcos(lista: &[Marco], z: f64, e: f64) -> String {
    let lista: Vec<&Marco> = lista.iter().filter(|m| m.parte == 0).collect();
    if lista.is_empty() {
        return String::new();
    }
    let u = e / z.max(0.01);
    let azul = "#0078d4";
    let mut s = String::from(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\" style=\"position:absolute;left:0;top:0;overflow:visible\">",
    );
    for (i, m) in lista.iter().enumerate() {
        let [x0, y0, x1, y1] = m.caja;
        s.push_str(&format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"none\" stroke=\"{azul}\" stroke-width=\"{}\"/>",
            num(x0),
            num(y0),
            num(x1 - x0),
            num(y1 - y0),
            num(2.0 * u)
        ));
        let n = (i + 1).to_string();
        let tam = 12.0 * u;
        let ancho = pixpin_motor2d::exportar::ancho_de_texto(&n, tam as f32) as f64 + 12.0 * u;
        s.push_str(&format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{azul}\"/><text x=\"{}\" y=\"{}\" font-family=\"Segoe UI, sans-serif\" font-size=\"{}\" fill=\"#fff\">{n}</text>",
            num(x0),
            num(y0),
            num(ancho),
            num(20.0 * u),
            num(x0 + 6.0 * u),
            // La linea de base de Segoe UI: la caja del texto empieza 3 px
            // por debajo del borde y su ascenso es 1,08 veces el tamano.
            num(y0 + 3.0 * u + 1.079 * tam),
            num(tam)
        ));
    }
    s.push_str("</svg>");
    s
}

/// Un numero corto para el SVG (dos decimales, sin ceros de mas).
fn num(v: f64) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let t = format!("{:.2}", v);
    let t = t.trim_end_matches('0').trim_end_matches('.');
    if t == "-0" { "0".into() } else { t.to_string() }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_mensajes_del_riel_se_entienden_y_los_demas_no_son_suyos() {
        let s = mensaje_de(
            r#"{"pixpin_riel":"sitio","x":0,"y":420,"arriba":420,"maximo":900,"dpr":1.25}"#,
        );
        match s {
            Some(DelGuion::Sitio(s)) => assert_eq!((s.y, s.dpr), (420.0, 1.25)),
            otro => panic!("{otro:?}"),
        }
        assert_eq!(
            mensaje_de(r#"{"pixpin_riel":"tecla","vk":73,"ctrl":false}"#),
            Some(DelGuion::Tecla {
                vk: 73,
                ctrl: false
            })
        );
        assert_eq!(
            mensaje_de(r#"{"pixpin_riel":"tecla","vk":77,"ctrl":true}"#),
            Some(DelGuion::Tecla { vk: 77, ctrl: true })
        );
        // Casos negativos: lo del lector de notas no es del riel, y un sitio
        // con numeros rotos no se toma.
        assert_eq!(
            mensaje_de(r#"{"tipo":"guardar","texto":"hola","version":3}"#),
            None
        );
        assert_eq!(mensaje_de(r#"{"pixpin_riel":"otra"}"#), None);
        assert_eq!(mensaje_de("no es json pixpin_riel"), None);
    }

    #[test]
    fn el_guion_de_la_capa_instala_una_vez_y_escapa_lo_que_mete() {
        let d = Deseado {
            tinta: "<svg a=\"1\">'</svg>".into(),
            ver_tinta: true,
            marcos: String::new(),
            ctrl_p: false,
            menu: true,
            ..Default::default()
        };
        let g = guion_poner(&d);
        assert!(g.starts_with(INSTALAR));
        assert!(
            g.ends_with(r#"poner("<svg a=\"1\">'</svg>","",true,false,true)"#),
            "{g}"
        );
        // Se instala una sola vez por pagina, y escucha el desplazamiento y
        // las teclas.
        assert!(INSTALAR.starts_with("(function(){\nif(window.__pixpinRiel)return;"));
        assert!(INSTALAR.contains("addEventListener('scroll'"));
        assert!(INSTALAR.contains("addEventListener('keydown'"));
        // Caso negativo: dentro de un campo de texto las letras son del
        // texto (el editor de notas usa Ctrl+M para la formula).
        assert!(INSTALAR.contains("k==='m'&&!ed"));
        assert!(INSTALAR.contains("else if(!ed&&!ev.shiftKey)"));
    }

    #[test]
    fn los_marcos_miden_en_la_pantalla_lo_mismo_que_en_la_ventana() {
        let m = |c: [f64; 4], parte: u32| Marco {
            caja: c,
            parte,
            vista: String::new(),
        };
        // Pantalla al 150 % y pagina al 100 %: 1,5 pixeles por pixel CSS; la
        // raya de 2 px de pantalla (a escala 1,5: 3 px) son 2 px CSS.
        let s = svg_de_los_marcos(
            &[
                m([10.0, 20.0, 110.0, 70.0], 0),
                m([0.0, 300.0, 50.0, 350.0], 0),
            ],
            1.5,
            1.5,
        );
        assert!(s.contains(r##"<rect x="10" y="20" width="100" height="50" fill="none" stroke="#0078d4" stroke-width="2"/>"##), "{s}");
        assert!(s.contains(">1</text>") && s.contains(">2</text>"));
        // Con zoom de pagina al 200 %, mas finos en CSS (iguales en pantalla).
        let z = svg_de_los_marcos(&[m([10.0, 20.0, 110.0, 70.0], 0)], 3.0, 1.5);
        assert!(z.contains(r#"stroke-width="1""#), "{z}");
        // Casos negativos: sin marcos (o solo de otra parte), nada.
        assert_eq!(svg_de_los_marcos(&[], 1.0, 1.0), "");
        assert_eq!(
            svg_de_los_marcos(&[m([0.0, 0.0, 9.0, 9.0], 2)], 1.0, 1.0),
            ""
        );
    }

    #[test]
    fn sin_tinta_no_hay_svg_y_con_tinta_va_colocada_en_el_documento() {
        let capa = Capa::default();
        assert_eq!(
            svg_de_la_tinta(
                &capa,
                ColorRgba {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0
                }
            ),
            ""
        );
        let mut capa = Capa::default();
        let mut tinta = crate::lector_tinta::Tinta::nueva();
        let puntos: Vec<pixpin_motor2d::vector::Punto2> = (0..20)
            .map(|i| {
                pixpin_motor2d::vector::Punto2::nuevo(100.0 + i as f32 * 5.0, 900.0 + i as f32)
            })
            .collect();
        tinta.trazar(&mut capa, &puntos);
        assert!(!capa.vacia());
        let s = svg_de_la_tinta(
            &capa,
            ColorRgba {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
        );
        assert!(s.starts_with("<svg style=\"position:absolute;left:"), "{s}");
        assert!(!s.contains("<?xml"));
        // Va donde se dibujo: cerca de y = 900 del documento, no arriba.
        let top: f64 = s
            .split("top:")
            .nth(1)
            .and_then(|t| t.split("px").next())
            .and_then(|t| t.parse().ok())
            .expect("top");
        assert!((850.0..900.0).contains(&top), "{top}");
    }

    #[test]
    fn los_numeros_salen_cortos() {
        assert_eq!(num(2.0), "2");
        assert_eq!(num(1.256), "1.26");
        assert_eq!(num(-0.001), "0");
        assert_eq!(num(f64::NAN), "0");
    }

    #[test]
    fn la_maqueta_va_y_vuelve_y_dice_si_el_texto_cae_igual() {
        let m = Maqueta {
            ancho: 768.0,
            tam: 16.0,
            ox: 212.5,
        };
        assert_eq!(Maqueta::de_texto(&m.a_texto()), Some(m));
        // Medio pixel de redondeo: cae igual; otra columna u otra letra, no.
        assert!(m.coincide(&Maqueta {
            ancho: 768.6,
            tam: 16.0,
            ox: 40.0
        }));
        assert!(!m.coincide(&Maqueta {
            ancho: 540.0,
            tam: 16.0,
            ox: 0.0
        }));
        assert!(!m.coincide(&Maqueta {
            ancho: 768.0,
            tam: 18.0,
            ox: 212.5
        }));
        // Lo que no es una maqueta no se toma por una.
        assert_eq!(Maqueta::de_texto("1.25 10 20"), None);
        assert_eq!(Maqueta::de_texto(r#"{"ancho":0,"tam":16}"#), None);
        // La pagina la recibe antes que la capa (la capa se cuelga con su desfase).
        let g = guion_poner(&Deseado {
            maqueta: Some(m.a_texto()),
            fija: true,
            ..Default::default()
        });
        let (antes, despues) = g.split_once("__pixpinMaqueta.poner(").unwrap();
        assert!(
            antes.contains("__pixpinRiel")
                && despues.starts_with(&format!("{},true)", m.a_texto()))
        );
        assert!(despues.contains("window.__pixpinRiel.poner("));
        let sin = guion_poner(&Deseado::default());
        assert!(sin.contains("__pixpinMaqueta.poner(null,false)"));
        // Y un sitio con la medida de la columna se lee.
        let s: super::super::riel::Sitio =
            serde_json::from_str(r#"{"x":1,"y":2,"maq":{"ancho":700,"tam":15,"ox":30}}"#).unwrap();
        assert_eq!(
            s.maq,
            Some(Maqueta {
                ancho: 700.0,
                tam: 15.0,
                ox: 30.0
            })
        );
    }
}
