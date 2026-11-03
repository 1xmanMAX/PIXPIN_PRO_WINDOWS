# PinDesk — 简体中文字符串。

app-nombre = PinDesk


resultado-copiar = 复制
resultado-guardar-como = 另存为…
resultado-guardar = 保存
resultado-descartar = 放弃

error-otra-instancia = PinDesk 已在运行。
error-atajo-ocupado = 无法注册 { $atajo }：另一个应用程序正在使用它。

captura-guardada = 截图已保存到 { $ruta }
captura-fallo = 无法截取屏幕：{ $motivo }

barra-copiar = 复制
barra-guardar = 保存
barra-guardar-como = 另存为…
barra-descartar = 放弃

pin-no-encontrado = 未找到

pin-copiar = 复制
pin-guardar-como = 另存为…
pin-abrir-ubicacion = 打开文件位置
pin-tamano-original = 原始大小
pin-grupo = 分组
pin-sin-grupo = 无分组
pin-color-rojo = 红色
pin-color-naranja = 橙色
pin-color-ambar = 琥珀色
pin-color-verde = 绿色
pin-color-cian = 青色
pin-color-azul = 蓝色
pin-color-violeta = 紫色
pin-color-rosa = 粉色
pin-ocultar-grupo = 隐藏此分组
pin-cerrar = 关闭
pin-eliminar = 从存储中删除…
pin-eliminar-confirmar = 这将从存储中永久删除它。确定吗？
grupos-ocultos = 已隐藏的分组

capa-guardar-titulo = 屏幕标注
capa-guardar-pregunta = 将绘图保留为贴图？
pin-reproducir = 播放
pin-pausar = 暂停
pin-sonido = 声音
barra-todo = 全选
pin-sin-codec = 无视频解码器
comando-capturar-region = 截图
comando-capturar-y-copiar = 截图并复制
comando-capturar-con-scroll = 滚动截图
comando-cuentagotas = 取色器
comando-pinear = 截图并贴图
comando-pinear-en-vivo = 实时贴图（实时查看某个区域）
comando-pinear-portapapeles = 贴出剪贴板
comando-anotar = 标注屏幕
comando-anotar-congelada = 标注截图
comando-abrir-chat = 打开项目聊天
comando-abrir-ajustes = 设置
comando-salir = 退出
comando-alternar-pines = 隐藏或显示贴图
comando-restaurar-ultimo-pin = 重新打开上次关闭的贴图
comando-cerrar-todos-los-pines = 关闭所有贴图
comando-ventana-encima = 将指针下的窗口置顶
comando-copiar-texto = 复制区域中的文字
comando-capturar-y-anotar = 截图并标注
comando-capturar-ultima-region = 重复上次截图
comando-grabar-gif = 将区域录制为 GIF

# The GIF recording bar (P5b).
grabar-empezar = 录制
grabar-parar = 停止
grabar-pausa = 暂停
grabar-seguir = 继续
grabar-cerrar = 关闭
grabar-por-segundo = { $ritmo }/秒
grabar-cuenta-atras = { $segundos }...

# The editor that opens when recording stops (P5b.4).
editor-ver = 播放
editor-pausar = 暂停
editor-anterior = 上一个
editor-siguiente = 下一个
editor-velocidad = { $veces }x
editor-guardar = 保存
editor-guardado-rapido = 快速
editor-copiar = 复制
editor-descartar = 放弃

# Click-through pins (P1.4).
comando-alternar-paso-de-clics = 让点击穿透贴图
pin-dejar-pasar-clic = 点击穿透

# Muting the shortcuts for a while (P1.7).
comando-silenciar-atajos = 暂停快捷键
bandeja-silenciada = PinDesk — 快捷键已暂停
aviso-atajos-silenciados = 快捷键已暂停
aviso-atajos-silenciados-detalle = 可在同一菜单中重新启用。
aviso-atajos-activos = 快捷键已启用
aviso-atajos-activos-detalle = { $cuantos } 个快捷键已恢复响应。

# Delayed capture (P2.1).
comando-capturar-con-retardo = 倒计时后截图
cuenta-atras-cancelar = 按 Esc 停止

# Pinning the Explorer selection (P1.6).
comando-pinear-seleccion = 贴出资源管理器中的选中项

# The settings window (P6).
ajustes-pestana-dibujo = 绘图
ajustes-iman = 吸附到关键点
ajustes-iman-esquinas = 角点和端点
ajustes-iman-medios = 中点
ajustes-iman-centros = 中心
ajustes-iman-radio = 吸附距离（像素）
ajustes-idioma = 语言
ajustes-idioma-sistema = 与 Windows 相同
ajustes-arranque = 随 Windows 启动
ajustes-nivel-auto = 自动
ajustes-nivel-completo = 完整
ajustes-nivel-ligero = 轻量
ajustes-pdf-aligerar = 添加到聊天时压缩 PDF
ajustes-pdf-nivel-sin-perdida = 无损
ajustes-pdf-nivel-equilibrado = 中等
ajustes-pdf-nivel-pequeno = 小
ajustes-pdf-nivel-extremo = 最大
ajustes-sin-atajo = 无快捷键
ajustes-pulsa-combinacion = 请按下组合键…
ajustes-choca = 重复

# Reading the text of a pin (P4.2).
pin-copiar-texto = 复制图片中的文字

# Opening an image pin in the editor canvas.
pin-abrir-en-lienzo = 在画布中打开
pin-congelar = 冻结为图片
pin-manejar = 远程控制
pin-dejar-de-manejar = 停止远程控制

# Pages of a pinned PDF.
pin-pagina-siguiente = 下一页
pin-pagina-anterior = 上一页
pin-extraer-pagina = 将此页提取为贴图
pin-extraer-todas = 提取所有页面
aviso-paginas-extraidas = 页面已提取
aviso-paginas-extraidas-detalle = { $hechas } / { $total }。其余页面未提取，以免屏幕无法使用。
chat-titulo = 项目
chat-sin-proyectos = 还没有项目
chat-elige-proyecto = 选择一个项目以查看
chat-hojas = { $cuantas ->
   *[other] { $cuantas } 页
  }
chat-sin-mensajes = 此项目中还没有内容
chat-clase-imagen = 图片
chat-clase-archivo = 文件
chat-clase-voz = 语音消息
chat-clase-dibujo = 绘图
chat-clase-pagina = 页面
chat-clase-proyecto = 项目
chat-clase-miniapp = 小程序
chat-escribe = 写点什么…
# The chat, like PinDesk Android's (guardados/MensajesActivity.kt).
chat-elegidos = { $n ->
   *[other] 已选 { $n } 项
}
chat-lienzo = 画布
chat-tabla = 表格
chat-viene-de = 来自
chat-copiar = 复制
chat-fijar = 置顶
chat-soltar = 取消置顶
chat-compartir = 分享…
chat-abrir-con = 打开方式…
abrir-con = 打开方式…
abrir-con-no-existe = 找不到文件“{ $nombre }”：它可能已被移动或删除。
abrir-con-fallo = Windows 无法为此文件显示“打开方式”。
chat-aligerar = 压缩 PDF
chat-renombrar = 重命名
chat-renombrar-barra = 名称：
chat-renombrar-teclas = Enter 保存 · Esc 保留原名称
chat-describir = 添加说明
chat-describir-editar = 编辑说明
chat-describir-barra = 说明：
chat-describir-teclas = Enter 保存 · Esc 保持原样
chat-pinear = 贴图
chat-recordar = 提醒我
chat-rescatar = 保留此项
chat-solo-la-foto = 仅显示照片
chat-ver-hilo = 查看评论
chat-reenviar = 转发
chat-unir = 添加到项目
chat-transcribir = 转为文字
chat-letra = 歌词或文本
chat-borrar = 删除
chat-borrar-aviso = 它将从对话中移除，附带的文件也会一并删除。是否继续？
chat-biblioteca-audio = 音频库
chat-fondo = 聊天背景
chat-escala-puesta = 界面缩放：{ $cuanto } %
chat-fondo-mar = 海洋
chat-fondo-arena = 沙滩
chat-fondo-brezo = 石楠
chat-fondo-bosque = 森林
chat-fondo-ocaso = 日落
chat-fondo-pizarra = 石板
chat-adj-telepronter = 提词器
chat-adj-pronunciar = 发音
chat-reenviado = 已转发
chat-copiado = 已复制
chat-compartir-sin-panel = Windows 未打开“共享”面板：内容已复制到剪贴板，可粘贴到任意位置
chat-pineado = 已贴图
chat-foto-en-la-nota = 照片已添加到文字中：将作为一条笔记一起发送
chat-sin-archivo = 此消息在这台电脑上没有文件
chat-sin-origen = 该画布不在此项目中
chat-sin-proyectos-otros = 没有其他项目可发送
chat-no-hay-unir = 在 Windows 上每个聊天本身就是项目：请使用“转发”将其发送到其他项目
chat-ver-dibujo-entero = 显示完整绘图
chat-no-se-pudo = 操作失败：请查看 PinDesk 日志
chat-fecha = { $mes }{ $dia }日
chat-fecha-con-ano = { $ano }年{ $mes }{ $dia }日
chat-mes-1 = 1月
chat-mes-2 = 2月
chat-mes-3 = 3月
chat-mes-4 = 4月
chat-mes-5 = 5月
chat-mes-6 = 6月
chat-mes-7 = 7月
chat-mes-8 = 8月
chat-mes-9 = 9月
chat-mes-10 = 10月
chat-mes-11 = 11月
chat-mes-12 = 12月
chat-hoy = 今天
chat-ayer = 昨天
chat-buscar = 搜索
chat-adjuntar = 附加
chat-fijado = 置顶消息
chat-sistema-memoria-menu = 助手记忆（打开其文件夹以查看或编辑）
info-galeria = 图库
info-todo = 全部
info-fotos = 照片
info-archivos = 文件
info-voz = 语音
info-dibujos = 绘图
info-fijados = 已置顶
info-buzon = 收件箱
info-videos = 视频
info-enlaces = 链接
info-resumen = { $n ->
   *[other] { $n } 项
}  ·  { $codigo }
info-previa-posicion = { $k } / { $n }
info-vacia = 这里还没有内容
info-mes = { $ano }年{ $mes }
info-vacia-galeria = 项目的页面将显示在这里
info-vacia-fotos = 此聊天中的照片将显示在这里
info-vacia-videos = 此聊天中的视频将显示在这里
info-vacia-archivos = 此聊天中的文件将显示在这里
info-vacia-enlaces = 此聊天中的链接将显示在这里
info-vacia-voz = 语音消息将显示在这里
info-vacia-dibujos = 此聊天中的绘图将显示在这里
info-vacia-fijados = 你置顶的内容将显示在这里
info-vacia-buzon = 收件箱为空
info-titulo = 信息
adjuntar-imagen = 照片或视频
adjuntar-archivo = 文件
adjuntar-lienzo = 新建画布
confirmar-pie = 添加说明…
confirmar-cancelar = 取消
adjuntar-tabla = 新建表格
tabla-nueva = 表格
chat-nombre-nuevo = 项目名称…
hoja-volver = 按 Esc 返回
menu-foto-lienzo = 在画布中打开
menu-foto-pin = 再次贴图
menu-foto-abrir = 用 Windows 打开
recibir-titulo = 从手机接收
recibir-como = 用手机上的 PinDesk 扫描二维码
recibir-esperando = 正在等待手机…
recibir-hecho = 已收到 { $cuantos } 项
recibir-salir = 按 Esc 关闭
comando-recibir-del-movil = 从手机接收…
comando-sincronizar = 同步
adjuntar-del-movil = 从手机获取…
proyecto-renombrar = 重命名
# The chat photo (25-oct-2026): image, emoji or colour in the circle.
foto-chat-menu = 更换聊天头像…
foto-chat-titulo = 聊天头像
foto-chat-imagen = 选择图片…
foto-chat-pegar = 粘贴图片
foto-chat-quitar = 默认
foto-chat-buscar = 搜索表情
foto-chat-sin-emojis = 没有叫这个名字的表情
foto-chat-fondo = 背景
foto-chat-recorte = 拖动以移动 · 滚动以缩放
foto-chat-guardar = 保存
foto-chat-cancelar = 取消
foto-chat-sin-imagen = 没有已复制的图片
foto-chat-error = 无法保存聊天头像
proyecto-borrar = 删除项目
proyecto-borrar-varios = 删除所选项
proyecto-borrar-aviso = 它将从列表中移除，其文件夹将移到 PinDesk 回收站。是否继续？
proyecto-papelera = 已删除的项目…
# Where each project is saved: its chosen folder or the usual place.
ubicacion-cambiar = 更改位置…
ubicacion-habitual = 移回默认位置
ubicacion-abrir = 打开项目文件夹
ubicacion-elegir-titulo = 选择项目的保存位置
ubicacion-nuevo = 新建项目
ubicacion-nuevo-en = 在文件夹中新建项目…
ubicacion-no-disponible = 位置不可用
ubicacion-no-disponible-abrir = 此项目的文件夹不可用（{ $ruta }）。请连接驱动器后重试。
ubicacion-movido = 项目已保存到 { $ruta }
ubicacion-vuelto = 项目已移回默认位置
ubicacion-error-red = 项目只能保存在本机的驱动器上，不能保存在网络驱动器上。
ubicacion-error-no-es-carpeta = 该文件夹在此电脑上不存在。
ubicacion-error-dentro = 该文件夹位于项目自身或 PinDesk 数据目录中。
ubicacion-error-mover = 无法移动项目，它仍在原位置。
ubicacion-error-abrir = 无法打开项目文件夹。

## Sync (copy of the phone's SincronizarActivity.kt)
sinc-titulo = 同步
sinc-subtitulo = 通过 Wi-Fi，无需联网。
sinc-este-aparato = 本设备
sinc-tus-aparatos = 你的设备
sinc-sin-grupo = 在一台设备上创建群组，再在其他设备上用其代码加入。只有群组内的设备会相互同步。
sinc-crear-grupo = 创建群组
sinc-unirme = 用代码加入
sinc-grupo-vacio = 群组中还没有其他设备。请用下方的代码添加。
sinc-disponible = 可用
sinc-no-encontrado = 未找到
sinc-sincronizar = 同步
sinc-probar = 尝试
sinc-con-todos = 与全部同步（{ $n }）
sinc-con-todos-como = 不再询问：沿用上次对每台设备的选择。
sinc-si-no-aparece = 如果没有出现：对方需要打开 PinDesk 并连接到此 Wi-Fi。
sinc-con-direccion = 连接到地址
sinc-anadir = 添加其他设备
sinc-anadir-como = 在另一台设备上：同步 →“用代码加入”。
sinc-toca-codigo = 点击查看代码
sinc-qr-como = 用手机相机扫描：会打开 PinDesk 并自动加入。10 分钟后隐藏。
sinc-red-titulo = PinDesk 没有网络权限
sinc-red-texto = Windows 防火墙不允许你的其他设备连接到 PinDesk，所以用代码加入时看不到它们。只需允许一次：Windows 会要求你以管理员身份确认。
sinc-red-boton = 允许网络访问
sinc-red-pidiendo = 正在等待 Windows 的许可…
sinc-red-listo = 已允许网络访问
sinc-red-hecho = 完成：你的其他设备现在可以在此网络中找到 PinDesk。
sinc-red-publica = PinDesk 已获准在专用网络中使用，但此 Wi‑Fi 被标记为“公用”。请在 设置 → 网络和 Internet → WLAN → 此网络 中将其设为专用。
sinc-red-cancelado = 未授予权限。你可以随时再试一次。
sinc-red-fallo = Windows 不允许添加该权限（可能是此电脑的策略阻止了它）。请向此电脑的管理员求助。
sinc-salir = 退出群组
sinc-pasar = 给别人发送内容
sinc-pasar-como = 仅此一次，发送给同一 Wi-Fi 下任何装有 PinDesk 的人。无需群组。
sinc-recibir = 接收
sinc-enviar = 发送
sinc-pasar-tambien = 收到的内容会进入项目列表或“收藏的消息”。
sinc-actividad = 活动
sinc-escuchando = 此窗口打开期间，可通过 { $dir } 找到这台电脑。
sinc-nombre-titulo = 本设备名称
sinc-nombre-como = 你的其他设备将看到这个名称。
sinc-unirme-titulo = 加入群组
sinc-unirme-como = 输入群组内某台设备在“添加其他设备”下显示的代码。该设备需要打开 PinDesk 并连接同一 Wi-Fi。
sinc-codigo = 代码
sinc-direccion = 地址（仅在找不到时）
sinc-direccion-titulo = 连接到地址
sinc-direccion-como = 输入另一台设备在同一窗口中显示的地址。
sinc-la-de-este = 本设备的地址：
sinc-salir-titulo = 退出群组？
sinc-salir-como = 本设备将停止与其他设备同步。不会删除任何内容。如果再次加入，第一次会询问双方发生的更改。
sinc-salir-boton = 退出
sinc-un-momento = 请稍候
sinc-no-se-pudo = 操作失败
sinc-guardar = 保存
sinc-unirme-boton = 加入
sinc-conectar = 连接
sinc-cancelar = 取消
sinc-ocultar = 隐藏
sinc-vale = 确定
sinc-con-todos-rondas = 不再询问，逐台进行并分两轮，确保每台设备最终都拥有全部内容。
sinc-elegir-titulo = 要同步什么？
sinc-elegir-con = 与 { $otro } 同步。勾选的内容在两边将保持一致。
sinc-en-los-dos = 两边都有
sinc-solo-en-uno = 仅一边有
sinc-este-aparato-n = 本设备 · { $n }
sinc-elegir-como = 两边都更改的内容会合并：逐个形状、逐个单元格、逐段合并。如果双方修改了同一处，以最后的更改为准，较早的版本保留在“备份”中。
sinc-todo = 全部
sinc-nada = 无
sinc-sincronizar-n = 同步 { $n } 项
sinc-sincronizando = 正在同步
sincro-borrado-alli-vuelve = 已在 { $otro } 上删除 · 将重新发送过去
sincro-borrado-alli-se-borra = 已在 { $otro } 上删除 · 将在此处删除（会保留备份）
sincro-borrado-alli-se-queda = 已在 { $otro } 上删除 · 未勾选：保留在此处
sincro-se-borran-aqui = 将在此处删除，因为它已在 { $otro } 上删除：{ $lista }。取消勾选可保留，或选择“以我为准”将其发回。
sincro-se-borran-alli = 将在 { $otro } 上删除，因为你已在此处删除：{ $lista }。
sincro-juntar = 合并
sincro-lo-mio-manda = 以我为准
sincro-lo-mio-manda-como = 仅单向：{ $yo } 的内容将覆盖 { $otro }。此处不会删除或更改任何内容；对方那边，不同之处将变得与此处相同，对方删除的内容会恢复。仅 { $otro } 拥有的内容会保留。被覆盖的内容会保留备份。
sincro-mandar-n = 发送 { $n } 项

## Receive over Wi-Fi (copy of the phone's RecibirActivity.kt)
rw-titulo = 通过 Wi-Fi 接收
rw-misma-wifi = 你需要与发送方连接同一 Wi-Fi（或连接其热点）。
rw-o-escribe = 或输入代码
rw-pista-pega = 此电脑没有摄像头：请输入六位数字，或用 Ctrl+V 粘贴其二维码的文本。
rw-recibir = 接收
rw-prefieres = 想让对方扫描？显示你自己的代码，让发送方用摄像头对准它。
rw-ensenar = 显示我的代码以便对方发送给我
rw-que-te-escaneen = 让对方扫描此码
rw-esperando-mio = 正在等待有人向你发送内容。你也可以把此代码读给对方，用于“发送 → 扫描接收方”。
rw-buscando = 正在查找发送方…
rw-conectando = 正在连接…
rw-no-encontrado = 没有人在用该代码发送。请检查代码，并确认你们连接的是同一 Wi-Fi（或其中一方的热点）。
rw-codigo-malo = 代码不正确，请检查。
rw-se-corto = 连接中断：{ $motivo }
rw-quiere = { $de } 想要发送给你
rw-aparato = 设备 { $codigo }
rw-proyecto = 项目“{ $nombre }”
rw-lienzo = 画布“{ $nombre }”
rw-nada-coincide = 这些内容与你已有的都不匹配：将作为新内容导入，不会改动你的任何内容。
rw-no-gracias = 不用了，谢谢
rw-aceptar = 接受
rw-recibiendo = 正在从 { $de } 接收
rw-de = { $hechos } / { $total }
rw-recibido = 已从 { $de } 接收
rw-no-guardado = 已收到，但无法保存。
rw-en-proyectos = { $nombre }：在项目列表中
rw-en-guardados = { $nombre }：在“收藏的消息”中
rw-en-carpeta = { $nombre }：仅在“recibidos”文件夹中
rw-no-se-pudo = 操作失败
rw-probar-otra-vez = 重试
rw-cerrar = 关闭

## Send over Wi-Fi (copy of the phone's EnviarActivity.kt)
ew-titulo = 通过 Wi-Fi 发送
ew-que = 要发送什么？
ew-que-como = 在手机上分享时选择；在这里可选择本机文件或聊天中的整个项目。
ew-archivos = 文件
ew-un-proyecto = 项目
ew-elige-proyecto = 选择项目
ew-sin-proyectos = 还没有项目。
ew-preparando = 正在准备…
ew-no-preparado = 无法准备要发送的内容。
ew-vas-a-enviar = 你将发送
ew-vas-a-enviar-n = 你将发送 { $n } 项内容
ew-y-mas = 以及另外 { $n } 项
ew-un-aparato = 1 台设备
ew-n-aparatos = { $n } 台设备
ew-pidiendo = 想要接收你发送的内容
ew-enviando = 正在发送 · { $hechos } / { $total }
ew-esperando-acepte = 正在等待对方接受…
ew-enviado = 已发送
ew-no-acepto = 已拒绝
ew-no-aprobado = 未批准
ew-no = 否
ew-enviar = 发送
ew-enviar-a-todos = 发送给所有等待中的设备
ew-pegar = 粘贴接收方的代码
ew-pegar-como = 在另一台设备上：通过 Wi-Fi 接收 →“显示我的代码以便对方发送给我”。在此输入其六位数字，或用 Ctrl+V 粘贴其二维码的文本。
ew-mandar = 发送
ew-dejar-de-pegar = 停止粘贴
ew-en-el-otro = 在另一部手机上：PinDesk → 同步 → 接收
ew-o-escribe = 或输入此代码
ew-misma-wifi = 双方连接同一 Wi-Fi
ew-misma-wifi-como = 如果没有 Wi-Fi，一方可以打开热点，另一方连接即可。不会使用互联网。
ew-malos = 有人输入了错误的代码（{ $n } / { $de }）。
ew-esperando = 正在等待… 可以有多台设备连接，每台都需要你按一下按钮。关闭此界面后代码将失效。
ew-quemado = 有人多次输入错误代码，此代码已失效。请重新发送以获取新代码。
ew-cerrar = 关闭
ew-grupo = 你的设备 · 无需代码
ew-grupo-como = 它们在你的群组中：直接发送，无需扫描。内容会进入对方当前打开的位置（聊天；画布只接收照片）或“收藏的消息”。
ew-grupo-se-ve = 在同一 Wi-Fi 下
ew-grupo-no-se-ve = 当前不可见：请在该设备上打开 PinDesk
ew-grupo-enviando = 正在发送…
ew-grupo-todos = 发送给所有可见设备
ew-grupo-actualiza = 请更新 { $movil } 上的 PinDesk：它暂不支持直接接收文件。请使用下方的代码。

