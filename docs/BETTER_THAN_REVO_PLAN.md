# Purifex · Desinstalar mejor que Revo (plan, 2026-11-24)

> Estado: **solo plan**. Lo ejecuta Sonnet, tarea a tarea; Opus prueba y revisa. Decisiones (D)
> esperan al autor. Este plan **manda sobre el orden** de `UNINSTALL_UX_PLAN.md`: las tareas de
> aquel que se usan (A.5b, A.7, D.1–D.5, B.1, E.1) se ejecutan cuando este plan lo dice, con los
> pasos escritos allí.

## Cómo usar este plan (para quien lo ejecuta)

- Una tarea = un commit, con `tools/ship.ps1` y `tools/check.ps1` en verde. Mira el CI con `gh`.
- Código, tests y commits en inglés; este documento en español.
- **Cada tarea se nota al usarla.** Si una tarea no cambia nada visible ni medible en la ventana
  real, para y pregunta.
- Tras cada tarea que toca la ventana: `tools/respond.ps1` (M.1) durante el recorrido, y la cifra
  va al commit.
- Cuando una tarea dice «haz X de `UNINSTALL_UX_PLAN.md`», sigue esos pasos al pie de la letra.

## Objetivo

Que desinstalar con Purifex sea **más corto, más claro y más completo** que con Revo:

| | Revo Pro (de memoria; se mide en M.0) | Purifex al acabar este plan |
|---|---|---|
| Preguntas de la app (sin contar las del desinstalador de la app) | 3: desinstalar, borrar registro, borrar archivos. Además hay que elegir modo | **1**, más el permiso de Windows una sola vez |
| Clics fuera del desinstalador | ~12 | **≤ 4**: Desinstalar, Desinstalar y limpiar, Quitar N restos, Cerrar |
| Ventana congelada | No | **Nunca más de 100 ms** sin responder, medido |
| Cada espera tiene indicador | Sí | Sí, y dice qué está haciendo |
| Restos | Lista en dos pantallas | Una lista; se puede **devolver todo** con un clic |
| Lo que queda tras limpiar | — | **Revo no encuentra nada** (Fex Sonda, `TEST_APP_PLAN.md` P.2) |

### El recorrido

1. **«Desinstalar X»**: una sola pregunta, que explica en cuatro líneas lo que va a pasar, con el
   botón **«Desinstalar y limpiar»**. Sin administrador, el botón lleva el escudo y la línea
   «Windows te pedirá permiso una vez». Purifex se reabre como administrador y sigue solo, **sin
   volver a preguntar**.
2. **«Preparando»**: copia del registro y punto de restauración, cada una con su indicador y una ✓
   al terminar. La ventana responde.
3. **«Desinstalando X»**: el desinstalador de la app está abierto. Purifex muestra un indicador
   girando y «Esperando al desinstalador…». Si el desinstalador se cierra y la app sigue
   instalada, lo dice y ofrece «Volver a intentar» o «Cancelar».
4. **«Buscando restos»**: los cuatro lugares, cada uno con su indicador o su ✓.
5. **«N restos de X»**: la lista, con lo seguro marcado. **Un botón: «Quitar N restos (tamaño)»**,
   sin otra pregunta: todo va a la cuarentena y se puede devolver.
6. **«Quitando»**: barra con «12 de 40».
7. **«Listo»**: qué se quitó, qué no y por qué, y el botón «Devolver todo». Si no se pudo quitar
   nada, es un error y no un «Listo».

## Punto de partida (medido el 2026-11-24 en el PC del autor)

- **«La app se muere».** Al confirmar «Desinstalar», el hilo de la ventana hace, uno tras otro:
  leer la lista de apps, `SeedProbe::observe` (carpeta y todas las extensiones de `Classes`),
  `remember`, **`SRSetRestorePointW`** (puede tardar desde segundos hasta un minuto), la copia del
  registro y lanzar el desinstalador (`Cleaning::uninstall`, `cleaning.rs:212`). Mientras tanto,
  nada se pinta.
- **Mientras espera al desinstalador, la ventana tartamudea.** `check_watch`
  (`core/apps.rs:344`) llama a `services.apps()` **cada 1,5 s** en el hilo de la ventana. Leer la
  lista tarda **~500 ms** (`purifex apps`: 540, 492, 505 y 535 ms), así que la ventana no responde
  un tercio del tiempo.
- **«Te pide más de una vez»**:
  1. Confirmar «Desinstalar».
  2. Las preguntas del desinstalador de la app.
  3. «¿Eliminar todo rastro?».
  4. Si falta administrador, «¿Reiniciar como administrador?» más el permiso de Windows, y
     Purifex vuelve a buscar.

  Además, el punto de restauración y la copia del registro necesitan administrador, así que sin
  él la red de seguridad falla en silencio.
