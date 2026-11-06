# PinDesk — chaînes en français.

app-nombre = PinDesk


resultado-copiar = Copier
resultado-guardar-como = Enregistrer sous…
resultado-guardar = Enregistrer
resultado-descartar = Ignorer

error-otra-instancia = PinDesk est déjà en cours d’exécution.
error-atajo-ocupado = Impossible d’enregistrer { $atajo } : une autre application l’utilise.

captura-guardada = Capture enregistrée dans { $ruta }
captura-fallo = Impossible de capturer l’écran : { $motivo }

barra-copiar = Copier
barra-guardar = Enregistrer
barra-guardar-como = Enregistrer sous…
barra-descartar = Ignorer

pin-no-encontrado = Introuvable

pin-copiar = Copier
pin-guardar-como = Enregistrer sous…
pin-abrir-ubicacion = Ouvrir l’emplacement du fichier
pin-tamano-original = Taille d’origine
pin-grupo = Groupe
pin-sin-grupo = Aucun groupe
pin-color-rojo = Rouge
pin-color-naranja = Orange
pin-color-ambar = Ambre
pin-color-verde = Vert
pin-color-cian = Cyan
pin-color-azul = Bleu
pin-color-violeta = Violet
pin-color-rosa = Rose
pin-ocultar-grupo = Masquer ce groupe
pin-cerrar = Fermer
pin-eliminar = Supprimer de la bibliothèque…
pin-eliminar-confirmar = L’élément sera définitivement supprimé de la bibliothèque. Voulez-vous continuer ?
grupos-ocultos = Groupes masqués

capa-guardar-titulo = Annotation de l’écran
capa-guardar-pregunta = Conserver le dessin comme épingle ?
pin-reproducir = Lire
pin-pausar = Pause
pin-sonido = Son
barra-todo = Tout sélectionner
pin-sin-codec = aucun codec vidéo
comando-capturar-region = Capturer
comando-capturar-y-copiar = Capturer et copier
comando-capturar-con-scroll = Capture défilante
comando-cuentagotas = Pipette
comando-pinear = Rogner et épingler
comando-pinear-en-vivo = Épingle en direct (suivre une zone en temps réel)
comando-pinear-portapapeles = Épingler le presse-papiers
comando-anotar = Annoter l’écran
comando-anotar-congelada = Annoter une capture
comando-abrir-chat = Ouvrir le chat des projets
comando-abrir-ajustes = Paramètres
comando-salir = Quitter
comando-alternar-pines = Masquer ou afficher les épingles
comando-restaurar-ultimo-pin = Rouvrir la dernière épingle fermée
comando-cerrar-todos-los-pines = Fermer toutes les épingles
comando-ventana-encima = Garder au premier plan la fenêtre sous le pointeur
comando-copiar-texto = Copier le texte d’une zone
comando-capturar-y-anotar = Capturer et annoter
comando-capturar-ultima-region = Répéter la dernière capture
comando-grabar-gif = Enregistrer une zone en GIF

# The GIF recording bar (P5b).
grabar-empezar = Enregistrer
grabar-parar = Arrêter
grabar-pausa = Pause
grabar-seguir = Reprendre
grabar-cerrar = Fermer
grabar-por-segundo = { $ritmo }/s
grabar-cuenta-atras = { $segundos }...

# The editor that opens when recording stops (P5b.4).
editor-ver = Lire
editor-pausar = Pause
editor-anterior = Précédente
editor-siguiente = Suivante
editor-velocidad = { $veces }x
editor-guardar = Enregistrer
editor-guardado-rapido = Rapide
editor-copiar = Copier
editor-descartar = Ignorer

# Click-through pins (P1.4).
comando-alternar-paso-de-clics = Laisser passer les clics à travers les épingles
pin-dejar-pasar-clic = Laisser passer les clics

# Muting the shortcuts for a while (P1.7).
comando-silenciar-atajos = Désactiver les raccourcis
bandeja-silenciada = PinDesk — raccourcis désactivés
aviso-atajos-silenciados = Raccourcis désactivés
aviso-atajos-silenciados-detalle = Réactivez-les depuis ce même menu.
aviso-atajos-activos = Raccourcis actifs
aviso-atajos-activos-detalle = { $cuantos } raccourcis répondent à nouveau.

# Delayed capture (P2.1).
comando-capturar-con-retardo = Capturer après un compte à rebours
cuenta-atras-cancelar = Échap pour arrêter

# Pinning the Explorer selection (P1.6).
comando-pinear-seleccion = Épingler la sélection de l’Explorateur

# The settings window (P6).
ajustes-pestana-dibujo = Dessin
ajustes-iman = Aimanter aux points remarquables
ajustes-iman-esquinas = Coins et extrémités
ajustes-iman-medios = Milieux
ajustes-iman-centros = Centres
ajustes-iman-radio = Distance d’accroche (px)
ajustes-idioma = Langue
ajustes-idioma-sistema = Comme Windows
ajustes-arranque = Démarrer avec Windows
ajustes-nivel-auto = Automatique
ajustes-nivel-completo = Complet
ajustes-nivel-ligero = Léger
ajustes-pdf-aligerar = Alléger les PDF ajoutés au chat
ajustes-pdf-nivel-sin-perdida = Exact
ajustes-pdf-nivel-equilibrado = Moyen
ajustes-pdf-nivel-pequeno = Petit
ajustes-pdf-nivel-extremo = Max
ajustes-sin-atajo = Aucun raccourci
ajustes-pulsa-combinacion = Appuyez sur la combinaison…
ajustes-choca = en double

# Reading the text of a pin (P4.2).
pin-copiar-texto = Copier le texte de l’image

# Opening an image pin in the editor canvas.
pin-abrir-en-lienzo = Ouvrir dans le canevas
pin-congelar = Figer en image
pin-manejar = Contrôler à distance
pin-dejar-de-manejar = Arrêter le contrôle à distance

# Pages of a pinned PDF.
pin-pagina-siguiente = Page suivante
pin-pagina-anterior = Page précédente
pin-extraer-pagina = Extraire cette page en épingle
pin-extraer-todas = Extraire toutes les pages
aviso-paginas-extraidas = Pages extraites
aviso-paginas-extraidas-detalle = { $hechas } sur { $total }. Les autres ont été omises pour que l’écran reste utilisable.
chat-titulo = Projets
chat-sin-proyectos = Aucun projet pour l’instant
chat-elige-proyecto = Choisissez un projet pour l’afficher
chat-hojas = { $cuantas ->
    [one] 1 feuille
   *[other] { $cuantas } feuilles
  }
chat-sin-mensajes = Rien dans ce projet pour l’instant
chat-clase-imagen = Image
chat-clase-archivo = Fichier
chat-clase-voz = Note vocale
chat-clase-dibujo = Dessin
chat-clase-pagina = Page
chat-clase-proyecto = Projet
chat-clase-miniapp = Mini-app
chat-escribe = Écrivez quelque chose…
# The chat, like PinDesk Android's (guardados/MensajesActivity.kt).
chat-elegidos = { $n ->
    [one] 1 sélectionné
   *[other] { $n } sélectionnés
}
chat-lienzo = Canevas
chat-tabla = Tableau
chat-viene-de = Provient de
chat-copiar = Copier
chat-fijar = Épingler en haut
chat-soltar = Retirer du haut
chat-compartir = Partager…
chat-abrir-con = Ouvrir avec…
abrir-con = Ouvrir avec…
abrir-con-no-existe = Fichier « { $nombre } » introuvable : il a peut-être été déplacé ou supprimé.
abrir-con-fallo = Windows n’a pas pu afficher « Ouvrir avec… » pour ce fichier.
chat-aligerar = Alléger le PDF
chat-renombrar = Renommer
chat-renombrar-barra = Nom :
chat-renombrar-teclas = Entrée enregistre · Échap garde l’ancien
chat-describir = Ajouter une légende
chat-describir-editar = Modifier la légende
chat-describir-barra = Légende :
chat-describir-teclas = Entrée enregistre · Échap laisse tel quel
chat-pinear = Épingler
chat-recordar = Me le rappeler
chat-rescatar = Conserver
chat-solo-la-foto = Afficher uniquement la photo
chat-ver-hilo = Voir les commentaires
chat-reenviar = Transférer
chat-unir = Ajouter au projet
chat-transcribir = Convertir en texte
chat-letra = Paroles ou texte
chat-borrar = Supprimer
chat-borrar-aviso = L’élément sera retiré de la conversation, ainsi que le fichier joint. Continuer ?
chat-biblioteca-audio = Bibliothèque audio
chat-fondo = Fond du chat
chat-escala-puesta = Échelle de l’interface : { $cuanto } %
chat-fondo-mar = Mer
chat-fondo-arena = Sable
chat-fondo-brezo = Bruyère
chat-fondo-bosque = Forêt
chat-fondo-ocaso = Crépuscule
chat-fondo-pizarra = Ardoise
chat-adj-telepronter = Prompteur
chat-adj-pronunciar = Prononciation
chat-reenviado = Transféré
chat-copiado = Copié
chat-compartir-sin-panel = Windows n’a pas ouvert le panneau Partager : c’est dans le presse-papiers, collez-le où vous voulez
chat-pineado = Épinglé
chat-foto-en-la-nota = Photo ajoutée à votre texte : le tout est envoyé en une seule note
chat-sin-archivo = Ce message n’a pas de fichier sur cet ordinateur
chat-sin-origen = Ce canevas n’est pas dans ce projet
chat-sin-proyectos-otros = Aucun autre projet vers lequel l’envoyer
chat-no-hay-unir = Sous Windows, chaque chat est déjà un projet : utilisez Transférer pour l’envoyer vers un autre
chat-ver-dibujo-entero = Afficher tout le dessin
chat-no-se-pudo = Opération impossible : consultez le journal de PinDesk
chat-fecha = { $dia } { $mes }
chat-fecha-con-ano = { $dia } { $mes } { $ano }
chat-mes-1 = janvier
chat-mes-2 = février
chat-mes-3 = mars
chat-mes-4 = avril
chat-mes-5 = mai
chat-mes-6 = juin
chat-mes-7 = juillet
chat-mes-8 = août
chat-mes-9 = septembre
chat-mes-10 = octobre
chat-mes-11 = novembre
chat-mes-12 = décembre
chat-hoy = Aujourd’hui
chat-ayer = Hier
chat-buscar = Rechercher
chat-adjuntar = Joindre
chat-fijado = Message épinglé
chat-sistema-memoria-menu = Mémoire de l’assistant (ouvrir son dossier pour la lire ou la modifier)
info-galeria = Galerie
info-todo = Tout
info-fotos = Photos
info-archivos = Fichiers
info-voz = Voix
info-dibujos = Dessins
info-fijados = Épinglés
info-buzon = Boîte de réception
info-videos = Vidéos
info-enlaces = Liens
info-resumen = { $n ->
    [one] 1 élément
   *[other] { $n } éléments
}  ·  { $codigo }
info-previa-posicion = { $k } sur { $n }
info-vacia = Rien ici pour l’instant
info-mes = { $mes } { $ano }
info-vacia-galeria = Les feuilles du projet apparaîtront ici
info-vacia-fotos = Les photos de ce chat apparaîtront ici
info-vacia-videos = Les vidéos de ce chat apparaîtront ici
info-vacia-archivos = Les fichiers de ce chat apparaîtront ici
info-vacia-enlaces = Les liens de ce chat apparaîtront ici
info-vacia-voz = Les notes vocales apparaîtront ici
info-vacia-dibujos = Les dessins de ce chat apparaîtront ici
info-vacia-fijados = Ce que vous épinglez apparaîtra ici
info-vacia-buzon = La boîte de réception est vide
info-titulo = Infos
adjuntar-imagen = Photo ou vidéo
adjuntar-archivo = Fichier
adjuntar-lienzo = Nouveau canevas
confirmar-pie = Ajouter une légende…
confirmar-cancelar = Annuler
adjuntar-tabla = Nouveau tableau
tabla-nueva = Tableau
chat-nombre-nuevo = Nom du projet…
hoja-volver = Échap pour revenir
menu-foto-lienzo = Ouvrir dans le canevas
menu-foto-pin = L’épingler à nouveau
menu-foto-abrir = Ouvrir avec Windows
recibir-titulo = Recevoir du téléphone
recibir-como = Scannez le code avec PinDesk sur votre téléphone
recibir-esperando = En attente du téléphone…
recibir-hecho = { $cuantos } éléments reçus
recibir-salir = Échap pour fermer
comando-recibir-del-movil = Recevoir du téléphone…
comando-sincronizar = Synchroniser
adjuntar-del-movil = Importer du téléphone…
proyecto-renombrar = Renommer
# The chat photo (25-oct-2026): image, emoji or colour in the circle.
foto-chat-menu = Changer la photo du chat…
foto-chat-titulo = Photo du chat
foto-chat-imagen = Choisir une image…
foto-chat-pegar = Coller l’image
foto-chat-quitar = Par défaut
foto-chat-buscar = Rechercher un emoji
foto-chat-sin-emojis = Aucun emoji ne porte ce nom
foto-chat-fondo = Arrière-plan
foto-chat-recorte = Faites glisser pour déplacer · molette pour zoomer
foto-chat-guardar = Enregistrer
foto-chat-cancelar = Annuler
foto-chat-sin-imagen = Aucune image copiée
foto-chat-error = Impossible d’enregistrer la photo du chat
proyecto-borrar = Supprimer le projet
proyecto-borrar-varios = Supprimer la sélection
proyecto-borrar-aviso = Il disparaîtra de la liste et son dossier ira dans la corbeille de PinDesk. Continuer ?
proyecto-papelera = Projets supprimés…
# Where each project is saved: its chosen folder or the usual place.
ubicacion-cambiar = Changer d’emplacement…
ubicacion-habitual = Revenir à l’emplacement habituel
ubicacion-abrir = Ouvrir le dossier du projet
ubicacion-elegir-titulo = Choisissez où enregistrer le projet
ubicacion-nuevo = Nouveau projet
ubicacion-nuevo-en = Nouveau projet dans un dossier…
ubicacion-no-disponible = Emplacement indisponible
ubicacion-no-disponible-abrir = Le dossier de ce projet n’est pas disponible ({ $ruta }). Connectez le lecteur et réessayez.
ubicacion-movido = Projet enregistré dans { $ruta }
ubicacion-vuelto = Le projet est revenu à l’emplacement habituel
ubicacion-error-red = Les projets ne peuvent être enregistrés que sur un lecteur de cet ordinateur, pas sur un lecteur réseau.
ubicacion-error-no-es-carpeta = Ce dossier n’existe pas sur cet ordinateur.
ubicacion-error-dentro = Ce dossier se trouve dans le projet lui-même ou dans les données de PinDesk.
ubicacion-error-mover = Impossible de déplacer le projet ; il est resté où il était.
ubicacion-error-abrir = Impossible d’ouvrir le dossier du projet.

## Sync (copy of the phone's SincronizarActivity.kt)
sinc-titulo = Synchronisation
sinc-subtitulo = Par Wi-Fi, sans internet.
sinc-este-aparato = Cet appareil
sinc-tus-aparatos = Vos appareils
sinc-sin-grupo = Créez un groupe sur l’un de vos appareils et rejoignez-le depuis les autres avec son code. Seuls les appareils du groupe se synchronisent entre eux.
sinc-crear-grupo = Créer un groupe
sinc-unirme = Rejoindre avec un code
sinc-grupo-vacio = Aucun autre appareil dans le groupe pour l’instant. Ajoutez-en un avec le code ci-dessous.
sinc-disponible = Disponible
sinc-no-encontrado = Introuvable
sinc-sincronizar = Synchroniser
sinc-probar = Tester
sinc-con-todos = Synchroniser avec tous ({ $n })
sinc-con-todos-como = Sans rien demander : reprend ce que vous avez choisi la dernière fois avec chacun.
sinc-si-no-aparece = S’il n’apparaît pas : PinDesk doit être ouvert et sur ce Wi-Fi.
sinc-con-direccion = Se connecter à une adresse
sinc-anadir = Ajouter un autre appareil
sinc-anadir-como = Sur l’autre appareil : Synchronisation → « Rejoindre avec un code ».
sinc-toca-codigo = Cliquez pour voir le code
sinc-qr-como = Scannez-le avec l’appareil photo du téléphone : il ouvre PinDesk et rejoint tout seul. Il se masque dans 10 minutes.
sinc-red-titulo = PinDesk n’a pas l’autorisation réseau
sinc-red-texto = Le pare-feu Windows empêche vos autres appareils d’atteindre PinDesk, c’est pourquoi ils n’apparaissent pas quand vous rejoignez avec le code. Autorisez-le une fois : Windows vous demandera de confirmer en tant qu’administrateur.
sinc-red-boton = Autoriser l’accès réseau
sinc-red-pidiendo = En attente de l’autorisation de Windows…
sinc-red-listo = Accès réseau autorisé
sinc-red-hecho = C’est fait : vos autres appareils peuvent maintenant trouver PinDesk sur ce réseau.
sinc-red-publica = PinDesk est autorisé sur les réseaux privés, mais ce Wi‑Fi est marqué « Public ». Rendez-le privé dans Paramètres → Réseau et Internet → Wi‑Fi → ce réseau.
sinc-red-cancelado = L’autorisation n’a pas été accordée. Vous pouvez réessayer quand vous voulez.
sinc-red-fallo = Windows n’a pas permis d’ajouter l’autorisation (une stratégie de ce PC l’empêche peut-être). Demandez de l’aide à la personne qui gère ce PC.
sinc-salir = Quitter le groupe
sinc-pasar = Envoyer quelque chose à quelqu’un
sinc-pasar-como = Une seule fois, à toute personne ayant PinDesk sur votre Wi-Fi. Pas besoin de groupe.
sinc-recibir = Recevoir
sinc-enviar = Envoyer
sinc-pasar-tambien = Ce qui arrive va dans la liste des projets ou dans « Messages enregistrés ».
sinc-actividad = Activité
sinc-escuchando = Cet ordinateur est joignable à l’adresse { $dir } tant que cette fenêtre est ouverte.
sinc-nombre-titulo = Nom de cet appareil
sinc-nombre-como = C’est ainsi que vos autres appareils le verront.
sinc-unirme-titulo = Rejoindre un groupe
sinc-unirme-como = Saisissez le code affiché sous « Ajouter un autre appareil » sur un appareil du groupe. Cet appareil doit avoir PinDesk ouvert et être sur le même Wi-Fi.
sinc-codigo = Code
sinc-direccion = Adresse (seulement s’il est introuvable)
sinc-direccion-titulo = Se connecter à une adresse
sinc-direccion-como = Saisissez l’adresse affichée sur l’autre appareil, dans cette même fenêtre.
sinc-la-de-este = Celle de cet appareil :
sinc-salir-titulo = Quitter le groupe ?
sinc-salir-como = Cet appareil cessera de se synchroniser avec les autres. Rien n’est supprimé. Si vous le rejoignez à nouveau, la première fois il vous interrogera sur ce qui a changé des deux côtés.
sinc-salir-boton = Quitter
sinc-un-momento = Un instant
sinc-no-se-pudo = Échec
sinc-guardar = Enregistrer
sinc-unirme-boton = Rejoindre
sinc-conectar = Connecter
sinc-cancelar = Annuler
sinc-ocultar = Masquer
sinc-vale = OK
sinc-con-todos-rondas = Sans rien demander, l’un après l’autre et en deux tours, pour que tout le monde finisse avec tout.
sinc-elegir-titulo = Que synchroniser ?
sinc-elegir-con = Avec { $otro }. Ce que vous cochez sera identique des deux côtés.
sinc-en-los-dos = Des deux côtés
sinc-solo-en-uno = D’un seul côté
sinc-este-aparato-n = Cet appareil · { $n }
sinc-elegir-como = Ce qui a changé des deux côtés est fusionné : forme par forme, cellule par cellule, paragraphe par paragraphe. Si les deux ont touché la même chose, la dernière modification l’emporte et la précédente reste dans « Sauvegardes ».
sinc-todo = Tout
sinc-nada = Rien
sinc-sincronizar-n = Synchroniser { $n }
sinc-sincronizando = Synchronisation
sincro-borrado-alli-vuelve = Supprimé sur { $otro } · sera renvoyé
sincro-borrado-alli-se-borra = Supprimé sur { $otro } · SERA SUPPRIMÉ ICI (une sauvegarde est conservée)
sincro-borrado-alli-se-queda = Supprimé sur { $otro } · décoché : reste ici
sincro-se-borran-aqui = Sera supprimé ICI, car il a été supprimé sur { $otro } : { $lista }. Décochez-le pour le garder, ou choisissez « Le mien l’emporte » pour le renvoyer.
sincro-se-borran-alli = Sera supprimé sur { $otro }, car vous l’avez supprimé ici : { $lista }.
sincro-juntar = Fusionner
sincro-lo-mio-manda = Le mien l’emporte
sincro-lo-mio-manda-como = Dans un seul sens : ce qu’a { $yo } écrase { $otro }. Rien n’est supprimé ni modifié ici ; là-bas, ce qui diffère devient comme ici et ce qui y avait été supprimé revient. Ce que seul { $otro } possède est conservé. Une sauvegarde de ce qui est écrasé est conservée.
sincro-mandar-n = Envoyer { $n }

## Receive over Wi-Fi (copy of the phone's RecibirActivity.kt)
rw-titulo = Recevoir par Wi-Fi
rw-misma-wifi = Vous devez être sur le même Wi-Fi que l’expéditeur (ou connecté à son point d’accès).
rw-o-escribe = ou saisissez le code
rw-pista-pega = L’ordinateur n’a pas de caméra : saisissez les six chiffres ou collez le texte de son QR avec Ctrl+V.
rw-recibir = Recevoir
rw-prefieres = Vous préférez être scanné ? Affichez votre propre code et laissez l’expéditeur le viser avec sa caméra.
rw-ensenar = Afficher mon code pour qu’on m’envoie
rw-que-te-escaneen = Faites scanner ceci
rw-esperando-mio = En attente d’un envoi. Vous pouvez aussi dicter ce code pour « Envoyer → Scanner le destinataire ».
rw-buscando = Recherche de l’expéditeur…
rw-conectando = Connexion…
rw-no-encontrado = Personne n’envoie avec ce code. Vérifiez le code et que vous êtes tous deux sur le même Wi-Fi (ou sur le point d’accès de l’un).
rw-codigo-malo = Ce code n’est pas le bon. Vérifiez-le.
rw-se-corto = Interrompu : { $motivo }
rw-quiere = { $de } veut vous envoyer
rw-aparato = Appareil { $codigo }
rw-proyecto = Projet « { $nombre } »
rw-lienzo = Canevas « { $nombre } »
rw-nada-coincide = Rien de tout ceci ne correspond à ce que vous avez : cela arrive comme nouveau, sans toucher à vos données.
rw-no-gracias = Non merci
rw-aceptar = Accepter
rw-recibiendo = Réception depuis { $de }
rw-de = { $hechos } sur { $total }
rw-recibido = Reçu de { $de }
rw-no-guardado = C’est arrivé, mais impossible de l’enregistrer.
rw-en-proyectos = { $nombre } : dans la liste des projets
rw-en-guardados = { $nombre } : dans « Messages enregistrés »
rw-en-carpeta = { $nombre } : uniquement dans le dossier « recibidos »
rw-no-se-pudo = Échec
rw-probar-otra-vez = Réessayer
rw-cerrar = Fermer

## Send over Wi-Fi (copy of the phone's EnviarActivity.kt)
ew-titulo = Envoyer par Wi-Fi
ew-que = Que voulez-vous envoyer ?
ew-que-como = Sur le téléphone, vous choisissez au moment de partager ; ici, des fichiers de cet ordinateur ou un projet entier du chat.
ew-archivos = Fichiers
ew-un-proyecto = Un projet
ew-elige-proyecto = Choisissez le projet
ew-sin-proyectos = Aucun projet pour l’instant.
ew-preparando = Préparation…
ew-no-preparado = Impossible de préparer ce que vous vouliez envoyer.
ew-vas-a-enviar = Vous envoyez
ew-vas-a-enviar-n = Vous envoyez { $n } éléments
ew-y-mas = et { $n } de plus
ew-un-aparato = Un appareil
ew-n-aparatos = { $n } appareils
ew-pidiendo = Veut recevoir ce que vous envoyez
ew-enviando = Envoi · { $hechos } sur { $total }
ew-esperando-acepte = En attente d’acceptation…
ew-enviado = Envoyé
ew-no-acepto = Refusé
ew-no-aprobado = Non approuvé
ew-no = Non
ew-enviar = Envoyer
ew-enviar-a-todos = Envoyer à tous ceux qui attendent
ew-pegar = Coller le code du destinataire
ew-pegar-como = Sur l’autre appareil : Recevoir par Wi-Fi → « Afficher mon code pour qu’on m’envoie ». Saisissez ici ses six chiffres ou collez le texte de son QR avec Ctrl+V.
ew-mandar = Envoyer
ew-dejar-de-pegar = Ne plus coller
ew-en-el-otro = Sur l’autre téléphone : PinDesk → Synchronisation → Recevoir
ew-o-escribe = ou saisissez ce code
ew-misma-wifi = Tous deux sur le même Wi-Fi
ew-misma-wifi-como = S’il n’y a pas de Wi-Fi, l’un peut activer son point d’accès et l’autre s’y connecter. Internet n’est pas utilisé.
ew-malos = Quelqu’un a essayé un code erroné ({ $n } sur { $de }).
ew-esperando = En attente… Plusieurs appareils peuvent se connecter ; vous appuyez sur le bouton pour chacun. Le code cesse de fonctionner à la fermeture de cet écran.
ew-quemado = Quelqu’un a essayé plusieurs fois un code erroné : ce code ne fonctionne plus. Renvoyez pour en obtenir un autre.
ew-cerrar = Fermer
ew-grupo = Vos appareils · sans code
ew-grupo-como = Ils sont dans votre groupe : l’envoi est direct, sans scan. Il arrive dans ce qu’ils ont d’ouvert (un chat ; un canevas n’accepte que des photos) ou dans Messages enregistrés.
ew-grupo-se-ve = Sur le même Wi-Fi
ew-grupo-no-se-ve = Invisible pour l’instant : ouvrez PinDesk sur cet appareil
ew-grupo-enviando = Envoi…
ew-grupo-todos = Envoyer à tous les appareils visibles
ew-grupo-actualiza = Mettez à jour PinDesk sur { $movil } : il ne peut pas encore recevoir de fichiers directs. Utilisez le code ci-dessous.