## «Update mine / Create as new» when receiving (RecibirActivity.kt)
rw-ya-tienes = 你已有代码相同的“{ $nombre }”
rw-actualizar = 更新我已有的
rw-como-nuevo = 作为新内容创建
rw-encima-de-lo-tuyo = 将覆盖你的内容。原有内容保留在“备份”中。
rw-entra-aparte = 将使用新代码单独导入：今后它将是另一项内容。
rw-proyecto-al-dia = “{ $nombre }”已更新
rw-guardados-al-dia = “收藏的消息”中的“{ $nombre }”已更新

# The header magnifier, which searches inside the conversation (task 1.3).
chat-buscar-aqui = 在此搜索…
chat-buscar-nada = 没有匹配项
chat-buscar-resultados = { $n ->
   *[other] { $n } 个结果
}
chat-buscar-en-contexto = 在上下文中查看
# The list search box also searches inside every chat: the section under the
# chats and its "show more" (25-oct-2026).
chat-buscar-mensajes = { $n ->
   *[other] 消息 · { $n }
}
chat-buscar-ver-mas = 再显示 { $n } 条
chat-buscar-en = 在 { $chat } 中
chat-pagina-n = 第 { $n } 页
chat-unido-a = 已添加到“{ $nombre }”
chat-unir-nada = 这里没有可添加的内容：仅支持照片、绘图、笔记和 PDF
chat-devolver = 重新添加到项目
chat-devuelto = 已回到“{ $nombre }”
chat-devolver-no = 无法重新添加
unir-pdf-como-imagenes = 作为图片添加
unir-pdf-empezado = 正在将 PDF 添加到项目…
unir-pdf-ya-va = 该 PDF 正在添加中
unir-pdf-no = 无法添加 PDF
unir-pdf-hecho = { $hojas ->
   *[other] 已向项目添加 { $hojas } 页
}
unir-pdf-pintadas = { $hojas ->
   *[other] 已添加 { $hojas } 页（无法复制 PDF：页面已被绘制）
}
unir-pdf-fotos = { $hojas ->
   *[other] 已将 { $paginas } 页中的 { $hojas } 页作为图片添加
}
unir-pdf-pintando = 正在绘制页面… { $hechas } / { $total }
unir-pdf-imagenes-progreso = 正在转换为图片… { $hechas } / { $total }

## Backups (copy of sincro/CopiasActivity.kt from the phone)
cop-titulo = 备份
cop-portada-como = 每个对话在通过 Wi-Fi 接收内容之前以及每次同步之前的状态。
cop-ver = 查看备份
cop-versiones = 早期版本
cop-como-estaba = 项目在每次接收传输和每次同步之前的状态。恢复到某个版本不会删除你之后所做的内容。
cop-todavia-nada = 还没有备份。在通过 Wi-Fi 接收内容和同步之前会自动创建备份。
cop-como-se-hacen = 每个对话保留最近 30 个备份。超过 40 MB 的文件不包含在备份中，无法恢复。
cop-hojas-mensajes = { $hojas } 页 · { $mensajes } 条消息
cop-sin-grandes =  · 不含 PDF（过大）
cop-volver = 恢复
cop-cancelar = 取消
cop-volver-titulo = 恢复到此备份？
cop-volver-texto = “{ $nombre }”将恢复到 { $cuando } 时的状态（{ $motivo }）：{ $hojas } 页。之后添加的画布会保留。会先保存当前状态的备份，以便撤销此操作。
cop-ha-vuelto = “{ $nombre }”已恢复到备份
cop-no-se-pudo = 无法恢复到备份：{ $motivo }
papelera-titulo = 已删除的项目
papelera-como = 恢复后完整如初：名称、按顺序排列的页面、PDF 和聊天都在。并且不再处于已删除状态，同步时不会再次被移除。
papelera-detalle = { $hojas } 页 · { $mensajes } 条消息 · 删除于 { $cuando }
papelera-recuperar = 恢复
papelera-recuperar-todos = 全部恢复
papelera-ha-vuelto = “{ $nombre }”已完整恢复
papelera-no-se-pudo = 无法恢复“{ $nombre }”：{ $motivo }
papelera-recuperados = 已恢复 { $total } 个项目中的 { $n } 个

## The document viewer: Word, EPUB books and web pages (task 2.1)
## The Office pin: Office's preview as a pin (2026-10-10)
office-titulo = Office 预览
office-abriendo = 正在用 Office 预览打开…
office-tarda = Office 需要一些时间（它需要启动）。你可以等待，或通过“打开方式…”（工具栏上的 ↗ 按钮）选择其他应用。
office-fallo = Office 无法预览此文件。请通过“打开方式…”（工具栏上的 ↗ 按钮）在其他应用中打开。
office-internet = 此文件来自互联网，出于安全考虑，Windows 不允许预览。请通过“打开方式…”（工具栏上的 ↗ 按钮）在其他应用中打开，或在其“属性”中解除锁定。
visor-titulo = PinDesk 查看器
visor-tamano = 文字大小
visor-quitar-marcadores = 移除书签
visor-guardar-pagina = 另存为网页
visor-guardar-pdf = 另存为 PDF
visor-abrir-carpeta = 打开文件夹
visor-guardado = 已保存到“{ $nombre }”
visor-no-guardado = 无法保存
visor-vacio = 此文档没有可显示的文字

## The readers: comfortable reading, annotating on top and the PDF reader
lector-indice = 目录
lector-letra-fijada = 字体已锁定：上面绘有笔记
lector-letra-fija = 等宽字体
lector-grosor-normal = 常规
lector-grosor-gruesa = 粗体
lector-quitar-tinta = 移除笔记
lector-tinta-quitada = 笔记已移除（停留在此页面时可按 Ctrl+Z 恢复）
lector-ayuda-docs = 滚轮阅读 · Ctrl+滚轮缩放 · L 朗读 · A 标注 · M 书签 · I 目录 · +/− 大小 · T 字体 · B 粗细 · Esc 退出
lector-pdf-titulo = PinDesk PDF 阅读器
lector-pdf-no-se-abre = 无法打开 PDF
lector-pdf-abriendo = 正在打开 PDF…
lector-pdf-reconociendo = 正在识别本页文字…
lector-pdf-reconocido = 文字已识别：现在可以选择并标记
lector-pdf-sin-texto = 本页未找到文字
lector-exportando = 正在保存带标注的 PDF… { $hechas } / { $total }
lector-anotado = 已标注
lector-office-titulo = PinDesk 中的 Word 和 PowerPoint
lector-presentar = 放映 (F5)
lector-modo-lectura = 阅读模式（重排文字）
lector-imprimir = 打印… (Ctrl+P)
lector-no-imprime = 无法打开打印对话框
lector-copiado = 已复制
lector-escuchar = 朗读 (L)
lector-sin-texto = 此文档没有可朗读的文字
lector-leyendo-texto = 正在从 PDF 中提取文字以朗读…
lector-sin-voz = Windows 没有“{ $idioma }”语音：请在“设置 > 时间和语言 > 语音”中添加
lector-voz-idioma = zh-CN
lector-voces-microsoft-no = ☐ Microsoft 语音 · 非常自然且免费；非官方服务，文字会发送给 Microsoft
lector-voces-microsoft-si = ☑ Microsoft 语音 · 非常自然且免费；非官方服务，文字会发送给 Microsoft
lector-voz-de-microsoft = Microsoft 语音：{ $voz }
lector-voz-del-pais = 本国的
lector-voz-pidiendo = 正在向 Microsoft 请求语音列表… 请稍后再按一次
lector-voz-sin-voces = 没有网络，或 Microsoft 没有该语言的语音
lector-voz-de-ia = AI，可朗读任何语言
lector-voz-mujer = 女声
lector-voz-hombre = 男声
lector-voz-parrafo = 第 { $n } 段，共 { $total } 段
lector-voz-preparando = 正在准备语音…
lector-voz-no-respondio = Microsoft 未响应
lector-voz-sin-voz = 无语音
lector-espacio-tope = 那一侧没有更多空间了
lector-espacio-nada = 那一侧已没有空间
lector-sin-lado = 横向已锁定：只能上下滚动
lector-con-lado = 可向各方向移动
lector-auto = 自动滚动 (D)
lector-auto-ppm = { $ppm } 字/分钟
lector-auto-quedan = 还需 { $cuanto } · 约 { $hora } 读完
lector-auto-menos-de-un-minuto = 不到 1 分钟
lector-auto-terminado = 已读完
lector-auto-contando = 正在统计字数…
lector-ver-como-texto = 以文本查看（类似 Word 文件）
# Lectura Veloz, the speed-reading app (27-Oct-2026).
lector-lectura-veloz = ⚡ 速读
lector-lectura-veloz-no-instalada = 此电脑未安装 Lectura Veloz（缺少 lecturaveloz:// 链接）。请安装后重试。
lector-lectura-veloz-no-se-pudo = 无法打开 Lectura Veloz：{ $motivo }
# Speed reading (27-oct-2026): the % next to the option, the bubble chip
# and the real reason when it can't open.
lector-lectura-veloz-con = ⚡ 速读 · { $pct } %
lector-lectura-veloz-sin-codigo = 此文档无法在 Lectura Veloz 中阅读：它不是聊天文件或已保存的笔记（没有消息代码）。
lector-lectura-veloz-formato = 此文档无法在 Lectura Veloz 中阅读：它不支持“.{ $ext }”文件。
lector-lectura-veloz-falta-archivo = 此文档无法在 Lectura Veloz 中阅读：该文件已不在此电脑上（{ $nombre }）。
chat-lectura-veloz-chapa = 速读 · { $pct } %
# The chip of what came from the phone's scanner (`escaneado`).
chat-escaneado = 已扫描
lector-pasando-a-texto = 正在将 PDF 转为文字…
lector-texto-escaneo = 此 PDF 没有可提取的文字：它可能是扫描件（页面的照片）
lector-texto-cifrado = 此 PDF 受密码保护
lector-texto-no-se-lee = 无法读取此 PDF
lector-ver-pdf-original = 以页面形式查看原始 PDF
lector-hoja = 页
sinc-presencia-apagada = 设置中已关闭在线状态（[sincro] presencia = false）：你可以从这里同步，但手机将找不到这台电脑，也无法呼叫它。
bandeja-abrir-documento = 打开文档…

## Canvas shapes and ink: arrowheads, fills and text style (group A)

## Build and measure: fill, trim, extend, points and angles (group C)

## Select, transform and bind (group B)

# The attachment the document viewer can open without leaving PinDesk.

# The seven mini-apps from the phone, and their panel.
mini-moneda = CNY
mini-tareas = 待办清单
mini-gastos = 支出
mini-cronometro = 秒表
mini-temporizador = 计时器
mini-contador = 计数器
mini-ruleta = 转盘
mini-alarma = 闹钟
mini-tarea-nueva = 输入待办事项后按 Enter · ↑↓ 选择，空格勾选，F2 编辑
mini-gasto-nuevo = 项目和金额（晚餐 42.50）后按 Enter · ↑↓ 选择，F2 编辑
mini-nombre-nuevo = 输入名称后按 Enter · 空白时按 Enter 转动，Del 删除
mini-limpiar-hechas = 清除已完成项
mini-ocultar-hechas = 隐藏已完成项
mini-mostrar-hechas = 显示已完成项
mini-avance = 已完成 { $hechas } / { $de }
mini-tarea-edad = { $dias ->
    [0] 今天
   *[other] { $dias } 天前
  }
mini-arrancar = 开始
mini-parar = 停止
mini-vuelta = 计圈
mini-reiniciar = 重置
mini-sortear = 转动
mini-alarma-encender = 开启
mini-alarma-apagar = 关闭
mini-guia-titulo = 输入新名称后按 Enter（Esc 保留原名称）
mini-guia-cambiar = 修改后按 Enter（Esc 保持原样）
mini-guia-duracion = 分钟数或 分:秒，然后按 Enter · 空格开始
mini-guia-hora = 时间（7:30），然后按 Enter · 空格开启
chat-biblioteca-vacia = 还没有语音消息或歌曲。
chat-biblioteca-musica = 音乐
chat-biblioteca-notas = 语音消息

# Recording a voice note from the chat.
chat-voz-muy-corta = 太短了：请多按住一会儿。
chat-voz-sin-microfono = 没有麦克风，或 PinDesk 无权使用。请检查“设置 › 隐私 › 麦克风”。
chat-voz-sin-codec = 此 Windows 没有音频编码器（通常是未安装媒体功能包的 N 版）。
chat-voz-fallo = 无法录制语音消息。

# Shrinking an attached PDF: it lowers the photos inside, and never for the worse.
pdf-aligerado = 已压缩：从 { $antes } 降到 { $despues }
pdf-aligerar-cifrado = 此 PDF 已加密，重写可能会损坏它。
pdf-aligerar-no-se-lee = 压缩后的文件无法正常读取，已保留原文件。
aligerar-progreso = 正在压缩… { $por }%
aligerar-empezado = 正在压缩 PDF…
aligerar-sin-mejora = 此 PDF 已经很小：保持原样。

# Reminders: the time set on a message and the notice that shows up when it
# arrives. The same times the phone offers, plus ten minutes.
chat-recordar-10-min = 10 分钟后
chat-recordar-1-hora = 1 小时后
chat-recordar-3-horas = 3 小时后
chat-recordar-esta-tarde = 今晚（18:00）
chat-recordar-manana = 明天（9:00）
chat-recordatorio-puesto = 我会在 { $cuando } 提醒你
chat-recordatorio-quitar = 移除提醒
chat-recordatorio-quitado = 提醒已移除
chat-recordatorio-titulo = 提醒

# A message menu entry: send the attachment to someone over Wi-Fi.

# Turning a voice note into text with Vosk. What is missing is named with its
# file and link: PinDesk never downloads anything on its own.
chat-transcribiendo = 正在转为文字… { $pct }%
chat-transcribir-hecha = 语音消息已转为文字
chat-transcribir-en-marcha = 另一条消息正在转换，请等待其完成
chat-transcribir-sin-mensaje = 该消息已不在对话中：文字无处可放
chat-transcribir-cancelada = 已取消转为文字
chat-transcribir-otra-vez = 重新转为文字
chat-transcripcion-no = 无法转为文字：音频中没有可识别的内容。
chat-letra-vacia = 此消息还没有文字。
chat-letra-copiada = 文字已复制（含时间戳）

## Music with its lyrics (karaoke, 27-Oct-2026)
chat-adj-musica = 音乐
musica-anadida = 歌曲已添加。点击铅笔即可输入或粘贴歌词
musica-anadidas = 已添加 { $n } 首歌曲
musica-no-es-audio = “{ $nombre }”不是可播放的音频
musica-sin-letra = 这首歌还没有歌词
musica-escribir-letra = 点击铅笔即可输入或粘贴歌词（纯文本或 LRC）
musica-sinc-primero-letra = 请先输入或粘贴歌词
musica-sinc-empieza = 在每一行开始时按空格（或“标记”）
musica-sinc-siguiente = 下一行：{ $linea }
musica-sinc-todas = 所有行均已标记。请保存歌词。
musica-sinc-marcar = 标记 { $n }/{ $total }
musica-sinc-guardar = 保存
musica-sinc-cancelar = 取消
musica-sinc-guardada = 歌词已同步
musica-sinc-teclas = 空格标记 · Backspace 撤销 · Enter 保存 · Esc 取消
musica-lrc-copiada = 歌词已复制为 LRC
musica-sola-empieza = 正在自动同步歌词：正在聆听歌曲…
musica-sola-avance = 正在同步歌词… { $pct } %
musica-sola-bajando = 正在下载语音识别器… { $pct } %
musica-sola-parada = 同步已停止
musica-sola-ocupada = 另一首歌曲正在同步
musica-sola-sin-reconocedor = 这台电脑无法聆听歌曲：缺少 ONNX Runtime
musica-sola-falta-modelo = 要自动设置时间，请点击“自动同步”（字幕图标）：将一次性下载语音识别器（约 160 MB）
musica-sola-no-se-oye = 无法在歌曲中识别出歌词。请使用时钟手动点按同步
musica-sola-fallo = 无法同步歌词
musica-sola-cambio = 同步期间歌词已更改：请再次点击“自动同步”
musica-sola-hecha = 歌词已自动同步（识别出 { $total } 行中的 { $n } 行）。如有偏差，请用时钟修正
musica-sola-hecha-poca = 歌词已粗略同步：仅识别出 { $total } 行中的 { $n } 行。请用时钟修正

chat-recordar-hora = 选择时间…
chat-recordar-hora-a-las = 提醒时间
chat-recordar-hora-teclas = 输入时间 · Enter 设置 · Esc 取消
chat-recordar-hora-queda = Enter：{ $cuando }
chat-recordar-hora-mal = 这不是有效时间：请按 18:30 这样的格式输入
chat-voz-sin-motor = 缺少语音识别器：请将 { $motor } 复制到 { $donde }
chat-voz-sin-modelo = 缺少 { $modelo } 语音模型：请从 { $enlace } 下载并解压到 { $donde }
chat-voz-idioma-sin-modelo = Vosk 没有此语言的语音模型
chat-voz-ruta-imposible = 模型文件夹（{ $donde }）包含识别器无法打开的字符；请将其移到不含重音符号的路径
chat-voz-modelo-ilegible = { $donde } 中的模型不完整或无法加载
chat-voz-no-es-audio = 无法读取此消息的音频
chat-voz-audio-vacio = 音频为空：没有可转换的内容
chat-voz-no-se-entiende = 无法识别消息中说的内容
chat-voz-sin-reconocedor = Windows 没有此语言的语音识别器：请在“设置 › 时间和语言 › 语音”中添加。
chat-voz-bajar-whisper = 转为文字需要 Whisper 语音模型（约 { $megas } MB）。现在正在下载，仅此一次，完成后将转换该消息。
chat-voz-bajando-whisper = 正在下载语音模型… { $pct }%
chat-voz-sin-modelo-whisper = 缺少 Whisper 语音模型（约 { $megas } MB）：请再次点击“转为文字”进行下载。
chat-voz-sin-modelo-qwen3 = 尚未下载 Qwen3-ASR（约 { $megas } MB）：请在“设置 › 语音”中选择它以下载，或改回 Whisper。
chat-voz-sin-onnx = 此 Windows 缺少 AI 运行时（onnxruntime.dll）：请将其复制到 { $donde } 或 PinDesk 所在目录。
chat-voz-descarga = 无法下载语音模型（{ $que }）。请检查网络连接，然后再次点击“转为文字”。
chat-voz-onnx = Whisper 语音引擎出错：{ $detalle }
chat-dictado-oyendo = 正在聆听… 请说话，说完后点击麦克风
chat-dictado-fallo = 听写失败：{ $razon }
chat-anotar-en = 记录到“{ $chat }”：{ $texto }
chat-anotar-donde = “{ $texto }”要记录到哪个聊天？请选择聊天
chat-anotar-tecla = Enter
chat-anotado-en = 已记录到“{ $chat }”
chat-dictado-preparando = 正在准备麦克风…

# The teleprompter: the text scrolls by and you read it aloud while it
# records. There is no synthetic voice anywhere, same as on the phone.
telepronter-titulo = 朗读
telepronter-ensayar = 排练
telepronter-parar = 停止
telepronter-grabar = 录制
telepronter-terminar = 完成
telepronter-tirar = 放弃
telepronter-velocidad = 速度和大小
telepronter-preparate = 准备
telepronter-muy-corta = 朗读时间太短
telepronter-sin-micro = 没有可用的麦克风
chat-telepronter-borrador = 输入框中的内容
chat-telepronter-sin-texto = 没有可朗读的文字：请在输入框中输入内容，或附加 .txt 或 .md 文件
chat-telepronter-abierto = 提词器已打开
chat-telepronter-hecha = 朗读已保存为语音消息

# The capture stack: the little pile in the corner (icon and panel).
pila-titulo-una = 1 张截图
pila-titulo-varias = 已堆叠 { $cuantas } 张截图
pila-copiar-elegidas = 复制所选
pila-copiar-todas = 全部复制
pila-quitar = 移除
pila-copiadas = 已复制 { $cuantas } 张截图：按 Ctrl+V 粘贴

# The drawing editor's properties panel (Excalidraw's).
panel-trazo = 描边
panel-fondo = 背景
panel-relleno = 填充
panel-grosor = 描边宽度
panel-estilo-trazo = 描边样式
panel-presion = 压感
panel-trazo-a-mano = 手绘感
panel-bordes = 边角
panel-tipo-flecha = 箭头类型
panel-fuente = 字体
panel-tamano-fuente = 字号
panel-alineacion-texto = 文本对齐
panel-puntas = 箭头
panel-opacidad = 不透明度
panel-capas = 图层
panel-alinear = 对齐
panel-acciones = 操作
panel-colores = 颜色
panel-tonos = 色调
panel-codigo-hex = 十六进制代码

## The canvas paper (Excalidraw viewBackgroundColor)
fondo-lienzo = 画布背景
fondo-papeles-del-movil = 手机纸张
fondo-lienzo-menu = 画布背景…

## Export and print the canvas (G1, G2, G3, F11)
exportar-menu = 导出…	Ctrl+Shift+E
exportar-copiar-png = 复制为 PNG	Ctrl+Shift+C
exportar-titulo = 导出画布
exportar-que = 内容
exportar-todo = 整个画布
exportar-seleccion = 所选内容
exportar-marcos = 每个画框
exportar-escala = 缩放
exportar-transparente = 透明背景
exportar-tipo-png = PNG 图片
exportar-tipo-svg = SVG 绘图
exportar-tipo-pdf = PDF 文档
exportar-tipo-html = 单文件网页
exportar-vacio = 此选项下没有可导出的内容。
exportar-fallo = 无法导出画布。
imprimir-menu = 打印…	Ctrl+P
imprimir-vacio = 没有可打印的内容。
imprimir-fallo = 无法打印画布。
exportar-chat = 导出…
imprimir-chat = 打印…
# F11: imprimir con vista previa (el dialogo de Windows y el boton de la barra)
imprimir-pista = 打印… (Ctrl+P)
imprimir-que = 打印内容
imprimir-que-marcos = 每个画框单独一页
imprimir-que-todo = 整个画布打印在一页上
imprimir-que-seleccion = 仅所选内容
marca-elige = 用什么标记？（1-9、0）· Esc 取消
marca-plantar = 在画布上点击放置标记的位置 · Esc 取消
marca-vista-guardada = 视图已随标记保存 · Shift+点击可更新
hojita-borrar = 清除
hojita-insertar = 插入
hojita-pegar = 作为贴图

