// **Las hojas del PDF que vinieron como lineas** (`pixpin_pdf::plano_web`, puerto de
// `PlanoWeb` del movil): cada una llega vacia —su SVG de la clase plano-hoja y su JSON al lado— y
// aqui se rellena con sus caminos, sus rotulos y sus fotos. Va en SVG y no en un lienzo porque
// la hoja del documento se amplia con `transform`: un SVG se vuelve a dibujar nitido a
// cualquier aumento y se imprime como lineas, y un lienzo se estiraria como una foto.
//
// Se rellena **despues de cargar** y no al leer el guion: el armazon guarda al empezar la
// pagina tal como llego (su PLANTILLA, para «Guardar»), y asi lo guardado sigue llevando el
// paquete comprimido y no los megas de caminos ya desplegados.
(function(){
"use strict";
// El descompresor (DEFLATE, RFC 1951) de `VisorPlano.INFLAR` del movil, tal cual.
function inflar(datos){
var pos=0, bit=0, salida=new Uint8Array(1<<18), n=0;
function sitio(k){ if(n+k>salida.length){ var g=new Uint8Array(Math.max(salida.length*2,n+k)); g.set(salida); salida=g; } }
function bits(k){ var v=0,i=0; while(i<k){ v|=((datos[pos]>>bit)&1)<<i; bit++; if(bit===8){bit=0;pos++;} i++; } return v; }
function mirar(k){ var v=0,p=pos,b=bit,i=0; while(i<k){ v|=((p<datos.length?datos[p]:0)>>b&1)<<i; b++; if(b===8){b=0;p++;} i++; } return v; }
function saltar(k){ bit+=k; pos+=bit>>3; bit&=7; }
function tabla(largos,max){
  var t=new Int32Array(1<<max), cuenta=new Int32Array(max+1), i, l;
  for(i=0;i<largos.length;i++) cuenta[largos[i]]++;
  cuenta[0]=0;
  var codigo=0, siguiente=new Int32Array(max+2);
  for(l=1;l<=max;l++){ codigo=(codigo+cuenta[l-1])<<1; siguiente[l]=codigo; }
  for(i=0;i<largos.length;i++){
    l=largos[i]; if(!l) continue;
    var c=siguiente[l]++, r=0, j;
    for(j=0;j<l;j++) r=(r<<1)|((c>>j)&1);
    for(j=r;j<(1<<max);j+=(1<<l)) t[j]=(i<<4)|l;
  }
  t.max=max;
  return t;
}
function simbolo(t){ var v=t[mirar(t.max)]; if(!v) throw new Error('deflate'); saltar(v&15); return v>>4; }
var LARGOS=[3,4,5,6,7,8,9,10,11,13,15,17,19,23,27,31,35,43,51,59,67,83,99,115,131,163,195,227,258];
var EXTRA_L=[0,0,0,0,0,0,0,0,1,1,1,1,2,2,2,2,3,3,3,3,4,4,4,4,5,5,5,5,0];
var DIST=[1,2,3,4,5,7,9,13,17,25,33,49,65,97,129,193,257,385,513,769,1025,1537,2049,3073,4097,6145,8193,12289,16385,24577];
var EXTRA_D=[0,0,0,0,1,1,2,2,3,3,4,4,5,5,6,6,7,7,8,8,9,9,10,10,11,11,12,12,13,13];
var ORDEN=[16,17,18,0,8,7,9,6,10,5,11,4,12,3,13,2,14,1,15];
var fijoL=null, fijoD=null, i;
for(;;){
  var ultimo=bits(1), tipo=bits(2), tl, td;
  if(tipo===0){
    if(bit){bit=0;pos++;}
    var largo=datos[pos]|(datos[pos+1]<<8); pos+=4;
    sitio(largo); salida.set(datos.subarray(pos,pos+largo),n); n+=largo; pos+=largo;
  } else {
    if(tipo===1){
      if(!fijoL){
        var ll=new Uint8Array(288);
        for(i=0;i<144;i++)ll[i]=8; for(;i<256;i++)ll[i]=9; for(;i<280;i++)ll[i]=7; for(;i<288;i++)ll[i]=8;
        fijoL=tabla(ll,9);
        var dd=new Uint8Array(30); for(i=0;i<30;i++)dd[i]=5;
        fijoD=tabla(dd,5);
      }
      tl=fijoL; td=fijoD;
    } else if(tipo===2){
      var hlit=bits(5)+257, hdist=bits(5)+1, hclen=bits(4)+4;
      var clen=new Uint8Array(19);
      for(i=0;i<hclen;i++) clen[ORDEN[i]]=bits(3);
      var tc=tabla(clen,7), todos=new Uint8Array(hlit+hdist), k=0, r;
      while(k<todos.length){
        var s=simbolo(tc);
        if(s<16) todos[k++]=s;
        else if(s===16){ r=3+bits(2); var v2=todos[k-1]; while(r--) todos[k++]=v2; }
        else if(s===17){ r=3+bits(3); while(r--) todos[k++]=0; }
        else { r=11+bits(7); while(r--) todos[k++]=0; }
      }
      var maxL=0, maxD=0;
      for(i=0;i<hlit;i++) if(todos[i]>maxL)maxL=todos[i];
      for(i=hlit;i<todos.length;i++) if(todos[i]>maxD)maxD=todos[i];
      tl=tabla(todos.subarray(0,hlit),maxL||1);
      td=tabla(todos.subarray(hlit),maxD||1);
    } else throw new Error('deflate');
    for(;;){
      var s2=simbolo(tl);
      if(s2===256) break;
      if(s2<256){ sitio(1); salida[n++]=s2; }
      else {
        var li=s2-257, cuanto=LARGOS[li]+bits(EXTRA_L[li]);
        var ds=simbolo(td), dist=DIST[ds]+bits(EXTRA_D[ds]);
        sitio(cuanto);
        var desde=n-dist;
        for(var q=0;q<cuanto;q++) salida[n++]=salida[desde+q];
      }
    }
  }
  if(ultimo) break;
}
return salida.subarray(0,n);
}
function deBase64(s){
  var bin=atob(s), out=new Uint8Array(bin.length);
  for(var i=0;i<bin.length;i++) out[i]=bin.charCodeAt(i);
  return out;
}
var NS='http://www.w3.org/2000/svg';
function el(tipo, attrs){
  var e=document.createElementNS(NS,tipo);
  for(var k in attrs) if(attrs.hasOwnProperty(k)) e.setAttribute(k,attrs[k]);
  return e;
}
// Un numero para `d` con los decimales justos: son millones y cada byte cuenta.
function nu(v){ return Math.round(v*100)/100; }
function familia(f){
  // `sans-serif-condensed` es el nombre del movil; el navegador necesita una pila de verdad.
  if(f==='sans-serif-condensed') return '"Arial Narrow","Helvetica Neue Condensed","Liberation Sans Narrow","Roboto Condensed",sans-serif';
  return f||'sans-serif';
}
// La foto con su mascara: el gris de la mascara pasa a ser el alfa, una sola vez.
function conMascara(u, k, listo){
  var img=new Image(), m=new Image(), hay=0;
  function juntar(){
    if(++hay<2) return;
    try{
      var w=img.naturalWidth, h=img.naturalHeight;
      if(!w||!h||m.naturalWidth!==w||m.naturalHeight!==h){ listo(u); return; }
      var c=document.createElement('canvas'); c.width=w; c.height=h;
      var g=c.getContext('2d'); g.drawImage(img,0,0);
      var d=g.getImageData(0,0,w,h); g.clearRect(0,0,w,h); g.drawImage(m,0,0);
      var a=g.getImageData(0,0,w,h).data, p=d.data;
      for(var i=0;i<p.length;i+=4) p[i+3]=a[i];
      g.putImageData(d,0,0);
      listo(c.toDataURL('image/png'));
    }catch(e){ listo(u); }
  }
  img.onload=juntar; m.onload=juntar;
  // Sin su mascara la foto taparia el plano con un rectangulo: se deja fuera.
  m.onerror=function(){ listo(null); };
  img.onerror=function(){ listo(null); };
  img.src=u; m.src=k;
}
function rellenar(svg, D){
  var esc=D.e, d=inflar(deBase64(D.datos)), p=0;
  function varint(){ var r=0,s=0,c; do{ c=d[p++]; r|=(c&0x7f)<<s; s+=7; }while(c&0x80); return (r>>>1)^-(r&1); }
  var apagada={};
  (D.capas||[]).forEach(function(c,i){ if(c.v===0) apagada[i]=1; });
  function visible(c){ return !(c>=0&&apagada[c]); }
  var grupo=document.createDocumentFragment();
  // El papel, blanco: un PDF no trae fondo.
  grupo.appendChild(el('rect',{x:0,y:0,width:D.a,height:D.b,fill:'#fff'}));
  // Las fotos debajo de todo: en un plano una foto es el fondo.
  (D.fotos||[]).forEach(function(f){
    var im=(D.imagenes||[])[f.i]; if(!im||!visible(f.c)) return;
    var nodo=el('image',{width:1,height:1,preserveAspectRatio:'none',transform:'matrix('+f.m.join(' ')+')'});
    if(f.o!==undefined) nodo.setAttribute('opacity',f.o);
    function poner(u){ if(u){ nodo.setAttribute('href',u); } else if(nodo.parentNode){ nodo.parentNode.removeChild(nodo); } }
    if(im.k){ if(!im.lista){ im.lista=[]; conMascara(im.u,im.k,function(u){ im.hecha=u||''; im.lista.forEach(function(f2){f2(u);}); }); } if(im.hecha!==undefined) poner(im.hecha||null); else im.lista.push(poner); }
    else poner(im.u);
    grupo.appendChild(nodo);
  });
  var x=0, y=0;
  (D.brochas||[]).forEach(function(b){
    var partes=[];
    for(var j=0;j<b.n;j++){
      var op=d[p++];
      if(op===1){ x+=varint(); y+=varint(); partes.push('M'+nu(x*esc)+' '+nu(y*esc)); }
      else if(op===2){ x+=varint(); y+=varint(); partes.push('L'+nu(x*esc)+' '+nu(y*esc)); }
      else if(op===3){
        var c=[];
        for(var k=0;k<3;k++){ x+=varint(); y+=varint(); c.push(nu(x*esc)+' '+nu(y*esc)); }
        partes.push('C'+c.join(' '));
      }
      else if(op===4) partes.push('Z');
    }
    if(!visible(b.c)||!partes.length) return;
    var a={d:partes.join('')};
    if(b.r){ a.fill=b.t; a.stroke='none'; if(b.p) a['fill-rule']='evenodd'; }
    else {
      a.fill='none'; a.stroke=b.t; a['stroke-linecap']='round'; a['stroke-linejoin']='round';
      // Un pelo del PDF (`0 w`) es «lo mas fino que se pueda», y **ninguna raya baja de un
      // pixel** (`VisorPlano`: max(grosor, 1 px)): las de menos de un pixel de la pagina van a
      // un pixel de pantalla a cualquier aumento. Sin esto, los bordes rojos de las calzadas de
      // un plano de AutoCAD (0,25 pt) salian desvaidos, casi rosas, al lado de lo que pinta
      // Windows.
      if(!(b.g>=1)){ a['stroke-width']=1; a['vector-effect']='non-scaling-stroke'; }
      else a['stroke-width']=b.g;
      if(b.d) a['stroke-dasharray']=b.d.join(' ');
    }
    if(b.o!==undefined) a.opacity=b.o;
    grupo.appendChild(el('path',a));
  });
  // Los rotulos con la letra del aparato, estirados hasta lo que ocupaban en el PDF: asi una
  // cota cae en su casilla aunque la letra no sea la misma.
  (D.textos||[]).forEach(function(t){
    if(!visible(t.c)) return;
    var n=el('text',{transform:'matrix('+t.m.join(' ')+') scale(0.01)','font-size':100,
      'font-family':familia(t.f),fill:t.t,x:0,y:0});
    if(t.w>0){ n.setAttribute('textLength',nu(t.w*100)); n.setAttribute('lengthAdjust','spacingAndGlyphs'); }
    if(t.n) n.setAttribute('font-weight','bold');
    if(t.i) n.setAttribute('font-style','italic');
    if(t.o!==undefined) n.setAttribute('opacity',t.o);
    n.textContent=t.s;
    grupo.appendChild(n);
  });
  svg.appendChild(grupo);
}
function todas(){
  var datos=document.querySelectorAll('script.plano-datos');
  for(var i=0;i<datos.length;i++){
    var s=datos[i], svg=document.getElementById(s.getAttribute('data-hoja'));
    if(!svg||svg.firstChild) continue;
    try{ rellenar(svg, JSON.parse(s.textContent)); }
    catch(e){ svg.setAttribute('data-roto','1'); }
  }
}
if(document.readyState==='loading') document.addEventListener('DOMContentLoaded', function(){ setTimeout(todas,0); });
else setTimeout(todas,0);
})();
