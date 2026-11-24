# Purifex · Experiencia del asistente de desinstalación (plan, 2026-11-24)

> Estado: **solo plan**. Las decisiones marcadas (D) esperan confirmación del autor antes de su
> bloque. Pensado para que lo ejecute Sonnet 5.5 tarea por tarea, sin volver a deducir nada.

## Cómo usar este plan (para quien lo ejecuta)

1. **Una tarea = un commit.** Haz las tareas en el orden de «Orden y dependencias». No juntes dos
   tareas en un commit ni empieces la siguiente con la anterior en rojo.
2. **Antes de escribir**, lee enteros los archivos que nombra la tarea. Si el código no es como el
   plan lo describe (una función no existe, tiene otro nombre u otra forma), **para**: escribe lo
   que encontraste en «Estado» al final de este documento y pregunta. No improvises otro diseño.
3. **Reglas de la casa** (skill `fexarium-rust`): capas (`domain` ← `app`/`platform`; `ui` solo ve
   `domain`), nada de lógica en `Core::apply` ni en el binario, funciones ≤ 50 líneas (clippy),
   archivos ≤ 400 líneas de producción (test `architecture.rs`), `unsafe` solo en `platform` con
   `// SAFETY:`, errores `Result<_, String>`, sin `println!`, sin `unwrap()` en producción. Los
   tests se llaman como frases en inglés (`a_search_runs_off_the_window_thread_and_reports_each_step`).
   Los comentarios y nombres del código, en inglés como el resto; los textos para la persona, en español.