# The share sheet (compartir.rs): one for everything, like the phone's.
compartir-titulo = 分享
compartir-menu = 分享…	Ctrl+Shift+S
compartir-proyecto = 分享…
compartir-lector = 分享…  (Ctrl+Shift+S)
compartir-boton = 分享
compartir-guardar = 另存为…
compartir-copiar = 复制到剪贴板
compartir-wifi = 通过 Wi‑Fi 发送到我的设备
compartir-boton-copiar = 复制
compartir-boton-wifi = Wi-Fi
compartir-boton-enlace = 链接
compartir-enlace-pista = 上传到 litterbox（保留 3 天）并复制链接：任何拿到链接的人都能打开
compartir-enlace-subiendo = 正在上传…
compartir-enlace-copiado = 链接已复制 · 3 天后删除
compartir-enlace-fallo = 无法上传：{ $motivo }
compartir-web = 网页（HTML）
compartir-adjuntos = 附件
compartir-pdf = PDF
compartir-png = PNG 图片
compartir-jpg = JPG 图片
compartir-svg = SVG 绘图
compartir-original = 原件
compartir-originales = 原件
compartir-editable = 可编辑
compartir-texto = 文本
compartir-csv = 表格（CSV）
compartir-pindesk = PinDesk (.pindesk)
compartir-dxf = DXF
compartir-md = Markdown
compartir-exportar = 导出…
compartir-paginas-de = 包含内容 · { $marcadas } / { $total }
compartir-una-pagina = 哪一页
compartir-va-entero = 按原样整体发送
compartir-todas = 全部
compartir-ninguna = 无
compartir-preparando = 正在准备…
compartir-marca-alguna = 请勾选至少一页
compartir-en-cuanto-este = 准备好后立即发送
compartir-peso-ficheros = { $peso } · { $n } 个文件
compartir-guardado = 已保存：{ $nombre }
compartir-copiado = 已复制到剪贴板
compartir-no-se-pudo = 无法准备
compartir-nada = 这里没有可分享的内容。
compartir-coma-decimal = .
compartir-lienzo = 画布
compartir-lienzo-entero = 整个画布
compartir-marco = 画框
compartir-pagina = 页面
compartir-anotada = 含笔记
compartir-sin-nombre = 未命名
compartir-tipo-lienzo = 画布
compartir-tipo-pagina = PDF 页面
compartir-tipo-foto = 照片
compartir-tipo-tabla = 表格
compartir-tipo-miniapp = 小程序
compartir-tipo-voz = 语音消息
compartir-tipo-nota = 笔记

# Drawing tools: the ones shown in the canvas, the reader, pins and the
# screen annotator. Turning one off removes it from the bar and its key.
herramientas-lazo = 套索
herramientas-rectangulo = 矩形
herramientas-rombo = 菱形
herramientas-elipse = 椭圆
herramientas-flecha = 箭头
herramientas-flecha-codos = 折线箭头
herramientas-flecha-libre = 手绘箭头
herramientas-linea = 直线
herramientas-lapiz = 钢笔
herramientas-grafito = 铅笔
herramientas-texto = 文本
herramientas-borrador = 橡皮擦
herramientas-resaltador = 荧光笔
herramientas-foco = 聚光灯
herramientas-lupa = 放大镜
herramientas-mosaico = 马赛克
herramientas-arco = 圆弧
herramientas-serie = 编号步骤
herramientas-punto = 带标签的点
herramientas-cota = 尺寸标注
herramientas-escalar = 校准比例
herramientas-escala-grafica = 比例尺
herramientas-marco = 画框
herramientas-relleno = 油漆桶
herramientas-recortar = 修剪
herramientas-extender = 延伸
herramientas-copiar-estilo = 复制样式

## D10: a PowerPoint in the chat (diapositivas.rs)
diapositivas-titulo = 演示文稿
diapositivas-no-es = 这不是演示文稿
diapositivas-keynote = 无法直接读取 Keynote 文件：请导出为 PowerPoint 或 PDF
diapositivas-sin-powerpoint = 放映 PowerPoint 需要安装 PowerPoint
diapositivas-abrir-con-otra = 用其他应用打开？
diapositivas-preparando = 正在用 PowerPoint 准备演示文稿…
diapositivas-no-se-pudo = 无法准备演示文稿
diapositivas-no-esta = 该演示文稿已不在此电脑上
diapositivas-siguiente = 下一张
diapositivas-notas = 备注
diapositivas-sin-notas = 此幻灯片没有备注
diapositivas-fin = 演示结束
diapositivas-transiciones-si = 切换效果：开（T）
diapositivas-transiciones-no = 切换效果：关（T）

## Chat: a page from a project, the link card, «Received from» and the inbox
chat-sin-paginas = 没有项目包含可附加的页面
chat-recibido-de = 来自 { $de }
# Linked versions in a shared chat (27-oct-2026): touching someone else's
# message makes your version, which replies to theirs, and theirs stays as is.
chat-version-sola = 版本
chat-version-sola-n = 版本 { $n }
chat-version-de = { $quien } 的消息的版本
chat-version-de-n = { $quien } 的消息的版本 { $n }
chat-version-de-lo-tuyo = 你的消息的版本
chat-version-de-lo-tuyo-n = 你的消息的版本 { $n }
chat-version-de-otro = 另一条消息的版本
chat-version-de-otro-n = 另一条消息的版本 { $n }
chat-version-de-la-lista = 版本 { $n } · { $quien }
chat-version-tuya = 版本 { $n } · 你的
chat-versiones = { $cuantas ->
   *[other] { $cuantas } 个版本
  }
chat-version-es-de = 这是 { $quien } 的消息：如果你修改它，将保存为你的版本，不会改动对方的内容
chat-version-guardada = 已保存为你的版本；原消息未改动
chat-version-no = 无法准备你的版本
chat-fecha-corta = { $mes }{ $dia }日
chat-enlace-no-abre = 无法打开链接

## D11: a spreadsheet in the chat is a book of tables (ventana_chat/libro.rs)
hoja-preparando = 正在准备工作表以供编辑…
hoja-editar-aviso = 更改保存在聊天中的工作表里；原文件保持不变
hoja-editando = 编辑中 · Esc 查看
hoja-rango-grande = 单元格太多，无法一次处理
hoja-no-editable = 此工作表没有表格：无法编辑
hoja-fondo-sin = 无颜色
hoja-letra-automatica = 自动
hoja-color-amarillo = 黄色
hoja-color-verde = 绿色
hoja-color-azul = 蓝色
hoja-color-rosa = 粉色
hoja-color-morado = 紫色
hoja-color-naranja = 橙色
hoja-color-gris = 灰色
hoja-color-celeste = 浅蓝色
hoja-color-rojo = 红色
hoja-numero-general = 常规
hoja-numero-numero = 数值（1,234.50）
hoja-numero-moneda = 货币
hoja-numero-porcentaje = 百分比（12.50 %）
hoja-numero-fecha = 日期

## D9: find in the document (buscador.rs, visor.rs, lector_pdf.rs)
buscar-pista = 在文档中查找
buscar-cuenta = { $n } / { $total }
buscar-nada = 无结果
buscar-leyendo = 正在读取文字…
buscar-sin-texto = 此 PDF 没有文字（它是图片）
buscar-cifrado = 无法搜索加密的 PDF

## D5: rename from the reader (renombrar_doc.rs)
renombrar-hecho = 已重命名为“{ $nombre }”
renombrar-ya-existe = 已存在名为“{ $nombre }”的文件
renombrar-no-se-pudo = 无法更改名称

## Canvas: zone, laser, image, shapes, graph and present (F8, F12, F14, G5)
herramientas-zona = 区域（圆角复制）
herramientas-laser = 激光笔
herramientas-imagen = 图片
herramientas-figuras = 形状
figuras-grafica = 函数图像…
figuras-tabla-en-blanco = 空白表格
figuras-pegar-tabla = 粘贴表格（来自 Excel）
figuras-recta = 数轴
figuras-plano = 笛卡尔坐标系
figuras-espacio = xyz 空间
figuras-coordenadas = 坐标表…
figuras-coordenadas-serie = 坐标：系列
figuras-coordenadas-nueva = 新建坐标系列…
figuras-coordenadas-esconder = 隐藏系列
figuras-coordenadas-ver = 显示系列
coordenadas-titulo = 坐标表
coordenadas-ayuda = X 和 Y 可用逗号或小数点，Y 向上 · 粘贴：来自 Excel 或 CSV · 没有行则删除该系列 · Enter：确定 · Esc：取消
figuras-guardar-seleccion = 将所选内容保存为形状…
figuras-quitar = 移除
figuras-mis-figuras = 我的形状
figuras-ventana-titulo = 形状
figuras-ventana-subtitulo = { $n } 个形状和模板
figuras-ventana-buscar = 搜索形状
figuras-ventana-todas = 全部
figuras-ventana-insertar-seccion = 插入
figuras-ventana-insertar = 插入
figuras-ventana-guardar-seleccion = 保存所选内容
figuras-ventana-vacio = 没有匹配的形状
figuras-ventana-elige = 选择一个形状以放大查看
figuras-ventana-piezas = { $n } 个部件
figuras-ventana-intervalo = 范围
figuras-ventana-desde = 从
figuras-ventana-hasta = 到
figuras-ventana-de-a = 每隔 … 拆分（每份一页）
figuras-ventana-de-a-pista = 全部放在一页
figuras-ventana-semanas-desde = 起始周
figuras-ventana-semanas-hasta = 结束周
figuras-ventana-mes = 月份
figuras-ventana-meses = 月数
figuras-ventana-una-hoja = 1 页
figuras-ventana-hojas = { $n } 页
figuras-ventana-renombrar = 重命名
figuras-ventana-quitar-seguro = 再次点击以移除
figuras-nombre-titulo = 保存为形状
figuras-nombre = 名称
figuras-nombre-ayuda = Enter：保存 · Esc：取消
figuras-sin-tabla = 剪贴板中没有表格
grafica-titulo = 函数图像
grafica-formulas = y =（每行一条曲线）
grafica-x-desde = x 从
grafica-x-hasta = x 到
grafica-y-desde = y 从
grafica-y-hasta = y 到
grafica-escala = 每单位像素数
grafica-ayuda = Enter：插入 · Shift+Enter：再加一条曲线 · Tab：下一个字段 · Esc：取消 · 分段函数：x^2 si x<0; 2x si x>=0
grafica-limites-mal = 范围不合理：“从”必须小于“到”
grafica-sin-formula = 请输入公式
grafica-formula-mal = 无法理解：
grafica-solo-x = 只能使用 x：
presentar-ayuda = ← → PgDn：下一张 · B：黑屏 · J：激光笔 · L：画笔 · Esc：退出
vista-mirando = 仅查看 · 右侧的铅笔可编辑 · F5：放映

## --- Voice: two languages, lyrics, teleprompter, pronounce, conversation, call (B6-B11) ---

ajustes-voz-segundo = 语音消息的第二语言
ajustes-voz-modo = 使用两种语言时
ajustes-voz-modo-cada-uno = 每部分按所说语言
ajustes-voz-modo-todo-en-uno = 全部按第一语言
ajustes-voz-ninguno = 无
ajustes-voz-ingles = 英语
ajustes-voz-espanol = 西班牙语
ajustes-voz-portugues = 葡萄牙语
chat-letra-editar = 编辑歌词
chat-letra-editando = 歌词正在编辑中
chat-letra-guardar = 保存
chat-letra-guardada = 歌词已保存
chat-cancelar-caja = 取消
chat-llamada-secreta = 私密通话
telepronter-pausa = 暂停
telepronter-seguir = 继续
telepronter-velocidad-muy-lenta = 很慢
telepronter-velocidad-lenta = 慢
telepronter-velocidad-normal = 正常
telepronter-velocidad-rapida = 快
telepronter-velocidad-muy-rapida = 很快
pronunciar-titulo = 发音练习
pronunciar-manten = 按住空格键（或麦克风）说话
pronunciar-suelta = 松开即可听到自己的声音
pronunciar-otra-vez = 再次按住以重复
pronunciar-guia = 参考
pronunciar-guia-texto = 输入文本
pronunciar-guia-fichero = 来自此电脑的图片、PDF 或笔记
pronunciar-sin-guia = 没有参考：请输入一段，或导入图片、PDF 或笔记
pronunciar-repetir = 再听一次 (R)
pronunciar-guardar = 保存到聊天 (Ctrl+G)
pronunciar-ya-guardada = 已保存
pronunciar-guardado = 已保存；正在转写，以便查看识别结果
pronunciar-idioma = 练习的语言
pronunciar-idioma-ajustes = 常用语言
pronunciar-oir-texto = 听文本
pronunciar-sin-voz = Windows 没有该语言的语音
pronunciar-no-se-pudo = 无法打开
pronunciar-toma = 上一次录音：{ $duracion }
pronunciar-pagina = 第 { $pagina } 页，共 { $paginas } 页
pronunciar-abierto = 发音练习已打开
conversacion-titulo = 对话
conversacion-explicacion = 持续录音；每个人开始说话时按下自己的编号（1 到 9），无人说话时按 0。转写结果会带上名字。
conversacion-cuantos = 有几个人在说话？
conversacion-persona = 第 { $n } 人
conversacion-empezar = 开始录音
conversacion-terminar = 结束并转写
conversacion-turnos = { $n } 次发言
conversacion-hablando = 正在说话…
conversacion-teclas = 按下说话人的编号 · 0：无人
conversacion-transcribiendo = 正在转写对话…
conversacion-abierta = 对话已打开
llamada-titulo = 通话
llamada-entrante = 来电
llamada-contestar = 接听
llamada-colgar = 挂断
llamada-altavoz = 扬声器
llamada-perdida = 未接来电

## PDF zone (E9): merge PDF pages into one canvas, as on the phone
fusionar-paginas = 合并到一个画布
fusionar-paginas-empieza = 正在将页面合并到画布…
fusionar-paginas-hecho = 完成：“{ $nombre }”已加入项目
fusionar-paginas-no = 无法合并这些页面

## Universe zone (batch 2: H2, H3, M1): the Cosmos theme outside the universe
ajustes-tema-cosmos = 宇宙主题（聊天中的星空）

## Zona pines (tanda 2: C3, C4, L3): magic word, board, sheet and tools
pin-convertir-en = 转换为
pin-herramienta-temporizador = 计时器
pin-herramienta-cronometro = 秒表
pin-herramienta-tareas = 待办清单
pin-herramienta-contador = 计数器
pin-herramienta-gastos = 支出
pin-herramienta-pizarra = 白板
pin-herramienta-ruleta = 转盘
pin-herramienta-lienzo = 画布
pin-herramienta-hoja = 纸张
pin-herramienta-tabla = 表格
pin-fondo-pizarra = 白板背景
pin-pizarra-blanca = 白色
pin-pizarra-negra = 黑色
pin-pizarra-azul = 石板蓝
pin-pizarra-verde = 黑板绿
pin-pauta-lisa = 空白
pin-pauta-cuadros = 方格
pin-pauta-rayas = 横线
pin-pauta-columnas = 分栏
pin-pauta-puntos = 点阵
pin-tiempo-cumplido = 时间到
pin-hoja-nombre = 纸张

## Window groups (H9) and Markdown note editor (H12)
bandeja-grupos-ventanas = 窗口组…
chat-adj-nota-md = 笔记
chat-editar-nota = 在笔记编辑器中打开
grupos-nada-abierto = 没有打开的画布、文档或笔记可保存
grupos-guardar = { $n ->
   *[other] 将打开的 { $n } 个窗口保存为组…
}
grupos-abrir = 打开“{ $nombre }” · { $n }
grupos-borrar = 删除“{ $nombre }”
grupos-alguna-falta = 组中的某些窗口已无法打开
grupos-sin-nombre = 组
grupos-nombre = 窗口组名称
grupos-guardar-boton = 保存
nota-md-nueva = 新建笔记
nota-md-sufijo = PinDesk 笔记
nota-md-negrita = 粗体
nota-md-cursiva = 斜体
nota-md-tachado = 删除线
nota-md-codigo = 代码
nota-md-enlace = 链接
nota-md-titulo1 = 标题
nota-md-titulo2 = 副标题
nota-md-titulo3 = 小节
nota-md-lista = 列表
nota-md-numerada = 编号列表
nota-md-casilla = 任务列表
nota-md-marcar = 勾选或取消勾选任务
nota-md-cita = 引用
nota-md-bloque = 代码块
nota-md-raya = 分隔线
nota-md-formula = 公式
nota-md-cortar = 剪切
nota-md-copiar = 复制
nota-md-pegar = 粘贴
nota-md-guardar = 保存
nota-md-copia = 保存 .md 副本…
nota-md-tipo = Markdown 笔记
nota-md-no-guardada = 无法保存笔记。仍要关闭并丢失更改吗？
nota-md-mayus = Shift
nota-md-intro = Enter

# H12, the new note editor window (header, toolbar, menus and tables)
nota-md-compartir = 分享
nota-md-tabla = 表格
nota-md-imagen = 图片
nota-md-fecha = 今天的日期
nota-md-pista-barra = 在笔记中输入 / 查看更多
nota-md-cambiar-titulo = 重命名
nota-md-fila-encima = 在上方插入行
nota-md-fila-debajo = 在下方插入行
nota-md-quitar-fila = 删除行
nota-md-columna-izquierda = 在左侧插入列
nota-md-columna-derecha = 在右侧插入列
nota-md-quitar-columna = 删除列
nota-md-quitar-tabla = 删除表格
nota-md-boton-fila-mas = + 行
nota-md-boton-fila-menos = − 行
nota-md-boton-columna-mas = + 列
nota-md-boton-columna-menos = − 列
nota-md-meses = 1月 2月 3月 4月 5月 6月 7月 8月 9月 10月 11月 12月
## Note tables: merge and colour cells (H12)
nota-md-boton-combinar = 合并
nota-md-boton-color = 颜色
nota-md-combinar-celdas = 合并单元格
nota-md-separar-celdas = 取消合并单元格
nota-md-color-fondo = 背景色
nota-md-color-letra = 文字颜色
nota-md-aplicar-a = 单元格|整行|整列
nota-md-nombres-fondo = 无颜色|红色|绿色|蓝色|黄色
nota-md-nombres-letra = 自动|红色|绿色|蓝色|橙色
nota-md-pista-color = 整行和整列：右键单击
nota-md-imagen-no-copiada = 无法将图片复制到笔记旁边。

## Note comments (H12, Sep 30): the panel on the right
nota-md-com-titulo = 评论
nota-md-com-comentar = 评论
nota-md-com-ninguno = 没有评论
nota-md-com-pista = 选中一些文字并点击“评论”（Ctrl+Alt+M）
nota-md-com-sin-ancla = 其对应的文字已不在笔记中
nota-md-com-responder-pista = 回复…
nota-md-com-responder = 回复
nota-md-com-guardar = 保存
nota-md-com-cancelar = 取消
nota-md-com-editar = 编辑
nota-md-com-borrar = 删除
nota-md-com-resuelto = 已解决
nota-md-com-ver-resueltos = 显示已解决
nota-md-com-ocultar-resueltos = 隐藏已解决
nota-md-com-editado = 已编辑
nota-md-com-borrar-hilo = 删除该评论及其回复？

## Markdown notes: images and live pages (H12)
nota-md-pagina-viva = 项目中的页面
nota-md-enlace-hoja = 链接到页面
nota-md-captura = 截图
nota-md-ver-grande = 查看原始大小
nota-md-abrir-hoja = 打开页面
nota-md-foto-pequena = 小
nota-md-foto-mediana = 中
nota-md-foto-grande = 大
nota-md-foto-columna = 栏宽
nota-md-quitar-foto = 移除图片
nota-md-hoja-borrada = 该页面已不存在：这是最后一份副本
nota-md-otros-proyectos = 其他项目
nota-md-sin-hojas = 还没有可添加到笔记的项目页面。
nota-md-solo-imagenes = 这里只能拖放图片。
chat-insertar-en-nota = 插入到笔记

## Canvas: weld vertices and number points (lienzo-geometria, 26-sep)
herramientas-nudo = 焊接顶点
herramientas-bolita = 小球：扫过以选择
lienzo-punto-serie = 点标签
lienzo-cota-pedir = 绘制时询问长度
lienzo-panel-dureza = 铅笔硬度
lienzo-guia-corto = 参考线
lienzo-panel-guias = 参考线
lienzo-cota-titulo = 多长、朝哪个方向？（Tab 切换字段）
lienzo-cota-largo = 长度
lienzo-cota-angulo = 角度

## Canvas: image (zone, magnifier, pixelate, share and reference; Sep 26)
lienzo-referencia-menu = 参考图片…
lienzo-pegar-menu = 粘贴
lienzo-pista-compartir = 分享… (Ctrl+Shift+S)
lienzo-pista-lupa = 放大镜 (Q)：在画布上点击封闭形状，可放大显示其下方内容；在屏幕上，放大光标下方区域
lienzo-pista-mosaico = 像素化（马赛克）：拖过想要隐藏的内容
lienzo-pista-zona = 区域 (Z)：拖出一个框，得到可移动的裁剪副本
lienzo-panel-mosaico = 遮盖
lienzo-panel-aumento = 放大倍数
lienzo-panel-guia = 指针
# The spotlight (Propiedad.OSCURECER and Propiedad.ZONA on the phone)
lienzo-panel-oscurecer = 变暗
lienzo-panel-zona-foco = 照亮区域
lienzo-pista-foco = 聚光灯 (F)：点击封闭形状，其周围区域（画框内）会变暗
lienzo-pista-serie = 编号步骤：每次点击放置下一个编号

## The Chat / Functions switch in the chat title bar
proyectos-interruptor-chat = 聊天
proyectos-interruptor-funciones = 功能
## The Functions screen: its tiles, in order
funciones-titulo = 功能
funciones-grabar = 录制会议
funciones-grabar-detalle = 课堂或会议，含截图和转写
funciones-lienzo = 新建画布
funciones-lienzo-detalle = 用于绘图或描摹的空白画布
funciones-tareas = 任务
funciones-tareas-detalle = 所有聊天中的任务列表
funciones-timeline = 时间线
funciones-timeline-detalle = 你每分钟的一天，以及你的课程
funciones-horario = 课程表
funciones-horario-detalle = 本周的课程，并提前提醒
funciones-galeria = 截图
funciones-galeria-detalle = 你保存的所有截图
funciones-figuras = 形状
funciones-figuras-detalle = 用于画布的形状库
figuras-mias = 我的形状
figuras-todas = 全部
figuras-vacia = 还没有形状。在画布上选中内容，然后选择“保存为形状”。
figuras-borrar = 删除形状
figuras-borrar-seguro = 再按一次以删除
figuras-elegir-para = 为“{ $chat }”选择形状
figuras-adjuntar-a = 附加到“{ $chat }”
figuras-adjuntar-pista = 也可以双击形状或按 Enter。
figuras-adjuntada = 已附加到“{ $chat }” ✓
figuras-param-titulo = 参数
figuras-param-por-hoja = 每页
figuras-param-ano = 年份
figuras-param-restablecer = 恢复默认
figuras-zoom-pista = 双指捏合或 Ctrl + 滚轮缩放 · 双击适应窗口
asistente-adj-figura = 形状
asistente-adj-pedir-figura = 请求新形状
asistente-pide-figura = 帮我做一个这样的形状：
asistente-mejora-figura = 改进形状“{ $figura }”：
chat-guardar-en-figuras = 保存到我的形状
chat-figura-sin-nombre = 来自 { $chat } 的形状
chat-figura-guardada = 已保存到你的形状，名为“{ $nombre }”
chat-figura-vacia = 该画布为空：没有可保存的内容
figuras-titulo = 形状
figuras-buscar = 搜索形状…
# The ☀/☾ switch of the shape panels: day templates or their «(night)» twins.
figuras-sin-resultados = 没有形状的名称或文字中包含该内容。
figuras-de-serie = 内置
figuras-tuya = 你的
figuras-mide = 尺寸
figuras-piezas = 部件
figuras-textos = 包含的文字
figuras-lineas = { $n } 条线
figuras-formas = { $n } 个形状
figuras-de-texto = { $n } 段文字
figuras-trazos = { $n } 笔手绘
figuras-otras = { $n } 个其他
bandeja-figuras = 形状…

