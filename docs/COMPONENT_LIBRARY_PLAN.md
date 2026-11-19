# Purifex · Plan de la librería de componentes (2026-11-18)

Objetivo: llevar a otros proyectos lo que ya funciona aquí (la interfaz nativa, los diálogos animados, la barra de desplazamiento, los iconos, la ventana y el renderizador, y los guardianes de arquitectura) **sin copiar y pegar y sin romper Purifex**.

Principio rector: **Purifex es el primer cliente de la librería, no su víctima.** Cada paso deja a Purifex igual (mismas capturas, mismas pruebas, CI en verde) y la librería sale como consecuencia, no como un rediseño.

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

### Fase 1 · Crates de la librería dentro del mismo repositorio
- 1.1 Crear `libs/fex-core`, `libs/fex-ui`, `libs/fex-win` como miembros del workspace; **mover** (con `git mv`, conservando historial) el código ya desacoplado; Purifex pasa a depender de ellos por ruta.
- 1.2 **Generalizar los guardianes**: la lista de dependencias permitidas por crate se lee de un archivo de configuración (`architecture.toml`); `fex-guards` expone las pruebas; Purifex las usa desde ahí. Añadir un guardián nuevo: **ningún crate de `libs/` menciona `purifex`** (texto ni dependencias).
- 1.3 Ejemplos mínimos que compilan y se capturan: `hello_window` (ventana + una tarjeta), `dialog_demo` (diálogo animado con casillas), `scroll_demo` (lista larga con barra y teclado).
- 1.4 Documentación: `README` por crate, guía «cómo montar una app nueva sobre la librería», lista de decisiones (este plan).
**Aceptación:** Purifex compila y pasa **solo** usando los crates de `libs/`; los ejemplos funcionan en la ventana real.

### Fase 2 · Sacar a repositorio propio
- 2.1 Extraer `libs/` a `fexarium-ui` conservando el historial (`git filter-repo --path libs/`); versión **0.1.0**; etiquetas.
- 2.2 CI propio: Windows (todo) y Linux (`fex-core`, `fex-ui`, `fex-guards`), clippy `-D warnings`, comparación de capturas de los ejemplos.
- 2.3 Purifex consume la librería por **dependencia de git con etiqueta** (no por ruta). Un `Cargo.toml` con `[patch]` opcional para desarrollar ambos a la vez.
- 2.4 Procedimiento documentado: cambiar la librería → etiqueta nueva → subir la versión en Purifex.

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
| 0 · Preparar | **L** (la mayor parte del trabajo) | Medio (refactor grande) | Sí: Purifex queda mejor diseñado aunque no haya librería |
| 1 · Crates internos | M | Bajo | Sí: ya hay librería reutilizable por ruta |
| 2 · Repositorio propio | S | Bajo | Sí |
| 3 · Segundo proyecto | M–L | Medio (la API cambiará) | — |
| 4 · Mantenimiento | S continuo | Bajo | — |

Lo que no depende del orden: **la Fase 0.1 (capturas de referencia) va primero**, siempre.

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
| Mantener dos repositorios | Monorepo interno primero (Fase 1); repo propio solo cuando haya un segundo consumidor |
| **Renderizador solo Windows** | El límite ya es un trait (`UiHandler` + lista de dibujo): otro backend (p. ej. otro sistema) es posible después sin tocar `fex-ui` |
| Licencias | Lucide es ISC (la licencia va con el crate); las fuentes (Segoe UI Variable, Cascadia Mono) son **del sistema**, no se redistribuyen |
| Deriva entre Purifex y la librería | Dependencia por etiqueta + `[patch]` local para desarrollar; CI de Purifex prueba contra la librería |

**Decisiones que necesito de ti**
1. **Nombre y visibilidad** del repositorio (propongo `fexarium-ui`; ¿público o privado? Si es público, ¿licencia MIT/Apache-2.0?).
2. **¿Monorepo interno primero?** (recomiendo sí: Fases 0–1 dentro de Purifex y repo propio en la Fase 2.)
3. **¿Cuál es el segundo proyecto?** Ayuda a elegir qué priorizar de la API (p. ej. si necesita tablas, formularios, gráficas…).
4. **Alcance multiplataforma:** ¿solo Windows por ahora (recomendado), o hay que dejar prevista otra plataforma?
