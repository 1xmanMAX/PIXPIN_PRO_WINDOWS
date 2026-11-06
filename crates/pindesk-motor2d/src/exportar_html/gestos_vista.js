// **Los gestos de la vista, iguales en todas las páginas que exporta PinDesk** (el lienzo, la
// tabla y el plano). Este fichero es el mismo en `pindesk-motor2d/src/exportar_html/` y en
// `pindesk-cad/src/`: se copia tal cual y una prueba mira que no se separen.
//
// - **Dos dedos en el trackpad mueven la vista en cualquier dirección**, círculos incluidos:
//   la rueda llega con `deltaX` y `deltaY` a la vez y se usan los dos. La vista sigue a los
//   dedos (lo de debajo se desplaza `-delta`).
// - **Pellizcar es acercar**, y solo acercar. Chrome, Edge y Firefox lo mandan como una rueda
//   con `ctrlKey` (en Windows y en macOS); Safari, como `gesturechange` con su `scale`. Se
//   acerca hacia el cursor con `exp(-deltaY·K)`, que es suave y simétrico (ir y volver deja la
//   vista donde estaba), y con tope por aviso para que una rueda con Ctrl no dé saltos.
// - **Rueda del ratón: acerca hacia el cursor**, suave (cada muesca se reparte en unos
//   fotogramas) y con los topes de cada visor. Con Mayúsculas mueve de lado y con Alt en
//   vertical, para quien no pueda arrastrar.
// - **Ratón o trackpad** ([pareceRaton]): `deltaMode` de líneas o páginas es ratón; un `deltaX`
//   es trackpad; en píxeles, la rueda da saltos grandes (|deltaY| ≥ 50: 100, 120, 53…, o 66,7
//   con la pantalla al 150 %) o `wheelDeltaY` múltiplo de 120, y el trackpad da poquito y con
//   decimales. Lo que diga el primer aviso vale para toda la ráfaga ([RAFAGA] ms sin avisos la
//   cierran), para que una ráfaga no cambie de idea a mitad: la inercia del trackpad sigue
//   moviendo aunque acabe con saltos grandes.
// - `deltaMode` 1 (líneas, Firefox con ratón) y 2 (páginas) se pasan a píxeles.
// - **Pantalla táctil**: un dedo arrastra y dos pellizcan y mueven a la vez, con Pointer
//   Events, si el visor no lleva ya los suyos (`dedos:true`). El visor que los lleva usa
//   [pellizco] para hacer la misma cuenta.
// - La página nunca se desplaza ni se amplía por su cuenta: `touch-action:none` y
//   `overscroll-behavior:none` en la caja, y `preventDefault` en la rueda (`passive:false`) y
//   en los `gesture*` de Safari.
// - **Una vista por fotograma**: los avisos se suman y se aplican en `requestAnimationFrame`.
var GestosVista=(function(){
"use strict";
var LINEA=40;          // píxeles por línea (deltaMode 1)
var PAGINA=800;        // píxeles por página (deltaMode 2) si la caja no dice su alto
var K_ZOOM=0.01;       // cuánto acerca cada píxel de rueda con Ctrl
var TOPE_ZOOM=25;      // píxeles de rueda como mucho por aviso: ×1,28
var SALTO=250;         // lo más que se mueve de verdad el centro de dos dedos entre dos avisos
var RAFAGA=200;        // ms sin avisos de rueda que cierran una ráfaga
var MUESCA=50;         // |deltaY| en píxeles desde el que una rueda sin deltaX es de ratón
var SUAVE=0.35;        // la parte del acercamiento del ratón que se da en cada fotograma
/** Un delta de rueda en píxeles, sea cual sea su `deltaMode`. */
function enPixeles(d,modo,pagina){
  d=+d||0;
  if(modo===1) return d*LINEA;
  if(modo===2) return d*(pagina||PAGINA);
  return d;
}
/** ¿Este aviso de rueda, solo, parece de la rueda de un ratón (y no de un trackpad)? */
function pareceRaton(e){
  if(e.deltaMode) return true;
  var dx=+e.deltaX||0, dy=+e.deltaY||0;
  if(dx||!dy) return false;
  if(Math.abs(dy)>=MUESCA) return true;
  var w=+e.wheelDeltaY||0;
  return dy===Math.round(dy)&&w!==0&&w%120===0;
}
/**
 * Lo que recuerda la ráfaga: `clasificar(e,t)` dice si el aviso es de ratón, con lo que dijo
 * el primero de su ráfaga. Solo cambia a ratón a mitad si llega en líneas o páginas, que eso
 * un trackpad no lo manda.
 */
function rafaga(){
  var hasta=-Infinity, raton=false;
  return function(e,t){
    if(t>hasta) raton=pareceRaton(e);
    else if(e.deltaMode) raton=true;
    hasta=t+RAFAGA;
    return raton;
  };
}
/**
 * Qué pide una rueda: `{zoom:f}` (f>1 acerca) con Ctrl o ⌘ —el pellizco del trackpad— o con
 * la rueda del ratón (`raton`; entonces `suave:true`), o `{dx,dy}`, cuánto se desplaza lo de
 * debajo en píxeles de pantalla.
 */
function deRueda(e,pagina,raton){
  var dx=enPixeles(e.deltaX,e.deltaMode,pagina), dy=enPixeles(e.deltaY,e.deltaMode,pagina);
  var zoom=e.ctrlKey||e.metaKey;
  if(!zoom&&raton){
    // Mayúsculas: de lado (si el navegador no lo ha girado ya); Alt: en vertical.
    if(e.shiftKey){ if(!dx){ dx=dy; dy=0; } return {dx:0-dx,dy:0-dy}; }
    if(e.altKey) return {dx:0-dx,dy:0-dy};
    if(!dy){ dy=dx; }
    var t=Math.max(-TOPE_ZOOM,Math.min(TOPE_ZOOM,dy));
    return {zoom:Math.exp(-t*K_ZOOM),suave:true};
  }
  if(zoom){
    var t2=Math.max(-TOPE_ZOOM,Math.min(TOPE_ZOOM,dy));
    return {zoom:Math.exp(-t2*K_ZOOM)};
  }
  return {dx:0-dx,dy:0-dy};   // 0-d y no -d: sin «-0»
}
/**
 * Acercar `f` veces alrededor del punto `(cx,cy)` de una vista `pantalla = mundo·z + (x,y)`:
 * lo que hay debajo del punto no se mueve. `min`/`max` limitan `z`.
 */
function ampliarEn(v,f,cx,cy,min,max){
  var z=v.z*f;
  if(min!=null) z=Math.max(min,z);
  if(max!=null) z=Math.min(max,z);
  var k=z/v.z;
  return {x:cx-(cx-v.x)*k, y:cy-(cy-v.y)*k, z:z};
}
/**
 * Dos dedos que pasan de `(a0,b0)` a `(a1,b1)`: cuánto acercar (`f`), alrededor de qué punto
 * (`cx,cy`, el centro de ahora) y cuánto se ha movido el centro (`dx,dy`). `null` si la
 * distancia salta a más del doble o a menos de la mitad entre dos avisos, o el centro salta
 * más de [SALTO] píxeles: eso no es un pellizco, es la palma apoyada.
 */
function pellizco(a0,b0,a1,b1){
  var d0=Math.hypot(a0.x-b0.x,a0.y-b0.y), d1=Math.hypot(a1.x-b1.x,a1.y-b1.y);
  if(!(d0>0)||!(d1>0)) return null;
  var f=d1/d0;
  if(f<0.5||f>2) return null;
  var cx=(a1.x+b1.x)/2, cy=(a1.y+b1.y)/2, dx=cx-(a0.x+b0.x)/2, dy=cy-(a0.y+b0.y)/2;
  if(Math.hypot(dx,dy)>SALTO) return null;
  return {f:f, cx:cx, cy:cy, dx:dx, dy:dy};
}
/**
 * Engancha los gestos a `caja`. `o.mover(dx,dy)` desplaza lo de debajo en píxeles de pantalla;
 * `o.ampliar(f,x,y)` acerca `f` veces (f>1 acerca) alrededor de `(x,y)` en coordenadas de
 * ventana (`clientX/Y`). Con `o.dedos`, también el ratón y los dedos (Pointer Events), y
 * `o.agarrado(b)` avisa de que se está arrastrando. `o.activo()` puede decir que no.
 */
function conectar(caja,o){
  var pend={dx:0,dy:0,f:1,x:0,y:0}, cuadro=0;
  // Lo que falta por acercar de la rueda del ratón, en logaritmo, y hacia dónde.
  var resto=0, rx=0, ry=0;
  var raf=window.requestAnimationFrame?function(fn){return requestAnimationFrame(fn);}:function(fn){return setTimeout(fn,16);};
  var reloj=typeof performance!=='undefined'&&performance.now?function(){return performance.now();}:function(){return Date.now();};
  var clasificar=rafaga();
  function volcar(){
    cuadro=0;
    var p=pend; pend={dx:0,dy:0,f:1,x:p.x,y:p.y};
    if(p.dx||p.dy) o.mover(p.dx,p.dy);
    if(p.f!==1) o.ampliar(p.f,p.x,p.y);
    if(resto){
      var paso=Math.abs(resto)<0.002?resto:resto*SUAVE;
      resto-=paso;
      o.ampliar(Math.exp(paso),rx,ry);
      if(resto) pedir();
    }
  }
  function pedir(){ if(!cuadro) cuadro=raf(volcar); }
  function mover(dx,dy){ pend.dx+=dx; pend.dy+=dy; pedir(); }
  function ampliar(f,x,y){ if(!(f>0)||!isFinite(f)) return; pend.f*=f; pend.x=x; pend.y=y; pedir(); }
  function ampliarSuave(f,x,y){ if(!(f>0)||!isFinite(f)) return; resto+=Math.log(f); rx=x; ry=y; pedir(); }
  function activo(){ return !o.activo||o.activo(); }
  function centro(){ var r=caja.getBoundingClientRect(); return {x:r.left+r.width/2, y:r.top+r.height/2}; }
  caja.style.touchAction='none';
  caja.style.overscrollBehavior='none';
  if(document.documentElement) document.documentElement.style.overscrollBehavior='none';
  // Safari manda el pellizco del trackpad como gesto; mientras dura, la rueda con Ctrl que
  // pudiera llegar a la vez no cuenta dos veces.
  var enGesto=false, escala=1, tactiles=new Set();
  caja.addEventListener('wheel',function(e){
    e.preventDefault();
    if(!activo()) return;
    var r=deRueda(e,caja.clientHeight,clasificar(e,reloj()));
    if(r.zoom!==undefined){ if(!enGesto)(r.suave?ampliarSuave:ampliar)(r.zoom,e.clientX,e.clientY); }
    else mover(r.dx,r.dy);
  },{passive:false});
  caja.addEventListener('gesturestart',function(e){ e.preventDefault(); enGesto=true; escala=e.scale||1; },{passive:false});
  caja.addEventListener('gesturechange',function(e){
    e.preventDefault();
    var s=e.scale||1, f=s/escala; escala=s;
    // En el iPad el pellizco de la pantalla también llega como gesto: de ese se encargan los
    // dedos, y si se contara aquí acercaría el doble.
    if(tactiles.size||!activo()) return;
    var c=e.clientX!=null&&(e.clientX||e.clientY)?{x:e.clientX,y:e.clientY}:centro();
    ampliar(f,c.x,c.y);
  },{passive:false});
  caja.addEventListener('gestureend',function(e){ e.preventDefault(); enGesto=false; escala=1; },{passive:false});
  function quitarTactil(e){ tactiles.delete(e.pointerId); }
  caja.addEventListener('pointerdown',function(e){ if(e.pointerType==='touch') tactiles.add(e.pointerId); },true);
  caja.addEventListener('pointerup',quitarTactil,true);
  caja.addEventListener('pointercancel',quitarTactil,true);
  if(o.dedos){
    var dedos=new Map();
    caja.addEventListener('pointerdown',function(e){
      if(!activo()) return;
      try{ caja.setPointerCapture(e.pointerId); }catch(x){}
      dedos.set(e.pointerId,{x:e.clientX,y:e.clientY});
      if(o.agarrado) o.agarrado(true);
    });
    caja.addEventListener('pointermove',function(e){
      var d=dedos.get(e.pointerId); if(!d) return;
      var ahora={x:e.clientX,y:e.clientY};
      if(dedos.size===2){
        var otro=null; dedos.forEach(function(v,k){ if(k!==e.pointerId) otro=v; });
        var p=pellizco(d,otro,ahora,otro);
        if(p){ mover(p.dx,p.dy); ampliar(p.f,p.cx,p.cy); }
      } else if(dedos.size===1) mover(ahora.x-d.x,ahora.y-d.y);
      dedos.set(e.pointerId,ahora);
    });
    var soltar=function(e){
      if(!dedos.delete(e.pointerId)) return;
      if(!dedos.size&&o.agarrado) o.agarrado(false);
    };
    caja.addEventListener('pointerup',soltar);
    caja.addEventListener('pointercancel',soltar);
  }
  return {mover:mover, ampliar:ampliar};
}
return {LINEA:LINEA, K_ZOOM:K_ZOOM, TOPE_ZOOM:TOPE_ZOOM, SALTO:SALTO, RAFAGA:RAFAGA, MUESCA:MUESCA,
  enPixeles:enPixeles, pareceRaton:pareceRaton, rafaga:rafaga, deRueda:deRueda,
  ampliarEn:ampliarEn, pellizco:pellizco, conectar:conectar};
})();
