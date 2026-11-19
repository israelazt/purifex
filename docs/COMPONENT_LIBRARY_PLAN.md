# Purifex · Plan de la librería de componentes (2026-11-18)

Objetivo: llevar a otros proyectos lo que ya funciona aquí (la interfaz nativa, los diálogos animados, la barra de desplazamiento, los iconos, la ventana y el renderizador, y los guardianes de arquitectura) **sin copiar y pegar y sin romper Purifex**.

Principio rector: **Purifex es el primer cliente de la librería, no su víctima.** Cada paso deja a Purifex igual (mismas capturas, mismas pruebas, CI en verde) y la librería sale como consecuencia, no como un rediseño.


**Decisión de 2026-11-18: repositorios separados desde el principio** (el trabajo va a ser público y se usará como portafolio). Eso cambia el orden: la librería **no** vive primero dentro de Purifex; se crea su repositorio pronto y Purifex pasa a consumirla. Lo que se conserva: Purifex sigue siendo el primer cliente y **no se rompe** (mismas capturas, mismas pruebas, CI en verde en cada paso).

---

## 1. Inventario medido

Datos del código de hoy (no suposiciones):

| Pieza | Dónde | Líneas | Acoplamiento a Purifex |
|---|---|---|---|
| Lista de dibujo, eventos, `TextMetrics`, macro de log | `purifex-domain` (`draw.rs`, `ui_event.rs`, `ports.rs`) | ~500 | **Ninguno** |
| Iconos Lucide (53, lector SVG→Bézier) + licencia | `purifex-domain/icons`, `purifex-ui/assets` | — | **Ninguno** |
| Animación (`anim.rs`), barra de desplazamiento (`scrollbar.rs`) | `purifex-ui` | 312 + 151 | **Ninguno** |
| Ventana Win32, Direct2D/DirectWrite, WIC, capturas a PNG | `purifex-platform` (`window/`, `gfx/`) | ~1350 | **Bajo**: usa `UiHandler`/`draw`; 3 recursos con `include_bytes!` (logo, icono) |
| `Canvas`, `widgets`, `dialog` | `purifex-ui` | 438 + 260 + 485 | **Medio**: importan `Action`/`Model` de `view`; `widgets` y `dialog` usan `theme::` fijo (21 usos) y textos en español |
| Núcleo de `Ui` (desplazar, hover por regiones, modal, animación) | `purifex-ui/ui.rs` | 997 | **Medio**: mezcla lo genérico con `Page`, `Ctrl+K→Drivers`, el buscador |
| Páginas (`*_page`, `shell`, `overview_*`…) | `purifex-ui` | — | **Total** (13 módulos importan `view`; ~250 usos de `theme::`): **se quedan en Purifex** |
| Guardianes de arquitectura | `purifex-domain/tests/architecture.rs` | ~210 | **Bajo**: nombres de crates escritos dentro |
| Motor de restos, cuarentena, `judge`, historial | `purifex-app`/`-platform` | — | **Total**: es dominio de Purifex; **no va a la librería** |

Conclusión: ~60 % de lo que quiero llevarme ya es independiente. Lo que estorba son **tres acoplamientos concretos**: el tipo `Action`, los colores fijos y los recursos/textos incrustados.

---

## 2. Arquitectura destino

Repositorio **`fexarium-ui`** (nombre propuesto, de la familia Fexarium), con crates pequeños:

```
fex-core     lista de dibujo (Color, Rect, Op, DrawList), UiEvent/Key, UiHandler, TextMetrics,
             macro de log, iconos Lucide + licencia                       (sin sistema, forbid(unsafe))
fex-ui       Canvas<A>, Anims, widgets, dialog, scrollbar, Theme, y el núcleo `Kernel<A>`
             (desplazamiento, hover por regiones, modal, animación)       (sin sistema, forbid(unsafe))
fex-win      ventana Win32, Direct2D/DirectWrite, WIC, capturas, icono    (único con unsafe; solo Windows)
fex-guards   las pruebas de arquitectura como biblioteca de pruebas parametrizable
```

Reglas de dependencias (las vigila `fex-guards`): `fex-core` ← `fex-ui` ← (apps); `fex-core` ← `fex-win` ← (apps). `fex-ui` y `fex-core` **compilan y se prueban en Linux** (como hoy las capas portables de Purifex); solo `fex-win` es Windows.

Lo que se queda en Purifex: `Model`, `Action`, `Page`, todas las páginas, `shell`, textos, paleta, motor de restos, adaptadores de sistema.

---

## 3. Los cambios de diseño (los que hacen posible la extracción)

