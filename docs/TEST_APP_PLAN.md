# Purifex · App de prueba «Fex Sonda» y comparación con Revo (plan, 2026-11-24)

> Estado: **solo plan**. Lo ejecuta Sonnet; Opus prueba y revisa. Decisiones marcadas (D) esperan
> confirmación del autor.

## Cómo usar este plan (para quien lo ejecuta)

- Una tarea = un commit, con `tools/ship.ps1` y `tools/check.ps1` en verde. Mira el CI de cada commit con `gh`.
- Código, tests y commits en inglés; este documento en español.
- Las tareas T.* son código. Las P.* son pruebas a mano del autor con Revo: no las ejecutes tú,
  deja el programa listo y escribe en «Estado» qué falta por mirar.
- Nada de lo que escribe la app de prueba puede llamarse como algo real: todo lleva `FexSonda`.

## Objetivo

Tener una app **instalable de verdad** que deja restos conocidos al desinstalarse, para comprobar
con Revo como árbitro:

1. **Revo ve los restos.** Al desinstalar Fex Sonda, el aviso de Revo «Desinstalar restos detectados»
   sale con números distintos de cero. Así se sabe que la app de prueba sirve.
2. **Purifex los quita.** Tras «Eliminar todo» en Purifex, Revo ya no encuentra nada de Fex Sonda.
3. **Purifex no se pasa.** Lo que no es de Fex Sonda (los controles) sigue en su sitio.
4. **Se puede deshacer.** «Devolver todo» lo pone de nuevo y Revo lo vuelve a ver.

## Punto de partida (2026-11-24)

- `tools/` solo tiene `check.ps1` y `ship.ps1`. El instalador de juguete que prometía
  `REVO_PARITY_PLAN.md` («Reglas que no cambian») no existe.
- Revo Uninstaller **Pro** está instalado (`RevoUninHelper.exe`, `RevoProcessDetector.sys`). Cuando
  otro programa desinstala una app, su monitor saca la ventana «Desinstalar restos detectados» con
  archivos y carpetas, elementos del registro y tamaño. Visto con VLC el 2026-11-24: 0 archivos y
  1 elemento del registro.
- Purifex ya tiene `apps traces <app>` (solo lectura), «Eliminar todo», la cuarentena y
  «Devolver todo». Sin administrador no puede quitar nada de HKLM (tarea A.7 de
  `UNINSTALL_UX_PLAN.md`).
- El caso VLC (`f93c62f`): una extensión que el desinstalador devuelve a otro programa no es de la
  app. Fex Sonda lo reproduce con un control, para que no vuelva a pasar.

## Decisiones

| # | Decisión | Motivo |
|---|---|---|
| T1 (D) | El instalador es nuestro: un binario Rust `fex-sonda` en el workspace, que hace de app, de instalador y de desinstalador. Recomiendo esto antes que Inno Setup. | No depende de nada externo, compila en CI, y el desinstalador olvida a propósito lo que queramos. |
| T2 (D) | Se prueba en el PC principal, no en una VM. | Revo Pro tiene la licencia en este PC. Es seguro porque todo se llama `FexSonda` y `fex-sonda purge` lo borra todo pase lo que pase. |
| T3 | Una sola lista de lo que se escribe (el manifiesto). `install`, `status` y `purge` la leen. | DRY: añadir un resto se hace en un solo sitio. |
| T4 (D) | Sin servicio de Windows, al menos en esta versión: archivos, registro, extensión, accesos directos y una tarea programada. | Un servicio es lo más delicado de crear y de quitar; se añade después si hace falta. |
| T5 | El instalador pide administrador (manifiesto `requireAdministrator`), y se prueba con Purifex como administrador. | Escribe en HKLM, `Program Files` y `ProgramData`, como un instalador real. |

## Fuera de este plan

- Automatizar Revo: no tiene línea de órdenes conocida. Las pruebas con Revo son a mano (P.*).
- Comparar con Revo en apps reales.
- Cambios en las reglas de Purifex. Si una prueba P falla, se anota en «Estado» y se abre una
  tarea nueva en el plan que corresponda.