## --- Canvas: graph and table dialogs, put into a cell (F12/F14) ---
cajetin-cancelar = 取消
grafica-insertar = 插入
figuras-guardar = 保存
figuras-editar-tabla = 编辑表格… (Enter)
figuras-meter-en-celda = 将所选内容放入其单元格
tabla-blanco-titulo = 空白表格
tabla-pegada-titulo = 粘贴的表格
tabla-editar-titulo = 编辑表格
tabla-insertar = 插入
tabla-aplicar = 应用
tabla-mas-fila = + 行
tabla-menos-fila = − 行
tabla-mas-columna = + 列
tabla-menos-columna = − 列
tabla-cabecera = 表头
tabla-pegar = 粘贴
tabla-ayuda = Tab 和方向键：切换单元格 · − 删除当前单元格所在的行或列 · Enter：确定 · Esc：取消

## --- Canvas: the Zone to the project chat (F8, mandarLaZona on the phone) ---
zona-arrastra-copia = 拖动：生成副本
zona-arrastra-chat = 拖动：发送到项目聊天
zona-al-chat = 发到聊天
zona-mandada = 区域已发送到“{ $proyecto }”聊天
zona-no-mandada = 无法发送区域
zona-viene-pdf = PDF“{ $proyecto }”→ 第 { $pagina } 页
zona-viene-lienzo = 画布“{ $nombre }”
zona-nombre-pagina = 第 { $pagina } 页的区域
zona-nombre-lienzo = { $nombre } 的区域
zona-lienzo = 画布
zona-hoja-lienzo = 画布
# Linked zone that leads to no canvas (salto_por_enlace).
zona-enlace-roto = 此区域未链接到此项目中的任何画布

## The canvas toolbar grouped like the phone's (GRUPOS_DE_FABRICA)
herramientas-mano = 选择和移动
herramientas-imprimir = 打印
herramientas-compartir = 分享
barra-grupo-elegir = 其他选择方式
barra-grupo-trazar = 手绘
barra-grupo-formas = 形状
barra-grupo-flechas = 箭头和线条
barra-grupo-arreglar = 修整已绘内容
barra-grupo-nombrar = 文本和标签
barra-grupo-tapar = 遮盖和指示
barra-grupo-medir = 测量
barra-grupo-marco = 画框和图片
barra-grupo-sacar = 分享和打印
barra-pista-mano = 选择和移动 (M)
barra-pista-deshacer = 撤销 (Ctrl+Z)
barra-pista-rehacer = 重做 (Ctrl+Y)
barra-pista-salir = 退出 (Esc)
ajustes-herramientas-sueltas = 始终可见
ajustes-herramientas-donde = 工具位置
ajustes-herramientas-donde-ayuda = 选择一个位置，关闭你在那里不用的工具，而不影响其他位置。
ajustes-herramientas-sitio-todos = 所有位置
ajustes-herramientas-sitio-pantalla = 屏幕
ajustes-herramientas-sitio-pin = 贴图
ajustes-herramientas-sitio-lienzo = 画布
ajustes-herramientas-sitio-lector = 阅读器

## PDF reader: «To project» (the notes move to a project's sheets and travel when syncing)
lector-al-proyecto = 转为项目
lector-al-proyecto-hecho = 已加入项目，并包含你的笔记
lector-al-proyecto-fallo = 无法创建项目

## «Save» in the viewers of files opened from outside (passing mode, Android 15a52aed)
guardar-de-paso = 保存到聊天
guardar-de-paso-guardados = 保存到“收藏的消息”
guardar-de-paso-en = 保存到“{ $chat }”
guardar-de-paso-hecho = 已保存到“{ $chat }”，并包含你的笔记
guardar-de-paso-fallo = 无法保存到聊天

## Word and book reader: the four types and four weights of the phone (K16)
lector-letra-serif = 衬线体
lector-letra-sans = 无衬线体
lector-letra-cursiva = 手写体
lector-grosor-fina = 细体
lector-grosor-negra = 特粗

## Screen annotator: its pill (CapaPantalla.kt on the phone) and saving to «Saved messages»
anotador-rotulo-dibujando = Alt + 双击中键：点击穿透
anotador-rotulo-atravesando = Alt + 双击中键：重新绘制
anotador-globo-titulo = 已标注的屏幕
anotador-globo-guardado = 已保存到“收藏的消息”
anotador-globo-fallo = 无法保存到“收藏的消息”
anotador-globo-copiado = 已复制到剪贴板
# Screen annotator: click-through in the toolbar (2026-09-29)
anotador-boton-atravesar = 点击穿透
anotador-pista-atravesar = 点击穿透（空格）：鼠标操作作用于下方内容，绘图保持可见。点击胶囊上的手形图标或 Alt + 双击中键即可返回

# Share sheet: a document PDF with its annotations or clean (2026-09-29)
compartir-con-anotaciones = 含标注
compartir-con-anotaciones-detalle = 在 PDF 上叠加墨迹、页边和书签；关闭时为干净的 PDF

# Share sheet: a PDF web page as lines or as images (2026-09-30, Android v0.98.1)
compartir-texto-buscable = 可搜索文字
compartir-texto-buscable-detalle = 页面以带文字的行呈现；关闭时为图片

# Secret call: who is calling and call back later (2026-09-30, Android v0.98.6)
chat-quien-llama = 来电者：{ $nombre }
chat-quien-llama-barra = 来电者：
chat-quien-llama-teclas = Enter：选择时间 · Esc：取消
llamada-volver-en = 稍后回拨
llamada-vuelve-a-las = 将在 { $hora } 回拨给你

## H12: editing without marks, the floating toolbar and the note font (Sep 30)
nota-md-texto-normal = 正文
nota-md-quitar-formato = 清除格式
nota-md-enlace-pista = 粘贴或输入链接后按 Enter
nota-md-emojis = 微笑|大笑|超爱|还好|赞|踩|谢谢|鼓掌|派对|完成|不|警告|爱心|火|星星|灵感
nota-md-letra-texto = 正文字体
nota-md-letra-titulos = 标题字体
nota-md-tamano = 字号
nota-md-tamanos = 小|正常|大|特大
nota-md-solo-esta-nota = 仅此笔记
nota-md-pista-tamano = Ctrl + 滚轮可逐级调整

# --- Notes: embedded documents, chat messages and audio (H12, Oct 1) ---
nota-md-documento = 文档
nota-md-del-chat = 来自聊天
nota-md-audio = 音频
nota-md-abrir = 打开
nota-md-quitar = 从笔记中移除
nota-md-ir-al-mensaje = 转到聊天中的消息
nota-md-mensaje-borrado = 已删除的消息
nota-md-no-esta = 不在此电脑上
nota-md-pasar-a-texto = 转写
nota-md-pasando-a-texto = 正在转写 { $pct } %
nota-md-sin-mensajes = 此项目的聊天中没有可插入的消息（独立笔记没有聊天）。
nota-md-nota-de-voz = 语音消息

## Export a note to Word (H12, 1-oct)
nota-md-exportar-word = 导出为 Word…
nota-md-tipo-word = Word 文档
compartir-word = Word

## Requests from other programs (Flow Launcher, scripts; docs/protocolo-pedidos.md)
pedido-tareas = 任务
pedido-escrito = 已写入“{ $proyecto }”。
pedido-al-lienzo = 已堆放到画布“{ $lienzo }”上。
microfono-abriendo = 正在打开麦克风…
microfono-guardando = 正在保存…
microfono-parar = 点击停止 · Esc 放弃
microfono-guardada = 语音消息已保存
microfono-convertir = 转为来电
microfono-listo = 完成
microfono-cuando = 什么时候给你打电话？
microfono-en-15-min = 15 分钟后
microfono-en-1-hora = 1 小时后
microfono-manana = 明天 9:00
microfono-poner = 设置来电
microfono-volver = 返回
microfono-sonara = 将在以下时间给你打电话：{ $cuando }
microfono-puesta = 完成：将在 { $cuando } 给你打电话
microfono-teclas = 输入时间（18:30）或使用方向键 · Enter 设置
soltar-aqui = 拖放到此处以添加到“{ $proyecto }”
soltar-anadiendo = 正在添加…
soltar-anadido-uno = 已向聊天添加 1 项
soltar-anadidos = 已向聊天添加 { $n } 项
soltar-ninguno = 无法添加任何内容
chat-anadir-arrastrando = 拖动以添加…
pedido-lista-hecha = 已在“{ $proyecto }”中创建列表“{ $lista }”。
pedido-tabla-hecha = 已在“{ $proyecto }”中创建表格“{ $tabla }”。
pedido-tarea-anadida = 已将“{ $tarea }”添加到“{ $lista }”。
pedido-tarea-hecha = “{ $tarea }”已标记为完成。
pedido-tarea-pendiente = “{ $tarea }”已重新设为待办。
pedido-roto = 来自其他程序的请求缺少内容；未执行任何操作。
pedido-sin-proyecto = 该项目已不在 PinDesk 中。
pedido-sin-mensaje = 该消息已不在聊天中。
pedido-no-es-lista = 该消息不是待办清单。
pedido-sin-tarea = 列表中没有第 { $indice } 项任务。
pedido-tarea-vacia = 不会添加没有文字的任务。
pedido-sin-fichero = 该文件已不在磁盘上。
pedido-fallo-disco = 无法保存所请求的内容。请查看 PinDesk 日志。
pedido-tarea-movida = 已将“{ $tarea }”移到“{ $lista }”。
pedido-tarea-cambio = 移动任务时列表发生了变化；未做任何改动。请重新查找。
pedido-tarea-quitada = 已将“{ $tarea }”从其列表中移除。
pedido-tarea-corregida = 任务已更改：“{ $tarea }”。
pedido-lista-borrada = 已从“{ $proyecto }”中删除列表“{ $lista }”。
pedido-timeline-apuntado = 已添加到时间线：“{ $titulo }”。
pedido-sin-capturas = 还没有截图。
pedido-fuera-de-capturas = 该文件不在截图文件夹中，不做处理。
pedido-no-es-imagen = 该文件不是图片。
pedido-imagen-no-se-pudo = 无法复制图片：{ $motivo }
pedido-sin-imagen = 找不到图片 { $ruta }；未添加任务。
pedido-nada-que-leer = 没有收到要朗读的文字。
pedido-ocr-copiado = 文字已复制（{ $n } 个字符）：“{ $texto }”
pedido-ocr-sin-texto = 图片中未找到文字。
pedido-ocr-no-disponible = 未安装 Windows OCR：请在“设置”中添加支持文字识别的语言。
pedido-ocr-fallo = 无法读取图片中的文字：{ $motivo }

## Command line and pindesk:// links (Oct 15)
orden-titulo = PinDesk
orden-confirmar-cabecera = 一个链接请求 PinDesk：
orden-confirmar-pregunta = 是否执行？
orden-motivo-ficheros = 它会打开或发送你磁盘上的文件。
orden-motivo-asistente = 它会向助手提问，助手可以在 PinDesk 中执行操作。
orden-motivo-microfono = 它会用麦克风录音。
orden-motivo-otros-aparatos = 它会向你的其他设备发送数据。
orden-motivo-automatizacion = 它会改动助手的自动化。
orden-motivo-borra = 它会删除内容。
orden-motivo-cambia = 它会更改 PinDesk 数据。
orden-prohibido-ruta-no-local = 链接不能打开网络文件或此电脑以外的路径。未执行任何操作。
orden-prohibido-crear-automatizacion = 链接不能创建自动化。未执行任何操作。
orden-no-se-pudo = PinDesk 无法执行：{ $motivo }
orden-sin-app = PinDesk 未打开且无法启动：{ $motivo }
orden-sin-respuesta = PinDesk 没有响应。如果卡住了，请关闭后重新打开。
orden-version = 此 PinDesk 版本低于请求所需：请更新。
orden-preguntar-titulo = 向助手询问{ $cuantos ->
    [1] { $nombre }
   *[other] { $cuantos } 个文件
}
orden-preguntar-enviar = 询问
orden-preguntar-cancelar = 取消

## Capture gallery (2-oct)
galeria-titulo = 截图
galeria-vacia = 还没有截图。用 Alt + 拖动复制的截图将显示在这里。
galeria-abrir-carpeta = 打开文件夹
galeria-copiada = 已复制到剪贴板
galeria-pineada = 已贴图
galeria-borrada = 已移到 PinDesk 回收站
galeria-no-se-pudo = 操作失败：{ $motivo }
galeria-conservada-aviso = 已保留：它在“收藏的消息”中，不会被删除
bandeja-galeria-capturas = 截图库…
# Lessons learned (3-oct, as on the phone)
chat-hacer-leccion = 记为心得
bandeja-leccion-nueva = 新建心得…
bandeja-tareas = ☑ 任务…
bandeja-timeline = 🕒 时间线和心得…

## Tareas (ventana)
tareas-titulo = 任务
tareas-apuntar-boton = 添加
tareas-hechas = 已完成（{ $n }）
tareas-vacia = 还没有任务。在上方输入并按 Enter：任务会进入收件箱，再从那里移到对应的分组。
tareas-cambio = 列表在此期间已更改，现已更新。请重新勾选。
tareas-mover = 移到…
tareas-mover-titulo = 将“{ $tarea }”移到…
tareas-sin-grupos = 还没有其他任务分组。在聊天中创建一个待办清单，它就会显示在这里。
tareas-movida = 已移到 { $lista }
tareas-imagen-no-esta = 该图片还不在此电脑上：将随同步到达。
tareas-pegar-no-imagen = 任务中只能粘贴图片或文字。
tareas-pegar-repetida = 该图片已作为 { $ficha } 添加。

# Opening a photo that Windows can only read with a Microsoft Store extension.
abrir-falta-extension = 无法打开 { $nombre }：Windows 需要“{ $extension }”，可在 Microsoft Store 免费获取。安装后请重新打开该文件。

## v2-menus
v2menus-responder = 回复
v2menus-anotar-encima = 在上面标注
v2menus-mas = 更多
v2menus-elegir-varios = 多选
v2menus-tecla-supr = Del
v2menus-tecla-clic = 点击
v2menus-adj-buscar = 搜索：表格、秒表、PDF…
v2menus-adj-mas = 更多
v2menus-adj-nada = 没有符合搜索的内容
v2menus-adj-lienzo = 画布
v2menus-adj-tareas = 待办清单
v2menus-adj-tabla = 表格
v2menus-adj-conversacion = 对话
v2menus-adj-captura = 截图
v2menus-adj-pagina = 项目页面
v2menus-adj-proyecto = 整个项目
v2menus-adj-movil = 从手机获取
v2menus-adj-pegar-copiado = 粘贴已复制的内容
v2menus-envio-enviar = 发送
v2menus-envio-anadir = 添加
v2menus-envio-fotos = { $cuantos ->
   *[other] { $cuantos } 张照片
} · 发送到 { $proyecto }
v2menus-envio-archivos = { $cuantos ->
   *[other] { $cuantos } 个文件
} · 发送到 { $proyecto }
## v2-ajustes
ajustes2-titulo = 设置
ajustes2-buscar = 搜索设置…
ajustes2-nota = 点击“完成”或按 Esc 关闭时保存。
ajustes2-nota-punto = = 与默认值不同。
ajustes2-deshacer = 撤销
ajustes2-listo = 完成
ajustes2-restablecer = 重置 { $seccion }
ajustes2-resultados = “{ $busqueda }”的搜索结果
ajustes2-resultados-sub = { $n ->
   *[other] 在所有分区中找到 { $n } 项设置。
  }
ajustes2-sin-resultados = 没有设置提到“{ $busqueda }”。请换个词试试。
ajustes2-sec-general = 常规
ajustes2-sec-general-sub = PinDesk 如何启动，以及如何处理聊天中的 PDF。
ajustes2-sec-captura = 截图
ajustes2-sec-captura-sub = 如何截图以及截图之后的处理。
ajustes2-sec-lienzo = 画布和工具
ajustes2-sec-lienzo-sub = 吸附、手绘笔画，以及工具栏中显示哪些工具。
ajustes2-sec-voz = 语音和转写
ajustes2-sec-voz-sub = 将双语语音消息转为文字。
ajustes2-sec-atajos = 快捷键
ajustes2-sec-atajos-sub = 点击一个框，然后按下按键。Delete 清除快捷键；Esc 取消。
ajustes2-sec-sincro = 同步
ajustes2-sec-sincro-sub = 这台电脑如何找到手机。
ajustes2-sec-apps = 默认应用
ajustes2-sec-apps-sub = 从文件资源管理器用 PinDesk 打开图片和视频。
ajustes2-sec-apariencia = 外观
ajustes2-sec-apariencia-sub = 聊天的外观。
ajustes2-sec-avanzado = 高级
ajustes2-sec-avanzado-sub = 性能、笔迹过滤和设置文件。
ajustes2-g-arranque = 启动时
ajustes2-g-pdf = PDF 文档
# Text recognition (OCR) for the whole app: Windows or Indra (docs/ocr.md).
ajustes2-g-ocr = 文字识别（OCR）
ajustes2-ocr-motor = OCR 引擎
ajustes2-ocr-windows = Windows
ajustes2-ocr-indra = Indra
ajustes2-ayuda-ocr = 用于文字截图、“描摹此处”、贴图、截图库、扫描版 PDF 和助手。Windows 随系统提供。Indra（Datalab）对照片、扫描件、倾斜页面和小字的识别效果好得多，在你的电脑上离线运行；选择它将下载 { $megas } MB。其权重采用 Datalab 的 OpenRAIL-M 许可：个人使用、研究以及营收低于 2000 万美元的公司可免费使用；其他商业用途需要其授权。
ajustes2-ayuda-ocr-indra-listo = Indra 已下载，并负责识别应用中的所有文字。如果出错，将改用 Windows OCR。
ajustes2-ayuda-ocr-windows-con-indra = 当前由 Windows OCR 识别。Indra 仍已下载：选择它即可使用，或在下方删除。
ajustes2-ocr-bajando = 正在下载 Indra… { $por-ciento } %。在此期间由 Windows OCR 识别。
ajustes2-ocr-fallo = 无法下载 Indra：{ $fallo }。再次选择即可重试；在此期间由 Windows OCR 识别。
ajustes2-ocr-licencia = Indra 许可
ajustes2-ayuda-ocr-licencia = Indra 的代码采用 Apache 2.0；其权重采用 Datalab 修改版 OpenRAIL-M。PinDesk 不附带这些权重：选择 Indra 时会从 Hugging Face 下载。
ajustes2-ocr-corregir = 用词典校正
ajustes2-ayuda-ocr-corregir = 仅适用于 Indra 和西班牙语页面：修正重音符号和低置信度的字母。不会改动数字、代码或名称。
ajustes2-ocr-vocabulario = 我的词汇
ajustes2-ocr-vocabulario-ninguno = 无。Indra 应优先识别的名称、缩写或术语（E030、Arequipa…）。
ajustes2-ocr-vocabulario-marcador = 术语, 另一个术语…
ajustes2-ocr-borrar = 删除 Indra
ajustes2-ayuda-ocr-borrar = 释放其占用的 26 MB 磁盘空间；重新由 Windows OCR 识别。
ajustes2-ocr-borrar-boton = 删除
# Transcribe with Whisper or Qwen3-ASR (25-oct-2026, docs/voz-qwen3.md)
ajustes2-voz-motor = 转写引擎
ajustes2-voz-motor-whisper = Whisper base
ajustes2-voz-motor-qwen3 = Qwen3-ASR
ajustes2-ayuda-voz-motor = 用于语音消息以及课堂或会议录音。Whisper base 为内置（160 MB，速度快）。Qwen3-ASR 对西班牙语、会议和课堂的识别效果好得多（错误减少一半或更多）；选择它将下载 { $megas } MB，转写时需要约 { $ram } GB 内存，耗时约为 Whisper 的四倍。许可：{ $licencia }。
ajustes2-ayuda-voz-motor-qwen3-listo = Qwen3-ASR 已下载（{ $total } MB，许可 { $licencia }），负责转写消息和录音。转写时需要约 { $ram } GB 内存；如果出错，将改用 Whisper。
ajustes2-ayuda-voz-motor-whisper-con-qwen3 = 当前由 Whisper 转写。Qwen3-ASR 已下载：选择它即可使用，或在下方删除。
ajustes2-ayuda-voz-motor-sin-memoria = 当前由 Whisper 转写。Qwen3-ASR 需要 4 GB 或以上内存的电脑。
ajustes2-voz-motor-bajando = 正在下载 Qwen3-ASR… { $por-ciento } %。在此期间由 Whisper 转写。
ajustes2-voz-motor-fallo = 无法下载 Qwen3-ASR：{ $fallo }。再次选择即可重试；在此期间由 Whisper 转写。
ajustes2-voz-qwen3-licencia = Qwen3-ASR 模型卡
ajustes2-ayuda-voz-qwen3-licencia = Qwen3-ASR-0.6B 由 Qwen（阿里巴巴）开发，采用 Apache-2.0 许可：可免费使用，包括工作场景。
ajustes2-voz-qwen3-borrar = 删除 Qwen3-ASR
ajustes2-ayuda-voz-qwen3-borrar = 释放其占用的 988 MB 磁盘空间；消息将重新由 Whisper 转写。
ajustes2-g-al-capturar = 截图时
ajustes2-g-gif = GIF 录制
ajustes2-g-guardar = 保存和清理
ajustes2-g-pila = 截图堆叠
ajustes2-g-ignorados = 忽略的程序
ajustes2-g-regiones = 已保存的区域
ajustes2-g-iman = 吸附
ajustes2-g-trazo = 手绘
ajustes2-g-voz = 语音消息
ajustes2-g-atajos = 全局快捷键
ajustes2-g-sincro = 在局域网中
ajustes2-g-apps = 打开图片和视频
ajustes2-g-tema = 主题
ajustes2-g-rendimiento = 性能
ajustes2-g-lapiz = 笔迹过滤（自然模式）
ajustes2-g-fichero = 设置文件
ajustes2-ayuda-arranque = 驻留在系统托盘中，不打开任何窗口。便携模式下不可用。
ajustes2-ayuda-idioma = 下次启动时生效。
ajustes2-g-ventanas = 窗口
ajustes2-escala = 界面缩放
ajustes2-ayuda-escala = 聊天中所有内容的大小。立即生效；在聊天中也可用 Ctrl + “+”、“-”和“0”调整。
ajustes2-grupos-ventanas = 窗口组
ajustes2-ayuda-grupos-ventanas = 将打开的窗口保存为一组，之后可一起重新打开。
ajustes2-ayuda-pdf-aligerar = 进入聊天后在后台进行。
ajustes2-pdf-nivel = 压缩程度
ajustes2-ayuda-pdf-nivel = 自动压缩时仅使用“无损”和“中等”；“小”和“最大”用于手动“压缩 PDF”。
ajustes2-retardo = 延时截图
ajustes2-ayuda-retardo = 截图前的倒计时，便于打开菜单或提示框。
ajustes2-color = 颜色格式
ajustes2-ayuda-color = 取色器复制颜色的方式。
ajustes2-scroll = 最大滚动高度
ajustes2-ayuda-scroll = 长截图的上限，避免无限长的页面耗尽内存。
ajustes2-gif-ritmo = 每秒帧数
ajustes2-ayuda-gif-ritmo = 越少文件越小。教程用 10 或 15 就足够了。
ajustes2-gif-retardo = 录制前延时
ajustes2-ayuda-gif-retardo = 从按下“录制”到第一帧之间的秒数。
ajustes2-carpeta = 截图文件夹
ajustes2-abrir = 打开
ajustes2-caducidad = 删除前的天数
ajustes2-ayuda-caducidad = 到期后将移到 Windows 回收站，已保留的除外。
ajustes2-apilar = 在角落堆叠
ajustes2-ayuda-apilar = 每次截图都会在角落留下一个方框；点击它即可收集后续截图，并用一次 Ctrl+V 全部粘贴。
ajustes2-esquina = 方框位置
ajustes2-ayuda-esquina = 在截图所在的显示器上。
ajustes2-icono = 屏幕显示时间
ajustes2-ayuda-icono = 最后一次截图后，未激活的方框保留多久。
ajustes2-ayuda-ignorado = 当此程序位于前台时，PinDesk 快捷键不起作用。
ajustes2-anadir-programa = 添加程序
ajustes2-ayuda-anadir-programa = 输入可执行文件名并按 Enter。
ajustes2-anadir = 添加
ajustes2-ayuda-region = { $x }, { $y } · { $ancho } × { $alto } px
ajustes2-ayuda-iman = 绘图时，光标会吸附到已有的内容上。
ajustes2-ayuda-iman-esquinas = 形状的角点和线条的端点。
ajustes2-ayuda-iman-medios = 每条边的中点。
ajustes2-ayuda-iman-centros = 每个形状的中心。
ajustes2-iman-radio = 吸附距离
ajustes2-ayuda-iman-radio = 以屏幕像素计。
ajustes2-suavizado = 笔画平滑
ajustes2-ayuda-suavizado = “自然”可消除手静止时的抖动，且笔尖不延迟。
ajustes2-ayuda-voz-segundo = Whisper 也能识别同一条消息中的这种语言。
ajustes2-ayuda-voz-modo = 如何处理用第二语言说的内容。
ajustes2-presencia = 允许手机找到我
ajustes2-ayuda-presencia = 手机打开“同步”后即可看到这台电脑。关闭后，不会在网络上打开任何端口。
ajustes2-lo-mio = 以我为准
ajustes2-ayuda-lo-mio = 同步时以这台电脑为准：只有此处不存在的内容才会从对方获取。
ajustes2-vista-rapida = 用空格键快速预览
ajustes2-ayuda-vista-rapida = 在文件资源管理器或桌面上选中文件并按空格：会立即在 PinDesk 中打开（照片、视频、PDF、Word、DWG 图纸、IFC 和 Revit 模型…）。再按一次空格关闭。
ajustes2-abrir-con = 为图片和视频提供 PinDesk 选项
ajustes2-ayuda-abrir-con = 显示在“打开方式”和“默认应用”中。关闭后会移除写入注册表的内容。
ajustes2-predeterminada = 默认应用
ajustes2-ayuda-predeterminada = Windows 只允许在“设置”中选择：在那里选择 PinDesk → 设为默认值。
ajustes2-hacer-predeterminada = 将 PinDesk 设为默认应用…
ajustes2-ayuda-cosmos = 聊天项目列表后方的星空。从下一个聊天开始生效。
ajustes2-nivel = 性能级别
ajustes2-ayuda-nivel = “自动”会在启动时检测电脑性能。下次启动时生效。
ajustes2-medir = 测量编辑器帧耗时
ajustes2-ayuda-medir = 记录每帧耗时。仅用于诊断。
ajustes2-ritmo = 在刷新前一刻绘制
ajustes2-ayuda-ritmo = 笔尖延迟更小；计算未赶上时会丢失一次刷新。
ajustes2-paneo = 滚动时不重绘
ajustes2-ayuda-paneo = 由 Windows 合成器移动和缩放画布。关闭后每帧都会重绘。
ajustes2-corte = 笔静止时的截止频率
ajustes2-ayuda-corte = 越低 = 抖动越小，起笔延迟越大。自动 = 1 Hz。
ajustes2-beta = 跟随速度的程度
ajustes2-ayuda-beta = 越高 = 笔尖越贴近光标，抖动越大。自动 = 0.007。
ajustes2-fichero = 设置文件
ajustes2-ayuda-fichero = 这里没有的设置仍保留在文件中，连同其注释。手动编辑在重启后生效。
ajustes2-abrir-fichero = 打开文件
ajustes2-u-s = { $n } 秒
ajustes2-u-px = { $n } px
ajustes2-u-dias = { $n ->
   *[other] { $n } 天
  }
