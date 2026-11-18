# Purifex · Diseño (2026-11-18)

Referencia: `design/screen.png`. Este documento dice **qué se toma de ella, qué se cambia y por qué**, y fija los valores del sistema de diseño (`crates/purifex-ui/src/theme.rs` los implementa).

## Qué se toma

| Elemento de la referencia | Cómo se aplica |
|---|---|
| Tema oscuro casi negro con **un solo acento verde menta** (`#00f2aa`) y rojo / azul / ámbar solo para estados | Variables CSS `--accent`, `--red`, `--blue`, `--amber`. Nada más lleva color |
| Tipografía **monoespaciada** para datos, etiquetas y rutas; titulares en sans grueso | `Cascadia Mono` → `Consolas` y `Segoe UI Variable` → `Segoe UI` (DirectWrite elige la primera instalada; nada se descarga) |
| Etiquetas en **mayúsculas con tracking**, 10–11 px, color apagado | `theme::eyebrow` |
| **Barra lateral** con icono + texto, elemento activo con borde y fondo menta tenue, insignias a la derecha | `shell::sidebar` |
| **Barra superior** con la marca y chips de estado | `shell::topbar` |
| **Tarjetas de indicadores** con esquinas marcadas (corchetes de 1 px en menta), etiqueta arriba, cifra grande, nota debajo | `widgets::kpi_card` |
| **Filtros en chips con contador** y un punto de color por estado | `drivers_page` (chips que pasan a otra fila si no caben) |
| **Tabla de dispositivos**: icono de la clase, nombre + ruta en mono, columnas (subsistema, versión, impacto, acción), etiquetas de estado | `drivers_page`, `widgets::tag` |
| **Consola de eventos** abajo, líneas con fecha y etiqueta entre corchetes | `overview_panels::console` |
| Panel de **parámetros** con casillas | Panel «Principios» |
| Iconos **Lucide** (ISC) en trazo de 2 px | `domain::icons`: el SVG de cada icono se lee a curvas Bézier y Direct2D lo traza |

## Qué se cambia (y por qué)

La referencia es un *mockup* con un lenguaje de «kernel / Ring0 / bypass de telemetría / inyección» que **Purifex no puede ni debe prometer**. Se adopta el aspecto, no esos mensajes:

- **Textos veraces.** Cada cifra y cada etiqueta sale de un dato medido (`scan`, `watch`). Nada de «DMA Ring-0», «bypass de telemetría», «WHQL Root 2025» ni cifras de ejemplo. Si un dato no existe todavía, la tarjeta dice «sin datos», no inventa.
- **Sin acciones que aún no existen.** «Inyectar», «Actualizar» y «Desplegar parches» no se dibujan hasta que haya instalación real (bloques 4–6 del plan). Lo que sí hay: **Reescanear** y consultar partidas. Las secciones futuras (Arreglos, Historial) aparecen apagadas con la insignia «pronto».
- **Latencia DPC en µs → tiempo en DPC/interrupciones en %**, que es lo que Purifex mide de verdad (p95 de la partida).
- **Sin barra de ventana propia:** la ventana usa el marco nativo de Windows, con la barra de título oscura (Windows 11).
- **Nombres en español** y sin jerga innecesaria («Controladores» no «Firmware & Drivers» salvo que sea firmware).

## Páginas de la primera entrega

| Página | Icono Lucide | Contenido |
|---|---|---|
| Estado general | `gauge` | 4 tarjetas (DPC, drivers, limitante principal, GPU), pistas del diagnóstico, consola de eventos, principios |
| Drivers | `layers` | Buscador, filtros por categoría con contador, tabla de dispositivos (agrupados), reescaneo |
| Partidas | `gamepad-2` | Lista de grabaciones y, al elegir una, sus indicadores y pistas |
| Arreglos | `sliders-horizontal` | Apagada («pronto») |
| Historial | `history` | Apagada («pronto») |

## Sistema de diseño

