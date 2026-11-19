# Purifex · Limpieza profunda de restos (plan, 2026-11-18)

Objetivo: que después de desinstalar una app, **Purifex encuentre los mismos restos que Revo Uninstaller** (y ofrezca «Eliminar todo rastro»), sin tocar nada que no sea de esa app.

Este plan amplía `docs/UNINSTALLER_PLAN.md`. Lo que ya existe (motor de restos, cuarentena, diálogos, vigilancia del desinstalador) no se rehace: se le añaden **nuevos tipos de resto** y **nuevas fuentes de evidencia**.

---

## 1. De dónde sale el plan (evidencia real)

Tras desinstalar **Kdenlive** en este PC, Purifex quitó 2 carpetas y Revo Uninstaller seguía mostrando **«Archivos y carpetas: 18 · Elementos del registro: 19 · 83 KB»**. Lo que seguía en el PC (buscado solo en lectura):

| Tipo | Ejemplos reales | Por qué Purifex no lo ve hoy |
|---|---|---|
| **Clave de asociación** | `HKLM\SOFTWARE\Classes\.kdenlive`, `…\Classes\Kdenlive` (con `DefaultIcon` y `shell\open\command` → `C:\Program Files\Kdenlive\bin\kdenlive.exe`) | Todo `…\Classes` se rechaza como «lugar del sistema» |
| **Entradas del Explorador (claves)** | `HKCU\…\Explorer\FileExts\.kdenlive` (+ `OpenWithList`, `OpenWithProgids`), `…\RecentDocs\.kdenlive`, `…\ComDlg32\OpenSavePidlMRU\kdenlive`, `…\Notifications\Settings\kdenlive`, `…\Start\TileProperties\W~{…}\Kdenlive`, `…\PushNotifications\Backup\kdenlive`, `CloudStore\…\appmetadata$kdenlive` | Solo se buscan claves en `Software\<nombre>`; estas están en rutas profundas |
| **Valores sueltos** | En `…\MuiCache`, `…\AppCompatFlags\Compatibility Assistant\Store`, `…\FeatureUsage\AppSwitched`, `…\Search\JumplistData`, `ApplicationAssociationToasts`, `IE\LowRegistry\Audio\PolicyConfig\PropertyStore`: **el *valor* (nombre o dato) apunta a `C:\Program Files\Kdenlive\…`** | Solo sé quitar claves enteras, no un valor dentro de una clave compartida |
| **Archivos con el nombre como prefijo** | `AppData\Local\kdenliverc`, `AppData\Local\State\kdenlivestaterc`, `AppData\Local\stalefiles\kdenlive` | Solo reconoce nombres *idénticos* a la app |
| **Volcados de error** | `AppData\Local\CrashDumps\kdenlive.exe.33876.dmp` (×2) | Nombre = `<exe>.exe.<pid>.dmp`, no el de la app |
| **Accesos recientes** | `AppData\Roaming\Microsoft\Windows\Recent\Vídeo.kdenlive.lnk` (×2) | Es un archivo del usuario con la extensión de la app, no con su nombre |

Dos conclusiones que mandan en el diseño:

1. **La evidencia no es solo el nombre.** Casi todo lo que falta se reconoce por **una referencia a la carpeta o al `.exe` de la app** (la ruta que la app registró), o por **la extensión de archivo que la app registró**.
2. **Hay que poder quitar un *valor* de una clave que no es de la app** (`MuiCache` es de Windows; solo ese valor es de la app).

---

## 2. Qué se busca y con qué evidencia

Cada resto lleva su **evidencia**, y sin evidencia no se propone. Nada se propone «por parecerse».

