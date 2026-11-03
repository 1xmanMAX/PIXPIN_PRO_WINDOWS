# PinDesk — Indonesian strings.

app-nombre = PinDesk


resultado-copiar = Salin
resultado-guardar-como = Simpan sebagai…
resultado-guardar = Simpan
resultado-descartar = Buang

error-otra-instancia = PinDesk sudah berjalan.
error-atajo-ocupado = Tidak dapat mendaftarkan { $atajo }: aplikasi lain sedang menggunakannya.

captura-guardada = Tangkapan layar disimpan ke { $ruta }
captura-fallo = Tidak dapat menangkap layar: { $motivo }

barra-copiar = Salin
barra-guardar = Simpan
barra-guardar-como = Simpan sebagai…
barra-descartar = Buang

pin-no-encontrado = Tidak ditemukan

pin-copiar = Salin
pin-guardar-como = Simpan sebagai…
pin-abrir-ubicacion = Buka lokasi file
pin-tamano-original = Ukuran asli
pin-grupo = Grup
pin-sin-grupo = Tanpa grup
pin-color-rojo = Merah
pin-color-naranja = Oranye
pin-color-ambar = Kuning tua
pin-color-verde = Hijau
pin-color-cian = Sian
pin-color-azul = Biru
pin-color-violeta = Ungu
pin-color-rosa = Merah muda
pin-ocultar-grupo = Sembunyikan grup ini
pin-cerrar = Tutup
pin-eliminar = Hapus dari penyimpanan…
pin-eliminar-confirmar = Ini akan dihapus permanen dari penyimpanan. Anda yakin?
grupos-ocultos = Grup tersembunyi

capa-guardar-titulo = Anotasi layar
capa-guardar-pregunta = Simpan gambar sebagai pin?
pin-reproducir = Putar
pin-pausar = Jeda
pin-sonido = Suara
barra-todo = Pilih semua
pin-sin-codec = tidak ada codec video
comando-capturar-region = Tangkap
comando-capturar-y-copiar = Tangkap dan salin
comando-capturar-con-scroll = Tangkapan bergulir
comando-cuentagotas = Pengambil warna
comando-pinear = Potong dan pin
comando-pinear-en-vivo = Pin langsung (pantau area secara langsung)
comando-pinear-portapapeles = Pin papan klip
comando-anotar = Anotasi layar
comando-anotar-congelada = Anotasi tangkapan layar
comando-abrir-chat = Buka obrolan proyek
comando-abrir-ajustes = Pengaturan
comando-salir = Keluar
comando-alternar-pines = Sembunyikan atau tampilkan pin
comando-restaurar-ultimo-pin = Buka kembali pin terakhir yang ditutup
comando-cerrar-todos-los-pines = Tutup semua pin
comando-ventana-encima = Jaga jendela di bawah penunjuk tetap di atas
comando-copiar-texto = Salin teks dari area
comando-capturar-y-anotar = Tangkap dan anotasi
comando-capturar-ultima-region = Ulangi tangkapan terakhir
comando-grabar-gif = Rekam area sebagai GIF

# The GIF recording bar (P5b).
grabar-empezar = Rekam
grabar-parar = Hentikan
grabar-pausa = Jeda
grabar-seguir = Lanjutkan
grabar-cerrar = Tutup
grabar-por-segundo = { $ritmo }/dtk
grabar-cuenta-atras = { $segundos }...

# The editor that opens when recording stops (P5b.4).
editor-ver = Putar
editor-pausar = Jeda
editor-anterior = Sebelumnya
editor-siguiente = Berikutnya
editor-velocidad = { $veces }x
editor-guardar = Simpan
editor-guardado-rapido = Cepat
editor-copiar = Salin
editor-descartar = Buang

# Click-through pins (P1.4).
comando-alternar-paso-de-clics = Teruskan klik menembus pin
pin-dejar-pasar-clic = Teruskan klik

# Muting the shortcuts for a while (P1.7).
comando-silenciar-atajos = Bisukan pintasan
bandeja-silenciada = PinDesk — pintasan dibisukan
aviso-atajos-silenciados = Pintasan dibisukan
aviso-atajos-silenciados-detalle = Aktifkan lagi dari menu yang sama.
aviso-atajos-activos = Pintasan aktif
aviso-atajos-activos-detalle = { $cuantos } pintasan merespons lagi.

# Delayed capture (P2.1).
comando-capturar-con-retardo = Tangkap setelah hitung mundur
cuenta-atras-cancelar = Esc untuk berhenti

# Pinning the Explorer selection (P1.6).
comando-pinear-seleccion = Pin pilihan di Explorer

# The settings window (P6).
ajustes-pestana-dibujo = Menggambar
ajustes-iman = Tempelkan ke titik penting
ajustes-iman-esquinas = Sudut dan titik ujung
ajustes-iman-medios = Titik tengah
ajustes-iman-centros = Pusat
ajustes-iman-radio = Jarak tangkap (px)
ajustes-idioma = Bahasa
ajustes-idioma-sistema = Sama dengan Windows
ajustes-arranque = Mulai bersama Windows
ajustes-nivel-auto = Otomatis
ajustes-nivel-completo = Penuh
ajustes-nivel-ligero = Ringan
ajustes-pdf-aligerar = Perkecil PDF saat ditambahkan ke obrolan
ajustes-pdf-nivel-sin-perdida = Persis
ajustes-pdf-nivel-equilibrado = Sedang
ajustes-pdf-nivel-pequeno = Kecil
ajustes-pdf-nivel-extremo = Maks
ajustes-sin-atajo = Tanpa pintasan
ajustes-pulsa-combinacion = Tekan kombinasinya…
ajustes-choca = duplikat

# Reading the text of a pin (P4.2).
pin-copiar-texto = Salin teks dalam gambar

# Opening an image pin in the editor canvas.
pin-abrir-en-lienzo = Buka di kanvas
pin-congelar = Bekukan sebagai gambar
pin-manejar = Kendalikan dari jarak jauh
pin-dejar-de-manejar = Berhenti mengendalikan dari jarak jauh

# Pages of a pinned PDF.
pin-pagina-siguiente = Halaman berikutnya
pin-pagina-anterior = Halaman sebelumnya
pin-extraer-pagina = Ekstrak halaman ini sebagai pin
pin-extraer-todas = Ekstrak semua halaman
aviso-paginas-extraidas = Halaman diekstrak
aviso-paginas-extraidas-detalle = { $hechas } dari { $total }. Sisanya dilewati agar layar tetap bisa digunakan.
chat-titulo = Proyek
chat-sin-proyectos = Belum ada proyek
chat-elige-proyecto = Pilih proyek untuk melihatnya
chat-hojas = { $cuantas ->
   *[other] { $cuantas } lembar
  }
chat-sin-mensajes = Belum ada apa pun di proyek ini
chat-clase-imagen = Gambar
chat-clase-archivo = File
chat-clase-voz = Catatan suara
chat-clase-dibujo = Gambar tangan
chat-clase-pagina = Halaman
chat-clase-proyecto = Proyek
chat-clase-miniapp = Aplikasi mini
chat-escribe = Tulis sesuatu…
# The chat, like PinDesk Android's (guardados/MensajesActivity.kt).
chat-elegidos = { $n ->
   *[other] { $n } dipilih
}
chat-lienzo = Kanvas
chat-tabla = Tabel
chat-viene-de = Berasal dari
chat-copiar = Salin
chat-fijar = Sematkan di atas
chat-soltar = Lepas dari atas
chat-compartir = Bagikan…
chat-abrir-con = Buka dengan…
abrir-con = Buka dengan…
abrir-con-no-existe = File “{ $nombre }” tidak ditemukan: mungkin sudah dipindahkan atau dihapus.
abrir-con-fallo = Windows tidak dapat menampilkan “Buka dengan…” untuk file ini.
chat-aligerar = Perkecil PDF
chat-renombrar = Ganti nama
chat-renombrar-barra = Nama:
chat-renombrar-teclas = Enter menyimpan · Esc mempertahankan yang lama
chat-describir = Tambah keterangan
chat-describir-editar = Edit keterangan
chat-describir-barra = Keterangan:
chat-describir-teclas = Enter menyimpan · Esc membiarkan seperti semula
chat-pinear = Pin
chat-recordar = Ingatkan saya
chat-rescatar = Simpan ini
chat-solo-la-foto = Tampilkan foto saja
chat-ver-hilo = Lihat komentar
chat-reenviar = Teruskan
chat-unir = Tambahkan ke proyek
chat-transcribir = Ubah menjadi teks
chat-letra = Lirik atau teks
chat-borrar = Hapus
chat-borrar-aviso = Ini akan dihapus dari percakapan. File lampiran ikut terhapus. Lanjutkan?
chat-biblioteca-audio = Pustaka audio
chat-fondo = Latar obrolan
chat-escala-puesta = Skala antarmuka: { $cuanto } %
chat-fondo-mar = Laut
chat-fondo-arena = Pasir
chat-fondo-brezo = Semak ungu
chat-fondo-bosque = Hutan
chat-fondo-ocaso = Senja
chat-fondo-pizarra = Batu tulis
chat-adj-telepronter = Teleprompter
chat-adj-pronunciar = Pelafalan
chat-reenviado = Diteruskan
chat-copiado = Disalin
chat-compartir-sin-panel = Windows tidak membuka panel Bagikan: isinya ada di papan klip, tempelkan di mana pun Anda mau
chat-pineado = Di-pin
chat-foto-en-la-nota = Foto ditambahkan ke teks Anda: semuanya dikirim sebagai satu catatan
chat-sin-archivo = Pesan ini tidak memiliki file di komputer ini
chat-sin-origen = Kanvas itu tidak ada di proyek ini
chat-sin-proyectos-otros = Tidak ada proyek lain untuk tujuan pengiriman
chat-no-hay-unir = Di Windows setiap obrolan sudah merupakan proyek: gunakan Teruskan untuk memindahkannya ke proyek lain
chat-ver-dibujo-entero = Tampilkan seluruh gambar
chat-no-se-pudo = Tidak dapat dilakukan: periksa log PinDesk
chat-fecha = { $dia } { $mes }
chat-fecha-con-ano = { $dia } { $mes } { $ano }
chat-mes-1 = Januari
chat-mes-2 = Februari
chat-mes-3 = Maret
chat-mes-4 = April
chat-mes-5 = Mei
chat-mes-6 = Juni
chat-mes-7 = Juli
chat-mes-8 = Agustus
chat-mes-9 = September
chat-mes-10 = Oktober
chat-mes-11 = November
chat-mes-12 = Desember
chat-hoy = Hari ini
chat-ayer = Kemarin
chat-buscar = Cari
chat-adjuntar = Lampirkan
chat-fijado = Pesan disematkan
chat-sistema-memoria-menu = Memori asisten (buka foldernya untuk membaca atau mengeditnya)
info-galeria = Galeri
info-todo = Semua
info-fotos = Foto
info-archivos = File
info-voz = Suara
info-dibujos = Gambar
info-fijados = Disematkan
info-buzon = Kotak masuk
info-videos = Video
info-enlaces = Tautan
info-resumen = { $n ->
   *[other] { $n } item
}  ·  { $codigo }
info-previa-posicion = { $k } dari { $n }
info-vacia = Belum ada apa pun di sini
info-mes = { $mes } { $ano }
info-vacia-galeria = Lembar proyek akan muncul di sini
info-vacia-fotos = Foto dari obrolan ini akan muncul di sini
info-vacia-videos = Video dari obrolan ini akan muncul di sini
info-vacia-archivos = File dari obrolan ini akan muncul di sini
info-vacia-enlaces = Tautan dari obrolan ini akan muncul di sini
info-vacia-voz = Catatan suara akan muncul di sini
info-vacia-dibujos = Gambar dari obrolan ini akan muncul di sini
info-vacia-fijados = Yang Anda sematkan akan muncul di sini
info-vacia-buzon = Kotak masuk kosong
info-titulo = Info
adjuntar-imagen = Foto atau video
adjuntar-archivo = File
adjuntar-lienzo = Kanvas baru
confirmar-pie = Tambahkan keterangan…
confirmar-cancelar = Batal
adjuntar-tabla = Tabel baru
tabla-nueva = Tabel
chat-nombre-nuevo = Nama proyek…
hoja-volver = Esc untuk kembali
menu-foto-lienzo = Buka di kanvas
menu-foto-pin = Pin lagi
menu-foto-abrir = Buka dengan Windows
recibir-titulo = Terima dari ponsel
recibir-como = Pindai kode dengan PinDesk di ponsel Anda
recibir-esperando = Menunggu ponsel…
recibir-hecho = { $cuantos } item tiba
recibir-salir = Esc untuk menutup
comando-recibir-del-movil = Terima dari ponsel…
comando-sincronizar = Sinkronkan
adjuntar-del-movil = Ambil dari ponsel…
proyecto-renombrar = Ganti nama
# The chat photo (25-oct-2026): image, emoji or colour in the circle.
foto-chat-menu = Ubah foto obrolan…
foto-chat-titulo = Foto obrolan
foto-chat-imagen = Pilih gambar…
foto-chat-pegar = Tempel gambar
foto-chat-quitar = Bawaan
foto-chat-buscar = Cari emoji
foto-chat-sin-emojis = Tidak ada emoji dengan nama itu
foto-chat-fondo = Latar
foto-chat-recorte = Seret untuk memindahkan · gulir untuk zoom
foto-chat-guardar = Simpan
foto-chat-cancelar = Batal
foto-chat-sin-imagen = Tidak ada gambar yang disalin
foto-chat-error = Tidak dapat menyimpan foto obrolan
proyecto-borrar = Hapus proyek
proyecto-borrar-varios = Hapus yang dipilih
proyecto-borrar-aviso = Proyek akan keluar dari daftar dan foldernya masuk ke tempat sampah PinDesk. Lanjutkan?
proyecto-papelera = Proyek yang dihapus…
# Where each project is saved: its chosen folder or the usual place.
ubicacion-cambiar = Ubah lokasi…
ubicacion-habitual = Kembalikan ke lokasi biasa
ubicacion-abrir = Buka folder proyek
ubicacion-elegir-titulo = Pilih tempat menyimpan proyek
ubicacion-nuevo = Proyek baru
ubicacion-nuevo-en = Proyek baru di folder…
ubicacion-no-disponible = Lokasi tidak tersedia
ubicacion-no-disponible-abrir = Folder proyek ini tidak tersedia ({ $ruta }). Sambungkan drive-nya dan coba lagi.
ubicacion-movido = Proyek disimpan di { $ruta }
ubicacion-vuelto = Proyek kembali ke lokasi biasa
ubicacion-error-red = Proyek hanya bisa disimpan di drive komputer ini, bukan di drive jaringan.
ubicacion-error-no-es-carpeta = Folder itu tidak ada di komputer ini.
ubicacion-error-dentro = Folder itu berada di dalam proyek itu sendiri atau di dalam data PinDesk.
ubicacion-error-mover = Proyek tidak dapat dipindahkan; proyek tetap di tempat semula.
ubicacion-error-abrir = Folder proyek tidak dapat dibuka.

## Sync (copy of the phone's SincronizarActivity.kt)
sinc-titulo = Sinkronkan
sinc-subtitulo = Lewat Wi-Fi, tanpa internet.
sinc-este-aparato = Perangkat ini
sinc-tus-aparatos = Perangkat Anda
sinc-sin-grupo = Buat grup di salah satu perangkat Anda dan bergabunglah dari perangkat lain dengan kodenya. Hanya perangkat dalam grup yang saling sinkron.
sinc-crear-grupo = Buat grup
sinc-unirme = Gabung dengan kode
sinc-grupo-vacio = Belum ada perangkat lain di grup. Tambahkan dengan kode di bawah.
sinc-disponible = Tersedia
sinc-no-encontrado = Tidak ditemukan
sinc-sincronizar = Sinkronkan
sinc-probar = Coba
sinc-con-todos = Sinkronkan dengan semua ({ $n })
sinc-con-todos-como = Tanpa bertanya: memakai pilihan terakhir Anda untuk masing-masing.
sinc-si-no-aparece = Jika tidak muncul: perangkat itu harus membuka PinDesk dan memakai Wi-Fi ini.
sinc-con-direccion = Sambungkan ke alamat
sinc-anadir = Tambah perangkat lain
sinc-anadir-como = Di perangkat lain: Sinkronkan → “Gabung dengan kode”.
sinc-toca-codigo = Klik untuk melihat kode
sinc-qr-como = Pindai dengan kamera ponsel: PinDesk terbuka dan bergabung sendiri. Disembunyikan dalam 10 menit.
sinc-red-titulo = PinDesk tidak punya izin jaringan
sinc-red-texto = Firewall Windows tidak mengizinkan perangkat Anda yang lain menjangkau PinDesk, jadi perangkat itu tidak muncul saat bergabung dengan kode. Izinkan sekali: Windows akan meminta konfirmasi sebagai administrator.
sinc-red-boton = Izinkan akses jaringan
sinc-red-pidiendo = Menunggu izin Windows…
sinc-red-listo = Akses jaringan diizinkan
sinc-red-hecho = Selesai: perangkat Anda yang lain kini bisa menemukan PinDesk di jaringan ini.
sinc-red-publica = PinDesk sudah diizinkan di jaringan pribadi, tetapi Wi‑Fi ini ditandai «Publik». Jadikan pribadi di Pengaturan → Jaringan & internet → Wi‑Fi → jaringan ini.
sinc-red-cancelado = Izin tidak diberikan. Anda bisa mencoba lagi kapan saja.
sinc-red-fallo = Windows tidak mengizinkan penambahan izin (mungkin dicegah kebijakan di PC ini). Minta bantuan pengelola PC ini.
sinc-salir = Keluar dari grup
sinc-pasar = Kirim sesuatu ke seseorang
sinc-pasar-como = Sekali saja, ke siapa pun yang memakai PinDesk di Wi-Fi Anda. Tidak perlu grup.
sinc-recibir = Terima
sinc-enviar = Kirim
sinc-pasar-tambien = Yang diterima masuk ke daftar proyek atau ke “Pesan tersimpan”.
sinc-actividad = Aktivitas
sinc-escuchando = Komputer ini dapat ditemukan di { $dir } selama jendela ini terbuka.
sinc-nombre-titulo = Nama perangkat ini
sinc-nombre-como = Begini perangkat lain Anda akan melihatnya.
sinc-unirme-titulo = Gabung ke grup
sinc-unirme-como = Ketik kode yang tampil di bawah “Tambah perangkat lain” pada perangkat di grup. Perangkat itu harus membuka PinDesk dan memakai Wi-Fi yang sama.
sinc-codigo = Kode
sinc-direccion = Alamat (hanya jika tidak ditemukan)
sinc-direccion-titulo = Sambungkan ke alamat
sinc-direccion-como = Ketik alamat yang tampil di perangkat lain, di jendela yang sama ini.
sinc-la-de-este = Alamat perangkat ini:
sinc-salir-titulo = Keluar dari grup?
sinc-salir-como = Perangkat ini akan berhenti sinkron dengan yang lain. Tidak ada yang dihapus. Jika Anda bergabung lagi, pertama kali akan ditanyakan apa yang berubah di kedua sisi.
sinc-salir-boton = Keluar
sinc-un-momento = Sebentar
sinc-no-se-pudo = Tidak berhasil
sinc-guardar = Simpan
sinc-unirme-boton = Gabung
sinc-conectar = Sambungkan
sinc-cancelar = Batal
sinc-ocultar = Sembunyikan
sinc-vale = OK
sinc-con-todos-rondas = Tanpa bertanya, satu per satu dan dalam dua putaran, agar semua akhirnya punya semuanya.
sinc-elegir-titulo = Apa yang disinkronkan?
sinc-elegir-con = Dengan { $otro }. Yang Anda centang akan sama di keduanya.
sinc-en-los-dos = Di keduanya
sinc-solo-en-uno = Hanya di satu
sinc-este-aparato-n = Perangkat ini · { $n }
sinc-elegir-como = Yang berubah di kedua sisi digabungkan: bentuk demi bentuk, sel demi sel, paragraf demi paragraf. Jika keduanya mengubah hal yang sama, perubahan terakhir yang menang, dan yang sebelumnya tetap ada di «Cadangan».
sinc-todo = Semua
sinc-nada = Tidak ada
sinc-sincronizar-n = Sinkronkan { $n }
sinc-sincronizando = Menyinkronkan
sincro-borrado-alli-vuelve = Dihapus di { $otro } · akan dikirim kembali
sincro-borrado-alli-se-borra = Dihapus di { $otro } · AKAN DIHAPUS DI SINI (cadangan disimpan)
sincro-borrado-alli-se-queda = Dihapus di { $otro } · tidak dicentang: tetap di sini
sincro-se-borran-aqui = Akan dihapus DI SINI, karena sudah dihapus di { $otro }: { $lista }. Hapus centangnya untuk menyimpan, atau pilih «Milik saya menang» untuk mengirimnya kembali.
sincro-se-borran-alli = Akan dihapus di { $otro }, karena Anda menghapusnya di sini: { $lista }.
sincro-juntar = Gabungkan
sincro-lo-mio-manda = Milik saya menang
sincro-lo-mio-manda-como = Satu arah saja: isi { $yo } menimpa { $otro }. Di sini tidak ada yang dihapus atau diubah; di sana, yang berbeda akan menjadi seperti di sini dan yang mereka hapus akan kembali. Yang hanya ada di { $otro } tetap disimpan. Cadangan dari yang ditimpa tetap disimpan.
sincro-mandar-n = Kirim { $n }

## Receive over Wi-Fi (copy of the phone's RecibirActivity.kt)
rw-titulo = Terima lewat Wi-Fi
rw-misma-wifi = Anda harus berada di Wi-Fi yang sama dengan pengirim (atau tersambung ke hotspot-nya).
rw-o-escribe = atau ketik kodenya
rw-pista-pega = Komputer tidak punya kamera: ketik enam digitnya atau tempel teks QR-nya dengan Ctrl+V.
rw-recibir = Terima
rw-prefieres = Lebih suka dipindai? Tampilkan kode Anda sendiri dan biarkan pengirim mengarahkan kameranya.
rw-ensenar = Tampilkan kode saya agar mereka bisa mengirim ke saya
rw-que-te-escaneen = Biarkan mereka memindai ini
rw-esperando-mio = Menunggu seseorang mengirimi Anda sesuatu. Anda juga bisa membacakan kode ini untuk «Kirim → Pindai penerima».
rw-buscando = Mencari pengirim…
rw-conectando = Menyambungkan…
rw-no-encontrado = Tidak ada yang mengirim dengan kode itu. Periksa kodenya dan pastikan Anda berdua di Wi-Fi yang sama (atau di hotspot salah satunya).
rw-codigo-malo = Kode itu salah. Periksa lagi.
rw-se-corto = Terputus: { $motivo }
rw-quiere = { $de } ingin mengirimi Anda
rw-aparato = Perangkat { $codigo }
rw-proyecto = Proyek «{ $nombre }»
rw-lienzo = Kanvas «{ $nombre }»
rw-nada-coincide = Tidak ada yang cocok dengan milik Anda: semuanya masuk sebagai baru, tanpa menyentuh milik Anda.
rw-no-gracias = Tidak, terima kasih
rw-aceptar = Terima
rw-recibiendo = Menerima dari { $de }
rw-de = { $hechos } dari { $total }
rw-recibido = Diterima dari { $de }
rw-no-guardado = Sudah tiba, tetapi tidak dapat disimpan.
rw-en-proyectos = { $nombre }: di daftar proyek
rw-en-guardados = { $nombre }: di «Pesan tersimpan»
rw-en-carpeta = { $nombre }: hanya di folder «recibidos»
rw-no-se-pudo = Tidak berhasil
rw-probar-otra-vez = Coba lagi
rw-cerrar = Tutup

## Send over Wi-Fi (copy of the phone's EnviarActivity.kt)
ew-titulo = Kirim lewat Wi-Fi
ew-que = Apa yang ingin Anda kirim?
ew-que-como = Di ponsel Anda memilih saat berbagi; di sini, file dari komputer ini atau seluruh proyek dari obrolan.
ew-archivos = File
ew-un-proyecto = Proyek
ew-elige-proyecto = Pilih proyeknya
ew-sin-proyectos = Belum ada proyek.
ew-preparando = Menyiapkan…
ew-no-preparado = Yang ingin Anda kirim tidak dapat disiapkan.
ew-vas-a-enviar = Anda mengirim
ew-vas-a-enviar-n = Anda mengirim { $n } item
ew-y-mas = dan { $n } lainnya
ew-un-aparato = Satu perangkat
ew-n-aparatos = { $n } perangkat
ew-pidiendo = Ingin menerima yang Anda kirim
ew-enviando = Mengirim · { $hechos } dari { $total }
ew-esperando-acepte = Menunggu mereka menerima…
ew-enviado = Terkirim
ew-no-acepto = Ditolak
ew-no-aprobado = Tidak disetujui
ew-no = Tidak
ew-enviar = Kirim
ew-enviar-a-todos = Kirim ke semua yang menunggu
ew-pegar = Tempel kode penerima
ew-pegar-como = Di perangkat lain: Terima lewat Wi-Fi → «Tampilkan kode saya agar mereka bisa mengirim ke saya». Ketik enam digitnya di sini atau tempel teks QR-nya dengan Ctrl+V.
ew-mandar = Kirimkan
ew-dejar-de-pegar = Berhenti menempel
ew-en-el-otro = Di ponsel lain: PinDesk → Sinkronkan → Terima
ew-o-escribe = atau ketik kode ini
ew-misma-wifi = Keduanya di Wi-Fi yang sama
ew-misma-wifi-como = Jika tidak ada Wi-Fi, salah satu bisa menyalakan hotspot dan yang lain bergabung. Internet tidak digunakan.
ew-malos = Seseorang mencoba kode yang salah ({ $n } dari { $de }).
ew-esperando = Menunggu… Beberapa perangkat bisa tersambung; Anda menekan tombol untuk masing-masing. Kode berhenti berlaku saat layar ini ditutup.
ew-quemado = Seseorang beberapa kali mencoba kode yang salah, jadi kode ini tidak berlaku lagi. Kirim ulang untuk mendapatkan kode baru.
ew-cerrar = Tutup
ew-grupo = Perangkat Anda · tanpa kode
ew-grupo-como = Mereka ada di grup Anda: langsung terkirim, tanpa pindai. Masuk ke apa pun yang sedang mereka buka (obrolan; kanvas hanya menerima foto) atau ke Pesan tersimpan.
ew-grupo-se-ve = Di Wi-Fi yang sama
ew-grupo-no-se-ve = Tidak terlihat sekarang: buka PinDesk di perangkat itu
ew-grupo-enviando = Mengirim…
ew-grupo-todos = Kirim ke semua yang terlihat
ew-grupo-actualiza = Perbarui PinDesk di { $movil }: belum bisa menerima file langsung. Biarkan memakai kode di bawah.