4. **Para cerrar una tarea:**
   - `powershell -File tools/check.ps1` en verde. Incluye fmt, clippy `-D warnings`, `unwrap_used`,
     clippy para Linux y los tests con las guardas.
   - Si la tarea toca la interfaz, capturas: `cargo build -q --example ui_states -p purifex`, luego
     `target\debug\examples\ui_states.exe wizard <ruta.png> <waiting|finding|removing|review|done>`.
     **Abre y mira el PNG.** La captura dibuja el instante 60 s, así que lo que gira debe verse
     bien en cualquier instante.
   - Commit y push con `powershell -File tools/ship.ps1 -Message "<mensaje>"`. El mensaje va en
     inglés y en el estilo del log (`git log -5`): una frase que dice qué cambia para la persona.
     Termínalo con la línea `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
   - Comprueba el CI de ese commit: `gh run list --limit 1`, y `gh run watch <id>` si sigue en
     marcha. Si falla, arréglalo en un commit nuevo antes de seguir.
5. **Al final de cada bloque hay una pausa de revisión**: deja capturas del antes y el después en
   la carpeta temporal de la sesión, escribe el resultado con fecha en «Estado» y espera al autor.
6. Comprueba también la ventana real: `cargo run --release -p purifex -- ui`. Lo que no se pudo
   comprobar así se dice en «Estado»; no se da por hecho.

## Objetivo

Que quitar una app se sienta rápido, vivo y seguro:

- La ventana **nunca se congela**: buscar restos y quitarlos ocurre mientras la ventana sigue
  dibujando y respondiendo.
- Cada paso tiene un **indicador de actividad que dice la verdad**: qué lugar se está mirando y
  cuántos restos van quitados de cuántos.
- El asistente es **una sola ventana estable**: misma anchura en todos los pasos, la altura cambia
  con suavidad y el contenido de cada paso entra con un desvanecido.
- El final **se celebra**: un ✓ que se dibuja, la cifra protagonista que cuenta hacia arriba y lo
  que queda por hacer como tarjetas, no como una fila de botones iguales.

Resultado visible al acabar: las capturas de los cinco estados (`waiting`, `finding`, `removing`,
`review`, `done`) y la ventana real quitando una app de prueba, sin un solo fotograma congelado
de más de 100 ms.

## Punto de partida (medido el 2026-11-24 en el PC del autor)

| Hecho | Dato |
|---|---|
| Buscar restos (`purifex apps traces <app>`, release, proceso completo) | Inkscape **3217 ms**, DBeaver **1546 ms**, Revo Uninstaller **1251 ms**. Arrancar el proceso y leer la lista cuesta unos 40 ms, así que casi todo es la búsqueda |
| Dónde se busca | `Core::tick` → `read_apps_page` → `find_traces_of` (`crates/purifex-app/src/core/reading.rs:126`) → `services.clean_app(name, false, None)`, **en el hilo de la ventana**. Mientras tanto no se dibuja nada |
| Quitar | `Core::cleaned` (`core/apps.rs:78`) → `clean_app(name, true, Some(keys))` → `Cleaning::clean` (`cleaning.rs:293`), que **vuelve a buscar** (otros 1–3 s) y luego mueve cada resto en `remove_all` (`cleaning.rs:337`). También en el hilo de la ventana |
| Hilos | Ya existe un patrón: `crates/purifex-app/src/offering.rs` (hilo + `mpsc` + `poll` sin esperar), y `Core::tick` lo recoge con `poll_offers` |
| Puerto de búsqueda | `TraceFinder` (`crates/purifex-domain/src/ports.rs:79`), sin `Send`. El adaptador real `WindowsTraceFinder` es un `struct` vacío y el falso `FakeFinder(Vec<Candidate>)` también podría ser `Send + Sync` |
| Fases de la búsqueda real | `WindowsTraceFinder::search` (`crates/purifex-platform/src/app_cleanup/finder.rs:237`): carpeta registrada; archivos y accesos de cada perfil (con un presupuesto `FILES_BUDGET` de 1500 ms); valores, asociaciones y claves del registro; lugares con su nombre; claves con su nombre; lugares del sistema |
| Ritmo de la ventana | `FRAME_TIMER` de 16 ms solo mientras `Ui::animating()`; `TICK_TIMER` cada 50 ms mientras `Core::needs_tick()` (`crates/purifex-platform/src/window/state.rs`) |
| Movimiento | `crates/purifex-ui/src/anim.rs`: `ease_out`, `hover`, `value` (cuenta desde 0 la primera vez), `reveal`, `keep_going` y el ajuste de Windows que apaga las animaciones |
| Primitivas de dibujo | `Op::{Fill, Stroke, Line, Text, Icon, Image, Clip, Unclip}`. **No hay arcos**: los indicadores se hacen con `Line` y `Fill`, y no hace falta tocar `platform` |
| Capturas | `render_png` dibuja en el instante 0 y en el 60 s (`crates/purifex-platform/src/gfx/capture.rs:65`) |
| Diálogo | `crates/purifex-ui/src/dialog.rs::draw`: fondo oscuro al 62 %, ventana centrada, altura de `height_of`. El asistente mide 760 de ancho y la lista de restos 900 (`traces_dialog.rs:22`), y por eso salta de un paso a otro |
| Deuda vista | `Core::expect_reading` (`core.rs:172`) repite la condición de `still_polling` (`core/reading.rs:212`). Se arregla en A.4 |

Iconos disponibles (no hay otros: no inventes nombres): `activity arrow-right audio-lines
chart-no-axes-combined circle-alert circle-check circle-dot clock cpu crosshair download flame
folder-open gamepad-2 gauge hard-drive history info key-round keyboard layers layout-dashboard
list-checks memory-stick microchip monitor mouse power package play radar refresh-cw rotate-ccw
router scan-search search settings shield shield-check sliders-horizontal sparkles square
square-terminal thermometer trash-2 triangle-alert undo-2 usb video wifi zap`.

## Decisiones tomadas

| # | Decisión | Motivo |
|---|---|---|
| U1 | La búsqueda de restos de la ventana corre en un hilo aparte, con el patrón de `offering.rs`. Solo se mueve al hilo el `TraceFinder` (que pasa a `Send + Sync` y a `Arc`). El resto de `Services` sigue en el hilo de la ventana | Es lo único que tarda segundos. Hacer `Send` todos los puertos obligaría a cambiar `Rc<RefCell>` por `Arc<Mutex>` en unos 76 sitios de los falsos, por nada |
| U2 | Quitar se hace **por tramos en `tick`**: en cada tick se mueven restos hasta gastar 30 ms y se pinta el avance. Sin hilos | Mover a la cuarentena es rápido por resto. Por tramos se consigue una barra de progreso real sin hacer `Send` el `CleanupStore` |
| U3 (D) | La ventana quita **exactamente los restos que mostró** (los de `model.traces`, filtrados por las marcas), tras comprobar otra vez que la app ya no está instalada. **No vuelve a buscar.** La línea de órdenes (`purifex apps clean`) sigue igual: busca y quita | Ahorra 1–3 s de espera muda. La persona vio cada resto con su evidencia, y la comprobación de que la app no está instalada se mantiene. **Recomiendo aceptarla.** Si el autor la rechaza, la búsqueda de A.5 se hace con el mismo hilo de A.3 antes de empezar a quitar |
| U4 | Los indicadores se dibujan con `Line` y `Fill` (indicador de 12 rayas, barra, anillo que late, ✓ trazo a trazo). No se añade ningún `Op` | No hace falta tocar `platform` ni Direct2D |
| U5 | Todo lo que se mueve depende del tiempo (`now_ms`) y respeta el ajuste de Windows que apaga las animaciones: entonces se dibuja la imagen quieta del final o una fija (el indicador, con todas las rayas iguales) | Se prueba pidiendo la imagen en un instante. Es accesible |
| U6 | La cifra del final dice **«a la cuarentena»**, no «liberados». El espacio solo se libera al vaciar la cuarentena (sola a los 7 días o con «Vaciar») | La regla de honestidad de `docs/DESIGN.md`: no se promete lo que no pasó |
| U7 | El asistente mide **900** de ancho en todos los pasos (lo mismo que la lista de restos). La altura se anima | Deja de saltar |
| U8 | Mientras el asistente está abierto, la página de Apps **no repite** su estado (el aviso de `m.message` y el panel «Restos de …»). Al ocultarlo con «Ocultar» vuelven a verse | Hoy el fondo compite con el diálogo |

## Fuera de este plan

- Arcos o trazos curvos nuevos en `platform` (U4).
- Animar otros diálogos (Cazador, lote, arranque, limpieza). Las piezas de `loaders.rs` quedan
  listas para ellos, pero no se aplican aquí.
- Cambiar qué se busca o qué se propone (`app_traces`, modos de búsqueda). Solo cambia *cuándo* y
  *cómo se ve*.
- Sonidos, confeti o partículas.
- La línea de órdenes y el servidor MCP no cambian (U3).

## Conceptos nuevos

| Tipo | Crate y archivo | Es |
|---|---|---|
| `SearchStep` (`Folders`, `Files`, `Registry`, `System`) con `ALL`, `label()` e `icon()` | `purifex-domain/src/search_step.rs` | Las partes de la búsqueda, como las ve la persona: «Carpetas», «Archivos y accesos», «Registro», «Sistema» |
| `TraceFinder::search_reporting(&self, app, step: &dyn Fn(SearchStep))` | `purifex-domain/src/ports.rs` | La búsqueda que avisa al terminar cada parte. Por defecto llama a `search` y avisa de todas al final |
| `Removing { app, done, total, bytes_done, bytes_total }` | `purifex-domain/src/view.rs` | Lo que la ventana sabe de un quitado en marcha |
| `Removed { removed, failed, bytes }` dentro de `Wizard.done` | `purifex-domain/src/view.rs` | El resultado en cifras. Hoy es un `String` |
| `Model.search_steps: Vec<SearchStep>` | `purifex-domain/src/view.rs` | Las partes ya terminadas de la búsqueda en marcha |
| `TraceSearch` | `purifex-app/src/trace_search.rs` | El hilo de la búsqueda, como `Offering`: `start`, `poll` y los pasos que llegan |
| `loaders.rs` (`spinner`, `progress_bar`, `pulse_ring`, `check_mark`, `breathing_dots`) | `purifex-ui/src/loaders.rs` | Los indicadores, como funciones del tiempo |

---

## Bloque 0 · Línea base

### 0.1 Capturas de antes y prueba del coste del movimiento
1. Construye el ejemplo y guarda las capturas de `waiting`, `finding`, `review` y `done` en la
   carpeta temporal de la sesión, con el prefijo `before-`. No se suben al repositorio.
2. Mide la CPU de la ventana real en reposo en la página Apps: `cargo run --release -p purifex -- ui`,
   y en PowerShell `(Get-Process purifex).CPU` dos veces con 10 s entre medias. Anota el número.
3. Escribe ambos datos con fecha en «Estado».

**Hecho cuando:** «Estado» tiene la CPU en reposo y la lista de capturas de antes. No hay commit
de código. Este paso no se sube salvo el cambio en «Estado», que va junto con A.1.

---

## Bloque A · Que nada se congele

### A.1 `SearchStep` en el dominio
1. Crea `crates/purifex-domain/src/search_step.rs` con
   `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)] pub enum SearchStep { Folders, Files, Registry, System }`,
   `pub const ALL: [SearchStep; 4]` en ese orden, `pub fn label(self) -> &'static str` («Carpetas»,
   «Archivos y accesos», «Registro», «Sistema») y `pub fn icon(self) -> &'static str`
   (`folder-open`, `layers`, `key-round`, `settings`).