## «Update mine / Create as new» when receiving (RecibirActivity.kt)
rw-ya-tienes = Vous avez déjà « { $nombre } » avec les mêmes codes
rw-actualizar = Mettre à jour le mien
rw-como-nuevo = Créer comme nouveau
rw-encima-de-lo-tuyo = Il écrase le vôtre. Ce qui s’y trouvait reste dans « Sauvegardes ».
rw-entra-aparte = Il arrive à part, avec de nouveaux codes : désormais, c’est un autre élément.
rw-proyecto-al-dia = « { $nombre } » mis à jour
rw-guardados-al-dia = « { $nombre } » mis à jour dans Messages enregistrés

# The header magnifier, which searches inside the conversation (task 1.3).
chat-buscar-aqui = Rechercher ici…
chat-buscar-nada = Aucun résultat
chat-buscar-resultados = { $n ->
    [one] 1 résultat
   *[other] { $n } résultats
}
chat-buscar-en-contexto = Voir dans le contexte
# The list search box also searches inside every chat: the section under the
# chats and its "show more" (25-oct-2026).
chat-buscar-mensajes = { $n ->
    [one] Messages · 1
   *[other] Messages · { $n }
}
chat-buscar-ver-mas = Afficher { $n } de plus
chat-buscar-en = dans { $chat }
chat-pagina-n = Page { $n }
chat-unido-a = Ajouté à « { $nombre } »
chat-unir-nada = Rien ici ne peut être ajouté : seulement des photos, dessins, notes et PDF
chat-devolver = Remettre dans le projet
chat-devuelto = De retour dans « { $nombre } »
chat-devolver-no = Impossible de le remettre
unir-pdf-como-imagenes = Ajouter en images
unir-pdf-empezado = Ajout du PDF au projet…
unir-pdf-ya-va = Ce PDF est déjà en cours d’ajout
unir-pdf-no = Impossible d’ajouter le PDF
unir-pdf-hecho = { $hojas ->
    [one] 1 feuille ajoutée au projet
   *[other] { $hojas } feuilles ajoutées au projet
}
unir-pdf-pintadas = { $hojas ->
    [one] 1 feuille ajoutée (le PDF n’a pas pu être copié : elle a été dessinée)
   *[other] { $hojas } feuilles ajoutées (le PDF n’a pas pu être copié : elles ont été dessinées)
}
unir-pdf-fotos = { $hojas ->
    [one] 1 page ajoutée en image
   *[other] { $hojas } pages sur { $paginas } ajoutées en images
}
unir-pdf-pintando = Dessin des pages… { $hechas } sur { $total }
unir-pdf-imagenes-progreso = Conversion en images… { $hechas } sur { $total }

## Backups (copy of sincro/CopiasActivity.kt from the phone)
cop-titulo = Sauvegardes
cop-portada-como = L’état de chaque conversation avant chaque réception par Wi-Fi et avant chaque synchronisation.
cop-ver = Voir les sauvegardes
cop-versiones = Versions précédentes
cop-como-estaba = L’état du projet avant chaque transfert reçu et chaque synchronisation. Revenir à l’une d’elles ne supprime pas ce que vous avez fait ensuite.
cop-todavia-nada = Aucune sauvegarde pour l’instant. Elles sont créées automatiquement avant chaque réception par Wi-Fi et avant chaque synchronisation.
cop-como-se-hacen = Les 30 dernières de chaque conversation sont conservées. Les fichiers de plus de 40 Mo ne sont pas sauvegardés : impossible de revenir en arrière pour eux.
cop-hojas-mensajes = { $hojas } feuilles · { $mensajes } messages
cop-sin-grandes =  · sans le PDF (trop volumineux)
cop-volver = Restaurer
cop-cancelar = Annuler
cop-volver-titulo = Restaurer cette sauvegarde ?
cop-volver-texto = « { $nombre } » revient à son état du { $cuando } ({ $motivo }) : { $hojas } feuilles. Les canevas ajoutés ensuite restent. Une sauvegarde de l’état actuel est d’abord enregistrée, pour pouvoir annuler.
cop-ha-vuelto = « { $nombre } » a été restauré depuis la sauvegarde
cop-no-se-pudo = Impossible de restaurer la sauvegarde : { $motivo }
papelera-titulo = Projets supprimés
papelera-como = Ils reviennent entiers : avec leur nom, leurs feuilles dans l’ordre, leur PDF et leur chat. Et ils ne sont plus marqués comme supprimés : la synchronisation ne les effacera plus.
papelera-detalle = { $hojas } feuilles · { $mensajes } messages · supprimé le { $cuando }
papelera-recuperar = Récupérer
papelera-recuperar-todos = Tout récupérer
papelera-ha-vuelto = « { $nombre } » est revenu, entier
papelera-no-se-pudo = Impossible de récupérer « { $nombre } » : { $motivo }
papelera-recuperados = { $n } projets récupérés sur { $total }

## The document viewer: Word, EPUB books and web pages (task 2.1)
## The Office pin: Office's preview as a pin (2026-10-10)
office-titulo = Aperçu Office
office-abriendo = Ouverture avec l’aperçu Office…
office-tarda = Office prend du temps (il doit démarrer). Vous pouvez attendre, ou choisir une application avec « Ouvrir avec… » (le bouton ↗ de la barre).
office-fallo = Office n’a pas pu afficher l’aperçu de ce fichier. Ouvrez-le dans une autre application avec « Ouvrir avec… » (le bouton ↗ de la barre).
office-internet = Ce fichier provient d’internet et, par sécurité, Windows n’autorise pas son aperçu. Ouvrez-le dans une autre application avec « Ouvrir avec… » (le bouton ↗ de la barre), ou débloquez-le dans ses Propriétés.
visor-titulo = Visionneuse PinDesk
visor-tamano = Taille du texte
visor-quitar-marcadores = Supprimer les signets
visor-guardar-pagina = Enregistrer comme page web
visor-guardar-pdf = Enregistrer en PDF
visor-abrir-carpeta = Ouvrir le dossier
visor-guardado = Enregistré dans « { $nombre } »
visor-no-guardado = Impossible d’enregistrer
visor-vacio = Ce document n’a pas de texte à afficher