ajustes2-u-hz = { $n } Hz
ajustes2-nunca = 从不
ajustes2-siempre = 始终
ajustes2-auto = 自动
## v2-buscar
comando-buscar = 🔍 搜索 PinDesk…
comando-grabar-clase = 🎙 录制课堂或会议…
clase-titulo = 录制课堂或会议
clase-microfono = 麦克风
clase-equipo = 电脑声音（Meet、Zoom…）
clase-guardar-en = 保存到：{ $proyecto }
clase-empezar = 开始录制
clase-nada = 请选择麦克风、电脑声音或两者
clase-captura = 截图
clase-terminar = 结束
clase-capturas = 截图：{ $n } · 单击即可重复截取该区域
clase-elige-zona = 在屏幕上选择区域…
clase-captura-hecha = 已在 { $minuto } 保存截图
clase-zona-cambiada = 区域已更改
clase-en-pausa = 已暂停：未在录制
clase-tirar = 再次按 ✕ 放弃录制
clase-sin-zona = 无法截取该区域
clase-abriendo = 正在打开音频…
clase-guardando = 正在保存课堂…
clase-texto = 正在转写… { $avance } %
clase-bajando = 正在下载识别器（仅首次）… { $avance } %
clase-lista = 已保存到“{ $proyecto }”
clase-lista-con-texto = 包含 { $n } 张截图和转写文本。在聊天中打开即可与图片对照查看。
clase-lista-sin-texto = 包含 { $n } 张截图。如需文字，请在聊天中对该消息点击“转为文字”。
clase-sin-texto = 不转文字
clase-listo = 完成
clase-nombre = 课堂 { $fecha }
clase-sin-sonido-equipo = 电脑没有声音：没有启用的扬声器或耳机
clase-video = 此屏幕的视频
clase-video-roto = 视频已停止；音频仍在录制
clase-video-guardado = 包含屏幕视频。
clase-calidad-ligero = 轻量
clase-calidad-normal = 标准
clase-calidad-nitido = 清晰
clase-calidad-ligero-pista = 720p 10 fps 视频：文件最小（每小时约 180 MB）
clase-calidad-normal-pista = 1080p 15 fps 视频：每小时最多约 300 MB
clase-calidad-nitido-pista = 原始屏幕 30 fps：小字仍清晰可读，文件更大
clase-video-con = 视频编码：{ $codificador }
clase-codificador-grafica = 显卡（几乎不占用处理器）
clase-codificador-software = 处理器
clase-codificador-cpu = 处理器（简易模式）
clase-zona-perdida = 未选择区域：请再次按“截图”
clase-salvado = 其余内容已保存：{ $n } 张截图
clase-disco-poco = 磁盘空间即将不足（少于 2 GB）
clase-disco-lleno = 磁盘快满了：视频已关闭，音频仍在录制
clase-suspendido = 电脑休眠了 { $minutos } 分钟；唤醒后录制已继续
clase-recuperar-titulo = 录制中断
clase-recuperar-pregunta =
    “{ $chat }”中有一次录制被中断（{ $nombre }，{ $minutos } 分钟）。
    是：恢复 · 否：放弃 · 取消：下次打开 PinDesk 时再询问
clase-recuperar-sin-nada = “{ $nombre }”只剩下一个无法打开的视频。删除它？
clase-recuperar-descartar = 放弃“{ $nombre }”？录制将被删除且无法恢复。
clase-recuperar-fallo = 无法恢复“{ $nombre }”。录制仍然安全，下次打开 PinDesk 时会再次提示。
clase-texto-de-fondo = 关闭（继续进行）
clase-abrir-para-estudiar = 打开学习
clase-camara-movil = 📱 手机摄像头
clase-camara-movil-n = 📱 手机摄像头 · { $n }
clase-camara-movil-apagada = 📱 手机摄像头：关闭
clase-foto-movil = 来自手机的照片 · { $minuto }
clase-clip-movil = 来自手机的片段 · { $minuto }
camara-movil-tarde = 从手机到“{ $titulo }” · { $minuto }
# Photos by their time, from the PC to the phone's recording and collaborators (28-oct-2026).
aporte-captura-al-movil = 为手机录制截图
aporte-de-la-grabacion = { $movil } 正在录制 · { $minuto }
aporte-enviada = 已发送到手机 · { $minuto }
aporte-pendiente = 离线：恢复连接后发送
aporte-fallo = 无法截取屏幕
chat-adjuntar-a-la-grabacion = 附加到录制…
chat-adjuntada-a-la-grabacion = 在录制中 · { $minuto }
chat-sin-grabacion = 此聊天中没有录制
chat-colaboradores-todos = 允许聊天中的所有人添加照片
chat-colaboradores-nadie = 此录制中仅限我的照片
clase-hora-sin-comprobar = 时间未验证
baliza-movil-aislado = 虽然你们在同一网络中，但找不到你的手机：此 Wi-Fi 可能隔离了设备（访客、酒店和办公网络常这样），或防火墙阻止了 PinDesk。请将电脑连接到手机热点，或让两者连接另一个 Wi-Fi。你拍的照片会保存在手机上，双方找到彼此后按其时间点加入。
baliza-movil-otra-red = 你的手机与这台电脑不在同一网络。请将两者连接到同一 Wi-Fi（或将电脑连接到手机热点）。你拍的照片会保存在手机上，双方找到彼此后按其时间点加入。
# In-person meeting (27-oct-2026): the computer's camera, photos and clips.
clase-camara = 摄像头：{ $camara }
clase-camara-buscando = 正在查找摄像头…
clase-camara-ninguna = 没有摄像头：“拍照”和“片段”不可用（仍会录制音频）
clase-foto = 拍照
clase-foto-hecha = 已在 { $minuto } 保存照片
clase-camara-abriendo = 正在打开摄像头…
clase-camara-lista = 摄像头就绪：照片尺寸 { $medida }
clase-camara-sin = 未连接摄像头
clase-camara-en-uso = 另一个应用正在使用摄像头（Teams、Zoom…）：请关闭后重试
clase-camara-sin-permiso = Windows 不允许访问摄像头：请在“设置 › 隐私 › 相机”中开启
clase-camara-fallo = 摄像头没有响应；请重试
clase-clip-empezado = 正在用摄像头录制片段… 按 ■ 停止
clase-clip-guardado = 已在 { $minuto } 保存片段
clase-clip-corto = 无法结束片段
# A single recording (29-oct-2026): no modes; Photo, Capture, Screen and Clip on the bar,
# the clock that pauses and ends, and full screen with the topics and the transcript.
clase-unica-sub = 始终录音；工具栏上有拍照、截图、屏幕和片段
clase-pantalla-al-empezar = 开始时录制屏幕
clase-pantalla = 屏幕
clase-clip = 片段
clase-pantalla-encendida = 正在录制屏幕；音频继续录制
clase-pantalla-apagada = 屏幕录制已关闭；音频仍在录制
clase-pista-unica = 点击时钟：暂停 · 按住：结束 · 按住“截图”：选择区域
clase-imagenes = { $n } 张图片和片段
clase-completa-temas = 我要讲的内容
clase-completa-sin-temas = 没有议题：请在开始前于“会议议题”中添加
clase-completa-dice = 正在说的内容
clase-completa-sin-frases = 正在聆听… 句子一说出就会显示在这里
clase-completa-sin-escucha = 实时转写需要 Jev（TypeSafe）密钥和语音识别器；完整文本会在结束后生成
clase-descartar = 放弃
clase-descartar-otra = 再次按“放弃”以删除录制
estudiar-clip = 摄像头视频 · { $duracion }
bandeja-grabando = PinDesk — 正在录制…
salir-grabando-titulo = 正在录制会议
salir-grabando-mensaje =
    退出前结束并保存录制？
    是：结束、保存到对应聊天并退出（稍后可在该消息上获取文字）。
    否：继续录制。
chat-estudiar-clase = 学习课堂
estudiar-titulo = 学习课堂
estudiar-transcripcion = 转写文本
estudiar-capturas = 截图
estudiar-marcadores = 书签
estudiar-marcador = 书签
estudiar-seguir = 跟随
estudiar-pista-teclas = 空格播放 · ←/→ 5 秒 · J/L 10 秒 · M 书签
estudiar-nota-del-marcador = 输入备注并按 Enter（Esc 取消）
estudiar-sin-marcadores = 按“书签”或 M 记录此时间点的内容。
estudiar-marcador-sin-nota = 书签
estudiar-repasar = 出声复习
repaso-titulo = 出声复习
repaso-cabecera = 出声复习 · { $clase }
repaso-explicacion = 用自己的话讲解这堂课，就像讲给别人听。Jev 会标记每个要点：✓ 已讲到，✗ 讲错，· 遗漏，? 不清楚。讲错或遗漏的要点会显示其时间点，方便你回到课堂中查看。
repaso-sin-puntos = 这堂课还没有要点。要点来自转写时 Jev 标记为重要的内容、带备注的书签（M）以及已讲过的议题。
repaso-recuento = 已讲到 { $cubiertos } / { $total } · 讲错 { $mal } · 遗漏 { $faltan } · 不清楚 { $dudosos }
repaso-preparando = 正在准备麦克风…
repaso-te-escucho = 正在聆听：开始讲解吧。
repaso-pulsa-hablar = 按“开始说话”（或空格）讲解这堂课。
repaso-hablar = 开始说话
repaso-parar = 停止
repaso-otra-vez = 重新开始
repaso-cerrar = 关闭
repaso-jev-fallo = Jev 没有响应：要点保持不变。
estudiar-marcador-guardado = 书签已保存
estudiar-no-se-guardo = 无法保存
estudiar-sin-texto = 还没有转写文本：在聊天中对该消息点击“转为文字”，文本会自动显示在这里。
estudiar-sin-capturas = 这堂课没有截图。
estudiar-sin-video = 此视频无法在这里播放；改为播放语音消息
estudiar-sin-sonido = 找不到这堂课的音频
estudiar-solo-audio = 仅音频
estudiar-cargando = 正在加载…
estudiar-ver-video = 显示视频
estudiar-ver-captura = 显示截图
estudiar-anotar = 标注
estudiar-ver-texto = 文字
estudiar-fotograma-guardado = 该帧已保存为截图：可在画布上标注
estudiar-nada-que-anotar = 这里还没有可标注的内容
estudiar-clip-movil = 手机视频 · { $duracion }
estudiar-sin-hevc = 要播放此片段，请从 Microsoft Store 安装“HEVC 视频扩展”
anki-enviar = 发送到 Anki
anki-pregunta-clase = 这里讲了什么？
anki-nada = 这里没有卡片。每行写一张，格式为“问题 :: 答案”，或“Q: …”和“A: …”，或使用填空 {"{{"}c1::像这样{"}}"}。
anki-hecho = Anki 中已有 { $n } 张卡片，牌组“{ $mazo }”
anki-ya-estaban = 这些卡片已在“{ $mazo }”中
anki-fichero = Anki 未打开（或缺少 AnkiConnect）。已将 { $n } 张卡片保存到 { $donde }：在 Anki 中选择“文件 → 导入”。
anki-fallo = Anki 未接受：{ $motivo }
# Sharing a recording (class or meeting): the HTML page and its formats.
compartir-anki = Anki
compartir-tipo-grabacion = 录制
grabacion-idioma = zh-CN
grabacion-fecha = { $ano }/{ $mes }/{ $dia } { $hora }
grabacion-transcripcion = 转写文本
grabacion-capturas = 截图
grabacion-captura = 截图
grabacion-marcadores = 书签
grabacion-marcador = 书签
grabacion-duracion = 时长
grabacion-sin-texto = 此录制还没有转写文本。
grabacion-buscar = 搜索转写文本
grabacion-seguir = 跟随音频
grabacion-reproducir = 播放或暂停
grabacion-velocidad = 速度
grabacion-ir-al-minuto = 跳到此时刻
grabacion-cerrar = 关闭
grabacion-audio = 音频
grabacion-video = 视频
buscar-todo-titulo = 搜索 PinDesk
buscar-todo-pista = 搜索文件、任务、心得、截图… 或输入一个字母
buscar-todo-pestana-todo = 全部
buscar-todo-pestana-archivos = 文件
buscar-todo-pestana-tareas = 任务
buscar-todo-pestana-lecciones = 心得
buscar-todo-pestana-capturas = 截图
buscar-todo-pestana-acciones = 操作
buscar-todo-cambia = 切换
buscar-todo-resultados = { $n ->
   *[other] { $n } 个结果
  }
buscar-todo-ninguno = 没有与“{ $consulta }”相关的内容
buscar-todo-vacia = 这里还没有内容
buscar-todo-ninguno-pista = 试试少输几个字，或在其他标签页中查找（Tab）
buscar-todo-recientes = 最近搜索
buscar-todo-borrar-todas = 全部清除
buscar-todo-abierto-hace-poco = 最近打开
buscar-todo-letras = 输入一个字母和空格
buscar-todo-mejor = 最佳结果
buscar-todo-grupo-tareas = 任务和心得
buscar-todo-grupo-archivos = 文件
buscar-todo-grupo-capturas = 截图
buscar-todo-grupo-acciones = 操作
buscar-todo-n-de-m = { $n } / { $total }
buscar-todo-pegar = 用以下按键粘贴图片
buscar-todo-pegar-2 = 它会与你输入的内容一起：
buscar-todo-pegar-ejemplo = t 检查裂缝
buscar-todo-letra-t = 新建任务
buscar-todo-letra-t-sub = 放入收件箱 · “t 买水泥”
buscar-todo-letra-n = 新建笔记
buscar-todo-letra-n-sub = 放入“收藏的消息” · “n 想法 @项目”
buscar-todo-letra-l = 新建画布
buscar-todo-letra-l-sub = 空白 · “l 平面图 @项目”
buscar-todo-letra-g = 截图库
buscar-todo-letra-g-sub = “g 墙” 在其中搜索
buscar-todo-letra-c = 截图
buscar-todo-letra-c-sub = 屏幕的一个区域
buscar-todo-letra-u = 贴出最近的截图
buscar-todo-letra-u-sub-nada = 还没有截图
buscar-todo-letra-a = 新建心得
buscar-todo-letra-a-sub = 你学到的东西 · “a 检查比例”
buscar-todo-accion-pinear = 贴图
buscar-todo-accion-abrir = 打开
buscar-todo-accion-apuntar = 添加
buscar-todo-accion-capturar = 截图
buscar-todo-accion-crear = 创建
buscar-todo-accion-grabar = 录制
buscar-todo-accion-enviar = 发送
buscar-todo-accion-copiar = 复制
buscar-todo-accion-reproducir = 播放
buscar-todo-accion-hecha = 完成
buscar-todo-accion-desmarcar = 取消完成
buscar-todo-accion-entrar = 进入
buscar-todo-accion-mostrar = 显示
buscar-todo-accion-pegar = 粘贴
buscar-todo-ver-chat = 在聊天中显示
buscar-todo-mas = 更多操作
buscar-todo-pie-moverse = 移动
buscar-todo-pie-abrir = 打开
buscar-todo-pie-adjuntar = 附加图片
buscar-todo-pie-cerrar = 关闭
buscar-todo-pie-borrar = 清除
buscar-todo-pie-mas = 更多操作
buscar-todo-pie-volver = 返回
buscar-todo-pie-flow = 在 Flow Launcher 中用“p”也一样
buscar-todo-abrir-con = 打开方式…
buscar-todo-borrar = 删除（可撤销）
buscar-todo-medidas = 截图 · { $ancho } × { $alto } · { $peso }
buscar-todo-caduca = { $dias ->
   *[other] { $dias } 天
  }后过期