2. Decláralo en `crates/purifex-domain/src/lib.rs`.
3. Tests: `ALL` tiene los cuatro sin repetir; cada icono existe en `icons::ICONS`.

**Hecho cuando:** `cargo test -p purifex-domain` pasa con los dos tests nuevos.

### A.2 El puerto avisa de cada parte, y es `Send + Sync`
1. En `crates/purifex-domain/src/ports.rs`, cambia `pub trait TraceFinder` a
   `pub trait TraceFinder: Send + Sync` y añade:
   ```rust
   /// The search, saying when each part of it has finished (for a window that shows progress).
   /// By default, the whole search and then every part at once.
   fn search_reporting(&self, app: &InstalledApp, step: &dyn Fn(SearchStep)) -> Result<Found, String> {
       let found = self.search(app);
       SearchStep::ALL.into_iter().for_each(step);
       found
   }
   ```
2. En `WindowsTraceFinder` (`crates/purifex-platform/src/app_cleanup/finder.rs`), sobrescribe
   `search_reporting` y haz que `search` lo llame con `&|_| {}`, para que haya una sola versión de
   la búsqueda. Avisa en este orden:
   - `Folders`, después de `registered_folder(app)` y `places_named(..)`;
   - `Files`, después de `files_of_everyone(..)`;
   - `Registry`, después de `observed_seeds` (sus `values`), `find_associations`,
     `find_program_keys` y `keys_named`;
   - `System`, después de `system_places`.

   Calcula cada grupo en una variable, en ese orden. Al final, junta los candidatos **en el mismo
   orden que hoy**, para que el resultado sea idéntico.
3. Arregla lo que deje de compilar por el `Send + Sync`: `FakeFinder` y `SeedingFinder`
   (`crates/purifex-app/src/cleaning.rs:824`). Si alguno guarda `Rc`/`RefCell`, pásalo a
   `Arc`/`Mutex` solo en ese falso.
4. Test en `finder.rs` con el patrón de sus tests: una búsqueda sobre una app inventada avisa de
   los cuatro pasos, cada uno una vez y en el orden de `ALL`.
5. Prueba de igualdad (refactor sin cambio de comportamiento): antes del cambio, guarda la salida
   de `target\release\purifex.exe apps traces DBeaver` y `... apps traces Inkscape`; después,
   compárala. Debe ser idéntica.

**Hecho cuando:** `tools/check.ps1` está en verde y las dos salidas de `apps traces` coinciden byte a byte.

### A.3 La búsqueda en `Cleaning` se parte en tres
1. En `crates/purifex-app/src/cleaning.rs`, cambia `finder: Option<Box<dyn TraceFinder>>` por
   `Option<Arc<dyn TraceFinder>>` y adapta su constructor o `with_*`.
2. Crea `pub struct SearchJob { app: InstalledApp, others: Vec<InstalledApp>, installed: bool, finder: Arc<dyn TraceFinder> }`
   (es `Send`) y estas tres funciones:
   - `pub fn prepare_search(&self, apps, all, query) -> Result<SearchJob, String>`: lo que hoy
     hace `clean` antes de `finder.search` (resolver la app y los demás). Es rápido.
   - `impl SearchJob { pub fn run(&self, step: &dyn Fn(SearchStep)) -> Result<Found, String> }`:
     llama a `finder.search_reporting`.
   - `pub fn finish_search(&self, job: &SearchJob, found: Found) -> CleanReport`: `propose` más el
     `CleanReport` de vista previa (`preview: true`, `incomplete`, `other_users_unread`,
     `still_installed`).