| Evidencia | Qué prueba | Ejemplo |
|---|---|---|
| `RegisteredInstall` | La carpeta que la app registró al instalarse | `C:\Program Files\obs-studio` |
| `NameMatch` (ya existe) | Nombre idéntico al de la app, en un sitio fijo | `AppData\Roaming\kdenlive` |
| `NamePrefix` (nueva) | Nombre = nombre de la app + sufijo conocido (`rc`, `staterc`, `.ini`, `.log`, `.json`, `.cfg`, `.db`, `.cache`) en una carpeta de datos del usuario | `kdenliverc`, `kdenlivestaterc` |
| `PathReference` (nueva) | Un valor o archivo **contiene la ruta de instalación o un `.exe` de la app**, con límite de ruta (no `…\Kdenlive2`) | valor de `MuiCache` |
| `Association` (nueva) | Clave de extensión/ProgId cuyo `shell\*\command` o `DefaultIcon` apunta **dentro de la carpeta de la app** | `Classes\.kdenlive` → `Classes\Kdenlive` |
| `AssociatedExtension` (nueva) | Clave/archivo que cuelga de una extensión que **la app registró** | `FileExts\.kdenlive`, `RecentDocs\.kdenlive`, `Recent\*.kdenlive.lnk` |
| `CrashDump` (nueva) | `<exe de la app>.exe[(n)].<pid>.dmp` en `CrashDumps` | los 2 volcados |

**Semillas (lo que hace falta saber de la app):** carpeta de instalación, nombres de `.exe`, extensiones asociadas.
- Carpeta: `InstallLocation`; si falta, la carpeta de `UninstallString` o `DisplayIcon`.
- `.exe`: los de la carpeta (mientras existe) y los vistos en referencias (`…\bin\kdenlive.exe`).
- Extensiones: las de las claves `Classes` que cumplen `Association`.
- **Todo esto se guarda en la instantánea** (`uninstalled.jsonl`) al lanzar el desinstalador, porque después la carpeta ya no existe. Hay que **ampliar `InstalledApp`** con `display_icon` y guardar las `.exe` y extensiones detectadas *antes* de desinstalar.

---

## 3. Cambios de diseño

### 3.1 Dominio (`purifex-domain`)
- `TraceKind::RegistryValue` y campo **`Trace.value: Option<String>`** (nombre del valor; `location` es la clave). Compatibilidad: `#[serde(default)]` para que el historial viejo siga leyéndose.
- `enum Evidence` y campo `Trace.evidence` (además del texto `why`): sirve para agrupar, probar y mostrar.
- `Seeds` (carpeta, exes, extensiones) y `seeds_of(app)`.
- Reglas nuevas en `app_traces::judge` (siguen siendo funciones puras):
  - **`PathReference`**: la ruta de la app debe aparecer **con límites de ruta** y la semilla debe tener **≥ 2 componentes bajo la unidad** y no ser un lugar amplio (`Program Files`, `Users`, etc.). Sin semilla válida → no hay referencias.
  - **Compartido**: si la ruta de otra app instalada contiene la semilla, o la semilla contiene la de otra app, se rechaza (igual que hoy).
  - **Valores**: solo en **claves de la lista fija** (§4). Fuera de ella, no se propone aunque contenga la ruta.
  - **Classes**: se permite `…\Classes\<X>` **solo con evidencia `Association` o `AssociatedExtension`**; el resto de `Classes` sigue prohibido.
- Las reglas duras de hoy **no se aflojan**: nunca `Windows`, `System32`, claves `Microsoft` *como clave*, `Program Files` entero, etc. Los valores sueltos de claves de Microsoft (MuiCache…) se permiten solo como *valor* con `PathReference`.
- `CleanReport` y su texto: **recuentos por tipo** («Archivos y carpetas: N · Registro: N · tamaño»).

### 3.2 Plataforma (`purifex-platform`)
- `registry.rs`: añadir **escritura acotada**: `delete_value`, `delete_key_tree`, `set_value_raw(type, bytes)`, `read_value_raw`. Es el único sitio con `unsafe` de registro. Solo se llama desde la cuarentena.
- `finder`: nuevas fuentes, cada una **barata y acotada** (§4). Parámetro `base` para poder probar contra una rama de pruebas de HKCU.
- `quarantine`: **valores** guardados como `valor.json` (tipo + bytes en hexadecimal + clave + nombre) y restaurados con `set_value_raw`; **claves** como hoy con `reg export`/`import`. Archivos como hoy (mover).
- **Permisos**: lo de HKLM necesita administrador. Primero detectar y **decir «requiere administrador»** por resto; después (fase 6) botón «Reiniciar como administrador».

### 3.3 Aplicación (`purifex-app`)
- `Cleaning::clean` ya filtra por `propose`; solo cambia el origen de los candidatos y el tipo de `Trace`.
- Instantánea ampliada: capturar semillas **al lanzar** el desinstalador (carpeta, exes, extensiones), no al terminar.
- Orden `purifex apps traces|clean` muestra los recuentos por tipo.