buscar-todo-caduca-hoy = 今天过期
buscar-todo-conservada = 已保留：永不过期
buscar-todo-conservada-ya = 已保留
buscar-todo-conservar = 保留
buscar-todo-borrada = 截图已删除
buscar-todo-deshacer = 撤销
buscar-todo-copiado = 已复制
## v2-lecciones
# The lessons window of the v2 redesign (Lecciones2.dc.html and Lecciones2-repaso.dc.html).
## v2-galeria
galeria-subtitulo = { $cuantas } · 保留 7 天
galeria-buscar = 搜索，包括截图中的文字
galeria-buscar-sin-ocr = 按名称或日期搜索
galeria-elegir = 选择
galeria-sin-resultados = 没有包含“{ $texto }”的截图。
galeria-hoy = 今天
galeria-ayer = 昨天
galeria-dia-0 = 星期一
galeria-dia-1 = 星期二
galeria-dia-2 = 星期三
galeria-dia-3 = 星期四
galeria-dia-4 = 星期五
galeria-dia-5 = 星期六
galeria-dia-6 = 星期日
galeria-elegir-dia = 选择整天
galeria-chip-video = 视频
galeria-pinear = 贴图
galeria-copiar = 复制
galeria-conservar = 保留
galeria-borrar = 删除
galeria-elegidas = { $cuantas ->
   *[other] 已选 { $cuantas } 项
}
galeria-elegir-todas = 全部
galeria-deshacer = 撤销
galeria-borradas = 已将 { $cuantas } 项移到 PinDesk 回收站
galeria-recuperadas = { $cuantas ->
   *[other] 已恢复 { $cuantas } 项
}
galeria-pineadas = 已将 { $cuantas } 项贴到屏幕上
galeria-copiadas = 已将 { $cuantas } 项复制为文件
galeria-conservadas-aviso = 已保留 { $cuantas } 项：它们在“收藏的消息”中
galeria-prorrogada = 再延长七天：稍后才会删除
## v2-pines
# The pin toolbar, its right-click menu and the "Open pins" panel.
pin-v2-anotar = 标注
pin-v2-mas = 更多
pin-v2-opacidad = 不透明度
pin-v2-pines-abiertos = 已打开的贴图…
pin-v2-abrir = 打开
pin-v2-pinear-pagina = 贴出此页
pin-v2-pinear-todas = 贴出所有页面
pin-v2-cerrar-pin = 关闭贴图
pin-v2-alejar = 缩小
pin-v2-acercar = 放大
pin-v2-en-vivo = 实时
pin-v2-tecla-espacio = 空格
pin-v2-tecla-rueda = 滚轮
pin-v2-tecla-re-pag = PgUp
pin-v2-tecla-av-pag = PgDn
pin-v2-tecla-mayus-rueda = Shift+滚轮
pin-v2-panel-titulo = 已打开的贴图
pin-v2-panel-buscar = 搜索贴图
pin-v2-panel-mostrar-todos = 全部显示
pin-v2-panel-ocultar-todos = 全部隐藏
pin-v2-panel-cerrar-todos = 全部关闭 · 可撤销
pin-v2-panel-deshacer = 撤销：重新打开
pin-v2-panel-sin-grupo = 无分组
pin-v2-panel-oculto = 已隐藏
pin-v2-panel-ayuda = 点击一行即可将贴图移到中央并高亮显示。眼睛图标可隐藏或重新显示它。
pin-v2-panel-vacio = 没有已打开的贴图。贴出截图、笔记或文件后，它会显示在这里。
pin-v2-panel-ocultar = 隐藏
pin-v2-panel-mostrar = 显示
pin-v2-panel-cerrar-este = 关闭此贴图
pin-v2-panel-cerrar-panel = 关闭面板
pin-v2-tipo-foto = 照片
pin-v2-tipo-nota = 笔记
pin-v2-tipo-pdf = PDF
pin-v2-tipo-video = 视频
pin-v2-tipo-archivo = 文件
pin-v2-tipo-vivo = 实时贴图
pin-v2-tipo-herramienta = 工具
pin-v2-marca-copiar = 复制
pin-v2-marca-resaltar = 高亮
pin-v2-marca-ondulada = 波浪线
pin-v2-marca-subrayar = 下划线
pin-v2-marca-tachar = 删除线
pin-v2-marca-tapar = 遮盖
pin-v2-marca-quitar = 移除标记
marcas-texto-color = 颜色
pin-v2-modo-texto = 选择文字
pin-v2-leyendo = 正在识别…
pin-v2-invertir-colores = 反转颜色
## v2-captura
captura2-zona = 区域
captura2-ventana = 窗口
captura2-pantalla = 屏幕
captura2-scroll = 滚动
captura2-gif = 录制 GIF
captura2-pin-vivo = 实时贴图
captura2-texto = 文字（OCR）
captura2-libre = 自由
captura2-repetir = 重复上次区域
captura2-ancho = 宽
captura2-alto = 高
captura2-sin-abrir = 无需打开此界面：
captura2-arrastrar-izq = + 左键拖动
captura2-arrastrar-der = + 右键拖动
captura2-arrastrar-centro = + 中键拖动
captura2-doble-centro = + 双击中键
captura2-gesto-copiar = 复制
captura2-gesto-pinear = 贴图
captura2-gesto-pin-vivo = 实时贴图
captura2-gesto-anotar = 在屏幕上绘制
captura2-salir = 退出
captura2-pista-ventana = 点击：截取窗口 · 拖动：自由区域
captura2-pista-zona = 点击：截取此处 · 拖动：自由区域
captura2-copiar-color = 复制颜色
captura2-cambiar-formato = HEX / RGB
captura2-copiar = 复制
captura2-pinear = 贴图
captura2-al-chat = 发到聊天
captura2-ocr = 文字
captura2-guardar = 保存
captura2-descartar = 放弃
captura2-buscar-proyecto = 搜索项目…
captura2-recientes = 最近
captura2-el-ultimo = 上次使用
captura2-comentario = 添加评论（可选）
captura2-enviar = 发送
captura2-sin-proyectos = 没有匹配的项目
captura2-enviada = 截图已发送到“{ $proyecto }”。
## v2-bandeja
# The tray panel (left click on the icon). The right-click menu keeps its
# usual keys.
bandeja2-sincronizar = 立即同步
bandeja2-sincronizado = 已同步 { $cuando }
bandeja2-sin-sincronizar = 尚未同步
bandeja2-hace-nada = 刚刚
bandeja2-hace-min = { $n } 分钟前
bandeja2-hace-h = { $n } 小时前
bandeja2-ayer = 昨天
bandeja2-hace-dias = { $n } 天前
bandeja2-buscar = 搜索操作…（直接输入）
bandeja2-buscar-favorito = 搜索要添加到收藏的内容…
bandeja2-sin-resultados = 没有叫这个名字的操作
bandeja2-capturar = 截图
bandeja2-capturar-sub = 用鼠标选择区域
bandeja2-grabar = 录制
bandeja2-modo-zona = 区域
bandeja2-modo-scroll = 滚动
bandeja2-modo-gif = GIF
bandeja2-modo-texto = 文字
bandeja2-modo-retardo = 延时
bandeja2-modo-color = 颜色
bandeja2-favoritos = 收藏
bandeja2-editar = 编辑
bandeja2-anadir = 添加
bandeja2-elige-favoritos = 选择你的收藏（{ $n } / { $tope }）
bandeja2-favoritos-llenos = 已有 { $tope } 个收藏：请先移除一个。
bandeja2-hecho = 完成
bandeja2-fav-pinear-copiado = 贴出已复制内容
bandeja2-fav-anotar = 标注屏幕
bandeja2-fav-pin-vivo = 实时贴图
bandeja2-pines = 贴图 · 屏幕上有 { $n } 个
bandeja2-ocultar-pines = 隐藏贴图
bandeja2-paso-clic = 点击穿透
bandeja2-atajos = 快捷键
bandeja2-silenciar = 暂停快捷键
bandeja2-silenciar-sub = 用于游戏或演讲
bandeja2-ultimas = 最近的截图
bandeja2-ver-galeria = 查看截图库
bandeja2-sin-capturas = 还没有截图
bandeja2-abrir-pindesk = 打开 PinDesk
bandeja2-tareas = 任务
bandeja2-lecciones = 心得
bandeja2-nueva-leccion = 新建心得
bandeja2-galeria = 截图库
bandeja2-documento = 打开文档…
bandeja2-grupos = 窗口组
bandeja2-para-hoy = 今天 { $n } 项
bandeja2-salir = 退出 PinDesk
bandeja2-salir-sub = 贴图将关闭
bandeja2-copiada = 已复制
bandeja2-borrada = 已移到回收站
bandeja2-devuelta = 已恢复
bandeja2-deshacer = 撤销
bandeja2-no-se-pudo = 操作失败：{ $motivo }

## tareas-v3
tareas3-buscar = 搜索任务、所属列表和聊天…（“今天”、“昨天”）
tareas3-encontradas = { $n ->
   *[other] 找到 { $n } 项任务
  }
tareas3-pendientes = { $n ->
    [0] 没有待办
   *[other] { $n } 项待办
  }
tareas3-sin-resultados = 没有与“{ $busqueda }”匹配的内容
tareas3-sin-resultados-pista = 会搜索每项任务的文字及其列表和聊天的名称，忽略重音和大小写。试试少输几个词，或输入“今天”或“昨天”。
tareas3-vaciar-busqueda = 清除搜索
tareas3-mover-intro = 移动
tareas3-mover-esc = 关闭
tareas3-quitada = 已移除“{ $tarea }”
tareas3-deshacer = 撤销
tareas3-repuesta = 任务已放回其列表
tareas3-cambio-quitar = 列表在此期间已更改，现已更新。请重新移除。
tareas3-borrar-lista = 删除列表
tareas3-borrar-lista-aviso = 删除“{ $chat }”中的列表“{ $lista }”及其 { $n ->
   *[other] { $n } 项任务
  }？它也会从聊天和手机中删除，且无法撤销。
tareas3-lista-borrada = 已删除列表“{ $lista }”

## timeline (5-oct-2026)
timeline-titulo = 时间线
timeline-sub-ultimas = 最近 24 小时 · { $n ->
   *[other] { $n } 个时刻
  }
timeline-sub-dia = { $n ->
   *[other] { $n } 个时刻
  }
timeline-ultimas-24h = 最近 24 小时
timeline-hoy = 今天
timeline-ayer = 昨天
timeline-exportar = 导出
timeline-volver = 返回
timeline-pista = 正在发生什么？输入或口述…
timeline-frase-titulo = 事情是这样的
timeline-frase-titulo-corta = 情况是
timeline-frase-descripcion = 具体来说
timeline-frase-descripcion-corta = 详细地说
timeline-vacio = 最近 24 小时还没有内容。
    输入或口述正在发生的事，然后按 Enter。
    口述时，«{ timeline-frase-titulo }…» 开始标题，«{ timeline-frase-descripcion }…» 开始描述。
timeline-vacio-dia = 那天没有记录任何内容。
timeline-nota-de-voz = 语音消息
timeline-sin-titulo-voz = 语音消息
timeline-sin-titulo-foto = 照片
timeline-apuntado = 已记录
timeline-corregido = 已修改
timeline-borrado = 时刻已删除
timeline-deshacer = 撤销
timeline-abrir = 打开
timeline-escribe-algo = 请为标题输入一些内容
timeline-no-guardado = 无法保存时间线
timeline-corrigiendo = 正在修改 { $hora } 的时刻 · Enter 保存，Esc 取消
timeline-voz-esperando = 🎙 语音消息将与文字一起保存：请按 Enter
timeline-pegar-no-imagen = 这既不是图片也不是文字
timeline-cal-elegidos = { $n ->
   *[other] 已选 { $n } 天
  }
timeline-cal-quitar = 清除选择
timeline-exportar-html = 网页（HTML）
timeline-exportar-pdf = PDF 文档
timeline-tipo-html = 网页
timeline-tipo-pdf = PDF 文档
timeline-nada-que-exportar = 那里没有可导出的内容
timeline-no-exportado = 无法保存文件
timeline-exportado = 已保存：{ $nombre }
timeline-exportado-pie = 由 PinDesk 导出 · { $fecha }
timeline-pdf-haciendo = 正在生成 PDF…
timeline-pdf-sin-edge = 生成 PDF 需要 Microsoft Edge；请导出为 HTML
timeline-pdf-fallo = 无法生成 PDF
timeline-mic-abriendo = 正在打开麦克风…
timeline-mic-escuchando = 正在聆听… 按麦克风（或 Ctrl+M）结束
timeline-mic-guardando = 正在保存你说的内容…
timeline-mic-bajando = 正在下载语音模型（仅一次）… { $pc } %
timeline-mic-pasando = 正在转写… { $pc } %
timeline-mic-sin-dictado = 这台电脑无法转写：请手动输入
timeline-mic-no-abre = 无法打开麦克风
timeline-mic-no-guarda = 无法保存你说的内容
timeline-mic-sin-microfono = 没有麦克风
timeline-mic-corto = 太短了：请多说一会儿
timeline-mic-error = 无法录音：{ $error }
timeline-mic-sin-texto = 未能识别：仅保留音频


## simple-lecciones

## galeria-menu (5-oct-2026, right click)
galeria-guardar-como = 另存为…
galeria-mostrar-en-carpeta = 在文件夹中显示
galeria-guardadas = { $cuantas ->
   *[other] 已保存 { $cuantas } 项
}
galeria-prorrogar = 再保留 7 天

## timeline-v2 (5-oct-2026): common bar, WeChat-like tabs and detail
timeline-pestana-hoy = 今天
timeline-pestana-momentos = 精选
timeline-pestana-estado = 日历
timeline-buscar-pista = 搜索时间线
timeline-sub-buscar = { $n ->
    [0] 未找到
   *[other] { $n } 个结果
  }
timeline-sub-momentos = { $n ->
    [0] 还没有时刻
   *[other] 共 { $n } 个时刻
  }
timeline-sin-resultados = 没有时刻包含该内容。
    会搜索标题和描述，忽略重音和大小写。
timeline-momentos-vacio = 你挑选的时刻会放在这里。在“今天”中，将鼠标悬停在卡片上并点击书签即可发送到这里。
timeline-a-momentos = 已发送到“精选”
timeline-fuera-de-momentos = 已从“精选”中移除
timeline-estado-sobre-todo = 主要是
timeline-exportar-busqueda = 搜索：“{ $que }”
timeline-pestana-lecciones = 心得
timeline-sub-lecciones = { $n ->
    [0] 没有心得
   *[other] { $n } 条心得
  }
timeline-lecciones-vacio = 还没有心得。点击某个时刻的灯泡即可将其记为心得。
timeline-gravedad-leve = 轻微
timeline-gravedad-importante = 重要
timeline-gravedad-grave = 严重
timeline-otra-vez-hecho = 已记录：目前共 { $n } 次
timeline-leccion = 心得
timeline-leccion-hecha = 💡 已记为心得：它在“心得”中
timeline-leccion-quitada = 已取消标记。心得仍保留在“心得”中。
timeline-leccion-fallo = 无法保存心得
timeline-leccion-por-que = 原因
timeline-leccion-proxima = 下次
timeline-tambien-lecciones = { $n ->
   *[other] 另有 { $n } 条心得匹配 · 查看
  }
timeline-copiar-imagen = 复制为图片
timeline-compartir-html = 分享为 HTML
timeline-guardar-imagen = 保存为图片…
timeline-imagen-copiada = 已复制为图片
timeline-imagen-fallo = 无法生成图片
timeline-tipo-png = PNG 图片
timeline-compartido-pie = PinDesk · 时间线

## tareas-v4
# The search box hint in the shared top bar: shorter than tareas3-buscar,
# which does not fit in the space the bar leaves.
tareas4-buscar = 搜索任务…
timeline-otra-vez-corto = ↻ 又发生了 +1
timeline-pagina-linea = 时间线
timeline-exportado-pesa = 文件大小为 { $mb } MB：可能难以发送
## v2-diseno
# The gallery with the v2 design system (Oct 6).
galeria-chapa-hoy = 今天
galeria-pista-caduca = { $dias ->
    [0] 今天删除
    [1] 明天删除
   *[other] { $dias } 天后删除
  } · 右键单击以保留
galeria-ya-conservadas = 它们已被保留
galeria-dia-cuantas = { $cuantas ->
   *[other] { $cuantas } 张截图
  }
timeline-sub-estado-dias = { $n ->
    [0] 还没有记录内容的日子
   *[other] { $n } 天有记录
  }
timeline-filtro-todas = 全部
timeline-orden-recientes = 最近
timeline-orden-repetidas = 重复最多
timeline-lecciones-sin-resultados = 没有心得包含该内容。
timeline-otra-vez-pildora = ↻ +1

## tareas-v5
# The bar subtitle: each group already counts what is pending.
tareas5-subtitulo = { $listas ->
   *[other] { $listas } 个列表
  }
tareas5-apuntar-pista = 新建任务…
tareas-recordare = ⏰ 我会在{ $cuando }提醒你
tareas-sin-recordatorio = 提醒已移除
tareas-apuntar-en-pista = 在“{ $lista }”中新建任务…
tareas5-corregir-pista = 任务内容…
tareas5-imagen-pista = 添加图片 · Ctrl+V 粘贴已复制的图片
tareas5-guardar = 保存
tareas5-corregida = 任务已更新
tareas5-vista-tarjetas = 卡片 · Ctrl+T
tareas5-vista-horario = 课程表 · Ctrl+H
tareas5-vista-kanban = Kanban 看板 · Ctrl+K
tareas5-marcar = 标记为完成
tareas5-desmarcar = 标记为待办
tareas5-editar = 编辑
tareas5-copiar-texto = 复制文字
tareas5-copiada = 文字已复制
tareas5-pinear-imagenes = { $n ->
   *[other] 贴出 { $n } 张图片
  }
tareas5-quitar = 移除

## salida (6-oct-2026): the floating window to drag out what was exported
salida-titulo = 可以分享了
salida-titulo-exportado = 已导出
salida-pista = 将文件拖到其他应用或文件夹
salida-todos = 一起拖动全部 { $n } 个
salida-arrastra-todos = 按住并拖动以带走全部
salida-soltado = 已放下 · 也可以放到其他位置
salida-copiado = 已复制到剪贴板
salida-pineado = 已贴图
salida-no-se-pudo = 操作失败
salida-copiar = 复制
salida-abrir = 打开
salida-carpeta = 在文件夹中显示
salida-pinear = 贴图
salida-compartir-menu = 分享…

## al-movil (6-oct-2026): a photo from the Flow plugin to the phone's open canvas
al-movil-lienzo = { $n ->
   *[other] 已将 { $n } 张图片发送到 { $movil } 上的画布
  }
al-movil-chat = { $movil } 已将其保存到通用对话中（没有打开的聊天或画布）
al-movil-chat-abierto = { $n ->
   *[other] 已将 { $n } 个文件发送到 { $movil } 上的“{ $chat }”聊天
  }
al-movil-solo-fotos = { $movil } 打开了画布：只能接收照片。请打开聊天以发送文件。
al-movil-negados = { $negados ->
   *[other] 有 { $negados } 个文件未发送：它们不是照片，且对方打开了画布
  }
al-movil-no-esta = { $movil } 未打开：请在手机上打开 PinDesk
al-movil-actualiza = 请更新 { $movil } 上的 PinDesk：它暂不支持直接接收文件。正在打开“通过 Wi-Fi 发送”。
al-movil-ocupado = { $movil } 正在与另一台设备同步。请稍后重试。
al-movil-otra-version = { $movil } 上的 PinDesk 版本不同：请同时更新两者
al-movil-cortado = 无法发送到 { $movil }：{ $motivo }
al-movil-sin-grupo = 这台电脑不在任何群组中：请在 PinDesk 的“同步”中设置
al-movil-no-es-del-grupo = 该设备已不在这台电脑的群组中
al-movil-sin-imagenes = 没有可发送的内容
al-frente-lienzo = { $de } 向你发送了“{ $nombre }”：它在打开的画布上
al-frente-chat-abierto = { $de } 向你发送了“{ $nombre }”到聊天“{ $chat }”
al-frente-guardados = { $de } 向你发送了“{ $nombre }”：它在“收藏的消息”中

## Excel sheet viewer (visor_hoja)
hoja-copiado = { $celdas ->
   *[other] 已复制 { $celdas } 个单元格
  }
hoja-demasiado = 单元格太多，无法一次复制（{ $celdas }）
hoja-cargando = 正在加载… { $filas } 行
hoja-enlace-no-abre = 无法打开链接

## Chats shared with people (Android 13-oct-2026, ChatCompartidoActivity)
cc-menu-compartir = 与他人共享
cc-menu-entrar = 加入共享聊天
chat-nuevo-normal = 普通聊天
chat-nuevo-normal-detalle = 仅你和你的设备
chat-nuevo-compartido = 共享聊天
chat-nuevo-compartido-detalle = 与其他人一起，各自在自己的设备上
cc-chapa-personas = { $n ->
   *[other] 已共享 · { $n } 人
}
cc-nuevo-error = 无法共享新聊天：它仍为普通聊天
cc-portada-titulo = 共享聊天
cc-portada-como = 与他人共用的项目聊天：对方只能看到这个聊天。要共享，请右键单击项目 → 与他人共享。
cc-portada-entrar = 加入共享聊天
cc-titulo-personas = 成员
cc-titulo-entrar = 加入聊天
cc-tu-nombre = 你在共享聊天中的名字
cc-guardar = 保存
cc-compartir-explica = 与其他人共享此聊天。
cc-compartir-detalle = 只会同步此聊天：其消息、文件、画布和标注。你的其他聊天和项目保持隐藏。所有人在同一 Wi-Fi 下相互同步。
cc-compartir = 共享
cc-ya-no-estas = 你已不在此聊天中。已有内容保留在这里，但不再同步。
cc-tu = （你）
cc-creo = 创建了聊天
cc-tu-otro-aparato = 你的另一台设备
cc-salio = 已退出
cc-quitado = 已移除
cc-quitar = 移除
cc-anadir = 添加
cc-cerrar-invitacion = 关闭
cc-sincronizar = 同步
cc-invitacion-como = 在另一台设备上：PinDesk → 加入共享聊天。扫描此码或输入代码。
cc-invitacion-vale = 此界面打开期间有效。你们必须连接同一 Wi-Fi。
cc-salir-del-chat = 退出聊天
cc-salir-titulo = 退出聊天？
cc-quitar-titulo = 移除 { $nombre }？
cc-salir-aviso = 你已有的内容保留在这里，但将不再与其他人同步。
cc-quitar-aviso = 对方会保留已有的内容，但不会再收到此聊天的任何新内容。
cc-salir = 退出
cc-cancelar = 取消
cc-entrar-como = 邀请你的人打开聊天 → 与他人共享 → 添加，然后向你展示一个代码。你只会获得该聊天：不会得到对方设备上的其他任何内容，你也不会给出任何自己的内容。这台电脑没有摄像头：请输入代码或粘贴其二维码的文本。
cc-escribe-codigo = 输入代码或粘贴二维码文本
cc-entrar = 加入
cc-ya-estas = 你已加入“{ $nombre }”
cc-con = 成员：{ $nombres }
cc-cerrar = 关闭
cc-compartido = 已共享。点击“添加”邀请他人。
cc-entro = { $nombre } 已加入。正在传送聊天…
cc-listo = 完成：对方已获得聊天。
cc-demasiados = 错误代码过多：邀请已关闭。
cc-has-salido = 你已退出。同步时其他人会得知。
cc-quitado-aviso = 已移除。对方同步时会得知。
cc-nadie = 还没有其他人。请点击“添加”。
cc-buscando = 正在查找聊天成员…
cc-sincronizando-con = 正在与 { $nombre } 同步…
cc-nadie-en-wifi = 当前此 Wi-Fi 下没有聊天成员。
cc-sincronizado = 已与 { $nombres } 同步。
cc-buscando-invitacion = 正在查找邀请你的人…
cc-entrando = 正在加入…
cc-trayendo = 正在传送“{ $nombre }”…
cc-no-encontrada = 未找到邀请。请检查代码，并确认你们连接的是同一 Wi-Fi。
cc-se-corto = 已中断：{ $motivo }
cc-abrir-chat = 打开聊天
cc-no-se-abre = 该聊天还不在这里：请再次同步。

## Pins like on Android (batch 6): color, stickers, paged note and own words
pin-v2-pegatina = 贴纸
pin-v2-sin-pegatina = 无贴纸
pin-v2-paginas = 页面
pin-v2-leer-seguida = 连续阅读
pin-v2-pagina = 页
pin-v2-minutos = 分钟
pin-color-copiado = 已复制
ajustes2-sec-pinear = 贴图
ajustes2-sec-pinear-sub = 贴出时会变成工具的词语。
ajustes2-sec-tareas = 任务
ajustes2-sec-tareas-sub = 输入任务时如何理解其内容。
ajustes2-g-urgencia = 紧急程度词（不含这些词时为普通任务）
ajustes2-urgente = 紧急
ajustes2-muy-urgente = 非常紧急
ajustes2-urgencia-ninguna = 无：没有内容会获得此紧急程度
ajustes2-g-palabras = 魔法词
ajustes2-ayuda-palabras = 单独复制其中一个词并贴出：即可打开对应工具。用逗号分隔。
ajustes2-palabras-ninguna = 无
ajustes2-palabras-cambiar = 更改
ajustes2-palabras-fabrica = 出厂词语
ajustes2-palabras-volver = 重置
ajustes2-palabras-marcador = 词语, 另一个词语…