3. Reescribe `Cleaning::clean` con esas tres: `prepare_search`, luego `run(&|_| {})`, luego
   `finish_search` y, si `confirmed`, lo de quitar que ya hay. Así hay una sola manera de buscar.
4. Los tests existentes de `cleaning.rs` deben pasar **sin tocarlos**.

**Hecho cuando:** `cargo test -p purifex-app` pasa sin cambiar ningún test existente.

### A.4 `TraceSearch`: el hilo, y `Services` lo ofrece
1. Crea `crates/purifex-app/src/trace_search.rs` copiando la forma de `offering.rs`.
   - `start(&mut self, job: SearchJob) -> Result<(), String>`: lanza un hilo llamado
     `"trace-search"`. Envía `Msg::Step(SearchStep)` por cada parte y `Msg::Done(job, Result<Found, String>)`
     al final; devuelve el `SearchJob` para que `finish_search` lo use.
   - Si ya había una búsqueda, la nueva **la sustituye**: se suelta el receptor viejo y el hilo
     viejo termina sin que nadie lo escuche.
   - `poll(&mut self) -> Polled`, con
     `enum Polled { Idle, Working(Vec<SearchStep>), Done(SearchJob, Result<Found, String>) }`.
     Usa `try_recv` en bucle hasta vaciar el canal; nunca espera. Si el canal se desconecta sin
     resultado: `Done` con `Err("la búsqueda terminó sin resultado")`.
2. En `Services` (`crates/purifex-app/src/services.rs`) añade dos métodos:
   - `fn start_traces(&mut self, query: &str) -> Result<(), String>`: lee `installed()` y llama a
     `cleaning.prepare_search` y a `trace_search.start`.
   - `fn poll_traces(&mut self) -> TracesPolled`, con
     `enum TracesPolled { Idle, Working(Vec<SearchStep>), Done(Result<CleanReport, String>) }`,
     que en `Done` llama a `cleaning.finish_search`.

   Impleméntalos en `App` y en cualquier otro tipo que implemente `Services` (búscalo con
   `rg "impl Services for"`).
3. Tests en `trace_search.rs`, con un `TraceFinder` lento hecho para el test (como `Slow` en
   `offering.rs`):
   - `poll` devuelve `Working` y luego `Done`;
   - una segunda `start` sustituye a la primera y solo llega el resultado de la segunda;
   - los pasos llegan en orden.

**Hecho cuando:** los tests nuevos pasan y `tools/check.ps1` está en verde.

### A.5 `Core` busca sin bloquear
1. Crea en `crates/purifex-app/src/core/apps.rs` una sola función
   `pub(super) fn begin_finding(&mut self, name: String)`, que pone `model.finding = Some(name)`,
   vacía `model.search_steps`, llama a `services.start_traces(&name)` (un error va a
   `model.error` y quita `finding`) y deja `self.pending = true`.

   Sustituye por ella los tres sitios que hoy ponen `finding = Some(..)`: `core/actions.rs:124`
   (`find_traces`), `core/apps.rs:389` (`check_watch`) y `core/reading.rs:70` (`read_apps_page`).
   Comprueba con `rg "finding = Some"` que no queda ninguno más.
2. Crea `fn poll_traces(&mut self) -> bool` en `core/reading.rs`:
   - `Working(steps)`: guarda los pasos en `model.search_steps` y devuelve si cambiaron.
   - `Done(Ok(report))`: hace **todo lo que hoy hace `find_traces_of` después de obtener el
     informe**. Mueve ese cuerpo a una función `traces_found(&mut self, name, report)` y llama a
     esta, sin duplicarlo. Después: `finding = None` y `search_steps` vacío.
   - `Done(Err(e))`: `model.error`, `finding = None`.
3. En `Core::tick` (`core.rs:222`), llama a `poll_traces()` junto a `poll_offers()` y
   `check_watch()`. Quita `m.finding.is_some()` de `missing()` y añade `self.model.finding.is_some()`
   a `still_polling()`. Quita la llamada a `find_traces_of` de `read_apps_page` y borra
   `find_traces_of` si queda sin uso.
4. Arregla la deuda: `expect_reading` usa `self.still_polling()` en lugar de repetir la condición.
5. Tests: los que hoy hacen `apply(FindTraces)` y luego `tick()` esperan el resultado al momento.
   - Añade en el módulo de tests de `core.rs` un ayudante
     `fn settle(c: &mut Core) { let end = Instant::now() + Duration::from_secs(5); while c.needs_tick() && Instant::now() < end { c.tick(); std::thread::yield_now(); } }`
     y úsalo donde haga falta. **No cambies lo que comprueban.**
   - Test nuevo: con un buscador lento, justo después de `FindTraces` el modelo tiene `finding`,
     `tick()` vuelve enseguida (menos de 50 ms con un buscador de 300 ms) y `search_steps` crece
     hasta tener los cuatro.
   - `removing_an_app_walks_through_one_window_from_the_uninstaller_to_the_result` (`core.rs:2521`)
     debe seguir pasando.
6. Prueba real: `cargo run --release -p purifex -- ui --find-traces Inkscape`, o el comando con el
   que la ventana busca al arrancar (mira `UiOptions::find_traces`). Comprueba que la ventana se
   puede mover y que el ratón responde mientras busca.