- **Sin indicadores.** `purifex-ui` no tiene spinner ni barra. D.1–D.5 de `UNINSTALL_UX_PLAN.md`
  están escritas y sin empezar.
- **Al quitar**: la ventana se cierra y no se ve nada hasta el final, que se anuncia como éxito
  aunque todo falle (A.7).
- Hecho y que sirve: la búsqueda en un hilo (A.1–A.5), quitar por tramos (A.6), Fex Sonda
  (`TEST_APP_PLAN.md` T.1–T.5).

## Decisiones

| # | Decisión | Motivo |
|---|---|---|
| M1 (D) | **El recorrido de desinstalar siempre corre como administrador.** Si Purifex no lo es, el permiso se pide **al empezar** y no a mitad. Recomiendo aceptarla | Una sola petición de permiso. Así el punto de restauración, la copia del registro y los restos de HKLM funcionan, y el desinstalador hereda el permiso sin volver a preguntar |
| M2 (D) | **Sin segunda confirmación para quitar.** El botón de la lista quita lo marcado. Recomiendo aceptarla | La lista ya enseña cada resto con su evidencia, todo va a la cuarentena y «Devolver todo» lo deshace |
| M3 (D) | La lista tiene **un solo botón** de quitar, «Quitar N restos», que cuenta lo marcado. Desaparecen «Eliminar marcados» y «Eliminar todo rastro». Recomiendo aceptarla | Dos botones parecidos obligan a pensar. Desmarcar es la forma de elegir |
| M4 | Preparar (sondear la app y el punto de restauración) se hace en un hilo, con el patrón de `TraceSearch`. Solo pasan a `Send + Sync` los puertos `SeedProbe` y `RestorePoints` | Son los dos que tardan. Lo mismo que U1: no se hacen `Send` todos los puertos |
| M5 | Para saber si el desinstalador terminó, se comprueba **solo la clave de la app** (`AppInventory::still_registered`), no la lista entera | Abrir una clave cuesta menos de 1 ms; leer la lista, ~500 ms |
| M6 | Las animaciones de adorno (C, E.2, E.3 y F de `UNINSTALL_UX_PLAN.md`) quedan **para después** de este plan | Primero que funcione, responda y diga la verdad |

## Fuera de este plan

- La desinstalación silenciosa con `QuietUninstallString` (menos clics, pero cambia lo que hace
  el desinstalador de la app). Plan aparte si el autor la quiere.
- Desinstalar varias apps a la vez (R7), el Cazador y el Monitor.
- Las reglas nuevas para restos que Revo ve y Purifex no: `TEST_APP_PLAN.md`, bloque G.

---

## Bloque 0 · La medida

### M.0 Revo, cronometrado (a mano, el autor; no bloquea)
Con Fex Sonda instalada (`fex-sonda install`), desinstálala **desde Revo** en modo Moderado y
anota, en «Estado»:
- clics, sin contar los del desinstalador;
- preguntas de Revo;
- segundos desde «Desinstalar» hasta «Finalizar»;
- cuántos restos encontró.

Después, `fex-sonda purge`. Con esto, la tabla del Objetivo deja de ser de memoria.

### M.1 `tools/respond.ps1`
1. Script de PowerShell que se engancha a un `purifex.exe` **ya abierto**
   (`-Name purifex`, el primero con ventana), o lo lanza con `-Launch "<argumentos>"`.
2. Cada 15 ms le pregunta a la ventana con
   `SendMessageTimeout(hwnd, WM_NULL, 0, 0, SMTO_ABORTIFHUNG, 10000)` (vía `Add-Type`), durante
   `-Seconds` (120 por defecto) o hasta que se cierre.
3. Al final escribe:
   - cuántas preguntas hizo;
   - cuántas esperas pasaron de 100 ms, con su momento (ms desde el inicio);
   - las 5 peores;
   - la CPU del proceso.

   Con `-Csv <ruta>`, todas las esperas.
4. No escribe nada en el repositorio.

**Hecho cuando:** con Purifex en reposo en Apps durante 10 s no hay ninguna espera de más de
100 ms. Con `-Launch "ui --find-traces Inkscape"` se ve la espera conocida (~435 ms, A.5b).

---

## Bloque 1 · Que no se muera