## Lo que deja Fex Sonda (el manifiesto)

`{F}` = `C:\Program Files\FexSonda`. «Lo quita el desinstalador» es lo que hace el desinstalador
oficial. El resto se queda **a propósito**.

| # | Dónde | Tipo | Lo quita el desinstalador | Purifex debe proponerlo |
|---|---|---|---|---|
| 1 | `{F}\fexsonda.exe` | archivo | sí | — |
| 2 | `{F}\uninstall.exe`, `{F}\logs\install.log` | carpeta | no | sí (carpeta registrada) |
| 3 | `HKLM\…\Uninstall\FexSonda` | clave | sí | — |
| 4 | `%APPDATA%\FexSonda\settings.ini` | carpeta | no | sí |
| 5 | `%LOCALAPPDATA%\FexSonda\cache\a.bin` (64 KB) | carpeta | no | sí |
| 6 | `C:\ProgramData\FexSonda\shared.dat` | carpeta | no | sí (administrador) |
| 7 | `HKCU\Software\FexSonda` (valores `LastRun`, `Path={F}\fexsonda.exe`) | clave | no | sí |
| 8 | `HKLM\SOFTWARE\FexSonda` (valor `InstallDir={F}`) | clave | no | sí (administrador) |
| 9 | `HKLM\SOFTWARE\Classes\.fexsonda` → `FexSonda.Doc` | clave | no | sí |
| 10 | `HKLM\SOFTWARE\Classes\FexSonda.Doc\shell\open\command` = `"{F}\fexsonda.exe" "%1"` | clave | **no** (así la extensión 9 sigue abriendo la app) | sí |
| 11 | Menú Inicio común: `Fex Sonda.lnk` → `{F}\fexsonda.exe` | archivo | no | sí |
| 12 | Tarea `\FexSonda\Update`, que ejecuta `{F}\fexsonda.exe --update` | tarea | no | sí (administrador) |
| C1 | **Control:** `HKLM\SOFTWARE\Classes\.fexcomun`. Antes de instalar es de `FexOtra.Doc` (comando en `C:\ProgramData\FexOtra\otra.exe`). El instalador la toma y el desinstalador se la devuelve. | clave | se devuelve | **no**: es el caso VLC |
| C2 | **Control:** la app «Fex Sonda Vecina», registrada en `Uninstall\FexSondaVecina` con carpeta `C:\Program Files\FexSondaVecina` y `HKCU\Software\FexSondaVecina`. | app instalada | — | **no**: otra app instalada que se parece |

Revo debe ver como mínimo 2, 4–8, 11 y 12. Lo que vea de más se anota en «Estado»: no es
un fallo de Purifex hasta revisarlo.

---

## Bloque T · La app de prueba

### T.1 El binario y el manifiesto
1. Crea el crate binario `apps/fex-sonda`, añádelo a `members` del `Cargo.toml` raíz con
   `[lints] workspace = true`, y sin dependencias. Llama a `reg.exe`, `schtasks.exe` y a
   PowerShell (`WScript.Shell` para el `.lnk`) con `std::process::Command`, pasando los
   argumentos como lista, nunca dentro de un texto de shell.
2. En `crates/purifex-domain/tests/architecture.rs`:
   - añade `("fex-sonda", &[])` a `allowed`;
   - añade sus archivos a las excepciones de `println!`, con el motivo: «herramienta de
     prueba que habla por consola».
3. Crea `apps/fex-sonda/src/manifest.rs` con `pub enum Item { File{..}, Dir{..}, RegKey{..}, RegValue{..}, Lnk{..}, Task{..} }`
   y `pub const ITEMS: &[(&str /*id: "2", "C1"…*/, Item, Role)]`, donde
   `Role { RemovedByUninstaller, Leftover, Control }`. Es la tabla de arriba, una fila por elemento.
4. Crea `apps/fex-sonda/src/main.rs` con las órdenes `install`, `uninstall`, `status`, `purge`
   y `run`. Sin orden y llamado `uninstall.exe`, hace `uninstall`.