**Hecho cuando:** `tools/check.ps1` está en verde y, en la ventana real, mover la ventana y pasar
el ratón por los botones durante la búsqueda funciona. Escríbelo en «Estado».

### A.6 Quitar por tramos (requiere U3 confirmada)
1. En `cleaning.rs`, extrae el cuerpo del bucle de `remove_all` a
   `pub fn remove_one(&mut self, app: &str, trace: &Trace, log: &mut Vec<CleanupEntry>, today: Date, done: &mut Removals) -> Result<(), String>`
   y haz que `remove_all` sea un bucle sobre él (DRY). Haz `Removals` `pub` con sus campos.
2. Añade a `Services`:
   - `fn begin_removal(&mut self, query: &str) -> Result<RemovalStart, String>`: comprueba que la
     app no está instalada (el mismo error que `clean` cuando `installed`) y devuelve
     `RemovalStart { log: Vec<CleanupEntry>, today: Date }`;
   - `fn remove_trace(&mut self, app: &str, trace: &Trace, start: &mut RemovalStart, done: &mut Removals) -> Result<(), String>`.
3. En `Core`, un campo privado `removing: Option<RemovalQueue>`, con
   `RemovalQueue { app, queue: VecDeque<Trace>, start: RemovalStart, done: Removals, total, bytes_total }`.
   `cleaned()` (`core/apps.rs:78`) deja de llamar a `clean_app`:
   - toma los restos de `model.traces` cuyas claves están en `checked_traces(everything)`
     (reutiliza esa función);
   - llama a `begin_removal`;
   - llena la cola, pone `model.removing = Some(Removing { .. })` y `traces_open = false`, y deja
     `pending = true`.
4. En `tick`, `fn remove_some(&mut self) -> bool`: saca restos de la cola mientras
   `Instant::now() - inicio < SLICE` (`const SLICE: Duration = Duration::from_millis(30)`) y
   actualiza `model.removing`. Cuando la cola se vacía, hace lo que hoy hace la rama `Ok(report)`
   de `cleaned` (`stuck`, `removal_said`, `in_use`, `traces`/`apps` a `None`, `expect_reading`)
   con los `Removals` acumulados. Mueve esa rama a una función y llámala desde aquí: no la copies.
   `still_polling()` incluye `self.removing.is_some()`.
5. Tests:
   - con 5 restos falsos, tras cada `tick` `done` crece y nunca pasa de `total`;
   - al acabar, el registro tiene las 5 entradas y `wizard.done` está puesto;
   - un resto que falla va a `stuck` y los demás se quitan;
   - si la app vuelve a estar instalada, no se quita nada y hay error.

**Hecho cuando:** `tools/check.ps1` está en verde. Con la prueba de `junk_real.rs` como modelo,
una prueba en sandbox (`apps/purifex/tests/`) quita tres restos reales por tramos y los devuelve
byte a byte.

**Pausa de revisión del bloque A.**

---

## Bloque D · Un indicador para cada momento

### D.1 El reloj para lo que gira
1. En `crates/purifex-ui/src/anim.rs` añade `pub fn clock(&mut self) -> Option<u64>`: con
   movimiento, marca `active = true` y devuelve `Some(now_ms)`; sin movimiento, devuelve `None`.
   En `canvas.rs`, `pub fn clock(&mut self) -> Option<u64>`, que llama a ese.
2. Test en `anim.rs`: con movimiento, `clock` mantiene `active`; sin movimiento, devuelve `None` y
   no lo activa.

**Hecho cuando:** el test pasa.

### D.2 `loaders.rs`
Crea `crates/purifex-ui/src/loaders.rs`, declarado en `lib.rs`. Todas las funciones reciben
`c: &mut Canvas` y usan `c.clock()`. Con `None`, dibujan la imagen fija indicada.

1. `spinner(c, cx, cy, r, color)`: 12 rayas (`c.line`) del radio `0.55·r` al `r`, de grosor 2.
   La raya `i` tiene alfa `0.15 + 0.85·((i + 12 − cabeza) % 12) / 11`, con
   `cabeza = (ms / 80) % 12` (una vuelta cada 960 ms). Fija: todas con alfa 0.6.
   **Comprueba si `Canvas::line` admite grosor.** Si no, añade `line_w` en `canvas.rs`: `Op::Line`
   ya tiene `width`.
2. `progress_bar(c, rect, fraction, color)`: fondo `theme::LINE` con radio 3 y relleno con el
   ancho `c.value("progress", fraction, 300)`. Para que la primera vez no cuente desde 0, añade
   `Anims::follow(name, target, ms)`, que empieza en `target` la primera vez y de ahí en adelante
   hace lo mismo que `value`. Encima, un brillo de 60 de ancho y alfa 0.25 que recorre la parte
   llena cada 1200 ms, recortado con `clip`. Fija: sin brillo.
3. `pulse_ring(c, rect, radius, color)`: un `stroke` que crece de `rect` a `rect.inset(-6)` con
   alfa de 0.5 a 0, cada 1400 ms. Fijo: nada.
4. `check_mark(c, center, size, t, color)`: con `t` de 0 a 1, el anillo `stroke` aparece con alfa
   `t·2` (limitado a 1) y la marca, de los puntos (4,12), (9,17) y (20,6) de la rejilla de 24
   escalados a `size`, se dibuja en dos `line`. El primer tramo ocupa `t` de 0.35 a 0.6 y el
   segundo de 0.6 a 1, con `ease_out`.