### 3.4 Interfaz
- El diálogo de restos se **agrupa**: «Archivos y carpetas (N)» y «Registro (N)», con tamaño total, como Revo. Las filas de registro muestran `clave` y, si es un valor, `valor`.
- Sigue el límite de filas listadas: lo que no cabe solo lo quita «Eliminar todo rastro» (ya implementado).
- El panel «Restos de X» muestra el resumen por tipo.

---

## 4. Dónde se mira (lista fija y acotada)

**Registro, claves/valores en HKCU** (y su equivalente en HKLM cuando exista):
`…\Explorer\FileExts\<ext>` · `…\Explorer\RecentDocs\<ext>` · `…\Explorer\ComDlg32\OpenSavePidlMRU\<ext>` · `…\Explorer\FeatureUsage\{AppSwitched,AppLaunch,ShowJumpView}` · `…\Shell\MuiCache` · `…\AppCompatFlags\Compatibility Assistant\Store` · `…\AppCompatFlags\Layers` · `…\Notifications\Settings\<app>` · `…\PushNotifications\Backup\<app>` · `…\Start\TileProperties` · `…\Search\JumplistData` · `…\ApplicationAssociationToasts` · `…\App Paths\<exe>` · `Software\Classes` (asociaciones) · `Software\<Editor>\<App>`.

**Registro, HKLM**: `SOFTWARE\Classes\<ext|ProgId>` (solo con evidencia de asociación) · `…\App Paths\<exe>` · `SOFTWARE\<nombre>` y `WOW6432Node` (ya) · `…\Uninstall\<clave>` si quedó huérfana.

**Archivos/carpetas**: raíces actuales (AppData, Local, ProgramData, Program Files, Documentos, Menú Inicio) **+** `AppData\Local\{CrashDumps,State,stalefiles}` y archivos sueltos en `Local`/`Roaming` con prefijo + sufijo conocido · `Recent\*.lnk` cuyo destino está en la carpeta de la app o cuya extensión es de la app · `Desktop` y `Public Desktop` (accesos directos al `.exe`).

**Presupuesto de rendimiento**: buscar restos < 2 s en este PC; enumerar `Classes` solo una vez y mirando `shell\open\command` / `DefaultIcon`, no todo su árbol; tope de entradas por fuente. Si se pasa, se avisa y se muestra lo encontrado.

**Fuera de alcance de este plan** (por riesgo o por necesitar más permisos; quedan apuntados): `UserAssist` (valores codificados), `Prefetch` y `BAM` (administrador), listas de saltos binarias, servicios, tareas programadas, reglas de firewall, extensiones de shell, entradas de inicio (siguiente plan), apps MSIX.

---

## 5. Seguridad (todo lo anterior queda igual y se suma esto)

1. **Sin evidencia no hay propuesta.** El buscador propone *candidatos*; **solo el dominio decide** (`judge`).
2. **Valores solo en la lista fija de claves**, y solo si el valor contiene la ruta de la app (con límites de ruta). Nunca por nombre suelto.
3. **Semilla rechazada** si es amplia, corta o genérica; **rechazo si la semilla pertenece a otra app instalada**.
4. **Vista previa siempre**, **confirmación** en diálogo, **cuarentena** y **deshacer** para *todos* los tipos, incluidos los valores.
5. **Nunca se limpia una app que sigue instalada** (ya implementado).
6. **Topes**: máximo de restos por operación y de bytes leídos por valor; si se superan, no se actúa y se explica.
7. **Registro de lo hecho** (`cleanup.jsonl`) con tipo, evidencia y dónde quedó la copia.
8. **Pruebas destructivas solo en ramas de prueba** (`HKCU\Software\PurifexTest…`, carpetas temporales) y con la app de mentira; nunca con apps reales sin confirmación del usuario.

---

## 6. Fases (tareas atómicas)

Cada tarea termina con **pruebas, `tools/check.ps1`, clippy de Linux, commit, push y CI en verde en ese SHA**.