### 3.1 `Action` genérica
`Canvas<A>`, `Hit<A>`, `DialogView<A>`, botones y filas pasan a ser genéricos sobre `A: Clone + PartialEq`. Hoy `Canvas::hit(rect, Action)`; mañana `Canvas<Action>`: **el código de las páginas de Purifex casi no cambia** (se fija `A = Action` con un alias `type Canvas<'a> = fex_ui::Canvas<'a, Action>`).

### 3.2 Núcleo de `Ui` separado de las páginas
`Ui` se parte en:
- **`Kernel<A>`** (genérico): desplazamiento suave, barra, teclas de desplazamiento, regiones de hover, modal y diálogos animados, escape.
- **`Ui` de Purifex**: `Kernel<Action>` + qué página dibujar + `Ctrl+K`, el buscador y la lectura de datos.
El `Kernel` recibe la pantalla por un trait pequeño: `fn draw(&mut self, canvas, area) -> alto` y `fn dialog(&self) -> Option<DialogView<A>>`.

### 3.3 Tema como datos
`Theme { colores, radios, espacios, fuentes }` que se pasa al `Canvas` (`c.theme()`). **Solo la librería** (widgets, diálogo, barra) usa el tema del canvas (~60 usos). Las páginas de Purifex siguen con su `theme::X` (la paleta menta de Purifex queda como `Theme::purifex()`), así que **no hay que reescribir los ~250 usos**.

### 3.4 Recursos y textos inyectados
- Imágenes: `Renderer::register_image(nombre, bytes)` y `WindowOptions { icon_png, title, size, min_size }`; se acaban los `include_bytes!` dentro del crate de ventana.
- Textos fijos de widgets y diálogos («Cancelar», «Confirmar»…): los pone **quien llama** (parámetros o un `Strings`), no la librería. La librería no contiene idioma.

### 3.5 Iconos extensibles
Registro de iconos con los 53 de Lucide por defecto y `register_icon(nombre, svg)` para añadir los de cada app; la licencia ISC viaja con el crate.

### 3.6 Pruebas y utilidades
`FakeText` y ayudantes de captura salen bajo la característica `testing`. Las pruebas genéricas (animación, barra, diálogo, núcleo) **se mueven con su código**.

---

## 4. Fases (tareas atómicas)

Cada tarea cierra con **pruebas, `tools/check.ps1`, clippy de Linux, commit, push y CI en verde en ese SHA**, y con la **comparación de capturas** de §6.A.

### Fase 0 · Preparar dentro de Purifex (sin mover nada de sitio)
- 0.1 **Red de seguridad visual:** una herramienta que dibuja cada página (y los diálogos) a PNG y la **compara con las capturas de referencia** (tolerancia mínima); guardar las de hoy como referencia. *Sin esto no se refactoriza.*
- 0.2 `Canvas`/`Hit`/botones/diálogo **genéricos sobre la acción**; alias en Purifex.
- 0.3 `Theme` como datos; `widgets`, `dialog` y `scrollbar` lo leen del canvas; `Theme::purifex()` reproduce los colores de hoy.
- 0.4 Separar el **`Kernel<A>`** de `Ui`; `Ui` de Purifex lo envuelve. Las ~50 pruebas de desplazamiento/modal/hover de `ui.rs` pasan a probar el kernel.
- 0.5 Textos de widgets/diálogo como parámetros; Purifex los pasa en español.
- 0.6 Recursos inyectables y `WindowOptions`; quitar `include_bytes!` del crate de ventana.
- 0.7 Registro de iconos extensible.
**Aceptación de la fase:** capturas idénticas, 0 cambios de comportamiento, CPU en reposo 0 %.

### Fase 1 · Repositorio `fexarium-ui` (público) y traslado con historial
- 1.1 **Preparar el repositorio vacío con todo lo de «presentable»** (ver §8): licencia, README, `.gitignore`, CI de Windows y Linux, plantillas de issues, descripción y etiquetas.
- 1.2 **Trasladar con historial**: clonar Purifex, `git filter-repo --path <los archivos de la librería>` y subir el resultado a `fexarium-ui`, de modo que cada archivo conserva sus commits y su autoría (el historial *es* parte del portafolio).
- 1.3 Reestructurar en los crates `fex-core`, `fex-ui`, `fex-win`, `fex-guards` (§2); `fex-ui` y `fex-core` compilan y pasan clippy en Linux.
- 1.4 **Generalizar los guardianes** (`architecture.toml` por proyecto) y añadir el que prohíbe la palabra «purifex» y cualquier dependencia de `purifex-*` dentro de la librería.
- 1.5 Tres **ejemplos** que compilan y se capturan: `hello_window`, `dialog_demo` (diálogo animado con casillas) y `scroll_demo` (lista larga con barra y teclado).
- 1.6 Primera etiqueta **`v0.1.0`**.
**Aceptación:** la librería compila y pasa su CI sola, sin Purifex; los ejemplos funcionan en la ventana real.