5. `breathing_dots(c, x, y, style)`: texto `"."`, `".."` o `"..."` según `(ms / 400) % 3`. Fijo: `"..."`.
6. Tests con `frame_at` (el ayudante de `ui.rs:477`) o con un `Canvas` directo:
   - el spinner en dos instantes separados 80 ms tiene alfas distintos y pide más fotogramas;
   - sin movimiento no pide fotogramas (`!c.moving()`);
   - `check_mark` en `t = 0` no dibuja la marca y en `t = 1` dibuja dos líneas completas.

**Hecho cuando:** los tests pasan y `loaders.rs` tiene ≤ 200 líneas de producción.

### D.3 Paso «Desinstalando»: el indicador alrededor del icono
1. En `crates/purifex-ui/src/wizard_dialog.rs`, el `DialogView` necesita decir qué indicador
   lleva. Añade a `DialogView` (en `dialog.rs`) `pub busy: Busy`, con
   `enum Busy { None, Spinner, Search(Vec<SearchStep>), Progress { done, total, bytes_done, bytes_total } }`
   y `Busy::None` en todos los demás diálogos. Si `DialogView` se construye en más de tres sitios,
   dale primero un constructor o `Default` (regla 10) en un commit aparte.
2. `waiting_step` pone `Busy::Spinner`. En `dialog::draw`, con `Busy::Spinner`, se dibuja
   `loaders::spinner` centrado en el `icon_box` (`rect.x + 44`, `rect.y + 44`), con `r = 28` y
   color `theme::AMBER`.
3. Texto más corto. Título: «Desinstalando {app}». Cuerpo: «Termina el desinstalador en su
   ventana. Purifex sigue solo cuando {app} desaparezca de la lista.», seguido de la línea
   «Esperando al desinstalador» con `breathing_dots`, en `theme::TEXT_3`.
4. Actualiza el test `each_state_of_the_removal_is_its_own_step_…` si cambia algo de lo que
   comprueba. El texto no lo comprueba.

**Hecho cuando:** la captura `waiting` muestra el indicador alrededor del icono y un test de
`ui.rs` comprueba que en ese estado `ui.animating()` es `true`.

### D.4 Paso «Buscando»: la lista de lugares
1. `finding_step` pone `Busy::Search(m.search_steps.clone())`.
2. En `dialog::draw`, debajo del cuerpo, con `Busy::Search(done)` se dibujan cuatro filas de 36 de
   alto, una por `SearchStep::ALL`: icono del paso, etiqueta y, a la derecha, el estado.
   - Hecho: `circle-check` en `theme::ACCENT`, y la etiqueta en `theme::TEXT`.
   - El primero no hecho: `spinner` con `r = 8`, y la etiqueta en `theme::TEXT`.
   - Los demás: `circle-dot` en `theme::TEXT_3`, y la etiqueta en `theme::TEXT_3`.

   `height_of` suma `4 × 36 + 16` cuando hay `Busy::Search`.
3. Al pasar de «no hecho» a «hecho», el `circle-check` entra con una escala de 0.6 a 1 en 220 ms:
   usa `c.value` con una clave `&'static str` por paso (`const KEYS: [&str; 4]`).
4. En el ejemplo `apps/purifex/examples/ui_states.rs`, el estado `finding` pone
   `model.search_steps = vec![Folders, Files]`.

**Hecho cuando:** la captura `finding` muestra dos lugares hechos, uno buscando y uno pendiente.

### D.5 Paso «Quitando»: la barra
1. En `wizard_dialog::view`, antes de las otras ramas: si `m.removing` es `Some(r)`, el paso es el
   2 («Quitar restos»).
   - Título: «Quitando restos de {app}».
   - Cuerpo: «Se mueven a la cuarentena; desde ahí se pueden devolver.».
   - `Busy::Progress`, sin botones (no se puede parar a medias). `cancel` sigue siendo
     `Action::CloseWizard`: se oculta y el trabajo sigue.
2. En `dialog::draw`, con `Busy::Progress` se dibuja la barra (`rect.w − 56` de ancho y 6 de alto)
   y debajo, en mono, «{done} de {total} · {size_text(bytes_done)}». `size_text` está en
   `purifex_domain::apps`; mira qué unidad recibe.
3. Estado `removing` nuevo en `ui_states.rs`: 8 de 14 y 512 MB de 642 MB.
4. Test: con `m.removing` puesto, el paso es 2, no hay botones y la barra se dibuja.

**Hecho cuando:** la captura `removing` muestra la barra a medias con su texto.

### D.6 Medir el coste
1. En la ventana real, con el paso «Desinstalando» abierto durante un minuto (desinstala una app
   de prueba o cualquier app pequeña que el autor diga), mide la CPU como en 0.1.
2. **Puerta:** si pasa del 8 % de un núcleo, haz una tarea nueva antes de seguir. Añade a
   `UiHandler` `fn frame_every_ms(&self) -> u32` (16 por defecto) y haz que `Ui` devuelva 33
   cuando lo único que se mueve es un `clock()` (un indicador) y no un `value` ni un `reveal`.
   `state.rs::pace` usa ese valor en `SetTimer`. Vuelve a medir.

**Hecho cuando:** «Estado» tiene la cifra de CPU con el indicador girando (y la de después si
hizo falta la puerta).

**Pausa de revisión del bloque D.**

---

## Bloque B · Una sola ventana estable

### B.1 La misma anchura
1. En `wizard_dialog::window`, `width: 900.0` (U7).
2. Comprueba en las capturas que los cinco estados miden lo mismo.

**Hecho cuando:** un test recorre los estados del asistente (con `view` de `wizard_dialog` y
`traces_view` para `review`) y comprueba que todos tienen `width == 900.0`.