## The readers: comfortable reading, annotating on top and the PDF reader
lector-indice = Sommaire
lector-letra-fijada = La police est verrouillée : des notes sont dessinées par-dessus
lector-letra-fija = Chasse fixe
lector-grosor-normal = Normal
lector-grosor-gruesa = Gras
lector-quitar-tinta = Supprimer les notes
lector-tinta-quitada = Notes supprimées (Ctrl+Z les rétablit tant que vous restez ici)
lector-ayuda-docs = Molette pour lire · Ctrl+molette zoom · L écouter · A annoter · M signet · I sommaire · +/− taille · T police · B graisse · Échap pour quitter
lector-pdf-titulo = Lecteur PDF PinDesk
lector-pdf-no-se-abre = Impossible d’ouvrir le PDF
lector-pdf-abriendo = Ouverture du PDF…
lector-pdf-reconociendo = Reconnaissance du texte de la page…
lector-pdf-reconocido = Texte reconnu : vous pouvez le sélectionner et le marquer
lector-pdf-sin-texto = Aucun texte trouvé sur cette page
lector-exportando = Enregistrement du PDF annoté… { $hechas } sur { $total }
lector-anotado = annoté
lector-office-titulo = Word et PowerPoint dans PinDesk
lector-presentar = Présenter (F5)
lector-modo-lectura = Mode lecture (texte recomposé)
lector-imprimir = Imprimer… (Ctrl+P)
lector-no-imprime = Impossible d’ouvrir la boîte de dialogue d’impression
lector-copiado = Copié
lector-escuchar = Écouter (L)
lector-sin-texto = Ce document n’a pas de texte à lire
lector-leyendo-texto = Extraction du texte du PDF pour le lire…
lector-sin-voz = Windows n’a pas de voix pour « { $idioma } » : ajoutez-en une dans Paramètres > Heure et langue > Voix
lector-voz-idioma = fr-FR
lector-voces-microsoft-no = ☐ Voix Microsoft · très naturelles et gratuites ; service non officiel, le texte est envoyé à Microsoft
lector-voces-microsoft-si = ☑ Voix Microsoft · très naturelles et gratuites ; service non officiel, le texte est envoyé à Microsoft
lector-voz-de-microsoft = Voix Microsoft : { $voz }
lector-voz-del-pais = celle du pays
lector-voz-pidiendo = Demande des voix à Microsoft… appuyez à nouveau dans un instant
lector-voz-sin-voces = Pas de réseau, ou Microsoft n’a pas de voix pour cette langue
lector-voz-de-ia = IA, lit toutes les langues
lector-voz-mujer = féminine
lector-voz-hombre = masculine
lector-voz-parrafo = Paragraphe { $n } sur { $total }
lector-voz-preparando = Préparation de la voix…
lector-voz-no-respondio = Microsoft n’a pas répondu
lector-voz-sin-voz = Aucune voix
lector-espacio-tope = Plus de place de ce côté
lector-espacio-nada = Il ne reste plus d’espace de ce côté
lector-sin-lado = Défilement latéral verrouillé : uniquement vers le haut et le bas
lector-con-lado = Défilement dans tous les sens
lector-auto = Défilement automatique (D)
lector-auto-ppm = { $ppm } mots/min
lector-auto-quedan = Fini dans { $cuanto } · à { $hora }
lector-auto-menos-de-un-minuto = moins d’1 min
lector-auto-terminado = Terminé
lector-auto-contando = Comptage des mots…
lector-ver-como-texto = Afficher en texte (comme un fichier Word)
# Lectura Veloz, the speed-reading app (27-Oct-2026).
lector-lectura-veloz = ⚡ Lecture rapide
lector-lectura-veloz-no-instalada = Lectura Veloz n’est pas installé sur ce PC (le lien lecturaveloz:// est absent). Installez-le et réessayez.
lector-lectura-veloz-no-se-pudo = Impossible d’ouvrir Lectura Veloz : { $motivo }
# Speed reading (27-oct-2026): the % next to the option, the bubble chip
# and the real reason when it can't open.
lector-lectura-veloz-con = ⚡ Lecture rapide · { $pct } %
lector-lectura-veloz-sin-codigo = Ce document ne peut pas être lu dans Lectura Veloz : ce n’est ni un fichier du chat ni une note enregistrée (il n’a pas de code de message).
lector-lectura-veloz-formato = Ce document ne peut pas être lu dans Lectura Veloz : il ne lit pas les fichiers « .{ $ext } ».
lector-lectura-veloz-falta-archivo = Ce document ne peut pas être lu dans Lectura Veloz : le fichier n’est plus sur cet ordinateur ({ $nombre }).
chat-lectura-veloz-chapa = Lecture rapide · { $pct } %
# The chip of what came from the phone's scanner (`escaneado`).
chat-escaneado = Numérisé
lector-pasando-a-texto = Conversion du PDF en texte…
lector-texto-escaneo = Ce PDF n’a pas de texte à extraire : c’est probablement une numérisation (une photo des pages)
lector-texto-cifrado = Le PDF est protégé par mot de passe
lector-texto-no-se-lee = Impossible de lire le PDF
lector-ver-pdf-original = Afficher le PDF d’origine, en pages
lector-hoja = Page
sinc-presencia-apagada = La présence est désactivée dans les paramètres ([sincro] presencia = false) : vous pouvez synchroniser d’ici, mais le téléphone ne trouvera pas cet ordinateur et ne pourra pas l’appeler.
bandeja-abrir-documento = Ouvrir un document…

## Canvas shapes and ink: arrowheads, fills and text style (group A)

## Build and measure: fill, trim, extend, points and angles (group C)

## Select, transform and bind (group B)

# The attachment the document viewer can open without leaving PinDesk.

# The seven mini-apps from the phone, and their panel.
mini-moneda = EUR
mini-tareas = Liste de tâches
mini-gastos = Dépenses
mini-cronometro = Chronomètre
mini-temporizador = Minuteur
mini-contador = Compteur
mini-ruleta = Roulette
mini-alarma = Alarme
mini-tarea-nueva = Une chose à faire, puis Entrée · ↑↓ choisir, Espace coche, F2 modifie
mini-gasto-nuevo = Libellé et montant (Dîner 42,50), puis Entrée · ↑↓ choisir, F2 modifie
mini-nombre-nuevo = Un nom, puis Entrée · Entrée à vide fait tourner, Suppr retire
mini-limpiar-hechas = Effacer les tâches faites
mini-ocultar-hechas = Masquer les tâches faites
mini-mostrar-hechas = Afficher les tâches faites
mini-avance = { $hechas } sur { $de } faites
mini-tarea-edad = { $dias ->
    [0] aujourd’hui
    [one] il y a 1 jour
   *[other] il y a { $dias } jours
  }
mini-arrancar = Démarrer
mini-parar = Arrêter
mini-vuelta = Tour
mini-reiniciar = Réinitialiser
mini-sortear = Tourner
mini-alarma-encender = Activer
mini-alarma-apagar = Désactiver
mini-guia-titulo = Nouveau nom, puis Entrée (Échap garde l’ancien)
mini-guia-cambiar = Corrigez puis appuyez sur Entrée (Échap laisse tel quel)
mini-guia-duracion = Minutes ou m:ss, puis Entrée · Espace démarre
mini-guia-hora = Heure (7:30), puis Entrée · Espace l’active
chat-biblioteca-vacia = Aucune note vocale ni chanson pour l’instant.
chat-biblioteca-musica = Musique
chat-biblioteca-notas = Notes vocales

# Recording a voice note from the chat.
chat-voz-muy-corta = Trop court : maintenez un peu plus longtemps.
chat-voz-sin-microfono = Aucun micro, ou PinDesk n’est pas autorisé à l’utiliser. Vérifiez Paramètres › Confidentialité › Microphone.
chat-voz-sin-codec = Ce Windows n’a pas d’encodeur audio (en général une édition N sans le Media Feature Pack).
chat-voz-fallo = Impossible d’enregistrer la note vocale.

# Shrinking an attached PDF: it lowers the photos inside, and never for the worse.
pdf-aligerado = Allégé : de { $antes } à { $despues }
pdf-aligerar-cifrado = Le PDF est chiffré et ne peut pas être réécrit sans risque.
pdf-aligerar-no-se-lee = Le fichier allégé ne se relit pas correctement ; l’original est conservé.
aligerar-progreso = Allègement… { $por } %
aligerar-empezado = Allègement du PDF…
aligerar-sin-mejora = Ce PDF était déjà léger : il reste tel quel.

# Reminders: the time set on a message and the notice that shows up when it
# arrives. The same times the phone offers, plus ten minutes.
chat-recordar-10-min = Dans 10 minutes
chat-recordar-1-hora = Dans 1 heure
chat-recordar-3-horas = Dans 3 heures
chat-recordar-esta-tarde = Ce soir (18:00)
chat-recordar-manana = Demain (9:00)
chat-recordatorio-puesto = Je vous le rappellerai à { $cuando }
chat-recordatorio-quitar = Supprimer le rappel
chat-recordatorio-quitado = Rappel supprimé
chat-recordatorio-titulo = Rappel

# A message menu entry: send the attachment to someone over Wi-Fi.

# Turning a voice note into text with Vosk. What is missing is named with its
# file and link: PinDesk never downloads anything on its own.
chat-transcribiendo = Conversion en texte… { $pct } %
chat-transcribir-hecha = Note vocale convertie en texte
chat-transcribir-en-marcha = Une autre note est déjà en cours de conversion ; attendez qu’elle se termine
chat-transcribir-sin-mensaje = La note n’est plus dans la conversation : le texte n’a nulle part où aller
chat-transcribir-cancelada = La conversion en texte a été annulée
chat-transcribir-otra-vez = Reconvertir en texte
chat-transcripcion-no = Impossible de convertir en texte : rien n’a été compris dans l’audio.
chat-letra-vacia = Cette note n’a pas encore de texte.
chat-letra-copiada = Texte copié, avec ses horodatages

## Music with its lyrics (karaoke, 27-Oct-2026)
chat-adj-musica = Musique
musica-anadida = Chanson ajoutée. Appuyez sur le crayon pour saisir ou coller ses paroles
musica-anadidas = { $n } chansons ajoutées
musica-no-es-audio = « { $nombre } » n’est pas un audio lisible
musica-sin-letra = Cette chanson n’a pas encore de paroles
musica-escribir-letra = Appuyez sur le crayon pour saisir ou coller les paroles (texte brut ou LRC)
musica-sinc-primero-letra = Saisissez ou collez d’abord les paroles
musica-sinc-empieza = Appuyez sur Espace (ou « Marquer ») au début de chaque ligne
musica-sinc-siguiente = Suivante : { $linea }
musica-sinc-todas = Toutes les lignes sont marquées. Enregistrez les paroles.
musica-sinc-marcar = Marquer { $n }/{ $total }
musica-sinc-guardar = Enregistrer
musica-sinc-cancelar = Annuler
musica-sinc-guardada = Paroles synchronisées
musica-sinc-teclas = Espace marque · Retour arrière annule · Entrée enregistre · Échap annule
musica-lrc-copiada = Paroles copiées en LRC
musica-sola-empieza = Synchronisation automatique des paroles : écoute de la chanson…
musica-sola-avance = Synchronisation des paroles… { $pct } %
musica-sola-bajando = Téléchargement de la reconnaissance vocale… { $pct } %
musica-sola-parada = Synchronisation arrêtée
musica-sola-ocupada = Une autre chanson est déjà en cours de synchronisation
musica-sola-sin-reconocedor = Cet ordinateur ne peut pas écouter la chanson : ONNX Runtime est manquant
musica-sola-falta-modelo = Pour placer les temps automatiquement, appuyez sur « Synchro auto » (les sous-titres) : la reconnaissance vocale est téléchargée une seule fois (environ 160 Mo)
musica-sola-no-se-oye = Les paroles n’ont pas pu être reconnues dans la chanson. Synchronisez-les en tapant avec l’horloge
musica-sola-fallo = Impossible de synchroniser les paroles
musica-sola-cambio = Les paroles ont changé pendant la synchronisation : appuyez à nouveau sur « Synchro auto »
musica-sola-hecha = Paroles synchronisées automatiquement ({ $n } lignes entendues sur { $total }). Si l’une est décalée, corrigez-la avec l’horloge
musica-sola-hecha-poca = Paroles synchronisées approximativement : seulement { $n } lignes entendues sur { $total }. Corrigez-les avec l’horloge

chat-recordar-hora = Choisir l’heure…
chat-recordar-hora-a-las = Me le rappeler à
chat-recordar-hora-teclas = Saisissez l’heure · Entrée valide · Échap annule
chat-recordar-hora-queda = Entrée : { $cuando }
chat-recordar-hora-mal = Ce n’est pas une heure : saisissez-la comme 18:30
chat-voz-sin-motor = La reconnaissance vocale est manquante : copiez { $motor } dans { $donde }
chat-voz-sin-modelo = Le modèle vocal { $modelo } est manquant : téléchargez-le depuis { $enlace } et décompressez-le dans { $donde }
chat-voz-idioma-sin-modelo = Vosk n’a pas de modèle vocal pour cette langue
chat-voz-ruta-imposible = Le dossier du modèle ({ $donde }) contient des caractères que la reconnaissance ne peut pas ouvrir ; déplacez-le vers un chemin sans accents
chat-voz-modelo-ilegible = Le modèle dans { $donde } est incomplet ou n’a pas pu être chargé
chat-voz-no-es-audio = Impossible de lire l’audio de cette note
chat-voz-audio-vacio = L’audio est vide : rien à convertir
chat-voz-no-se-entiende = Rien de ce qui est dit dans la note n’a pu être compris
chat-voz-sin-reconocedor = Windows n’a pas de reconnaissance vocale pour cette langue : ajoutez-la dans Paramètres › Heure et langue › Voix.
chat-voz-bajar-whisper = La conversion en texte nécessite le modèle vocal Whisper (environ { $megas } Mo). Il est en cours de téléchargement, une seule fois, puis la note sera convertie.
chat-voz-bajando-whisper = Téléchargement du modèle vocal… { $pct } %
chat-voz-sin-modelo-whisper = Le modèle vocal Whisper (environ { $megas } Mo) est manquant : appuyez à nouveau sur « Convertir en texte » pour le télécharger.
chat-voz-sin-modelo-qwen3 = Qwen3-ASR n’est pas téléchargé (environ { $megas } Mo) : choisissez-le dans Paramètres › Voix pour le télécharger, ou revenez à Whisper.
chat-voz-sin-onnx = Ce Windows n’a pas le moteur d’IA (onnxruntime.dll) : copiez-le dans { $donde } ou à côté de PinDesk.
chat-voz-descarga = Impossible de télécharger le modèle vocal ({ $que }). Vérifiez la connexion et appuyez à nouveau sur « Convertir en texte ».
chat-voz-onnx = Le moteur vocal Whisper a échoué : { $detalle }
chat-dictado-oyendo = À l’écoute… parlez, puis cliquez sur le micro quand vous avez fini
chat-dictado-fallo = La dictée a échoué : { $razon }
chat-anotar-en = Noter dans « { $chat } » : { $texto }
chat-anotar-donde = Dans quel chat placer « { $texto } » ? Choisissez un chat
chat-anotar-tecla = Entrée
chat-anotado-en = Noté dans « { $chat } »
chat-dictado-preparando = Préparation du micro…

# The teleprompter: the text scrolls by and you read it aloud while it
# records. There is no synthetic voice anywhere, same as on the phone.
telepronter-titulo = Lire à voix haute
telepronter-ensayar = Répéter
telepronter-parar = Arrêter
telepronter-grabar = Enregistrer
telepronter-terminar = Terminer
telepronter-tirar = Ignorer
telepronter-velocidad = vitesse et taille
telepronter-preparate = Préparez-vous
telepronter-muy-corta = Cette lecture était trop courte
telepronter-sin-micro = Aucun micro disponible
chat-telepronter-borrador = Ce qui est saisi dans la zone
chat-telepronter-sin-texto = Aucun texte à lire : saisissez quelque chose dans la zone ou joignez un .txt ou .md
chat-telepronter-abierto = Le prompteur est déjà ouvert
chat-telepronter-hecha = Lecture enregistrée comme note vocale

# The capture stack: the little pile in the corner (icon and panel).
pila-titulo-una = 1 capture
pila-titulo-varias = { $cuantas } captures empilées
pila-copiar-elegidas = Copier la sélection
pila-copiar-todas = Tout copier
pila-quitar = Retirer
pila-copiadas = { $cuantas } captures copiées : collez-les avec Ctrl+V

# The drawing editor's properties panel (Excalidraw's).
panel-trazo = Trait
panel-fondo = Arrière-plan
panel-relleno = Remplissage
panel-grosor = Épaisseur du trait
panel-estilo-trazo = Style du trait
panel-presion = Pression
panel-trazo-a-mano = Style main levée
panel-bordes = Bords
panel-tipo-flecha = Type de flèche
panel-fuente = Police
panel-tamano-fuente = Taille de police
panel-alineacion-texto = Alignement du texte
panel-puntas = Pointes
panel-opacidad = Opacité
panel-capas = Calques
panel-alinear = Aligner
panel-acciones = Actions
panel-colores = Couleurs
panel-tonos = Nuances
panel-codigo-hex = Code hexa

## The canvas paper (Excalidraw viewBackgroundColor)
fondo-lienzo = Fond du canevas
fondo-papeles-del-movil = Papiers du téléphone
fondo-lienzo-menu = Fond du canevas…

## Export and print the canvas (G1, G2, G3, F11)
exportar-menu = Exporter…	Ctrl+Maj+E
exportar-copiar-png = Copier en PNG	Ctrl+Maj+C
exportar-titulo = Exporter le canevas
exportar-que = Quoi
exportar-todo = Tout le canevas
exportar-seleccion = Sélection
exportar-marcos = Chaque cadre
exportar-escala = Échelle
exportar-transparente = Fond transparent
exportar-tipo-png = Image PNG
exportar-tipo-svg = Dessin SVG
exportar-tipo-pdf = Document PDF
exportar-tipo-html = Page web en un seul fichier
exportar-vacio = Il n’y a rien à exporter avec ceci.
exportar-fallo = Impossible d’exporter le canevas.
imprimir-menu = Imprimer…	Ctrl+P
imprimir-vacio = Il n’y a rien à imprimer.
imprimir-fallo = Impossible d’imprimer le canevas.
exportar-chat = Exporter…
imprimir-chat = Imprimer…
# F11: imprimir con vista previa (el dialogo de Windows y el boton de la barra)
imprimir-pista = Imprimer… (Ctrl+P)
imprimir-que = Quoi imprimer
imprimir-que-marcos = Chaque cadre sur sa propre page
imprimir-que-todo = Tout le canevas sur une page
imprimir-que-seleccion = Seulement la sélection
marca-elige = Avec quoi le marquer ? (1-9, 0) · Échap pour annuler
marca-plantar = Cliquez sur le canevas à l’endroit de la marque · Échap annule
marca-vista-guardada = Vue enregistrée avec la marque · Maj+clic pour la mettre à jour
hojita-borrar = Effacer
hojita-insertar = Insérer
hojita-pegar = En épingle

# The share sheet (compartir.rs): one for everything, like the phone's.
compartir-titulo = Partager
compartir-menu = Partager…	Ctrl+Maj+S
compartir-proyecto = Partager…
compartir-lector = Partager…  (Ctrl+Maj+S)
compartir-boton = Partager
compartir-guardar = Enregistrer sous…
compartir-copiar = Copier dans le presse-papiers
compartir-wifi = Par Wi‑Fi vers mes appareils
compartir-boton-copiar = Copier
compartir-boton-wifi = Wi-Fi
compartir-boton-enlace = Lien
compartir-enlace-pista = Le téléverse sur litterbox (3 jours) et copie le lien : toute personne qui l’a peut l’ouvrir
compartir-enlace-subiendo = Téléversement…
compartir-enlace-copiado = Lien copié · supprimé dans 3 jours
compartir-enlace-fallo = Échec du téléversement : { $motivo }
compartir-web = Page web (HTML)
compartir-adjuntos = Pièces jointes
compartir-pdf = PDF
compartir-png = Image PNG
compartir-jpg = Image JPG
compartir-svg = Dessin SVG
compartir-original = Original
compartir-originales = Originaux
compartir-editable = Modifiable
compartir-texto = Texte
compartir-csv = Tableau (CSV)
compartir-pindesk = PinDesk (.pindesk)
compartir-dxf = DXF
compartir-md = Markdown
compartir-exportar = Exporter…
compartir-paginas-de = À inclure · { $marcadas } sur { $total }
compartir-una-pagina = Quelle page
compartir-va-entero = Envoyé entier, tel quel
compartir-todas = Toutes
compartir-ninguna = Aucune
compartir-preparando = Préparation…
compartir-marca-alguna = Cochez au moins une page
compartir-en-cuanto-este = Dès que c’est prêt
compartir-peso-ficheros = { $peso } · { $n } fichiers
compartir-guardado = Enregistré : { $nombre }
compartir-copiado = Copié dans le presse-papiers
compartir-no-se-pudo = Impossible de le préparer
compartir-nada = Il n’y a rien à partager ici.
compartir-coma-decimal = ,
compartir-lienzo = Canevas
compartir-lienzo-entero = Tout le canevas
compartir-marco = Cadre
compartir-pagina = Page
compartir-anotada = Avec les notes
compartir-sin-nombre = Sans titre
compartir-tipo-lienzo = Canevas
compartir-tipo-pagina = Page PDF
compartir-tipo-foto = Photo
compartir-tipo-tabla = Tableau
compartir-tipo-miniapp = Mini-app
compartir-tipo-voz = Note vocale
compartir-tipo-nota = Note

# Drawing tools: the ones shown in the canvas, the reader, pins and the
# screen annotator. Turning one off removes it from the bar and its key.
herramientas-lazo = Lasso
herramientas-rectangulo = Rectangle
herramientas-rombo = Losange
herramientas-elipse = Ellipse
herramientas-flecha = Flèche
herramientas-flecha-codos = Flèche coudée
herramientas-flecha-libre = Flèche à main levée
herramientas-linea = Ligne
herramientas-lapiz = Stylo
herramientas-grafito = Crayon
herramientas-texto = Texte
herramientas-borrador = Gomme
herramientas-resaltador = Surligneur
herramientas-foco = Projecteur
herramientas-lupa = Loupe
herramientas-mosaico = Mosaïque
herramientas-arco = Arc
herramientas-serie = Étapes numérotées
herramientas-punto = Point nommé
herramientas-cota = Cote
herramientas-escalar = Calibrer l’échelle
herramientas-escala-grafica = Barre d’échelle
herramientas-marco = Cadre
herramientas-relleno = Pot de peinture
herramientas-recortar = Ajuster
herramientas-extender = Prolonger
herramientas-copiar-estilo = Copier le style

## D10: a PowerPoint in the chat (diapositivas.rs)
diapositivas-titulo = Présentation
diapositivas-no-es = Ceci n’est pas une présentation
diapositivas-keynote = Les fichiers Keynote ne peuvent pas être lus directement : exportez-les en PowerPoint ou PDF
diapositivas-sin-powerpoint = Présenter un PowerPoint nécessite que PowerPoint soit installé
diapositivas-abrir-con-otra = L’ouvrir avec une autre application ?
diapositivas-preparando = Préparation de la présentation avec PowerPoint…
diapositivas-no-se-pudo = Impossible de préparer la présentation
diapositivas-no-esta = La présentation n’est plus sur cet ordinateur
diapositivas-siguiente = Suivante
diapositivas-notas = Notes
diapositivas-sin-notas = Cette diapositive n’a pas de notes
diapositivas-fin = Fin de la présentation
diapositivas-transiciones-si = Transitions activées (T)
diapositivas-transiciones-no = Transitions désactivées (T)

## Chat: a page from a project, the link card, «Received from» and the inbox
chat-sin-paginas = Aucun projet n’a de pages à joindre
chat-recibido-de = Reçu de { $de }
# Linked versions in a shared chat (27-oct-2026): touching someone else's
# message makes your version, which replies to theirs, and theirs stays as is.
chat-version-sola = Version
chat-version-sola-n = Version { $n }
chat-version-de = Version du message de { $quien }
chat-version-de-n = Version { $n } du message de { $quien }
chat-version-de-lo-tuyo = Version du vôtre
chat-version-de-lo-tuyo-n = Version { $n } du vôtre
chat-version-de-otro = Version d’un autre message
chat-version-de-otro-n = Version { $n } d’un autre message
chat-version-de-la-lista = Version { $n } · { $quien }
chat-version-tuya = Version { $n } · la vôtre
chat-versiones = { $cuantas ->
    [one] 1 version
   *[other] { $cuantas } versions
  }
chat-version-es-de = Ce message est de { $quien } : si vous le modifiez, il est enregistré comme votre version et le sien reste intact
chat-version-guardada = Enregistré comme votre version ; l’original reste intact
chat-version-no = Impossible de préparer votre version
chat-fecha-corta = { $dia } { $mes }
chat-enlace-no-abre = Impossible d’ouvrir le lien

## D11: a spreadsheet in the chat is a book of tables (ventana_chat/libro.rs)
hoja-preparando = Préparation de la feuille pour la modifier…
hoja-editar-aviso = Les modifications sont enregistrées dans la feuille du chat ; le fichier d’origine reste inchangé
hoja-editando = Modification · Échap pour afficher
hoja-rango-grande = Trop de cellules à la fois
hoja-no-editable = Cette feuille n’a pas de tableau : elle ne peut pas être modifiée
hoja-fondo-sin = Sans couleur
hoja-letra-automatica = Automatique
hoja-color-amarillo = Jaune
hoja-color-verde = Vert
hoja-color-azul = Bleu
hoja-color-rosa = Rose
hoja-color-morado = Violet
hoja-color-naranja = Orange
hoja-color-gris = Gris
hoja-color-celeste = Bleu clair
hoja-color-rojo = Rouge
hoja-numero-general = Standard
hoja-numero-numero = Nombre (1 234,50)
hoja-numero-moneda = Monétaire
hoja-numero-porcentaje = Pourcentage (12,50 %)
hoja-numero-fecha = Date

## D9: find in the document (buscador.rs, visor.rs, lector_pdf.rs)
buscar-pista = Rechercher dans le document
buscar-cuenta = { $n } sur { $total }
buscar-nada = Aucun résultat
buscar-leyendo = Lecture du texte…
buscar-sin-texto = Ce PDF n’a pas de texte (c’est une image)
buscar-cifrado = Impossible de rechercher dans un PDF chiffré

## D5: rename from the reader (renombrar_doc.rs)
renombrar-hecho = S’appelle désormais « { $nombre } »
renombrar-ya-existe = Un fichier nommé « { $nombre } » existe déjà
renombrar-no-se-pudo = Impossible de changer le nom

## Canvas: zone, laser, image, shapes, graph and present (F8, F12, F14, G5)
herramientas-zona = Zone (copie arrondie)
herramientas-laser = Pointeur laser
herramientas-imagen = Image
herramientas-figuras = Formes
figuras-grafica = Graphe d’une fonction…
figuras-tabla-en-blanco = Tableau vierge
figuras-pegar-tabla = Coller un tableau (depuis Excel)
figuras-recta = Droite graduée
figuras-plano = Plan cartésien
figuras-espacio = Espace xyz
figuras-coordenadas = Tableau de coordonnées…
figuras-coordenadas-serie = Coordonnées : série
figuras-coordenadas-nueva = Nouvelle série de coordonnées…
figuras-coordenadas-esconder = Masquer la série
figuras-coordenadas-ver = Afficher la série
coordenadas-titulo = Tableau de coordonnées
coordenadas-ayuda = X et Y avec virgule ou point, Y vers le haut · Coller : depuis Excel ou un CSV · sans lignes, la série est supprimée · Entrée : valider · Échap : annuler
figuras-guardar-seleccion = Enregistrer la sélection comme forme…
figuras-quitar = Retirer
figuras-mis-figuras = Mes formes
figuras-ventana-titulo = Formes
figuras-ventana-subtitulo = { $n } formes et modèles
figuras-ventana-buscar = Rechercher des formes
figuras-ventana-todas = Toutes
figuras-ventana-insertar-seccion = Insérer
figuras-ventana-insertar = Insérer
figuras-ventana-guardar-seleccion = Enregistrer la sélection
figuras-ventana-vacio = Aucune forme ne correspond
figuras-ventana-elige = Choisissez une forme pour l’agrandir
figuras-ventana-piezas = { $n } pièces
figuras-ventana-intervalo = Plage
figuras-ventana-desde = De
figuras-ventana-hasta = À
figuras-ventana-de-a = Diviser tous les … (une feuille chacun)
figuras-ventana-de-a-pista = Tout sur une feuille
figuras-ventana-semanas-desde = De la semaine
figuras-ventana-semanas-hasta = À la semaine
figuras-ventana-mes = Mois
figuras-ventana-meses = Combien de mois
figuras-ventana-una-hoja = 1 feuille
figuras-ventana-hojas = { $n } feuilles
figuras-ventana-renombrar = Renommer
figuras-ventana-quitar-seguro = Cliquez à nouveau pour la retirer
figuras-nombre-titulo = Enregistrer comme forme
figuras-nombre = Nom
figuras-nombre-ayuda = Entrée : enregistrer · Échap : annuler
figuras-sin-tabla = Le presse-papiers ne contient pas de tableau
grafica-titulo = Graphe d’une fonction
grafica-formulas = y = (une courbe par ligne)
grafica-x-desde = x de
grafica-x-hasta = x à
grafica-y-desde = y de
grafica-y-hasta = y à
grafica-escala = Pixels par unité
grafica-ayuda = Entrée : insérer · Maj+Entrée : autre courbe · Tab : champ suivant · Échap : annuler · par morceaux : x^2 si x<0; 2x si x>=0
grafica-limites-mal = Les limites n’ont pas de sens : « de » doit être inférieur à « à »
grafica-sin-formula = Saisissez une formule
grafica-formula-mal = Incompréhensible :
grafica-solo-x = Seul x peut être utilisé :
presentar-ayuda = ← → PgSuiv : suivante · B : noir · J : laser · L : stylo · Échap : quitter
vista-mirando = Lecture seule · le crayon à droite permet de modifier · F5 : présenter

## --- Voice: two languages, lyrics, teleprompter, pronounce, conversation, call (B6-B11) ---

ajustes-voz-segundo = Deuxième langue des notes vocales
ajustes-voz-modo = Avec deux langues
ajustes-voz-modo-cada-uno = Chaque partie telle qu’elle a été dite
ajustes-voz-modo-todo-en-uno = Tout dans la première
ajustes-voz-ninguno = Aucune
ajustes-voz-ingles = Anglais
ajustes-voz-espanol = Espagnol
ajustes-voz-portugues = Portugais
chat-letra-editar = Modifier les paroles
chat-letra-editando = Les paroles sont déjà en cours de modification
chat-letra-guardar = Enregistrer
chat-letra-guardada = Paroles enregistrées
chat-cancelar-caja = Annuler
chat-llamada-secreta = Appel secret
telepronter-pausa = Pause
telepronter-seguir = Reprendre
telepronter-velocidad-muy-lenta = très lente
telepronter-velocidad-lenta = lente
telepronter-velocidad-normal = normale
telepronter-velocidad-rapida = rapide
telepronter-velocidad-muy-rapida = très rapide
pronunciar-titulo = Prononcer
pronunciar-manten = Maintenez la barre d’espace (ou le micro) et parlez
pronunciar-suelta = Relâchez pour vous écouter
pronunciar-otra-vez = Maintenez à nouveau pour recommencer
pronunciar-guia = Guide
pronunciar-guia-texto = Écrire un texte
pronunciar-guia-fichero = Une image, un PDF ou une note de cet ordinateur
pronunciar-sin-guia = Pas de guide : écrivez-en un, ou apportez une image, un PDF ou une note
pronunciar-repetir = Réécouter (R)
pronunciar-guardar = Enregistrer dans le chat (Ctrl+G)
pronunciar-ya-guardada = Enregistré
pronunciar-guardado = Enregistré ; transcription en cours pour voir ce qui a été compris
pronunciar-idioma = Langue que vous pratiquez
pronunciar-idioma-ajustes = L’habituelle
pronunciar-oir-texto = Écouter le texte
pronunciar-sin-voz = Windows n’a pas de voix pour cette langue
pronunciar-no-se-pudo = Impossible de l’ouvrir
pronunciar-toma = Dernière prise : { $duracion }
pronunciar-pagina = Page { $pagina } sur { $paginas }
pronunciar-abierto = Prononcer est déjà ouvert
conversacion-titulo = Conversation
conversacion-explicacion = L’enregistrement est continu ; chaque personne appuie sur son numéro (1 à 9) quand elle commence à parler, et 0 quand personne ne parle. La transcription sort avec les noms.
conversacion-cuantos = Combien de personnes parlent ?
conversacion-persona = Personne { $n }
conversacion-empezar = Démarrer l’enregistrement
conversacion-terminar = Terminer et transcrire
conversacion-turnos = { $n } tours de parole
conversacion-hablando = Parle…
conversacion-teclas = Appuyez sur le numéro de qui parle · 0 : personne
conversacion-transcribiendo = Transcription de la conversation…
conversacion-abierta = La conversation est déjà ouverte
llamada-titulo = Appel
llamada-entrante = Appel entrant
llamada-contestar = Répondre
llamada-colgar = Raccrocher
llamada-altavoz = Haut-parleur
llamada-perdida = Appel manqué

## PDF zone (E9): merge PDF pages into one canvas, as on the phone
fusionar-paginas = Fusionner en un canevas
fusionar-paginas-empieza = Assemblage des pages dans un canevas…
fusionar-paginas-hecho = Terminé : « { $nombre } » est maintenant dans le projet
fusionar-paginas-no = Impossible de fusionner ces pages

## Universe zone (batch 2: H2, H3, M1): the Cosmos theme outside the universe
ajustes-tema-cosmos = Thème Cosmos (ciel étoilé dans le chat)

## Zona pines (tanda 2: C3, C4, L3): magic word, board, sheet and tools
pin-convertir-en = Convertir en
pin-herramienta-temporizador = Minuteur
pin-herramienta-cronometro = Chronomètre
pin-herramienta-tareas = Liste de tâches
pin-herramienta-contador = Compteur
pin-herramienta-gastos = Dépenses
pin-herramienta-pizarra = Tableau
pin-herramienta-ruleta = Roulette
pin-herramienta-lienzo = Canevas
pin-herramienta-hoja = Feuille
pin-herramienta-tabla = Tableau de données
pin-fondo-pizarra = Fond du tableau
pin-pizarra-blanca = Blanc
pin-pizarra-negra = Noir
pin-pizarra-azul = Bleu ardoise
pin-pizarra-verde = Vert tableau noir
pin-pauta-lisa = Uni
pin-pauta-cuadros = Grille
pin-pauta-rayas = Lignes
pin-pauta-columnas = Colonnes
pin-pauta-puntos = Points
pin-tiempo-cumplido = Temps écoulé
pin-hoja-nombre = Feuille

## Window groups (H9) and Markdown note editor (H12)
bandeja-grupos-ventanas = Groupes de fenêtres…
chat-adj-nota-md = Note
chat-editar-nota = Ouvrir dans l’éditeur de notes
grupos-nada-abierto = Aucun canevas, document ou note ouvert à enregistrer
grupos-guardar = { $n ->
    [one] Enregistrer la fenêtre ouverte comme groupe…
   *[other] Enregistrer les { $n } fenêtres ouvertes comme groupe…
}
grupos-abrir = Ouvrir « { $nombre } » · { $n }
grupos-borrar = Supprimer « { $nombre } »
grupos-alguna-falta = Une fenêtre du groupe ne peut plus être ouverte
grupos-sin-nombre = Groupe
grupos-nombre = Nom du groupe de fenêtres
grupos-guardar-boton = Enregistrer
nota-md-nueva = Nouvelle note
nota-md-sufijo = Note PinDesk
nota-md-negrita = Gras
nota-md-cursiva = Italique
nota-md-tachado = Barré
nota-md-codigo = Code
nota-md-enlace = Lien
nota-md-titulo1 = Titre
nota-md-titulo2 = Sous-titre
nota-md-titulo3 = Section
nota-md-lista = Liste
nota-md-numerada = Liste numérotée
nota-md-casilla = Liste de tâches
nota-md-marcar = Cocher ou décocher la tâche
nota-md-cita = Citation
nota-md-bloque = Bloc de code
nota-md-raya = Séparateur
nota-md-formula = Formule
nota-md-cortar = Couper
nota-md-copiar = Copier
nota-md-pegar = Coller
nota-md-guardar = Enregistrer
nota-md-copia = Enregistrer une copie .md…
nota-md-tipo = Note Markdown
nota-md-no-guardada = La note n’a pas pu être enregistrée. Fermer quand même et perdre les modifications ?
nota-md-mayus = Maj
nota-md-intro = Entrée

# H12, the new note editor window (header, toolbar, menus and tables)
nota-md-compartir = Partager
nota-md-tabla = Tableau
nota-md-imagen = Image
nota-md-fecha = Date du jour
nota-md-pista-barra = Tapez / dans la note pour plus d’options
nota-md-cambiar-titulo = Renommer
nota-md-fila-encima = Insérer une ligne au-dessus
nota-md-fila-debajo = Insérer une ligne en dessous
nota-md-quitar-fila = Supprimer la ligne
nota-md-columna-izquierda = Insérer une colonne à gauche
nota-md-columna-derecha = Insérer une colonne à droite
nota-md-quitar-columna = Supprimer la colonne
nota-md-quitar-tabla = Supprimer le tableau
nota-md-boton-fila-mas = + Ligne
nota-md-boton-fila-menos = − Ligne
nota-md-boton-columna-mas = + Colonne
nota-md-boton-columna-menos = − Colonne
nota-md-meses = janv. févr. mars avr. mai juin juil. août sept. oct. nov. déc.
## Note tables: merge and colour cells (H12)
nota-md-boton-combinar = Fusionner
nota-md-boton-color = Couleur
nota-md-combinar-celdas = Fusionner les cellules
nota-md-separar-celdas = Annuler la fusion
nota-md-color-fondo = Couleur de fond
nota-md-color-letra = Couleur du texte
nota-md-aplicar-a = Cellules|Toute la ligne|Toute la colonne
nota-md-nombres-fondo = Sans couleur|Rouge|Vert|Bleu|Jaune
nota-md-nombres-letra = Automatique|Rouge|Vert|Bleu|Orange
nota-md-pista-color = Lignes et colonnes entières : clic droit
nota-md-imagen-no-copiada = L’image n’a pas pu être copiée à côté de la note.

## Note comments (H12, Sep 30): the panel on the right
nota-md-com-titulo = Commentaires
nota-md-com-comentar = Commenter
nota-md-com-ninguno = Aucun commentaire
nota-md-com-pista = Sélectionnez du texte et appuyez sur Commenter (Ctrl+Alt+M)
nota-md-com-sin-ancla = Leur texte n’est plus dans la note
nota-md-com-responder-pista = Répondre…
nota-md-com-responder = Répondre
nota-md-com-guardar = Enregistrer
nota-md-com-cancelar = Annuler
nota-md-com-editar = Modifier
nota-md-com-borrar = Supprimer
nota-md-com-resuelto = résolu
nota-md-com-ver-resueltos = Afficher les résolus
nota-md-com-ocultar-resueltos = Masquer les résolus
nota-md-com-editado = modifié
nota-md-com-borrar-hilo = Supprimer le commentaire et ses réponses ?

## Markdown notes: images and live pages (H12)
nota-md-pagina-viva = Page d’un projet
nota-md-enlace-hoja = Lien vers une feuille
nota-md-captura = Capture d’écran
nota-md-ver-grande = Afficher en taille réelle
nota-md-abrir-hoja = Ouvrir la feuille
nota-md-foto-pequena = Petite
nota-md-foto-mediana = Moyenne
nota-md-foto-grande = Grande
nota-md-foto-columna = Largeur de colonne
nota-md-quitar-foto = Retirer l’image
nota-md-hoja-borrada = La feuille n’existe plus : ceci est la dernière copie
nota-md-otros-proyectos = Autres projets
nota-md-sin-hojas = Aucune feuille de projet à ajouter à la note pour l’instant.
nota-md-solo-imagenes = Seules des images peuvent être déposées ici.
chat-insertar-en-nota = Insérer dans une note

## Canvas: weld vertices and number points (lienzo-geometria, 26-sep)
herramientas-nudo = Souder des sommets
herramientas-bolita = Boule : balayez pour sélectionner
lienzo-punto-serie = Étiquettes des points
lienzo-cota-pedir = Demander la longueur au tracé
lienzo-panel-dureza = Dureté du crayon
lienzo-guia-corto = Guide
lienzo-panel-guias = Lignes guides
lienzo-cota-titulo = Quelle longueur et quelle direction ? (Tab change de champ)
lienzo-cota-largo = Longueur
lienzo-cota-angulo = Angle

## Canvas: image (zone, magnifier, pixelate, share and reference; Sep 26)
lienzo-referencia-menu = Image de référence…
lienzo-pegar-menu = Coller
lienzo-pista-compartir = Partager… (Ctrl+Maj+S)
lienzo-pista-lupa = Loupe (Q) : sur le canevas, touchez une forme fermée pour montrer ce qu’il y a dessous, agrandi ; à l’écran, zoome sous le curseur
lienzo-pista-mosaico = Pixeliser (mosaïque) : faites glisser sur ce que vous voulez masquer
lienzo-pista-zona = Zone (Z) : tracez un cadre pour obtenir une copie rognée déplaçable
lienzo-panel-mosaico = Masque
lienzo-panel-aumento = Grossissement
lienzo-panel-guia = Pointeur
# The spotlight (Propiedad.OSCURECER and Propiedad.ZONA on the phone)
lienzo-panel-oscurecer = Assombrir
lienzo-panel-zona-foco = Zone éclairée
lienzo-pista-foco = Projecteur (F) : touchez une forme fermée et la zone autour s’assombrit, dans son cadre
lienzo-pista-serie = Étapes numérotées : chaque clic place le numéro suivant

## The Chat / Functions switch in the chat title bar
proyectos-interruptor-chat = Chat
proyectos-interruptor-funciones = Fonctions
## The Functions screen: its tiles, in order
funciones-titulo = Fonctions
funciones-grabar = Enregistrer une réunion
funciones-grabar-detalle = Cours ou réunion, avec captures et transcription
funciones-lienzo = Nouveau canevas
funciones-lienzo-detalle = Un canevas vierge pour dessiner ou décalquer
funciones-tareas = Tâches
funciones-tareas-detalle = Les listes de tâches de tous vos chats
funciones-timeline = Chronologie
funciones-timeline-detalle = Votre journée minute par minute, et vos cours
funciones-horario = Emploi du temps
funciones-horario-detalle = Vos cours de la semaine, avec un rappel avant
funciones-galeria = Captures
funciones-galeria-detalle = Toutes les captures que vous avez enregistrées
funciones-figuras = Formes
funciones-figuras-detalle = Votre bibliothèque de formes pour les canevas
figuras-mias = Mes formes
figuras-todas = Toutes
figuras-vacia = Aucune forme pour l’instant. Sur un canevas, sélectionnez quelque chose puis « Enregistrer comme forme ».
figuras-borrar = Supprimer la forme
figuras-borrar-seguro = Appuyez à nouveau pour supprimer
# El boton de una figura que vino de PinDesk Plantillas; $app es su nombre.
figuras-abrir-en-plantillas = Ouvrir dans { $app }
figuras-elegir-para = Choisissez une forme pour « { $chat } »
figuras-adjuntar-a = Joindre à « { $chat } »
figuras-adjuntar-pista = Ou double-cliquez sur la forme, ou appuyez sur Entrée.
figuras-adjuntada = Jointe à « { $chat } » ✓
figuras-param-titulo = Paramètres
figuras-param-por-hoja = Par feuille
figuras-param-ano = Année
figuras-param-restablecer = Réinitialiser
figuras-zoom-pista = Pincez ou Ctrl + molette pour zoomer · double-clic pour ajuster
asistente-adj-figura = Forme
asistente-adj-pedir-figura = Demander une nouvelle forme
asistente-pide-figura = Fais-moi une forme de
asistente-mejora-figura = Améliore la forme « { $figura } » :
chat-guardar-en-figuras = Enregistrer dans mes formes
chat-figura-sin-nombre = Forme de { $chat }
chat-figura-guardada = Enregistrée dans vos formes sous « { $nombre } »
chat-figura-vacia = Ce canevas est vide : rien à enregistrer
figuras-titulo = Formes
figuras-buscar = Rechercher une forme…
# The ☀/☾ switch of the shape panels: day templates or their «(night)» twins.
figuras-sin-resultados = Aucune forme ne contient cela dans son nom ou ses textes.
figuras-de-serie = intégrée
figuras-tuya = la vôtre
figuras-mide = Taille
figuras-piezas = Pièces
figuras-textos = Textes qu’elle contient
figuras-lineas = { $n } lignes
figuras-formas = { $n } formes
figuras-de-texto = { $n } textes
figuras-trazos = { $n } traits à main levée
figuras-otras = { $n } autres
bandeja-figuras = Formes…

## --- Canvas: graph and table dialogs, put into a cell (F12/F14) ---
cajetin-cancelar = Annuler
grafica-insertar = Insérer
figuras-guardar = Enregistrer
figuras-editar-tabla = Modifier le tableau… (Entrée)
figuras-meter-en-celda = Placer la sélection dans sa cellule
tabla-blanco-titulo = Tableau vierge
tabla-pegada-titulo = Tableau collé
tabla-editar-titulo = Modifier le tableau
tabla-insertar = Insérer
tabla-aplicar = Appliquer
tabla-mas-fila = + Ligne
tabla-menos-fila = − Ligne
tabla-mas-columna = + Colonne
tabla-menos-columna = − Colonne
tabla-cabecera = En-tête
tabla-pegar = Coller
tabla-ayuda = Tab et flèches : autre cellule · − supprime la ligne ou la colonne de la cellule active · Entrée : valider · Échap : annuler

## --- Canvas: the Zone to the project chat (F8, mandarLaZona on the phone) ---
zona-arrastra-copia = Faites glisser : une copie sort
zona-arrastra-chat = Faites glisser : elle va dans le chat du projet
zona-al-chat = Vers le chat
zona-mandada = Zone envoyée au chat « { $proyecto } »
zona-no-mandada = Impossible d’envoyer la zone
zona-viene-pdf = PDF « { $proyecto } » → page { $pagina }
zona-viene-lienzo = Canevas « { $nombre } »
zona-nombre-pagina = Zone de la page { $pagina }
zona-nombre-lienzo = Zone de { $nombre }
zona-lienzo = canevas
zona-hoja-lienzo = Canevas
# Linked zone that leads to no canvas (salto_por_enlace).
zona-enlace-roto = Cette zone ne mène à aucun canevas de ce projet

## The canvas toolbar grouped like the phone's (GRUPOS_DE_FABRICA)
herramientas-mano = Sélectionner et déplacer
herramientas-imprimir = Imprimer
herramientas-compartir = Partager
barra-grupo-elegir = Autres façons de sélectionner
barra-grupo-trazar = Main levée
barra-grupo-formas = Formes
barra-grupo-flechas = Flèches et lignes
barra-grupo-arreglar = Corriger le dessin
barra-grupo-nombrar = Texte et étiquettes
barra-grupo-tapar = Masquer et pointer
barra-grupo-medir = Mesurer
barra-grupo-marco = Cadre et image
barra-grupo-sacar = Partager et imprimer
barra-pista-mano = Sélectionner et déplacer (M)
barra-pista-deshacer = Annuler (Ctrl+Z)
barra-pista-rehacer = Rétablir (Ctrl+Y)
barra-pista-salir = Quitter (Échap)
ajustes-herramientas-sueltas = Toujours visibles
ajustes-herramientas-donde = Outils dans
ajustes-herramientas-donde-ayuda = Choisissez un endroit pour y désactiver les outils que vous n’utilisez pas, sans les retirer ailleurs.
ajustes-herramientas-sitio-todos = Partout
ajustes-herramientas-sitio-pantalla = Écran
ajustes-herramientas-sitio-pin = Épingles
ajustes-herramientas-sitio-lienzo = Canevas
ajustes-herramientas-sitio-lector = Lecteur

## PDF reader: «To project» (the notes move to a project's sheets and travel when syncing)
lector-al-proyecto = Vers un projet
lector-al-proyecto-hecho = C’est maintenant dans les projets, avec vos notes
lector-al-proyecto-fallo = Impossible de créer le projet

## «Save» in the viewers of files opened from outside (passing mode, Android 15a52aed)
guardar-de-paso = Enregistrer dans un chat
guardar-de-paso-guardados = Dans « Messages enregistrés »
guardar-de-paso-en = Dans « { $chat } »
guardar-de-paso-hecho = Enregistré dans « { $chat } », avec vos notes
guardar-de-paso-fallo = Impossible de l’enregistrer dans le chat

## Word and book reader: the four types and four weights of the phone (K16)
lector-letra-serif = Serif
lector-letra-sans = Sans
lector-letra-cursiva = Cursive
lector-grosor-fina = Fin
lector-grosor-negra = Noir

## Screen annotator: its pill (CapaPantalla.kt on the phone) and saving to «Saved messages»
anotador-rotulo-dibujando = Alt + double clic milieu : traverser
anotador-rotulo-atravesando = Alt + double clic milieu : dessiner à nouveau
anotador-globo-titulo = Écran annoté
anotador-globo-guardado = Enregistré dans Messages enregistrés
anotador-globo-fallo = Impossible d’enregistrer dans Messages enregistrés
anotador-globo-copiado = Copié dans le presse-papiers
# Screen annotator: click-through in the toolbar (2026-09-29)
anotador-boton-atravesar = Traverser
anotador-pista-atravesar = Traverser (Espace) : la souris agit sur ce qui est dessous et le dessin reste visible. Revenez avec la main sur la pastille ou avec Alt + double clic milieu

# Share sheet: a document PDF with its annotations or clean (2026-09-29)
compartir-con-anotaciones = Avec les annotations
compartir-con-anotaciones-detalle = Encre, marges et signets par-dessus le PDF ; désactivé, le PDF vierge

# Share sheet: a PDF web page as lines or as images (2026-09-30, Android v0.98.1)
compartir-texto-buscable = Texte interrogeable
compartir-texto-buscable-detalle = Pages en lignes avec leur texte ; désactivé, en images

# Secret call: who is calling and call back later (2026-09-30, Android v0.98.6)
chat-quien-llama = Qui appelle : { $nombre }
chat-quien-llama-barra = Qui appelle :
chat-quien-llama-teclas = Entrée : choisir l’heure · Échap annule
llamada-volver-en = Rappeler dans
llamada-vuelve-a-las = Il vous rappellera à { $hora }

## H12: editing without marks, the floating toolbar and the note font (Sep 30)
nota-md-texto-normal = Texte normal
nota-md-quitar-formato = Effacer la mise en forme
nota-md-enlace-pista = Collez ou saisissez un lien et appuyez sur Entrée
nota-md-emojis = Sourire|Rire|J’adore|Bien|J’aime|Je n’aime pas|Merci|Applaudissements|Fête|Fait|Non|Attention|Cœur|Feu|Étoile|Idée
nota-md-letra-texto = Police du texte
nota-md-letra-titulos = Police des titres
nota-md-tamano = Taille de police
nota-md-tamanos = Petite|Normale|Grande|Très grande
nota-md-solo-esta-nota = Uniquement dans cette note
nota-md-pista-tamano = Ctrl + molette la change d’un cran à la fois

# --- Notes: embedded documents, chat messages and audio (H12, Oct 1) ---
nota-md-documento = Document
nota-md-del-chat = Du chat
nota-md-audio = Audio
nota-md-abrir = Ouvrir
nota-md-quitar = Retirer de la note
nota-md-ir-al-mensaje = Aller au message dans le chat
nota-md-mensaje-borrado = Message supprimé
nota-md-no-esta = Absent de cet ordinateur
nota-md-pasar-a-texto = Transcrire
nota-md-pasando-a-texto = Transcription { $pct } %
nota-md-sin-mensajes = Le chat de ce projet n’a aucun message à insérer (une note autonome n’a pas de chat).
nota-md-nota-de-voz = Note vocale

## Export a note to Word (H12, 1-oct)
nota-md-exportar-word = Exporter vers Word…
nota-md-tipo-word = Document Word
compartir-word = Word

## Requests from other programs (Flow Launcher, scripts; docs/protocolo-pedidos.md)
pedido-tareas = Tâches
pedido-escrito = Écrit dans « { $proyecto } ».
pedido-al-lienzo = Empilé sur le canevas « { $lienzo } ».
microfono-abriendo = Ouverture du micro…
microfono-guardando = Enregistrement…
microfono-parar = Cliquez pour arrêter · Échap l’ignore
microfono-guardada = Note vocale enregistrée
microfono-convertir = Transformer en appel
microfono-listo = Terminé
microfono-cuando = À quelle heure dois-je vous appeler ?
microfono-en-15-min = Dans 15 min
microfono-en-1-hora = Dans 1 h
microfono-manana = Demain 9:00
microfono-poner = Programmer l’appel
microfono-volver = Retour
microfono-sonara = Il vous appellera : { $cuando }
microfono-puesta = C’est fait : il vous appellera { $cuando }
microfono-teclas = Saisissez l’heure (18:30) ou utilisez les flèches · Entrée valide
soltar-aqui = Déposez ici pour ajouter à « { $proyecto } »
soltar-anadiendo = Ajout…
soltar-anadido-uno = 1 élément ajouté au chat
soltar-anadidos = { $n } éléments ajoutés au chat
soltar-ninguno = Rien n’a pu être ajouté
chat-anadir-arrastrando = Ajouter par glisser-déposer…
pedido-lista-hecha = Liste « { $lista } » créée dans « { $proyecto } ».
pedido-tabla-hecha = Tableau « { $tabla } » créé dans « { $proyecto } ».
pedido-tarea-anadida = « { $tarea } » ajoutée à « { $lista } ».
pedido-tarea-hecha = « { $tarea } » marquée comme faite.
pedido-tarea-pendiente = « { $tarea } » est de nouveau en attente.
pedido-roto = Il manquait quelque chose à une demande d’un autre programme ; rien n’a été fait.
pedido-sin-proyecto = Ce projet n’est plus dans PinDesk.
pedido-sin-mensaje = Ce message n’est plus dans le chat.
pedido-no-es-lista = Ce message n’est pas une liste de tâches.
pedido-sin-tarea = La liste n’a pas de tâche numéro { $indice }.
pedido-tarea-vacia = Une tâche sans texte n’est pas ajoutée.
pedido-sin-fichero = Ce fichier n’est plus sur le disque.
pedido-fallo-disco = Impossible d’enregistrer ce qui a été demandé. Consultez le journal de PinDesk.
pedido-tarea-movida = « { $tarea } » déplacée vers « { $lista } ».
pedido-tarea-cambio = La liste a changé pendant le déplacement de la tâche ; rien n’a été touché. Recherchez-la à nouveau.
pedido-tarea-quitada = « { $tarea } » retirée de sa liste.
pedido-tarea-corregida = Tâche modifiée : « { $tarea } ».
pedido-lista-borrada = Liste « { $lista } » supprimée de « { $proyecto } ».
pedido-timeline-apuntado = Ajouté à la chronologie : « { $titulo } ».
pedido-sin-capturas = Aucune capture pour l’instant.
pedido-fuera-de-capturas = Ce fichier n’est pas dans le dossier des captures ; il n’est pas touché.
pedido-no-es-imagen = Ce fichier n’est pas une image.
pedido-imagen-no-se-pudo = Impossible de copier l’image : { $motivo }
pedido-sin-imagen = L’image { $ruta } est introuvable ; la tâche n’a pas été ajoutée.
pedido-nada-que-leer = Aucun texte reçu à lire à voix haute.
pedido-ocr-copiado = Texte copié ({ $n } caractères) : « { $texto } »
pedido-ocr-sin-texto = Aucun texte trouvé dans l’image.
pedido-ocr-no-disponible = L’OCR de Windows n’est pas installé : ajoutez une langue avec reconnaissance de texte dans Paramètres.
pedido-ocr-fallo = Impossible de lire le texte de l’image : { $motivo }

## Command line and pindesk:// links (Oct 15)
orden-titulo = PinDesk
orden-confirmar-cabecera = Un lien demande à PinDesk de :
orden-confirmar-pregunta = Le faire ?
orden-motivo-ficheros = Il ouvre ou envoie des fichiers de votre disque.
orden-motivo-asistente = Il interroge l’assistant, qui peut agir dans PinDesk.
orden-motivo-microfono = Il enregistre avec le micro.
orden-motivo-otros-aparatos = Il envoie des données à vos autres appareils.
orden-motivo-automatizacion = Il touche à une automatisation de l’assistant.
orden-motivo-borra = Il supprime quelque chose.
orden-motivo-cambia = Il modifie des données de PinDesk.
orden-prohibido-ruta-no-local = Un lien ne peut pas ouvrir des fichiers réseau ou des chemins hors de cet ordinateur. Rien n’a été fait.
orden-prohibido-crear-automatizacion = Un lien ne peut pas créer d’automatisations. Rien n’a été fait.
orden-no-se-pudo = PinDesk n’a pas pu le faire : { $motivo }
orden-sin-app = PinDesk n’est pas ouvert et n’a pas pu être démarré : { $motivo }
orden-sin-respuesta = PinDesk n’a pas répondu. S’il est bloqué, fermez-le et rouvrez-le.
orden-version = Ce PinDesk est plus ancien que la demande : mettez-le à jour.
orden-preguntar-titulo = Interroger l’assistant sur { $cuantos ->
    [one] { $nombre }
   *[other] { $cuantos } fichiers
}
orden-preguntar-enviar = Demander
orden-preguntar-cancelar = Annuler

## Capture gallery (2-oct)
galeria-titulo = Captures
galeria-vacia = Aucune capture pour l’instant. Celles que vous copiez avec Alt + glisser apparaîtront ici.
galeria-abrir-carpeta = Ouvrir le dossier
galeria-copiada = Copiée dans le presse-papiers
galeria-pineada = Épinglée
galeria-borrada = Déplacée dans la corbeille de PinDesk
galeria-no-se-pudo = Opération impossible : { $motivo }
galeria-conservada-aviso = Conservée : elle est dans Messages enregistrés et ne sera pas supprimée
bandeja-galeria-capturas = Galerie des captures…
# Lessons learned (3-oct, as on the phone)
chat-hacer-leccion = Créer une leçon
bandeja-leccion-nueva = Nouvelle leçon…
bandeja-tareas = ☑ Tâches…
bandeja-timeline = 🕒 Chronologie et leçons…

## Tareas (ventana)
tareas-titulo = Tâches
tareas-apuntar-boton = Ajouter
tareas-hechas = Faites ({ $n })
tareas-vacia = Aucune tâche pour l’instant. Écrivez ci-dessus et appuyez sur Entrée : elle va dans la Boîte de réception, d’où vous la déplacez vers son groupe.
tareas-cambio = La liste a changé entre-temps ; elle est maintenant à jour. Cochez à nouveau.
tareas-mover = Déplacer vers…
tareas-mover-titulo = Déplacer « { $tarea } » vers…
tareas-sin-grupos = Il n’y a pas encore d’autres groupes de tâches. Créez une liste de tâches dans un chat et elle apparaîtra ici.
tareas-movida = Déplacée vers { $lista }
tareas-imagen-no-esta = Cette image n’est pas encore sur cet ordinateur : elle arrivera avec la synchronisation.
tareas-pegar-no-imagen = Seuls des images ou du texte peuvent être collés dans une tâche.
tareas-pegar-repetida = Cette image est déjà présente en tant que { $ficha }.

# Opening a photo that Windows can only read with a Microsoft Store extension.
abrir-falta-extension = Impossible d’ouvrir { $nombre } : Windows a besoin de « { $extension } », gratuit dans le Microsoft Store. Installez-le et rouvrez le fichier.

## v2-menus
v2menus-responder = Répondre
v2menus-anotar-encima = Annoter par-dessus
v2menus-mas = Plus
v2menus-elegir-varios = Sélectionner plusieurs
v2menus-tecla-supr = Suppr
v2menus-tecla-clic = clic
v2menus-adj-buscar = Rechercher : tableau, chronomètre, PDF…
v2menus-adj-mas = PLUS
v2menus-adj-nada = Rien ne correspond à votre recherche
v2menus-adj-lienzo = Canevas
v2menus-adj-tareas = Liste de tâches
v2menus-adj-tabla = Tableau
v2menus-adj-conversacion = Conversation
v2menus-adj-captura = Capture d’écran
v2menus-adj-pagina = Une page de projet
v2menus-adj-proyecto = Projet entier
v2menus-adj-movil = Importer du téléphone
v2menus-adj-pegar-copiado = Coller ce qui a été copié
v2menus-envio-enviar = Envoyer
v2menus-envio-anadir = Ajouter
v2menus-envio-fotos = { $cuantos ->
    [one] 1 photo
   *[other] { $cuantos } photos
} · vers { $proyecto }
v2menus-envio-archivos = { $cuantos ->
    [one] 1 fichier
   *[other] { $cuantos } fichiers
} · vers { $proyecto }
## v2-ajustes
ajustes2-titulo = Paramètres
ajustes2-buscar = Rechercher dans les paramètres…
ajustes2-nota = Enregistré à la fermeture avec Terminé ou Échap.
ajustes2-nota-punto = = différent de la valeur par défaut.
ajustes2-deshacer = Annuler
ajustes2-listo = Terminé
ajustes2-restablecer = Réinitialiser { $seccion }
ajustes2-resultados = Résultats pour « { $busqueda } »
ajustes2-resultados-sub = { $n ->
    [one] 1 paramètre trouvé dans toutes les sections.
   *[other] { $n } paramètres trouvés dans toutes les sections.
  }
ajustes2-sin-resultados = Aucun paramètre ne mentionne « { $busqueda } ». Essayez un autre mot.
ajustes2-sec-general = Général
ajustes2-sec-general-sub = Le démarrage de PinDesk et ce qu’il fait des PDF dans le chat.
ajustes2-sec-captura = Capture
ajustes2-sec-captura-sub = Comment une capture est prise et ce qu’il en advient ensuite.
ajustes2-sec-lienzo = Canevas et outils
ajustes2-sec-lienzo-sub = L’aimant, les traits à main levée et les outils affichés dans la barre.
ajustes2-sec-voz = Voix et transcription
ajustes2-sec-voz-sub = La conversion en texte des notes vocales en deux langues.
ajustes2-sec-atajos = Raccourcis
ajustes2-sec-atajos-sub = Cliquez sur une case, puis appuyez sur les touches. Suppr retire le raccourci ; Échap annule.
ajustes2-sec-sincro = Synchronisation
ajustes2-sec-sincro-sub = Comment cet ordinateur trouve le téléphone.
ajustes2-sec-apps = Applications par défaut
ajustes2-sec-apps-sub = Ouvrir les images et vidéos avec PinDesk depuis l’Explorateur de fichiers.
ajustes2-sec-apariencia = Apparence
ajustes2-sec-apariencia-sub = L’aspect du chat.
ajustes2-sec-avanzado = Avancé
ajustes2-sec-avanzado-sub = Performances, le filtre du stylet et le fichier de paramètres.
ajustes2-g-arranque = Au démarrage
ajustes2-g-pdf = Documents PDF
# Text recognition (OCR) for the whole app: Windows or Indra (docs/ocr.md).
ajustes2-g-ocr = Reconnaissance de texte (OCR)
ajustes2-ocr-motor = Moteur OCR
ajustes2-ocr-windows = Windows
ajustes2-ocr-indra = Indra
ajustes2-ayuda-ocr = Utilisé par la capture de texte, « Décalquer ceci », les épingles, la galerie, les PDF numérisés et l’assistant. Windows est fourni avec le système. Indra (Datalab) lit bien mieux les photos, numérisations, pages inclinées et petits caractères, sur votre PC et hors ligne ; le choisir télécharge { $megas } Mo. Ses poids sont sous licence OpenRAIL-M de Datalab : gratuits pour un usage personnel, la recherche et les entreprises de moins de 20 M$ ; tout autre usage commercial nécessite leur licence.
ajustes2-ayuda-ocr-indra-listo = Indra est téléchargé et lit tout le texte de l’application. En cas d’échec, l’OCR de Windows est utilisé.
ajustes2-ayuda-ocr-windows-con-indra = L’OCR de Windows est utilisé. Indra est toujours téléchargé : choisissez-le pour l’utiliser, ou supprimez-le ci-dessous.
ajustes2-ocr-bajando = Téléchargement d’Indra… { $por-ciento } %. L’OCR de Windows lit en attendant.
ajustes2-ocr-fallo = Impossible de télécharger Indra : { $fallo }. Choisissez-le à nouveau pour réessayer ; l’OCR de Windows lit en attendant.
ajustes2-ocr-licencia = Licence d’Indra
ajustes2-ayuda-ocr-licencia = Le code d’Indra est sous Apache 2.0 ; ses poids, sous OpenRAIL-M modifiée de Datalab. PinDesk ne les fournit pas : vous les téléchargez depuis Hugging Face en choisissant Indra.
ajustes2-ocr-corregir = Corriger avec le dictionnaire
ajustes2-ayuda-ocr-corregir = Uniquement avec Indra et sur des pages en espagnol : corrige les accents et les lettres peu sûres. Ne touche jamais aux nombres, codes ou noms.
ajustes2-ocr-vocabulario = Mon vocabulaire
ajustes2-ocr-vocabulario-ninguno = Aucun. Noms, sigles ou termes qu’Indra doit privilégier (E030, Arequipa…).
ajustes2-ocr-vocabulario-marcador = terme, autre terme…
ajustes2-ocr-borrar = Supprimer Indra
ajustes2-ayuda-ocr-borrar = Libère ses 26 Mo sur le disque ; l’OCR de Windows reprend la lecture.
ajustes2-ocr-borrar-boton = Supprimer
# Transcribe with Whisper or Qwen3-ASR (25-oct-2026, docs/voz-qwen3.md)
ajustes2-voz-motor = Transcrire avec
ajustes2-voz-motor-whisper = Whisper base
ajustes2-voz-motor-qwen3 = Qwen3-ASR
ajustes2-ayuda-voz-motor = Pour les notes vocales et les enregistrements de cours ou de réunions. Whisper base est intégré (160 Mo, rapide). Qwen3-ASR comprend bien mieux l’espagnol, les réunions et les salles de classe (deux fois moins d’erreurs, voire moins) ; le choisir télécharge { $megas } Mo, nécessite environ { $ram } Go de mémoire pendant la transcription et prend environ quatre fois plus de temps que Whisper. Licence { $licencia }.
ajustes2-ayuda-voz-motor-qwen3-listo = Qwen3-ASR est téléchargé ({ $total } Mo, licence { $licencia }) et transcrit les notes et enregistrements. Il nécessite environ { $ram } Go de mémoire pendant la transcription ; en cas d’échec, Whisper est utilisé.
ajustes2-ayuda-voz-motor-whisper-con-qwen3 = Whisper transcrit. Qwen3-ASR est téléchargé : choisissez-le pour l’utiliser, ou supprimez-le ci-dessous.
ajustes2-ayuda-voz-motor-sin-memoria = Whisper transcrit. Qwen3-ASR nécessite un PC avec 4 Go de mémoire ou plus.
ajustes2-voz-motor-bajando = Téléchargement de Qwen3-ASR… { $por-ciento } %. Whisper transcrit en attendant.
ajustes2-voz-motor-fallo = Impossible de télécharger Qwen3-ASR : { $fallo }. Choisissez-le à nouveau pour réessayer ; Whisper transcrit en attendant.
ajustes2-voz-qwen3-licencia = Fiche du modèle Qwen3-ASR
ajustes2-ayuda-voz-qwen3-licencia = Qwen3-ASR-0.6B de Qwen (Alibaba), sous licence Apache-2.0 : libre d’utilisation, y compris au travail.
ajustes2-voz-qwen3-borrar = Supprimer Qwen3-ASR
ajustes2-ayuda-voz-qwen3-borrar = Libère ses 988 Mo sur le disque ; les notes sont de nouveau transcrites avec Whisper.
ajustes2-g-al-capturar = Lors de la capture
ajustes2-g-gif = Enregistrement GIF
ajustes2-g-guardar = Enregistrement et nettoyage
ajustes2-g-pila = La pile de captures
ajustes2-g-ignorados = Programmes ignorés
ajustes2-g-regiones = Régions enregistrées
ajustes2-g-iman = Aimant
ajustes2-g-trazo = Main levée
ajustes2-g-voz = Notes vocales
ajustes2-g-atajos = Raccourcis globaux
ajustes2-g-sincro = Sur le réseau local
ajustes2-g-apps = Ouverture des images et vidéos
ajustes2-g-tema = Thème
ajustes2-g-rendimiento = Performances
ajustes2-g-lapiz = Filtre du stylet (mode Naturel)
ajustes2-g-fichero = Le fichier de paramètres
ajustes2-ayuda-arranque = Reste dans la zone de notification sans ouvrir de fenêtre. Indisponible en mode portable.
ajustes2-ayuda-idioma = S’applique au prochain démarrage.
ajustes2-g-ventanas = Fenêtres
ajustes2-escala = Échelle de l’interface
ajustes2-ayuda-escala = La taille de tout le chat. S’applique immédiatement ; dans le chat, aussi avec Ctrl + « + », « - » et « 0 ».
ajustes2-grupos-ventanas = Groupes de fenêtres
ajustes2-ayuda-grupos-ventanas = Enregistrez les fenêtres ouvertes comme groupe et rouvrez-les ensemble.
ajustes2-ayuda-pdf-aligerar = Après son arrivée dans le chat, en arrière-plan.
ajustes2-pdf-nivel = Niveau d’allègement
ajustes2-ayuda-pdf-nivel = À l’arrivée, seuls Exact et Moyen sont utilisés ; Petit et Max. servent à « Alléger le PDF » à la main.
ajustes2-retardo = Capture différée
ajustes2-ayuda-retardo = Compte à rebours pour ouvrir un menu ou une info-bulle avant la capture.
ajustes2-color = Format de couleur
ajustes2-ayuda-color = Comment la pipette copie la couleur.
ajustes2-scroll = Hauteur maximale de défilement
ajustes2-ayuda-scroll = Limite des captures longues, pour qu’une page sans fin n’épuise pas la mémoire.
ajustes2-gif-ritmo = Images par seconde
ajustes2-ayuda-gif-ritmo = Moins = fichier plus léger. 10 ou 15 suffisent pour des tutoriels.
ajustes2-gif-retardo = Délai avant l’enregistrement
ajustes2-ayuda-gif-retardo = Secondes entre l’appui sur « Enregistrer » et la première image.
ajustes2-carpeta = Dossier des captures
ajustes2-abrir = Ouvrir
ajustes2-caducidad = Jours avant suppression
ajustes2-ayuda-caducidad = Ensuite, elles vont dans la Corbeille de Windows, sauf celles conservées.
ajustes2-apilar = Empiler dans le coin
ajustes2-ayuda-apilar = Chaque capture laisse une case dans le coin ; cliquez dessus pour rassembler les suivantes et les coller toutes d’un seul Ctrl+V.
ajustes2-esquina = Coin de la case
ajustes2-ayuda-esquina = Sur l’écran où la capture a été prise.
ajustes2-icono = Durée à l’écran
ajustes2-ayuda-icono = Combien de temps la case non activée reste après la dernière capture.
ajustes2-ayuda-ignorado = Tant que ce programme est au premier plan, les raccourcis de PinDesk ne font rien.
ajustes2-anadir-programa = Ajouter un programme
ajustes2-ayuda-anadir-programa = Saisissez le nom de l’exécutable et appuyez sur Entrée.
ajustes2-anadir = Ajouter
ajustes2-ayuda-region = { $x }, { $y } · { $ancho } × { $alto } px
ajustes2-ayuda-iman = Pendant le dessin, le curseur s’aimante à ce qui existe déjà.
ajustes2-ayuda-iman-esquinas = Coins des formes et extrémités des lignes.
ajustes2-ayuda-iman-medios = Le milieu de chaque côté.
ajustes2-ayuda-iman-centros = Le centre de chaque forme.
ajustes2-iman-radio = Distance d’aimantation
ajustes2-ayuda-iman-radio = En pixels d’écran.
ajustes2-suavizado = Lissage du trait
ajustes2-ayuda-suavizado = Naturel supprime le tremblement d’une main immobile sans retarder la pointe.
ajustes2-ayuda-voz-segundo = Whisper comprend aussi cette langue dans la même note.
ajustes2-ayuda-voz-modo = Que faire de ce qui est dit dans la deuxième langue.
ajustes2-presencia = Laisser le téléphone me trouver
ajustes2-ayuda-presencia = Le téléphone voit cet ordinateur dès qu’il ouvre Synchronisation. Désactivé, aucun port n’est ouvert sur le réseau.
ajustes2-lo-mio = Le mien l’emporte
ajustes2-ayuda-lo-mio = Lors de la synchronisation, cet ordinateur l’emporte : seul ce qui n’existe pas ici vient de l’autre côté.
ajustes2-vista-rapida = Aperçu rapide avec la barre d’espace
ajustes2-ayuda-vista-rapida = Dans l’Explorateur de fichiers ou sur le bureau, sélectionnez des fichiers et appuyez sur Espace : ils s’ouvrent aussitôt dans PinDesk (photos, vidéos, PDF, Word, plans DWG, modèles IFC et Revit…). Espace à nouveau le ferme.
ajustes2-abrir-con = Proposer PinDesk pour les images et vidéos
ajustes2-ayuda-abrir-con = Apparaît dans « Ouvrir avec » et dans Applications par défaut. Le désactiver retire ce qui a été écrit dans le registre.
ajustes2-predeterminada = Application par défaut
ajustes2-ayuda-predeterminada = Windows ne permet de la choisir que dans Paramètres : là, PinDesk → Définir par défaut.
ajustes2-hacer-predeterminada = Faire de PinDesk l’application par défaut…
ajustes2-ayuda-cosmos = Ciel étoilé derrière la liste des projets du chat. S’applique à partir du prochain chat.
ajustes2-nivel = Niveau de performances
ajustes2-ayuda-nivel = Automatique évalue l’ordinateur au démarrage. S’applique au prochain démarrage.
ajustes2-medir = Mesurer les images de l’éditeur
ajustes2-ayuda-medir = Journalise la durée de chaque image. Uniquement pour le diagnostic.
ajustes2-ritmo = Peindre juste avant le rafraîchissement
ajustes2-ayuda-ritmo = La pointe du stylet traîne moins ; un calcul manqué perd un rafraîchissement.
ajustes2-paneo = Défiler sans redessiner
ajustes2-ayuda-paneo = Le compositeur de Windows déplace et zoome le canevas. Désactivé, chaque image est redessinée.
ajustes2-corte = Coupure avec le stylet immobile
ajustes2-ayuda-corte = Plus bas = moins de tremblement et plus de retard au départ. Auto = 1 Hz.
ajustes2-beta = Suivi de la vitesse
ajustes2-ayuda-beta = Plus haut = pointe plus proche du curseur et plus de tremblement. Auto = 0,007.
ajustes2-fichero = Fichier de paramètres
ajustes2-ayuda-fichero = Ce qui n’est pas ici y figure toujours, avec ses commentaires. Les modifications manuelles s’appliquent après un redémarrage.
ajustes2-abrir-fichero = Ouvrir le fichier
ajustes2-u-s = { $n } s
ajustes2-u-px = { $n } px
ajustes2-u-dias = { $n ->
    [one] 1 jour
   *[other] { $n } jours
  }
ajustes2-u-hz = { $n } Hz
ajustes2-nunca = Jamais
ajustes2-siempre = Toujours
ajustes2-auto = Auto
## v2-buscar
comando-buscar = 🔍 Rechercher dans PinDesk…
comando-grabar-clase = 🎙 Enregistrer un cours ou une réunion…
clase-titulo = Enregistrer un cours ou une réunion
clase-microfono = Micro
clase-equipo = Son de l’ordinateur (Meet, Zoom…)
clase-guardar-en = Enregistrer dans : { $proyecto }
clase-empezar = Démarrer l’enregistrement
clase-nada = Choisissez le micro, le son de l’ordinateur ou les deux
clase-captura = Capturer
clase-terminar = Terminer
clase-capturas = Captures : { $n } · un clic répète la zone
clase-elige-zona = Choisissez la zone à l’écran…
clase-captura-hecha = Capture enregistrée à { $minuto }
clase-zona-cambiada = Zone modifiée
clase-en-pausa = En pause : rien n’est enregistré
clase-tirar = Appuyez à nouveau sur ✕ pour ignorer l’enregistrement
clase-sin-zona = Impossible de capturer la zone
clase-abriendo = Ouverture de l’audio…
clase-guardando = Enregistrement du cours…
clase-texto = Transcription… { $avance } %
clase-bajando = Téléchargement de la reconnaissance (première fois seulement)… { $avance } %
clase-lista = Enregistré dans « { $proyecto } »
clase-lista-con-texto = Avec { $n } captures et la transcription. Ouvrez-le dans le chat pour la voir à côté des images.
clase-lista-sin-texto = Avec { $n } captures. Pour le texte, appuyez sur « Convertir en texte » sur la note dans le chat.
clase-sin-texto = Le laisser sans texte
clase-listo = Terminé
clase-nombre = Cours { $fecha }
clase-sin-sonido-equipo = On n’entend rien de l’ordinateur : aucun haut-parleur ni casque n’est actif
clase-video = Vidéo de cet écran
clase-video-roto = La vidéo s’est arrêtée ; l’audio continue d’être enregistré
clase-video-guardado = Avec la vidéo de l’écran.
clase-calidad-ligero = Légère
clase-calidad-normal = Normale
clase-calidad-nitido = Nette
clase-calidad-ligero-pista = Vidéo 720p à 10 i/s : les plus petits fichiers (environ 180 Mo par heure)
clase-calidad-normal-pista = Vidéo 1080p à 15 i/s : environ 300 Mo par heure au maximum
clase-calidad-nitido-pista = L’écran tel quel à 30 i/s : les petits textes restent lisibles, fichiers plus gros
clase-video-con = Vidéo avec { $codificador }
clase-codificador-grafica = la carte graphique (sollicite à peine le processeur)
clase-codificador-software = le processeur
clase-codificador-cpu = le processeur, mode simple
clase-zona-perdida = Aucune zone choisie : appuyez à nouveau sur « Capturer »
clase-salvado = Le reste a été enregistré : { $n } captures
clase-disco-poco = Le disque manque d’espace (moins de 2 Go)
clase-disco-lleno = Disque presque plein : la vidéo a été fermée et l’audio continue d’être enregistré
clase-suspendido = L’ordinateur s’est mis en veille pendant { $minutos } min ; l’enregistrement a repris au réveil
clase-recuperar-titulo = Enregistrement interrompu
clase-recuperar-pregunta =
    Un enregistrement a été interrompu ({ $nombre }, { $minutos } min) dans « { $chat } ».
    Oui : Récupérer · Non : Ignorer · Annuler : redemander au prochain démarrage de PinDesk
clase-recuperar-sin-nada = Il ne reste de « { $nombre } » qu’une vidéo impossible à ouvrir. La supprimer ?
clase-recuperar-descartar = Ignorer « { $nombre } » ? L’enregistrement sera supprimé et ne pourra pas être récupéré.
clase-recuperar-fallo = « { $nombre } » n’a pas pu être récupéré. L’enregistrement reste en lieu sûr et sera proposé à nouveau à l’ouverture de PinDesk.
clase-texto-de-fondo = Fermer (continue)
clase-abrir-para-estudiar = Ouvrir pour étudier
clase-camara-movil = 📱 Caméra du téléphone
clase-camara-movil-n = 📱 Caméra du téléphone · { $n }
clase-camara-movil-apagada = 📱 Caméra du téléphone : désactivée
clase-foto-movil = Photo du téléphone · { $minuto }
clase-clip-movil = Clip du téléphone · { $minuto }
camara-movil-tarde = Du téléphone vers « { $titulo } » · { $minuto }
# Photos by their time, from the PC to the phone's recording and collaborators (28-oct-2026).
aporte-captura-al-movil = Capture pour l’enregistrement du téléphone
aporte-de-la-grabacion = { $movil } enregistre · { $minuto }
aporte-enviada = Envoyée au téléphone · { $minuto }
aporte-pendiente = Hors ligne : sera envoyée au retour de la connexion
aporte-fallo = Impossible de capturer l’écran
chat-adjuntar-a-la-grabacion = Joindre à l’enregistrement…
chat-adjuntada-a-la-grabacion = Dans l’enregistrement · { $minuto }
chat-sin-grabacion = Aucun enregistrement dans ce chat
chat-colaboradores-todos = Permettre à tout le chat d’ajouter des photos
chat-colaboradores-nadie = Seulement mes photos dans cet enregistrement
clase-hora-sin-comprobar = heure non vérifiée
baliza-movil-aislado = Votre téléphone n’apparaît pas, bien que vous soyez sur le même réseau : ce Wi-Fi isole peut-être les appareils (c’est le cas des réseaux invités, d’hôtel et de bureau) ou un pare-feu bloque PinDesk. Connectez le PC au point d’accès du téléphone, ou les deux à un autre Wi-Fi. Les photos prises attendent sur le téléphone et s’insèrent à leur minute dès qu’ils se retrouvent.
baliza-movil-otra-red = Votre téléphone est sur un autre réseau que ce PC. Connectez les deux au même Wi-Fi (ou le PC au point d’accès du téléphone). Les photos prises attendent sur le téléphone et s’insèrent à leur minute dès qu’ils se retrouvent.
# In-person meeting (27-oct-2026): the computer's camera, photos and clips.
clase-camara = Caméra : { $camara }
clase-camara-buscando = Recherche des caméras…
clase-camara-ninguna = Aucune caméra : Photo et Clip ne fonctionneront pas (l’audio est enregistré quand même)
clase-foto = Photo
clase-foto-hecha = Photo enregistrée à { $minuto }
clase-camara-abriendo = Ouverture de la caméra…
clase-camara-lista = Caméra prête : photos en { $medida }
clase-camara-sin = Aucune caméra connectée
clase-camara-en-uso = Une autre application utilise la caméra (Teams, Zoom…) : fermez-la et réessayez
clase-camara-sin-permiso = Windows n’autorise pas l’accès à la caméra : activez-le dans Paramètres › Confidentialité › Caméra
clase-camara-fallo = La caméra n’a pas répondu ; réessayez
clase-clip-empezado = Enregistrement d’un clip avec la caméra… appuyez sur ■ pour l’arrêter
clase-clip-guardado = Clip enregistré à { $minuto }
clase-clip-corto = Impossible de finaliser le clip
# A single recording (29-oct-2026): no modes; Photo, Capture, Screen and Clip on the bar,
# the clock that pauses and ends, and full screen with the topics and the transcript.
clase-unica-sub = Toujours le son ; dans la barre, Photo, Capturer, Écran et Clip
clase-pantalla-al-empezar = Enregistrer l’écran dès le début
clase-pantalla = Écran
clase-clip = Clip
clase-pantalla-encendida = Enregistrement de l’écran ; l’audio continue
clase-pantalla-apagada = Écran désactivé ; l’audio continue d’être enregistré
clase-pista-unica = Cliquez sur l’horloge : pause · maintenez-la : terminer · maintenez « Capturer » : une zone
clase-imagenes = { $n } images et clips
clase-completa-temas = Ce que je vais dire
clase-completa-sin-temas = Aucun sujet : ajoutez-les dans « Sujets de la réunion » avant de commencer
clase-completa-dice = Ce qui se dit
clase-completa-sin-frases = À l’écoute… les phrases apparaissent ici dès qu’elles sont dites
clase-completa-sin-escucha = La transcription en direct nécessite la clé Jev (TypeSafe) et la reconnaissance vocale ; le texte complet arrive à la fin
clase-descartar = Ignorer
clase-descartar-otra = Appuyez à nouveau sur « Ignorer » pour supprimer l’enregistrement
estudiar-clip = Vidéo de la caméra · { $duracion }
bandeja-grabando = PinDesk — Enregistrement…
salir-grabando-titulo = Une réunion est en cours d’enregistrement
salir-grabando-mensaje =
    Terminer et enregistrer l’enregistrement avant de quitter ?
    Oui : terminer, l’enregistrer dans son chat et quitter (demander le texte plus tard sur la note).
    Non : continuer l’enregistrement.
chat-estudiar-clase = Étudier le cours
estudiar-titulo = Étudier le cours
estudiar-transcripcion = Transcription
estudiar-capturas = Captures
estudiar-marcadores = Signets
estudiar-marcador = Signet
estudiar-seguir = Suivre
estudiar-pista-teclas = Espace lit · ←/→ 5 s · J/L 10 s · M signet
estudiar-nota-del-marcador = Saisissez une note et appuyez sur Entrée (Échap annule)
estudiar-sin-marcadores = Appuyez sur « Signet » ou M pour noter quelque chose à cette minute.
estudiar-marcador-sin-nota = Signet
estudiar-repasar = Réviser à voix haute
repaso-titulo = Réviser à voix haute
repaso-cabecera = Réviser à voix haute · { $clase }
repaso-explicacion = Expliquez le cours avec vos propres mots, comme si vous le racontiez à quelqu’un. Jev marque chaque point clé : ✓ couvert, ✗ erroné, · manquant, ? flou. Ce qui est erroné ou manquant affiche sa minute pour revenir au cours.
repaso-sin-puntos = Ce cours n’a pas encore de points clés. Ils viennent de ce que Jev marque comme important lors de la transcription, des signets avec note (M) et des sujets abordés.
repaso-recuento = { $cubiertos } sur { $total } couverts · { $mal } erronés · { $faltan } manquants · { $dudosos } flous
repaso-preparando = Préparation du micro…
repaso-te-escucho = À l’écoute : commencez à expliquer.
repaso-pulsa-hablar = Appuyez sur « Commencer à parler » (ou Espace) et expliquez le cours.
repaso-hablar = Commencer à parler
repaso-parar = Arrêter
repaso-otra-vez = Recommencer
repaso-cerrar = Fermer
repaso-jev-fallo = Jev n’a pas répondu : les points restent tels quels.
estudiar-marcador-guardado = Signet enregistré
estudiar-no-se-guardo = Impossible de l’enregistrer
estudiar-sin-texto = Pas encore de transcription : appuyez sur « Convertir en texte » sur la note dans le chat et elle apparaîtra ici d’elle-même.
estudiar-sin-capturas = Ce cours n’a pas de captures.
estudiar-sin-video = Cette vidéo ne peut pas être lue ici ; la note vocale est lue à la place
estudiar-sin-sonido = L’audio de ce cours est introuvable
estudiar-solo-audio = Audio seul
estudiar-cargando = Chargement…
estudiar-ver-video = Afficher la vidéo
estudiar-ver-captura = Afficher la capture
estudiar-anotar = Annoter
estudiar-ver-texto = Texte
estudiar-fotograma-guardado = Image enregistrée comme capture : annotez-la sur le canevas
estudiar-nada-que-anotar = Rien à annoter ici pour l’instant
estudiar-clip-movil = Vidéo du téléphone · { $duracion }
estudiar-sin-hevc = Pour lire ce clip, installez « Extensions vidéo HEVC » depuis le Microsoft Store
anki-enviar = Envoyer vers Anki
anki-pregunta-clase = Qu’est-ce qui a été expliqué ici ?
anki-nada = Aucune carte ici. Écrivez-en une par ligne sous la forme « question :: réponse », ou « Q: … » et « R: … », ou avec des trous {"{{"}c1::comme ceci{"}}"}.
anki-hecho = { $n } cartes dans Anki, paquet « { $mazo } »
anki-ya-estaban = Ces cartes étaient déjà dans « { $mazo } »
anki-fichero = Anki n’est pas ouvert (ou AnkiConnect est absent). J’ai enregistré { $n } cartes dans { $donde } : dans Anki, Fichier → Importer.
anki-fallo = Anki ne les a pas acceptées : { $motivo }
# Sharing a recording (class or meeting): the HTML page and its formats.
compartir-anki = Anki
compartir-tipo-grabacion = Enregistrement
grabacion-idioma = fr
grabacion-fecha = { $dia }/{ $mes }/{ $ano } { $hora }
grabacion-transcripcion = Transcription
grabacion-capturas = Captures
grabacion-captura = Capture
grabacion-marcadores = Signets
grabacion-marcador = Signet
grabacion-duracion = Durée
grabacion-sin-texto = Cet enregistrement n’a pas encore de transcription.
grabacion-buscar = Rechercher dans la transcription
grabacion-seguir = Suivre l’audio
grabacion-reproducir = Lecture ou pause
grabacion-velocidad = Vitesse
grabacion-ir-al-minuto = Aller à ce moment
grabacion-cerrar = Fermer
grabacion-audio = Audio
grabacion-video = Vidéo
buscar-todo-titulo = Rechercher dans PinDesk
buscar-todo-pista = Rechercher fichiers, tâches, leçons, captures… ou tapez une lettre
buscar-todo-pestana-todo = Tout
buscar-todo-pestana-archivos = Fichiers
buscar-todo-pestana-tareas = Tâches
buscar-todo-pestana-lecciones = Leçons
buscar-todo-pestana-capturas = Captures
buscar-todo-pestana-acciones = Actions
buscar-todo-cambia = changer
buscar-todo-resultados = { $n ->
    [one] 1 résultat
   *[other] { $n } résultats
  }
buscar-todo-ninguno = Rien pour « { $consulta } »
buscar-todo-vacia = Rien ici pour l’instant
buscar-todo-ninguno-pista = Essayez moins de lettres, ou cherchez dans un autre onglet (Tab)
buscar-todo-recientes = Recherches récentes
buscar-todo-borrar-todas = Tout effacer
buscar-todo-abierto-hace-poco = Ouverts récemment
buscar-todo-letras = Tapez une lettre et un espace
buscar-todo-mejor = Meilleur résultat
buscar-todo-grupo-tareas = Tâches et leçons
buscar-todo-grupo-archivos = Fichiers
buscar-todo-grupo-capturas = Captures
buscar-todo-grupo-acciones = Actions
buscar-todo-n-de-m = { $n } sur { $total }
buscar-todo-pegar = Collez une image avec
buscar-todo-pegar-2 = et elle accompagne ce que vous tapez :
buscar-todo-pegar-ejemplo = t vérifier fissure
buscar-todo-letra-t = Nouvelle tâche
buscar-todo-letra-t-sub = vers la Boîte de réception · « t acheter du ciment »
buscar-todo-letra-n = Nouvelle note
buscar-todo-letra-n-sub = dans Messages enregistrés · « n idée @projet »
buscar-todo-letra-l = Nouveau canevas
buscar-todo-letra-l-sub = vierge · « l plan @projet »
buscar-todo-letra-g = Galerie des captures
buscar-todo-letra-g-sub = « g mur » cherche dedans
buscar-todo-letra-c = Capturer
buscar-todo-letra-c-sub = une zone de l’écran
buscar-todo-letra-u = Épingler la dernière capture
buscar-todo-letra-u-sub-nada = aucune capture pour l’instant
buscar-todo-letra-a = Nouvelle leçon
buscar-todo-letra-a-sub = ce que vous avez appris · « a vérifier l’échelle »
buscar-todo-accion-pinear = Épingler
buscar-todo-accion-abrir = Ouvrir
buscar-todo-accion-apuntar = Ajouter
buscar-todo-accion-capturar = Capturer
buscar-todo-accion-crear = Créer
buscar-todo-accion-grabar = Enregistrer
buscar-todo-accion-enviar = Envoyer
buscar-todo-accion-copiar = Copier
buscar-todo-accion-reproducir = Lire
buscar-todo-accion-hecha = Fait
buscar-todo-accion-desmarcar = Annuler « fait »
buscar-todo-accion-entrar = Entrer
buscar-todo-accion-mostrar = Afficher
buscar-todo-accion-pegar = Coller
buscar-todo-ver-chat = Afficher dans le chat
buscar-todo-mas = Plus d’actions
buscar-todo-pie-moverse = naviguer
buscar-todo-pie-abrir = ouvrir
buscar-todo-pie-adjuntar = joindre une image
buscar-todo-pie-cerrar = fermer
buscar-todo-pie-borrar = effacer
buscar-todo-pie-mas = plus d’actions
buscar-todo-pie-volver = retour
buscar-todo-pie-flow = Pareil dans Flow Launcher avec « p »
buscar-todo-abrir-con = Ouvrir avec…
buscar-todo-borrar = Supprimer (annulable)
buscar-todo-medidas = Capture · { $ancho } × { $alto } · { $peso }
buscar-todo-caduca = Expire dans { $dias ->
    [one] 1 jour
   *[other] { $dias } jours
  }
buscar-todo-caduca-hoy = Expire aujourd’hui
buscar-todo-conservada = Conservée : n’expire jamais
buscar-todo-conservada-ya = Conservée
buscar-todo-conservar = Conserver
buscar-todo-borrada = Capture supprimée
buscar-todo-deshacer = Annuler
buscar-todo-copiado = Copié
## v2-lecciones
# The lessons window of the v2 redesign (Lecciones2.dc.html and Lecciones2-repaso.dc.html).
## v2-galeria
galeria-subtitulo = { $cuantas } · conservées 7 jours
galeria-buscar = Rechercher, y compris le texte des captures
galeria-buscar-sin-ocr = Rechercher par nom ou date
galeria-elegir = Sélectionner
galeria-sin-resultados = Aucune capture avec « { $texto } ».
galeria-hoy = Aujourd’hui
galeria-ayer = Hier
galeria-dia-0 = Lundi
galeria-dia-1 = Mardi
galeria-dia-2 = Mercredi
galeria-dia-3 = Jeudi
galeria-dia-4 = Vendredi
galeria-dia-5 = Samedi
galeria-dia-6 = Dimanche
galeria-elegir-dia = Sélectionner toute la journée
galeria-chip-video = Vidéo
galeria-pinear = Épingler
galeria-copiar = Copier
galeria-conservar = Conserver
galeria-borrar = Supprimer
galeria-elegidas = { $cuantas ->
    [one] 1 sélectionnée
   *[other] { $cuantas } sélectionnées
}
galeria-elegir-todas = Toutes
galeria-deshacer = Annuler
galeria-borradas = { $cuantas } déplacées dans la corbeille de PinDesk
galeria-recuperadas = { $cuantas ->
    [one] Restaurée
   *[other] { $cuantas } restaurées
}
galeria-pineadas = { $cuantas } épinglées à l’écran
galeria-copiadas = { $cuantas } copiées en tant que fichiers
galeria-conservadas-aviso = { $cuantas } conservées : elles sont dans Messages enregistrés
galeria-prorrogada = Sept jours de plus : elle sera supprimée plus tard
## v2-pines
# The pin toolbar, its right-click menu and the "Open pins" panel.
pin-v2-anotar = Annoter
pin-v2-mas = Plus
pin-v2-opacidad = Opacité
pin-v2-pines-abiertos = Épingles ouvertes…
pin-v2-abrir = Ouvrir
pin-v2-pinear-pagina = Épingler cette page
pin-v2-pinear-todas = Épingler toutes les pages
pin-v2-cerrar-pin = Fermer l’épingle
pin-v2-alejar = Zoom arrière
pin-v2-acercar = Zoom avant
pin-v2-en-vivo = DIRECT
pin-v2-tecla-espacio = Espace
pin-v2-tecla-rueda = Molette
pin-v2-tecla-re-pag = PgPréc
pin-v2-tecla-av-pag = PgSuiv
pin-v2-tecla-mayus-rueda = Maj+Molette
pin-v2-panel-titulo = Épingles ouvertes
pin-v2-panel-buscar = Rechercher une épingle
pin-v2-panel-mostrar-todos = Tout afficher
pin-v2-panel-ocultar-todos = Tout masquer
pin-v2-panel-cerrar-todos = Tout fermer · annulable
pin-v2-panel-deshacer = Annuler : les rouvrir
pin-v2-panel-sin-grupo = Aucun groupe
pin-v2-panel-oculto = Masquée
pin-v2-panel-ayuda = Cliquez sur une ligne pour amener l’épingle au centre et la mettre en évidence. L’œil la masque ou l’affiche à nouveau.
pin-v2-panel-vacio = Aucune épingle ouverte. Épinglez une capture, une note ou un fichier et elle apparaîtra ici.
pin-v2-panel-ocultar = Masquer
pin-v2-panel-mostrar = Afficher
pin-v2-panel-cerrar-este = Fermer cette épingle
pin-v2-panel-cerrar-panel = Fermer le panneau
pin-v2-tipo-foto = Photo
pin-v2-tipo-nota = Note
pin-v2-tipo-pdf = PDF
pin-v2-tipo-video = Vidéo
pin-v2-tipo-archivo = Fichier
pin-v2-tipo-vivo = Épingle en direct
pin-v2-tipo-herramienta = Outil
pin-v2-marca-copiar = Copier
pin-v2-marca-resaltar = Surligner
pin-v2-marca-ondulada = Ligne ondulée
pin-v2-marca-subrayar = Souligner
pin-v2-marca-tachar = Barrer
pin-v2-marca-tapar = Masquer
pin-v2-marca-quitar = Retirer la marque
marcas-texto-color = Couleur
pin-v2-modo-texto = Sélectionner du texte
pin-v2-leyendo = Lecture…
pin-v2-invertir-colores = Inverser les couleurs
## v2-captura
captura2-zona = Zone
captura2-ventana = Fenêtre
captura2-pantalla = Écran
captura2-scroll = Défilante
captura2-gif = Enregistrer en GIF
captura2-pin-vivo = Épingle en direct
captura2-texto = Texte (OCR)
captura2-libre = Libre
captura2-repetir = Répéter la dernière zone
captura2-ancho = L
captura2-alto = H
captura2-sin-abrir = Sans ouvrir ceci :
captura2-arrastrar-izq = + glisser gauche
captura2-arrastrar-der = + glisser droit
captura2-arrastrar-centro = + glisser milieu
captura2-doble-centro = + double clic milieu
captura2-gesto-copiar = copier
captura2-gesto-pinear = épingler
captura2-gesto-pin-vivo = épingle en direct
captura2-gesto-anotar = dessiner à l’écran
captura2-salir = quitter
captura2-pista-ventana = Clic : capturer la fenêtre · Glisser : zone libre
captura2-pista-zona = Clic : capturer ceci · Glisser : zone libre
captura2-copiar-color = copier la couleur
captura2-cambiar-formato = HEX / RGB
captura2-copiar = Copier
captura2-pinear = Épingler
captura2-al-chat = Vers le chat
captura2-ocr = Texte
captura2-guardar = Enregistrer
captura2-descartar = Ignorer
captura2-buscar-proyecto = Rechercher un projet…
captura2-recientes = Récents
captura2-el-ultimo = dernier utilisé
captura2-comentario = Ajouter un commentaire (facultatif)
captura2-enviar = Envoyer
captura2-sin-proyectos = Aucun projet ne correspond
captura2-enviada = Capture envoyée à « { $proyecto } ».
## v2-bandeja
# The tray panel (left click on the icon). The right-click menu keeps its
# usual keys.
bandeja2-sincronizar = Synchroniser maintenant
bandeja2-sincronizado = Synchronisé { $cuando }
bandeja2-sin-sincronizar = Pas encore synchronisé
bandeja2-hace-nada = à l’instant
bandeja2-hace-min = il y a { $n } min
bandeja2-hace-h = il y a { $n } h
bandeja2-ayer = hier
bandeja2-hace-dias = il y a { $n } jours
bandeja2-buscar = Rechercher une action… (tapez directement)
bandeja2-buscar-favorito = Rechercher un favori à ajouter…
bandeja2-sin-resultados = Aucune action ne porte ce nom
bandeja2-capturar = Capturer
bandeja2-capturar-sub = Choisissez la zone à la souris
bandeja2-grabar = Enregistrer
bandeja2-modo-zona = Zone
bandeja2-modo-scroll = Défilement
bandeja2-modo-gif = GIF
bandeja2-modo-texto = Texte
bandeja2-modo-retardo = Retardateur
bandeja2-modo-color = Couleur
bandeja2-favoritos = Favoris
bandeja2-editar = Modifier
bandeja2-anadir = Ajouter
bandeja2-elige-favoritos = Choisissez vos favoris ({ $n } sur { $tope })
bandeja2-favoritos-llenos = Il y a déjà { $tope } favoris : retirez-en un d’abord.
bandeja2-hecho = Terminé
bandeja2-fav-pinear-copiado = Épingler la copie
bandeja2-fav-anotar = Annoter l’écran
bandeja2-fav-pin-vivo = Épingle en direct
bandeja2-pines = Épingles · { $n } à l’écran
bandeja2-ocultar-pines = Masquer les épingles
bandeja2-paso-clic = Laisser passer les clics
bandeja2-atajos = Raccourcis
bandeja2-silenciar = Désactiver les raccourcis
bandeja2-silenciar-sub = pour les jeux ou les présentations
bandeja2-ultimas = Dernières captures
bandeja2-ver-galeria = Voir la galerie
bandeja2-sin-capturas = Aucune capture pour l’instant
bandeja2-abrir-pindesk = Ouvrir PinDesk
bandeja2-tareas = Tâches
bandeja2-lecciones = Leçons
bandeja2-nueva-leccion = Nouvelle leçon
bandeja2-galeria = Galerie des captures
bandeja2-documento = Ouvrir un document…
bandeja2-grupos = Groupes de fenêtres
bandeja2-para-hoy = { $n } pour aujourd’hui
bandeja2-salir = Quitter PinDesk
bandeja2-salir-sub = les épingles seront fermées
bandeja2-copiada = Copiée
bandeja2-borrada = Déplacée dans la corbeille
bandeja2-devuelta = Restaurée
bandeja2-deshacer = Annuler
bandeja2-no-se-pudo = Opération impossible : { $motivo }

## tareas-v3
tareas3-buscar = Rechercher des tâches, leurs listes et chats… (« aujourd’hui », « hier »)
tareas3-encontradas = { $n ->
    [one] 1 tâche trouvée
   *[other] { $n } tâches trouvées
  }
tareas3-pendientes = { $n ->
    [0] rien en attente
    [one] 1 en attente
   *[other] { $n } en attente
  }
tareas3-sin-resultados = Rien ne correspond à « { $busqueda } »
tareas3-sin-resultados-pista = Recherche dans le texte de chaque tâche et le nom de sa liste et de son chat, sans tenir compte des accents ni de la casse. Essayez moins de mots, ou « aujourd’hui » ou « hier ».
tareas3-vaciar-busqueda = Effacer la recherche
tareas3-mover-intro = déplacer
tareas3-mover-esc = fermer
tareas3-quitada = « { $tarea } » retirée
tareas3-deshacer = Annuler
tareas3-repuesta = Tâche remise dans sa liste
tareas3-cambio-quitar = La liste a changé entre-temps ; elle est maintenant à jour. Retirez-la à nouveau.
tareas3-borrar-lista = Supprimer la liste
tareas3-borrar-lista-aviso = Supprimer la liste « { $lista } » de « { $chat } » avec { $n ->
    [one] sa tâche
   *[other] ses { $n } tâches
  } ? Elle est aussi supprimée du chat et du téléphone, et c’est irréversible.
tareas3-lista-borrada = Liste « { $lista } » supprimée

## timeline (5-oct-2026)
timeline-titulo = Chronologie
timeline-sub-ultimas = Dernières 24 heures · { $n ->
    [one] 1 moment
   *[other] { $n } moments
  }
timeline-sub-dia = { $n ->
    [one] 1 moment
   *[other] { $n } moments
  }
timeline-ultimas-24h = Dernières 24 heures
timeline-hoy = Aujourd’hui
timeline-ayer = Hier
timeline-exportar = Exporter
timeline-volver = Retour
timeline-pista = Que se passe-t-il ? Écrivez ou dictez…
timeline-frase-titulo = voilà ce qui s’est passé
timeline-frase-titulo-corta = il s’est passé que
timeline-frase-descripcion = et je te raconte
timeline-frase-descripcion-corta = je te raconte
timeline-vacio = Rien dans les dernières 24 heures pour l’instant.
    Écrivez ou dictez ce qui se passe et appuyez sur Entrée.
    En dictée, « { timeline-frase-titulo }… » commence le titre et « { timeline-frase-descripcion }… » la description.
timeline-vacio-dia = Rien n’a été noté ce jour-là.
timeline-nota-de-voz = Note vocale
timeline-sin-titulo-voz = Note vocale
timeline-sin-titulo-foto = Photo
timeline-apuntado = Noté
timeline-corregido = Corrigé
timeline-borrado = Moment supprimé
timeline-deshacer = Annuler
timeline-abrir = Ouvrir
timeline-escribe-algo = Écrivez quelque chose pour le titre
timeline-no-guardado = Impossible d’enregistrer la chronologie
timeline-corrigiendo = Correction du moment de { $hora } · Entrée enregistre, Échap annule
timeline-voz-esperando = 🎙 La note vocale accompagne le texte : appuyez sur Entrée
timeline-pegar-no-imagen = Ce n’est ni une image ni du texte
timeline-cal-elegidos = { $n ->
    [one] 1 jour sélectionné
   *[other] { $n } jours sélectionnés
  }
timeline-cal-quitar = Effacer la sélection
timeline-exportar-html = Page web (HTML)
timeline-exportar-pdf = Document PDF
timeline-tipo-html = Page web
timeline-tipo-pdf = Document PDF
timeline-nada-que-exportar = Rien à exporter ici
timeline-no-exportado = Impossible d’enregistrer le fichier
timeline-exportado = Enregistré : { $nombre }
timeline-exportado-pie = Exporté avec PinDesk · { $fecha }
timeline-pdf-haciendo = Création du PDF…
timeline-pdf-sin-edge = Le PDF nécessite Microsoft Edge ; exportez en HTML
timeline-pdf-fallo = Impossible de créer le PDF
timeline-mic-abriendo = Ouverture du micro…
timeline-mic-escuchando = À l’écoute… appuyez sur le micro (ou Ctrl+M) pour terminer
timeline-mic-guardando = Enregistrement de ce que vous avez dit…
timeline-mic-bajando = Téléchargement du modèle vocal (une seule fois)… { $pc } %
timeline-mic-pasando = Transcription… { $pc } %
timeline-mic-sin-dictado = Cet ordinateur ne peut pas transcrire : saisissez-le
timeline-mic-no-abre = Impossible d’ouvrir le micro
timeline-mic-no-guarda = Impossible d’enregistrer ce que vous avez dit
timeline-mic-sin-microfono = Aucun micro
timeline-mic-corto = Trop court : parlez un peu plus longtemps
timeline-mic-error = Impossible d’enregistrer : { $error }
timeline-mic-sin-texto = Non compris : seul l’audio est conservé


## simple-lecciones

## galeria-menu (5-oct-2026, right click)
galeria-guardar-como = Enregistrer sous…
galeria-mostrar-en-carpeta = Afficher dans le dossier
galeria-guardadas = { $cuantas ->
    [one] Enregistrée
   *[other] { $cuantas } enregistrées
}
galeria-prorrogar = Accorder 7 jours de plus

## timeline-v2 (5-oct-2026): common bar, WeChat-like tabs and detail
timeline-pestana-hoy = Aujourd’hui
timeline-pestana-momentos = Moments
timeline-pestana-estado = Calendrier
timeline-buscar-pista = Rechercher dans la chronologie
timeline-sub-buscar = { $n ->
    [0] Aucun résultat
    [one] 1 résultat
   *[other] { $n } résultats
  }
timeline-sub-momentos = { $n ->
    [0] Aucun moment pour l’instant
    [one] 1 moment au total
   *[other] { $n } moments au total
  }
timeline-sin-resultados = Aucun moment ne contient cela.
    La recherche porte sur le titre et la description, sans tenir compte des accents ni de la casse.
timeline-momentos-vacio = Les moments que vous choisissez arrivent ici. Dans « Aujourd’hui », survolez une carte et appuyez sur le signet pour l’envoyer ici.
timeline-a-momentos = Envoyé dans Moments
timeline-fuera-de-momentos = Retiré de Moments
timeline-estado-sobre-todo = Surtout
timeline-exportar-busqueda = Recherche : « { $que } »
timeline-pestana-lecciones = Leçons
timeline-sub-lecciones = { $n ->
    [0] Aucune leçon
    [one] 1 leçon apprise
   *[other] { $n } leçons apprises
  }
timeline-lecciones-vacio = Aucune leçon pour l’instant. Appuyez sur l’ampoule d’un moment pour en faire une leçon.
timeline-gravedad-leve = Mineure
timeline-gravedad-importante = Importante
timeline-gravedad-grave = Grave
timeline-otra-vez-hecho = Noté : { $n } fois maintenant
timeline-leccion = Leçon
timeline-leccion-hecha = 💡 Leçon créée : elle est dans « Leçons »
timeline-leccion-quitada = N’est plus marqué. La leçon reste dans « Leçons ».
timeline-leccion-fallo = Impossible d’enregistrer la leçon
timeline-leccion-por-que = Pourquoi
timeline-leccion-proxima = La prochaine fois
timeline-tambien-lecciones = { $n ->
    [one] 1 leçon correspond aussi · Voir
   *[other] { $n } leçons correspondent aussi · Voir
  }
timeline-copiar-imagen = Copier en image
timeline-compartir-html = Partager en HTML
timeline-guardar-imagen = Enregistrer en image…
timeline-imagen-copiada = Copié en image
timeline-imagen-fallo = Impossible de créer l’image
timeline-tipo-png = Image PNG
timeline-compartido-pie = PinDesk · Chronologie

## tareas-v4
# The search box hint in the shared top bar: shorter than tareas3-buscar,
# which does not fit in the space the bar leaves.
tareas4-buscar = Rechercher des tâches…
timeline-otra-vez-corto = ↻ C’est arrivé à nouveau +1
timeline-pagina-linea = Chronologie
timeline-exportado-pesa = il fait { $mb } Mo : il risque d’être difficile à envoyer

## v2-diseno
# The gallery with the v2 design system (Oct 6).
galeria-chapa-hoy = aujourd’hui
galeria-pista-caduca = { $dias ->
    [0] Supprimée aujourd’hui
    [one] Supprimée demain
   *[other] Supprimée dans { $dias } jours
  } · clic droit pour conserver
galeria-ya-conservadas = Elles étaient déjà conservées
galeria-dia-cuantas = { $cuantas ->
    [one] 1 capture
   *[other] { $cuantas } captures
  }
timeline-sub-estado-dias = { $n ->
    [0] Aucun jour avec quelque chose pour l’instant
    [one] 1 jour avec quelque chose
   *[other] { $n } jours avec quelque chose
  }
timeline-filtro-todas = Toutes
timeline-orden-recientes = Récentes
timeline-orden-repetidas = Les plus répétées
timeline-lecciones-sin-resultados = Aucune leçon ne contient cela.
timeline-otra-vez-pildora = ↻ +1

## tareas-v5
# The bar subtitle: each group already counts what is pending.
tareas5-subtitulo = { $listas ->
    [one] 1 liste
   *[other] { $listas } listes
  }
tareas5-apuntar-pista = Nouvelle tâche…
tareas-recordare = ⏰ Je vous le rappellerai { $cuando }
tareas-sin-recordatorio = Rappel supprimé
tareas-apuntar-en-pista = Nouvelle tâche dans « { $lista } »…
tareas5-corregir-pista = Texte de la tâche…
tareas5-imagen-pista = Ajouter des images · Ctrl+V colle une image copiée
tareas5-guardar = Enregistrer
tareas5-corregida = Tâche mise à jour
tareas5-vista-tarjetas = Cartes · Ctrl+T
tareas5-vista-horario = Emploi du temps · Ctrl+H
tareas5-vista-kanban = Tableau Kanban · Ctrl+K
tareas5-marcar = Marquer comme faite
tareas5-desmarcar = Marquer comme en attente
tareas5-editar = Modifier
tareas5-copiar-texto = Copier le texte
tareas5-copiada = Texte copié
tareas5-pinear-imagenes = { $n ->
    [one] Épingler l’image
   *[other] Épingler les { $n } images
  }
tareas5-quitar = Retirer

## salida (6-oct-2026): the floating window to drag out what was exported
salida-titulo = Prêt à partager
salida-titulo-exportado = Exporté
salida-pista = Faites glisser un fichier vers une autre application ou un dossier
salida-todos = Faire glisser les { $n } ensemble
salida-arrastra-todos = Maintenez et faites glisser pour tous les prendre
salida-soltado = Déposé · vous pouvez aussi le déposer ailleurs
salida-copiado = Copié dans le presse-papiers
salida-pineado = Épinglé
salida-no-se-pudo = Opération impossible
salida-copiar = Copier
salida-abrir = Ouvrir
salida-carpeta = Afficher dans le dossier
salida-pinear = Épingler
salida-compartir-menu = Partager…

## al-movil (6-oct-2026): a photo from the Flow plugin to the phone's open canvas
al-movil-lienzo = { $n ->
    [one] Envoyé au canevas sur { $movil }
   *[other] { $n } images envoyées au canevas sur { $movil }
  }
al-movil-chat = { $movil } l’a enregistré dans la conversation générale (aucun chat ni canevas n’était ouvert)
al-movil-chat-abierto = { $n ->
    [one] Envoyé au chat « { $chat } » sur { $movil }
   *[other] { $n } fichiers envoyés au chat « { $chat } » sur { $movil }
  }
al-movil-solo-fotos = { $movil } a un canevas ouvert : il n’accepte que des photos. Ouvrez un chat pour lui envoyer des fichiers.
al-movil-negados = { $negados ->
    [one] 1 fichier n’a pas été envoyé : ce n’était pas une photo et un canevas est ouvert
   *[other] { $negados } fichiers n’ont pas été envoyés : ce n’étaient pas des photos et un canevas est ouvert
  }
al-movil-no-esta = { $movil } n’est pas ouvert : ouvrez PinDesk sur le téléphone
al-movil-actualiza = Mettez à jour PinDesk sur { $movil } : il ne peut pas encore recevoir de fichiers directs. Ouverture de « Envoyer par Wi-Fi ».
al-movil-ocupado = { $movil } se synchronise avec un autre appareil. Réessayez dans un instant.
al-movil-otra-version = { $movil } a une autre version de PinDesk : mettez les deux à jour
al-movil-cortado = Impossible d’envoyer à { $movil } : { $motivo }
al-movil-sin-grupo = Ce PC n’est dans aucun groupe : configurez-le dans Synchronisation de PinDesk
al-movil-no-es-del-grupo = Cet appareil n’est plus dans le groupe de ce PC
al-movil-sin-imagenes = Il n’y avait rien à envoyer
al-frente-lienzo = { $de } vous a envoyé « { $nombre } » : c’est sur le canevas ouvert
al-frente-chat-abierto = { $de } vous a envoyé « { $nombre } » dans le chat « { $chat } »
al-frente-guardados = { $de } vous a envoyé « { $nombre } » : c’est dans Messages enregistrés

## Excel sheet viewer (visor_hoja)
hoja-copiado = { $celdas ->
    [one] 1 cellule copiée
   *[other] { $celdas } cellules copiées
  }
hoja-demasiado = Trop de cellules à copier à la fois ({ $celdas })
hoja-cargando = Chargement… { $filas } lignes
hoja-enlace-no-abre = Impossible d’ouvrir le lien

## Chats shared with people (Android 13-oct-2026, ChatCompartidoActivity)
cc-menu-compartir = Partager avec des personnes
cc-menu-entrar = Rejoindre un chat partagé
chat-nuevo-normal = Chat normal
chat-nuevo-normal-detalle = Juste vous et vos appareils
chat-nuevo-compartido = Chat partagé
chat-nuevo-compartido-detalle = Avec d’autres personnes, chacune de son côté
cc-chapa-personas = { $n ->
    [one] Partagé · 1 personne
   *[other] Partagé · { $n } personnes
}
cc-nuevo-error = Impossible de partager le nouveau chat : il reste un chat normal
cc-portada-titulo = Chats partagés
cc-portada-como = Un chat de projet que vous tenez avec quelqu’un d’autre : il ne voit que ce chat. Pour en partager un, clic droit sur le projet → Partager avec des personnes.
cc-portada-entrar = Rejoindre un chat partagé
cc-titulo-personas = Personnes
cc-titulo-entrar = Rejoindre un chat
cc-tu-nombre = Votre nom dans les chats partagés
cc-guardar = Enregistrer
cc-compartir-explica = Partagez ce chat avec d’autres personnes.
cc-compartir-detalle = Seul ce chat est transmis : ses messages, fichiers, canevas et annotations. Vos autres chats et projets restent cachés. Chacun se synchronise avec les autres sur le même Wi-Fi.
cc-compartir = Partager
cc-ya-no-estas = Vous n’êtes plus dans ce chat. Ce que vous aviez reste ici, mais n’est plus synchronisé.
cc-tu = (vous)
cc-creo = a créé le chat
cc-tu-otro-aparato = votre autre appareil
cc-salio = est parti
cc-quitado = retiré
cc-quitar = Retirer
cc-anadir = Ajouter
cc-cerrar-invitacion = Fermer
cc-sincronizar = Synchroniser
cc-invitacion-como = Sur l’autre appareil : PinDesk → Rejoindre un chat partagé. Scannez ceci ou saisissez le code.
cc-invitacion-vale = Valable tant que cet écran reste ouvert. Vous devez être sur le même Wi-Fi.
cc-salir-del-chat = Quitter le chat
cc-salir-titulo = Quitter le chat ?
cc-quitar-titulo = Retirer { $nombre } ?
cc-salir-aviso = Ce que vous avez reste ici, mais ne sera plus synchronisé avec les autres.
cc-quitar-aviso = Cette personne garde ce qu’elle a déjà, mais ne recevra plus rien de nouveau de ce chat.
cc-salir = Quitter
cc-cancelar = Annuler
cc-entrar-como = La personne qui vous invite ouvre le chat → Partager avec des personnes → Ajouter, et vous montre un code. Vous ne recevez que ce chat : rien d’autre de son appareil, et vous ne lui donnez rien du vôtre. Cet ordinateur n’a pas de caméra : saisissez le code ou collez le texte de son QR.
cc-escribe-codigo = Saisissez le code ou collez le texte du QR
cc-entrar = Rejoindre
cc-ya-estas = Vous êtes dans « { $nombre } »
cc-con = Avec : { $nombres }
cc-cerrar = Fermer
cc-compartido = Partagé. Appuyez sur Ajouter pour inviter quelqu’un.
cc-entro = { $nombre } a rejoint. Transfert du chat…
cc-listo = Terminé : la personne a le chat.
cc-demasiados = Trop de codes erronés : invitation fermée.
cc-has-salido = Vous êtes parti. Les autres le sauront à la synchronisation.
cc-quitado-aviso = Retiré. La personne le saura à la synchronisation.
cc-nadie = Personne d’autre pour l’instant. Appuyez sur Ajouter.
cc-buscando = Recherche des membres du chat…
cc-sincronizando-con = Synchronisation avec { $nombre }…
cc-nadie-en-wifi = Aucun membre du chat n’est sur ce Wi-Fi pour le moment.
cc-sincronizado = Synchronisé avec { $nombres }.
cc-buscando-invitacion = Recherche de la personne qui vous a invité…
cc-entrando = Connexion…
cc-trayendo = Transfert de « { $nombre } »…
cc-no-encontrada = L’invitation est introuvable. Vérifiez le code et que vous êtes sur le même Wi-Fi.
cc-se-corto = Interrompu : { $motivo }
cc-abrir-chat = Ouvrir le chat
cc-no-se-abre = Ce chat n’est pas encore ici : synchronisez à nouveau.

## Pins like on Android (batch 6): color, stickers, paged note and own words
pin-v2-pegatina = Autocollant
pin-v2-sin-pegatina = Sans autocollant
pin-v2-paginas = Pages
pin-v2-leer-seguida = Lecture continue
pin-v2-pagina = Page
pin-v2-minutos = min
pin-color-copiado = Copié
ajustes2-sec-pinear = Épinglage
ajustes2-sec-pinear-sub = Les mots qui deviennent un outil une fois épinglés.
ajustes2-sec-tareas = Tâches
ajustes2-sec-tareas-sub = Ce qui est compris quand vous saisissez une tâche.
ajustes2-g-urgencia = Mots d’urgence (sans aucun, la tâche est normale)
ajustes2-urgente = Urgent
ajustes2-muy-urgente = Très urgent
ajustes2-urgencia-ninguna = Aucun : rien ne reçoit cette urgence
ajustes2-g-palabras = Mots magiques
ajustes2-ayuda-palabras = Copiez-en un seul et épinglez-le : l’outil s’ouvre. Séparez-les par des virgules.
ajustes2-palabras-ninguna = Aucun
ajustes2-palabras-cambiar = Modifier
ajustes2-palabras-fabrica = Mots d’usine
ajustes2-palabras-volver = Réinitialiser
ajustes2-palabras-marcador = mot, autre mot…

## Local assistant (13-oct-2026)
asistente-mencionar = Mentionner : l’assistant ou où dessiner
asistente-abrir-lienzo = Ouvrir le canevas
asistente-abrir-fuente = Ouvrir la source citée
asistente-ocupado = L’assistant répond encore : attendez ou appuyez sur Arrêter.
asistente-fallo = L’assistant n’a pas pu commencer à répondre.
asistente-nube-titulo = La clé cloud est manquante
asistente-nube-explica = L’assistant répond avec { $nube } : collez sa clé dans Paramètres → Assistant (obtenez-en une sur { $pagina }). Les commandes simples (« note pain », « ouvre la galerie ») fonctionnent sans clé.
asistente-abrir-ajustes = Ouvrir les paramètres
asistente-viejos-titulo = Les modèles locaux ne sont plus utilisés
asistente-viejos-explica = L’assistant répond désormais uniquement via le cloud (Claude ou DeepSeek). Ce que vous avez téléchargé pour le modèle de ce PC occupe encore { $tamano } sur le disque.
asistente-viejos-liberar = Libérer { $tamano }
asistente-viejos-dejar = Les garder
asistente-viejos-borrados = Terminé : les modèles locaux ont été supprimés et l’espace est libéré.
asistente-viejos-no-borrados = Tous les modèles locaux n’ont pas pu être supprimés. Fermez ce qui les utilise et réessayez depuis Paramètres → Assistant.
ajustes2-asistente-borrar-locales = Anciens modèles locaux
ajustes2-ayuda-asistente-borrar-locales = Ce qui a été téléchargé pour l’assistant local, inutilisé : { $tamano } dans %LOCALAPPDATA%\PinDesk\modelos.

## Cloud assistant (13-oct-2026)
ajustes2-sec-asistente = Assistant
ajustes2-sec-asistente-sub = Qui répond dans le chat « Asistente » : Claude ou DeepSeek, avec votre clé.
ajustes2-g-asistente = Moteur de l’assistant
ajustes2-asistente-motor = Moteur
ajustes2-ayuda-asistente-motor = Répond via internet avec votre clé : Claude Haiku ou DeepSeek.
asistente-confirmar-si = Oui
asistente-confirmar-no = Non
asistente-otro = Autre…
asistente-otro-escribe = Écrivez votre réponse ci-dessous et envoyez-la
asistente-recomendada = recommandée
asistente-receta-guardar = Enregistrer comme recette
asistente-modelo = Modèle
asistente-nueva-conversacion = Nouvelle conversation
asistente-nuevo-chat = Nouveau chat
asistente-borrar-chat = Vider le chat
asistente-borrar-chat-aviso = Tous les messages du chat « Assistant » sont supprimés (avec leurs pièces jointes). Continuer ?
asistente-escuchar = Écouter
asistente-modelo-cambiado = { $modelo } répond maintenant.
asistente-receta-guardada = Recette enregistrée : { $nombre }
asistente-receta-llenas = Il y a trop de recettes : supprimez-en une d’abord.
asistente-receta-renombrar = Renommer
asistente-receta-editar = Modifier la demande
asistente-receta-icono = Changer l’icône
asistente-receta-izquierda = Déplacer à gauche
asistente-receta-derecha = Déplacer à droite
asistente-receta-borrar = Supprimer la recette
asistente-receta-editando-nombre = Nom de la recette « { $nombre } » · Entrée enregistre · Échap annule
asistente-receta-editando-pedido = Demande de la recette « { $nombre } » · Entrée enregistre · Échap annule
ajustes2-asistente-docs-nube = Vos documents et le cloud
ajustes2-ayuda-asistente-docs-nube = Avec Claude ou DeepSeek, ce que l’assistant trouve dans vos PDF, notes et chats passe par internet pour vous répondre.
ajustes2-docs-nube-preguntar = Demander
ajustes2-docs-nube-si = Autoriser
ajustes2-docs-nube-no = Ne pas autoriser
ajustes2-g-claves = Clés d’API
ajustes2-clave-de = Clé { $proveedor }
ajustes2-clave-marcador = Collez la clé ici
ajustes2-clave-probar = Enregistrer et tester
ajustes2-clave-borrar-de = Supprimer la clé { $proveedor }
ajustes2-ayuda-clave-borrar = Elle est supprimée de ce PC. La clé reste valide dans votre compte jusqu’à ce que vous la révoquiez là-bas.
ajustes2-clave-borrar = Supprimer
ajustes2-clave-guardada = Enregistrée et chiffrée sur ce PC :
ajustes2-clave-falta = Aucune clé. Obtenez-en une sur
ajustes2-clave-probando = test…
ajustes2-g-conexiones = Connexions de l’assistant
ajustes2-conexion-cliente = ID client { $servicio }
ajustes2-conexion-cliente-marcador = Collez l’ID client (ou le JSON fourni par Google)
ajustes2-conexion-guardar = Enregistrer
ajustes2-conexion-de = { $servicio }
ajustes2-conexion-conectar = Connecter
ajustes2-conexion-desconectar = Déconnecter
ajustes2-conexion-sin-cliente = Aucun ID client. Créez-le une fois sur { $consola } (étapes : docs/conexiones-google-drive.md).
ajustes2-conexion-cliente-guardado = ID enregistré et chiffré sur ce PC :
ajustes2-conexion-conectado = Connecté
ajustes2-conexion-sin-conectar = Non connecté. Avec votre autorisation, l’assistant recherche, lit, télécharge et téléverse des fichiers dans votre { $servicio }.
ajustes2-conexion-conectando = Terminez dans le navigateur…

## Assistant automations (Oct 13)
automatizacion-creada = Automatisation « { $nombre } » créée.
automatizacion-activada = Automatisation « { $nombre } » activée.
automatizacion-desactivada = Automatisation « { $nombre } » désactivée.
automatizacion-borrada = Automatisation « { $nombre } » supprimée.
automatizacion-en-marcha = Exécution de « { $nombre } »…
automatizacion-no-esta = Il n’existe pas d’automatisation « { $id } ».
automatizacion-rota = Impossible d’enregistrer l’automatisation : { $motivo }
ajustes2-g-automatizaciones = Automatisations
ajustes2-automatizaciones-vacio = Aucune pour l’instant
ajustes2-ayuda-automatizaciones-vacio = Demandez à l’assistant : « à chaque capture, mets-la dans le chat du chantier », « tous les jours à 8 h, résumé des tâches en attente ».
ajustes2-automatizacion-correr = Exécuter maintenant
ajustes2-automatizacion-otro-aparato = s’exécute sur { $aparato }
ajustes2-automatizacion-rota = Non compris : { $motivo }
ajustes2-automatizacion-borrar = Supprimer l’automatisation « { $nombre } » ? C’est irréversible.
ajustes-asistente-pensar = Réfléchir avant de répondre
ajustes2-ayuda-asistente-pensar = Avant d’agir, l’assistant réfléchit et écrit ce qu’il a compris ; sa réflexion s’affiche, repliée, au-dessus de sa réponse. Réfléchir prend du temps : en automatique, il ne réfléchit que pour les tâches complexes, comme résoudre un problème, planifier ou enchaîner plusieurs étapes, ou quand vous le demandez (« réfléchis bien », « étape par étape »).
asistente-pensar-auto = Automatique (seulement pour les tâches complexes)
asistente-pensar-si = Toujours
asistente-pensar-no = Jamais
asistente-pensando = Réflexion…
asistente-penso = A réfléchi { $s } s
asistente-penso-poco = A réfléchi un instant
asistente-reiniciado = Contexte renouvelé : reprise à partir de la fiche de travail

## Assistant voice (13-oct-2026): talk to it and hear the answer
ajustes2-g-asistente-voz = Voix de l’assistant
ajustes2-asistente-revisar-voz = Relire ce que vous avez dit avant l’envoi
ajustes2-ayuda-asistente-revisar-voz = Avec le micro du chat « Asistente » : ce que vous dites est écrit dans la zone pour que vous le corrigiez et l’envoyiez vous-même. Désactivé, c’est envoyé immédiatement.
asistente-voz-pasando = Conversion de votre voix en texte… { $pct } %
asistente-voz-bajando = Téléchargement de la reconnaissance vocale (une fois, ~160 Mo)… { $pct } %
asistente-voz-nada = Rien n’a été entendu : parlez plus près du micro et réessayez.
asistente-voz-sin-whisper = Ce PC ne peut pas convertir la voix en texte : ONNX Runtime de Windows est manquant.
asistente-voz-fallo = Impossible de convertir votre voix en texte.
asistente-voz-cancelado = Annulé : rien n’a été envoyé.
asistente-voz-nada-que-leer = Cette réponse n’a pas de texte à lire.

# The buttons of the Windows notifications (15-oct).
notif-hecha = Fait
notif-mas-10 = +10 min
notif-abrir = Ouvrir
notif-conservar = Conserver
notif-ver-galeria = Ouvrir la galerie
notif-captura-se-va = Cette capture disparaît { $cuando }
notif-capturas-se-van = { $n } captures disparaissent dans les prochaines 24 h
notif-capturas-titulo = Captures bientôt expirées

# Geometry sheets (exact arc, angle mark, gradient)
herramientas-angulo = Marque d’angle
lienzo-pista-arco = Arc (sur un guide : tracez un ovale de référence ; ou par centre et deux points, ou par trois points, dans le panneau)
lienzo-pista-angulo = Angle (W) : cliquez à l’intérieur de l’angle, près de la jonction de deux lignes ; place α, β, θ… sans répétition (↑ ↓ ou Alt+molette changent la lettre ; W à nouveau : l’angle droit avec son petit carré)
lienzo-panel-modo-angulo = Lettre d’angle
lienzo-panel-recientes = Récentes
lienzo-panel-personalizado = Personnalisée
lienzo-angulo-modo-griega = α β θ
lienzo-angulo-modo-simple = Sans lettre
lienzo-angulo-modo-recto = 90° ∟
lienzo-panel-degradado = Remplissage dégradé
lienzo-panel-cierre-arco = Fermeture de l’arc
lienzo-panel-modo-arco = Tracer l’arc
lienzo-panel-arcos-angulo = Arcs
lienzo-degradado-liso = Uni
lienzo-degradado-lineal = Linéaire
lienzo-degradado-radial = Radial
lienzo-cierre-abierto = Ouvert
lienzo-cierre-sector = Secteur
lienzo-cierre-segmento = Segment
lienzo-arco-modo-guia = Guide
lienzo-arco-modo-centro = Centre
lienzo-arco-modo-tres = 3 points

## Tasks: single-task pin and class timetable (17-oct-2026)
tareas-fijar-pin = 📌 Épingler comme rappel
pin-tarea-nota = 💬 Pourquoi pas maintenant ?
pin-tarea-pista = Pourquoi pas maintenant · Entrée enregistre
# Tasks: urgency and day/time picker (18-oct-2026)
tareas-urgente = Urgent
tareas-muy-urgente = Très urgent
tareas-selector-pista = Choisissez le jour et l’heure du rappel
tareas-selector-sin = Sans rappel
tareas-selector-quitar = Retirer
tareas-selector-hoy = Aujourd’hui
tareas-selector-manana = Demain
tareas-dia-0 = lun.
tareas-dia-1 = mar.
tareas-dia-2 = mer.
tareas-dia-3 = jeu.
tareas-dia-4 = ven.
tareas-dia-5 = sam.
tareas-dia-6 = dim.
bandeja-horario = 🎓 Emploi du temps…
bandeja-horario-grabar = 🎙 Note vocale pour le prochain cours
horario-sin-clases = Aucun cours dans l’emploi du temps pour l’instant : ouvrez « Emploi du temps… » et écrivez-en un par ligne.
horario-guardada = Vous l’entendrez au prochain cours de { $clase }

## Weekly timetable window and subjects (18-oct-2026)
horario-subtitulo = { $asignaturas } matières · { $clases } cours
horario-nueva = Nouvelle matière (@salle facultatif)
horario-domingo = Dimanche
horario-como-texto = En texte
horario-vacio = Saisissez une matière ci-dessus et faites-la glisser sur la semaine.
horario-tareas-de = Tâches de { $asignatura }
horario-sin-tareas = Aucune tâche en attente. Dans Tâches, choisissez la matière en en ajoutant une.
horario-abrir-tareas = Ouvrir dans Tâches
horario-aula = Salle
horario-nombre = Nom
horario-nota = Note vocale (ce que lit l’appel)
horario-avisar = Rappeler (minutes avant)
horario-avisar-pista = { $min } (celui de l’emploi du temps)
horario-guardar = Enregistrer
horario-cancelar = Annuler
horario-borrar = Supprimer
horario-deshacer = Annuler
horario-borrada = Cours supprimé
horario-asignatura-borrada = « { $asignatura } » retirée de l’emploi du temps (ses tâches restent dans Tâches)
horario-repetida = « { $asignatura } » est déjà dans la liste
horario-arrastra = Faites glisser « { $asignatura } » sur la semaine
horario-no-guardado = Impossible d’enregistrer l’emploi du temps
tareas-destino-pista = Liste ou matière de la tâche
tareas-sin-asignatura = Sans matière (vers la Boîte de réception)
asignaturas-todas = Toutes
chat-asignatura-poner = Matière…
chat-asignatura-quitar = Sans matière
chat-enviar-a-asignatura = Envoyer à une matière…
chat-enviado-a-asignatura = Copié dans « { $chat } » · { $asignatura }
tareas-apuntar-en-asignatura = Nouvelle tâche de { $asignatura }…

# Viewers: the frame to print (2026-10-19)
marco-imprimir-pista = Tracez un cadre sur ce que vous voulez imprimer · Entrée : tout · Échap : quitter
marcos-pista = Faites glisser pour cadrer · un numéro : aller à ce cadre · Ctrl+P : tout imprimer · Suppr : retirer le dernier · I ou Échap : quitter
marcos-imprimir = Imprimer les cadres (Ctrl+P)
marcos-borrar = Effacer les cadres

# Right-click on the rail's frame button (27-oct-2026): what to print
marcos-menu-los-marcos = Imprimer les cadres ({ $n })
marcos-menu-sin-marcos = Imprimer les cadres (aucun pour l’instant : appuyez sur le cadre et faites glisser)
marcos-menu-entero-pdf = Imprimer tout le PDF
marcos-menu-entero-word = Imprimer tout le document Word
marcos-menu-entero-powerpoint = Imprimer tout le PowerPoint
marcos-menu-entero-hoja = Imprimer toute la feuille
marcos-menu-entero-pagina = Imprimer toute la page
marcos-menu-entero-documento = Imprimer tout le document
marcos-menu-entero-plano = Imprimer tout le plan
marcos-menu-entero-modelo = Imprimer la vue du modèle
marcos-menu-no-imprime = Impossible d’imprimer tout le document

riel-marcador = Signet (Ctrl+M)
riel-anotar = Annoter (A)
riel-marcos = Cadres à imprimer (I)
barra-mover = Déplacer (glisser)
barra-oscuro = Mode nuit
barra-pantalla = Plein écran (F11)
barra-fijar = Toujours au premier plan
barra-cerrar = Fermer (Échap)
visor-letra = Police et espacement (T / B)
lector-mas = Plus d’options (G)
visor-indice-pista = Sommaire (C)
riel-editar = Modifier (Alt+R)
riel-mirar = Revenir à la lecture (Alt+R ou Échap)

# Pin rail: the frame button of a pin that is not a photo (19-oct-2026).
riel-imprimir-zona = Imprimer une zone (Ctrl+P)

# The spreadsheet viewer bar: name and key of each button (19-oct-2026).
hoja-pista-editar = Modifier les cellules (F2)
hoja-pista-ver = Revenir à la lecture (Échap)
hoja-pista-negrita = Gras (Ctrl+B)
hoja-pista-cursiva = Italique (Ctrl+I)
hoja-pista-subrayado = Souligné (Ctrl+U)
hoja-pista-tachado = Barré (Ctrl+5)
hoja-pista-resaltar = Couleur de remplissage
hoja-pista-letra = Couleur de police
hoja-pista-alinear = Aligner (Ctrl+Maj+L / E / R)
hoja-pista-numero = Format de nombre
hoja-pista-ajustar = Renvoyer à la ligne
hoja-pista-borrar-formato = Effacer la mise en forme
## Tooltips of the web viewers' and Office pin bars (19-oct)
web-recargar = Actualiser (F5)
web-exportar = Exporter (Ctrl+E)
web-navegador = Ouvrir dans le navigateur
nota-pista-ver = Mode lecture
nota-pista-editar = Modifier
nota-pista-buscar = Rechercher (Ctrl+F)
nota-pista-comentarios = Commentaires
nota-pista-compartir = Partager
office-pista-abrir-con = Ouvrir avec…
office-pista-presentar = Présenter

## Page folders in the chat (26-oct-2026): a named notebook
carpeta-nueva = Dossier de pages
carpeta-nombre-de-serie = Cahier
carpeta-pagina = Page
carpeta-paginas = { $n ->
    [one] 1 page
   *[other] { $n } pages
}
carpeta-vacia-detalle = Vide
carpeta-anadir-a = Ajouter au dossier…
carpeta-nueva-con = Nouveau dossier avec cette page
carpeta-anadida = Ajoutée à « { $carpeta } » sous le nom « { $titulo } »
carpeta-ya-estaba = Déjà dans « { $carpeta } »
carpeta-abrir = Ouvrir le dossier
carpeta-nueva-pagina = Nouvelle page
carpeta-compartir = Partager
carpeta-exportar-pdf = Exporter en PDF
carpeta-exportando = Export du PDF…
carpeta-exportado = PDF enregistré : { $nombre }
carpeta-no-se-exporto = Export impossible : aucune page n’a de contenu
carpeta-vacia = Ce dossier est vide. Appuyez sur « Nouvelle page », ou dans le chat choisissez « Ajouter au dossier… » dans le menu d’un canevas ou d’une image.
carpeta-no-esta = Ce dossier n’est plus dans le chat
carpeta-pista = Faites glisser une page pour la déplacer · F2 change son titre · Suppr la retire (elle réapparaît dans le chat)
carpeta-escribe-titulo = Saisissez le titre et appuyez sur Entrée
carpeta-grabacion = Enregistrement
carpeta-grabacion-detalle = { $duracion } · { $capturas ->
    [one] 1 capture
   *[other] { $capturas } captures
}
carpeta-estudiar = Étudier
carpeta-adjunto-insignia = Pièce jointe · ne fait pas partie de l’enregistrement
carpeta-adjuntar = Joindre au cours
carpeta-adjuntado = Joint au cours : { $nombre }
carpeta-grabacion-pista = Cliquez sur un élément pour l’étudier à sa minute · « Joindre au cours » ajoute un fichier, une photo ou une note sans le mélanger à l’enregistrement

## The assistant reads the web (Oct 28)
asistente-web-buscando = 🌐 recherche sur le web…
asistente-web-leyendo = 🌐 lecture de { $dominio }…
asistente-web-leyendo-pagina = 🌐 lecture de la page…
## TypeSafe Jev (28-oct-2026, docs/jev.md)
ajustes2-g-jev = TypeSafe (Jev)
ajustes2-jev-proveedor = TypeSafe (Jev)
ajustes2-jev-probada = fonctionne ({ $modelo })
ajustes2-jev-reunion = L’essentiel des cours et réunions
ajustes2-ayuda-jev-reunion = Lors de la transcription d’un enregistrement, marque les tâches, décisions, questions et ce qui tombera à l’examen. Affiché dans « Étudier le cours » et sur la page partagée. Chaque ligne est envoyée avec ses voisines et le titre.
ajustes2-jev-anki = Seulement les cartes Anki utiles à étudier
ajustes2-ayuda-jev-anki = Lors de l’envoi d’un cours vers Anki, garde les captures dont le passage explique quelque chose à étudier et écarte les bavardages. Ce qui a été dit dans chaque passage est envoyé.
ajustes2-jev-urgencia = Priorité des tâches selon le contexte
ajustes2-ayuda-jev-urgencia = Urgente, haute, normale ou basse selon ce qui se passe en cas de retard, qui attend, son heure et les derniers messages du chat. Ce que vous marquez (⏫, 🔼) l’emporte. Le texte de la tâche, sa liste et les derniers messages de son chat sont envoyés.
ajustes2-jev-modelo = Choisir le modèle pour chaque demande
ajustes2-ayuda-jev-modelo = Les demandes simples vont au modèle rapide (Haiku, DeepSeek Flash) et celles qui demandent du raisonnement au modèle puissant (Sonnet, DeepSeek V4 Pro). La demande et les trois derniers messages sont envoyés.
clase-importante-todo = Tout
clase-importante-tarea = Tâche
clase-importante-decision = Décision
clase-importante-pregunta = Question
clase-importante-examen = Examen
clase-importante-buscando = Recherche de l’essentiel…
clase-importante-tareas = + { $n } tâches vers Tâches
clase-importante-anadidas = { $n } tâches ajoutées à la Boîte de réception des Tâches
tareas-prioridad-urgente = Urgente
tareas-prioridad-alta = Haute
tareas-prioridad-baja = Basse
tareas-prioridad-normal = Normale
tareas-jev-consecuencia-nada = sans conséquence
tareas-jev-consecuencia-leve = conséquence légère
tareas-jev-consecuencia-media = conséquence moyenne
tareas-jev-consecuencia-grave = conséquence grave
tareas-jev-la-esperan = quelqu'un attend
tareas-jev-nadie-espera = personne n'attend
tareas-jev-plazo-ajeno = échéance fixée par un autre
tareas-jev-la-piden = demandée dans le chat
tareas-jev-sin-fecha = sans date
tareas-jev-ya-paso = dépassée
tareas-jev-hoy = aujourd'hui
tareas-jev-manana = demain
tareas-jev-en-dias = dans { $n } jours
tareas-jev-por-la-lista = { $cuando } (d'après le titre de la liste)
anki-jev-quitadas = Jev a écarté { $n } captures sans rien à étudier
## Meeting topics (28-oct-2026, docs/jev.md)
ajustes2-jev-temas = Sujets de réunion abordés
ajustes2-ayuda-jev-temas = Pendant que vous enregistrez une réunion avec des sujets, ce qui est entendu est transcrit sur cet ordinateur toutes les 15 s et Jev coche les sujets déjà abordés. Ce que vous cochez l’emporte. Le texte de chaque passage, le précédent, les sujets et le titre sont envoyés.
ajustes2-jev-dictado = Dicter vers un chat
ajustes2-ayuda-jev-dictado = Quand vous dictez dans la zone du chat (clic droit sur le micro), « note dans le chat du chantier qu’il n’y a plus de vis » propose de le noter dans ce chat : Entrée le note, Échap l’ignore. Chaque phrase dictée, la précédente et les noms de vos chats récents sont envoyés.
ajustes2-jev-repaso = Réviser à voix haute
ajustes2-ayuda-jev-repaso = Dans « Étudier le cours », expliquez le cours à voix haute et Jev marque les points clés que vous couvrez, ceux où vous vous trompez et ceux qui manquent. Ce que vous expliquez, le titre et les points clés sont envoyés.
clase-temas-ninguno = Sujets de la réunion (facultatif)
clase-temas-n = Sujets de la réunion · { $n }
clase-temas-titulo = Sujets de la réunion
clase-temas-sub = Un par ligne. Pendant l’enregistrement, Jev coche ceux qui sont abordés.
clase-temas-escribe = Saisissez un sujet et appuyez sur Entrée…
clase-temas-pegar = Coller
clase-temas-de-lista = Depuis une liste
clase-temas-lista = Liste « { $lista } » : { $n } sujets
clase-temas-sin-listas = Aucune liste de tâches avec des éléments en attente
clase-temas-pegados = { $n } sujets ajoutés
clase-temas-nada-que-pegar = Le presse-papiers ne contient pas de texte
clase-temas-vaciar = Vider
clase-temas-listo = Terminé
clase-temas-anteriores = { $n } sujets restants de la réunion précédente
clase-temas-tratado = ✓ Sujet { $n } abordé · { $minuto }
clase-temas-escuchando = À l’écoute : Jev coche les sujets au fur et à mesure
clase-temas-sin-jev = Pas de Jev (la clé TypeSafe est manquante) : cochez les sujets à la main
clase-temas-apagado = Les sujets avec Jev sont désactivés dans les paramètres : cochez-les à la main
clase-temas-sin-modelo = Sans le modèle vocal, pas d’écoute en direct : cochez les sujets à la main
clase-temas-jev-fallo = Jev ne répond pas : cochez les sujets à la main
clase-temas-cargado = Ordinateur occupé : écoute réduite pour ne pas ralentir l’enregistrement
clase-temas-sin-tratar = Non abordés
clase-temas-proxima = Reporter à la prochaine réunion
clase-temas-a-tareas = Ajouter aux Tâches
clase-temas-pasados = { $n } sujets attendent la prochaine réunion
clase-temas-anadidos = { $n } sujets ajoutés à la Boîte de réception des Tâches
clase-temas-lista-de = Sujets · { $nombre }
## What is heard in the meeting (29-oct-2026, docs/jev.md, «En la reunión»)
ajustes2-jev-tareas-dichas = Tâches dites en réunion
ajustes2-ayuda-jev-tareas-dichas = Pendant que vous enregistrez une réunion, « j’envoie le devis vendredi » apparaît dans la barre comme tâche proposée, avec sa date et son urgence. Elle ne va dans Tâches que si vous l’acceptez. Chaque phrase est envoyée avec les deux précédentes et le titre.
ajustes2-jev-urgencia-en-vivo = Urgence des tâches dite en réunion
ajustes2-ayuda-jev-urgencia-en-vivo = « C’est pour aujourd’hui » augmente la priorité ou change la date d’une tâche en attente de ce chat ou de la Boîte de réception. C’est appliqué automatiquement et annulable depuis la barre. Chaque phrase est envoyée avec le texte d’au plus 8 tâches.
ajustes2-jev-tema-olvidado = Avertir des sujets oubliés
ajustes2-ayuda-jev-tema-olvidado = Avec « Durée prévue » dans Préparer, avertit vers la fin (ou quand on conclut) des sujets de l’ordre du jour pas encore abordés. Chaque phrase et les sujets sont envoyés.
ajustes2-jev-tiempo-por-tema = Minutes par sujet
ajustes2-ayuda-jev-tiempo-por-tema = Combien de temps chaque sujet de l’ordre du jour a été discuté, dans « Étudier le cours » et sur la page partagée. Chaque phrase et les sujets sont envoyés.
ajustes2-jev-decisiones = Décisions de la réunion
ajustes2-ayuda-jev-decisiones = « C’est décidé : on avance la commande » est noté avec sa minute (pas « on pourrait… »). Affiché dans « Étudier le cours » et sur la page partagée. Chaque phrase est envoyée.
ajustes2-jev-preguntas-pendientes = Questions sans réponse
ajustes2-ayuda-jev-preguntas-pendientes = Les questions restées sans réponse pendant la réunion, à la fin dans « Étudier le cours », pour les déplacer vers Tâches. Chaque phrase et les questions encore ouvertes sont envoyées.
ajustes2-jev-fechas-al-horario = Dates dites, vers l’agenda
ajustes2-ayuda-jev-fechas-al-horario = « On se voit mardi à 15 h » apparaît dans la barre comme événement proposé ; si vous l’acceptez, il va dans Tâches avec son heure et s’affiche dans l’Agenda. Chaque phrase est envoyée.
clase-vivo-tarea = Tâche
clase-vivo-evento = Événement
clase-vivo-deshacer = Annuler
clase-vivo-decision = Décision notée · { $minuto }
clase-vivo-tarea-apuntada = Tâche ajoutée à la Boîte de réception des Tâches
clase-vivo-evento-apuntado = Événement ajouté aux Tâches (affiché dans l’Agenda)
clase-vivo-deshecho = Annulé : la tâche est revenue à son état précédent
clase-vivo-no-se-apunto = Impossible de l’ajouter : vérifiez Tâches
clase-vivo-falla = Jev ne répond pas : rien n’est proposé pour l’instant
clase-vivo-queda-poco = Il reste { $minutos } min et il manque encore : { $temas }
clase-vivo-cumplido = Le temps est écoulé et il manque encore : { $temas }
clase-vivo-cierran = On conclut et il manque encore : { $temas }
clase-vivo-fecha-nueva = échéance désormais { $fecha }
clase-vivo-prioridad-normal = Normale
clase-vivo-duracion = Durée prévue
clase-vivo-duracion-n = ⏱ { $minutos } min
clase-acta-decisiones = Décisions
clase-acta-preguntas = Questions sans réponse
clase-acta-propuestas = Propositions non acceptées
clase-acta-aceptar = Accepter
clase-acta-aceptada = Ajoutée aux Tâches
clase-acta-anadidas = { $n } questions ajoutées à la Boîte de réception des Tâches

# --- Lo nuevo de noviembre ---
aviso-ia-abrir = Ouvrir
aviso-ia-si = Oui
aviso-ia-no = Non
aviso-ia-permiso = 🔐 J’ai besoin de votre autorisation : { $pregunta }

    Si vous ne répondez pas, je ne le fais pas.
ia-fondo-titulo = 🤖 Assistant
ia-fondo-sigue = « continue »
ia-fondo-sigue-fuera = « @asistente continue »
ia-fondo-otra-vez = ↻ PinDesk s’est fermé avant que je termine. Je reprends depuis le début dans la réponse ci-dessous.
ia-fondo-ya-actuo = Certaines étapes étaient déjà faites, je ne le refais donc pas de moi-même (elles se feraient deux fois). Écrivez { $sigue } et je continue à partir d’ici.
ia-fondo-ya-se-retomo = Je l’avais déjà repris une fois et cela s’est de nouveau interrompu, je n’insiste donc pas. Écrivez { $sigue } si vous voulez que je réessaie.
ia-fondo-viejo = Trop de temps a passé pour le reprendre de moi-même. Écrivez { $sigue } si vous le voulez encore.
ia-fondo-sin-pregunta = Je ne trouve pas votre demande (a-t-elle été supprimée ?), je ne peux donc pas la reprendre.
ia-fondo-no-se-pudo = Je n’ai pas pu la reprendre : { $aviso }
ia-fondo-no-empezo = je n’ai pas pu commencer à répondre.
ia-fondo-cortado = ⚠️ PinDesk s’est fermé avant la fin. { $porque }
ia-fondo-sigo-en-el-chat = 💬 Je continue dans le chat.
ia-fondo-sigo-de-fondo = ⏳ Je continue votre demande même si le chat est fermé. Je vous préviens quand j’ai fini.
ia-fondo-pregunta = ❓ J’ai besoin que vous me répondiez dans le chat : { $pregunta }

    Appuyez sur « Ouvrir ». Si vous ne répondez pas, je choisis l’option la plus sûre.
leccion-ventana = Leçon
leccion-ventana-nueva = Nouvelle leçon
leccion-titulo = 💡 Leçon
leccion-titulo-nueva = 💡 Nouvelle leçon
leccion-tipo-leccion = 💡 Leçon
leccion-tipo-error = ⚠️ Erreur
leccion-tipo-acierto = ✅ Réussite
leccion-gravedad-leve = Mineure
leccion-gravedad-importante = Importante
leccion-gravedad-grave = Grave
leccion-sin-proyecto = Sans projet (chat général)
leccion-foto-ilegible = Impossible de lire cette photo
leccion-adjunto-no-esta = Ce fichier n’est pas encore sur cet ordinateur
leccion-galeria = 🖼 Galerie
leccion-guardar = Enregistrer
leccion-pista-titulo = Qu’avez-vous appris ? Écrivez-le ou dictez-le d’une traite
leccion-se-parece = Elle ressemble à une leçon que vous avez déjà
leccion-paso-otra-vez = 🔁 C’est arrivé de nouveau
leccion-area = Domaine
leccion-etiquetas = Étiquettes
leccion-anadir-etiqueta = + étiquette
leccion-menos-detalle = ▴  Moins de détails
leccion-mas-detalle = ▾  Ce qui s’est passé, pourquoi et ce que je ferai autrement
leccion-que-paso = Ce qui s’est passé
leccion-por-que = Pourquoi c’est arrivé
leccion-causas = Causes (une touche)
leccion-proxima = La prochaine fois… (si X arrive, je ferai Y)
leccion-referencias = Mots pour la retrouver (séparés par des virgules)
leccion-relacionadas = Liées
leccion-volvio-a-pasar = 🔁 Ça m’est encore arrivé
leccion-volvio-a-pasar-n = 🔁 Ça m’est encore arrivé ({ $veces } fois jusqu’ici)
leccion-mic-escuchando = 🎙️ Je vous écoute… appuyez sur le micro pour terminer
leccion-mic-sin-dictado = Pas de dictée sur cet ordinateur : écrivez la leçon
leccion-mic-sin-texto = Rien n’a été compris
leccion-mic-no-pasa = Impossible de convertir en texte
leccion-mic-corto = Trop court : gardez le micro allumé un peu plus longtemps
leccion-audio = 🎙 Audio
leccion-audio-abriendo = 🎙 Ouverture…
leccion-audio-guardando = 🎙 Enregistrement…
leccion-audio-parar = ⏹ Arrêter { $duracion }
leccion-audio-sin-microfono = Impossible d’utiliser le micro
leccion-audio-corto = Trop court : non enregistré
leccion-audio-no-guarda = Impossible d’enregistrer l’audio
horario-evaluacion-en = { $dias ->
        [1] demain
       *[other] dans { $dias } jours
    }
horario-evaluacion-recuerda = N’oubliez pas : { $que }, { $en }, { $cuando }
horario-evento-en = { $evento } à { $lugar }
horario-dia-0 = Lun
horario-dia-1 = Mar
horario-dia-2 = Mer
horario-dia-3 = Jeu
horario-dia-4 = Ven
horario-dia-5 = Sam
horario-dia-6 = Dim
lista-icono-capturar = Capturer
lista-icono-capturar-largo = Capturer une zone de l’écran
lista-icono-mensajes = Messages enregistrés
lista-icono-mensajes-largo = Ouvrir Messages enregistrés
lista-icono-proyectos = Projets
lista-icono-proyectos-largo = Voir vos projets
lista-icono-sincronizar = Synchroniser
lista-icono-sincronizar-largo = Synchroniser avec vos appareils
lista-icono-recientes = Projets récents
lista-icono-proyecto-largo = Ouvrir le projet { $nombre }
cad-plano-abriendo = Ouverture du plan…
cad-plano-vacio = Le plan n’a rien à afficher
cad-plano-error = Impossible d’ouvrir le plan
cad-plano-pista-acotar = Cliquez sur les points pour mesurer · Double-clic ou Entrée : terminer · Échap : quitter
cad-plano-todo = Voir tout le plan
cad-tema = Fond clair ou sombre
cad-acotar = Coter
cad-borrar-cotas = Effacer les cotes
cad-encima = Toujours au premier plan
cad-cerrar = Fermer
cad-tres = Voir en 3D
cad-marcos = Cadres à imprimer
cad-imprimir = Imprimer…
cad-exportar = Exporter (HTML, PDF, PNG, DXF…)…
cad-borrar-marcos = Effacer les cadres
cad-plano-pista-marcos = Faites glisser pour cadrer ce qui s’imprime · Ctrl+P : imprimer · Suppr : retirer le dernier · Échap : quitter
cad-plano-marcador = Signet ici
cad-anotar = Annoter
cad-sin-dibujo-civil = { $n } objets Civil 3D ont été enregistrés sans leur graphisme : enregistrez avec PROXYGRAPHICS = 1 ou exportez en LandXML
cad-letras = Polices du plan…
cad-letras-faltan = Ce plan demande des polices absentes ; ses textes utilisent des polices proches et peuvent déborder de leurs cadres :
cad-letras-no-falta = Aucune police nommée par le plan ne manque.
cad-letras-donde = Les SHX se trouvent généralement dans C:\Program Files\Autodesk\AutoCAD 20xx\Fonts ou à côté du plan qu’on vous a envoyé ;
    les TrueType, dans C:\Windows\Fonts. Celles que vous ajoutez ici l’emportent sur celles d’AutoCAD, de Windows et de PinDesk.
cad-letras-anadir = Ajouter des polices (SHX, TTF, OTF, TTC)…
cad-letras-quitar = Retirer les miennes ({ $n })
cad-faltan-letras = { $n } polices du plan manquent : appuyez sur Aa en haut pour les ajouter
cad-releyendo = Lecture du plan avec vos polices…
cad-boton-mover = Déplacer (faire glisser)
cad-boton-marcador = Signet
cad-boton-anotar = Annoter
cad-boton-letras = Polices du plan
cad-corte = Zone de coupe
cad-capas = Niveaux et catégories
cad-boton-pantalla = Plein écran
cad-boton-guardar = Enregistrer dans un chat
cad-modelo-abriendo = Ouverture de la maquette…
cad-modelo-vacio = La maquette n’a rien à afficher
cad-modelo-error = Impossible d’ouvrir la maquette
cad-modelo-todo = Voir toute la maquette
cad-aislar = Isoler la sélection
cad-niveles = NIVEAUX
cad-categorias = CATÉGORIES
cad-sin-nivel = Sans niveau
cad-mostrar-todo = Tout afficher
cad-aristas = Arêtes
cad-planta = Vue en plan
cad-altura-corte = Coupe
cad-modelo-pista = Glisser : orbite · Clic droit : déplacer · Molette : zoom · Clic : qu’est-ce que c’est · Autre clic : l’isoler
cad-modelo-marcador = Signet ici (avec la coupe et ce qui est masqué)
cad-imprimir-marcos = Imprimer les cadres…
cad-modelo-pista-marcos = Faites glisser pour cadrer ce qui s’imprime · Ctrl+P : imprimer · Suppr : retirer le dernier · Les numéros : aller à sa vue · Échap : quitter
cad-modelo-pista-anotar = Ce que vous annotez ne se voit que dans la vue de ce signet
cad-modelo-anotar = Annoter cette vue
cad-croquis-abriendo = Ouverture du croquis…
cad-croquis-vacio = Le croquis est vide
cad-croquis-error = Impossible d’ouvrir le croquis
cad-croquis-todo = Voir tout le croquis
cad-croquis-capas = Calques
cad-croquis-grupos = GROUPES
cad-croquis-tipos = TYPES
cad-croquis-todo-corto = Tout
cad-croquis-pista = Glisser : orbite · Clic droit : déplacer · Molette : zoom · 1–9 : vues enregistrées
cad-web-todo = Tout voir
cad-web-tema = Clair / sombre
cad-web-capas = Calques
cad-web-grosores = Épaisseurs
cad-web-anotaciones = Annotations
cad-web-todas = Toutes
cad-web-ninguna = Aucune
cad-web-sin-soporte = Ce navigateur ne sait pas décompresser le plan : ouvrez-le avec une version récente d’Edge, Chrome, Firefox ou Safari.
cad-web-pista = Molette ou pincer : zoom · Deux doigts ou glisser : déplacer · Double-clic : tout voir · F, B, L
lamina-proyecto = PROJET
lamina-lamina = PLANCHE
lamina-titulo = TITRE
lamina-dibujo = DESSINÉ PAR
lamina-escala = ÉCHELLE
lamina-sin-escala = Sans échelle
lamina-fecha = DATE
lamina-unidades = UNITÉS
lamina-opcion = Planche
lamina-con-membrete = Avec cadre et cartouche
lamina-sin-marco = Sans cadre
sinc-fondo-solo = Ce PC n’a personne d’autre dans le groupe : rejoignez-en un dans Synchronisation
sinc-fondo-no-es-del-grupo = Cet appareil ne fait plus partie du groupe
sinc-fondo-aparatos = { $n ->
        [one] { $n } appareil
       *[other] { $n } appareils
    }
sinc-fondo-sincronizando = Synchronisation avec { $quien }…
sinc-fondo-ya-en-marcha = Une synchronisation est déjà en cours : attendez qu’elle se termine
sinc-fondo-fallo = Impossible de synchroniser : { $error }
lector-voz-en-linea = en ligne
auto-origen = Automatisation
ia-nube-falta-la-clave = La clé { $proveedor } manque : collez-la dans Paramètres → Assistant (vous l’obtenez sur { $pagina }).
figuras-ventana-no-es-numero = « { $texto } » n’est pas un nombre
figuras-ventana-mes-mal = « { $texto } » : écrivez le mois sous la forme 2026-11
tecla-intro = Entrée
tecla-supr = Suppr
ia-fondo-hecho = ✅ Terminé.
ia-hecho = Terminé.
croquis-titulo = Croquis { $id }
chat-ajustes-bandeja = Les paramètres s’ouvrent depuis l’icône de PinDesk dans la zone de notification
ia-estado-funciona = fonctionne
ia-estado-sin-clave = pas de clé
ia-estado-desconocido = pas encore vérifié
ia-estado-clave-mala = la clé n’est pas valide ({ $codigo })
ia-estado-sin-saldo = le compte n’a plus de crédit
ia-estado-limite = trop de requêtes ou quota épuisé
ia-estado-red = pas de connexion
ia-estado-servidor = le service est en panne ({ $codigo })
ia-estado-otro = ne répond pas correctement ({ $codigo })
ia-estado-comprobando = vérification…
ia-estado-hace-nada = à l’instant
ia-estado-hace-min = il y a { $n } min
ia-estado-hace-horas = il y a { $n } h
ia-estado-pista = Clic : vérifier maintenant ou aller aux clés
ia-estado-comprobar = Vérifier maintenant
ia-estado-claves = Clés dans Paramètres → Assistant
ia-estado-sin-ajustes = Les paramètres s’ouvrent depuis l’icône de PinDesk dans la zone de notification
chat-mes-dia-1 = janvier
chat-mes-dia-2 = février
chat-mes-dia-3 = mars
chat-mes-dia-4 = avril
chat-mes-dia-5 = mai
chat-mes-dia-6 = juin
chat-mes-dia-7 = juillet
chat-mes-dia-8 = août
chat-mes-dia-9 = septembre
chat-mes-dia-10 = octobre
chat-mes-dia-11 = novembre
chat-mes-dia-12 = décembre
ajustes2-voz-en-vivo = Écouter la réunion en direct avec
ajustes2-voz-en-vivo-sistema = Windows ou Whisper
ajustes2-voz-en-vivo-vosk = Vosk
ajustes2-ayuda-voz-en-vivo = Pour les sujets et le reste de la réunion avec Jev. Vosk affiche ce qui se dit au fur et à mesure et clôt chaque phrase quelques dixièmes de seconde après que vous vous taisez (avec Whisper, environ 3 s), hors ligne. Le choisir télécharge { $megas } Mo (Apache-2.0).
ajustes2-ayuda-voz-en-vivo-vosk-listo = Écoute avec Vosk, hors ligne : ce qui se dit apparaît dans le plein écran de l’enregistrement. S’il ne se charge pas un jour, Windows ou Whisper prend le relais.
ajustes2-ayuda-voz-en-vivo-sin-idioma = Vosk est téléchargé pour l’espagnol et l’anglais ; dans cette langue, c’est Windows ou Whisper qui écoute.
ajustes2-voz-en-vivo-bajando = Téléchargement de Vosk… { $por-ciento } %. En attendant, Windows ou Whisper écoute.
ajustes2-voz-en-vivo-fallo = Impossible de télécharger Vosk : { $fallo }. Choisissez-le à nouveau pour réessayer.
ajustes2-voz-vosk-borrar = Supprimer Vosk
ajustes2-ayuda-voz-vosk-borrar = Libère ce qui a été téléchargé (environ 100 Mo sur le disque). Il est retéléchargé quand vous le choisissez.
clase-temas-bateria = Batterie faible : pas d’écoute en direct pour l’économiser (l’enregistrement continue)
chat-anotar-fallo-red = Pas de connexion : Jev n’a pas pu vérifier s’il fallait le noter dans un autre chat. Ce que vous avez dicté est toujours dans la zone de saisie.
chat-anotar-fallo = Jev n’a pas répondu ({ $motivo }). Ce que vous avez dicté est toujours dans la zone de saisie.
ajustes2-g-privacidad = Confidentialité et données
ajustes2-priv-politica = Politique de confidentialité
ajustes2-ayuda-priv-politica = Ce que PinDesk fait de vos données (en espagnol et en anglais) : tout reste sur ce PC sauf ce qui est indiqué ci-dessous, et seulement quand vous l’utilisez.
ajustes2-g-privacidad-sale = Ce qui quitte ce PC et où ça va
ajustes2-priv-ver-ajustes = Voir ses paramètres
ajustes2-priv-nada-mas = Rien d’autre
ajustes2-ayuda-priv-nada-mas = Pas de publicité, pas d’analytique, pas de rapports de plantage, pas de compte et pas de serveurs PinDesk. La dictée et la transcription se font sur ce PC, hors ligne.
ajustes2-g-privacidad-datos = Vos données
ajustes2-priv-carpeta = Vos données, sur votre disque
ajustes2-ayuda-priv-carpeta = Captures, chats, notes, enregistrements et paramètres se trouvent dans { $carpeta }. Pour les emporter, copiez le dossier ; pour les supprimer, fermez PinDesk et supprimez-le. Les clés de l’IA sont chiffrées avec votre utilisateur Windows.
ajustes2-priv-reportes = Signalements de ce qu’écrit l’IA
ajustes2-ayuda-priv-reportes = { $n ->
    [one] { $n } enregistré dans reportes-ia.jsonl (dans le dossier de données).
   *[other] { $n } enregistrés dans reportes-ia.jsonl (dans le dossier de données).
}
privacidad-asistente-que = Ce que vous demandez à l’assistant, le chat récent et ce que vous joignez
privacidad-asistente-adonde = Anthropic (Claude) ou DeepSeek
privacidad-asistente-cuando = Seulement si vous collez votre propre clé et posez une question. La clé est stockée chiffrée sur ce PC.
privacidad-jev-que = Les textes de chaque fonction de Jev : phrases d’une réunion, lignes d’un cours, tâches, ce que vous dictez ou expliquez
privacidad-jev-adonde = TypeSafe (api.typesafe.ai)
privacidad-jev-cuando = Seulement avec votre clé TypeSafe et cette fonction activée. Jamais l’audio.
privacidad-web-que = Ce que l’assistant recherche et les pages qu’il lit pour vous répondre
privacidad-web-adonde = DuckDuckGo ou Brave Search, et le site qu’il lit
privacidad-web-cuando = Quand l’assistant cherche sur le web. Sans cookies ni votre clé d’IA.
privacidad-google-que = Les fichiers Drive que vous choisissez et les événements de l’agenda
privacidad-google-adonde = Google (Drive, Calendar)
privacidad-google-cuando = Seulement si vous connectez votre compte Google dans Paramètres › Assistant ; vous le déconnectez au même endroit.
privacidad-voces-que = Le texte lu à voix haute avec une voix Microsoft
privacidad-voces-adonde = Microsoft (les voix d’Edge)
privacidad-voces-cuando = Seulement si vous choisissez une de ces voix dans le lecteur ; celles de Windows fonctionnent hors ligne.
privacidad-enlace-que = Le fichier que vous choisissez de partager par lien
privacidad-enlace-adonde = litterbox.catbox.moe ou temp.sh
privacidad-enlace-cuando = Seulement quand vous demandez un lien de partage ; il s’efface de lui-même en quelques heures ou jours.
privacidad-modelos-que = Rien de vous : seuls les fichiers sont demandés
privacidad-modelos-adonde = huggingface.co, github.com et alphacephei.com (Whisper, Qwen3-ASR, Vosk, Indra)
privacidad-modelos-cuando = Lors du téléchargement d’un modèle vocal ou de lecture de texte, quand vous le demandez.
privacidad-sincro-que = Vos captures, notes et chats synchronisés
privacidad-sincro-adonde = Vos propres appareils (le téléphone), via votre réseau
privacidad-sincro-cuando = Lors de la synchronisation : directement sur votre réseau, chiffré, sans passer par des serveurs.
reportar-ia = Signaler la réponse…
reportar-ia-motivo-ofensivo = Offensante
reportar-ia-motivo-peligroso = Dangereuse
reportar-ia-motivo-falso = Fausse ou trompeuse
reportar-ia-motivo-otro = Autre motif
reportar-ia-hecho = Signalement enregistré dans reportes-ia.jsonl ; votre messagerie s’ouvre pour l’envoyer
reportar-ia-asunto = Signalement d’une réponse de l’IA
reportar-ia-campo-motivo = Motif
reportar-ia-campo-funcion = Fonction
reportar-ia-campo-fecha = Date
reportar-ia-campo-version = Version
reportar-ia-campo-contenido = Ce que l’IA a écrit
reportar-ia-comentario = (Si vous voulez, dites-nous ici ce qui ne va pas.)
repaso-jev-fallo-red = Pas de connexion.

## Exporter les tâches : la page, le PDF et le Markdown d'une liste (1-nov-2026)
tareas-exportar-titulo = Tâches
tareas-exportar-exportado = Exporté le { $fecha }
tareas-exportar-pendientes = À faire
tareas-exportar-hechas = Faites
tareas-exportar-completado = Terminé
tareas-exportar-pendiente = À faire
tareas-exportar-hecha = Faite
tareas-exportar-todo-hecho = Tout est fait
tareas-exportar-vacia = Aucune tâche
tareas-exportar-recordatorio = Rappel
tareas-exportar-pasada = En retard
tareas-exportar-creada = Créée le { $fecha }
tareas-exportar-de = { $hechas } sur { $total }
tareas-exportar-foto = Photo
compartir-tipo-tareas = Liste de tâches

## La nube de los chats compartidos (1-nov-2026, sincronizar::nube)
chat-nuevo-nube = Discussion partagée dans le cloud
chat-nuevo-nube-detalle = Ce que vous envoyez, les autres le récupèrent où qu'ils soient
nube-modo-titulo = Mode de synchronisation
nube-modo-local = Réseau local
nube-modo-nube = Cloud
nube-modo-local-detalle = Se synchronise quand vous êtes sur le même Wi-Fi, gratuitement et sans serveur.
nube-modo-nube-detalle = Passe par votre cloud : ce que vous envoyez, les autres le récupèrent où qu'ils soient, et c'est visible sur le web. Seules les modifications sont envoyées.
nube-falta-configurar = Pas encore de cloud : indiquez son adresse et votre clé dans Synchroniser → Cloud.
nube-titulo = Cloud
nube-portada-como = Les discussions partagées réglées sur « Cloud » passent par votre serveur. Les vôtres restent sur le réseau local.
nube-ajustes = Réglages du cloud
nube-explica = Votre serveur conserve les discussions partagées qui passent par le cloud et les affiche sur le web. Les discussions personnelles restent sur le réseau local, sans serveur.
nube-direccion = Adresse web
nube-llave = Clé de l'appareil (pxn_…)
nube-llave-guardada = Clé enregistrée :
nube-olvidar = Oublier
nube-guardar = Enregistrer
nube-probar = Tester
nube-guardado = Enregistré.
nube-direccion-mala = L'adresse doit commencer par https://.
nube-llave-mala = La clé commence par pxn_ (le site vous la donne quand vous enregistrez cet appareil).
nube-probado = Connecté : espace { $espacio }.
nube-no-contesta = Le cloud ne répond pas :
nube-que-va = Seules les modifications sont envoyées : sur un plan annoté, le nouveau trait et non tout le plan.
nube-en-linea = En ligne
nube-sin-conexion = Hors ligne
nube-cerrar-titulo = Modifications hors ligne
nube-cerrar-pregunta =
    Vous avez annoté ce document hors ligne.
    Oui : abandonner ces modifications (celles des autres restent).
    Non : repasser en ligne et les envoyer.
    Annuler : le laisser hors ligne pour la prochaine fois.

## Comentario avanzado: la pegatina con su minichat (3-nov-2026)
comentario-tipo-urgente = Urgent
comentario-tipo-alta = Priorité haute
comentario-tipo-media = Priorité moyenne
comentario-tipo-nota = Note
comentario-abrir = Ouvrir la discussion
comentario-resolver = Résoudre
comentario-reabrir = Rouvrir
comentario-crear-tarea = Créer une tâche
comentario-tarea-creada = Tâche créée dans la boîte de réception
comentario-quitar = Retirer
comentario-lista-titulo = Commentaires
comentario-lista-vacia = Pas encore de commentaires. Choisissez un type et cliquez là où vous voulez le coller.
comentario-plantando = Cliquez là où vous voulez coller le commentaire (Échap pour quitter)
comentario-escribir = Écrivez… (Entrée envoie)
comentario-sin-entradas = Écrivez le premier message, collez une image ou joignez un fichier.
comentario-resuelto = Résolu
comentario-sin-sitio = Ce document n’est pas enregistré : impossible de joindre quoi que ce soit.
comentario-abre-el-chat = Ouvrez la discussion pour aller à ce message.
comentario-elegir-atajo = Raccourci vers…
comentario-sin-atajos = Sélectionnez quelque chose sur la toile, ou ouvrez un document d’une discussion, pour créer un raccourci.
comentario-elemento = Élément
comentario-mensaje = Message de la discussion
comentario-lienzo-propio = Toile
comentario-anotar = Annoter
comentario-anotada = Annotée