### M.2 Esperar al desinstalador sin leer la lista
1. En `ports.rs`, añade a `AppInventory`:
   `fn still_registered(&self, id: &str) -> Result<bool, String>`. Responde si la clave de
   desinstalación de esa app sigue existiendo, abriendo **solo** esa clave. Mira cómo
   `apps_registry.rs` forma el `id` (`HKLM64:<subclave>` y similares) y deshazlo ahí mismo, con
   una función pura con tests.
   - Las apps de la Tienda (`store_apps.rs`) responden con su propia consulta del paquete. Si no
     hay una barata, que la función diga `Err` y `check_watch` use la lista como hoy, **solo**
     para ellas.
2. `check_watch` usa `still_registered`. La lista completa se lee **una vez**, cuando la app ya no
   está (para `model.apps`), y nunca mientras se espera.
3. `WATCH_EVERY` baja a 500 ms. Ahora es barato y se nota antes que terminó.
4. Test con el falso: mientras se espera, `apps()` no se llama ni una vez, y `still_registered` sí.

**Hecho cuando:** `respond.ps1` durante un minuto de «Desinstalando» (Fex Sonda) no ve ninguna
espera de más de 100 ms. Antes había una de ~500 ms cada 1,5 s; anota las dos cifras en el commit.

### M.3 Los indicadores
Haz **D.1** y **D.2** de `UNINSTALL_UX_PLAN.md` (el reloj y `loaders.rs`), en dos commits.

**Hecho cuando:** lo que dicen D.1 y D.2.

### M.4 «Desinstalando» con su indicador
Haz **D.3** de `UNINSTALL_UX_PLAN.md`.

**Hecho cuando:** lo que dice D.3. Además, `respond.ps1` mide la CPU con el indicador girando
durante un minuto. Si pasa del 8 % de un núcleo, haz la puerta de D.6 antes de seguir.

### M.5 Preparar fuera de la ventana, y el paso «Preparando»
1. Parte `Cleaning::uninstall` (`cleaning.rs:212`) en tres:
   - `prepare_uninstall(&self, apps, query) -> Result<UninstallJob, String>`: busca la app y
     comprueba `protection()` y el lanzador. Es rápido.
   - `UninstallJob::run(self, step: &dyn Fn(PrepStep)) -> Prepared`, en un hilo: `observe` y el
     punto de restauración, avisando de cada paso.
   - `finish_uninstall(&mut self, prepared) -> Result<Started, String>`, en el hilo de la
     ventana: `remember`, la copia del registro y `launch`, en este orden. El punto de
     restauración sigue yendo **antes** que el desinstalador.

   Mide cuánto tarda la copia del registro. Si pasa de 50 ms, muévela también a `run`.
2. `SeedProbe` y `RestorePoints` pasan a `Send + Sync` y se guardan en `Arc`. Si sus falsos usan
   `Rc<RefCell>`, cámbialos a `Arc<Mutex>` **solo en esos dos**.
3. En el dominio, `enum PrepStep { Registry, RestorePoint }`, con su etiqueta en español.
4. Un `UninstallPrep` con `start` y `poll`, como `trace_search.rs` (hilo y `mpsc`, sin esperar
   nunca). `Services` ofrece `start_uninstall(id)` y `poll_uninstall() -> UninstallPolled`.
5. `Core::uninstalled` llama a `start_uninstall`:
   - pone `model.preparing = Some(Preparing { app, done: Vec<PrepStep> })` y abre el asistente;
   - `tick` hace el `poll`;
   - al terminar, `finish_uninstall` y sigue como hoy (`waiting`, `watching`).
6. En `wizard_dialog.rs`, el paso «Preparando» (paso 0) usa la lista de pasos de D.4:
   `Busy::Search`, generalizado a una lista de `(etiqueta, hecho)`, para que la lista de D.4
   y esta sean **una sola** forma de dibujar (DRY). Si D.4 aún no está hecha, haz aquí la
   lista genérica y D.4 la usará.
7. Tests:
   - con un `RestorePoints` falso que tarda 3 s, ningún `tick` pasa de 50 ms, `preparing` avanza y
     al final se lanza el desinstalador **después** del punto de restauración;
   - si el punto de restauración falla, se sigue igual que hoy, con el motivo en
     `safeguards_text`.

**Hecho cuando:** con Fex Sonda, `respond.ps1` desde el clic en «Desinstalar y limpiar» hasta que
se abre su desinstalador no ve ninguna espera de más de 100 ms. La captura `preparing`
(`ui_states.rs`) muestra una ✓ y un indicador.

### M.6 Juzgar fuera de la ventana
Haz **A.5b** de `UNINSTALL_UX_PLAN.md`.