### Fase 2 · Purifex pasa a consumir la librería
- 2.1 En Purifex, sustituir el código trasladado por la dependencia **de git con etiqueta** (`fexarium-ui = { git = "…", tag = "v0.1.0" }`); borrar lo duplicado.
- 2.2 **Desarrollo conjunto sin esperar etiquetas:** repositorios hermanos en el disco y un `.cargo/config.toml` local (no versionado) con `[patch."https://github.com/israelazt/fexarium-ui"]` apuntando a la copia local. El CI de Purifex siempre usa la etiqueta.
- 2.3 Procedimiento documentado: cambio en la librería → etiqueta nueva → subir la versión en Purifex → ambos CI en verde.
**Aceptación:** capturas de Purifex idénticas a las de referencia de la Fase 0; sin código duplicado entre repos.

### Fase 3 · Segundo proyecto como prueba de verdad
- 3.1 Crear el proyecto nuevo con la skill `fexarium-rust` ya apuntando a la librería (la skill genera dependencias en lugar de copiar código).
- 3.2 Construir su primera pantalla **solo con la API pública**. Todo lo que falte o estorbe se anota en `docs/API_FRICTION.md`.
- 3.3 Corregir la API según esa fricción (y migrar Purifex si hace falta). **Esto es lo que hace útil la librería**: la forma que tiene hoy es la de una sola app.
- 3.4 Con un tercer uso (o tras un par de ciclos), congelar y publicar **1.0** con compatibilidad semver.

### Fase 4 · Mantenimiento
- 4.1 `CHANGELOG` y política: cambio incompatible = versión mayor; deprecaciones con un ciclo de aviso.
- 4.2 Actualizar la skill `fexarium-rust` y su generador (dependencias en vez de copias; guardianes desde `fex-guards`).
- 4.3 Documentar el patrón (no el código) de la parte de Purifex que **no** se extrae: `judge`, cuarentena, historial, deshacer; como guía para otros proyectos con cambios destructivos.

---

## 5. Orden recomendado y esfuerzo relativo

| Fase | Esfuerzo | Riesgo | Se puede parar aquí |
|---|---|---|---|
| 0 · Preparar dentro de Purifex | **L** (la mayor parte del trabajo) | Medio (refactor grande) | Sí: Purifex queda mejor diseñado aunque no haya librería |
| 1 · Repositorio de la librería con historial | M | Medio (dos repos que evolucionan juntos) | Sí: ya hay librería pública reutilizable |
| 2 · Purifex consume la librería | S–M | Bajo | Sí |
| 3 · Segundo proyecto | M–L | Medio (la API cambiará) | — |
| 4 · Mantenimiento | S continuo | Bajo | — |
| **P · Presentación pública** (§8) | M, **en paralelo** a todo | Bajo | Se hace por repositorio, empezando por Purifex |

Lo que no depende del orden: **la Fase 0.1 (capturas de referencia) va primero**, siempre. Y **la Fase P para Purifex puede empezar hoy**: no depende de la librería.

---

## 6. Criterios de aceptación globales

A. **Purifex idéntico:** capturas de todas las páginas y diálogos iguales antes y después de cada fase (tolerancia mínima), y mismo comportamiento en las pruebas existentes.
B. **Aislamiento:** ningún crate de `libs/` depende de `purifex-*` ni contiene la palabra «purifex» (guardián automático).
C. **Portabilidad conservada:** `fex-core`, `fex-ui` y `fex-guards` compilan y pasan clippy en Linux.
D. **Coste:** ventana abierta y quieta 0 % de CPU; animación solo mientras algo se mueve (como hoy).
E. **Uso real:** el segundo proyecto monta su primera pantalla sin tocar el código de la librería salvo por fricciones registradas.
F. **Documentación:** un `README` por crate, tres ejemplos que compilan y una guía de arranque.

---

## 7. Riesgos y decisiones abiertas

| Riesgo | Mitigación |
|---|---|
| **Abstracción prematura** (la forma de una sola app) | Fases 0–2 sin prometer estabilidad (`0.x`); congelar `1.0` solo tras un segundo/tercer proyecto |
| Los genéricos (`Canvas<A>`) complican el código | Alias de tipo en cada app (`type Canvas = fex_ui::Canvas<Action>`); se prueba en la Fase 0 antes de mover nada |
| Refactor grande rompe lo visual | 0.1 primero; capturas de referencia por cada paso |
| Dos repositorios que evolucionan a la vez (por decisión de publicarlos por separado) | Etiquetas semver, `[patch]` local para desarrollar juntos, CI de cada repo contra la etiqueta, y el traslado se hace **tarde dentro de la Fase 1** (después de la Fase 0, cuando la API ya está desacoplada) |
| **Renderizador solo Windows** | El límite ya es un trait (`UiHandler` + lista de dibujo): otro backend (p. ej. otro sistema) es posible después sin tocar `fex-ui` |
| Licencias | Lucide es ISC (la licencia va con el crate); las fuentes (Segoe UI Variable, Cascadia Mono) son **del sistema**, no se redistribuyen |
| Deriva entre Purifex y la librería | Dependencia por etiqueta + `[patch]` local para desarrollar; CI de Purifex prueba contra la librería |