### B.2 La altura se desliza
1. En `dialog::draw`, si `v.steps.is_some()`, la altura es `c.follow("wizard_h", h, 240)`
   (de D.2). Si no, `h` como hoy.
2. La ventana sigue centrada. Al cambiar la altura, crece hacia arriba y hacia abajo a la vez, y
   así está bien.
3. Test: entre dos fotogramas a 0 ms y 120 ms tras pasar de `finding` a `review`, la altura está
   entre las dos, y a 400 ms es la final.

**Hecho cuando:** el test pasa.

### B.3 El contenido de cada paso entra
1. En `Ui` (`ui.rs`), guarda `wizard_step: Option<(usize, u64)>`: el paso y cuándo empezó.
   Cámbialo cuando el paso de `dialog.steps` sea distinto del guardado.
2. Añade `Canvas::reveal_since(start_ms, delay_ms)`, como `reveal` pero con su propio inicio.
   `dialog::draw` recibe ese inicio (un argumento más, o un campo en `DialogView` que `ui.rs`
   rellena) y, después de `draw_steps`, llama a `c.reveal_since(inicio, 0)` antes de
   `heading_and_body`, y a `c.rest()` antes de los botones. Los botones no se mueven, para que no
   se escapen del puntero.
3. La cabecera de pasos **no** entra: se queda quieta.
4. Test: justo tras cambiar de paso, el título se dibuja con alfa menor que 1, y a 300 ms con alfa 1.

**Hecho cuando:** el test pasa y en la ventana real se ve el paso nuevo entrar sin parpadeo.

### B.4 Un fondo que no compite
1. En `dialog::draw`, el fondo oscuro sube a `0.72 · t` cuando `v.steps.is_some()`. El resto de
   los diálogos se queda en 0.62.
2. En `crates/purifex-ui/src/apps_page.rs`, si `m.wizard.is_some()`, no se dibuja el aviso de
   `m.message` (línea 318) ni el panel «Restos de …» (línea 58 y siguientes). Mira cómo se
   decide si el panel se dibuja y añade la condición ahí, en un solo sitio.
3. Comprueba que con el asistente oculto («Ocultar» pone `wizard = None`) el panel y su «Dejar de
   esperar» vuelven a verse. Ya hay un test de `waiting` en `apps_page.rs:591`: amplíalo.

**Hecho cuando:** las capturas no muestran el aviso repetido detrás del asistente y los tests de
`apps_page` pasan.

**Pausa de revisión del bloque B.**

---

## Bloque C · Pasos que transmiten progreso

### C.1 La línea que une los pasos
1. En `wizard_dialog::draw_steps`, entre el punto `i` y el `i + 1` dibuja una `line` desde
   `dot.right() + 8` hasta `x_siguiente + 24 − 8`, a la altura del centro del punto, en
   `theme::LINE`.
2. Encima, la parte hecha en `theme::ACCENT`: su largo total es
   `c.follow("wizard_line", paso as f32, 400)` medido en «pasos». El tramo `i` se llena con
   `(llenado − i).clamp(0, 1)`.
3. Quita la línea horizontal de debajo de la cabecera si queda redundante. Mira la captura.

**Hecho cuando:** la captura de `review` muestra los tramos 1–2 y 2–3 llenos y el 3–4 vacío.

### C.2 El paso actual late; el hecho rebota
1. El paso actual (no el último ya hecho) lleva `loaders::pulse_ring` alrededor de su punto.
2. Cuando un paso pasa a hecho, su `circle-check` entra con una escala de 0.6 a 1.08 y luego a 1:
   `c.value` con clave por paso, 260 ms, y la curva `ease_back_out` que añades a `anim.rs` con su
   test (sube un poco por encima de 1 y vuelve).

**Hecho cuando:** tests de `draw_steps` en dos instantes: el anillo cambia y el ✓ nuevo pasa de 1.

**Pausa de revisión del bloque C.**

---

## Bloque E · Un final que se disfrute

### E.1 El resultado en cifras
1. En `purifex-domain/src/view.rs`:
   `pub struct Removed { pub removed: usize, pub failed: usize, pub bytes: u64 }`,
   y `Wizard.done: Option<Removed>`.
2. `removal_said` (`core/apps.rs:129`) rellena `Removed`. `bytes` es la suma de
   `entry.trace.bytes` de `removed`. `model.message` sigue igual que hoy.
3. `wizard_dialog::done_body` construye el texto a partir de las cifras: «{removed} restos a la
   cuarentena» y, si `failed > 0`, «, {failed} no se pudieron quitar». Ajusta los tests y el
   ejemplo `ui_states.rs` (estado `done`: 14, 2 y 642 MB).

**Hecho cuando:** `tools/check.ps1` está en verde y la captura `done` sale igual que antes salvo
el texto.

### E.2 El ✓ que se dibuja y la cifra que cuenta
1. Con `wizard.done` puesto, `DialogView` lleva `busy: Busy::Done { bytes, removed }` (añade la
   variante). En lugar del `icon_box`, `dialog::draw` dibuja `loaders::check_mark` de tamaño 56,
   centrado arriba, con `t` de `c.reveal_since(inicio_del_paso, 0)` alargado a 700 ms. Para eso,
   añade `duration` a `reveal_since`, o una función hermana.
2. Debajo, la cifra con `theme::sans(34, theme::TEXT).bold()`:
   `size_text(c.value("wizard_bytes", bytes, COUNT_MS))`, centrada, con la etiqueta «a la
   cuarentena · {removed} restos» en `theme::eyebrow`. Si `bytes == 0` (solo registro), la cifra
   es `{removed}` y la etiqueta «restos a la cuarentena».