## Local assistant (13-oct-2026)
asistente-mencionar = 提及：助手或绘制位置
asistente-abrir-lienzo = 打开画布
asistente-abrir-fuente = 打开引用的来源
asistente-ocupado = 助手仍在回答：请等待或点击停止。
asistente-fallo = 助手无法开始回答。
asistente-nube-titulo = 缺少云端密钥
asistente-nube-explica = 助手使用 { $nube } 回答：请在“设置 → 助手”中粘贴其密钥（可在 { $pagina } 获取）。简单命令（“记下面包”、“打开截图库”）无需密钥即可使用。
asistente-abrir-ajustes = 打开设置
asistente-viejos-titulo = 不再使用本地模型
asistente-viejos-explica = 助手现在只通过云端（Claude 或 DeepSeek）回答。你为本机模型下载的内容仍占用 { $tamano } 磁盘空间。
asistente-viejos-liberar = 释放 { $tamano }
asistente-viejos-dejar = 保留
asistente-viejos-borrados = 完成：本地模型已删除，空间已释放。
asistente-viejos-no-borrados = 无法删除所有本地模型。请关闭正在使用它们的程序，然后在“设置 → 助手”中重试。
ajustes2-asistente-borrar-locales = 旧的本地模型
ajustes2-ayuda-asistente-borrar-locales = 为本地助手下载、现已不再使用的内容：{ $tamano }，位于 %LOCALAPPDATA%\PinDesk\modelos。

## Cloud assistant (13-oct-2026)
ajustes2-sec-asistente = 助手
ajustes2-sec-asistente-sub = 在“Asistente”聊天中由谁回答：Claude 或 DeepSeek，使用你的密钥。
ajustes2-g-asistente = 助手引擎
ajustes2-asistente-motor = 引擎
ajustes2-ayuda-asistente-motor = 使用你的密钥通过互联网回答：Claude Haiku 或 DeepSeek。
asistente-confirmar-si = 是
asistente-confirmar-no = 否
asistente-otro = 其他…
asistente-otro-escribe = 在下方输入你的回答并发送
asistente-recomendada = 推荐
asistente-receta-guardar = 保存为配方
asistente-modelo = 模型
asistente-nueva-conversacion = 新对话
asistente-nuevo-chat = 新聊天
asistente-borrar-chat = 清空聊天
asistente-borrar-chat-aviso = “助手”聊天中的所有消息都将被删除（包括附件）。是否继续？
asistente-escuchar = 朗读
asistente-modelo-cambiado = 现在由 { $modelo } 回答。
asistente-receta-guardada = 配方已保存：{ $nombre }
asistente-receta-llenas = 配方太多了：请先删除一个。
asistente-receta-renombrar = 重命名
asistente-receta-editar = 编辑请求
asistente-receta-icono = 更换图标
asistente-receta-izquierda = 左移
asistente-receta-derecha = 右移
asistente-receta-borrar = 删除配方
asistente-receta-editando-nombre = 配方“{ $nombre }”的名称 · Enter 保存 · Esc 取消
asistente-receta-editando-pedido = 配方“{ $nombre }”的请求 · Enter 保存 · Esc 取消
ajustes2-asistente-docs-nube = 你的文档与云端
ajustes2-ayuda-asistente-docs-nube = 使用 Claude 或 DeepSeek 时，助手在你的 PDF、笔记和聊天中找到的内容会通过互联网发送以便回答你。
ajustes2-docs-nube-preguntar = 询问
ajustes2-docs-nube-si = 允许
ajustes2-docs-nube-no = 不允许
ajustes2-g-claves = API 密钥
ajustes2-clave-de = { $proveedor } 密钥
ajustes2-clave-marcador = 在此粘贴密钥
ajustes2-clave-probar = 保存并测试
ajustes2-clave-borrar-de = 删除 { $proveedor } 密钥
ajustes2-ayuda-clave-borrar = 会从这台电脑上删除。在你的账户中撤销之前，该密钥仍然有效。
ajustes2-clave-borrar = 删除
ajustes2-clave-guardada = 已加密保存在这台电脑上：
ajustes2-clave-falta = 没有密钥。获取地址：
ajustes2-clave-probando = 正在测试…
ajustes2-g-conexiones = 助手连接
ajustes2-conexion-cliente = { $servicio } 客户端 ID
ajustes2-conexion-cliente-marcador = 粘贴客户端 ID（或 Google 提供的 JSON）
ajustes2-conexion-guardar = 保存
ajustes2-conexion-de = { $servicio }
ajustes2-conexion-conectar = 连接
ajustes2-conexion-desconectar = 断开连接
ajustes2-conexion-sin-cliente = 没有客户端 ID。请在 { $consola } 创建一次（步骤：docs/conexiones-google-drive.md）。
ajustes2-conexion-cliente-guardado = ID 已加密保存在这台电脑上：
ajustes2-conexion-conectado = 已连接
ajustes2-conexion-sin-conectar = 未连接。经你允许后，助手可以在你的 { $servicio } 中搜索、读取、下载和上传文件。
ajustes2-conexion-conectando = 请在浏览器中完成…

## Assistant automations (Oct 13)
automatizacion-creada = 已创建自动化“{ $nombre }”。
automatizacion-activada = 已开启自动化“{ $nombre }”。
automatizacion-desactivada = 已关闭自动化“{ $nombre }”。
automatizacion-borrada = 已删除自动化“{ $nombre }”。
automatizacion-en-marcha = 正在运行“{ $nombre }”…
automatizacion-no-esta = 没有自动化“{ $id }”。
automatizacion-rota = 无法保存自动化：{ $motivo }
ajustes2-g-automatizaciones = 自动化
ajustes2-automatizaciones-vacio = 还没有
ajustes2-ayuda-automatizaciones-vacio = 问问助手：“每次截图后，把它放到工地聊天中”、“每天 8 点，汇总待办任务”。
ajustes2-automatizacion-correr = 立即运行
ajustes2-automatizacion-otro-aparato = 在 { $aparato } 上运行
ajustes2-automatizacion-rota = 无法理解：{ $motivo }
ajustes2-automatizacion-borrar = 删除自动化“{ $nombre }”？此操作无法撤销。
ajustes-asistente-pensar = 回答前先思考
ajustes2-ayuda-asistente-pensar = 在行动之前，助手会先思考并写下它的理解；思考过程会折叠显示在回答上方。思考需要时间：设为自动时，仅在复杂任务（如解决问题、制定计划或连续多个步骤）或你要求时（“好好想想”、“一步一步来”）才会思考。
asistente-pensar-auto = 自动（仅复杂任务）
asistente-pensar-si = 始终
asistente-pensar-no = 从不
asistente-pensando = 正在思考…
asistente-penso = 思考了 { $s } 秒
asistente-penso-poco = 思考了片刻
asistente-reiniciado = 上下文已更新：从工作表继续

## Assistant voice (13-oct-2026): talk to it and hear the answer
ajustes2-g-asistente-voz = 助手语音
ajustes2-asistente-revisar-voz = 发送前检查你说的内容
ajustes2-ayuda-asistente-revisar-voz = 使用“Asistente”聊天的麦克风时：你说的话会写入输入框，便于修改后自行发送。关闭后会立即发送。
asistente-voz-pasando = 正在将语音转为文字… { $pct } %
asistente-voz-bajando = 正在下载语音识别器（仅一次，约 160 MB）… { $pct } %
asistente-voz-nada = 没有听到声音：请靠近麦克风说话后重试。
asistente-voz-sin-whisper = 这台电脑无法将语音转为文字：缺少 Windows 的 ONNX Runtime。
asistente-voz-fallo = 无法将你的语音转为文字。
asistente-voz-cancelado = 已取消：未发送任何内容。
asistente-voz-nada-que-leer = 此回答没有可朗读的文字。

# The buttons of the Windows notifications (15-oct).
notif-hecha = 完成
notif-mas-10 = +10 分钟
notif-abrir = 打开
notif-conservar = 保留
notif-ver-galeria = 打开截图库
notif-captura-se-va = 此截图将于{ $cuando }删除
notif-capturas-se-van = { $n } 张截图将在 24 小时内删除
notif-capturas-titulo = 截图即将过期

# Geometry sheets (exact arc, angle mark, gradient)
herramientas-angulo = 角度标记
lienzo-pista-arco = 圆弧（基于参考线：描出参考椭圆；或在面板中选择“圆心和两点”或“三点”）
lienzo-pista-angulo = 角度 (W)：在两条线相交处附近的角内点击；会放置 α、β、θ… 且不重复（↑ ↓ 或 Alt+滚轮更换字母；再按 W：带小方块的 90° 角）
lienzo-panel-modo-angulo = 角度字母
lienzo-panel-recientes = 最近使用
lienzo-panel-personalizado = 自定义
lienzo-angulo-modo-griega = α β θ
lienzo-angulo-modo-simple = 无字母
lienzo-angulo-modo-recto = 90° ∟
lienzo-panel-degradado = 渐变填充
lienzo-panel-cierre-arco = 圆弧闭合
lienzo-panel-modo-arco = 绘制圆弧
lienzo-panel-arcos-angulo = 圆弧
lienzo-degradado-liso = 纯色
lienzo-degradado-lineal = 线性
lienzo-degradado-radial = 径向
lienzo-cierre-abierto = 开放
lienzo-cierre-sector = 扇形
lienzo-cierre-segmento = 弓形
lienzo-arco-modo-guia = 参考线
lienzo-arco-modo-centro = 圆心
lienzo-arco-modo-tres = 3 点

## Tasks: single-task pin and class timetable (17-oct-2026)
tareas-fijar-pin = 📌 贴出作为提醒
pin-tarea-nota = 💬 为什么现在不做？
pin-tarea-pista = 为什么现在不做 · Enter 保存
# Tasks: urgency and day/time picker (18-oct-2026)
tareas-urgente = 紧急
tareas-muy-urgente = 非常紧急
tareas-selector-pista = 选择提醒的日期和时间
tareas-selector-sin = 不提醒
tareas-selector-quitar = 移除
tareas-selector-hoy = 今天
tareas-selector-manana = 明天
tareas-dia-0 = 周一
tareas-dia-1 = 周二
tareas-dia-2 = 周三
tareas-dia-3 = 周四
tareas-dia-4 = 周五
tareas-dia-5 = 周六
tareas-dia-6 = 周日
bandeja-horario = 🎓 课程表…
bandeja-horario-grabar = 🎙 为下一节课录语音消息
horario-sin-clases = 课程表中还没有课程：打开“课程表…”，每行写一节课。
horario-guardada = 你将在下一节 { $clase } 课时听到它

## Weekly timetable window and subjects (18-oct-2026)
horario-subtitulo = { $asignaturas } 门科目 · { $clases } 节课
horario-nueva = 新科目（可选 @教室）
horario-domingo = 星期日
horario-como-texto = 以文本形式
horario-vacio = 在上方输入科目，然后拖到周视图中。
horario-tareas-de = { $asignatura } 的任务
horario-sin-tareas = 没有待办任务。在“任务”中添加时选择科目即可。
horario-abrir-tareas = 在“任务”中打开
horario-aula = 教室
horario-nombre = 名称
horario-nota = 语音消息（来电时朗读的内容）
horario-avisar = 提醒（提前分钟数）
horario-avisar-pista = { $min }（课程表的设置）
horario-guardar = 保存
horario-cancelar = 取消
horario-borrar = 删除
horario-deshacer = 撤销
horario-borrada = 课程已删除
horario-asignatura-borrada = 已从课程表中移除“{ $asignatura }”（其任务仍保留在“任务”中）
horario-repetida = “{ $asignatura }”已在列表中
horario-arrastra = 将“{ $asignatura }”拖到周视图中
horario-no-guardado = 无法保存课程表
tareas-destino-pista = 任务的清单或科目
tareas-sin-asignatura = 无科目（放入收件箱）
tareas-apuntar-en-asignatura = 新建 { $asignatura } 任务…

# Viewers: the frame to print (2026-10-19)
marco-imprimir-pista = 在要打印的内容上拖出一个框 · Enter：全部 · Esc：退出
marcos-pista = 拖动以框选 · 数字：跳到该框 · Ctrl+P：全部打印 · Del：删除最后一个 · I 或 Esc：退出
marcos-imprimir = 打印各框 (Ctrl+P)
marcos-borrar = 清除框

# Right-click on the rail's frame button (27-oct-2026): what to print
marcos-menu-los-marcos = 打印各框（{ $n }）
marcos-menu-sin-marcos = 打印各框（还没有：按下框按钮并拖动）
marcos-menu-entero-pdf = 打印整个 PDF
marcos-menu-entero-word = 打印整个 Word 文档
marcos-menu-entero-powerpoint = 打印整个 PowerPoint
marcos-menu-entero-hoja = 打印整个工作表
marcos-menu-entero-pagina = 打印整个页面
marcos-menu-entero-documento = 打印整个文档
marcos-menu-entero-plano = 打印整张图纸
marcos-menu-entero-modelo = 打印模型视图
marcos-menu-no-imprime = 无法打印整个文档

riel-marcador = 书签 (Ctrl+M)
riel-anotar = 标注 (A)
riel-marcos = 打印框 (I)
barra-mover = 移动（拖动）
barra-oscuro = 夜间模式
barra-pantalla = 全屏 (F11)
barra-fijar = 始终置顶
barra-cerrar = 关闭 (Esc)
visor-letra = 字体和间距 (T / B)
lector-mas = 更多选项 (G)
visor-indice-pista = 目录 (C)
riel-editar = 编辑 (Alt+R)
riel-mirar = 返回查看（Alt+R 或 Esc）

# Pin rail: the frame button of a pin that is not a photo (19-oct-2026).
riel-imprimir-zona = 打印区域 (Ctrl+P)

# The spreadsheet viewer bar: name and key of each button (19-oct-2026).
hoja-pista-editar = 编辑单元格 (F2)
hoja-pista-ver = 返回查看 (Esc)
hoja-pista-negrita = 粗体 (Ctrl+B)
hoja-pista-cursiva = 斜体 (Ctrl+I)
hoja-pista-subrayado = 下划线 (Ctrl+U)
hoja-pista-tachado = 删除线 (Ctrl+5)
hoja-pista-resaltar = 填充颜色
hoja-pista-letra = 字体颜色
hoja-pista-alinear = 对齐（Ctrl+Shift+L / E / R）
hoja-pista-numero = 数字格式
hoja-pista-ajustar = 自动换行
hoja-pista-borrar-formato = 清除格式
## Tooltips of the web viewers' and Office pin bars (19-oct)
web-recargar = 重新加载 (F5)
web-exportar = 导出 (Ctrl+E)
web-navegador = 在浏览器中打开
nota-pista-ver = 阅读视图
nota-pista-editar = 编辑
nota-pista-buscar = 查找 (Ctrl+F)
nota-pista-comentarios = 评论
nota-pista-compartir = 分享
office-pista-abrir-con = 打开方式…
office-pista-presentar = 放映

## Page folders in the chat (26-oct-2026): a named notebook
carpeta-nueva = 页面文件夹
carpeta-nombre-de-serie = 笔记本
carpeta-pagina = 页面
carpeta-paginas = { $n ->
   *[other] { $n } 页
}
carpeta-vacia-detalle = 空
carpeta-anadir-a = 添加到文件夹…
carpeta-nueva-con = 用此页新建文件夹
carpeta-anadida = 已添加到“{ $carpeta }”，标题为“{ $titulo }”
carpeta-ya-estaba = 已在“{ $carpeta }”中
carpeta-abrir = 打开文件夹
carpeta-nueva-pagina = 新建页面
carpeta-compartir = 分享
carpeta-exportar-pdf = 导出 PDF
carpeta-exportando = 正在导出 PDF…
carpeta-exportado = PDF 已保存：{ $nombre }
carpeta-no-se-exporto = 无法导出：没有页面包含内容
carpeta-vacia = 此文件夹为空。点击“新建页面”，或在聊天中画布或图片的菜单里选择“添加到文件夹…”。
carpeta-no-esta = 该文件夹已不在聊天中
carpeta-pista = 拖动页面以移动 · F2 更改标题 · Del 移除（它会重新显示在聊天中）
carpeta-escribe-titulo = 输入标题并按 Enter
carpeta-grabacion = 录制
carpeta-grabacion-detalle = { $duracion } · { $capturas ->
   *[other] { $capturas } 张截图
}
carpeta-estudiar = 学习
carpeta-adjunto-insignia = 附件 · 不属于录制内容
carpeta-adjuntar = 附加到课堂
carpeta-adjuntado = 已附加到课堂：{ $nombre }
carpeta-grabacion-pista = 点击某项即可在对应时间点学习 · “附加到课堂”可添加文件、照片或笔记，不会与录制内容混在一起

## The assistant reads the web (Oct 28)
asistente-web-buscando = 🌐 正在搜索网络…
asistente-web-leyendo = 🌐 正在阅读 { $dominio }…
asistente-web-leyendo-pagina = 🌐 正在阅读页面…
## TypeSafe Jev (28-oct-2026, docs/jev.md)
ajustes2-g-jev = TypeSafe (Jev)
ajustes2-jev-proveedor = TypeSafe (Jev)
ajustes2-jev-probada = 可用（{ $modelo }）
ajustes2-jev-reunion = 课堂和会议中的重点
ajustes2-ayuda-jev-reunion = 转写录制时，标记任务、决定、问题以及考试内容。显示在“学习课堂”和共享页面中。每一行会连同其前后行和标题一起发送。
ajustes2-jev-anki = 只保留值得学习的 Anki 卡片
ajustes2-ayuda-jev-anki = 将课堂发送到 Anki 时，保留其片段讲解了学习内容的截图，去掉闲聊。会发送每个片段中说的内容。
ajustes2-jev-urgencia = 根据上下文确定任务优先级
ajustes2-ayuda-jev-urgencia = 根据延误的后果、谁在等待、时间以及最近的聊天消息判断为紧急、高、普通或低。你标记的（⏫、🔼）优先。会发送任务文字、其列表和所在聊天的最近消息。
ajustes2-jev-modelo = 为每个请求选择模型
ajustes2-ayuda-jev-modelo = 简单的请求交给快速模型（Haiku、DeepSeek Flash），需要推理的交给强模型（Sonnet、DeepSeek V4 Pro）。会发送请求和最近三条消息。
clase-importante-todo = 全部
clase-importante-tarea = 任务
clase-importante-decision = 决定
clase-importante-pregunta = 问题
clase-importante-examen = 考试
clase-importante-buscando = 正在查找重点…
clase-importante-tareas = + 将 { $n } 项任务加入“任务”
clase-importante-anadidas = 已将 { $n } 项任务添加到“任务”收件箱
tareas-prioridad-urgente = 紧急
tareas-prioridad-alta = 高
tareas-prioridad-baja = 低
tareas-prioridad-normal = 普通
tareas-jev-consecuencia-nada = 没有后果
tareas-jev-consecuencia-leve = 后果轻微
tareas-jev-consecuencia-media = 后果中等
tareas-jev-consecuencia-grave = 后果严重
tareas-jev-la-esperan = 有人在等
tareas-jev-nadie-espera = 没人在等
tareas-jev-plazo-ajeno = 他人设定的期限
tareas-jev-la-piden = 聊天中有人催
tareas-jev-sin-fecha = 无日期
tareas-jev-ya-paso = 已过期
tareas-jev-hoy = 今天
tareas-jev-manana = 明天
tareas-jev-en-dias = { $n } 天后
tareas-jev-por-la-lista = { $cuando }（来自列表标题）
anki-jev-quitadas = Jev 去掉了 { $n } 张没有学习内容的截图
## Meeting topics (28-oct-2026, docs/jev.md)
ajustes2-jev-temas = 已讨论的会议议题
ajustes2-ayuda-jev-temas = 录制带议题的会议时，每 15 秒在这台电脑上转写一次听到的内容，Jev 会勾选已讨论的议题。你勾选的优先。会发送每个片段及前一个片段的文字、议题和标题。
ajustes2-jev-dictado = 口述到聊天
ajustes2-ayuda-jev-dictado = 在聊天输入框中口述时（右键单击麦克风），说“在工地聊天中记下螺丝用完了”会提议记录到该聊天：Enter 记录，Esc 忽略。会发送每句口述内容、前一句以及你最近聊天的名称。
ajustes2-jev-repaso = 出声复习
ajustes2-ayuda-jev-repaso = 在“学习课堂”中，出声讲解这堂课，Jev 会标记你讲到了哪些要点、哪些讲错了、哪些遗漏了。会发送你的讲解、标题和要点。
clase-temas-ninguno = 会议议题（可选）
clase-temas-n = 会议议题 · { $n }
clase-temas-titulo = 会议议题
clase-temas-sub = 每行一个。录制时，Jev 会勾选已讨论的议题。
clase-temas-escribe = 输入议题并按 Enter…
clase-temas-pegar = 粘贴
clase-temas-de-lista = 来自列表
clase-temas-lista = 列表“{ $lista }”：{ $n } 个议题
clase-temas-sin-listas = 没有包含待办内容的任务列表
clase-temas-pegados = 已添加 { $n } 个议题
clase-temas-nada-que-pegar = 剪贴板中没有文字
clase-temas-vaciar = 清空
clase-temas-listo = 完成
clase-temas-anteriores = 上次会议遗留了 { $n } 个议题
clase-temas-tratado = ✓ 议题 { $n } 已讨论 · { $minuto }
clase-temas-escuchando = 正在聆听：Jev 会在议题讨论后勾选
clase-temas-sin-jev = 没有 Jev（缺少 TypeSafe 密钥）：请手动勾选议题
clase-temas-apagado = “设置”中已关闭 Jev 议题功能：请手动勾选
clase-temas-sin-modelo = 没有语音模型就无法实时聆听：请手动勾选议题
clase-temas-jev-fallo = Jev 没有响应：请手动勾选议题
clase-temas-cargado = 电脑繁忙：降低聆听频率，以免拖慢录制
clase-temas-sin-tratar = 未讨论
clase-temas-proxima = 移到下次会议
clase-temas-a-tareas = 添加到“任务”
clase-temas-pasados = { $n } 个议题留待下次会议
clase-temas-anadidos = 已将 { $n } 个议题添加到“任务”收件箱
clase-temas-lista-de = 议题 · { $nombre }
## What is heard in the meeting (29-oct-2026, docs/jev.md, «En la reunión»)
ajustes2-jev-tareas-dichas = 会议中提到的任务
ajustes2-ayuda-jev-tareas-dichas = 录制会议时，“我周五发报价”会作为建议任务出现在工具栏中，并带有日期和紧急程度。只有你接受后才会加入“任务”。每句话会连同前两句和标题一起发送。
ajustes2-jev-urgencia-en-vivo = 会议中提到的任务紧急程度
ajustes2-ayuda-jev-urgencia-en-vivo = “这个今天要”会提高此聊天或收件箱中某项待办任务的优先级或更改其日期。会自动应用，可在工具栏中撤销。每句话会连同最多 8 项任务的文字一起发送。
ajustes2-jev-tema-olvidado = 提醒被遗忘的议题
ajustes2-ayuda-jev-tema-olvidado = 在“准备”中设置了“计划时长”时，会在接近结束（或大家准备收尾）时提醒尚未讨论的议程议题。会发送每句话和议题。
ajustes2-jev-tiempo-por-tema = 每个议题的分钟数
ajustes2-ayuda-jev-tiempo-por-tema = 每个议程议题讨论了多久，显示在“学习课堂”和共享页面中。会发送每句话和议题。
ajustes2-jev-decisiones = 会议决定
ajustes2-ayuda-jev-decisiones = “就这么定了：订单提前”会连同其时间点一起记录（“我们可以…”不算）。显示在“学习课堂”和共享页面中。会发送每句话。
ajustes2-jev-preguntas-pendientes = 未回答的问题
ajustes2-ayuda-jev-preguntas-pendientes = 会议中无人回答的问题，在结束时显示于“学习课堂”，以便移到“任务”。会发送每句话和仍未解决的问题。
ajustes2-jev-fechas-al-horario = 提到的日期加入日程
ajustes2-ayuda-jev-fechas-al-horario = “周二 3 点见”会作为建议事件出现在工具栏中；接受后，它会带着时间加入“任务”并显示在“日程”中。会发送每句话。
clase-vivo-tarea = 任务
clase-vivo-evento = 事件
clase-vivo-deshacer = 撤销
clase-vivo-decision = 已记录决定 · { $minuto }
clase-vivo-tarea-apuntada = 任务已添加到“任务”收件箱
clase-vivo-evento-apuntado = 事件已添加到“任务”（显示在“日程”中）
clase-vivo-deshecho = 已撤销：任务已恢复原样
clase-vivo-no-se-apunto = 无法添加：请检查“任务”
clase-vivo-falla = Jev 没有响应：暂时不提供建议
clase-vivo-queda-poco = 还剩 { $minutos } 分钟，仍未讨论：{ $temas }
clase-vivo-cumplido = 时间已到，仍未讨论：{ $temas }
clase-vivo-cierran = 大家正在收尾，仍未讨论：{ $temas }
clase-vivo-fecha-nueva = 现在截止于 { $fecha }
clase-vivo-prioridad-normal = 普通
clase-vivo-duracion = 计划时长
clase-vivo-duracion-n = ⏱ { $minutos } 分钟
clase-acta-decisiones = 决定
clase-acta-preguntas = 未回答的问题
clase-acta-propuestas = 未接受的建议
clase-acta-aceptar = 接受
clase-acta-aceptada = 已添加到“任务”
clase-acta-anadidas = 已将 { $n } 个问题添加到“任务”收件箱