### Fase 1 · Modelo y evidencia (dominio, sin sistema)
- 1.1 `Trace.value`, `TraceKind::RegistryValue`, `Evidence`; serde con valores por defecto; prueba de que el historial viejo se lee.
- 1.2 `Seeds` y `seeds_of`: carpeta/exes/extensiones; rechazo de semillas amplias/cortas; pruebas con `C:\Program Files`, `C:\`, nombres de 1–2 letras.
- 1.3 `path_refers_to(texto, semilla)`: límites de ruta, mayúsculas, `/` vs `\`, `\\?\Volume{…}\` y `\Device\HarddiskVolumeN\…` (aparecen en el registro real). Pruebas con `Kdenlive` vs `Kdenlive2`, `kdenlive.exe` suelto.
- 1.4 Reglas `PathReference`, `NamePrefix`, `CrashDump`, `Association`, `AssociatedExtension` en `judge`, con la lista fija de claves permitidas. **Casos tramposos:** valor con ruta de otra app; `MuiCache` de `…\Kdenlive2\…`; `Classes\.txt`; clave `Microsoft\…` con el nombre de la app.
- 1.5 Recuentos por tipo en `CleanReport` y su texto.

### Fase 2 · Escritura de registro y cuarentena de valores (plataforma)
- 2.1 `registry.rs`: lectura cruda y escritura acotada (`delete_value`, `set_value_raw`, `delete_key_tree`), solo accesible desde la cuarentena.
- 2.2 `quarantine`: valor → `valor.json` (tipo+bytes hex) y restauración exacta; prueba de ida y vuelta con **REG_SZ, REG_DWORD, REG_BINARY, REG_MULTI_SZ, REG_EXPAND_SZ** en una rama `HKCU\Software\PurifexTest`.
- 2.3 Errores de permisos: «requiere administrador» por resto, sin parar los demás.

### Fase 3 · Buscadores (plataforma, solo lectura)
- 3.1 Semillas al **lanzar** el desinstalador: guardar carpeta/exes/extensiones en `uninstalled.jsonl` (`InstalledApp` ampliado con `display_icon`).
- 3.2 Buscador de **archivos** con prefijos, `CrashDumps`, `Recent\*.lnk` (leer el destino del acceso directo), escritorio.
- 3.3 Buscador de **valores** en las claves de la lista fija (HKCU y HKLM).
- 3.4 Buscador de **asociaciones** (`Classes`) y de **claves por extensión** (`FileExts`, `RecentDocs`, `OpenSavePidlMRU`).
- 3.5 Medir tiempos en este PC; ajustar topes.

### Fase 4 · Integración (aplicación y orden)
- 4.1 Conectar los buscadores a `Cleaning` (los candidatos pasan por `judge`).
- 4.2 `purifex apps traces NOMBRE` con recuentos por tipo y evidencia por línea.
- 4.3 `--json` con tipo/evidencia/valor.

### Fase 5 · Interfaz
- 5.1 Diálogo de restos agrupado: «Archivos y carpetas (N)» / «Registro (N)», tamaño total.
- 5.2 Filas de registro con `clave` + `valor`; etiqueta de evidencia («referencia a la carpeta de la app», «asociación de archivo»…).
- 5.3 Resumen por tipo en el panel y en el aviso de «ya se desinstaló».
- 5.4 Capturas de comprobación (diálogo con ambos grupos).

### Fase 6 · Permisos (administrador)
- 6.1 Detectar si el proceso está elevado; marcar en el diálogo qué restos lo necesitan.
- 6.2 Botón **«Reiniciar como administrador»** (una confirmación) y reanudar en la misma pantalla.

### Fase 7 · Cuarentena (cierra el hueco de «no queda rastro»)
- 7.1 **«Vaciar cuarentena»** con confirmación (borrado definitivo, solo dentro de la carpeta de cuarentena de Purifex).
- 7.2 **Caducidad automática a los 7 días.**
- 7.3 Mostrar el espacio que ocupa la cuarentena.

---

## 7. Criterios de aceptación

**A. App de mentira (automática, repetible).** Una app registrada con: carpeta de instalación, `AppData` con `<nombre>rc`, extensión `.pfx` + ProgId con `shell\open\command` hacia su carpeta, valores en una copia de `MuiCache`/`AppCompatFlags`/`FeatureUsage`, `FileExts\.pfx`, `RecentDocs\.pfx`, un `.lnk` reciente y un `.dmp`. Un desinstalador de juguete deja todo. Debe cumplirse:
  1. La vista previa lista **todo** y **nada más**.
  2. «Eliminar todo rastro» lo quita; la cuarentena contiene copia de cada cosa.
  3. «Devolver» restaura **exactamente** (tipo y datos de cada valor) y se compara byte a byte.
  4. Una **segunda app de mentira** del mismo editor, con su carpeta y valores, **queda intacta**.

**B. Kdenlive real (con los restos de este PC).** Antes de que nadie pulse «Limpiar» en Revo, la vista previa de Purifex debe incluir como mínimo los restos de la tabla de §1: las 2 claves de `Classes`, `FileExts\.kdenlive`, `RecentDocs\.kdenlive`, `OpenSavePidlMRU\kdenlive`, `Notifications\Settings\kdenlive`, los valores de `MuiCache`/`AppCompatFlags`/`FeatureUsage`, `kdenliverc`, `kdenlivestaterc`, `stalefiles\kdenlive`, los 2 `.dmp` y los 2 `.lnk`. **Meta: igualar o superar los recuentos de Revo (18 archivos y carpetas, 19 elementos de registro) y no proponer nada que no sea de Kdenlive.** Cualquier resto de Revo que no encontremos se documenta con su motivo.

**C. Rendimiento y seguridad.** Buscar restos < 2 s; la ventana no se bloquea; ningún resto fuera de las reglas; todas las pruebas destructivas en ramas de prueba.

---

## 8. Riesgos y decisiones abiertas

| Riesgo | Mitigación |
|---|---|
| Un valor con la ruta de la app pero que Windows necesita | Lista fija de claves (cachés y listas de uso), evidencia por ruta con límites, vista previa y deshacer |
| Quitar de más en `Classes` | Solo con evidencia de asociación y comando dentro de la carpeta de la app; prueba con `.txt` |
| La app ya no tiene carpeta ni `.exe` al buscar | Semillas capturadas **antes** de desinstalar (3.1); para apps desinstaladas por fuera, solo por nombre y con menos evidencia (se avisa) |
| HKLM sin permisos | Fase 6; mientras tanto se informa por resto |
| Lentitud al recorrer `Classes`/Recent | Presupuesto, topes y búsqueda perezosa solo al pedir restos |
| Coincidencias de nombres cortos/genéricos | Semilla mínima de 3 letras no genéricas (ya); ruta con límites |

**Decisiones que necesito de ti:**
1. ¿Se incluyen los restos de **HKLM** desde la fase 3 (informando «requiere administrador» cuando no se pueda), o se hace todo HKCU primero y HKLM con la fase 6? *Recomiendo lo segundo para tener algo probado antes.*
2. ¿Se muestran los **accesos recientes y volcados de error** en la lista por defecto marcados, o desmarcados? *Recomiendo marcados, igual que Revo, con «Eliminar todo rastro» como atajo.*
3. Orden de las fases: *recomiendo 1 → 2 → 3 (HKCU y archivos) → 4 → 5 → 7 → 6.* La fase 7 (vaciar cuarentena) va antes que los permisos porque sin ella «eliminar» sigue sin liberar espacio.

---

## 9. Estado (2026-11-19)

- **Fases 1, 2 y 3 hechas** (dominio, escritura de registro y cuarentena de valores, buscadores). Falta conectar los buscadores a `Cleaning` (4.1): hoy `Cleaning::clean` juzga con `seeds_of(app)`, sin las extensiones que el buscador descubre en `Classes`; hay que devolverlas junto a los candidatos para que `judge_with` las use.
- **3.5, tiempos medidos en este PC** (`cargo test --release -p purifex-platform measure_the_real_search -- --ignored --nocapture`, en caliente): búsqueda completa **≈ 0,4 s** (descubrir extensiones en `Classes` ≈ 0,1 s, archivos ≈ 0,25 s, valores ≈ 0,01 s, asociaciones ≈ 0,04 s). **En frío** (primera lectura de los accesos directos tras un rato) la parte de archivos tardó **5,6 s**: se añadió un **presupuesto de 1,5 s** para leer destinos de accesos directos; al pasarse, se sigue con lo encontrado y se deja un aviso en el registro de Purifex. Pendiente: mostrar ese aviso en la interfaz (fase 5).
- Fuera de la lista fija por ahora: `Start\TileProperties\W~{…}\<app>`, `CloudStore\…\appmetadata$<app>` y `…\PolicyConfig\PropertyStore\…` (rutas con subclaves variables): se documentarán con su motivo al comparar con Revo (criterio B).