## «Update mine / Create as new» when receiving (RecibirActivity.kt)
rw-ya-tienes = Anda sudah punya «{ $nombre }» dengan kode yang sama
rw-actualizar = Perbarui yang saya punya
rw-como-nuevo = Buat sebagai baru
rw-encima-de-lo-tuyo = Akan ditulis di atas milik Anda. Isi sebelumnya tetap di «Cadangan».
rw-entra-aparte = Masuk terpisah, dengan kode baru: mulai sekarang menjadi item lain.
rw-proyecto-al-dia = «{ $nombre }» diperbarui
rw-guardados-al-dia = «{ $nombre }» diperbarui di Pesan tersimpan

# The header magnifier, which searches inside the conversation (task 1.3).
chat-buscar-aqui = Cari di sini…
chat-buscar-nada = Tidak ada yang cocok
chat-buscar-resultados = { $n ->
   *[other] { $n } hasil
}
chat-buscar-en-contexto = Lihat dalam konteks
# The list search box also searches inside every chat: the section under the
# chats and its "show more" (25-oct-2026).
chat-buscar-mensajes = { $n ->
   *[other] Pesan · { $n }
}
chat-buscar-ver-mas = Tampilkan { $n } lagi
chat-buscar-en = di { $chat }
chat-pagina-n = Halaman { $n }
chat-unido-a = Ditambahkan ke "{ $nombre }"
chat-unir-nada = Tidak ada yang bisa ditambahkan di sini: hanya foto, gambar, catatan, dan PDF
chat-devolver = Tambahkan kembali ke proyek
chat-devuelto = Kembali di "{ $nombre }"
chat-devolver-no = Tidak dapat menambahkannya kembali
unir-pdf-como-imagenes = Tambahkan sebagai gambar
unir-pdf-empezado = Menambahkan PDF ke proyek…
unir-pdf-ya-va = PDF itu sedang ditambahkan
unir-pdf-no = Tidak dapat menambahkan PDF
unir-pdf-hecho = { $hojas ->
   *[other] { $hojas } lembar ditambahkan ke proyek
}
unir-pdf-pintadas = { $hojas ->
   *[other] { $hojas } lembar ditambahkan (PDF tidak dapat disalin: digambar ulang)
}
unir-pdf-fotos = { $hojas ->
   *[other] { $hojas } dari { $paginas } halaman ditambahkan sebagai gambar
}
unir-pdf-pintando = Menggambar halaman… { $hechas } dari { $total }
unir-pdf-imagenes-progreso = Mengubah menjadi gambar… { $hechas } dari { $total }

## Backups (copy of sincro/CopiasActivity.kt from the phone)
cop-titulo = Cadangan
cop-portada-como = Keadaan setiap percakapan sebelum menerima sesuatu lewat Wi-Fi dan sebelum setiap sinkronisasi.
cop-ver = Lihat cadangan
cop-versiones = Versi sebelumnya
cop-como-estaba = Keadaan proyek sebelum setiap transfer yang diterima dan setiap sinkronisasi. Kembali ke salah satunya tidak menghapus yang Anda lakukan setelahnya.
cop-todavia-nada = Belum ada cadangan. Cadangan dibuat otomatis sebelum menerima sesuatu lewat Wi-Fi dan sebelum sinkronisasi.
cop-como-se-hacen = 30 cadangan terakhir dari setiap percakapan disimpan. File di atas 40 MB tidak masuk cadangan: untuk itu tidak bisa kembali.
cop-hojas-mensajes = { $hojas } lembar · { $mensajes } pesan
cop-sin-grandes =  · tanpa PDF (terlalu besar)
cop-volver = Kembalikan
cop-cancelar = Batal
cop-volver-titulo = Kembali ke cadangan ini?
cop-volver-texto = «{ $nombre }» kembali seperti pada { $cuando } ({ $motivo }): { $hojas } lembar. Kanvas yang Anda tambahkan setelahnya tetap ada. Cadangan keadaan sekarang disimpan lebih dulu, jadi ini bisa dibatalkan.
cop-ha-vuelto = «{ $nombre }» sudah kembali ke cadangan
cop-no-se-pudo = Tidak dapat kembali ke cadangan: { $motivo }
papelera-titulo = Proyek yang dihapus
papelera-como = Proyek kembali utuh: dengan namanya, lembarnya berurutan, PDF-nya, dan obrolannya. Dan tidak lagi dianggap terhapus, jadi sinkronisasi tidak akan menghapusnya lagi.
papelera-detalle = { $hojas } lembar · { $mensajes } pesan · dihapus pada { $cuando }
papelera-recuperar = Pulihkan
papelera-recuperar-todos = Pulihkan semua
papelera-ha-vuelto = «{ $nombre }» kembali, utuh
papelera-no-se-pudo = Tidak dapat memulihkan «{ $nombre }»: { $motivo }
papelera-recuperados = { $n } dari { $total } proyek dipulihkan

## The document viewer: Word, EPUB books and web pages (task 2.1)
## The Office pin: Office's preview as a pin (2026-10-10)
office-titulo = Pratinjau Office
office-abriendo = Membuka dengan pratinjau Office…
office-tarda = Office memerlukan waktu (harus dijalankan dulu). Anda bisa menunggu, atau memilih aplikasi dengan “Buka dengan…” (tombol ↗ di bilah).
office-fallo = Office tidak dapat menampilkan pratinjau file ini. Buka di aplikasi lain dengan “Buka dengan…” (tombol ↗ di bilah).
office-internet = File ini berasal dari internet dan, demi keamanan, Windows tidak mengizinkan pratinjaunya. Buka di aplikasi lain dengan “Buka dengan…” (tombol ↗ di bilah), atau buka blokirnya di Properti.
visor-titulo = Penampil PinDesk
visor-tamano = Ukuran teks
visor-quitar-marcadores = Hapus penanda
visor-guardar-pagina = Simpan sebagai halaman web
visor-guardar-pdf = Simpan sebagai PDF
visor-abrir-carpeta = Buka folder
visor-guardado = Disimpan ke "{ $nombre }"
visor-no-guardado = Tidak dapat menyimpan
visor-vacio = Dokumen ini tidak punya teks untuk ditampilkan