**Hecho cuando:** lo que dice A.5b.

**Pausa de revisión: el autor desinstala Fex Sonda y dice si ya no «se muere».**

---

## Bloque 2 · Una sola pregunta

### M.7 El permiso, al empezar (requiere M1)
1. En el dominio, cambia el puerto `Elevation::restart_elevated(&self, resume: Option<&str>)` por
   `restart_elevated(&self, resume: Resume)`, con
   `enum Resume { Nothing, FindTraces(String), Uninstall(String /* id */) }`. Actualiza el
   adaptador (`elevation.rs`): `Uninstall(id)` relanza con `ui --uninstall <id>`.
2. En el binario (`apps/purifex/src`), `--uninstall <id>` en `UiOptions`, y
   `resume_uninstall(core, id)` como `resume_traces`, que aplica `Go(Apps)`, espera a que la lista
   esté leída, `AskUninstall(id)` y `Confirm`. La persona ya confirmó antes del permiso: **no se
   le vuelve a preguntar**.
3. En `Core::confirmed`, con `Confirm::Uninstall(id)` y `model.elevated == Some(false)`, se llama
   a `restart_elevated(Resume::Uninstall(id))` en lugar de desinstalar. Si la persona dice que no
   en Windows:
   - `model.error` = «Sin permiso de administrador no se puede hacer la copia de seguridad ni
     quitar los restos del sistema.»;
   - el diálogo sigue ahí, con el botón «Desinstalar sin limpiar a fondo», que desinstala como
     hoy, sin administrador.
4. La pregunta de desinstalar (`dialog.rs`, la de `Confirm::Uninstall`):
   - título: «Desinstalar {app}»;
   - cuerpo, cuatro líneas: «1. Copia de seguridad del registro y punto de restauración. 2. Se
     abre el desinstalador de {app}. 3. Purifex busca lo que dejó. 4. Lo quita a una cuarentena
     de la que se puede devolver.»;
   - botón principal: «Desinstalar y limpiar», con el icono `shield-check` si falta
     administrador, y debajo, en `TEXT_3`, «Windows te pedirá permiso una vez.».
5. Tests:
   - sin administrador, confirmar pide el reinicio con `Resume::Uninstall` y no lanza nada;
   - `resume_uninstall` lanza el desinstalador sin que haga falta otro `Confirm` de la persona;
   - si Windows dice que no, aparece el botón de desinstalar sin limpiar.

**Hecho cuando:** sin administrador, desinstalar Fex Sonda pide **un** permiso de Windows y ninguna
otra pregunta de Purifex hasta la lista de restos. La comprobación la hace el autor.

### M.8 Quitar sin segunda pregunta, con un solo botón (requiere M2 y M3)
1. `Action::AskClean` y `Action::AskCleanAll` desaparecen de la lista de restos y su lugar lo
   ocupa `Action::CleanChecked(name)`, que llama a `cleaned(&name, false)` directamente.
   `Confirm::Clean` y `Confirm::CleanAll` se quedan solo si algo más los usa. Si no, se borran.
   Busca todos los usos con `grep`.
2. En `traces_dialog.rs`:
   - los botones son «Cerrar» y uno principal, «Quitar {n} restos ({tamaño})», con `n` los
     marcados y el tamaño su suma;
   - con `n = 0`, el botón está apagado y dice «Marca algo para quitar»;
   - los restos que requieren administrador cuentan si Purifex lo es; si no, se ven apagados con
     «requiere administrador» (con M.7 ya no debería pasar en el recorrido).
3. El texto de antes (`CLEAN_BODY`: la cuarentena, que se puede devolver) pasa a una línea fija
   bajo la lista, en `TEXT_3`.
4. Actualiza los tests que usan `AskCleanAll` y `Confirm`. Lo que comprobaban se mantiene: lo que
   se quita, los `stuck` y las conjeturas sin marcar.

**Hecho cuando:** de la lista a «Quitando» hay **un** clic, y `check.ps1` está en verde.

### M.9 Decir la verdad cuando no se quita nada
Haz **A.7** de `UNINSTALL_UX_PLAN.md`. Su paso 1 (pedir permiso si todo requiere administrador)
ya no hace falta con M.7: hazlo solo para la búsqueda de restos que se empieza a mano, sin
desinstalar.

**Hecho cuando:** lo que dice A.7.

---

## Bloque 3 · Que siempre se vea qué pasa

### M.10 «Buscando», con sus cuatro lugares
Haz **D.4** de `UNINSTALL_UX_PLAN.md`, con la lista genérica de M.5.6.