**Decisiones que necesito de ti**
1. **Nombre del repositorio de la librería** (propongo `fexarium-ui`) y **licencia** (recomiendo `MIT OR Apache-2.0`, la combinación habitual en Rust).
2. **Idioma de los README:** inglés para alcance en LinkedIn (con una versión en español), o solo español. Recomiendo **inglés como principal** y español como segundo archivo.
3. **¿Cuál es el segundo proyecto?** Decide qué priorizar de la API (tablas, formularios, gráficas…).
4. **Alcance multiplataforma:** solo Windows por ahora (recomendado).
5. **Correo de los commits:** hoy es el personal; para repositorios públicos puede usarse el de GitHub (`<id>+israelazt@users.noreply.github.com`) a partir de ahora. Reescribir el historial ya publicado es opcional y delicado: no lo recomiendo salvo que quieras ocultarlo.

---

## 8. Presentación pública (LinkedIn / portafolio)

Lo que convierte un buen repositorio en uno que se **entiende en 30 segundos**. Se aplica a cada repositorio (`purifex`, `fexarium-ui`, y la plantilla/skill si se publica).

**Estado de hoy (comprobado):** `purifex` es público pero **sin licencia, sin descripción, sin etiquetas, sin capturas y con el README desactualizado** (37 líneas; no menciona la interfaz, el desinstalador, los diálogos ni el servidor MCP). Sin licencia, por defecto **nadie puede reutilizar el código legalmente**. No hay rutas ni datos personales en los archivos versionados.

**Checklist por repositorio**
1. **Licencia** (`LICENSE-MIT` y `LICENSE-APACHE`), y la de Lucide (ISC) donde se usan sus iconos.
2. **README que cuenta una historia:** una frase de qué es; **capturas o un GIF corto** (el diálogo animado, la lista de restos, la barra); qué lo hace distinto (nativo, 0 % de CPU en reposo, sin telemetría, sin proceso residente); **cifras medidas** (1 proceso, ~48 MB frente a 16 procesos y 936 MB de la primera versión web); cómo compilar; arquitectura en un diagrama.
3. **`ARCHITECTURE.md` y registro de decisiones** (ya hay base: capas, guardianes, planes). Es lo que más impresiona a un revisor técnico.
4. **Insignias** del CI (Windows y Linux), versión y licencia.
5. **Descripción, temas (topics) y imagen social** del repositorio (con el logo); fijar los repositorios en el perfil de GitHub.
6. **Releases:** cada etiqueta publica un `.zip` con `purifex.exe`, su **hash SHA-256** y las notas de versión. Aviso: un `.exe` sin firmar muestra la advertencia de SmartScreen; hay que explicarlo en el README (la firma de código es de pago).
7. **`CONTRIBUTING`, plantillas de issues y `SECURITY.md`** (cómo reportar un fallo).
8. **Avisos legales y de responsabilidad:** Purifex **borra y mueve archivos y claves de registro**: dejar claro que se usa bajo responsabilidad propia, que hay vista previa, cuarentena y deshacer, y que se probó con apps de mentira. «League of Legends» es marca de Riot Games: añadir *«no afiliado ni respaldado por Riot Games»*. En la documentación pública, comparar con herramientas como Revo Uninstaller **sin copiar su marca ni su código** («inspirado en herramientas de desinstalación con limpieza de restos»).
9. **Revisar lo que no es tuyo antes de publicar más:** `design/screen.png` es una referencia de diseño; **confirmar su origen y permiso** antes de dejarla en un repositorio público (si no es tuya, quitarla del historial o sustituirla por capturas propias). Los iconos son Lucide (ISC, con su licencia); las fuentes son del sistema y no se distribuyen.
10. **Sin datos personales:** ya se comprueba; mantener un chequeo en el CI (rutas de usuario, correos, tokens) para que no entren.

**Contenido para LinkedIn (sin código):** lo que se puede contar con evidencia propia — de una app web de 936 MB a una nativa de 48 MB y 0 % de CPU; cómo se decidió que la IA solo proponga y el código verifique; cómo se probó el desinstalador (apps de mentira, cuarentena con deshacer); y la comparación honesta con Revo (18 archivos y 19 claves frente a 2 carpetas) como punto de partida de la limpieza profunda. Los números salen de las mediciones y pruebas ya documentadas.