## The readers: comfortable reading, annotating on top and the PDF reader
lector-indice = Daftar isi
lector-letra-fijada = Huruf dikunci: ada catatan yang digambar di atasnya
lector-letra-fija = Monospace
lector-grosor-normal = Biasa
lector-grosor-gruesa = Tebal
lector-quitar-tinta = Hapus catatan
lector-tinta-quitada = Catatan dihapus (Ctrl+Z mengembalikannya selama Anda tetap di sini)
lector-ayuda-docs = Gulir untuk membaca · Ctrl+gulir zoom · L dengarkan · A anotasi · M penanda · I daftar isi · +/− ukuran · T huruf · B ketebalan · Esc untuk keluar
lector-pdf-titulo = Pembaca PDF PinDesk
lector-pdf-no-se-abre = Tidak dapat membuka PDF
lector-pdf-abriendo = Membuka PDF…
lector-pdf-reconociendo = Mengenali teks di halaman…
lector-pdf-reconocido = Teks dikenali: sekarang bisa dipilih dan ditandai
lector-pdf-sin-texto = Tidak ada teks di halaman ini
lector-exportando = Menyimpan PDF beranotasi… { $hechas } dari { $total }
lector-anotado = beranotasi
lector-office-titulo = Word dan PowerPoint di PinDesk
lector-presentar = Presentasi (F5)
lector-modo-lectura = Mode baca (teks disusun ulang)
lector-imprimir = Cetak… (Ctrl+P)
lector-no-imprime = Tidak dapat membuka dialog cetak
lector-copiado = Disalin
lector-escuchar = Dengarkan (L)
lector-sin-texto = Dokumen ini tidak punya teks untuk dibaca
lector-leyendo-texto = Mengambil teks dari PDF untuk membacanya…
lector-sin-voz = Windows tidak punya suara untuk “{ $idioma }”: tambahkan di Pengaturan > Waktu & bahasa > Ucapan
lector-voz-idioma = id-ID
lector-voces-microsoft-no = ☐ Suara Microsoft · sangat alami dan gratis; bukan layanan resmi, dan teksnya dikirim ke Microsoft
lector-voces-microsoft-si = ☑ Suara Microsoft · sangat alami dan gratis; bukan layanan resmi, dan teksnya dikirim ke Microsoft
lector-voz-de-microsoft = Suara Microsoft: { $voz }
lector-voz-del-pais = suara negara
lector-voz-pidiendo = Meminta daftar suara ke Microsoft… tekan lagi sebentar lagi
lector-voz-sin-voces = Tidak ada jaringan, atau Microsoft tidak punya suara untuk bahasa ini
lector-voz-de-ia = AI, membaca bahasa apa pun
lector-voz-mujer = perempuan
lector-voz-hombre = laki-laki
lector-voz-parrafo = Paragraf { $n } dari { $total }
lector-voz-preparando = Menyiapkan suara…
lector-voz-no-respondio = Microsoft tidak menjawab
lector-voz-sin-voz = Tidak ada suara
lector-espacio-tope = Tidak ada ruang lagi di sisi itu
lector-espacio-nada = Sisi itu sudah tidak punya ruang
lector-sin-lado = Gerak ke samping dikunci: hanya gulir ke atas dan ke bawah
lector-con-lado = Bisa bergerak ke segala arah
lector-auto = Gulir otomatis (D)
lector-auto-ppm = { $ppm } kata/menit
lector-auto-quedan = Selesai dalam { $cuanto } · pukul { $hora }
lector-auto-menos-de-un-minuto = kurang dari 1 mnt
lector-auto-terminado = Selesai
lector-auto-contando = Menghitung kata…
lector-ver-como-texto = Lihat sebagai teks (seperti file Word)
# Lectura Veloz, the speed-reading app (27-Oct-2026).
lector-lectura-veloz = ⚡ Baca cepat
lector-lectura-veloz-no-instalada = Lectura Veloz tidak terpasang di PC ini (tautan lecturaveloz:// tidak ada). Pasang lalu coba lagi.
lector-lectura-veloz-no-se-pudo = Tidak dapat membuka Lectura Veloz: { $motivo }
# Speed reading (27-oct-2026): the % next to the option, the bubble chip
# and the real reason when it can't open.
lector-lectura-veloz-con = ⚡ Baca cepat · { $pct } %
lector-lectura-veloz-sin-codigo = Dokumen ini tidak bisa dibaca di Lectura Veloz: bukan file obrolan atau catatan tersimpan (tidak punya kode pesan).
lector-lectura-veloz-formato = Dokumen ini tidak bisa dibaca di Lectura Veloz: file «.{ $ext }» tidak didukung.
lector-lectura-veloz-falta-archivo = Dokumen ini tidak bisa dibaca di Lectura Veloz: filenya sudah tidak ada di komputer ini ({ $nombre }).
chat-lectura-veloz-chapa = Baca cepat · { $pct } %
# The chip of what came from the phone's scanner (`escaneado`).
chat-escaneado = Dipindai
lector-pasando-a-texto = Mengubah PDF menjadi teks…
lector-texto-escaneo = PDF ini tidak punya teks untuk diambil: kemungkinan hasil pindaian (foto halaman)
lector-texto-cifrado = PDF dilindungi kata sandi
lector-texto-no-se-lee = PDF tidak dapat dibaca
lector-ver-pdf-original = Lihat PDF asli, per halaman
lector-hoja = Halaman
sinc-presencia-apagada = Kehadiran dinonaktifkan di pengaturan ([sincro] presencia = false): Anda bisa sinkron dari sini, tetapi ponsel tidak akan menemukan komputer ini atau bisa memanggilnya.
bandeja-abrir-documento = Buka dokumen…

## Canvas shapes and ink: arrowheads, fills and text style (group A)

## Build and measure: fill, trim, extend, points and angles (group C)

## Select, transform and bind (group B)

# The attachment the document viewer can open without leaving PinDesk.

# The seven mini-apps from the phone, and their panel.
mini-moneda = USD
mini-tareas = Daftar tugas
mini-gastos = Pengeluaran
mini-cronometro = Stopwatch
mini-temporizador = Timer
mini-contador = Penghitung
mini-ruleta = Roda putar
mini-alarma = Alarm
mini-tarea-nueva = Sesuatu yang harus dilakukan, lalu Enter · ↑↓ pilih, Spasi centang, F2 edit
mini-gasto-nuevo = Item dan jumlah (Makan malam 42.50), lalu Enter · ↑↓ pilih, F2 edit
mini-nombre-nuevo = Sebuah nama, lalu Enter · Enter saat kosong memutar, Del menghapus
mini-limpiar-hechas = Hapus yang selesai
mini-ocultar-hechas = Sembunyikan yang selesai
mini-mostrar-hechas = Tampilkan yang selesai
mini-avance = { $hechas } dari { $de } selesai
mini-tarea-edad = { $dias ->
    [0] hari ini
   *[other] { $dias } hari lalu
  }
mini-arrancar = Mulai
mini-parar = Berhenti
mini-vuelta = Putaran
mini-reiniciar = Atur ulang
mini-sortear = Putar
mini-alarma-encender = Nyalakan
mini-alarma-apagar = Matikan
mini-guia-titulo = Nama baru, lalu tekan Enter (Esc mempertahankan yang lama)
mini-guia-cambiar = Perbaiki dan tekan Enter (Esc membiarkannya seperti semula)
mini-guia-duracion = Menit atau m:dd, lalu Enter · Spasi memulai
mini-guia-hora = Waktu (7:30), lalu Enter · Spasi menyalakan
chat-biblioteca-vacia = Belum ada catatan suara atau lagu.
chat-biblioteca-musica = Musik
chat-biblioteca-notas = Catatan suara

# Recording a voice note from the chat.
chat-voz-muy-corta = Terlalu pendek: tahan sedikit lebih lama.
chat-voz-sin-microfono = Tidak ada mikrofon, atau PinDesk tidak diizinkan memakainya. Periksa Pengaturan › Privasi › Mikrofon.
chat-voz-sin-codec = Windows ini tidak punya encoder audio (biasanya edisi N tanpa paket fitur media).
chat-voz-fallo = Catatan suara tidak dapat direkam.

# Shrinking an attached PDF: it lowers the photos inside, and never for the worse.
pdf-aligerado = Diperkecil: dari { $antes } menjadi { $despues }
pdf-aligerar-cifrado = PDF dienkripsi dan tidak bisa ditulis ulang tanpa risiko.
pdf-aligerar-no-se-lee = File yang diperkecil tidak terbaca dengan benar; file asli dipertahankan.
aligerar-progreso = Memperkecil… { $por }%
aligerar-empezado = Memperkecil PDF…
aligerar-sin-mejora = PDF ini sudah ringan: dibiarkan seperti semula.

# Reminders: the time set on a message and the notice that shows up when it
# arrives. The same times the phone offers, plus ten minutes.
chat-recordar-10-min = Dalam 10 menit
chat-recordar-1-hora = Dalam 1 jam
chat-recordar-3-horas = Dalam 3 jam
chat-recordar-esta-tarde = Sore ini (18:00)
chat-recordar-manana = Besok (9:00)
chat-recordatorio-puesto = Saya akan mengingatkan Anda pada { $cuando }
chat-recordatorio-quitar = Hapus pengingat
chat-recordatorio-quitado = Pengingat dihapus
chat-recordatorio-titulo = Pengingat

# A message menu entry: send the attachment to someone over Wi-Fi.

# Turning a voice note into text with Vosk. What is missing is named with its
# file and link: PinDesk never downloads anything on its own.
chat-transcribiendo = Mengubah menjadi teks… { $pct }%
chat-transcribir-hecha = Catatan suara diubah menjadi teks
chat-transcribir-en-marcha = Catatan lain sedang diubah; tunggu sampai selesai
chat-transcribir-sin-mensaje = Catatan sudah tidak ada di percakapan: teksnya tidak punya tujuan
chat-transcribir-cancelada = Pengubahan ke teks dibatalkan
chat-transcribir-otra-vez = Ubah menjadi teks lagi
chat-transcripcion-no = Tidak dapat mengubah menjadi teks: tidak ada yang dipahami dari audio.
chat-letra-vacia = Catatan ini belum punya teks.
chat-letra-copiada = Teks disalin, beserta penanda waktunya

## Music with its lyrics (karaoke, 27-Oct-2026)
chat-adj-musica = Musik
musica-anadida = Lagu ditambahkan. Tekan pensil untuk mengetik atau menempel liriknya
musica-anadidas = { $n } lagu ditambahkan
musica-no-es-audio = “{ $nombre }” bukan audio yang bisa diputar
musica-sin-letra = Lagu ini belum punya lirik
musica-escribir-letra = Tekan pensil untuk mengetik atau menempel lirik (teks biasa atau LRC)
musica-sinc-primero-letra = Ketik atau tempel liriknya dulu
musica-sinc-empieza = Tekan Spasi (atau “Tandai”) tepat saat setiap baris dimulai
musica-sinc-siguiente = Berikutnya: { $linea }
musica-sinc-todas = Semua baris sudah ditandai. Simpan liriknya.
musica-sinc-marcar = Tandai { $n }/{ $total }
musica-sinc-guardar = Simpan
musica-sinc-cancelar = Batal
musica-sinc-guardada = Lirik disinkronkan
musica-sinc-teclas = Spasi menandai · Backspace mengurungkan · Enter menyimpan · Esc membatalkan
musica-lrc-copiada = Lirik disalin sebagai LRC
musica-sola-empieza = Menyinkronkan lirik otomatis: mendengarkan lagu…
musica-sola-avance = Menyinkronkan lirik… { $pct } %
musica-sola-bajando = Mengunduh pengenal ucapan… { $pct } %
musica-sola-parada = Sinkronisasi dihentikan
musica-sola-ocupada = Lagu lain sedang disinkronkan
musica-sola-sin-reconocedor = Komputer ini tidak bisa mendengarkan lagu: ONNX Runtime tidak ada
musica-sola-falta-modelo = Untuk mengatur waktu otomatis, tekan «Sinkron otomatis» (subtitel): pengenal ucapan diunduh sekali (sekitar 160 MB)
musica-sola-no-se-oye = Lirik tidak dapat dikenali di lagu. Sinkronkan dengan mengetuk memakai jam
musica-sola-fallo = Lirik tidak dapat disinkronkan
musica-sola-cambio = Lirik berubah saat disinkronkan: tekan «Sinkron otomatis» lagi
musica-sola-hecha = Lirik disinkronkan otomatis ({ $n } dari { $total } baris terdengar). Jika ada yang meleset, perbaiki dengan jam
musica-sola-hecha-poca = Lirik disinkronkan secara kasar: hanya { $n } dari { $total } baris terdengar. Perbaiki dengan jam

chat-recordar-hora = Pilih waktu…
chat-recordar-hora-a-las = Ingatkan saya pukul
chat-recordar-hora-teclas = Ketik waktu · Enter mengatur · Esc keluar
chat-recordar-hora-queda = Enter: { $cuando }
chat-recordar-hora-mal = Itu bukan waktu: ketik seperti 18:30
chat-voz-sin-motor = Pengenal ucapan tidak ada: salin { $motor } ke { $donde }
chat-voz-sin-modelo = Model ucapan { $modelo } tidak ada: unduh dari { $enlace } dan ekstrak di dalam { $donde }
chat-voz-idioma-sin-modelo = Vosk tidak punya model ucapan untuk bahasa ini
chat-voz-ruta-imposible = Folder model ({ $donde }) memiliki karakter yang tidak bisa dibuka pengenal; pindahkan ke jalur tanpa aksen
chat-voz-modelo-ilegible = Model di { $donde } tidak lengkap atau tidak dapat dimuat
chat-voz-no-es-audio = Audio catatan ini tidak dapat dibaca
chat-voz-audio-vacio = Audionya kosong: tidak ada yang bisa diubah
chat-voz-no-se-entiende = Tidak ada yang dipahami dari ucapan di catatan
chat-voz-sin-reconocedor = Windows tidak punya pengenal ucapan untuk bahasa ini: tambahkan di Pengaturan › Waktu & bahasa › Ucapan.
chat-voz-bajar-whisper = Mengubah menjadi teks memerlukan model ucapan Whisper (sekitar { $megas } MB). Sedang diunduh sekarang, hanya sekali ini, lalu catatan akan diubah.
chat-voz-bajando-whisper = Mengunduh model ucapan… { $pct }%
chat-voz-sin-modelo-whisper = Model ucapan Whisper (sekitar { $megas } MB) tidak ada: tekan «Ubah menjadi teks» lagi untuk mengunduhnya.
chat-voz-sin-modelo-qwen3 = Qwen3-ASR belum diunduh (sekitar { $megas } MB): pilih di Pengaturan › Suara untuk mengunduhnya, atau kembali ke Whisper.
chat-voz-sin-onnx = Windows ini tidak punya runtime AI (onnxruntime.dll): salin ke { $donde } atau di samping PinDesk.
chat-voz-descarga = Model ucapan tidak dapat diunduh ({ $que }). Periksa koneksi dan tekan «Ubah menjadi teks» lagi.
chat-voz-onnx = Mesin ucapan Whisper gagal: { $detalle }
chat-dictado-oyendo = Mendengarkan… bicaralah, lalu klik mikrofon jika selesai
chat-dictado-fallo = Dikte gagal: { $razon }
chat-anotar-en = Catat di "{ $chat }": { $texto }
chat-anotar-donde = Ke obrolan mana "{ $texto }" dikirim? Pilih obrolan
chat-anotar-tecla = Enter
chat-anotado-en = Dicatat di "{ $chat }"
chat-dictado-preparando = Menyiapkan mikrofon…

# The teleprompter: the text scrolls by and you read it aloud while it
# records. There is no synthetic voice anywhere, same as on the phone.
telepronter-titulo = Baca keras-keras
telepronter-ensayar = Latihan
telepronter-parar = Berhenti
telepronter-grabar = Rekam
telepronter-terminar = Selesai
telepronter-tirar = Buang
telepronter-velocidad = kecepatan dan ukuran
telepronter-preparate = Bersiaplah
telepronter-muy-corta = Bacaan itu terlalu pendek
telepronter-sin-micro = Tidak ada mikrofon yang tersedia
chat-telepronter-borrador = Yang diketik di kotak
chat-telepronter-sin-texto = Tidak ada teks untuk dibaca: ketik sesuatu di kotak atau lampirkan .txt atau .md
chat-telepronter-abierto = Teleprompter sudah terbuka
chat-telepronter-hecha = Bacaan disimpan sebagai catatan suara

# The capture stack: the little pile in the corner (icon and panel).
pila-titulo-una = 1 tangkapan
pila-titulo-varias = { $cuantas } tangkapan ditumpuk
pila-copiar-elegidas = Salin yang dipilih
pila-copiar-todas = Salin semua
pila-quitar = Hapus
pila-copiadas = { $cuantas } tangkapan disalin: tempelkan dengan Ctrl+V

# The drawing editor's properties panel (Excalidraw's).
panel-trazo = Goresan
panel-fondo = Latar
panel-relleno = Isian
panel-grosor = Lebar goresan
panel-estilo-trazo = Gaya goresan
panel-presion = Tekanan
panel-trazo-a-mano = Kekasaran
panel-bordes = Tepi
panel-tipo-flecha = Jenis panah
panel-fuente = Jenis huruf
panel-tamano-fuente = Ukuran huruf
panel-alineacion-texto = Perataan teks
panel-puntas = Ujung panah
panel-opacidad = Opasitas
panel-capas = Lapisan
panel-alinear = Ratakan
panel-acciones = Tindakan
panel-colores = Warna
panel-tonos = Corak
panel-codigo-hex = Kode heks

## The canvas paper (Excalidraw viewBackgroundColor)
fondo-lienzo = Latar kanvas
fondo-papeles-del-movil = Kertas dari ponsel
fondo-lienzo-menu = Latar kanvas…

## Export and print the canvas (G1, G2, G3, F11)
exportar-menu = Ekspor…	Ctrl+Shift+E
exportar-copiar-png = Salin sebagai PNG	Ctrl+Shift+C
exportar-titulo = Ekspor kanvas
exportar-que = Apa
exportar-todo = Seluruh kanvas
exportar-seleccion = Pilihan
exportar-marcos = Setiap bingkai
exportar-escala = Skala
exportar-transparente = Latar transparan
exportar-tipo-png = Gambar PNG
exportar-tipo-svg = Gambar SVG
exportar-tipo-pdf = Dokumen PDF
exportar-tipo-html = Halaman web satu file
exportar-vacio = Tidak ada yang bisa diekspor dengan ini.
exportar-fallo = Kanvas tidak dapat diekspor.
imprimir-menu = Cetak…	Ctrl+P
imprimir-vacio = Tidak ada yang bisa dicetak.
imprimir-fallo = Kanvas tidak dapat dicetak.
exportar-chat = Ekspor…
imprimir-chat = Cetak…
# F11: imprimir con vista previa (el dialogo de Windows y el boton de la barra)
imprimir-pista = Cetak… (Ctrl+P)
imprimir-que = Apa yang dicetak
imprimir-que-marcos = Setiap bingkai di halamannya sendiri
imprimir-que-todo = Seluruh kanvas di satu halaman
imprimir-que-seleccion = Hanya pilihan
marca-elige = Tandai dengan apa? (1-9, 0) · Esc untuk batal
marca-plantar = Klik di kanvas tempat tanda diletakkan · Esc membatalkan
marca-vista-guardada = Tampilan disimpan bersama tanda · Shift+klik tanda untuk memperbarui
hojita-borrar = Bersihkan
hojita-insertar = Sisipkan
hojita-pegar = Sebagai pin

# The share sheet (compartir.rs): one for everything, like the phone's.
compartir-titulo = Bagikan
compartir-menu = Bagikan…	Ctrl+Shift+S
compartir-proyecto = Bagikan…
compartir-lector = Bagikan…  (Ctrl+Shift+S)
compartir-boton = Bagikan
compartir-guardar = Simpan sebagai…
compartir-copiar = Salin ke papan klip
compartir-wifi = Lewat Wi‑Fi ke perangkat saya
compartir-boton-copiar = Salin
compartir-boton-wifi = Wi-Fi
compartir-boton-enlace = Tautan
compartir-enlace-pista = Mengunggah ke litterbox (3 hari) dan menyalin tautannya: siapa pun yang punya tautan bisa membukanya
compartir-enlace-subiendo = Mengunggah…
compartir-enlace-copiado = Tautan disalin · dihapus dalam 3 hari
compartir-enlace-fallo = Tidak dapat mengunggah: { $motivo }
compartir-web = Halaman web (HTML)
compartir-adjuntos = Lampiran
compartir-pdf = PDF
compartir-png = Gambar PNG
compartir-jpg = Gambar JPG
compartir-svg = Gambar SVG
compartir-original = Asli
compartir-originales = File asli
compartir-editable = Dapat diedit
compartir-texto = Teks
compartir-csv = Tabel (CSV)
compartir-pindesk = PinDesk (.pindesk)
compartir-dxf = DXF
compartir-md = Markdown
compartir-exportar = Ekspor…
compartir-paginas-de = Yang disertakan · { $marcadas } dari { $total }
compartir-una-pagina = Halaman mana
compartir-va-entero = Dikirim utuh, apa adanya
compartir-todas = Semua
compartir-ninguna = Tidak ada
compartir-preparando = Menyiapkan…
compartir-marca-alguna = Centang setidaknya satu halaman
compartir-en-cuanto-este = Segera setelah siap
compartir-peso-ficheros = { $peso } · { $n } file
compartir-guardado = Disimpan: { $nombre }
compartir-copiado = Disalin ke papan klip
compartir-no-se-pudo = Tidak dapat disiapkan
compartir-nada = Tidak ada yang bisa dibagikan di sini.
compartir-coma-decimal = ,
compartir-lienzo = Kanvas
compartir-lienzo-entero = Seluruh kanvas
compartir-marco = Bingkai
compartir-pagina = Halaman
compartir-anotada = Dengan catatan
compartir-sin-nombre = Tanpa judul
compartir-tipo-lienzo = Kanvas
compartir-tipo-pagina = Halaman PDF
compartir-tipo-foto = Foto
compartir-tipo-tabla = Tabel
compartir-tipo-miniapp = Aplikasi mini
compartir-tipo-voz = Catatan suara
compartir-tipo-nota = Catatan

# Drawing tools: the ones shown in the canvas, the reader, pins and the
# screen annotator. Turning one off removes it from the bar and its key.
herramientas-lazo = Laso
herramientas-rectangulo = Persegi panjang
herramientas-rombo = Belah ketupat
herramientas-elipse = Elips
herramientas-flecha = Panah
herramientas-flecha-codos = Panah siku
herramientas-flecha-libre = Panah bebas
herramientas-linea = Garis
herramientas-lapiz = Pena
herramientas-grafito = Pensil
herramientas-texto = Teks
herramientas-borrador = Penghapus
herramientas-resaltador = Stabilo
herramientas-foco = Sorotan
herramientas-lupa = Kaca pembesar
herramientas-mosaico = Mosaik
herramientas-arco = Busur
herramientas-serie = Langkah bernomor
herramientas-punto = Titik berlabel
herramientas-cota = Dimensi
herramientas-escalar = Kalibrasi skala
herramientas-escala-grafica = Batang skala
herramientas-marco = Bingkai
herramientas-relleno = Ember isi
herramientas-recortar = Pangkas
herramientas-extender = Perpanjang
herramientas-copiar-estilo = Salin gaya

## D10: a PowerPoint in the chat (diapositivas.rs)
diapositivas-titulo = Presentasi
diapositivas-no-es = Ini bukan presentasi
diapositivas-keynote = File Keynote tidak bisa dibaca langsung: ekspor ke PowerPoint atau PDF
diapositivas-sin-powerpoint = Menampilkan PowerPoint memerlukan PowerPoint terpasang
diapositivas-abrir-con-otra = Buka dengan aplikasi lain?
diapositivas-preparando = Menyiapkan presentasi dengan PowerPoint…
diapositivas-no-se-pudo = Tidak dapat menyiapkan presentasi
diapositivas-no-esta = Presentasi sudah tidak ada di komputer ini
diapositivas-siguiente = Berikutnya
diapositivas-notas = Catatan
diapositivas-sin-notas = Slide ini tidak punya catatan
diapositivas-fin = Akhir presentasi
diapositivas-transiciones-si = Transisi aktif (T)
diapositivas-transiciones-no = Transisi nonaktif (T)

## Chat: a page from a project, the link card, «Received from» and the inbox
chat-sin-paginas = Tidak ada proyek yang punya halaman untuk dilampirkan
chat-recibido-de = Diterima dari { $de }
# Linked versions in a shared chat (27-oct-2026): touching someone else's
# message makes your version, which replies to theirs, and theirs stays as is.
chat-version-sola = Versi
chat-version-sola-n = Versi { $n }
chat-version-de = Versi dari pesan { $quien }
chat-version-de-n = Versi { $n } dari pesan { $quien }
chat-version-de-lo-tuyo = Versi dari pesan Anda
chat-version-de-lo-tuyo-n = Versi { $n } dari pesan Anda
chat-version-de-otro = Versi dari pesan lain
chat-version-de-otro-n = Versi { $n } dari pesan lain
chat-version-de-la-lista = Versi { $n } · { $quien }
chat-version-tuya = Versi { $n } · milik Anda
chat-versiones = { $cuantas ->
   *[other] { $cuantas } versi
  }
chat-version-es-de = Ini milik { $quien }: jika Anda mengubahnya, akan disimpan sebagai versi Anda dan milik mereka tidak disentuh
chat-version-guardada = Disimpan sebagai versi Anda; aslinya tidak disentuh
chat-version-no = Tidak dapat menyiapkan versi Anda
chat-fecha-corta = { $dia } { $mes }
chat-enlace-no-abre = Tautan tidak dapat dibuka

## D11: a spreadsheet in the chat is a book of tables (ventana_chat/libro.rs)
hoja-preparando = Menyiapkan lembar untuk diedit…
hoja-editar-aviso = Perubahan disimpan di lembar obrolan; file aslinya tetap sama
hoja-editando = Mengedit · Esc untuk melihat
hoja-rango-grande = Terlalu banyak sel untuk sekaligus
hoja-no-editable = Lembar ini tidak punya tabel: tidak bisa diedit
hoja-fondo-sin = Tanpa warna
hoja-letra-automatica = Otomatis
hoja-color-amarillo = Kuning
hoja-color-verde = Hijau
hoja-color-azul = Biru
hoja-color-rosa = Merah muda
hoja-color-morado = Ungu
hoja-color-naranja = Oranye
hoja-color-gris = Abu-abu
hoja-color-celeste = Biru muda
hoja-color-rojo = Merah
hoja-numero-general = Umum
hoja-numero-numero = Angka (1.234,50)
hoja-numero-moneda = Mata uang
hoja-numero-porcentaje = Persen (12,50 %)
hoja-numero-fecha = Tanggal

## D9: find in the document (buscador.rs, visor.rs, lector_pdf.rs)
buscar-pista = Cari di dokumen
buscar-cuenta = { $n } dari { $total }
buscar-nada = Tidak ada hasil
buscar-leyendo = Membaca teks…
buscar-sin-texto = PDF ini tidak punya teks (berupa gambar)
buscar-cifrado = Tidak bisa mencari di PDF terenkripsi

## D5: rename from the reader (renombrar_doc.rs)
renombrar-hecho = Sekarang bernama "{ $nombre }"
renombrar-ya-existe = Sudah ada file bernama "{ $nombre }"
renombrar-no-se-pudo = Tidak dapat mengganti nama

## Canvas: zone, laser, image, shapes, graph and present (F8, F12, F14, G5)
herramientas-zona = Zona (salinan membulat)
herramientas-laser = Penunjuk laser
herramientas-imagen = Gambar
herramientas-figuras = Bentuk
figuras-grafica = Grafik fungsi…
figuras-tabla-en-blanco = Tabel kosong
figuras-pegar-tabla = Tempel tabel (dari Excel)
figuras-recta = Garis bilangan
figuras-plano = Bidang Kartesius
figuras-espacio = Ruang xyz
figuras-coordenadas = Tabel koordinat…
figuras-coordenadas-serie = Koordinat: seri
figuras-coordenadas-nueva = Seri koordinat baru…
figuras-coordenadas-esconder = Sembunyikan seri
figuras-coordenadas-ver = Tampilkan seri
coordenadas-titulo = Tabel koordinat
coordenadas-ayuda = X dan Y dengan koma atau titik, Y naik ke atas · Tempel: dari Excel atau CSV · tanpa baris menghapus seri · Enter: terima · Esc: batal
figuras-guardar-seleccion = Simpan pilihan sebagai bentuk…
figuras-quitar = Hapus
figuras-mis-figuras = Bentuk saya
figuras-ventana-titulo = Bentuk
figuras-ventana-subtitulo = { $n } bentuk dan templat
figuras-ventana-buscar = Cari bentuk
figuras-ventana-todas = Semua
figuras-ventana-insertar-seccion = Sisipkan
figuras-ventana-insertar = Sisipkan
figuras-ventana-guardar-seleccion = Simpan pilihan
figuras-ventana-vacio = Tidak ada bentuk yang cocok
figuras-ventana-elige = Pilih bentuk untuk melihatnya dalam ukuran besar
figuras-ventana-piezas = { $n } bagian
figuras-ventana-intervalo = Rentang
figuras-ventana-desde = Dari
figuras-ventana-hasta = Sampai
figuras-ventana-de-a = Pisah setiap … (satu lembar masing-masing)
figuras-ventana-de-a-pista = Semua di satu lembar
figuras-ventana-semanas-desde = Dari minggu
figuras-ventana-semanas-hasta = Sampai minggu
figuras-ventana-mes = Bulan
figuras-ventana-meses = Berapa bulan
figuras-ventana-una-hoja = 1 lembar
figuras-ventana-hojas = { $n } lembar
figuras-ventana-renombrar = Ganti nama
figuras-ventana-quitar-seguro = Klik lagi untuk menghapusnya
figuras-nombre-titulo = Simpan sebagai bentuk
figuras-nombre = Nama
figuras-nombre-ayuda = Enter: simpan · Esc: batal
figuras-sin-tabla = Papan klip tidak berisi tabel
grafica-titulo = Grafik fungsi
grafica-formulas = y = (satu kurva per baris)
grafica-x-desde = x dari
grafica-x-hasta = x sampai
grafica-y-desde = y dari
grafica-y-hasta = y sampai
grafica-escala = Piksel per satuan
grafica-ayuda = Enter: sisipkan · Shift+Enter: kurva lain · Tab: kolom berikutnya · Esc: batal · per bagian: x^2 si x<0; 2x si x>=0
grafica-limites-mal = Batasnya tidak masuk akal: "dari" harus lebih kecil daripada "sampai"
grafica-sin-formula = Ketik rumus
grafica-formula-mal = Tidak dapat dipahami:
grafica-solo-x = Hanya boleh memakai x:
presentar-ayuda = ← → PgDn: berikutnya · B: hitam · J: laser · L: pena · Esc: keluar
vista-mirando = Hanya lihat · pensil di kanan untuk mengedit · F5: presentasi

## --- Voice: two languages, lyrics, teleprompter, pronounce, conversation, call (B6-B11) ---

ajustes-voz-segundo = Bahasa kedua catatan suara
ajustes-voz-modo = Dengan dua bahasa
ajustes-voz-modo-cada-uno = Setiap bagian sesuai cara diucapkan
ajustes-voz-modo-todo-en-uno = Semua dalam bahasa pertama
ajustes-voz-ninguno = Tidak ada
ajustes-voz-ingles = Inggris
ajustes-voz-espanol = Spanyol
ajustes-voz-portugues = Portugis
chat-letra-editar = Edit lirik
chat-letra-editando = Lirik sedang diedit
chat-letra-guardar = Simpan
chat-letra-guardada = Lirik disimpan
chat-cancelar-caja = Batal
chat-llamada-secreta = Panggilan rahasia
telepronter-pausa = Jeda
telepronter-seguir = Lanjutkan
telepronter-velocidad-muy-lenta = sangat lambat
telepronter-velocidad-lenta = lambat
telepronter-velocidad-normal = normal
telepronter-velocidad-rapida = cepat
telepronter-velocidad-muy-rapida = sangat cepat
pronunciar-titulo = Pelafalan
pronunciar-manten = Tahan spasi (atau mikrofon) dan bicaralah
pronunciar-suelta = Lepaskan untuk mendengar diri Anda
pronunciar-otra-vez = Tahan lagi untuk mengulang
pronunciar-guia = Panduan
pronunciar-guia-texto = Tulis teks
pronunciar-guia-fichero = Gambar, PDF, atau catatan dari komputer ini
pronunciar-sin-guia = Tanpa panduan: tulis satu, atau bawa gambar, PDF, atau catatan
pronunciar-repetir = Dengar lagi (R)
pronunciar-guardar = Simpan ke obrolan (Ctrl+G)
pronunciar-ya-guardada = Tersimpan
pronunciar-guardado = Tersimpan; sedang ditranskripsi agar Anda bisa melihat apa yang dipahami
pronunciar-idioma = Bahasa yang Anda latih
pronunciar-idioma-ajustes = Yang biasa
pronunciar-oir-texto = Dengarkan teks
pronunciar-sin-voz = Windows tidak punya suara untuk bahasa itu
pronunciar-no-se-pudo = Itu tidak dapat dibuka
pronunciar-toma = Rekaman terakhir: { $duracion }
pronunciar-pagina = Halaman { $pagina } dari { $paginas }
pronunciar-abierto = Pelafalan sudah terbuka
conversacion-titulo = Percakapan
conversacion-explicacion = Merekam tanpa henti; setiap orang menekan nomornya (1 sampai 9) saat mulai bicara, dan 0 saat tidak ada yang bicara. Transkripnya keluar dengan nama-nama.
conversacion-cuantos = Berapa orang yang berbicara?
conversacion-persona = Orang { $n }
conversacion-empezar = Mulai merekam
conversacion-terminar = Selesai dan transkripsikan
conversacion-turnos = { $n } giliran
conversacion-hablando = Berbicara…
conversacion-teclas = Tekan nomor orang yang berbicara · 0: tidak ada
conversacion-transcribiendo = Mentranskripsikan percakapan…
conversacion-abierta = Percakapan sudah terbuka
llamada-titulo = Panggilan
llamada-entrante = Panggilan masuk
llamada-contestar = Jawab
llamada-colgar = Tutup telepon
llamada-altavoz = Pengeras suara
llamada-perdida = Panggilan tak terjawab

## PDF zone (E9): merge PDF pages into one canvas, as on the phone
fusionar-paginas = Gabungkan ke satu kanvas
fusionar-paginas-empieza = Menyatukan halaman di kanvas…
fusionar-paginas-hecho = Selesai: “{ $nombre }” sekarang ada di proyek
fusionar-paginas-no = Halaman-halaman itu tidak dapat digabungkan

## Universe zone (batch 2: H2, H3, M1): the Cosmos theme outside the universe
ajustes-tema-cosmos = Tema Kosmos (langit berbintang di obrolan)

## Zona pines (tanda 2: C3, C4, L3): magic word, board, sheet and tools
pin-convertir-en = Ubah menjadi
pin-herramienta-temporizador = Timer
pin-herramienta-cronometro = Stopwatch
pin-herramienta-tareas = Daftar tugas
pin-herramienta-contador = Penghitung
pin-herramienta-gastos = Pengeluaran
pin-herramienta-pizarra = Papan
pin-herramienta-ruleta = Roda putar
pin-herramienta-lienzo = Kanvas
pin-herramienta-hoja = Lembar
pin-herramienta-tabla = Tabel
pin-fondo-pizarra = Latar papan
pin-pizarra-blanca = Putih
pin-pizarra-negra = Hitam
pin-pizarra-azul = Biru batu tulis
pin-pizarra-verde = Hijau papan tulis
pin-pauta-lisa = Polos
pin-pauta-cuadros = Kisi
pin-pauta-rayas = Garis
pin-pauta-columnas = Kolom
pin-pauta-puntos = Titik
pin-tiempo-cumplido = Waktu habis
pin-hoja-nombre = Lembar

## Window groups (H9) and Markdown note editor (H12)
bandeja-grupos-ventanas = Grup jendela…
chat-adj-nota-md = Catatan
chat-editar-nota = Buka di editor catatan
grupos-nada-abierto = Tidak ada kanvas, dokumen, atau catatan terbuka untuk disimpan
grupos-guardar = { $n ->
   *[other] Simpan { $n } jendela terbuka sebagai grup…
}
grupos-abrir = Buka “{ $nombre }” · { $n }
grupos-borrar = Hapus “{ $nombre }”
grupos-alguna-falta = Ada jendela di grup yang tidak bisa dibuka lagi
grupos-sin-nombre = Grup
grupos-nombre = Nama grup jendela
grupos-guardar-boton = Simpan
nota-md-nueva = Catatan baru
nota-md-sufijo = Catatan PinDesk
nota-md-negrita = Tebal
nota-md-cursiva = Miring
nota-md-tachado = Coret
nota-md-codigo = Kode
nota-md-enlace = Tautan
nota-md-titulo1 = Judul
nota-md-titulo2 = Subjudul
nota-md-titulo3 = Bagian
nota-md-lista = Daftar
nota-md-numerada = Daftar bernomor
nota-md-casilla = Daftar tugas
nota-md-marcar = Centang atau hapus centang tugas
nota-md-cita = Kutipan
nota-md-bloque = Blok kode
nota-md-raya = Pemisah
nota-md-formula = Rumus
nota-md-cortar = Potong
nota-md-copiar = Salin
nota-md-pegar = Tempel
nota-md-guardar = Simpan
nota-md-copia = Simpan salinan .md…
nota-md-tipo = Catatan Markdown
nota-md-no-guardada = Catatan tidak dapat disimpan. Tetap tutup dan kehilangan perubahan?
nota-md-mayus = Shift
nota-md-intro = Enter

# H12, the new note editor window (header, toolbar, menus and tables)
nota-md-compartir = Bagikan
nota-md-tabla = Tabel
nota-md-imagen = Gambar
nota-md-fecha = Tanggal hari ini
nota-md-pista-barra = Ketik / di catatan untuk opsi lain
nota-md-cambiar-titulo = Ganti nama
nota-md-fila-encima = Sisipkan baris di atas
nota-md-fila-debajo = Sisipkan baris di bawah
nota-md-quitar-fila = Hapus baris
nota-md-columna-izquierda = Sisipkan kolom di kiri
nota-md-columna-derecha = Sisipkan kolom di kanan
nota-md-quitar-columna = Hapus kolom
nota-md-quitar-tabla = Hapus tabel
nota-md-boton-fila-mas = + Baris
nota-md-boton-fila-menos = − Baris
nota-md-boton-columna-mas = + Kolom
nota-md-boton-columna-menos = − Kolom
nota-md-meses = Jan Feb Mar Apr Mei Jun Jul Agu Sep Okt Nov Des
## Note tables: merge and colour cells (H12)
nota-md-boton-combinar = Gabungkan
nota-md-boton-color = Warna
nota-md-combinar-celdas = Gabungkan sel
nota-md-separar-celdas = Pisahkan sel
nota-md-color-fondo = Warna latar
nota-md-color-letra = Warna teks
nota-md-aplicar-a = Sel|Seluruh baris|Seluruh kolom
nota-md-nombres-fondo = Tanpa warna|Merah|Hijau|Biru|Kuning
nota-md-nombres-letra = Otomatis|Merah|Hijau|Biru|Oranye
nota-md-pista-color = Seluruh baris dan kolom: klik kanan
nota-md-imagen-no-copiada = Gambar tidak dapat disalin ke samping catatan.

## Note comments (H12, Sep 30): the panel on the right
nota-md-com-titulo = Komentar
nota-md-com-comentar = Komentari
nota-md-com-ninguno = Tidak ada komentar
nota-md-com-pista = Pilih teks lalu tekan Komentari (Ctrl+Alt+M)
nota-md-com-sin-ancla = Teksnya sudah tidak ada di catatan
nota-md-com-responder-pista = Balas…
nota-md-com-responder = Balas
nota-md-com-guardar = Simpan
nota-md-com-cancelar = Batal
nota-md-com-editar = Edit
nota-md-com-borrar = Hapus
nota-md-com-resuelto = selesai
nota-md-com-ver-resueltos = Tampilkan yang selesai
nota-md-com-ocultar-resueltos = Sembunyikan yang selesai
nota-md-com-editado = diedit
nota-md-com-borrar-hilo = Hapus komentar beserta balasannya?

## Markdown notes: images and live pages (H12)
nota-md-pagina-viva = Halaman dari proyek
nota-md-enlace-hoja = Tautan ke lembar
nota-md-captura = Tangkapan layar
nota-md-ver-grande = Lihat ukuran penuh
nota-md-abrir-hoja = Buka lembar
nota-md-foto-pequena = Kecil
nota-md-foto-mediana = Sedang
nota-md-foto-grande = Besar
nota-md-foto-columna = Selebar kolom
nota-md-quitar-foto = Hapus gambar
nota-md-hoja-borrada = Lembarnya sudah hilang: ini salinan terakhir
nota-md-otros-proyectos = Proyek lain
nota-md-sin-hojas = Belum ada lembar proyek untuk ditambahkan ke catatan.
nota-md-solo-imagenes = Hanya gambar yang bisa dilepas di sini.
chat-insertar-en-nota = Sisipkan ke catatan

## Canvas: weld vertices and number points (lienzo-geometria, 26-sep)
herramientas-nudo = Las titik sudut
herramientas-bolita = Bola: sapukan untuk memilih
lienzo-punto-serie = Label titik
lienzo-cota-pedir = Tanyakan panjang saat digambar
lienzo-panel-dureza = Kekerasan pensil
lienzo-guia-corto = Panduan
lienzo-panel-guias = Garis panduan
lienzo-cota-titulo = Berapa panjang dan ke arah mana? (Tab berpindah kolom)
lienzo-cota-largo = Panjang
lienzo-cota-angulo = Sudut

## Canvas: image (zone, magnifier, pixelate, share and reference; Sep 26)
lienzo-referencia-menu = Gambar referensi…
lienzo-pegar-menu = Tempel
lienzo-pista-compartir = Bagikan… (Ctrl+Shift+S)
lienzo-pista-lupa = Kaca pembesar (Q): di kanvas, ketuk bentuk tertutup untuk menampilkan yang ada di bawahnya, diperbesar; di layar, memperbesar area di bawah kursor
lienzo-pista-mosaico = Pikselasi (mosaik): seret di atas yang ingin Anda sembunyikan
lienzo-pista-zona = Zona (Z): seret kotak untuk mendapatkan salinan terpotong yang bisa dipindahkan
lienzo-panel-mosaico = Penutup
lienzo-panel-aumento = Perbesaran
lienzo-panel-guia = Penunjuk
# The spotlight (Propiedad.OSCURECER and Propiedad.ZONA on the phone)
lienzo-panel-oscurecer = Gelapkan
lienzo-panel-zona-foco = Area terang
lienzo-pista-foco = Sorotan (F): ketuk bentuk tertutup dan area di sekitarnya menjadi gelap, di dalam bingkainya
lienzo-pista-serie = Langkah bernomor: setiap klik menaruh nomor berikutnya

## The Chat / Functions switch in the chat title bar
proyectos-interruptor-chat = Obrolan
proyectos-interruptor-funciones = Fungsi
## The Functions screen: its tiles, in order
funciones-titulo = Fungsi
funciones-grabar = Rekam rapat
funciones-grabar-detalle = Kelas atau rapat, dengan tangkapan layar dan transkrip
funciones-lienzo = Kanvas baru
funciones-lienzo-detalle = Kanvas kosong untuk menggambar atau menjiplak
funciones-tareas = Tugas
funciones-tareas-detalle = Daftar tugas dari semua obrolan Anda
funciones-timeline = Lini masa
funciones-timeline-detalle = Hari Anda menit demi menit, dan pelajaran Anda
funciones-horario = Jadwal
funciones-horario-detalle = Kelas Anda minggu ini, dengan pengingat sebelumnya
funciones-galeria = Tangkapan layar
funciones-galeria-detalle = Semua tangkapan layar yang Anda simpan
funciones-figuras = Bentuk
funciones-figuras-detalle = Pustaka bentuk Anda untuk kanvas
figuras-mias = Bentuk saya
figuras-todas = Semua
figuras-vacia = Belum ada bentuk. Di kanvas, pilih sesuatu lalu “Simpan sebagai bentuk”.
figuras-borrar = Hapus bentuk
figuras-borrar-seguro = Tekan lagi untuk menghapus
figuras-elegir-para = Pilih bentuk untuk “{ $chat }”
figuras-adjuntar-a = Lampirkan ke “{ $chat }”
figuras-adjuntar-pista = Atau klik dua kali bentuknya, atau tekan Enter.
figuras-adjuntada = Dilampirkan ke “{ $chat }” ✓
asistente-adj-figura = Bentuk
asistente-adj-pedir-figura = Minta bentuk baru
asistente-pide-figura = Buatkan saya bentuk
asistente-mejora-figura = Perbaiki bentuk “{ $figura }”:
chat-guardar-en-figuras = Simpan ke bentuk saya
chat-figura-sin-nombre = Bentuk dari { $chat }
chat-figura-guardada = Disimpan ke bentuk Anda sebagai “{ $nombre }”
chat-figura-vacia = Kanvas itu kosong: tidak ada yang bisa disimpan
figuras-titulo = Bentuk
figuras-buscar = Cari bentuk…
# The ☀/☾ switch of the shape panels: day templates or their «(night)» twins.
figuras-sin-resultados = Tidak ada bentuk dengan itu di nama atau teksnya.
figuras-de-serie = bawaan
figuras-tuya = milik Anda
figuras-mide = Ukuran
figuras-piezas = Bagian
figuras-textos = Teks yang dimiliki
figuras-lineas = { $n } garis
figuras-formas = { $n } bentuk
figuras-de-texto = { $n } teks
figuras-trazos = { $n } goresan tangan
figuras-otras = { $n } lainnya
bandeja-figuras = Bentuk…

## --- Canvas: graph and table dialogs, put into a cell (F12/F14) ---
cajetin-cancelar = Batal
grafica-insertar = Sisipkan
figuras-guardar = Simpan
figuras-editar-tabla = Edit tabel… (Enter)
figuras-meter-en-celda = Masukkan pilihan ke selnya
tabla-blanco-titulo = Tabel kosong
tabla-pegada-titulo = Tabel yang ditempel
tabla-editar-titulo = Edit tabel
tabla-insertar = Sisipkan
tabla-aplicar = Terapkan
tabla-mas-fila = + Baris
tabla-menos-fila = − Baris
tabla-mas-columna = + Kolom
tabla-menos-columna = − Kolom
tabla-cabecera = Tajuk
tabla-pegar = Tempel
tabla-ayuda = Tab dan panah: sel lain · − menghapus baris atau kolom sel aktif · Enter: terima · Esc: batal

## --- Canvas: the Zone to the project chat (F8, mandarLaZona on the phone) ---
zona-arrastra-copia = Seret: muncul salinan
zona-arrastra-chat = Seret: dikirim ke obrolan proyek
zona-al-chat = Ke obrolan
zona-mandada = Zona dikirim ke obrolan «{ $proyecto }»
zona-no-mandada = Zona tidak dapat dikirim
zona-viene-pdf = PDF «{ $proyecto }» → halaman { $pagina }
zona-viene-lienzo = Kanvas «{ $nombre }»
zona-nombre-pagina = Zona halaman { $pagina }
zona-nombre-lienzo = Zona { $nombre }
zona-lienzo = kanvas
zona-hoja-lienzo = Kanvas
# Linked zone that leads to no canvas (salto_por_enlace).
zona-enlace-roto = Zona ini tidak mengarah ke kanvas mana pun di proyek ini

## The canvas toolbar grouped like the phone's (GRUPOS_DE_FABRICA)
herramientas-mano = Pilih dan pindahkan
herramientas-imprimir = Cetak
herramientas-compartir = Bagikan
barra-grupo-elegir = Cara lain memilih
barra-grupo-trazar = Tangan bebas
barra-grupo-formas = Bentuk
barra-grupo-flechas = Panah dan garis
barra-grupo-arreglar = Perbaiki gambar
barra-grupo-nombrar = Teks dan label
barra-grupo-tapar = Tutupi dan tunjuk
barra-grupo-medir = Ukur
barra-grupo-marco = Bingkai dan gambar
barra-grupo-sacar = Bagikan dan cetak
barra-pista-mano = Pilih dan pindahkan (M)
barra-pista-deshacer = Urungkan (Ctrl+Z)
barra-pista-rehacer = Ulangi (Ctrl+Y)
barra-pista-salir = Keluar (Esc)
ajustes-herramientas-sueltas = Selalu terlihat
ajustes-herramientas-donde = Alat di
ajustes-herramientas-donde-ayuda = Pilih tempat untuk menonaktifkan alat yang tidak Anda pakai di sana, tanpa menghapusnya di tempat lain.
ajustes-herramientas-sitio-todos = Di mana saja
ajustes-herramientas-sitio-pantalla = Layar
ajustes-herramientas-sitio-pin = Pin
ajustes-herramientas-sitio-lienzo = Kanvas
ajustes-herramientas-sitio-lector = Pembaca

## PDF reader: «To project» (the notes move to a project's sheets and travel when syncing)
lector-al-proyecto = Ke proyek
lector-al-proyecto-hecho = Sekarang sudah ada di proyek, dengan catatan Anda
lector-al-proyecto-fallo = Proyek tidak dapat dibuat

## «Save» in the viewers of files opened from outside (passing mode, Android 15a52aed)
guardar-de-paso = Simpan ke obrolan
guardar-de-paso-guardados = Ke «Pesan tersimpan»
guardar-de-paso-en = Ke «{ $chat }»
guardar-de-paso-hecho = Disimpan ke «{ $chat }», dengan catatan Anda
guardar-de-paso-fallo = Tidak dapat disimpan ke obrolan

## Word and book reader: the four types and four weights of the phone (K16)
lector-letra-serif = Serif
lector-letra-sans = Sans
lector-letra-cursiva = Kursif
lector-grosor-fina = Tipis
lector-grosor-negra = Hitam

## Screen annotator: its pill (CapaPantalla.kt on the phone) and saving to «Saved messages»
anotador-rotulo-dibujando = Alt + klik tengah ganda: klik menembus
anotador-rotulo-atravesando = Alt + klik tengah ganda: menggambar lagi
anotador-globo-titulo = Layar beranotasi
anotador-globo-guardado = Disimpan ke Pesan tersimpan
anotador-globo-fallo = Tidak dapat menyimpan ke Pesan tersimpan
anotador-globo-copiado = Disalin ke papan klip
# Screen annotator: click-through in the toolbar (2026-09-29)
anotador-boton-atravesar = Klik menembus
anotador-pista-atravesar = Klik menembus (Spasi): mouse mengenai yang ada di bawah dan gambar tetap terlihat. Kembali dengan tangan di pil atau dengan Alt + klik tengah ganda

# Share sheet: a document PDF with its annotations or clean (2026-09-29)
compartir-con-anotaciones = Dengan anotasi
compartir-con-anotaciones-detalle = Tinta, margin, dan penanda di atas PDF; jika nonaktif, PDF bersih

# Share sheet: a PDF web page as lines or as images (2026-09-30, Android v0.98.1)
compartir-texto-buscable = Teks yang bisa dicari
compartir-texto-buscable-detalle = Halaman sebagai baris dengan teksnya; jika nonaktif, sebagai gambar

# Secret call: who is calling and call back later (2026-09-30, Android v0.98.6)
chat-quien-llama = Yang menelepon: { $nombre }
chat-quien-llama-barra = Yang menelepon:
chat-quien-llama-teclas = Enter: pilih waktu · Esc membiarkannya
llamada-volver-en = Telepon balik dalam
llamada-vuelve-a-las = Akan menelepon Anda kembali pukul { $hora }

## H12: editing without marks, the floating toolbar and the note font (Sep 30)
nota-md-texto-normal = Teks normal
nota-md-quitar-formato = Hapus format
nota-md-enlace-pista = Tempel atau ketik tautan lalu tekan Enter
nota-md-emojis = Senyum|Tertawa|Suka sekali|Oke|Suka|Tidak suka|Terima kasih|Tepuk tangan|Pesta|Selesai|Tidak|Peringatan|Hati|Api|Bintang|Ide
nota-md-letra-texto = Huruf teks
nota-md-letra-titulos = Huruf judul
nota-md-tamano = Ukuran huruf
nota-md-tamanos = Kecil|Normal|Besar|Sangat besar
nota-md-solo-esta-nota = Hanya di catatan ini
nota-md-pista-tamano = Ctrl + gulir mengubahnya satu langkah demi satu langkah

# --- Notes: embedded documents, chat messages and audio (H12, Oct 1) ---
nota-md-documento = Dokumen
nota-md-del-chat = Dari obrolan
nota-md-audio = Audio
nota-md-abrir = Buka
nota-md-quitar = Hapus dari catatan
nota-md-ir-al-mensaje = Buka pesan di obrolan
nota-md-mensaje-borrado = Pesan dihapus
nota-md-no-esta = Tidak ada di komputer ini
nota-md-pasar-a-texto = Transkripsikan
nota-md-pasando-a-texto = Mentranskripsikan { $pct } %
nota-md-sin-mensajes = Obrolan proyek ini tidak punya pesan untuk disisipkan (catatan mandiri tidak punya obrolan).
nota-md-nota-de-voz = Catatan suara

## Export a note to Word (H12, 1-oct)
nota-md-exportar-word = Ekspor ke Word…
nota-md-tipo-word = Dokumen Word
compartir-word = Word

## Requests from other programs (Flow Launcher, scripts; docs/protocolo-pedidos.md)
pedido-tareas = Tugas
pedido-escrito = Ditulis di “{ $proyecto }”.
pedido-al-lienzo = Ditumpuk di kanvas “{ $lienzo }”.
microfono-abriendo = Membuka mikrofon…
microfono-guardando = Menyimpan…
microfono-parar = Klik untuk berhenti · Esc membuangnya
microfono-guardada = Catatan suara disimpan
microfono-convertir = Ubah menjadi panggilan
microfono-listo = Selesai
microfono-cuando = Jam berapa saya harus menelepon Anda?
microfono-en-15-min = Dalam 15 mnt
microfono-en-1-hora = Dalam 1 jam
microfono-manana = Besok 9:00
microfono-poner = Atur panggilan
microfono-volver = Kembali
microfono-sonara = Akan menelepon Anda: { $cuando }
microfono-puesta = Selesai: akan menelepon Anda { $cuando }
microfono-teclas = Ketik waktu (18:30) atau gunakan panah · Enter mengatur
soltar-aqui = Lepaskan di sini untuk menambahkan ke “{ $proyecto }”
soltar-anadiendo = Menambahkan…
soltar-anadido-uno = 1 ditambahkan ke obrolan
soltar-anadidos = { $n } ditambahkan ke obrolan
soltar-ninguno = Tidak ada yang bisa ditambahkan
chat-anadir-arrastrando = Tambah dengan menyeret…
pedido-lista-hecha = Daftar “{ $lista }” dibuat di “{ $proyecto }”.
pedido-tabla-hecha = Tabel “{ $tabla }” dibuat di “{ $proyecto }”.
pedido-tarea-anadida = “{ $tarea }” ditambahkan ke “{ $lista }”.
pedido-tarea-hecha = “{ $tarea }” ditandai selesai.
pedido-tarea-pendiente = “{ $tarea }” tertunda lagi.
pedido-roto = Permintaan dari program lain kurang lengkap; tidak ada yang dilakukan.
pedido-sin-proyecto = Proyek itu sudah tidak ada di PinDesk.
pedido-sin-mensaje = Pesan itu sudah tidak ada di obrolan.
pedido-no-es-lista = Pesan itu bukan daftar tugas.
pedido-sin-tarea = Daftar tidak punya tugas nomor { $indice }.
pedido-tarea-vacia = Tugas tanpa teks tidak ditambahkan.
pedido-sin-fichero = File itu sudah tidak ada di disk.
pedido-fallo-disco = Yang diminta tidak dapat disimpan. Periksa log PinDesk.
pedido-tarea-movida = “{ $tarea }” dipindahkan ke “{ $lista }”.
pedido-tarea-cambio = Daftar berubah saat tugas dipindahkan; tidak ada yang disentuh. Cari lagi.
pedido-tarea-quitada = “{ $tarea }” dihapus dari daftarnya.
pedido-tarea-corregida = Tugas diubah: “{ $tarea }”.
pedido-lista-borrada = Daftar “{ $lista }” dihapus dari “{ $proyecto }”.
pedido-timeline-apuntado = Ditambahkan ke lini masa: “{ $titulo }”.
pedido-sin-capturas = Belum ada tangkapan.
pedido-fuera-de-capturas = File itu tidak ada di folder tangkapan; dibiarkan saja.
pedido-no-es-imagen = File itu bukan gambar.
pedido-imagen-no-se-pudo = Gambar tidak dapat disalin: { $motivo }
pedido-sin-imagen = Gambar { $ruta } tidak ditemukan; tugas tidak ditambahkan.
pedido-nada-que-leer = Tidak ada teks yang diterima untuk dibacakan.
pedido-ocr-copiado = Teks disalin ({ $n } karakter): “{ $texto }”
pedido-ocr-sin-texto = Tidak ditemukan teks di gambar.
pedido-ocr-no-disponible = OCR Windows tidak terpasang: tambahkan bahasa dengan pengenalan teks di Pengaturan.
pedido-ocr-fallo = Tidak dapat membaca teks di gambar: { $motivo }

## Command line and pindesk:// links (Oct 15)
orden-titulo = PinDesk
orden-confirmar-cabecera = Sebuah tautan meminta PinDesk untuk:
orden-confirmar-pregunta = Lakukan?
orden-motivo-ficheros = Membuka atau mengirim file dari disk Anda.
orden-motivo-asistente = Bertanya ke asisten, yang bisa melakukan sesuatu di PinDesk.
orden-motivo-microfono = Merekam dengan mikrofon.
orden-motivo-otros-aparatos = Mengirim data ke perangkat Anda yang lain.
orden-motivo-automatizacion = Menyentuh otomatisasi asisten.
orden-motivo-borra = Menghapus sesuatu.
orden-motivo-cambia = Mengubah data PinDesk.
orden-prohibido-ruta-no-local = Tautan tidak bisa membuka file jaringan atau jalur di luar komputer ini. Tidak ada yang dilakukan.
orden-prohibido-crear-automatizacion = Tautan tidak bisa membuat otomatisasi. Tidak ada yang dilakukan.
orden-no-se-pudo = PinDesk tidak dapat melakukannya: { $motivo }
orden-sin-app = PinDesk tidak terbuka dan tidak dapat dijalankan: { $motivo }
orden-sin-respuesta = PinDesk tidak menjawab. Jika macet, tutup lalu buka lagi.
orden-version = PinDesk ini lebih lama dari permintaannya: perbarui.
orden-preguntar-titulo = Tanya asisten tentang { $cuantos ->
    [one] { $nombre }
   *[other] { $cuantos } file
}
orden-preguntar-enviar = Tanya
orden-preguntar-cancelar = Batal

## Capture gallery (2-oct)
galeria-titulo = Tangkapan
galeria-vacia = Belum ada tangkapan. Yang Anda salin dengan Alt + seret akan muncul di sini.
galeria-abrir-carpeta = Buka folder
galeria-copiada = Disalin ke papan klip
galeria-pineada = Di-pin
galeria-borrada = Dipindahkan ke tempat sampah PinDesk
galeria-no-se-pudo = Tidak dapat dilakukan: { $motivo }
galeria-conservada-aviso = Dipertahankan: ada di Pesan tersimpan dan tidak akan dihapus
bandeja-galeria-capturas = Galeri tangkapan…
# Lessons learned (3-oct, as on the phone)
chat-hacer-leccion = Buat pelajaran
bandeja-leccion-nueva = Pelajaran baru…
bandeja-tareas = ☑ Tugas…
bandeja-timeline = 🕒 Lini masa dan pelajaran…

## Tareas (ventana)
tareas-titulo = Tugas
tareas-apuntar-boton = Tambah
tareas-hechas = Selesai ({ $n })
tareas-vacia = Belum ada tugas. Ketik di atas dan tekan Enter: tugas masuk ke Kotak masuk, dan dari sana Anda memindahkannya ke grupnya.
tareas-cambio = Daftar berubah sementara itu; sekarang sudah terbaru. Centang lagi.
tareas-mover = Pindahkan ke…
tareas-mover-titulo = Pindahkan “{ $tarea }” ke…
tareas-sin-grupos = Belum ada grup tugas lain. Buat daftar tugas di obrolan dan akan muncul di sini.
tareas-movida = Dipindahkan ke { $lista }
tareas-imagen-no-esta = Gambar itu belum ada di komputer ini: akan tiba bersama sinkronisasi.
tareas-pegar-no-imagen = Hanya gambar atau teks yang bisa ditempel ke tugas.
tareas-pegar-repetida = Gambar itu sudah ada sebagai { $ficha }.

# Opening a photo that Windows can only read with a Microsoft Store extension.
abrir-falta-extension = Tidak dapat membuka { $nombre }: Windows memerlukan "{ $extension }", gratis di Microsoft Store. Pasang lalu buka file itu lagi.

## v2-menus
v2menus-responder = Balas
v2menus-anotar-encima = Anotasi di atasnya
v2menus-mas = Lainnya
v2menus-elegir-varios = Pilih beberapa
v2menus-tecla-supr = Del
v2menus-tecla-clic = klik
v2menus-adj-buscar = Cari: tabel, stopwatch, PDF…
v2menus-adj-mas = LAINNYA
v2menus-adj-nada = Tidak ada yang cocok dengan pencarian Anda
v2menus-adj-lienzo = Kanvas
v2menus-adj-tareas = Daftar tugas
v2menus-adj-tabla = Tabel
v2menus-adj-conversacion = Percakapan
v2menus-adj-captura = Tangkapan layar
v2menus-adj-pagina = Halaman proyek
v2menus-adj-proyecto = Seluruh proyek
v2menus-adj-movil = Ambil dari ponsel
v2menus-adj-pegar-copiado = Tempel yang disalin
v2menus-envio-enviar = Kirim
v2menus-envio-anadir = Tambah
v2menus-envio-fotos = { $cuantos ->
   *[other] { $cuantos } foto
} · ke { $proyecto }
v2menus-envio-archivos = { $cuantos ->
   *[other] { $cuantos } file
} · ke { $proyecto }
## v2-ajustes
ajustes2-titulo = Pengaturan
ajustes2-buscar = Cari pengaturan…
ajustes2-nota = Disimpan saat Anda menutup dengan Selesai atau Esc.
ajustes2-nota-punto = = berbeda dari bawaan.
ajustes2-deshacer = Urungkan
ajustes2-listo = Selesai
ajustes2-restablecer = Atur ulang { $seccion }
ajustes2-resultados = Hasil untuk “{ $busqueda }”
ajustes2-resultados-sub = { $n ->
   *[other] { $n } pengaturan ditemukan di semua bagian.
  }
ajustes2-sin-resultados = Tidak ada pengaturan yang menyebut “{ $busqueda }”. Coba kata lain.
ajustes2-sec-general = Umum
ajustes2-sec-general-sub = Cara PinDesk dimulai dan apa yang dilakukannya dengan PDF di obrolan.
ajustes2-sec-captura = Tangkapan
ajustes2-sec-captura-sub = Cara tangkapan diambil dan apa yang terjadi setelahnya.
ajustes2-sec-lienzo = Kanvas dan alat
ajustes2-sec-lienzo-sub = Magnet, goresan tangan bebas, dan alat yang tampil di bilah alat.
ajustes2-sec-voz = Suara dan transkripsi
ajustes2-sec-voz-sub = Mengubah catatan suara dua bahasa menjadi teks.
ajustes2-sec-atajos = Pintasan
ajustes2-sec-atajos-sub = Klik kotak, lalu tekan tombolnya. Delete membuatnya tanpa pintasan; Esc membatalkan.
ajustes2-sec-sincro = Sinkronisasi
ajustes2-sec-sincro-sub = Cara komputer ini menemukan ponsel.
ajustes2-sec-apps = Aplikasi bawaan
ajustes2-sec-apps-sub = Buka gambar dan video dengan PinDesk dari File Explorer.
ajustes2-sec-apariencia = Tampilan
ajustes2-sec-apariencia-sub = Tampilan obrolan.
ajustes2-sec-avanzado = Lanjutan
ajustes2-sec-avanzado-sub = Performa, filter pena, dan file pengaturan.
ajustes2-g-arranque = Saat memulai
ajustes2-g-pdf = Dokumen PDF
# Text recognition (OCR) for the whole app: Windows or Indra (docs/ocr.md).
ajustes2-g-ocr = Pengenalan teks (OCR)
ajustes2-ocr-motor = Mesin OCR
ajustes2-ocr-windows = Windows
ajustes2-ocr-indra = Indra
ajustes2-ayuda-ocr = Dipakai oleh tangkapan teks, «Jiplak ini», pin, galeri, PDF pindaian, dan asisten. Windows sudah ada di sistem. Indra (Datalab) jauh lebih baik membaca foto, pindaian, halaman miring, dan huruf kecil, di PC Anda dan tanpa internet; memilihnya mengunduh { $megas } MB. Bobotnya berlisensi OpenRAIL-M dari Datalab: gratis untuk penggunaan pribadi, riset, dan perusahaan di bawah $20 juta; penggunaan komersial lain memerlukan lisensi mereka.
ajustes2-ayuda-ocr-indra-listo = Indra sudah diunduh dan membaca semua teks di aplikasi. Jika suatu saat gagal, OCR Windows yang dipakai.
ajustes2-ayuda-ocr-windows-con-indra = OCR Windows sedang membaca. Indra masih terunduh: pilih untuk memakainya, atau hapus di bawah.
ajustes2-ocr-bajando = Mengunduh Indra… { $por-ciento } %. Sementara itu OCR Windows yang membaca.
ajustes2-ocr-fallo = Indra tidak dapat diunduh: { $fallo }. Pilih lagi untuk mencoba ulang; sementara itu OCR Windows yang membaca.
ajustes2-ocr-licencia = Lisensi Indra
ajustes2-ayuda-ocr-licencia = Kode Indra berlisensi Apache 2.0; bobotnya, OpenRAIL-M modifikasi dari Datalab. PinDesk tidak menyertakannya: Anda mengunduhnya dari Hugging Face saat memilih Indra.
ajustes2-ocr-corregir = Koreksi dengan kamus
ajustes2-ayuda-ocr-corregir = Hanya dengan Indra dan pada halaman berbahasa Spanyol: memperbaiki aksen dan huruf dengan keyakinan rendah. Tidak pernah menyentuh angka, kode, atau nama.
ajustes2-ocr-vocabulario = Kosakata saya
ajustes2-ocr-vocabulario-ninguno = Tidak ada. Nama, singkatan, atau istilah yang harus diutamakan Indra (E030, Arequipa…).
ajustes2-ocr-vocabulario-marcador = istilah, istilah lain…
ajustes2-ocr-borrar = Hapus Indra
ajustes2-ayuda-ocr-borrar = Mengosongkan 26 MB disk; OCR Windows membaca lagi.
ajustes2-ocr-borrar-boton = Hapus
# Transcribe with Whisper or Qwen3-ASR (25-oct-2026, docs/voz-qwen3.md)
ajustes2-voz-motor = Transkripsikan dengan
ajustes2-voz-motor-whisper = Whisper base
ajustes2-voz-motor-qwen3 = Qwen3-ASR
ajustes2-ayuda-voz-motor = Untuk catatan suara dan rekaman kelas atau rapat. Whisper base sudah tersedia (160 MB, cepat). Qwen3-ASR jauh lebih memahami bahasa Spanyol, rapat, dan ruang kelas (kesalahan separuhnya atau kurang); memilihnya mengunduh { $megas } MB, perlu sekitar { $ram } GB memori saat mentranskripsi, dan memakan waktu sekitar empat kali lebih lama daripada Whisper. Lisensi { $licencia }.
ajustes2-ayuda-voz-motor-qwen3-listo = Qwen3-ASR sudah diunduh ({ $total } MB, lisensi { $licencia }) dan mentranskripsi catatan serta rekaman. Perlu sekitar { $ram } GB memori saat mentranskripsi; jika suatu saat gagal, Whisper yang dipakai.
ajustes2-ayuda-voz-motor-whisper-con-qwen3 = Whisper yang mentranskripsi. Qwen3-ASR sudah diunduh: pilih untuk memakainya, atau hapus di bawah.
ajustes2-ayuda-voz-motor-sin-memoria = Whisper yang mentranskripsi. Qwen3-ASR memerlukan PC dengan memori 4 GB atau lebih.
ajustes2-voz-motor-bajando = Mengunduh Qwen3-ASR… { $por-ciento } %. Sementara itu Whisper yang mentranskripsi.
ajustes2-voz-motor-fallo = Tidak dapat mengunduh Qwen3-ASR: { $fallo }. Pilih lagi untuk mencoba ulang; sementara itu Whisper yang mentranskripsi.
ajustes2-voz-qwen3-licencia = Kartu model Qwen3-ASR
ajustes2-ayuda-voz-qwen3-licencia = Qwen3-ASR-0.6B dari Qwen (Alibaba), berlisensi Apache-2.0: bebas dipakai, termasuk untuk kerja.
ajustes2-voz-qwen3-borrar = Hapus Qwen3-ASR
ajustes2-ayuda-voz-qwen3-borrar = Mengosongkan 988 MB disk; catatan kembali ditranskripsi dengan Whisper.
ajustes2-g-al-capturar = Saat menangkap
ajustes2-g-gif = Perekaman GIF
ajustes2-g-guardar = Penyimpanan dan pembersihan
ajustes2-g-pila = Tumpukan tangkapan
ajustes2-g-ignorados = Program yang diabaikan
ajustes2-g-regiones = Area tersimpan
ajustes2-g-iman = Magnet
ajustes2-g-trazo = Tangan bebas
ajustes2-g-voz = Catatan suara
ajustes2-g-atajos = Pintasan global
ajustes2-g-sincro = Di jaringan lokal
ajustes2-g-apps = Membuka gambar dan video
ajustes2-g-tema = Tema
ajustes2-g-rendimiento = Performa
ajustes2-g-lapiz = Filter pena (mode Alami)
ajustes2-g-fichero = File pengaturan
ajustes2-ayuda-arranque = Tetap di baki sistem tanpa membuka jendela apa pun. Tidak tersedia dalam mode portabel.
ajustes2-ayuda-idioma = Berlaku mulai saat dijalankan berikutnya.
ajustes2-g-ventanas = Jendela
ajustes2-escala = Skala antarmuka
ajustes2-ayuda-escala = Ukuran semua elemen di obrolan. Langsung berlaku; di obrolan juga dengan Ctrl + "+", "-" dan "0".
ajustes2-grupos-ventanas = Grup jendela
ajustes2-ayuda-grupos-ventanas = Simpan jendela yang terbuka sebagai grup dan buka lagi bersama-sama.
ajustes2-ayuda-pdf-aligerar = Setelah masuk ke obrolan, di latar belakang.
ajustes2-pdf-nivel = Seberapa kecil
ajustes2-ayuda-pdf-nivel = Saat masuk hanya Persis dan Sedang yang dipakai; Kecil dan Maks. untuk “Perkecil PDF” secara manual.
ajustes2-retardo = Tangkapan tertunda
ajustes2-ayuda-retardo = Hitung mundur untuk membuka menu atau tooltip sebelum menangkap.
ajustes2-color = Format warna
ajustes2-ayuda-color = Cara pengambil warna menyalin warna.
ajustes2-scroll = Tinggi gulir maksimum
ajustes2-ayuda-scroll = Batas untuk tangkapan panjang, agar halaman tanpa ujung tidak menghabiskan memori.
ajustes2-gif-ritmo = Bingkai per detik
ajustes2-ayuda-gif-ritmo = Lebih sedikit = file lebih ringan. 10 atau 15 cukup untuk tutorial.
ajustes2-gif-retardo = Jeda sebelum merekam
ajustes2-ayuda-gif-retardo = Detik antara menekan “Rekam” dan bingkai pertama.
ajustes2-carpeta = Folder tangkapan
ajustes2-abrir = Buka
ajustes2-caducidad = Hari sampai dihapus
ajustes2-ayuda-caducidad = Setelah itu masuk ke Recycle Bin Windows, kecuali yang dipertahankan.
ajustes2-apilar = Tumpuk di sudut
ajustes2-ayuda-apilar = Setiap tangkapan meninggalkan kotak di sudut; klik untuk mengumpulkan tangkapan berikutnya dan menempel semuanya dengan satu Ctrl+V.
ajustes2-esquina = Sudut kotak
ajustes2-ayuda-esquina = Di monitor tempat tangkapan diambil.
ajustes2-icono = Waktu di layar
ajustes2-ayuda-icono = Berapa lama kotak yang tidak aktif bertahan setelah tangkapan terakhir.
ajustes2-ayuda-ignorado = Selama program ini di depan, pintasan PinDesk tidak melakukan apa-apa.
ajustes2-anadir-programa = Tambah program
ajustes2-ayuda-anadir-programa = Ketik nama file eksekusinya lalu tekan Enter.
ajustes2-anadir = Tambah
ajustes2-ayuda-region = { $x }, { $y } · { $ancho } × { $alto } px
ajustes2-ayuda-iman = Saat menggambar, kursor menempel ke yang sudah ada.
ajustes2-ayuda-iman-esquinas = Sudut bentuk dan ujung garis.
ajustes2-ayuda-iman-medios = Titik tengah setiap sisi.
ajustes2-ayuda-iman-centros = Pusat setiap bentuk.
ajustes2-iman-radio = Jarak tempel
ajustes2-ayuda-iman-radio = Dalam piksel layar.
ajustes2-suavizado = Penghalusan goresan
ajustes2-ayuda-suavizado = Alami menghilangkan getaran tangan yang diam tanpa memperlambat ujung pena.
ajustes2-ayuda-voz-segundo = Whisper juga memahami bahasa ini di dalam catatan yang sama.
ajustes2-ayuda-voz-modo = Apa yang dilakukan dengan yang diucapkan dalam bahasa kedua.
ajustes2-presencia = Biarkan ponsel menemukan saya
ajustes2-ayuda-presencia = Ponsel melihat komputer ini begitu membuka Sinkronkan. Jika nonaktif, tidak ada port yang dibuka di jaringan.
ajustes2-lo-mio = Milik saya menang
ajustes2-ayuda-lo-mio = Saat sinkron, komputer ini yang menang: hanya yang tidak ada di sini yang datang dari sisi lain.
ajustes2-vista-rapida = Lihat cepat dengan spasi
ajustes2-ayuda-vista-rapida = Di File Explorer atau di desktop, pilih file dan tekan Spasi: langsung terbuka di PinDesk (foto, video, PDF, Word, denah DWG, model IFC dan Revit…). Spasi lagi untuk menutup.
ajustes2-abrir-con = Tawarkan PinDesk untuk gambar dan video
ajustes2-ayuda-abrir-con = Muncul di “Buka dengan” dan di Aplikasi bawaan. Menonaktifkannya menghapus yang ditulis ke registri.
ajustes2-predeterminada = Aplikasi bawaan
ajustes2-ayuda-predeterminada = Windows hanya mengizinkan memilihnya di Pengaturan: di sana, PinDesk → Atur bawaan.
ajustes2-hacer-predeterminada = Jadikan PinDesk aplikasi bawaan…
ajustes2-ayuda-cosmos = Langit berbintang di belakang daftar proyek obrolan. Berlaku mulai obrolan berikutnya.
ajustes2-nivel = Tingkat performa
ajustes2-ayuda-nivel = Otomatis mengukur komputer saat dijalankan. Berlaku mulai saat dijalankan berikutnya.
ajustes2-medir = Ukur bingkai editor
ajustes2-ayuda-medir = Mencatat berapa lama setiap bingkai. Hanya untuk diagnostik.
ajustes2-ritmo = Gambar tepat sebelum penyegaran
ajustes2-ayuda-ritmo = Ujung pena lebih sedikit tertinggal; perhitungan yang meleset kehilangan satu penyegaran.
ajustes2-paneo = Gulir tanpa menggambar ulang
ajustes2-ayuda-paneo = Kompositor Windows yang memindahkan dan memperbesar kanvas. Jika nonaktif, setiap bingkai digambar ulang.
ajustes2-corte = Ambang saat pena diam
ajustes2-ayuda-corte = Lebih rendah = getaran berkurang dan jeda lebih besar saat mulai. Otomatis = 1 Hz.
ajustes2-beta = Seberapa mengikuti kecepatan
ajustes2-ayuda-beta = Lebih tinggi = ujung lebih dekat ke kursor dan lebih bergetar. Otomatis = 0.007.
ajustes2-fichero = File pengaturan
ajustes2-ayuda-fichero = Yang tidak ada di sini tetap ada di sana, beserta komentarnya. Edit manual berlaku setelah dijalankan ulang.
ajustes2-abrir-fichero = Buka file
ajustes2-u-s = { $n } dtk
ajustes2-u-px = { $n } px
ajustes2-u-dias = { $n ->
   *[other] { $n } hari
  }
ajustes2-u-hz = { $n } Hz
ajustes2-nunca = Tidak pernah
ajustes2-siempre = Selalu
ajustes2-auto = Otomatis
## v2-buscar
comando-buscar = 🔍 Cari di PinDesk…
comando-grabar-clase = 🎙 Rekam kelas atau rapat…
clase-titulo = Rekam kelas atau rapat
clase-microfono = Mikrofon
clase-equipo = Suara komputer (Meet, Zoom…)
clase-guardar-en = Simpan ke: { $proyecto }
clase-empezar = Mulai merekam
clase-nada = Pilih mikrofon, suara komputer, atau keduanya
clase-captura = Tangkap
clase-terminar = Selesai
clase-capturas = Tangkapan layar: { $n } · satu klik mengulang area
clase-elige-zona = Pilih area di layar…
clase-captura-hecha = Tangkapan layar disimpan di { $minuto }
clase-zona-cambiada = Area diubah
clase-en-pausa = Dijeda: tidak ada yang direkam
clase-tirar = Tekan ✕ lagi untuk membuang rekaman
clase-sin-zona = Tidak dapat menangkap area
clase-abriendo = Membuka audio…
clase-guardando = Menyimpan kelas…
clase-texto = Mentranskripsikan… { $avance } %
clase-bajando = Mengunduh pengenal (hanya pertama kali)… { $avance } %
clase-lista = Disimpan ke “{ $proyecto }”
clase-lista-con-texto = Dengan { $n } tangkapan layar dan transkrip. Buka di obrolan untuk melihatnya di samping gambar.
clase-lista-sin-texto = Dengan { $n } tangkapan layar. Untuk teksnya, tekan “Ubah menjadi teks” pada catatan di obrolan.
clase-sin-texto = Biarkan tanpa teks
clase-listo = Selesai
clase-nombre = Kelas { $fecha }
clase-sin-sonido-equipo = Tidak ada suara dari komputer: tidak ada speaker atau headphone yang aktif
clase-video = Video layar ini
clase-video-roto = Video berhenti; audio masih direkam
clase-video-guardado = Dengan video layar.
clase-calidad-ligero = Ringan
clase-calidad-normal = Normal
clase-calidad-nitido = Tajam
clase-calidad-ligero-pista = Video 720p pada 10 fps: file terkecil (sekitar 180 MB per jam)
clase-calidad-normal-pista = Video 1080p pada 15 fps: paling banyak sekitar 300 MB per jam
clase-calidad-nitido-pista = Layar apa adanya pada 30 fps: teks kecil tetap terbaca, file lebih besar
clase-video-con = Video dengan { $codificador }
clase-codificador-grafica = kartu grafis (hampir tidak memakai prosesor)
clase-codificador-software = prosesor
clase-codificador-cpu = prosesor, mode sederhana
clase-zona-perdida = Tidak ada area yang dipilih: tekan “Tangkap” lagi
clase-salvado = Sisanya disimpan: { $n } tangkapan layar
clase-disco-poco = Ruang disk hampir habis (kurang dari 2 GB)
clase-disco-lleno = Disk hampir penuh: video ditutup dan audio masih direkam
clase-suspendido = Komputer tertidur selama { $minutos } mnt; rekaman berlanjut saat bangun
clase-recuperar-titulo = Rekaman terputus
clase-recuperar-pregunta =
    Sebuah rekaman terputus ({ $nombre }, { $minutos } mnt) di “{ $chat }”.
    Ya: Pulihkan · Tidak: Buang · Batal: tanya lagi saat PinDesk dibuka berikutnya
clase-recuperar-sin-nada = Dari “{ $nombre }” hanya tersisa video yang tidak bisa dibuka. Hapus?
clase-recuperar-descartar = Buang “{ $nombre }”? Rekaman akan dihapus dan tidak bisa dipulihkan.
clase-recuperar-fallo = “{ $nombre }” tidak dapat dipulihkan. Rekaman masih aman dan akan ditawarkan lagi saat PinDesk dibuka.
clase-texto-de-fondo = Tutup (tetap berjalan)
clase-abrir-para-estudiar = Buka untuk belajar
clase-camara-movil = 📱 Kamera ponsel
clase-camara-movil-n = 📱 Kamera ponsel · { $n }
clase-camara-movil-apagada = 📱 Kamera ponsel: nonaktif
clase-foto-movil = Foto dari ponsel · { $minuto }
clase-clip-movil = Klip dari ponsel · { $minuto }
camara-movil-tarde = Dari ponsel ke “{ $titulo }” · { $minuto }
# Photos by their time, from the PC to the phone's recording and collaborators (28-oct-2026).
aporte-captura-al-movil = Tangkapan layar untuk rekaman ponsel
aporte-de-la-grabacion = { $movil } sedang merekam · { $minuto }
aporte-enviada = Dikirim ke ponsel · { $minuto }
aporte-pendiente = Offline: akan dikirim saat tersambung lagi
aporte-fallo = Tidak dapat menangkap layar
chat-adjuntar-a-la-grabacion = Lampirkan ke rekaman…
chat-adjuntada-a-la-grabacion = Di rekaman · { $minuto }
chat-sin-grabacion = Tidak ada rekaman di obrolan ini
chat-colaboradores-todos = Izinkan semua orang di obrolan menambahkan foto
chat-colaboradores-nadie = Hanya foto saya di rekaman ini
clase-hora-sin-comprobar = waktu belum diverifikasi
baliza-movil-aislado = Ponsel Anda tidak muncul, padahal di jaringan yang sama: Wi-Fi ini mungkin memisahkan perangkat (jaringan tamu, hotel, dan kantor sering begitu) atau firewall mungkin memblokir PinDesk. Sambungkan PC ke hotspot ponsel, atau keduanya ke Wi-Fi lain. Foto yang Anda ambil menunggu di ponsel dan masuk di menitnya setelah keduanya saling menemukan.
baliza-movil-otra-red = Ponsel Anda berada di jaringan yang berbeda dari PC ini. Sambungkan keduanya ke Wi-Fi yang sama (atau PC ke hotspot ponsel). Foto yang Anda ambil menunggu di ponsel dan masuk di menitnya setelah keduanya saling menemukan.
# In-person meeting (27-oct-2026): the computer's camera, photos and clips.
clase-camara = Kamera: { $camara }
clase-camara-buscando = Mencari kamera…
clase-camara-ninguna = Tidak ada kamera: Foto dan Klip tidak akan berfungsi (audio tetap direkam)
clase-foto = Foto
clase-foto-hecha = Foto disimpan di { $minuto }
clase-camara-abriendo = Membuka kamera…
clase-camara-lista = Kamera siap: foto { $medida }
clase-camara-sin = Tidak ada kamera yang tersambung
clase-camara-en-uso = Aplikasi lain sedang memakai kamera (Teams, Zoom…): tutup lalu coba lagi
clase-camara-sin-permiso = Windows tidak mengizinkan akses kamera: aktifkan di Pengaturan › Privasi › Kamera
clase-camara-fallo = Kamera tidak merespons; coba lagi
clase-clip-empezado = Merekam klip dengan kamera… tekan ■ untuk menghentikan
clase-clip-guardado = Klip disimpan di { $minuto }
clase-clip-corto = Klip tidak dapat ditutup
# A single recording (29-oct-2026): no modes; Photo, Capture, Screen and Clip on the bar,
# the clock that pauses and ends, and full screen with the topics and the transcript.
clase-unica-sub = Suara selalu; di bilah, Foto, Tangkap, Layar, dan Klip
clase-pantalla-al-empezar = Mulai merekam layar
clase-pantalla = Layar
clase-clip = Klip
clase-pantalla-encendida = Merekam layar; audio tetap berjalan
clase-pantalla-apagada = Layar nonaktif; audio masih direkam
clase-pista-unica = Klik jam: jeda · tahan: akhiri · tahan “Tangkap”: area
clase-imagenes = { $n } gambar dan klip
clase-completa-temas = Yang akan saya sampaikan
clase-completa-sin-temas = Tidak ada topik: tambahkan di “Topik rapat” sebelum mulai
clase-completa-dice = Yang sedang dikatakan
clase-completa-sin-frases = Mendengarkan… kalimat muncul di sini segera setelah diucapkan
clase-completa-sin-escucha = Transkrip langsung memerlukan kunci Jev (TypeSafe) dan pengenal ucapan; teks lengkap muncul saat Anda selesai
clase-descartar = Buang
clase-descartar-otra = Tekan “Buang” lagi untuk menghapus rekaman
estudiar-clip = Video kamera · { $duracion }
bandeja-grabando = PinDesk — Merekam…
salir-grabando-titulo = Rapat sedang direkam
salir-grabando-mensaje =
    Selesaikan dan simpan rekaman sebelum keluar?
    Ya: selesai, simpan ke obrolannya, lalu keluar (minta teksnya nanti di catatan).
    Tidak: lanjutkan merekam.
chat-estudiar-clase = Pelajari kelas
estudiar-titulo = Pelajari kelas
estudiar-transcripcion = Transkrip
estudiar-capturas = Tangkapan layar
estudiar-marcadores = Penanda
estudiar-marcador = Penanda
estudiar-seguir = Ikuti
estudiar-pista-teclas = Spasi memutar · ←/→ 5 dtk · J/L 10 dtk · M penanda
estudiar-nota-del-marcador = Ketik catatan lalu tekan Enter (Esc membatalkan)
estudiar-sin-marcadores = Tekan “Penanda” atau M untuk mencatat sesuatu di menit ini.
estudiar-marcador-sin-nota = Penanda
estudiar-repasar = Ulas dengan suara
repaso-titulo = Ulas dengan suara
repaso-cabecera = Ulas dengan suara · { $clase }
repaso-explicacion = Jelaskan kelas dengan kata-kata Anda sendiri, seperti bercerita kepada seseorang. Jev menandai setiap poin penting: ✓ dibahas, ✗ salah, · terlewat, ? kurang jelas. Yang salah atau terlewat menampilkan menitnya agar Anda bisa kembali ke kelas.
repaso-sin-puntos = Kelas ini belum punya poin penting. Poin berasal dari yang ditandai penting oleh Jev saat ditranskripsi, dari penanda dengan catatan (M), dan dari topik yang dibahas.
repaso-recuento = { $cubiertos } dari { $total } dibahas · { $mal } salah · { $faltan } terlewat · { $dudosos } kurang jelas
repaso-preparando = Menyiapkan mikrofon…
repaso-te-escucho = Mendengarkan: mulailah menjelaskan.
repaso-pulsa-hablar = Tekan "Mulai bicara" (atau spasi) dan jelaskan kelasnya.
repaso-hablar = Mulai bicara
repaso-parar = Berhenti
repaso-otra-vez = Mulai lagi
repaso-cerrar = Tutup
repaso-jev-fallo = Jev tidak menjawab: poin-poinnya tetap seperti semula.
estudiar-marcador-guardado = Penanda disimpan
estudiar-no-se-guardo = Tidak dapat menyimpan
estudiar-sin-texto = Belum ada transkrip: tekan “Ubah menjadi teks” pada catatan di obrolan dan transkrip akan muncul di sini dengan sendirinya.
estudiar-sin-capturas = Kelas ini tidak punya tangkapan layar.
estudiar-sin-video = Video ini tidak bisa diputar di sini; catatan suara yang diputar sebagai gantinya
estudiar-sin-sonido = Audio kelas ini tidak ditemukan
estudiar-solo-audio = Hanya audio
estudiar-cargando = Memuat…
estudiar-ver-video = Tampilkan video
estudiar-ver-captura = Tampilkan tangkapan layar
estudiar-anotar = Anotasi
estudiar-ver-texto = Teks
estudiar-fotograma-guardado = Bingkai disimpan sebagai tangkapan layar: beri anotasi di kanvas
estudiar-nada-que-anotar = Belum ada yang bisa dianotasi di sini
estudiar-clip-movil = Video ponsel · { $duracion }
estudiar-sin-hevc = Untuk memutar klip ini, pasang “HEVC Video Extensions” dari Microsoft Store
anki-enviar = Kirim ke Anki
anki-pregunta-clase = Apa yang dijelaskan di sini?
anki-nada = Tidak ada kartu di sini. Tulis satu per baris sebagai “pertanyaan :: jawaban”, atau “Q: …” dan “A: …”, atau dengan celah {"{{"}c1::seperti ini{"}}"}.
anki-hecho = { $n } kartu di Anki, dek “{ $mazo }”
anki-ya-estaban = Kartu-kartu itu sudah ada di “{ $mazo }”
anki-fichero = Anki tidak terbuka (atau AnkiConnect tidak ada). Saya menyimpan { $n } kartu di { $donde }: di Anki, File → Impor.
anki-fallo = Anki tidak menerimanya: { $motivo }
# Sharing a recording (class or meeting): the HTML page and its formats.
compartir-anki = Anki
compartir-tipo-grabacion = Rekaman
grabacion-idioma = id
grabacion-fecha = { $dia }/{ $mes }/{ $ano } { $hora }
grabacion-transcripcion = Transkrip
grabacion-capturas = Tangkapan layar
grabacion-captura = Tangkapan layar
grabacion-marcadores = Penanda
grabacion-marcador = Penanda
grabacion-duracion = Durasi
grabacion-sin-texto = Rekaman ini belum punya transkrip.
grabacion-buscar = Cari di transkrip
grabacion-seguir = Ikuti audio
grabacion-reproducir = Putar atau jeda
grabacion-velocidad = Kecepatan
grabacion-ir-al-minuto = Buka momen ini
grabacion-cerrar = Tutup
grabacion-audio = Audio
grabacion-video = Video
buscar-todo-titulo = Cari di PinDesk
buscar-todo-pista = Cari file, tugas, pelajaran, tangkapan layar… atau ketik satu huruf
buscar-todo-pestana-todo = Semua
buscar-todo-pestana-archivos = File
buscar-todo-pestana-tareas = Tugas
buscar-todo-pestana-lecciones = Pelajaran
buscar-todo-pestana-capturas = Tangkapan layar
buscar-todo-pestana-acciones = Tindakan
buscar-todo-cambia = ganti
buscar-todo-resultados = { $n ->
   *[other] { $n } hasil
  }
buscar-todo-ninguno = Tidak ada hasil untuk “{ $consulta }”
buscar-todo-vacia = Belum ada apa pun di sini
buscar-todo-ninguno-pista = Coba lebih sedikit huruf, atau cari di tab lain (Tab)
buscar-todo-recientes = Pencarian terbaru
buscar-todo-borrar-todas = Hapus semua
buscar-todo-abierto-hace-poco = Baru dibuka
buscar-todo-letras = Ketik satu huruf dan spasi
buscar-todo-mejor = Hasil teratas
buscar-todo-grupo-tareas = Tugas dan pelajaran
buscar-todo-grupo-archivos = File
buscar-todo-grupo-capturas = Tangkapan layar
buscar-todo-grupo-acciones = Tindakan
buscar-todo-n-de-m = { $n } dari { $total }
buscar-todo-pegar = Tempel gambar dengan
buscar-todo-pegar-2 = dan gambar itu ikut dengan yang Anda ketik:
buscar-todo-pegar-ejemplo = t periksa retakan
buscar-todo-letra-t = Tugas baru
buscar-todo-letra-t-sub = ke Kotak masuk · “t beli semen”
buscar-todo-letra-n = Catatan baru
buscar-todo-letra-n-sub = di Pesan tersimpan · “n ide @proyek”
buscar-todo-letra-l = Kanvas baru
buscar-todo-letra-l-sub = kosong · “l denah @proyek”
buscar-todo-letra-g = Galeri tangkapan layar
buscar-todo-letra-g-sub = “g dinding” mencari di dalamnya
buscar-todo-letra-c = Tangkap
buscar-todo-letra-c-sub = area layar
buscar-todo-letra-u = Pin tangkapan layar terakhir
buscar-todo-letra-u-sub-nada = belum ada tangkapan layar
buscar-todo-letra-a = Pelajaran baru
buscar-todo-letra-a-sub = yang Anda pelajari · “a periksa skala”
buscar-todo-accion-pinear = Pin
buscar-todo-accion-abrir = Buka
buscar-todo-accion-apuntar = Tambah
buscar-todo-accion-capturar = Tangkap
buscar-todo-accion-crear = Buat
buscar-todo-accion-grabar = Rekam
buscar-todo-accion-enviar = Kirim
buscar-todo-accion-copiar = Salin
buscar-todo-accion-reproducir = Putar
buscar-todo-accion-hecha = Selesai
buscar-todo-accion-desmarcar = Batalkan selesai
buscar-todo-accion-entrar = Masuk
buscar-todo-accion-mostrar = Tampilkan
buscar-todo-accion-pegar = Tempel
buscar-todo-ver-chat = Tampilkan di obrolan
buscar-todo-mas = Tindakan lain
buscar-todo-pie-moverse = pindah
buscar-todo-pie-abrir = buka
buscar-todo-pie-adjuntar = lampirkan gambar
buscar-todo-pie-cerrar = tutup
buscar-todo-pie-borrar = hapus
buscar-todo-pie-mas = tindakan lain
buscar-todo-pie-volver = kembali
buscar-todo-pie-flow = Sama di Flow Launcher dengan “p”
buscar-todo-abrir-con = Buka dengan…
buscar-todo-borrar = Hapus (bisa diurungkan)
buscar-todo-medidas = Tangkapan layar · { $ancho } × { $alto } · { $peso }
buscar-todo-caduca = Kedaluwarsa dalam { $dias ->
   *[other] { $dias } hari
  }
buscar-todo-caduca-hoy = Kedaluwarsa hari ini
buscar-todo-conservada = Dipertahankan: tidak pernah kedaluwarsa
buscar-todo-conservada-ya = Dipertahankan
buscar-todo-conservar = Pertahankan
buscar-todo-borrada = Tangkapan layar dihapus
buscar-todo-deshacer = Urungkan
buscar-todo-copiado = Disalin
## v2-lecciones
# The lessons window of the v2 redesign (Lecciones2.dc.html and Lecciones2-repaso.dc.html).
## v2-galeria
galeria-subtitulo = { $cuantas } · disimpan selama 7 hari
galeria-buscar = Cari, termasuk teks di tangkapan
galeria-buscar-sin-ocr = Cari berdasarkan nama atau tanggal
galeria-elegir = Pilih
galeria-sin-resultados = Tidak ada tangkapan dengan “{ $texto }”.
galeria-hoy = Hari ini
galeria-ayer = Kemarin
galeria-dia-0 = Senin
galeria-dia-1 = Selasa
galeria-dia-2 = Rabu
galeria-dia-3 = Kamis
galeria-dia-4 = Jumat
galeria-dia-5 = Sabtu
galeria-dia-6 = Minggu
galeria-elegir-dia = Pilih seharian
galeria-chip-video = Video
galeria-pinear = Pin
galeria-copiar = Salin
galeria-conservar = Pertahankan
galeria-borrar = Hapus
galeria-elegidas = { $cuantas ->
   *[other] { $cuantas } dipilih
}
galeria-elegir-todas = Semua
galeria-deshacer = Urungkan
galeria-borradas = { $cuantas } dipindahkan ke tempat sampah PinDesk
galeria-recuperadas = { $cuantas ->
    [one] Dipulihkan
   *[other] { $cuantas } dipulihkan
}
galeria-pineadas = { $cuantas } di-pin di layar
galeria-copiadas = { $cuantas } disalin sebagai file
galeria-conservadas-aviso = { $cuantas } dipertahankan: ada di Pesan tersimpan
galeria-prorrogada = Tujuh hari lagi: akan dihapus nanti
## v2-pines
# The pin toolbar, its right-click menu and the "Open pins" panel.
pin-v2-anotar = Anotasi
pin-v2-mas = Lainnya
pin-v2-opacidad = Opasitas
pin-v2-pines-abiertos = Pin terbuka…
pin-v2-abrir = Buka
pin-v2-pinear-pagina = Pin halaman ini
pin-v2-pinear-todas = Pin semua halaman
pin-v2-cerrar-pin = Tutup pin
pin-v2-alejar = Perkecil
pin-v2-acercar = Perbesar
pin-v2-en-vivo = LIVE
pin-v2-tecla-espacio = Spasi
pin-v2-tecla-rueda = Roda
pin-v2-tecla-re-pag = PgUp
pin-v2-tecla-av-pag = PgDn
pin-v2-tecla-mayus-rueda = Shift+Roda
pin-v2-panel-titulo = Pin terbuka
pin-v2-panel-buscar = Cari pin
pin-v2-panel-mostrar-todos = Tampilkan semua
pin-v2-panel-ocultar-todos = Sembunyikan semua
pin-v2-panel-cerrar-todos = Tutup semua · bisa diurungkan
pin-v2-panel-deshacer = Urungkan: buka kembali
pin-v2-panel-sin-grupo = Tanpa grup
pin-v2-panel-oculto = Tersembunyi
pin-v2-panel-ayuda = Klik baris untuk membawa pin ke tengah dan menyorotnya. Ikon mata menyembunyikan atau menampilkannya lagi.
pin-v2-panel-vacio = Tidak ada pin terbuka. Pin tangkapan, catatan, atau file dan akan muncul di sini.
pin-v2-panel-ocultar = Sembunyikan
pin-v2-panel-mostrar = Tampilkan
pin-v2-panel-cerrar-este = Tutup pin ini
pin-v2-panel-cerrar-panel = Tutup panel
pin-v2-tipo-foto = Foto
pin-v2-tipo-nota = Catatan
pin-v2-tipo-pdf = PDF
pin-v2-tipo-video = Video
pin-v2-tipo-archivo = File
pin-v2-tipo-vivo = Pin langsung
pin-v2-tipo-herramienta = Alat
pin-v2-marca-copiar = Salin
pin-v2-marca-resaltar = Sorot
pin-v2-marca-ondulada = Garis bergelombang
pin-v2-marca-subrayar = Garis bawah
pin-v2-marca-tachar = Coret
pin-v2-marca-tapar = Tutupi
pin-v2-marca-quitar = Hapus tanda
marcas-texto-color = Warna
pin-v2-modo-texto = Pilih teks
pin-v2-leyendo = Membaca…
pin-v2-invertir-colores = Balik warna
## v2-captura
captura2-zona = Area
captura2-ventana = Jendela
captura2-pantalla = Layar
captura2-scroll = Bergulir
captura2-gif = Rekam GIF
captura2-pin-vivo = Pin langsung
captura2-texto = Teks (OCR)
captura2-libre = Bebas
captura2-repetir = Ulangi area terakhir
captura2-ancho = L
captura2-alto = T
captura2-sin-abrir = Tanpa membuka ini:
captura2-arrastrar-izq = + seret kiri
captura2-arrastrar-der = + seret kanan
captura2-arrastrar-centro = + seret tengah
captura2-doble-centro = + klik tengah ganda
captura2-gesto-copiar = salin
captura2-gesto-pinear = pin
captura2-gesto-pin-vivo = pin langsung
captura2-gesto-anotar = gambar di layar
captura2-salir = keluar
captura2-pista-ventana = Klik: tangkap jendela · Seret: area bebas
captura2-pista-zona = Klik: tangkap ini · Seret: area bebas
captura2-copiar-color = salin warna
captura2-cambiar-formato = HEX / RGB
captura2-copiar = Salin
captura2-pinear = Pin
captura2-al-chat = Ke obrolan
captura2-ocr = Teks
captura2-guardar = Simpan
captura2-descartar = Buang
captura2-buscar-proyecto = Cari proyek…
captura2-recientes = Terbaru
captura2-el-ultimo = terakhir dipakai
captura2-comentario = Tambahkan komentar (opsional)
captura2-enviar = Kirim
captura2-sin-proyectos = Tidak ada proyek yang cocok
captura2-enviada = Tangkapan dikirim ke “{ $proyecto }”.
## v2-bandeja
# The tray panel (left click on the icon). The right-click menu keeps its
# usual keys.
bandeja2-sincronizar = Sinkronkan sekarang
bandeja2-sincronizado = Disinkronkan { $cuando }
bandeja2-sin-sincronizar = Belum disinkronkan
bandeja2-hace-nada = baru saja
bandeja2-hace-min = { $n } mnt lalu
bandeja2-hace-h = { $n } jam lalu
bandeja2-ayer = kemarin
bandeja2-hace-dias = { $n } hari lalu
bandeja2-buscar = Cari tindakan… (ketik saja)
bandeja2-buscar-favorito = Cari yang ingin ditambahkan sebagai favorit…
bandeja2-sin-resultados = Tidak ada tindakan dengan nama itu
bandeja2-capturar = Tangkap
bandeja2-capturar-sub = Pilih area dengan mouse
bandeja2-grabar = Rekam
bandeja2-modo-zona = Area
bandeja2-modo-scroll = Gulir
bandeja2-modo-gif = GIF
bandeja2-modo-texto = Teks
bandeja2-modo-retardo = Tunda
bandeja2-modo-color = Warna
bandeja2-favoritos = Favorit
bandeja2-editar = Edit
bandeja2-anadir = Tambah
bandeja2-elige-favoritos = Pilih favorit Anda ({ $n } dari { $tope })
bandeja2-favoritos-llenos = Sudah ada { $tope } favorit: hapus satu dulu.
bandeja2-hecho = Selesai
bandeja2-fav-pinear-copiado = Pin yang disalin
bandeja2-fav-anotar = Anotasi layar
bandeja2-fav-pin-vivo = Pin langsung
bandeja2-pines = Pin · { $n } di layar
bandeja2-ocultar-pines = Sembunyikan pin
bandeja2-paso-clic = Teruskan klik
bandeja2-atajos = Pintasan
bandeja2-silenciar = Bisukan pintasan
bandeja2-silenciar-sub = untuk game atau presentasi
bandeja2-ultimas = Tangkapan terbaru
bandeja2-ver-galeria = Lihat galeri
bandeja2-sin-capturas = Belum ada tangkapan
bandeja2-abrir-pindesk = Buka PinDesk
bandeja2-tareas = Tugas
bandeja2-lecciones = Pelajaran
bandeja2-nueva-leccion = Pelajaran baru
bandeja2-galeria = Galeri tangkapan
bandeja2-documento = Buka dokumen…
bandeja2-grupos = Grup jendela
bandeja2-para-hoy = { $n } untuk hari ini
bandeja2-salir = Keluar dari PinDesk
bandeja2-salir-sub = pin akan ditutup
bandeja2-copiada = Disalin
bandeja2-borrada = Dipindahkan ke tempat sampah
bandeja2-devuelta = Dipulihkan
bandeja2-deshacer = Urungkan
bandeja2-no-se-pudo = Tidak dapat dilakukan: { $motivo }

## tareas-v3
tareas3-buscar = Cari tugas, daftarnya, dan obrolannya… ("hari ini", "kemarin")
tareas3-encontradas = { $n ->
   *[other] { $n } tugas ditemukan
  }
tareas3-pendientes = { $n ->
    [0] tidak ada yang tertunda
   *[other] { $n } tertunda
  }
tareas3-sin-resultados = Tidak ada yang cocok dengan "{ $busqueda }"
tareas3-sin-resultados-pista = Mencari teks setiap tugas serta nama daftar dan obrolannya, tanpa memedulikan aksen dan huruf besar/kecil. Coba lebih sedikit kata, atau "hari ini" atau "kemarin".
tareas3-vaciar-busqueda = Hapus pencarian
tareas3-mover-intro = pindah
tareas3-mover-esc = tutup
tareas3-quitada = “{ $tarea }” dihapus
tareas3-deshacer = Urungkan
tareas3-repuesta = Tugas dikembalikan ke daftarnya
tareas3-cambio-quitar = Daftar berubah sementara itu; sekarang sudah terbaru. Hapus lagi.
tareas3-borrar-lista = Hapus daftar
tareas3-borrar-lista-aviso = Hapus daftar “{ $lista }” di “{ $chat }” beserta { $n ->
   *[other] { $n } tugasnya
  }? Daftar juga dihapus dari obrolan dan ponsel, dan tidak bisa diurungkan.
tareas3-lista-borrada = Daftar “{ $lista }” dihapus

## timeline (5-oct-2026)
timeline-titulo = Lini masa
timeline-sub-ultimas = 24 jam terakhir · { $n ->
   *[other] { $n } momen
  }
timeline-sub-dia = { $n ->
   *[other] { $n } momen
  }
timeline-ultimas-24h = 24 jam terakhir
timeline-hoy = Hari ini
timeline-ayer = Kemarin
timeline-exportar = Ekspor
timeline-volver = Kembali
timeline-pista = Apa yang sedang terjadi? Ketik atau diktekan…
timeline-frase-titulo = begini kejadiannya
timeline-frase-titulo-corta = yang terjadi adalah
timeline-frase-descripcion = dan ceritanya begini
timeline-frase-descripcion-corta = ceritanya begini
timeline-vacio = Belum ada apa pun dalam 24 jam terakhir.
    Ketik atau diktekan apa yang sedang terjadi lalu tekan Enter.
    Saat mendikte, «{ timeline-frase-titulo }…» memulai judul dan «{ timeline-frase-descripcion }…» deskripsinya.
timeline-vacio-dia = Tidak ada yang dicatat pada hari itu.
timeline-nota-de-voz = Catatan suara
timeline-sin-titulo-voz = Catatan suara
timeline-sin-titulo-foto = Foto
timeline-apuntado = Dicatat
timeline-corregido = Dikoreksi
timeline-borrado = Momen dihapus
timeline-deshacer = Urungkan
timeline-abrir = Buka
timeline-escribe-algo = Ketik sesuatu untuk judul
timeline-no-guardado = Tidak dapat menyimpan lini masa
timeline-corrigiendo = Mengoreksi momen { $hora } · Enter menyimpan, Esc membatalkan
timeline-voz-esperando = 🎙 Catatan suara ikut dengan teks: tekan Enter
timeline-pegar-no-imagen = Itu bukan gambar maupun teks
timeline-cal-elegidos = { $n ->
   *[other] { $n } hari dipilih
  }
timeline-cal-quitar = Hapus pilihan
timeline-exportar-html = Halaman web (HTML)
timeline-exportar-pdf = Dokumen PDF
timeline-tipo-html = Halaman web
timeline-tipo-pdf = Dokumen PDF
timeline-nada-que-exportar = Tidak ada yang bisa diekspor di sana
timeline-no-exportado = Tidak dapat menyimpan file
timeline-exportado = Disimpan: { $nombre }
timeline-exportado-pie = Diekspor dengan PinDesk · { $fecha }
timeline-pdf-haciendo = Membuat PDF…
timeline-pdf-sin-edge = PDF memerlukan Microsoft Edge; ekspor sebagai HTML
timeline-pdf-fallo = Tidak dapat membuat PDF
timeline-mic-abriendo = Membuka mikrofon…
timeline-mic-escuchando = Mendengarkan… tekan mikrofon (atau Ctrl+M) untuk selesai
timeline-mic-guardando = Menyimpan yang Anda ucapkan…
timeline-mic-bajando = Mengunduh model suara (sekali)… { $pc } %
timeline-mic-pasando = Mentranskripsikan… { $pc } %
timeline-mic-sin-dictado = Komputer ini tidak bisa mentranskripsi: ketik saja
timeline-mic-no-abre = Tidak dapat membuka mikrofon
timeline-mic-no-guarda = Tidak dapat menyimpan yang Anda ucapkan
timeline-mic-sin-microfono = Tidak ada mikrofon
timeline-mic-corto = Terlalu pendek: bicaralah sedikit lebih lama
timeline-mic-error = Tidak dapat merekam: { $error }
timeline-mic-sin-texto = Tidak dipahami: hanya audio yang disimpan


## simple-lecciones

## galeria-menu (5-oct-2026, right click)
galeria-guardar-como = Simpan sebagai…
galeria-mostrar-en-carpeta = Tampilkan di folder
galeria-guardadas = { $cuantas ->
    [one] Disimpan
   *[other] { $cuantas } disimpan
}
galeria-prorrogar = Beri 7 hari lagi

## timeline-v2 (5-oct-2026): common bar, WeChat-like tabs and detail
timeline-pestana-hoy = Hari ini
timeline-pestana-momentos = Momen
timeline-pestana-estado = Kalender
timeline-buscar-pista = Cari di lini masa
timeline-sub-buscar = { $n ->
    [0] Tidak ditemukan apa pun
   *[other] { $n } hasil
  }
timeline-sub-momentos = { $n ->
    [0] Belum ada momen
   *[other] Total { $n } momen
  }
timeline-sin-resultados = Tidak ada momen yang memuat itu.
    Judul dan deskripsi dicari tanpa memedulikan aksen dan huruf besar/kecil.
timeline-momentos-vacio = Momen yang Anda pilih masuk ke sini. Di "Hari ini", arahkan ke kartu dan tekan penanda untuk mengirimnya ke sini.
timeline-a-momentos = Dikirim ke Momen
timeline-fuera-de-momentos = Dihapus dari Momen
timeline-estado-sobre-todo = Terutama
timeline-exportar-busqueda = Cari: "{ $que }"
timeline-pestana-lecciones = Pelajaran
timeline-sub-lecciones = { $n ->
    [0] Tidak ada pelajaran
   *[other] { $n } pelajaran dipetik
  }
timeline-lecciones-vacio = Belum ada pelajaran. Tekan bola lampu pada momen untuk menjadikannya pelajaran.
timeline-gravedad-leve = Ringan
timeline-gravedad-importante = Penting
timeline-gravedad-grave = Serius
timeline-otra-vez-hecho = Dicatat: sekarang { $n } kali
timeline-leccion = Pelajaran
timeline-leccion-hecha = 💡 Pelajaran dibuat: ada di "Pelajaran"
timeline-leccion-quitada = Tidak lagi ditandai. Pelajaran tetap ada di "Pelajaran".
timeline-leccion-fallo = Tidak dapat menyimpan pelajaran
timeline-leccion-por-que = Mengapa
timeline-leccion-proxima = Lain kali
timeline-tambien-lecciones = { $n ->
   *[other] { $n } pelajaran juga cocok · Lihat
  }
timeline-copiar-imagen = Salin sebagai gambar
timeline-compartir-html = Bagikan sebagai HTML
timeline-guardar-imagen = Simpan sebagai gambar…
timeline-imagen-copiada = Disalin sebagai gambar
timeline-imagen-fallo = Tidak dapat membuat gambar
timeline-tipo-png = Gambar PNG
timeline-compartido-pie = PinDesk · Lini masa

## tareas-v4
# The search box hint in the shared top bar: shorter than tareas3-buscar,
# which does not fit in the space the bar leaves.
tareas4-buscar = Cari tugas…
timeline-otra-vez-corto = ↻ Terjadi lagi +1
timeline-pagina-linea = Lini masa
timeline-exportado-pesa = ukurannya { $mb } MB: mungkin sulit dikirim

## v2-diseno
# The gallery with the v2 design system (Oct 6).
galeria-chapa-hoy = hari ini
galeria-pista-caduca = { $dias ->
    [0] Dihapus hari ini
    [one] Dihapus besok
   *[other] Dihapus dalam { $dias } hari
  } · klik kanan untuk mempertahankan
galeria-ya-conservadas = Sudah dipertahankan sebelumnya
galeria-dia-cuantas = { $cuantas ->
   *[other] { $cuantas } tangkapan
  }
timeline-sub-estado-dias = { $n ->
    [0] Belum ada hari yang berisi
   *[other] { $n } hari berisi
  }
timeline-filtro-todas = Semua
timeline-orden-recientes = Terbaru
timeline-orden-repetidas = Paling sering berulang
timeline-lecciones-sin-resultados = Tidak ada pelajaran yang memuat itu.
timeline-otra-vez-pildora = ↻ +1

## tareas-v5
# The bar subtitle: each group already counts what is pending.
tareas5-subtitulo = { $listas ->
   *[other] { $listas } daftar
  }
tareas5-apuntar-pista = Tugas baru…
tareas-recordare = ⏰ Saya akan mengingatkan Anda { $cuando }
tareas-sin-recordatorio = Pengingat dihapus
tareas-apuntar-en-pista = Tugas baru di “{ $lista }”…
tareas5-corregir-pista = Teks tugas…
tareas5-imagen-pista = Tambah gambar · Ctrl+V menempel yang disalin
tareas5-guardar = Simpan
tareas5-corregida = Tugas diperbarui
tareas5-vista-tarjetas = Kartu · Ctrl+T
tareas5-vista-horario = Jadwal · Ctrl+H
tareas5-vista-kanban = Papan Kanban · Ctrl+K
tareas5-marcar = Tandai selesai
tareas5-desmarcar = Tandai tertunda
tareas5-editar = Edit
tareas5-copiar-texto = Salin teks
tareas5-copiada = Teks disalin
tareas5-pinear-imagenes = { $n ->
    [one] Pin gambarnya
   *[other] Pin { $n } gambar
  }
tareas5-quitar = Hapus

## salida (6-oct-2026): the floating window to drag out what was exported
salida-titulo = Siap dibagikan
salida-titulo-exportado = Diekspor
salida-pista = Seret file ke aplikasi lain atau ke folder
salida-todos = Seret { $n } sekaligus
salida-arrastra-todos = Tahan dan seret untuk membawa semuanya
salida-soltado = Dilepas · Anda juga bisa melepasnya di tempat lain
salida-copiado = Disalin ke papan klip
salida-pineado = Di-pin
salida-no-se-pudo = Tidak dapat dilakukan
salida-copiar = Salin
salida-abrir = Buka
salida-carpeta = Tampilkan di folder
salida-pinear = Pin
salida-compartir-menu = Bagikan…

## al-movil (6-oct-2026): a photo from the Flow plugin to the phone's open canvas
al-movil-lienzo = { $n ->
    [one] Dikirim ke kanvas di { $movil }
   *[other] { $n } gambar dikirim ke kanvas di { $movil }
  }
al-movil-chat = { $movil } menyimpannya di percakapan umum (tidak ada obrolan atau kanvas yang terbuka)
al-movil-chat-abierto = { $n ->
    [one] Dikirim ke obrolan «{ $chat }» di { $movil }
   *[other] { $n } file dikirim ke obrolan «{ $chat }» di { $movil }
  }
al-movil-solo-fotos = { $movil } sedang membuka kanvas: hanya menerima foto. Buka obrolan untuk mengirim file.
al-movil-negados = { $negados ->
   *[other] { $negados } file tidak dikirim: bukan foto dan kanvas sedang terbuka
  }
al-movil-no-esta = { $movil } tidak terbuka: buka PinDesk di ponsel
al-movil-actualiza = Perbarui PinDesk di { $movil }: belum bisa menerima file langsung. Membuka «Kirim lewat Wi-Fi».
al-movil-ocupado = { $movil } sedang sinkron dengan perangkat lain. Coba lagi sebentar lagi.
al-movil-otra-version = { $movil } memakai versi PinDesk lain: perbarui keduanya
al-movil-cortado = Tidak dapat mengirim ke { $movil }: { $motivo }
al-movil-sin-grupo = PC ini tidak ada di grup: atur di Sinkronkan PinDesk
al-movil-no-es-del-grupo = Perangkat itu sudah tidak ada di grup PC ini
al-movil-sin-imagenes = Tidak ada yang bisa dikirim
al-frente-lienzo = { $de } mengirimi Anda “{ $nombre }”: ada di kanvas yang terbuka
al-frente-chat-abierto = { $de } mengirimi Anda “{ $nombre }” ke obrolan “{ $chat }”
al-frente-guardados = { $de } mengirimi Anda “{ $nombre }”: ada di Pesan tersimpan

## Excel sheet viewer (visor_hoja)
hoja-copiado = { $celdas ->
   *[other] { $celdas } sel disalin
  }
hoja-demasiado = Terlalu banyak sel untuk disalin sekaligus ({ $celdas })
hoja-cargando = Memuat… { $filas } baris
hoja-enlace-no-abre = Tautan tidak dapat dibuka

## Chats shared with people (Android 13-oct-2026, ChatCompartidoActivity)
cc-menu-compartir = Bagikan dengan orang lain
cc-menu-entrar = Gabung ke obrolan bersama
chat-nuevo-normal = Obrolan biasa
chat-nuevo-normal-detalle = Hanya Anda dan perangkat Anda
chat-nuevo-compartido = Obrolan bersama
chat-nuevo-compartido-detalle = Dengan orang lain, masing-masing di perangkatnya sendiri
cc-chapa-personas = { $n ->
   *[other] Dibagikan · { $n } orang
}
cc-nuevo-error = Tidak dapat membagikan obrolan baru: tetap menjadi obrolan biasa
cc-portada-titulo = Obrolan bersama
cc-portada-como = Obrolan proyek yang Anda kelola bersama orang lain: mereka hanya melihat obrolan itu. Untuk membagikannya, klik kanan proyek → Bagikan dengan orang lain.
cc-portada-entrar = Gabung ke obrolan bersama
cc-titulo-personas = Orang
cc-titulo-entrar = Gabung ke obrolan
cc-tu-nombre = Nama Anda di obrolan bersama
cc-guardar = Simpan
cc-compartir-explica = Bagikan obrolan ini dengan orang lain.
cc-compartir-detalle = Hanya obrolan ini yang dikirim: pesan, file, kanvas, dan anotasinya. Obrolan dan proyek Anda yang lain tetap tersembunyi. Semua orang saling sinkron di Wi-Fi yang sama.
cc-compartir = Bagikan
cc-ya-no-estas = Anda sudah tidak ada di obrolan ini. Yang Anda punya tetap di sini, tetapi tidak lagi sinkron.
cc-tu = (Anda)
cc-creo = membuat obrolan
cc-tu-otro-aparato = perangkat Anda yang lain
cc-salio = keluar
cc-quitado = dikeluarkan
cc-quitar = Keluarkan
cc-anadir = Tambah
cc-cerrar-invitacion = Tutup
cc-sincronizar = Sinkronkan
cc-invitacion-como = Di perangkat lain: PinDesk → Gabung ke obrolan bersama. Pindai ini atau ketik kodenya.
cc-invitacion-vale = Berlaku selama layar ini terbuka. Anda harus di Wi-Fi yang sama.
cc-salir-del-chat = Keluar dari obrolan
cc-salir-titulo = Keluar dari obrolan?
cc-quitar-titulo = Keluarkan { $nombre }?
cc-salir-aviso = Yang Anda punya tetap di sini, tetapi akan berhenti sinkron dengan yang lain.
cc-quitar-aviso = Mereka tetap menyimpan yang sudah mereka punya, tetapi tidak akan mendapat hal baru dari obrolan ini.
cc-salir = Keluar
cc-cancelar = Batal
cc-entrar-como = Orang yang mengundang Anda membuka obrolan → Bagikan dengan orang lain → Tambah, lalu menunjukkan kode. Anda hanya mendapat obrolan itu: tidak ada hal lain dari perangkatnya, dan Anda tidak memberikan apa pun milik Anda. Komputer ini tidak punya kamera: ketik kodenya atau tempel teks QR-nya.
cc-escribe-codigo = Ketik kode atau tempel teks QR
cc-entrar = Gabung
cc-ya-estas = Anda ada di “{ $nombre }”
cc-con = Dengan: { $nombres }
cc-cerrar = Tutup
cc-compartido = Dibagikan. Tekan Tambah untuk mengundang seseorang.
cc-entro = { $nombre } bergabung. Mengirim obrolan…
cc-listo = Selesai: mereka sudah punya obrolannya.
cc-demasiados = Terlalu banyak kode salah: undangan ditutup.
cc-has-salido = Anda keluar. Yang lain akan tahu saat sinkron.
cc-quitado-aviso = Dikeluarkan. Mereka akan tahu saat sinkron.
cc-nadie = Belum ada orang lain. Tekan Tambah.
cc-buscando = Mencari anggota obrolan…
cc-sincronizando-con = Menyinkronkan dengan { $nombre }…
cc-nadie-en-wifi = Tidak ada anggota obrolan di Wi-Fi ini saat ini.
cc-sincronizado = Disinkronkan dengan { $nombres }.
cc-buscando-invitacion = Mencari orang yang mengundang Anda…
cc-entrando = Bergabung…
cc-trayendo = Mengambil “{ $nombre }”…
cc-no-encontrada = Undangan tidak ditemukan. Periksa kodenya dan pastikan Anda di Wi-Fi yang sama.
cc-se-corto = Terputus: { $motivo }
cc-abrir-chat = Buka obrolan
cc-no-se-abre = Obrolan itu belum ada di sini: sinkronkan lagi.

## Pins like on Android (batch 6): color, stickers, paged note and own words
pin-v2-pegatina = Stiker
pin-v2-sin-pegatina = Tanpa stiker
pin-v2-paginas = Halaman
pin-v2-leer-seguida = Baca bersambung
pin-v2-pagina = Halaman
pin-v2-minutos = mnt
pin-color-copiado = Disalin
ajustes2-sec-pinear = Pin
ajustes2-sec-pinear-sub = Kata-kata yang menjadi alat saat di-pin.
ajustes2-sec-tareas = Tugas
ajustes2-sec-tareas-sub = Apa yang dipahami saat Anda mengetik tugas.
ajustes2-g-urgencia = Kata urgensi (tanpa kata ini, tugas bersifat normal)
ajustes2-urgente = Mendesak
ajustes2-muy-urgente = Sangat mendesak
ajustes2-urgencia-ninguna = Tidak ada: tidak ada yang mendapat urgensi ini
ajustes2-g-palabras = Kata ajaib
ajustes2-ayuda-palabras = Salin satu kata saja lalu pin: alatnya terbuka. Pisahkan dengan koma.
ajustes2-palabras-ninguna = Tidak ada
ajustes2-palabras-cambiar = Ubah
ajustes2-palabras-fabrica = Kata bawaan
ajustes2-palabras-volver = Atur ulang
ajustes2-palabras-marcador = kata, kata lain…

## Local assistant (13-oct-2026)
asistente-mencionar = Sebut: asisten atau tempat menggambar
asistente-abrir-lienzo = Buka kanvas
asistente-abrir-fuente = Buka sumber yang dikutip
asistente-ocupado = Asisten masih menjawab: tunggu atau tekan berhenti.
asistente-fallo = Asisten tidak dapat mulai menjawab.
asistente-nube-titulo = Kunci cloud tidak ada
asistente-nube-explica = Asisten menjawab dengan { $nube }: tempel kuncinya di Pengaturan → Asisten (dapatkan di { $pagina }). Perintah sederhana ("catat roti", "buka galeri") berfungsi tanpa kunci.
asistente-abrir-ajustes = Buka Pengaturan
asistente-viejos-titulo = Model lokal tidak lagi dipakai
asistente-viejos-explica = Asisten sekarang hanya menjawab lewat cloud (Claude atau DeepSeek). Yang Anda unduh untuk model PC ini masih memakan { $tamano } di disk.
asistente-viejos-liberar = Kosongkan { $tamano }
asistente-viejos-dejar = Biarkan
asistente-viejos-borrados = Selesai: model lokal dihapus dan ruangnya kosong.
asistente-viejos-no-borrados = Tidak semua model lokal dapat dihapus. Tutup yang memakainya dan coba lagi dari Pengaturan → Asisten.
ajustes2-asistente-borrar-locales = Model lokal lama
ajustes2-ayuda-asistente-borrar-locales = Yang diunduh untuk asisten lokal, tidak lagi dipakai: { $tamano } di %LOCALAPPDATA%\PinDesk\modelos.

## Cloud assistant (13-oct-2026)
ajustes2-sec-asistente = Asisten
ajustes2-sec-asistente-sub = Siapa yang menjawab di obrolan «Asistente»: Claude atau DeepSeek, dengan kunci Anda.
ajustes2-g-asistente = Mesin asisten
ajustes2-asistente-motor = Mesin
ajustes2-ayuda-asistente-motor = Menjawab lewat internet dengan kunci Anda: Claude Haiku atau DeepSeek.
asistente-confirmar-si = Ya
asistente-confirmar-no = Tidak
asistente-otro = Lainnya…
asistente-otro-escribe = Ketik jawaban Anda di bawah lalu kirim
asistente-recomendada = disarankan
asistente-receta-guardar = Simpan sebagai resep
asistente-modelo = Model
asistente-nueva-conversacion = Percakapan baru
asistente-nuevo-chat = Obrolan baru
asistente-borrar-chat = Bersihkan obrolan
asistente-borrar-chat-aviso = Semua pesan di obrolan «Asisten» dihapus (beserta lampirannya). Lanjutkan?
asistente-escuchar = Dengarkan
asistente-modelo-cambiado = { $modelo } menjawab sekarang.
asistente-receta-guardada = Resep disimpan: { $nombre }
asistente-receta-llenas = Terlalu banyak resep: hapus satu dulu.
asistente-receta-renombrar = Ganti nama
asistente-receta-editar = Edit permintaan
asistente-receta-icono = Ubah ikon
asistente-receta-izquierda = Pindah ke kiri
asistente-receta-derecha = Pindah ke kanan
asistente-receta-borrar = Hapus resep
asistente-receta-editando-nombre = Nama resep «{ $nombre }» · Enter menyimpan · Esc membatalkan
asistente-receta-editando-pedido = Permintaan resep «{ $nombre }» · Enter menyimpan · Esc membatalkan
ajustes2-asistente-docs-nube = Dokumen Anda dan cloud
ajustes2-ayuda-asistente-docs-nube = Dengan Claude atau DeepSeek, yang ditemukan asisten di PDF, catatan, dan obrolan Anda dikirim lewat internet untuk menjawab Anda.
ajustes2-docs-nube-preguntar = Tanyakan
ajustes2-docs-nube-si = Izinkan
ajustes2-docs-nube-no = Jangan izinkan
ajustes2-g-claves = Kunci API
ajustes2-clave-de = Kunci { $proveedor }
ajustes2-clave-marcador = Tempel kunci di sini
ajustes2-clave-probar = Simpan dan uji
ajustes2-clave-borrar-de = Hapus kunci { $proveedor }
ajustes2-ayuda-clave-borrar = Kunci dihapus dari PC ini. Kunci tetap berlaku di akun Anda sampai Anda mencabutnya di sana.
ajustes2-clave-borrar = Hapus
ajustes2-clave-guardada = Disimpan dan dienkripsi di PC ini:
ajustes2-clave-falta = Tidak ada kunci. Dapatkan di
ajustes2-clave-probando = menguji…
ajustes2-g-conexiones = Koneksi asisten
ajustes2-conexion-cliente = ID klien { $servicio }
ajustes2-conexion-cliente-marcador = Tempel ID klien (atau JSON yang diberikan Google)
ajustes2-conexion-guardar = Simpan
ajustes2-conexion-de = { $servicio }
ajustes2-conexion-conectar = Sambungkan
ajustes2-conexion-desconectar = Putuskan
ajustes2-conexion-sin-cliente = Tidak ada ID klien. Buat sekali di { $consola } (langkah: docs/conexiones-google-drive.md).
ajustes2-conexion-cliente-guardado = ID disimpan dan dienkripsi di PC ini:
ajustes2-conexion-conectado = Tersambung
ajustes2-conexion-sin-conectar = Tidak tersambung. Dengan izin Anda, asisten mencari, membaca, mengunduh, dan mengunggah file di { $servicio } Anda.
ajustes2-conexion-conectando = Selesaikan di browser…

## Assistant automations (Oct 13)
automatizacion-creada = Otomatisasi “{ $nombre }” dibuat.
automatizacion-activada = Otomatisasi “{ $nombre }” diaktifkan.
automatizacion-desactivada = Otomatisasi “{ $nombre }” dinonaktifkan.
automatizacion-borrada = Otomatisasi “{ $nombre }” dihapus.
automatizacion-en-marcha = Menjalankan “{ $nombre }”…
automatizacion-no-esta = Tidak ada otomatisasi “{ $id }”.
automatizacion-rota = Otomatisasi tidak dapat disimpan: { $motivo }
ajustes2-g-automatizaciones = Otomatisasi
ajustes2-automatizaciones-vacio = Belum ada
ajustes2-ayuda-automatizaciones-vacio = Minta ke asisten: “setiap kali saya menangkap, taruh di obrolan proyek”, “setiap hari pukul 8, ringkasan tugas tertunda”.
ajustes2-automatizacion-correr = Jalankan sekarang
ajustes2-automatizacion-otro-aparato = berjalan di { $aparato }
ajustes2-automatizacion-rota = Tidak dipahami: { $motivo }
ajustes2-automatizacion-borrar = Hapus otomatisasi “{ $nombre }”? Ini tidak bisa diurungkan.
ajustes-asistente-pensar = Berpikir sebelum menjawab
ajustes2-ayuda-asistente-pensar = Sebelum bertindak, asisten berpikir dan menulis apa yang dipahaminya; pemikirannya tampil, terlipat, di atas jawabannya. Berpikir butuh waktu: pada mode otomatis hanya berpikir untuk tugas rumit, seperti memecahkan masalah, membuat rencana, atau beberapa langkah berturut-turut, atau saat Anda meminta («pikirkan baik-baik», «langkah demi langkah»).
asistente-pensar-auto = Otomatis (hanya untuk tugas rumit)
asistente-pensar-si = Selalu
asistente-pensar-no = Tidak pernah
asistente-pensando = Berpikir…
asistente-penso = Berpikir selama { $s } dtk
asistente-penso-poco = Berpikir sejenak
asistente-reiniciado = Konteks diperbarui: melanjutkan dari lembar kerja

## Assistant voice (13-oct-2026): talk to it and hear the answer
ajustes2-g-asistente-voz = Suara asisten
ajustes2-asistente-revisar-voz = Tinjau ucapan Anda sebelum mengirim
ajustes2-ayuda-asistente-revisar-voz = Dengan mikrofon obrolan «Asistente»: yang Anda ucapkan ditulis di kotak agar bisa Anda perbaiki dan kirim sendiri. Jika nonaktif, langsung dikirim.
asistente-voz-pasando = Mengubah suara Anda menjadi teks… { $pct } %
asistente-voz-bajando = Mengunduh pengenal ucapan (sekali, ~160 MB)… { $pct } %
asistente-voz-nada = Tidak ada yang terdengar: bicaralah lebih dekat ke mikrofon dan coba lagi.
asistente-voz-sin-whisper = PC ini tidak bisa mengubah suara menjadi teks: ONNX Runtime Windows tidak ada.
asistente-voz-fallo = Suara Anda tidak dapat diubah menjadi teks.
asistente-voz-cancelado = Dibatalkan: tidak ada yang dikirim.
asistente-voz-nada-que-leer = Jawaban ini tidak punya teks untuk dibaca.

# The buttons of the Windows notifications (15-oct).
notif-hecha = Selesai
notif-mas-10 = +10 mnt
notif-abrir = Buka
notif-conservar = Pertahankan
notif-ver-galeria = Buka galeri
notif-captura-se-va = Tangkapan layar ini akan hilang { $cuando }
notif-capturas-se-van = { $n } tangkapan layar akan hilang dalam 24 jam ke depan
notif-capturas-titulo = Tangkapan layar akan kedaluwarsa

# Geometry sheets (exact arc, angle mark, gradient)
herramientas-angulo = Tanda sudut
lienzo-pista-arco = Busur (pada panduan: telusuri oval acuan; atau dengan pusat dan dua titik, atau dengan tiga titik, di panel)
lienzo-pista-angulo = Sudut (W): klik di dalam sudut, dekat tempat dua garis bertemu; menaruh α, β, θ… tanpa mengulang (↑ ↓ atau Alt+gulir mengganti huruf; W lagi: sudut 90° dengan kotak kecilnya)
lienzo-panel-modo-angulo = Huruf sudut
lienzo-panel-recientes = Terbaru
lienzo-panel-personalizado = Kustom
lienzo-angulo-modo-griega = α β θ
lienzo-angulo-modo-simple = Tanpa huruf
lienzo-angulo-modo-recto = 90° ∟
lienzo-panel-degradado = Isian gradien
lienzo-panel-cierre-arco = Penutup busur
lienzo-panel-modo-arco = Gambar busur
lienzo-panel-arcos-angulo = Busur
lienzo-degradado-liso = Datar
lienzo-degradado-lineal = Linear
lienzo-degradado-radial = Radial
lienzo-cierre-abierto = Terbuka
lienzo-cierre-sector = Juring
lienzo-cierre-segmento = Tembereng
lienzo-arco-modo-guia = Panduan
lienzo-arco-modo-centro = Pusat
lienzo-arco-modo-tres = 3 titik

## Tasks: single-task pin and class timetable (17-oct-2026)
tareas-fijar-pin = 📌 Pin sebagai pengingat
pin-tarea-nota = 💬 Mengapa tidak sekarang?
pin-tarea-pista = Mengapa tidak sekarang · Enter menyimpan
# Tasks: urgency and day/time picker (18-oct-2026)
tareas-urgente = Mendesak
tareas-muy-urgente = Sangat mendesak
tareas-selector-pista = Pilih hari dan waktu pengingat
tareas-selector-sin = Tanpa pengingat
tareas-selector-quitar = Hapus
tareas-selector-hoy = Hari ini
tareas-selector-manana = Besok
tareas-dia-0 = Sen
tareas-dia-1 = Sel
tareas-dia-2 = Rab
tareas-dia-3 = Kam
tareas-dia-4 = Jum
tareas-dia-5 = Sab
tareas-dia-6 = Min
bandeja-horario = 🎓 Jadwal kelas…
bandeja-horario-grabar = 🎙 Catatan suara untuk kelas berikutnya
horario-sin-clases = Belum ada kelas di jadwal: buka “Jadwal kelas…” dan tulis satu per baris.
horario-guardada = Anda akan mendengarnya di kelas { $clase } berikutnya

## Weekly timetable window and subjects (18-oct-2026)
horario-subtitulo = { $asignaturas } mata pelajaran · { $clases } kelas
horario-nueva = Mata pelajaran baru (@ruang opsional)
horario-domingo = Minggu
horario-como-texto = Sebagai teks
horario-vacio = Ketik mata pelajaran di atas dan seret ke minggu.
horario-tareas-de = Tugas { $asignatura }
horario-sin-tareas = Tidak ada tugas tertunda. Di Tugas, pilih mata pelajaran saat menambahkannya.
horario-abrir-tareas = Buka di Tugas
horario-aula = Ruang
horario-nombre = Nama
horario-nota = Catatan suara (yang dibacakan panggilan)
horario-avisar = Ingatkan (menit sebelumnya)
horario-avisar-pista = { $min } (sesuai jadwal)
horario-guardar = Simpan
horario-cancelar = Batal
horario-borrar = Hapus
horario-deshacer = Urungkan
horario-borrada = Kelas dihapus
horario-asignatura-borrada = “{ $asignatura }” dihapus dari jadwal (tugasnya tetap di Tugas)
horario-repetida = “{ $asignatura }” sudah ada di daftar
horario-arrastra = Seret “{ $asignatura }” ke minggu
horario-no-guardado = Jadwal tidak dapat disimpan
tareas-destino-pista = Daftar atau mata pelajaran tugas
tareas-sin-asignatura = Tanpa mata pelajaran (ke Kotak masuk)
tareas-apuntar-en-asignatura = Tugas { $asignatura } baru…

# Viewers: the frame to print (2026-10-19)
marco-imprimir-pista = Seret kotak di atas yang ingin dicetak · Enter: semuanya · Esc: keluar
marcos-pista = Seret untuk membingkai · angka: buka bingkai itu · Ctrl+P: cetak semua · Del: hapus yang terakhir · I atau Esc: keluar
marcos-imprimir = Cetak bingkai (Ctrl+P)
marcos-borrar = Hapus bingkai

# Right-click on the rail's frame button (27-oct-2026): what to print
marcos-menu-los-marcos = Cetak bingkai ({ $n })
marcos-menu-sin-marcos = Cetak bingkai (belum ada: tekan bingkai lalu seret)
marcos-menu-entero-pdf = Cetak seluruh PDF
marcos-menu-entero-word = Cetak seluruh dokumen Word
marcos-menu-entero-powerpoint = Cetak seluruh PowerPoint
marcos-menu-entero-hoja = Cetak seluruh lembar
marcos-menu-entero-pagina = Cetak seluruh halaman
marcos-menu-entero-documento = Cetak seluruh dokumen
marcos-menu-entero-plano = Cetak seluruh gambar teknik
marcos-menu-entero-modelo = Cetak tampilan model
marcos-menu-no-imprime = Seluruh dokumen tidak dapat dicetak

riel-marcador = Penanda (Ctrl+M)
riel-anotar = Anotasi (A)
riel-marcos = Bingkai cetak (I)
barra-mover = Pindahkan (seret)
barra-oscuro = Mode malam
barra-pantalla = Layar penuh (F11)
barra-fijar = Selalu di atas
barra-cerrar = Tutup (Esc)
visor-letra = Huruf dan spasi (T / B)
lector-mas = Opsi lain (G)
visor-indice-pista = Daftar isi (C)
riel-editar = Edit (Alt+R)
riel-mirar = Kembali melihat (Alt+R atau Esc)

# Pin rail: the frame button of a pin that is not a photo (19-oct-2026).
riel-imprimir-zona = Cetak area (Ctrl+P)

# The spreadsheet viewer bar: name and key of each button (19-oct-2026).
hoja-pista-editar = Edit sel (F2)
hoja-pista-ver = Kembali melihat (Esc)
hoja-pista-negrita = Tebal (Ctrl+B)
hoja-pista-cursiva = Miring (Ctrl+I)
hoja-pista-subrayado = Garis bawah (Ctrl+U)
hoja-pista-tachado = Coret (Ctrl+5)
hoja-pista-resaltar = Warna isian
hoja-pista-letra = Warna huruf
hoja-pista-alinear = Ratakan (Ctrl+Shift+L / E / R)
hoja-pista-numero = Format angka
hoja-pista-ajustar = Bungkus teks
hoja-pista-borrar-formato = Hapus format
## Tooltips of the web viewers' and Office pin bars (19-oct)
web-recargar = Muat ulang (F5)
web-exportar = Ekspor (Ctrl+E)
web-navegador = Buka di browser
nota-pista-ver = Tampilan baca
nota-pista-editar = Edit
nota-pista-buscar = Cari (Ctrl+F)
nota-pista-comentarios = Komentar
nota-pista-compartir = Bagikan
office-pista-abrir-con = Buka dengan…
office-pista-presentar = Presentasi

## Page folders in the chat (26-oct-2026): a named notebook
carpeta-nueva = Folder halaman
carpeta-nombre-de-serie = Buku catatan
carpeta-pagina = Halaman
carpeta-paginas = { $n ->
   *[other] { $n } halaman
}
carpeta-vacia-detalle = Kosong
carpeta-anadir-a = Tambahkan ke folder…
carpeta-nueva-con = Folder baru dengan halaman ini
carpeta-anadida = Ditambahkan ke “{ $carpeta }” sebagai “{ $titulo }”
carpeta-ya-estaba = Sudah ada di “{ $carpeta }”
carpeta-abrir = Buka folder
carpeta-nueva-pagina = Halaman baru
carpeta-compartir = Bagikan
carpeta-exportar-pdf = Ekspor PDF
carpeta-exportando = Mengekspor PDF…
carpeta-exportado = PDF disimpan: { $nombre }
carpeta-no-se-exporto = Tidak dapat mengekspor: tidak ada halaman yang berisi
carpeta-vacia = Folder ini kosong. Tekan “Halaman baru”, atau di obrolan pilih “Tambahkan ke folder…” di menu kanvas atau gambar.
carpeta-no-esta = Folder itu sudah tidak ada di obrolan
carpeta-pista = Seret halaman untuk memindahkannya · F2 mengubah judulnya · Del menghapusnya (muncul lagi di obrolan)
carpeta-escribe-titulo = Ketik judul lalu tekan Enter
carpeta-grabacion = Rekaman
carpeta-grabacion-detalle = { $duracion } · { $capturas ->
   *[other] { $capturas } tangkapan
}
carpeta-estudiar = Pelajari
carpeta-adjunto-insignia = Lampiran · bukan bagian dari rekaman
carpeta-adjuntar = Lampirkan ke kelas
carpeta-adjuntado = Dilampirkan ke kelas: { $nombre }
carpeta-grabacion-pista = Klik item untuk mempelajarinya di menitnya · “Lampirkan ke kelas” menambahkan file, foto, atau catatan tanpa mencampurnya dengan yang direkam

## The assistant reads the web (Oct 28)
asistente-web-buscando = 🌐 mencari di web…
asistente-web-leyendo = 🌐 membaca { $dominio }…
asistente-web-leyendo-pagina = 🌐 membaca halaman…
## TypeSafe Jev (28-oct-2026, docs/jev.md)
ajustes2-g-jev = TypeSafe (Jev)
ajustes2-jev-proveedor = TypeSafe (Jev)
ajustes2-jev-probada = berfungsi ({ $modelo })
ajustes2-jev-reunion = Yang penting di kelas dan rapat
ajustes2-ayuda-jev-reunion = Saat rekaman ditranskripsi, menandai tugas, keputusan, pertanyaan, dan apa yang akan keluar di ujian. Ditampilkan di “Pelajari kelas” dan di halaman bersama. Setiap baris dikirim bersama baris di sekitarnya dan judulnya.
ajustes2-jev-anki = Hanya kartu Anki yang layak dipelajari
ajustes2-ayuda-jev-anki = Saat kelas dikirim ke Anki, mempertahankan tangkapan yang bagiannya menjelaskan sesuatu untuk dipelajari dan membuang obrolan ringan. Yang diucapkan di setiap bagian dikirim.
ajustes2-jev-urgencia = Prioritas tugas dari konteks
ajustes2-ayuda-jev-urgencia = Mendesak, tinggi, normal, atau rendah berdasarkan akibat jika terlambat, siapa yang menunggu, waktunya, dan pesan obrolan terbaru. Tanda Anda (⏫, 🔼) yang menang. Teks tugas, daftarnya, dan pesan terbaru di obrolannya dikirim.
ajustes2-jev-modelo = Pilih model untuk setiap permintaan
ajustes2-ayuda-jev-modelo = Permintaan mudah ke model cepat (Haiku, DeepSeek Flash) dan yang perlu penalaran ke model kuat (Sonnet, DeepSeek V4 Pro). Permintaan dan tiga pesan terakhir dikirim.
clase-importante-todo = Semua
clase-importante-tarea = Tugas
clase-importante-decision = Keputusan
clase-importante-pregunta = Pertanyaan
clase-importante-examen = Ujian
clase-importante-buscando = Mencari yang penting…
clase-importante-tareas = + { $n } tugas ke Tugas
clase-importante-anadidas = { $n } tugas ditambahkan ke kotak masuk Tugas
tareas-prioridad-urgente = Mendesak
tareas-prioridad-alta = Tinggi
tareas-prioridad-baja = Rendah
tareas-prioridad-normal = Normal
tareas-jev-consecuencia-nada = tanpa akibat
tareas-jev-consecuencia-leve = akibat ringan
tareas-jev-consecuencia-media = akibat sedang
tareas-jev-consecuencia-grave = akibat serius
tareas-jev-la-esperan = ada yang menunggu
tareas-jev-nadie-espera = tidak ada yang menunggu
tareas-jev-plazo-ajeno = tenggat dari orang lain
tareas-jev-la-piden = diminta di obrolan
tareas-jev-sin-fecha = tanpa tanggal
tareas-jev-ya-paso = sudah lewat
tareas-jev-hoy = hari ini
tareas-jev-manana = besok
tareas-jev-en-dias = dalam { $n } hari
tareas-jev-por-la-lista = { $cuando } (dari judul daftar)
anki-jev-quitadas = Jev melewatkan { $n } tangkapan yang tidak berisi bahan belajar
## Meeting topics (28-oct-2026, docs/jev.md)
ajustes2-jev-temas = Topik rapat yang dibahas
ajustes2-ayuda-jev-temas = Saat Anda merekam rapat dengan topik, yang terdengar ditranskripsi di komputer ini setiap 15 dtk dan Jev mencentang topik yang sudah dibahas. Centang Anda yang menang. Teks setiap bagian, bagian sebelumnya, topik, dan judul dikirim.
ajustes2-jev-dictado = Dikte ke obrolan
ajustes2-ayuda-jev-dictado = Saat Anda mendikte di kotak obrolan (klik kanan mikrofon), "catat di obrolan proyek bahwa sekrup kita habis" menawarkan untuk mencatatnya di obrolan itu: Enter mencatat, Escape mengabaikan. Setiap kalimat yang didiktekan, kalimat sebelumnya, dan nama obrolan terbaru Anda dikirim.
ajustes2-jev-repaso = Ulas dengan suara
ajustes2-ayuda-jev-repaso = Di "Pelajari kelas", jelaskan kelas dengan suara dan Jev menandai poin penting mana yang Anda bahas, mana yang salah, dan mana yang terlewat. Penjelasan Anda, judul, dan poin penting dikirim.
clase-temas-ninguno = Topik rapat (opsional)
clase-temas-n = Topik rapat · { $n }
clase-temas-titulo = Topik rapat
clase-temas-sub = Satu per baris. Saat Anda merekam, Jev mencentang yang sudah dibahas.
clase-temas-escribe = Ketik topik lalu tekan Enter…
clase-temas-pegar = Tempel
clase-temas-de-lista = Dari daftar
clase-temas-lista = Daftar “{ $lista }”: { $n } topik
clase-temas-sin-listas = Tidak ada daftar tugas yang berisi item tertunda
clase-temas-pegados = { $n } topik ditambahkan
clase-temas-nada-que-pegar = Tidak ada teks di papan klip
clase-temas-vaciar = Bersihkan
clase-temas-listo = Selesai
clase-temas-anteriores = { $n } topik tersisa dari rapat sebelumnya
clase-temas-tratado = ✓ Topik { $n } dibahas · { $minuto }
clase-temas-escuchando = Mendengarkan: Jev mencentang topik saat dibahas
clase-temas-sin-jev = Tanpa Jev (kunci TypeSafe tidak ada): centang topik secara manual
clase-temas-apagado = Topik dengan Jev dinonaktifkan di Pengaturan: centang secara manual
clase-temas-sin-modelo = Tanpa model ucapan tidak ada pendengaran langsung: centang topik secara manual
clase-temas-jev-fallo = Jev tidak menjawab: centang topik secara manual
clase-temas-cargado = Komputer sibuk: lebih jarang mendengarkan agar rekaman tidak melambat
clase-temas-sin-tratar = Belum dibahas
clase-temas-proxima = Pindahkan ke rapat berikutnya
clase-temas-a-tareas = Tambahkan ke Tugas
clase-temas-pasados = { $n } topik menunggu rapat berikutnya
clase-temas-anadidos = { $n } topik ditambahkan ke kotak masuk Tugas
clase-temas-lista-de = Topik · { $nombre }
## What is heard in the meeting (29-oct-2026, docs/jev.md, «En la reunión»)
ajustes2-jev-tareas-dichas = Tugas yang disebut di rapat
ajustes2-ayuda-jev-tareas-dichas = Saat Anda merekam rapat, “saya kirim penawarannya hari Jumat” muncul di bilah sebagai usulan tugas, dengan tanggal dan urgensinya. Hanya masuk ke Tugas jika Anda menerimanya. Setiap kalimat dikirim bersama dua kalimat sebelumnya dan judulnya.
ajustes2-jev-urgencia-en-vivo = Urgensi tugas yang disebut di rapat
ajustes2-ayuda-jev-urgencia-en-vivo = “Ini untuk hari ini” menaikkan prioritas atau mengubah tanggal tugas tertunda dari obrolan ini atau Kotak masuk. Diterapkan otomatis dan bisa diurungkan dari bilah. Setiap kalimat dikirim bersama teks hingga 8 tugas.
ajustes2-jev-tema-olvidado = Peringatkan topik yang terlupa
ajustes2-ayuda-jev-tema-olvidado = Dengan “Durasi rencana” di Persiapan, memperingatkan menjelang akhir (atau saat orang mulai menutup) tentang topik agenda yang belum dibahas. Setiap kalimat dan topiknya dikirim.
ajustes2-jev-tiempo-por-tema = Menit per topik
ajustes2-ayuda-jev-tiempo-por-tema = Berapa lama setiap topik agenda dibahas, di “Pelajari kelas” dan di halaman bersama. Setiap kalimat dan topiknya dikirim.
ajustes2-jev-decisiones = Keputusan rapat
ajustes2-ayuda-jev-decisiones = “Sudah diputuskan: pesanannya kita majukan” dicatat dengan menitnya (bukan “kita bisa…”). Ditampilkan di “Pelajari kelas” dan di halaman bersama. Setiap kalimat dikirim.
ajustes2-jev-preguntas-pendientes = Pertanyaan tak terjawab
ajustes2-ayuda-jev-preguntas-pendientes = Pertanyaan yang tidak dijawab siapa pun di rapat, di bagian akhir “Pelajari kelas”, untuk dipindahkan ke Tugas. Setiap kalimat dan pertanyaan yang masih terbuka dikirim.
ajustes2-jev-fechas-al-horario = Tanggal yang disebut, ke jadwal
ajustes2-ayuda-jev-fechas-al-horario = “Sampai jumpa Selasa jam 3” muncul di bilah sebagai usulan acara; jika Anda menerimanya, masuk ke Tugas dengan waktunya dan tampil di Jadwal. Setiap kalimat dikirim.
clase-vivo-tarea = Tugas
clase-vivo-evento = Acara
clase-vivo-deshacer = Urungkan
clase-vivo-decision = Keputusan dicatat · { $minuto }
clase-vivo-tarea-apuntada = Tugas ditambahkan ke kotak masuk Tugas
clase-vivo-evento-apuntado = Acara ditambahkan ke Tugas (tampil di Jadwal)
clase-vivo-deshecho = Diurungkan: tugas kembali seperti semula
clase-vivo-no-se-apunto = Tidak dapat menambahkan: periksa Tugas
clase-vivo-falla = Jev tidak menjawab: untuk saat ini tidak ada usulan
clase-vivo-queda-poco = Tersisa { $minutos } mnt dan masih belum: { $temas }
clase-vivo-cumplido = Waktu habis dan masih belum: { $temas }
clase-vivo-cierran = Orang mulai menutup dan masih belum: { $temas }
clase-vivo-fecha-nueva = sekarang jatuh tempo { $fecha }
clase-vivo-prioridad-normal = Normal
clase-vivo-duracion = Durasi rencana
clase-vivo-duracion-n = ⏱ { $minutos } mnt
clase-acta-decisiones = Keputusan
clase-acta-preguntas = Pertanyaan tak terjawab
clase-acta-propuestas = Usulan yang tidak diterima
clase-acta-aceptar = Terima
clase-acta-aceptada = Ditambahkan ke Tugas
clase-acta-anadidas = { $n } pertanyaan ditambahkan ke kotak masuk Tugas

# --- Lo nuevo de noviembre ---
aviso-ia-abrir = Buka
aviso-ia-si = Ya
aviso-ia-no = Tidak
aviso-ia-permiso = 🔐 Saya perlu izin Anda: { $pregunta }

    Jika Anda tidak menjawab, saya tidak akan melakukannya.
ia-fondo-titulo = 🤖 Asisten
ia-fondo-sigue = «lanjutkan»
ia-fondo-sigue-fuera = «@asistente lanjutkan»
ia-fondo-otra-vez = ↻ PinDesk tertutup sebelum saya selesai. Saya mulai lagi dari awal di balasan di bawah.
ia-fondo-ya-actuo = Beberapa langkah sudah dilakukan, jadi saya tidak akan mengulanginya sendiri (nanti terjadi dua kali). Ketik { $sigue } dan saya lanjutkan dari sini.
ia-fondo-ya-se-retomo = Saya sudah melanjutkannya sekali dan terputus lagi, jadi saya berhenti di situ. Ketik { $sigue } jika Anda ingin saya mencoba lagi.
ia-fondo-viejo = Sudah terlalu lama untuk melanjutkannya sendiri. Ketik { $sigue } jika Anda masih menginginkannya.
ia-fondo-sin-pregunta = Saya tidak menemukan permintaan Anda (apakah sudah dihapus?), jadi saya tidak bisa melanjutkannya.
ia-fondo-no-se-pudo = Saya tidak bisa melanjutkannya: { $aviso }
ia-fondo-no-empezo = saya tidak bisa mulai menjawab.
ia-fondo-cortado = ⚠️ PinDesk tertutup sebelum ini selesai. { $porque }
ia-fondo-sigo-en-el-chat = 💬 Saya lanjutkan di obrolan.
ia-fondo-sigo-de-fondo = ⏳ Saya masih mengerjakan permintaan Anda meskipun obrolan sudah ditutup. Saya kabari setelah selesai.
ia-fondo-pregunta = ❓ Saya perlu jawaban Anda di obrolan: { $pregunta }

    Tekan «Buka». Jika Anda tidak menjawab, saya pilih opsi yang paling aman.
leccion-ventana = Pelajaran
leccion-ventana-nueva = Pelajaran baru
leccion-titulo = 💡 Pelajaran
leccion-titulo-nueva = 💡 Pelajaran baru
leccion-tipo-leccion = 💡 Pelajaran
leccion-tipo-error = ⚠️ Kesalahan
leccion-tipo-acierto = ✅ Keberhasilan
leccion-gravedad-leve = Ringan
leccion-gravedad-importante = Penting
leccion-gravedad-grave = Serius
leccion-sin-proyecto = Tanpa proyek (obrolan umum)
leccion-foto-ilegible = Foto itu tidak dapat dibaca
leccion-adjunto-no-esta = File itu belum ada di komputer ini
leccion-galeria = 🖼 Galeri
leccion-guardar = Simpan
leccion-pista-titulo = Apa yang Anda pelajari? Ketik atau diktekan sekaligus
leccion-se-parece = Mirip dengan yang sudah Anda punya
leccion-paso-otra-vez = 🔁 Terjadi lagi
leccion-area = Bidang
leccion-etiquetas = Label
leccion-anadir-etiqueta = + label
leccion-menos-detalle = ▴  Lebih sedikit detail
leccion-mas-detalle = ▾  Apa yang terjadi, mengapa, dan apa yang akan saya lakukan berbeda
leccion-que-paso = Apa yang terjadi
leccion-por-que = Mengapa terjadi
leccion-causas = Penyebab (sekali ketuk)
leccion-proxima = Lain kali… (jika X terjadi, saya akan melakukan Y)
leccion-referencias = Kata untuk menemukannya (pisahkan dengan koma)
leccion-relacionadas = Terkait
leccion-volvio-a-pasar = 🔁 Terjadi lagi pada saya
leccion-volvio-a-pasar-n = 🔁 Terjadi lagi pada saya (sudah { $veces } kali)
leccion-mic-escuchando = 🎙️ Saya mendengarkan… tekan mikrofon untuk selesai
leccion-mic-sin-dictado = Tidak ada dikte di komputer ini: ketik pelajarannya
leccion-mic-sin-texto = Tidak ada yang bisa dipahami
leccion-mic-no-pasa = Tidak dapat diubah menjadi teks
leccion-mic-corto = Terlalu pendek: biarkan mikrofon menyala sedikit lebih lama
leccion-audio = 🎙 Audio
leccion-audio-abriendo = 🎙 Membuka…
leccion-audio-guardando = 🎙 Menyimpan…
leccion-audio-parar = ⏹ Hentikan { $duracion }
leccion-audio-sin-microfono = Mikrofon tidak dapat digunakan
leccion-audio-corto = Terlalu pendek: tidak disimpan
leccion-audio-no-guarda = Audio tidak dapat disimpan
horario-evaluacion-en = { $dias ->
        [1] besok
       *[other] dalam { $dias } hari
    }
horario-evaluacion-recuerda = Ingat: { $que }, { $en }, { $cuando }
horario-evento-en = { $evento } di { $lugar }
horario-dia-0 = Sen
horario-dia-1 = Sel
horario-dia-2 = Rab
horario-dia-3 = Kam
horario-dia-4 = Jum
horario-dia-5 = Sab
horario-dia-6 = Min
lista-icono-capturar = Tangkap
lista-icono-capturar-largo = Tangkap area layar
lista-icono-mensajes = Pesan tersimpan
lista-icono-mensajes-largo = Buka Pesan tersimpan
lista-icono-proyectos = Proyek
lista-icono-proyectos-largo = Lihat proyek Anda
lista-icono-sincronizar = Sinkronkan
lista-icono-sincronizar-largo = Sinkronkan dengan perangkat Anda
lista-icono-recientes = Proyek terbaru
lista-icono-proyecto-largo = Buka proyek { $nombre }
cad-plano-abriendo = Membuka gambar…
cad-plano-vacio = Gambar ini tidak berisi apa pun untuk ditampilkan
cad-plano-error = Gambar tidak dapat dibuka
cad-plano-pista-acotar = Klik titik untuk mengukur · Klik ganda atau Enter: selesai · Esc: keluar
cad-plano-todo = Tampilkan seluruh gambar
cad-tema = Latar terang atau gelap
cad-acotar = Ukur
cad-borrar-cotas = Hapus ukuran
cad-encima = Selalu di atas
cad-cerrar = Tutup
cad-tres = Lihat dalam 3D
cad-marcos = Bingkai untuk dicetak
cad-imprimir = Cetak…
cad-exportar = Ekspor (HTML, PDF, PNG, DXF…)…
cad-borrar-marcos = Hapus bingkai
cad-plano-pista-marcos = Seret untuk membingkai yang dicetak · Ctrl+P: cetak · Del: hapus yang terakhir · Esc: keluar
cad-plano-marcador = Penanda di sini
cad-anotar = Anotasi
cad-sin-dibujo-civil = { $n } objek Civil 3D disimpan tanpa grafiknya: simpan dengan PROXYGRAPHICS = 1 atau ekspor ke LandXML
cad-letras = Font gambar…
cad-letras-faltan = Gambar ini meminta font yang tidak ada di sini; teksnya memakai font serupa dan mungkin tidak muat di kotaknya:
cad-letras-no-falta = Tidak ada font yang disebut namanya oleh gambar ini yang kurang.
cad-letras-donde = Font SHX biasanya ada di C:\Program Files\Autodesk\AutoCAD 20xx\Fonts atau di samping gambar yang dikirim kepada Anda;
    font TrueType, di C:\Windows\Fonts. Font yang Anda tambahkan di sini lebih diutamakan daripada font AutoCAD, Windows, dan PinDesk.
cad-letras-anadir = Tambahkan font (SHX, TTF, OTF, TTC)…
cad-letras-quitar = Hapus font saya ({ $n })
cad-faltan-letras = { $n } font gambar tidak ada: klik Aa di atas untuk menambahkannya
cad-releyendo = Membaca gambar dengan font Anda…
cad-boton-mover = Pindahkan (seret)
cad-boton-marcador = Penanda
cad-boton-anotar = Anotasi
cad-boton-letras = Font gambar
cad-corte = Kotak potongan
cad-capas = Level dan kategori
cad-boton-pantalla = Layar penuh
cad-boton-guardar = Simpan ke obrolan
cad-modelo-abriendo = Membuka model…
cad-modelo-vacio = Model ini tidak berisi apa pun untuk ditampilkan
cad-modelo-error = Model tidak dapat dibuka
cad-modelo-todo = Tampilkan seluruh model
cad-aislar = Isolasi yang dipilih
cad-niveles = LEVEL
cad-categorias = KATEGORI
cad-sin-nivel = Tanpa level
cad-mostrar-todo = Tampilkan semua
cad-aristas = Tepi
cad-planta = Tampak denah
cad-altura-corte = Potongan
cad-modelo-pista = Seret: putar · Klik kanan: geser · Gulir: zoom · Klik: lihat apa itu · Klik lagi: isolasi
cad-modelo-marcador = Penanda di sini (dengan potongan dan yang disembunyikan)
cad-imprimir-marcos = Cetak bingkai…
cad-modelo-pista-marcos = Seret untuk membingkai yang dicetak · Ctrl+P: cetak · Del: hapus yang terakhir · Angka: ke tampilannya · Esc: keluar
cad-modelo-pista-anotar = Anotasi Anda hanya terlihat di tampilan penanda ini
cad-modelo-anotar = Anotasi pada tampilan ini
cad-croquis-abriendo = Membuka sketsa…
cad-croquis-vacio = Sketsa kosong
cad-croquis-error = Sketsa tidak dapat dibuka
cad-croquis-todo = Tampilkan seluruh sketsa
cad-croquis-capas = Lapisan
cad-croquis-grupos = GRUP
cad-croquis-tipos = JENIS
cad-croquis-todo-corto = Semua
cad-croquis-pista = Seret: putar · Klik kanan: geser · Gulir: zoom · 1–9: tampilan tersimpan
cad-web-todo = Tampilkan semua
cad-web-tema = Terang / gelap
cad-web-capas = Lapisan
cad-web-grosores = Ketebalan garis
cad-web-anotaciones = Anotasi
cad-web-todas = Semua
cad-web-ninguna = Tidak ada
cad-web-sin-soporte = Browser ini tidak bisa membuka kompresi gambar: buka dengan Edge, Chrome, Firefox, atau Safari versi terbaru.
cad-web-pista = Gulir atau dua jari: geser · Ctrl+gulir atau cubit: zoom · Klik ganda: tampilkan semua · F, B, L
lamina-proyecto = PROYEK
lamina-lamina = LEMBAR
lamina-titulo = JUDUL
lamina-dibujo = DIGAMBAR OLEH
lamina-escala = SKALA
lamina-sin-escala = Tanpa skala
lamina-fecha = TANGGAL
lamina-unidades = SATUAN
lamina-opcion = Lembar
lamina-con-membrete = Dengan bingkai dan kop gambar
lamina-sin-marco = Tanpa bingkai
sinc-fondo-solo = PC ini tidak punya siapa pun lagi di grup: bergabunglah ke grup di Sinkronkan
sinc-fondo-no-es-del-grupo = Perangkat itu tidak lagi ada di grup
sinc-fondo-aparatos = { $n } perangkat
sinc-fondo-sincronizando = Menyinkronkan dengan { $quien }…
sinc-fondo-ya-en-marcha = Sinkronisasi sudah berjalan: tunggu sampai selesai
sinc-fondo-fallo = Tidak dapat menyinkronkan: { $error }
lector-voz-en-linea = online
auto-origen = Otomatisasi
ia-nube-falta-la-clave = Kunci { $proveedor } belum ada: tempelkan di Pengaturan → Asisten (dapatkan di { $pagina }).
figuras-ventana-no-es-numero = «{ $texto }» bukan angka
figuras-ventana-mes-mal = «{ $texto }»: tulis bulan seperti 2026-11
tecla-intro = Enter
tecla-supr = Del
ia-fondo-hecho = ✅ Selesai.
ia-hecho = Selesai.
croquis-titulo = Sketsa { $id }
chat-ajustes-bandeja = Pengaturan dibuka dari ikon PinDesk di baki sistem
ia-estado-funciona = berfungsi
ia-estado-sin-clave = tanpa kunci
ia-estado-desconocido = belum diperiksa
ia-estado-clave-mala = kunci tidak valid ({ $codigo })
ia-estado-sin-saldo = akun tidak punya saldo
ia-estado-limite = terlalu banyak permintaan atau kuota habis
ia-estado-red = tidak ada koneksi
ia-estado-servidor = layanan sedang gagal ({ $codigo })
ia-estado-otro = tidak menjawab dengan benar ({ $codigo })
ia-estado-comprobando = memeriksa…
ia-estado-hace-nada = baru saja
ia-estado-hace-min = { $n } mnt lalu
ia-estado-hace-horas = { $n } jam lalu
ia-estado-pista = Klik: periksa sekarang atau buka kunci
ia-estado-comprobar = Periksa sekarang
ia-estado-claves = Kunci di Pengaturan → Asisten
ia-estado-sin-ajustes = Pengaturan dibuka dari ikon PinDesk di baki sistem
chat-mes-dia-1 = Januari
chat-mes-dia-2 = Februari
chat-mes-dia-3 = Maret
chat-mes-dia-4 = April
chat-mes-dia-5 = Mei
chat-mes-dia-6 = Juni
chat-mes-dia-7 = Juli
chat-mes-dia-8 = Agustus
chat-mes-dia-9 = September
chat-mes-dia-10 = Oktober
chat-mes-dia-11 = November
chat-mes-dia-12 = Desember
ajustes2-voz-en-vivo = Dengarkan rapat secara langsung dengan
ajustes2-voz-en-vivo-sistema = Windows atau Whisper
ajustes2-voz-en-vivo-vosk = Vosk
ajustes2-ayuda-voz-en-vivo = Untuk topik dan bagian lain rapat dengan Jev. Vosk menampilkan apa yang sedang diucapkan dan menutup setiap kalimat sepersekian detik setelah Anda diam (dengan Whisper, sekitar 3 dtk), tanpa jaringan. Memilihnya mengunduh { $megas } MB (Apache-2.0).
ajustes2-ayuda-voz-en-vivo-vosk-listo = Mendengarkan dengan Vosk, tanpa jaringan: apa yang sedang diucapkan muncul di layar penuh rekaman. Jika suatu saat gagal dimuat, Windows atau Whisper yang dipakai.
ajustes2-ayuda-voz-en-vivo-sin-idioma = Vosk diunduh untuk bahasa Spanyol dan Inggris; dalam bahasa ini, Windows atau Whisper yang mendengarkan.
ajustes2-voz-en-vivo-bajando = Mengunduh Vosk… { $por-ciento } %. Sementara itu Windows atau Whisper yang mendengarkan.
ajustes2-voz-en-vivo-fallo = Vosk tidak dapat diunduh: { $fallo }. Pilih lagi untuk mencoba ulang.
ajustes2-voz-vosk-borrar = Hapus Vosk
ajustes2-ayuda-voz-vosk-borrar = Mengosongkan yang sudah diunduh (sekitar 100 MB di disk). Diunduh lagi saat Anda memilihnya.
clase-temas-bateria = Baterai lemah: tidak mendengarkan langsung agar hemat (rekaman tetap berjalan)
chat-anotar-fallo-red = Tidak ada koneksi: Jev tidak dapat memeriksa apakah perlu dicatat di obrolan lain. Yang Anda diktekan masih ada di kotak.
chat-anotar-fallo = Jev tidak menjawab ({ $motivo }). Yang Anda diktekan masih ada di kotak.
ajustes2-g-privacidad = Privasi dan data
ajustes2-priv-politica = Kebijakan privasi
ajustes2-ayuda-priv-politica = Apa yang dilakukan PinDesk dengan data Anda (dalam bahasa Spanyol dan Inggris): semuanya tetap di PC ini kecuali yang tercantum di bawah, dan hanya saat Anda menggunakannya.
ajustes2-g-privacidad-sale = Apa yang keluar dari PC ini dan ke mana
ajustes2-priv-ver-ajustes = Lihat pengaturannya
ajustes2-priv-nada-mas = Tidak ada lagi
ajustes2-ayuda-priv-nada-mas = Tanpa iklan, tanpa analitik, tanpa laporan kerusakan, tanpa akun, dan tanpa server PinDesk. Dikte dan transkripsi berjalan di PC ini, tanpa jaringan.
ajustes2-g-privacidad-datos = Data Anda
ajustes2-priv-carpeta = Data Anda, di disk Anda
ajustes2-ayuda-priv-carpeta = Tangkapan, obrolan, catatan, rekaman, dan pengaturan tersimpan di { $carpeta }. Untuk membawanya, salin foldernya; untuk menghapusnya, tutup PinDesk lalu hapus folder itu. Kunci AI dienkripsi dengan pengguna Windows Anda.
ajustes2-priv-reportes = Laporan tentang tulisan AI
ajustes2-ayuda-priv-reportes = { $n } tersimpan di reportes-ia.jsonl (di folder data).
privacidad-asistente-que = Yang Anda tanyakan kepada asisten, bagian terbaru obrolan, dan yang Anda lampirkan
privacidad-asistente-adonde = Anthropic (Claude) atau DeepSeek
privacidad-asistente-cuando = Hanya jika Anda menempelkan kunci Anda sendiri dan bertanya. Kunci disimpan terenkripsi di PC ini.
privacidad-jev-que = Teks untuk setiap fitur Jev: kalimat rapat, baris kelas, tugas, yang Anda diktekan atau jelaskan
privacidad-jev-adonde = TypeSafe (api.typesafe.ai)
privacidad-jev-cuando = Hanya dengan kunci TypeSafe Anda dan fitur itu dinyalakan. Tidak pernah audionya.
privacidad-web-que = Yang dicari asisten dan halaman yang dibacanya untuk menjawab Anda
privacidad-web-adonde = DuckDuckGo atau Brave Search, dan situs yang dibacanya
privacidad-web-cuando = Saat asisten mencari di web. Tanpa cookie dan tanpa kunci AI Anda.
privacidad-google-que = File Drive yang Anda pilih dan acara kalender
privacidad-google-adonde = Google (Drive, Calendar)
privacidad-google-cuando = Hanya jika Anda menghubungkan akun Google di Pengaturan › Asisten; putuskan dari sana.
privacidad-voces-que = Teks yang dibacakan dengan suara Microsoft
privacidad-voces-adonde = Microsoft (suara Edge)
privacidad-voces-cuando = Hanya jika Anda memilih salah satu suara itu di pembaca; suara Windows berfungsi tanpa jaringan.
privacidad-enlace-que = File yang Anda pilih untuk dibagikan lewat tautan
privacidad-enlace-adonde = litterbox.catbox.moe atau temp.sh
privacidad-enlace-cuando = Hanya saat Anda meminta tautan berbagi; tautan terhapus sendiri dalam hitungan jam atau hari.
privacidad-modelos-que = Tidak ada data Anda: hanya file yang diminta
privacidad-modelos-adonde = huggingface.co, github.com, dan alphacephei.com (Whisper, Qwen3-ASR, Vosk, Indra)
privacidad-modelos-cuando = Saat mengunduh model suara atau pembaca teks, ketika Anda memintanya.
privacidad-sincro-que = Tangkapan, catatan, dan obrolan Anda yang tersinkron
privacidad-sincro-adonde = Perangkat Anda sendiri (ponsel), lewat jaringan Anda
privacidad-sincro-cuando = Saat menyinkronkan: langsung di jaringan Anda, terenkripsi, tanpa melewati server.
reportar-ia = Laporkan respons…
reportar-ia-motivo-ofensivo = Menyinggung
reportar-ia-motivo-peligroso = Berbahaya
reportar-ia-motivo-falso = Salah atau menyesatkan
reportar-ia-motivo-otro = Alasan lain
reportar-ia-hecho = Laporan disimpan di reportes-ia.jsonl; email Anda terbuka untuk mengirimnya
reportar-ia-asunto = Laporan respons AI
reportar-ia-campo-motivo = Alasan
reportar-ia-campo-funcion = Fitur
reportar-ia-campo-fecha = Tanggal
reportar-ia-campo-version = Versi
reportar-ia-campo-contenido = Yang ditulis AI
reportar-ia-comentario = (Jika mau, ceritakan di sini apa yang salah.)
repaso-jev-fallo-red = Tidak ada koneksi.

## Ekspor tugas: halaman, PDF, dan Markdown sebuah daftar (1-11-2026)
tareas-exportar-titulo = Tugas
tareas-exportar-exportado = Diekspor pada { $fecha }
tareas-exportar-pendientes = Belum selesai
tareas-exportar-hechas = Selesai
tareas-exportar-completado = Tuntas
tareas-exportar-pendiente = Belum selesai
tareas-exportar-hecha = Selesai
tareas-exportar-todo-hecho = Semua selesai
tareas-exportar-vacia = Tidak ada tugas
tareas-exportar-recordatorio = Pengingat
tareas-exportar-pasada = Lewat waktu
tareas-exportar-creada = Dibuat { $fecha }
tareas-exportar-de = { $hechas } dari { $total }
tareas-exportar-foto = Foto
compartir-tipo-tareas = Daftar tugas

## La nube de los chats compartidos (1-nov-2026, sincronizar::nube)
chat-nuevo-nube = Obrolan bersama di cloud
chat-nuevo-nube-detalle = Yang kamu unggah, orang lain mengunduhnya di mana pun mereka berada
nube-modo-titulo = Cara sinkron
nube-modo-local = Jaringan lokal
nube-modo-nube = Cloud
nube-modo-local-detalle = Sinkron saat kalian di Wi-Fi yang sama, gratis dan tanpa server.
nube-modo-nube-detalle = Lewat cloud-mu: yang kamu unggah diunduh orang lain di mana pun mereka berada, dan terlihat di web. Hanya perubahan yang diunggah.
nube-falta-configurar = Cloud belum diatur: isi alamat dan kuncimu di Sinkron → Cloud.
nube-titulo = Cloud
nube-portada-como = Obrolan bersama yang disetel ke «Cloud» lewat servermu. Obrolanmu sendiri tetap lewat jaringan lokal.
nube-ajustes = Pengaturan cloud
nube-explica = Servermu menyimpan obrolan bersama yang lewat cloud dan menampilkannya di web. Obrolan pribadi tetap lewat jaringan lokal, tanpa server.
nube-direccion = Alamat web
nube-llave = Kunci perangkat (pxn_…)
nube-llave-guardada = Kunci tersimpan:
nube-olvidar = Lupakan
nube-guardar = Simpan
nube-probar = Uji
nube-guardado = Tersimpan.
nube-direccion-mala = Alamat harus diawali https://.
nube-llave-mala = Kunci diawali pxn_ (diberikan web saat mendaftarkan perangkat ini).
nube-probado = Terhubung: ruang { $espacio }.
nube-no-contesta = Cloud tidak menjawab:
nube-que-va = Hanya perubahan yang diunggah: pada denah beranotasi, goresan baru dan bukan seluruh denah.
nube-en-linea = Daring
nube-sin-conexion = Luring
nube-cerrar-titulo = Perubahan luring
nube-cerrar-pregunta =
    Kamu menganotasi dokumen ini secara luring.
    Ya: buang perubahan itu (milik orang lain tetap).
    Tidak: kembali daring dan unggah.
    Batal: biarkan luring untuk lain kali.

## Comentario avanzado: la pegatina con su minichat (3-nov-2026)
comentario-tipo-urgente = Mendesak
comentario-tipo-alta = Prioritas tinggi
comentario-tipo-media = Prioritas sedang
comentario-tipo-nota = Catatan
comentario-abrir = Buka obrolan
comentario-resolver = Selesaikan
comentario-reabrir = Buka lagi
comentario-crear-tarea = Buat tugas
comentario-tarea-creada = Tugas dibuat di Kotak Masuk
comentario-quitar = Hapus
comentario-lista-titulo = Komentar
comentario-lista-vacia = Belum ada komentar. Pilih jenis lalu klik tempat menempelkannya.
comentario-plantando = Klik tempat untuk menempelkan komentar (Esc untuk keluar)
comentario-escribir = Tulis… (Enter mengirim)
comentario-sin-entradas = Tulis pesan pertama, tempel gambar, atau lampirkan file.
comentario-resuelto = Selesai
comentario-sin-sitio = Dokumen ini belum disimpan: tidak bisa melampirkan apa pun.
comentario-abre-el-chat = Buka obrolan untuk menuju pesan itu.
comentario-elegir-atajo = Pintasan ke…
comentario-sin-atajos = Pilih sesuatu di kanvas, atau buka dokumen dari obrolan, untuk membuat pintasan.
comentario-elemento = Elemen
comentario-mensaje = Pesan obrolan
comentario-lienzo-propio = Kanvas
comentario-anotar = Anotasi
comentario-anotada = Beranotasi