**Hecho cuando:** `cargo build -p fex-sonda` compila y `check.ps1` está en verde, guardas incluidas.

### T.2 `install` y `run`
1. Si no hay administrador, `install` termina con un mensaje claro y código 1. Comprueba si lo es
   con `reg.exe query HKU\S-1-5-19`, que solo funciona como administrador.
2. Crea primero los controles, si no existen:
   - C1: `FexOtra.Doc`, con su comando en `C:\ProgramData\FexOtra\otra.exe` (un archivo vacío
     basta), y `.fexcomun` apuntando a `FexOtra.Doc`;
   - C2: la app vecina, con su clave `Uninstall` (`DisplayName`, `InstallLocation`,
     `UninstallString` hacia un `.exe` vacío de su carpeta), su carpeta y su clave de HKCU.
3. Copia el ejecutable propio a `{F}\fexsonda.exe` y a `{F}\uninstall.exe`, y escribe
   `{F}\logs\install.log`.
4. Registra `Uninstall\FexSonda` con:
   - `DisplayName = "Fex Sonda"`, `DisplayVersion = "1.0.0"`, `Publisher = "Fexarium (prueba)"`;
   - `InstallLocation = {F}`, `DisplayIcon = {F}\fexsonda.exe`;
   - `UninstallString = "{F}\uninstall.exe"`, con comillas;
   - `EstimatedSize`.
5. Escribe los elementos 6–12, guarda el dueño anterior de `.fexcomun` en
   `HKLM\SOFTWARE\FexSonda\PrevFexcomun` y haz que `.fexcomun` apunte a `FexSonda.Doc`.
6. Al final, ejecuta `run` como el usuario: escribe los elementos 4, 5 y 7, como si la app se
   hubiera usado una vez.
7. Si `install` se ejecuta dos veces, el resultado es el mismo que con una.

**Hecho cuando:** después de `install`, `status` dice que todo está presente, y la app «Fex Sonda»
aparece en Configuración > Aplicaciones, en Revo y en la lista de Purifex.

### T.3 `uninstall`: lo que hace el desinstalador oficial
1. Borra `{F}\fexsonda.exe` y la clave `Uninstall\FexSonda`.
2. Devuelve `.fexcomun` a `FexOtra.Doc` (lee `PrevFexcomun`) y borra solo el valor `PrevFexcomun`.
3. No toca nada más. `uninstall.exe` sigue en la carpeta porque se está ejecutando, y eso es
   realista: muchos desinstaladores dejan su carpeta.
4. Sale con código 0. Purifex espera a que termine el proceso.

**Hecho cuando:** después de `uninstall`, `status` muestra los elementos 1 y 3 ausentes,
2 y 4–12 presentes, C1 apuntando a `FexOtra.Doc` y C2 intacta.

### T.4 `status` y `purge`
1. `status` recorre `ITEMS` y escribe una línea por elemento:
   `id  presente|ausente  rol  dónde`. Para C1 escribe también a quién apunta. Al final, los
   totales por rol. Con `--json`, lo mismo en JSON (`serde` no: escribe el JSON a mano, son
   pocos campos), para comparar antes y después.
2. `purge` borra **todo** lo del manifiesto, controles incluidos, y deja C1 como antes de la
   primera instalación (si `.fexcomun` no existía, se borra). Solo toca rutas que contienen
   `FexSonda` o `FexOtra`: compruébalo en el código antes de borrar cada una y, si no las
   contiene, aborta.
3. `purge` no toca la cuarentena de Purifex. Lo que quede ahí de Fex Sonda se vacía desde
   Purifex («Vaciar cuarentena») o caduca solo a los siete días.

**Hecho cuando:** `install`, `uninstall`, `purge` y luego `status` dejan todo ausente y C1 como
estaba. Repetir el ciclo tres veces da siempre lo mismo.