### Color
| Token | Valor | Uso |
|---|---|---|
| `--bg` | `#07080b` | Fondo de la ventana y de la consola |
| `--bg-side` | `#090b0f` | Barra lateral |
| `--panel` | `#0d0f15` | Tarjetas y paneles |
| `--panel-2` | `#0c0e14` | Tabla |
| `--line` | `#1e2026` | Bordes |
| `--line-soft` | `#161922` | Separadores de filas |
| `--text` | `#f2f2f2` | Texto principal |
| `--text-2` | `#9aa3b2` | Texto secundario |
| `--text-3` | `#5e6876` | Etiquetas, ayudas |
| `--accent` | `#00f2aa` | Acción principal, activo, correcto |
| `--red` | `#f43f5e` | Fallo, sin driver |
| `--blue` | `#38bdf8` | Información, recomendado |
| `--amber` | `#fbbf24` | Aviso, optimización |

Cada color de estado tiene un fondo tenue (`rgba(color, .12)`) y un borde (`rgba(color, .35)`) para sus etiquetas.

### Tipografía
- Datos, etiquetas, rutas, consola: mono, 12–13 px. Etiquetas: 10–11 px, mayúsculas, `letter-spacing: .12em`.
- Titulares: `Segoe UI Variable Display`, 700, 34 px (página), 22 px (cifra de tarjeta).

### Forma y espacio
- Radio 8 px (tarjetas), 6 px (botones, chips), 999 px (etiquetas y puntos).
- Rejilla de 8 px; tarjetas con 16–20 px de relleno; ancho de la barra lateral 264 px.
- Un solo foco: el botón principal es el único elemento relleno de menta en pantalla.

### Iconos
Lucide `1.75.0` (ISC), cuadrícula de 24, trazo de 2, extremos y uniones redondeados. 52 iconos (`crates/purifex-domain/src/icons/`), cada uno con su SVG original; el lector (`svg_path.rs`) convierte arcos, círculos y rectángulos a curvas cúbicas. Un test comprueba que los 52 se leen y caben en su cuadrícula. La licencia está en `crates/purifex-ui/assets/LICENSE-lucide.txt`.

### Accesibilidad
Contraste del texto sobre fondo ≥ 4.5:1 (texto principal y secundario); el buscador con foco se enciende en menta y muestra el cursor; los estados no dependen solo del color (llevan icono y texto); no hay animaciones.

## Cómo está construido

Interfaz **nativa** (decidido 2026-11-18). La primera versión fue una página web en una ventana de Edge; medida en este PC ocupaba **16 procesos, 936 MB y un núcleo ocupado**, y se descartó: una app que diagnostica rendimiento no puede gastar el que mide.

```
purifex-domain   Model, Action, DrawList (formas, texto, iconos), iconos, y todo lo que se muestra (Overview, DriversView, Analysis::stat_rows)
purifex-ui       Construye la lista de dibujo desde el Model y convierte ratón y teclas en Action. No conoce el sistema: mide el texto por un puerto
purifex-app      Core: aplica cada Action y lee lo que falta (solo la primera vez)
purifex-platform Ventana Win32 (solo mensajes), Direct2D (dibujo), DirectWrite (texto), WIC (captura a PNG)
purifex          shell/: conecta Core, Ui y la ventana. Sin lógica
```

Bucle: mensaje del sistema → `UiEvent` → `Ui::event` → `Action` → `Core::apply` → `Ui::frame(Model)` → lista de dibujo → Direct2D. La ventana espera en `GetMessage`: **sin eventos no hay CPU**. Una acción que necesita leer (reescanear) marca «leyendo», la ventana pinta eso y solo entonces `Core::tick` lee, así que la pantalla nunca queda congelada sin explicación.

**Medido** (2026-11-18, ventana abierta y quieta): 1 proceso, 48 MB, 0 % de CPU; `scan` sin ventana, 9 MB de pico. Cerrar la ventana termina el programa.

`purifex ui --screenshot archivo.png [--page overview|drivers|sessions] [--size 1440x900] [--scale 2]` dibuja la misma pantalla sin abrir la ventana: es como se compara con la referencia desde un script.