3. El título pasa a «{app} se fue». El cuerpo, a una frase: «Todo se puede devolver durante 7 días.».
   **Comprueba en el código** que la cuarentena caduca a los 7 días: busca `expired` en `core.rs`
   y en `purging.rs`. Si no es así, pon el plazo real.

**Hecho cuando:** la captura `done` muestra el ✓ entero y la cifra final (instante 60 s), y un
test en el instante 100 ms muestra el ✓ a medias y la cifra por debajo de la final.

### E.3 Lo que queda, como tarjetas
1. `done_buttons` deja un solo botón principal, «Listo» (`Action::CloseWizard`, icono
   `circle-check`). «Devolver todo» pasa a `aside` (abajo a la izquierda, `history`).
2. «Cerrar programas» y «Borrar al reiniciar» pasan a `rows` de un tipo nuevo de fila: una
   tarjeta de 56 de alto con un icono (`triangle-alert` en `theme::AMBER`), el texto (la frase que
   hoy añade `done_body`) y su botón a la derecha. Puedes ampliar `Row` con
   `action_button: Option<Btn>`, o crear `Card` junto a `Row`. Elige lo que menos toque y explícalo
   en el commit.
3. Las tarjetas entran con `reveal_since` con 120 ms de retraso entre una y otra, después de la cifra.

**Hecho cuando:** la captura `done` muestra la cifra, dos tarjetas con su botón y abajo «Devolver
todo» (izquierda) y «Listo» (derecha). Los tests de `the_last_step_offers_what_is_left_…` siguen
comprobando las tres acciones.

**Pausa de revisión del bloque E.**

---

## Bloque F · Pequeños detalles

### F.1 La lista de restos entra en cascada
1. En `dialog::list`, las primeras 12 filas entran con `reveal_since(inicio_del_paso, 18 × i)`,
   como las filas de la tabla de controladores.

**Hecho cuando:** un test a 0 ms muestra la fila 0 más visible que la 5.

### F.2 Desmarcar atenúa con suavidad
1. En `dialog::row`, el alfa del texto de una fila es `c.value` (clave por fila: usa
   `anim::key_of` como `hover` y añade `Anims::value_keyed(u64, …)` si `value` solo acepta
   `&'static str`). Va hacia 1 marcada y 0.45 desmarcada, en 140 ms.

**Hecho cuando:** un test que desmarca una fila ve el alfa bajar en dos fotogramas, no de golpe.

### F.3 El botón se hunde al pulsarlo
1. **Investiga primero:** mira si `Ui` sabe que el botón izquierdo está abajo (`WM_LBUTTONDOWN` /
   `UiEvent`). Si no lo sabe, **no lo añadas en este plan**: escríbelo en «Estado» como pendiente.
2. Si lo sabe: en `widgets::button`, con el botón pulsado sobre él, dibuja el rectángulo con
   `inset(1.5)` y el relleno un 8 % más oscuro.

**Hecho cuando:** está hecho y probado, o anotado como pendiente con el motivo.

**Pausa de revisión del bloque F.**

---

## Orden y dependencias

```
0.1 → A.1 → A.2 → A.3 → A.4 → A.5 ──┐
                         (U3) A.6 ──┤
D.1 → D.2 → D.3, D.4 (necesita A.5), D.5 (necesita A.6) → D.6
B.1 → B.2 (necesita D.2: follow) → B.3 → B.4
C.1 (necesita D.2) → C.2
E.1 → E.2 (necesita B.3: reveal_since) → E.3
F.1 (necesita B.3) → F.2 → F.3
```

El camino más corto a algo que se nota: **A.1 → A.5 → D.1 → D.4**. La ventana deja de congelarse
al buscar y enseña qué está mirando. Orden completo: 0 → A → D → B → C → E → F.

## Riesgos

| Riesgo | Mitigación |
|---|---|
| Un `Send + Sync` en `TraceFinder` rompe un falso que usa `Rc` | Se cambia solo ese falso a `Arc<Mutex>` (A.2.3). No se toca ningún otro puerto (U1) |
| Una búsqueda vieja llega después de una nueva (cambio de modo, «Buscar otra vez») | `start` sustituye el receptor y el resultado viejo se pierde (test en A.4) |
| La persona cierra la ventana mientras el hilo busca | El hilo solo lee. Al soltar el receptor, su `send` falla en silencio (como `offering.rs`) |
| Tests que esperaban el resultado en el mismo `tick` | El ayudante `settle` (A.5.5), sin cambiar lo que comprueban |
| El indicador gira durante minutos mientras se espera al desinstalador y gasta CPU | Medido en D.6, con puerta a 30 fps |
| Un tramo de 30 ms se pasa con un resto enorme (copia entre discos) | Es un solo resto por encima del tramo; la barra avanza a saltos pero no se congela más que ese resto. Si una prueba real pasa de 500 ms por resto, se anota en «Estado» y se decide con el autor si ese caso va al hilo |
| `wizard_dialog.rs` o `dialog.rs` pasan de 400 líneas de producción | El dibujo de `Busy` va a un archivo propio (`wizard_busy.rs`) en cuanto se acerque. La guarda de `architecture.rs` avisa |
| Las capturas (instante 60 s) salen con algo a medias | Todo lo que tiene fin acaba antes de 60 s; lo que gira se ve bien en cualquier instante (U5) |

## Estado

- 2026-11-24: plan escrito. Medidas de partida en «Punto de partida». Falta que el autor confirme U3.
