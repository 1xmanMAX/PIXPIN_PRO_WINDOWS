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
// - **Rueda del ratón**: mueve en vertical como en un documento; con Mayúsculas, en
//   horizontal; con Ctrl (o ⌘), acerca. No se intenta adivinar si es ratón o trackpad: es lo
//   que más falla, y así el trackpad siempre se comporta igual.
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
/** Un delta de rueda en píxeles, sea cual sea su `deltaMode`. */
function enPixeles(d,modo,pagina){
  d=+d||0;
  if(modo===1) return d*LINEA;
  if(modo===2) return d*(pagina||PAGINA);
  return d;
}
/**
 * Qué pide una rueda: `{zoom:f}` (f>1 acerca) con Ctrl o ⌘ —el pellizco del trackpad—, o
 * `{dx,dy}`, cuánto se desplaza lo de debajo en píxeles de pantalla.
 */
function deRueda(e,pagina){
  var dx=enPixeles(e.deltaX,e.deltaMode,pagina), dy=enPixeles(e.deltaY,e.deltaMode,pagina);
  if(e.ctrlKey||e.metaKey){
    var t=Math.max(-TOPE_ZOOM,Math.min(TOPE_ZOOM,dy));
    return {zoom:Math.exp(-t*K_ZOOM)};
  }
  // Mayúsculas con una rueda que solo sabe ir arriba y abajo: de lado.
  if(e.shiftKey&&!dx){ dx=dy; dy=0; }
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
  var raf=window.requestAnimationFrame?function(fn){return requestAnimationFrame(fn);}:function(fn){return setTimeout(fn,16);};
  function volcar(){
    cuadro=0;
    var p=pend; pend={dx:0,dy:0,f:1,x:p.x,y:p.y};
    if(p.dx||p.dy) o.mover(p.dx,p.dy);
    if(p.f!==1) o.ampliar(p.f,p.x,p.y);
  }
  function pedir(){ if(!cuadro) cuadro=raf(volcar); }
  function mover(dx,dy){ pend.dx+=dx; pend.dy+=dy; pedir(); }
  function ampliar(f,x,y){ if(!(f>0)||!isFinite(f)) return; pend.f*=f; pend.x=x; pend.y=y; pedir(); }
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
    var r=deRueda(e,caja.clientHeight);
    if(r.zoom!==undefined){ if(!enGesto) ampliar(r.zoom,e.clientX,e.clientY); }
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
return {LINEA:LINEA, K_ZOOM:K_ZOOM, TOPE_ZOOM:TOPE_ZOOM, SALTO:SALTO, enPixeles:enPixeles, deRueda:deRueda,
  ampliarEn:ampliarEn, pellizco:pellizco, conectar:conectar};
})();
