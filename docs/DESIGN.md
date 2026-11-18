# Purifex · Diseño (2026-11-18)

Referencia: `design/screen.png`. Este documento dice **qué se toma de ella, qué se cambia y por qué**, y fija los valores del sistema de diseño (`crates/purifex-ui/assets/app.css` los implementa).

## Qué se toma

| Elemento de la referencia | Cómo se aplica |
|---|---|
| Tema oscuro casi negro con **un solo acento verde menta** (`#00f2aa`) y rojo / azul / ámbar solo para estados | Variables CSS `--accent`, `--red`, `--blue`, `--amber`. Nada más lleva color |
| Tipografía **monoespaciada** para datos, etiquetas y rutas; titulares en sans grueso | `Cascadia Mono` → `Consolas` (siempre presentes en Windows 11, sin descargar fuentes) y `Segoe UI Variable` |
| Etiquetas en **mayúsculas con tracking**, 10–11 px, color apagado | Clase `.eyebrow` |
| **Barra lateral** con icono + texto, elemento activo con borde y fondo menta tenue, insignias a la derecha | `.nav`, `.nav-item.active`, `.badge` |
| **Barra superior** con la marca y chips de estado | `.topbar`, `.chip` |
| **Tarjetas de indicadores** con esquinas marcadas (corchetes de 1 px en menta), etiqueta arriba, cifra grande, nota debajo | `.kpi` con pseudo-elementos en las 4 esquinas |
| **Filtros en chips con contador** y un punto de color por estado | `.filters`, `.filter` |
| **Tabla de dispositivos**: icono de la clase, nombre + ruta en mono, columnas (subsistema, versión, impacto, acción), etiquetas de estado | `.table`, `.tag` |
| **Consola de eventos** abajo, líneas con hora y etiqueta entre corchetes | `.console` |
| Panel de **parámetros** con casillas | Panel «Principios» |
| Iconos **Lucide** (ISC) en trazo de 2 px, heredando el color del texto | `icons.js` (SVG inline, subconjunto) |

## Qué se cambia (y por qué)

La referencia es un *mockup* con un lenguaje de «kernel / Ring0 / bypass de telemetría / inyección» que **Purifex no puede ni debe prometer**. Se adopta el aspecto, no esos mensajes:

- **Textos veraces.** Cada cifra y cada etiqueta sale de un dato medido (`scan`, `watch`). Nada de «DMA Ring-0», «bypass de telemetría», «WHQL Root 2025» ni cifras de ejemplo. Si un dato no existe todavía, la tarjeta dice «sin datos», no inventa.
- **Sin acciones que aún no existen.** «Inyectar», «Actualizar» y «Desplegar parches» no se dibujan hasta que haya instalación real (bloques 4–6 del plan). Lo que sí hay: **Reescanear** y consultar partidas. Las secciones futuras (Arreglos, Historial) aparecen apagadas con la insignia «pronto».
- **Latencia DPC en µs → tiempo en DPC/interrupciones en %**, que es lo que Purifex mide de verdad (p95 de la partida).
- **Sin barra de ventana propia:** la ventana es la de Edge en modo aplicación (tiene su marco nativo y sus botones).
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
Lucide `1.75.0` (ISC), 24×24, `stroke-width: 2`, `currentColor`; 16 px en listas y botones, 18 px en tarjetas. La licencia está en `crates/purifex-ui/assets/LICENSE-lucide.txt`. Los iconos usados están en `icons.js`; un test comprueba que ningún icono pedido falta.

### Accesibilidad
Contraste del texto sobre fondo ≥ 4.5:1 (texto principal y secundario); foco visible con anillo menta; los estados no dependen solo del color (llevan icono y texto); `prefers-reduced-motion` desactiva las animaciones.

## Cómo está construido

```
purifex-ui (assets)   HTML + CSS + JS + iconos, incluidos en el .exe. Sin lógica de decisión
purifex-domain        Lo que se muestra se decide aquí: Overview, DriversView, categorías (con tests)
purifex-app           web: la API local (rutas, token, validación) y el análisis de partidas
purifex (binario)     shell: el socket en 127.0.0.1, abrir Edge en modo aplicación, apagado por inactividad
```

Seguridad del servidor local (una página web cualquiera no debe poder hablar con Purifex):
1. Solo escucha en `127.0.0.1`, en un puerto aleatorio.
2. Cada ejecución genera un **token** aleatorio; toda petición a `/api/` debe llevarlo en la cabecera `X-Purifex-Token`. La URL inicial lo trae una vez y la página lo quita de la barra.
3. Se valida la cabecera `Host` (defensa contra *DNS rebinding*).
4. Sin CORS; cabeceras `Content-Security-Policy`, `X-Content-Type-Options` y `Cache-Control: no-store`.
5. En esta entrega la API es **solo de lectura**.
6. El servidor se apaga si la página deja de latir 15 s (se cerró la ventana): no queda ningún proceso residente.