### T.5 La comparación de Purifex, sin Revo
1. Crea `tools/sonda.ps1`, con las órdenes `ciclo` y `comparar`:
   - `ciclo` ejecuta `install`, `uninstall` y luego `purifex apps traces "Fex Sonda"`, y guarda
     la salida en `target/sonda/traces.txt`;
   - `comparar` cruza esa salida con `status --json`: cada elemento `Leftover` debe aparecer
     propuesto y ningún `Control` debe aparecer. Escribe una tabla y sale con código 1 si algo falla.
2. Este script no borra nada.

**Hecho cuando:** `tools/sonda.ps1 ciclo; tools/sonda.ps1 comparar` termina en verde. Si
Purifex no propone algo, no lo arregles aquí: anótalo en «Estado» con la línea de `traces.txt`
y abre una tarea en `UNINSTALL_UX_PLAN.md` o en `REVO_PARITY_PLAN.md`.

**Pausa de revisión del bloque T.**

---

## Bloque P · Pruebas con Revo (a mano, el autor)

Antes de cada prueba: `fex-sonda purge`, y Purifex abierto **como administrador**.

### P.1 Revo ve los restos (la app de prueba sirve)
1. `fex-sonda install`, como administrador.
2. Desinstala «Fex Sonda» **desde Purifex** (asistente, «Desinstalar y limpiar»).
3. Sale el aviso de Revo «Desinstalar restos detectados». Anota sus tres cifras y haz una captura.
   **No pulses «Limpiar»**: cierra el aviso.
4. **Esperado:** archivos y carpetas ≥ 4 y elementos del registro ≥ 3. Con cero, la app de prueba
   no sirve y se revisa T.3 antes de seguir.

### P.2 Purifex los quita y Revo ya no los ve
1. Sigue desde P.1: la lista de restos de Purifex está abierta. Comprueba que C1 y C2 no aparecen.
2. Pulsa «Eliminar todo».
3. `fex-sonda status`: los `Leftover` ausentes, C1 apuntando a `FexOtra.Doc` y C2 intacta.
4. En Revo, «Desinstalación forzada» con el nombre «Fex Sonda» y la carpeta `{F}`.
   **Por verificar:** si Revo Pro acepta una app que ya no está en la lista. Si no la acepta,
   reinstala con `fex-sonda install`, desinstala desde Revo con el escaneo «Avanzado» y comprueba
   que solo ve lo nuevo, comparándolo con `status`.
5. **Esperado:** Revo no encuentra restos de Fex Sonda. Anota lo que encuentre, con una captura.

### P.3 Se puede deshacer
1. En Purifex, «Devolver todo» de Fex Sonda.
2. `fex-sonda status`: los `Leftover` vuelven a estar presentes, iguales que antes.
3. Revo («Desinstalación forzada», como en P.2) vuelve a verlos.
4. `fex-sonda purge`.

**Hecho cuando:** P.1–P.3 están anotadas en «Estado» con las cifras de Revo y las capturas
(las capturas no se suben al repositorio).

---

## Orden y dependencias

```
T.1 → T.2 → T.3 → T.4 → T.5 → (pausa) → P.1 → P.2 → P.3
```

Con `UNINSTALL_UX_PLAN.md`: P.2 necesita **A.7** (si no, «Eliminar todo» sin administrador falla sin
decir por qué) o Purifex abierto como administrador. Lo más corto es T.1–T.4 y luego P.1, que ya
prueba que Revo ve los restos.

## Riesgos

| Riesgo | Mitigación |
|---|---|
| La app de prueba toca algo real | Todo lleva `FexSonda` o `FexOtra`, y `purge` aborta si una ruta no lo contiene. |
| Un ciclo a medias deja basura | `purge` borra todo pase lo que pase y se puede repetir. |
| Revo marca de más (por ejemplo, `.fexcomun`) | Se anota como hallazgo. Purifex no copia los errores de Revo. |
| Revo Pro no deja buscar una app ya desinstalada | La alternativa de P.2, paso 4. |
| La tarea programada o HKLM sin administrador | `install` lo exige (T5), y P.* se hace con Purifex como administrador. |

## Estado

- 2026-11-24: plan escrito (Opus 5.5). Falta que el autor confirme T1, T2 y T4.