### M.11 «Quitando», con su barra
Haz **D.5** de `UNINSTALL_UX_PLAN.md`. Además, la ventana de restos **no** se cierra al quitar: el
asistente pasa al paso 2 con la barra (hoy, `begin_removal` pone `traces_open = false` y el
asistente no lo enseña).

**Hecho cuando:** lo de D.5, y con Fex Sonda la barra se ve avanzar en la ventana real.

### M.12 «Listo», con cifras
Haz **E.1** de `UNINSTALL_UX_PLAN.md` (sin E.2 ni E.3). El botón «Devolver todo» está en este
paso, a la vista.

### M.13 El desinstalador se cerró, pero la app sigue
1. Cambia `Uninstaller::launch` para que devuelva un `Box<dyn Running>`, con el puerto
   `trait Running { fn finished(&self) -> bool; }`. En Windows guarda el `Child` del `cmd /C` y
   usa `try_wait`.
2. En `check_watch`: si `finished()` y la app sigue registrada **10 s después**, el asistente
   pasa a «{app} sigue instalada», con el texto «El desinstalador se cerró sin quitarla. ¿La
   cancelaste?» y los botones «Volver a intentar» (lanza otra vez) y «Cancelar» (cierra).
   - Algunos desinstaladores se copian a `%TEMP%` y relanzan: el `cmd` termina enseguida
     mientras el desinstalador real sigue. Por eso, mientras la clave exista y no hayan pasado
     esos 10 s, se sigue esperando. Si durante el tiempo de espera la clave desaparece, se sigue
     normal.
3. Tests con un falso de `Running`: terminó y la app sigue → el paso nuevo; terminó y la app no
   está → «Buscando».

**Hecho cuando:** con Fex Sonda, cancelar su desinstalador lleva a «sigue instalada» en ≤ 11 s,
y no se queda en «Desinstalando» para siempre.

### M.14 Una ventana que no salta y un fondo que no compite
Haz **B.1** de `UNINSTALL_UX_PLAN.md` (900 de ancho en todos los pasos) y la decisión **U8**:
mientras el asistente está abierto, la página de Apps no enseña `m.message` ni el panel
«Restos de …». Son dos commits.

**Pausa de revisión: el autor hace el recorrido entero.**

---

## Bloque 4 · La prueba

### M.15 El recorrido, medido (Opus y el autor)
Con Purifex **sin** administrador y Fex Sonda instalada:
1. `tools/respond.ps1 -Name purifex -Seconds 300 -Csv …` durante todo el recorrido.
2. Anota, en «Estado»: clics, preguntas de Purifex, permisos de Windows, segundos de principio a
   fin, la espera más larga y la CPU.
3. Sigue con `TEST_APP_PLAN.md` P.1–P.3: ¿Revo ve algo después de Purifex?
4. Rellena la tabla del Objetivo con las cifras reales de los dos (M.0).

**Hecho cuando:** la tabla del Objetivo tiene cifras medidas y Purifex gana o empata en cada fila.
Una fila en la que pierda se convierte en una tarea nueva, aquí.

## Orden y dependencias

```
M.1 → M.2 → M.3 → M.4 → M.5 → M.6 → (pausa) → M.7 (M1) → M.8 (M2, M3) → M.9
    → M.10 → M.11 → M.12 → M.13 → M.14 → (pausa) → M.15
M.0 en cualquier momento antes de M.15.
```

Lo primero que se nota:
- tras **M.2**, la espera deja de tartamudear;
- tras **M.4**, hay un indicador girando;
- tras **M.5**, el clic ya no congela.

## Riesgos

| Riesgo | Mitigación |
|---|---|
| El id de una app no sirve para abrir su clave (Tienda, MSI raros) | `still_registered` responde `Err` y se usa la lista solo para esa app (M.2.1) |
| Relanzar como administrador pierde el estado | `--uninstall <id>` lleva lo único que hace falta, y el recorrido empieza de cero, ya con permiso |
| Quitar sin preguntar quita algo que no debía | Solo se quita lo marcado y visto, a la cuarentena, con «Devolver todo» en el paso final (M.12) |
| El indicador gasta CPU | La puerta de D.6 (M.4) |
| Desinstaladores que se relanzan desde `%TEMP%` | La espera de 10 s con la clave todavía presente (M.13.2) |

## Estado

- 2026-11-24: plan escrito (Opus 5.5), tras la prueba del autor: «sigue siendo horrenda la
  experiencia de usuario, te pide más de una vez desinstalar, no damos retroalimentación entre
  acciones, no hay loaders, la app se muere». Faltan las decisiones M1, M2 y M3.