# --- Lo nuevo de noviembre ---
# El aviso flotante del asistente
aviso-ia-abrir = 打开
aviso-ia-si = 是
aviso-ia-no = 否
aviso-ia-permiso = 🔐 我需要你的许可：{ $pregunta }

    如果你不回答，我就不做。
# Los turnos del asistente que siguen con el chat cerrado
ia-fondo-titulo = 🤖 助手
ia-fondo-sigue = “继续”
ia-fondo-sigue-fuera = “@asistente 继续”
ia-fondo-otra-vez = ↻ PinDesk 在我完成之前关闭了。我会在下方的回复中从头重新开始。
ia-fondo-ya-actuo = 有些步骤已经完成，所以我不会自行重复（否则会做两次）。输入 { $sigue }，我就从这里继续。
ia-fondo-ya-se-retomo = 我已经重新开始过一次，但又被中断了，所以就到此为止。如果想让我再试一次，请输入 { $sigue }。
ia-fondo-viejo = 时间过去太久，无法自行继续。如果你仍然需要，请输入 { $sigue }。
ia-fondo-sin-pregunta = 找不到你的请求（是否已被删除？），所以无法继续。
ia-fondo-no-se-pudo = 无法继续：{ $aviso }
ia-fondo-no-empezo = 我没能开始回答。
ia-fondo-cortado = ⚠️ PinDesk 在此完成之前关闭了。{ $porque }
ia-fondo-sigo-en-el-chat = 💬 我会在聊天中继续。
ia-fondo-sigo-de-fondo = ⏳ 虽然聊天已关闭，我仍在处理你的请求。完成后会通知你。
ia-fondo-pregunta = ❓ 需要你在聊天中回答：{ $pregunta }

    请点击“打开”。如果你不回答，我会选择最稳妥的做法。
# La ficha de una leccion
leccion-ventana = 心得
leccion-ventana-nueva = 新建心得
leccion-titulo = 💡 心得
leccion-titulo-nueva = 💡 新建心得
leccion-tipo-leccion = 💡 心得
leccion-tipo-error = ⚠️ 失误
leccion-tipo-acierto = ✅ 成功
leccion-gravedad-leve = 轻微
leccion-gravedad-importante = 重要
leccion-gravedad-grave = 严重
leccion-sin-proyecto = 无项目（通用聊天）
leccion-foto-ilegible = 无法读取该照片
leccion-adjunto-no-esta = 该文件尚未在这台电脑上
leccion-galeria = 🖼 图库
leccion-guardar = 保存
leccion-pista-titulo = 你学到了什么？一口气输入或口述出来
leccion-se-parece = 这条和你已有的一条很相似
leccion-paso-otra-vez = 🔁 又发生了
leccion-area = 领域
leccion-etiquetas = 标签
leccion-anadir-etiqueta = + 标签
leccion-menos-detalle = ▴  收起详情
leccion-mas-detalle = ▾  发生了什么、为什么，以及我下次会怎么做
leccion-que-paso = 发生了什么
leccion-por-que = 为什么会发生
leccion-causas = 原因（点一下即可）
leccion-proxima = 下次……（如果发生 X，我就做 Y）
leccion-referencias = 用于查找的关键词（用逗号分隔）
leccion-relacionadas = 相关
leccion-volvio-a-pasar = 🔁 我又遇到了
leccion-volvio-a-pasar-n = 🔁 我又遇到了（已 { $veces } 次）
leccion-mic-escuchando = 🎙️ 正在听……点击麦克风结束
leccion-mic-sin-dictado = 这台电脑上没有听写功能：请输入心得
leccion-mic-sin-texto = 什么也没听懂
leccion-mic-no-pasa = 无法转为文字
leccion-mic-corto = 太短了：请让麦克风多开一会儿
leccion-audio = 🎙 音频
leccion-audio-abriendo = 🎙 正在打开…
leccion-audio-guardando = 🎙 正在保存…
leccion-audio-parar = ⏹ 停止 { $duracion }
leccion-audio-sin-microfono = 无法使用麦克风
leccion-audio-corto = 太短：未保存
leccion-audio-no-guarda = 无法保存音频
# El horario
horario-evaluacion-en = { $dias ->
        [1] 明天
       *[other] { $dias } 天后
    }
horario-evaluacion-recuerda = 提醒：{ $que }，{ $en }，{ $cuando }
horario-evento-en = { $evento }，地点：{ $lugar }
horario-dia-0 = 周一
horario-dia-1 = 周二
horario-dia-2 = 周三
horario-dia-3 = 周四
horario-dia-4 = 周五
horario-dia-5 = 周六
horario-dia-6 = 周日
# La Jump List del icono de la barra de tareas
lista-icono-capturar = 截图
lista-icono-capturar-largo = 截取屏幕的一个区域
lista-icono-mensajes = 收藏的消息
lista-icono-mensajes-largo = 打开“收藏的消息”
lista-icono-proyectos = 项目
lista-icono-proyectos-largo = 查看你的项目
lista-icono-sincronizar = 同步
lista-icono-sincronizar-largo = 与你的设备同步
lista-icono-recientes = 最近的项目
lista-icono-proyecto-largo = 打开项目 { $nombre }
# Los visores de planos, modelos y croquis 3D
cad-plano-abriendo = 正在打开图纸…
cad-plano-vacio = 图纸中没有可显示的内容
cad-plano-error = 无法打开图纸
cad-plano-pista-acotar = 点击各点进行测量 · 双击或 Enter：完成 · Esc：退出
cad-plano-todo = 显示整张图纸
cad-tema = 浅色或深色背景
cad-acotar = 测量
cad-borrar-cotas = 清除测量
cad-encima = 始终置顶
cad-cerrar = 关闭
cad-tres = 以 3D 查看
cad-marcos = 打印框
cad-imprimir = 打印…
cad-exportar = 导出（HTML、PDF、PNG、DXF…）…
cad-borrar-marcos = 清除打印框
cad-plano-pista-marcos = 拖动框选要打印的内容 · Ctrl+P：打印 · Del：删除最后一个 · Esc：退出
cad-plano-marcador = 在此添加书签
cad-anotar = 批注
cad-sin-dibujo-civil = 有 { $n } 个 Civil 3D 对象保存时没有图形：请用 PROXYGRAPHICS = 1 保存，或导出为 LandXML
cad-letras = 图纸字体…
cad-letras-faltan = 此图纸需要的字体不在本机；其文字会改用相似字体显示，可能放不进原来的框里：
cad-letras-no-falta = 图纸按名称指定的字体都没有缺失。
cad-letras-donde = SHX 字体通常位于 C:\Program Files\Autodesk\AutoCAD 20xx\Fonts，或在别人发给你的图纸旁边；
    TrueType 字体位于 C:\Windows\Fonts。你在这里添加的字体优先于 AutoCAD、Windows 和 PinDesk 的字体。
cad-letras-anadir = 添加字体（SHX、TTF、OTF、TTC）…
cad-letras-quitar = 移除我添加的（{ $n }）
cad-faltan-letras = 图纸缺少 { $n } 种字体：点击上方的 Aa 添加
cad-releyendo = 正在用你的字体读取图纸…
cad-boton-mover = 移动（拖动）
cad-boton-marcador = 书签
cad-boton-anotar = 批注
cad-boton-letras = 图纸字体
cad-corte = 剖切框
cad-capas = 楼层和类别
cad-boton-pantalla = 全屏
cad-boton-guardar = 保存到聊天
cad-modelo-abriendo = 正在打开模型…
cad-modelo-vacio = 模型中没有可显示的内容
cad-modelo-error = 无法打开模型
cad-modelo-todo = 显示整个模型
cad-aislar = 隔离所选元素
cad-niveles = 楼层
cad-categorias = 类别
cad-sin-nivel = 无楼层
cad-mostrar-todo = 全部显示
cad-aristas = 边线
cad-planta = 平面视图
cad-altura-corte = 剖切
cad-modelo-pista = 拖动：旋转 · 右键拖动：平移 · 滚轮：缩放 · 单击：查看是什么 · 再次单击：隔离
cad-modelo-marcador = 在此添加书签（含剖切和隐藏的内容）
cad-imprimir-marcos = 打印这些框…
cad-modelo-pista-marcos = 拖动框选要打印的内容 · Ctrl+P：打印 · Del：删除最后一个 · 数字：转到其视图 · Esc：退出
cad-modelo-pista-anotar = 你的批注只在此书签的视图中可见
cad-modelo-anotar = 在此视图上批注
cad-croquis-abriendo = 正在打开草图…
cad-croquis-vacio = 草图是空的
cad-croquis-error = 无法打开草图
cad-croquis-todo = 显示整个草图
cad-croquis-capas = 图层
cad-croquis-grupos = 组
cad-croquis-tipos = 类型
cad-croquis-todo-corto = 全部
cad-croquis-pista = 拖动：旋转 · 右键拖动：平移 · 滚轮：缩放 · 1–9：保存的视图
# La pagina HTML de un plano exportado
cad-web-todo = 全部显示
cad-web-tema = 浅色 / 深色
cad-web-capas = 图层
cad-web-grosores = 线宽
cad-web-anotaciones = 批注
cad-web-todas = 全部
cad-web-ninguna = 无
cad-web-sin-soporte = 此浏览器无法解压图纸：请用较新版本的 Edge、Chrome、Firefox 或 Safari 打开。
cad-web-pista = 滚轮或双指：平移 · Ctrl+滚轮或捏合：缩放 · 双击：全部显示 · F、B、L
# El membrete de un plano impreso
lamina-proyecto = 项目
lamina-lamina = 图号
lamina-titulo = 图名
lamina-dibujo = 绘图
lamina-escala = 比例
lamina-sin-escala = 不按比例
lamina-fecha = 日期
lamina-unidades = 单位
lamina-opcion = 图幅
lamina-con-membrete = 带图框和标题栏
lamina-sin-marco = 无图框
# Sincronizar en segundo plano
sinc-fondo-solo = 这台电脑的群组里没有其他设备：请在“同步”中加入一个
sinc-fondo-no-es-del-grupo = 该设备已不在群组中
sinc-fondo-aparatos = { $n } 台设备
sinc-fondo-sincronizando = 正在与 { $quien } 同步…
sinc-fondo-ya-en-marcha = 已经在同步：请等待完成
sinc-fondo-fallo = 无法同步：{ $error }
# Leer en voz
lector-voz-en-linea = 在线
# Automatizaciones
auto-origen = 自动化
# El asistente de la nube sin clave
ia-nube-falta-la-clave = 缺少 { $proveedor } 的密钥：请在“设置 → 助手”中粘贴（可在 { $pagina } 获取）。
# Los campos de una figura que se alarga
figuras-ventana-no-es-numero = “{ $texto }”不是数字
figuras-ventana-mes-mal = “{ $texto }”：月份请写成 2026-11
# Las teclas
tecla-intro = Enter
tecla-supr = Del
ia-fondo-hecho = ✅ 完成。
ia-hecho = 完成。
croquis-titulo = 草图 { $id }
chat-ajustes-bandeja = 设置可从系统托盘中的 PinDesk 图标打开
# El estado de la IA
ia-estado-funciona = 正常
ia-estado-sin-clave = 无密钥
ia-estado-desconocido = 尚未检查
ia-estado-clave-mala = 密钥无效（{ $codigo }）
ia-estado-sin-saldo = 账户余额不足
ia-estado-limite = 请求过多或配额已用完
ia-estado-red = 无网络连接
ia-estado-servidor = 服务出错（{ $codigo }）
ia-estado-otro = 响应异常（{ $codigo }）
ia-estado-comprobando = 正在检查…
ia-estado-hace-nada = 刚刚
ia-estado-hace-min = { $n } 分钟前
ia-estado-hace-horas = { $n } 小时前
ia-estado-pista = 单击：立即检查或前往密钥设置
ia-estado-comprobar = 立即检查
ia-estado-claves = 密钥在“设置 → 助手”中
ia-estado-sin-ajustes = 设置可从系统托盘中的 PinDesk 图标打开
# El mes dentro de una fecha
chat-mes-dia-1 = 1月
chat-mes-dia-2 = 2月
chat-mes-dia-3 = 3月
chat-mes-dia-4 = 4月
chat-mes-dia-5 = 5月
chat-mes-dia-6 = 6月
chat-mes-dia-7 = 7月
chat-mes-dia-8 = 8月
chat-mes-dia-9 = 9月
chat-mes-dia-10 = 10月
chat-mes-dia-11 = 11月
chat-mes-dia-12 = 12月
# Escuchar la reunion en vivo
ajustes2-voz-en-vivo = 实时收听会议所用引擎
ajustes2-voz-en-vivo-sistema = Windows 或 Whisper
ajustes2-voz-en-vivo-vosk = Vosk
ajustes2-ayuda-voz-en-vivo = 用于 Jev 的会议议题等会议功能。Vosk 会实时显示正在说的话，并在你停顿后零点几秒内结束每句话（Whisper 约需 3 秒），无需联网。选择它将下载 { $megas } MB（Apache-2.0）。
ajustes2-ayuda-voz-en-vivo-vosk-listo = 正在用 Vosk 离线收听：正在说的话会显示在录音的全屏界面中。如果某次加载失败，则改用 Windows 或 Whisper。
ajustes2-ayuda-voz-en-vivo-sin-idioma = Vosk 只下载西班牙语和英语版本；在此语言下由 Windows 或 Whisper 收听。
ajustes2-voz-en-vivo-bajando = 正在下载 Vosk… { $por-ciento } %。在此期间由 Windows 或 Whisper 收听。
ajustes2-voz-en-vivo-fallo = 无法下载 Vosk：{ $fallo }。再次选择它即可重试。
ajustes2-voz-vosk-borrar = 删除 Vosk
ajustes2-ayuda-voz-vosk-borrar = 释放已下载的内容（约占 100 MB 磁盘空间）。再次选择时会重新下载。
clase-temas-bateria = 电量低：为节省电量暂停实时收听（录音继续）
chat-anotar-fallo-red = 无网络连接：Jev 无法判断是否要记到另一个聊天中。你口述的内容仍在输入框里。
chat-anotar-fallo = Jev 没有响应（{ $motivo }）。你口述的内容仍在输入框里。
# Privacidad y datos
ajustes2-g-privacidad = 隐私与数据
ajustes2-priv-politica = 隐私政策
ajustes2-ayuda-priv-politica = PinDesk 如何处理你的数据（西班牙语和英语）：除下方所列内容外，一切都留在这台电脑上，而且只在你使用时才会发生。
ajustes2-g-privacidad-sale = 哪些内容会离开这台电脑、去往何处
ajustes2-priv-ver-ajustes = 相关设置
ajustes2-priv-nada-mas = 没有别的
ajustes2-ayuda-priv-nada-mas = 没有广告、没有统计分析、没有崩溃报告、没有账户，也没有 PinDesk 服务器。听写和转写都在这台电脑上离线运行。
ajustes2-g-privacidad-datos = 你的数据
ajustes2-priv-carpeta = 你的数据，在你的磁盘上
ajustes2-ayuda-priv-carpeta = 截图、聊天、笔记、录音和设置都存放在 { $carpeta }。要带走它们，复制该文件夹即可；要删除它们，先关闭 PinDesk 再删除该文件夹。AI 密钥使用你的 Windows 用户加密。
ajustes2-priv-reportes = AI 所写内容的举报
ajustes2-ayuda-priv-reportes = 已保存 { $n } 条，位于 reportes-ia.jsonl（在数据文件夹中）。
privacidad-asistente-que = 你问助手的问题、最近的聊天内容以及你附加的内容
privacidad-asistente-adonde = Anthropic（Claude）或 DeepSeek
privacidad-asistente-cuando = 仅当你粘贴自己的密钥并提问时。密钥加密保存在这台电脑上。
privacidad-jev-que = Jev 各项功能的文本：会议中的句子、课堂的文字、任务、你口述或讲解的内容
privacidad-jev-adonde = TypeSafe（api.typesafe.ai）
privacidad-jev-cuando = 仅当你有 TypeSafe 密钥并开启了该功能时。从不发送音频。
privacidad-web-que = 助手搜索的内容，以及它为回答你而阅读的网页
privacidad-web-adonde = DuckDuckGo 或 Brave Search，以及它阅读的网站
privacidad-web-cuando = 当助手在网上搜索时。不带 Cookie，也不带你的 AI 密钥。
privacidad-google-que = 你选择的 Drive 文件和日历事件
privacidad-google-adonde = Google（Drive、Calendar）
privacidad-google-cuando = 仅当你在“设置 › 助手”中连接了 Google 账户时；也可在那里断开。
privacidad-voces-que = 用 Microsoft 语音朗读的文本
privacidad-voces-adonde = Microsoft（Edge 语音）
privacidad-voces-cuando = 仅当你在阅读器中选择这些语音之一时；Windows 语音离线工作。
privacidad-enlace-que = 你选择通过链接分享的文件
privacidad-enlace-adonde = litterbox.catbox.moe 或 temp.sh
privacidad-enlace-cuando = 仅当你请求分享链接时；它会在几小时或几天内自动删除。
privacidad-modelos-que = 不含你的任何内容：只请求下载文件
privacidad-modelos-adonde = huggingface.co、github.com 和 alphacephei.com（Whisper、Qwen3-ASR、Vosk、Indra）
privacidad-modelos-cuando = 在你要求下载语音或文字识别模型时。
privacidad-sincro-que = 你同步的截图、笔记和聊天
privacidad-sincro-adonde = 你自己的设备（手机），通过你的网络
privacidad-sincro-cuando = 同步时：直接在你的网络内加密传输，不经过任何服务器。
# Reportar lo que escribe la IA
reportar-ia = 举报回答…
reportar-ia-motivo-ofensivo = 冒犯性内容
reportar-ia-motivo-peligroso = 危险内容
reportar-ia-motivo-falso = 虚假或误导
reportar-ia-motivo-otro = 其他原因
reportar-ia-hecho = 举报已保存到 reportes-ia.jsonl；将打开你的邮件以便发送
reportar-ia-asunto = 举报一条 AI 回答
reportar-ia-campo-motivo = 原因
reportar-ia-campo-funcion = 功能
reportar-ia-campo-fecha = 日期
reportar-ia-campo-version = 版本
reportar-ia-campo-contenido = AI 所写的内容
reportar-ia-comentario = （如果愿意，请在这里说明哪里有问题。）
repaso-jev-fallo-red = 无网络连接。

## 导出任务：清单的网页、PDF 和 Markdown（2026-11-01）
tareas-exportar-titulo = 任务
tareas-exportar-exportado = 导出于 { $fecha }
tareas-exportar-pendientes = 待办
tareas-exportar-hechas = 已完成
tareas-exportar-completado = 完成度
tareas-exportar-pendiente = 待办
tareas-exportar-hecha = 已完成
tareas-exportar-todo-hecho = 全部完成
tareas-exportar-vacia = 没有任务
tareas-exportar-recordatorio = 提醒
tareas-exportar-pasada = 已逾期
tareas-exportar-creada = 创建于 { $fecha }
tareas-exportar-de = { $hechas } / { $total }
tareas-exportar-foto = 照片
compartir-tipo-tareas = 任务清单

## La nube de los chats compartidos (1-nov-2026, sincronizar::nube)
chat-nuevo-nube = 云端共享聊天
chat-nuevo-nube-detalle = 你上传的内容，其他人在任何地方都能下载
nube-modo-titulo = 同步方式
nube-modo-local = 局域网
nube-modo-nube = 云端
nube-modo-local-detalle = 在同一 Wi-Fi 下同步，免费且无需服务器。
nube-modo-nube-detalle = 通过你的云端：你上传的内容其他人在任何地方都能下载，并可在网页上查看。只上传变化的部分。
nube-falta-configurar = 尚未设置云端：请在 同步 → 云端 中填写地址和密钥。
nube-titulo = 云端
nube-portada-como = 设为「云端」的共享聊天通过你的服务器同步。你自己的聊天仍走局域网。
nube-ajustes = 云端设置
nube-explica = 你的服务器保存通过云端同步的共享聊天，并在网页上显示。个人聊天仍走局域网，无需服务器。
nube-direccion = 网址
nube-llave = 设备密钥（pxn_…）
nube-llave-guardada = 已保存的密钥：
nube-olvidar = 忘记
nube-guardar = 保存
nube-probar = 测试
nube-guardado = 已保存。
nube-direccion-mala = 网址必须以 https:// 开头。
nube-llave-mala = 密钥以 pxn_ 开头（注册此设备时网页会给你）。
nube-probado = 已连接：空间 { $espacio }。
nube-no-contesta = 云端没有响应：
nube-que-va = 只上传变化的部分：在已标注的图纸上，只上传新的笔画，而不是整张图纸。
nube-en-linea = 在线
nube-sin-conexion = 离线
nube-cerrar-titulo = 离线修改
nube-cerrar-pregunta =
    你在离线状态下标注了此文档。
    是：放弃这些修改（保留其他人的）。
    否：恢复在线并上传。
    取消：下次仍保持离线。
