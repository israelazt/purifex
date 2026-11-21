# Purifex · Paridad con Revo Uninstaller (plan, 2026-11-20)

## Objetivo

Que Purifex haga lo mismo que Revo Uninstaller Pro, de principio a fin: elegir una app (de la
lista, arrastrando una mira sobre su ventana, o por su nombre), **desinstalarla con su propio
desinstalador, buscar todo lo que deja y quitarlo**, con copia de seguridad previa y deshacer.
Un solo gesto del usuario, sin pasos manuales entre medias.

Este plan **amplía** `UNINSTALLER_PLAN.md` (el desinstalador) y `DEEP_CLEAN_PLAN.md` (la
búsqueda de restos). No los sustituye: lo que ya está hecho se reutiliza.

## Lo que sí y lo que no se puede igualar

| Función de Revo | Se iguala | Nota |
|---|---|---|
| Desinstalar + escanear + borrar restos | Sí | Es el núcleo, bloques R1–R3 |
| Modos de escaneo Seguro / Moderado / Avanzado | Sí | R3 |
| Desinstalación forzada (app rota o ya borrada) | Sí | R4 |
| Modo Cazador (mira sobre una ventana) | Sí | R5 |
| Monitor de instalaciones y base de registros | Sí, por instantáneas | R6. Revo vigila en tiempo real; con instantáneas antes/después el resultado es el mismo pero no se ve qué proceso escribió cada cosa |
| Varias apps a la vez | Sí | R7 |
| Gestor de inicio (autorun) | Sí | R8 |
| Copias de seguridad y puntos de restauración | Sí | R2 |
| Limpiadores (basura, navegadores, rastros de uso) y borrado irrecuperable | Sí, al final | R9, es otra herramienta que Revo incluye aparte |
| **Base de registros de la comunidad de Revo** (logs ya hechos por otros) | **No** | Es un servicio online suyo. Se sustituye por el monitor propio (R6) y por las reglas de Purifex |
| Marca, iconos y textos de Revo | No | Se copia la función, no el producto |

Revo no publica una lista oficial y completa de funciones; esta se ha armado con su ayuda y con
reseñas ([Softpedia](https://www.softpedia.com/reviews/windows/Revo-Uninstaller-Pro-Review-409696.shtml),
[ghacks](https://www.ghacks.net/2011/12/10/revo-uninstaller-pro-review-giveaway/),
[IObit, Modo Cazador](https://www.iobit.com/en-geo/what-is-hunter-mode-in-revo-uninstaller.php)).
Antes de cerrar R5–R9 hay que comprobar cada función contra la versión actual de Revo (instalarla
en una VM y compararla pantalla por pantalla).

## Dónde estamos (verificado en el código, 2026-11-20)

| Pieza | Estado |
|---|---|
| Lista de apps clásicas + Tienda, buscador, protegidas | Hecho |
| Desinstalar con el oficial (clásico y MSIX), esperar a que desaparezca, abrir los restos | Hecho (`Como Revo`, `UNINSTALLER_PLAN.md`) |
| Restos de archivos, accesos y claves del registro, con evidencia | Hecho (`DEEP_CLEAN_PLAN.md` fases 1–5) |
| Cuarentena 7 días, `.reg` de lo borrado, devolver la última app | Hecho |
| Reinicio como administrador y restos «requiere administrador» | Hecho |
| **«Eliminar todo rastro» sobre una app aún instalada** | **Falla: el botón se rechaza.** Es lo que pidió el usuario y el primer bloque (R1) |
| Servicios, tareas, inicio, firewall, menú contextual, asociaciones | Falta |
| Punto de restauración, página de copias de seguridad | Falta |
| Modos de escaneo, desinstalación forzada, cazador, monitor, lote, autorun, limpiadores | Falta |
| Fecha de instalación, tipo 32/64 bits, ordenar por columna, menú de clic derecho | Hecho (R10) |

## Reglas que no cambian

Las de siempre: nada compartido ni de Windows se toca, todo va a cuarentena y se puede devolver,
cada borrado se anota, ≤ 400 líneas por archivo, funciones ≤ 50, hexagonal (`Port` en el dominio,
adaptador en `purifex-platform`), `tools/check.ps1` en verde y CI en verde antes de seguir. Una
tarea = un commit.

Pruebas con borrado real: **solo** con una app de prueba creada para ello (`tools/` tendrá un
instalador de juguete que deja archivos, claves, un servicio y una tarea) o en una VM. Nunca con
apps reales del PC principal. Para que el usuario vea el flujo real, él elige una app de prueba.

---

## R1 · Un solo gesto: «Desinstalar y limpiar» (hecho 2026-11-20)

Al revisar el código se vio que el flujo ya existía (desinstalar → esperar → buscar restos →
abrir la lista con todo marcado → «Eliminar todo rastro»). Es el mismo que Revo, que también
enseña la lista antes de borrar. Lo que fallaba era entrar por «Buscar restos» en una app aún
instalada. Por eso **no se hizo la máquina de estados `Removal`** pensada al principio: sobraba.

Hecho:
- El diálogo de restos de una app instalada ofrece **«Desinstalar y limpiar»** (pide confirmación
  y sigue el flujo de siempre) en vez de botones que se rechazaban (`be53ac6`, `e595cb0`).
- El panel de Apps dice que **espera** al desinstalador y ofrece **«Dejar de esperar»**; pasados
  30 minutos avisa de que la app sigue instalada en vez de callar (`4b585b7`).
- Con una sola confirmación al principio y la lista de revisión al final, igual que Revo.

Queda para R2: la copia previa, que hoy no forma parte del flujo.

## R2 · Red de seguridad: copia antes de tocar nada (hecho 2026-11-20)

Hecho:
- **Punto de restauración** (`1cafb9c`): puerto `RestorePoints` con `SRSetRestorePointW`. Se pide
  antes de lanzar cada desinstalador. Sin administrador no se crea y el desinstalador sigue; el
  mensaje dice cuál de las dos cosas pasó. Windows guarda como mucho uno cada 24 h, y el mensaje lo
  dice con esas palabras («se pidió», no «se creó»).
- **Copia del registro** (`6536f46`): antes de desinstalar se exporta la clave `Uninstall` de la app
  a `%LOCALAPPDATA%\Purifex\backups\<hora>-<app>\registro.reg`. Las apps de la Tienda no tienen
  esa clave y se saltan. Si la copia falla se anota en el registro de errores y el desinstalador
  sigue.
- **Copias en pantalla** (`1686860`): el panel «Copias del registro de apps desinstaladas» de la
  página de Apps lista las tres más recientes de apps que ya no están, con «Registrar de nuevo»
  (`reg import`). Solo importa un archivo que está en `backups.jsonl`, y nunca si la clave ya existe.

Cambios respecto a lo planeado:
- **No hay página «Copias de seguridad»**: un panel en la página de Apps basta mientras haya pocas.
  Una página aparte tendría sentido con R7 (lotes) y R6 (monitor), que producen muchas más.
- **No hay ajuste «Crear punto de restauración»**: Purifex aún no tiene página de ajustes y el
  punto solo se puede crear siendo administrador, así que sin permisos se omite solo.

Pendiente, anotado:
- **Pedir el punto bloquea la ventana unos segundos** (se pide en el hilo de la ventana). Habrá que
  medirlo siendo administrador y, si molesta, hacer el paso en un hilo antes de lanzar el
  desinstalador.
- **«Devolver todo» de una operación** (restos + registro a la vez) y «Borrar copia».
- El punto de restauración **no se ha probado de verdad**: sin administrador solo falla de forma
  controlada. Hay que probarlo con «Reiniciar como administrador».

## R3 · Escaneo completo y por modos (casi cerrado, 2026-11-20)

Hecho, cada punto con su prueba, y una prueba de punta a punta con el sistema real (carpeta, tarea
y entrada de inicio de una app de prueba: salen a la cuarentena y vuelven):
- **Servicios** (`968c823`): evidencia `ServicePath`, el `ImagePath` (con variables expandidas)
  apunta a la carpeta de la app. Se quitan como cualquier clave de registro (copia `.reg` y devolver).
- **Tareas programadas** (`0afc438`): tipo `Task`, evidencia `TaskAction`, leídas de
  `schtasks /query /xml` (igual en todos los idiomas y sin pedir administrador para leer). Quitar
  guarda la definición y borra con `schtasks`; devolver la recrea y nunca pisa una que exista. La
  ventana las agrupa aparte («Tareas programadas»).
- **Inicio** (`f2e9081`): `Run` y `RunOnce` (usuario y máquina, vistas de 32 y 64 bits).
- **Firewall** (`50443ec`): una regla es un valor de `FirewallRules` cuyo texto (`App=...`) nombra un
  programa dentro de la carpeta de la app; el nombre de la regla es un código y no prueba nada.
- **COM y menú contextual** (`2171d6e`): clases CLSID cuyo servidor está en la carpeta, y verbos
  (`shell`) y manejadores (`ContextMenuHandlers`) del menú contextual que ejecutan o cargan un programa
  de la carpeta, solo en los sitios fijos. La búsqueda ejecuta estos pasos en paralelo: 0,8 s en
  este PC (1,6 s en serie).
- **Borrado al reiniciar** (`d5191b1`, `253b45d`): lo que un programa tiene abierto no se puede mover a
  la cuarentena. Se detecta de verdad (abrir sin compartir falla solo si hay otro manejador abierto) y
  el panel «Restos de X en uso» ofrece «Borrar al reiniciar (N)» con una advertencia: Windows lo borra
  en el próximo arranque y **no se puede devolver**; hace falta administrador. Solo se entrega lo que las
  reglas aún proponen para una app que ya no está y que sigue abierto; cada uno queda en el registro
  (estado `Scheduled`) sin copia. Sin administrador no se programa nada y la oferta se queda.
- **Modos Seguro y Moderado** (`8dec8de`): el botón «Modo» junto al título de la ventana de restos.
  Seguro deja solo lo que apunta a la carpeta de la app; Moderado es lo de siempre. El modo llega
  también a la eliminación: «Eliminar todo rastro» en Seguro nunca toma lo que la ventana no mostró.
  **Avanzado** (nombres parciales, «posible» y desmarcado por defecto) queda pendiente: necesita una
  búsqueda más amplia y es lo más arriesgado.
- **Lista desplazable** (`7a26391`): se acabó el tope de 9 filas. La ventana lista todo y las filas
  se mueven con la rueda dentro de una ventana de altura fija, con barra fina; «Eliminar marcados»
  ya cubre todas las filas, no solo las que cabían. Es una lista plana agrupada con cabeceras
  (archivos, tareas, registro), no un árbol plegable.
- Hallazgo de seguridad que destapó una prueba: otra app instalada en la misma carpeta ahora bloquea
  también servicios y tareas.

Falta de R3: bibliotecas de tipos (TypeLib), otros sitios de Windows (SharedDLLs, Installer), escanear
a todos los usuarios, el modo Avanzado y, si se quiere, plegar los grupos de la lista.

Notas:
- Las tareas siempre se marcan «requiere administrador»: las de máquina las crea un instalador con
  permisos y no hay forma barata de saber de antemano cuáles puede borrar el usuario.
- Una ruta con acentos dentro de una tarea puede no reconocerse (`schtasks` escribe en la página de
  códigos de la consola): no se ha medido.
- El borrado al reiniciar con administrador no se ha probado de verdad (programaría el borrado de
  archivos reales): sí el orden (archivos antes que su carpeta), la detección de «en uso» y el rechazo
  sin administrador.
- Servicios no se probaron con uno real (crear uno pide administrador): sí el buscador sobre una
  rama de prueba del registro y la regla del dominio.

Plan original del bloque (lo hecho arriba cuenta como cumplido):

Aquí está lo que más distingue a Revo: cuántos sitios mira.

1. **Servicios** (`HKLM\SYSTEM\CurrentControlSet\Services`): evidencia = su `ImagePath` está en
   la carpeta de la app. Quitar = parar + borrar el servicio. Prueba con el servicio de juguete.
2. **Tareas programadas** (`\Microsoft\Windows\TaskScheduler` y la carpeta de tareas): evidencia
   = la acción apunta a la carpeta de la app.
3. **Inicio:** `Run`/`RunOnce` (HKCU y HKLM), carpeta Inicio, `StartupApproved`.
4. **Firewall:** reglas con `App=` o `Name=` de la app (`SharedAccess\Parameters\FirewallPolicy`).
5. **Menú contextual y asociaciones:** `*\shellex`, `Directory\shell`, `ContextMenuHandlers`,
   `HKCR\.ext` → ProgId de la app, `Applications\`, `App Paths`.
6. **COM y bibliotecas de tipos:** `CLSID`, `TypeLib`, `Interface`, solo si la ruta del servidor
   está en la carpeta de la app.
7. **Otros sitios de Windows:** `SharedDLLs`, `MUICache`, `Installer\UserData`, `Uninstall`
   huérfano, `AppCompatFlags`. Siempre con la misma regla: la ruta es de la app.
8. **Todos los usuarios:** `C:\Users\*\AppData` y los `HKEY_USERS` cargados (necesita
   administrador; sin él se dice «solo se miró tu usuario»).
9. **Modos:** *Seguro* = solo evidencia fuerte (ruta registrada, ruta exacta); *Moderado* = suma
   coincidencia de editor y nombre con la regla de «nada compartido»; *Avanzado* = suma
   coincidencias por nombre, marcadas «posible» y **desmarcadas por defecto**. El modo es un
   ajuste de la ventana de restos. Pruebas por modo con casos tramposos.
10. **Archivos en uso:** si no se puede mover, programar el borrado al reiniciar
    (`MoveFileEx` con `MOVEFILE_DELAY_UNTIL_REBOOT`) y decirlo en el resultado.
11. **Lista de restos como árbol** con las mismas pestañas de Revo (archivos, registro,
    servicios, tareas...), mostrando la evidencia de cada uno; el límite de 9 filas desaparece
    con una lista con desplazamiento (ver `COMPONENT_LIBRARY_PLAN.md`).

*Hecho cuando:* contra la app de prueba, el escaneo Avanzado encuentra todo lo que se sembró
(archivos, claves, servicio, tarea, inicio, firewall, menú) y el Seguro solo lo evidente; y
ninguna ejecución toca nada de las reglas duras.

## R4 · Desinstalación forzada (cerrado, 2026-11-21)

Hecho (puntos 1 y 3): `forced_app` (dominio) lee lo que escribe la persona (nombre, carpeta o `.exe`)
y lo convierte en una app ya borrada, que pasa por las mismas reglas de siempre. Se rechaza una
carpeta demasiado amplia, de Windows o compartida (`Program Files`, `Common Files`, `Microsoft*`...),
un nombre corto o genérico, y una carpeta que usa una app aún instalada (en cualquier dirección:
la misma, dentro o por encima). Sirve desde la caja de la página Apps («Buscar sus restos») y desde
`purifex apps traces|clean <ruta>`.
Punto 4 hecho: si de la app borrada siguen corriendo programas desde su carpeta, un panel los
lista («X sigue abierta») y ofrece «Cerrar programas (N)» con una advertencia (se cierran a la
fuerza y lo no guardado se pierde); después se busca otra vez. Solo se cierra lo que corre desde
dentro de esa carpeta, nunca Purifex ni una carpeta vecina (`Old Tool` no es `Old Tool Two`); una
app aún instalada no se toca. Probado con un programa real arrancado desde una carpeta temporal.
Punto 2 hecho en lo esencial: **Ctrl+F** lleva a Apps y da el teclado a la caja, que ya acepta un
nombre, una carpeta o un `.exe` (la caja muestra «Ctrl F» en esa página). Y la entrada del menú de
clic derecho, «Desinstalación forzada», quita una app instalada sin su desinstalador (ver R10).
**R4 cerrado (2026-11-21).**

Para apps rotas, sin desinstalador o ya borradas a medias.

1. **Dominio:** `ForcedTarget` = nombre y/o carpeta y/o `.exe` indicados por el usuario; se
   convierte en una `InstalledApp` sintética con las mismas reglas de protección.
2. **Entradas:** botón «Desinstalación forzada» (Ctrl+F) que pide nombre o carpeta, y la
   misma opción en el menú de clic derecho de una app cuyo desinstalador falló.
3. **Flujo:** sin desinstalador oficial; se salta al escaneo (R3) en modo Moderado y se enseña
   la lista completa para confirmar. Un nombre demasiado corto o genérico se rechaza (ya existe).
4. **Cierre de procesos:** si hay procesos de la app en la carpeta, se enseña la lista y se
   ofrece cerrarlos antes (confirmando).

## R5 · Modo Cazador

1. **Plataforma:** ventana mínima de mira que captura el ratón (`SetCapture`) y al soltar usa
   `WindowFromPoint` → proceso → ruta del `.exe` → la app a la que pertenece.
2. **Resolución:** el `.exe` se busca entre las carpetas de instalación conocidas; si es una
   app de la Tienda, por su `PackageFullName`; si es un acceso directo del escritorio o la
   bandeja, por el destino del acceso.
3. **Acciones al soltar:** *Desinstalar*, *Detener y quitar del inicio*, *Cerrar proceso*,
   *Abrir carpeta*, *Cancelar* (las del Cazador de Revo).
4. **Interfaz:** icono de la mira siempre visible (flotante, arrastrable) y atajo de teclado.
5. Prueba automática del tramo «ruta de `.exe` → app» (el arrastre real se prueba a mano).

## R6 · Monitor de instalaciones y base de registros

1. **Instantánea:** `Snapshot` de las ubicaciones del escaneo (R3) con ruta, tamaño y fecha o
   valor. Acotada y rápida (sin leer contenidos).
2. **Instalar con monitor:** el usuario elige un instalador; Purifex toma la instantánea,
   lo ejecuta, espera, toma otra y guarda la **diferencia** como registro (`log`), enlazado a la
   app por el registro de desinstalación que aparezca.
3. **Base de registros** (`%LOCALAPPDATA%\Purifex\logs\`, JSONL): lista, ver, borrar,
   **exportar e importar**.
4. **Uso en la limpieza:** si la app tiene registro, sus elementos entran como evidencia
   `Monitored` (la más fuerte): se proponen aunque no haya coincidencia de nombre.
5. **Más adelante (opcional):** vigilar en tiempo real con ETW (archivos y registro por
   proceso) para saber qué proceso escribió cada cosa. Necesita administrador y es más
   frágil: se decide después de ver el resultado de las instantáneas.

## R7 · Varias apps a la vez

1. Selección múltiple con casillas en la lista y barra «Desinstalar N».
2. Cola: una a una (los desinstaladores abren ventanas), cada una con su copia (R2), y un
   resumen al final con lo hecho y lo fallado.
3. Un solo punto de restauración para todo el lote.

## R8 · Gestor de inicio (autorun)

1. **Lectura:** `Run`/`RunOnce` (HKCU, HKLM, WOW6432), carpetas Inicio, servicios de arranque
   automático, tareas con disparador de inicio de sesión. Una lista, con editor y estado.
2. **Acciones:** activar/desactivar (`StartupApproved`, sin borrar), quitar (a cuarentena) y
   abrir la carpeta del programa.
3. **Interfaz:** página «Inicio» y enlace desde cada app («arranca con Windows»).

## R9 · Herramientas de limpieza (las de la sección «Herramientas» de Revo)

Cada una es independiente y puede quedarse fuera sin romper lo anterior.

1. **Basura de Windows:** temporales, miniaturas, volcados, caché de Windows Update; con tamaño
   y vista previa; todo a cuarentena.
2. **Navegadores:** caché, cookies, historial de Edge, Chrome, Firefox (con el navegador
   cerrado; las contraseñas nunca se tocan).
3. **Rastros de uso («evidencia»):** documentos recientes, listas de salto, `MUICache`, historial
   de ejecutar.
4. **Borrado irrecuperable:** sobrescribir y borrar archivos/carpetas elegidos, con aviso claro
   de que **no se puede deshacer** (la única función que no pasa por cuarentena).
5. **Complementos de navegador:** listar y quitar extensiones.

## R10 · Lista de apps al nivel de Revo (cerrado, 2026-11-21)

Hecho (puntos 1, 2 y 5): se leen `InstallDate`, `URLInfoAbout` y `HelpLink` (solo se guarda una
dirección `http(s)`). La lista muestra fecha de instalación, tamaño y tipo («64 bits», «32 bits»,
«Usuario», «Tienda»); las cabeceras «App y editor», «Instalada» y «Tamaño» ordenan; y los filtros
«Todas / Programas / Apps de Windows / Instaladas hace poco (30 días)» llevan su cuenta. Las apps de
la Tienda no dicen cuándo se instalaron: no cuentan como recientes.
Punto 3 hecho: **clic derecho** sobre una app abre un menú con Desinstalar, Desinstalación
forzada, Abrir carpeta de instalación, Abrir clave de registro, Sitio web y Propiedades (lo que no
aplica a esa app se ve apagado). «Propiedades» (o pulsar la fila) abre los detalles bajo la fila:
carpeta, web, clave de registro y comando de desinstalación, con botones para abrirlos. Solo se
abre una carpeta, una página o una clave que una app de la lista registró; las páginas son `https`;
la clave se abre con Regedit (escribe su ajuste `LastKey` y Windows pide permiso).
Punto 4 hecho: dos vistas, «Detalles» y «Lista» (compacta), y el filtro, el orden y la vista se
recuerdan en `%LOCALAPPDATA%\Purifex\apps-view.json`.
**Desinstalación forzada de una app instalada** (la del menú y la que faltaba en R4): no se usa su
desinstalador; tras el mismo punto de restauración y copia del registro, su entrada de `Uninstall`
va a la cuarentena (se puede devolver) y se buscan y muestran sus restos como siempre. Se rechaza
una app protegida, una de la Tienda (no tiene entrada propia) y una entrada que no se pudo quitar.
**R10 cerrado (2026-11-21).**

1. Columnas: nombre, editor, versión, tamaño, **fecha de instalación**, tipo (32/64 bits, Tienda).
2. Ordenar por columna; filtros «Todas / Programas / Apps de Windows / Instaladas hace poco».
3. Menú de clic derecho: Desinstalar, Desinstalación forzada, Abrir carpeta de instalación,
   Abrir clave de registro, Sitio web, Propiedades.
4. Vistas: lista y detalles; la elección se recuerda.
5. Leer `InstallDate`, `URLInfoAbout`, `HelpLink` del registro (hoy no se leen).

---

## Orden y por qué

`R1 → R2 → R3 → R4 → R10 → R5 → R7 → R6 → R8 → R9`

- **R1** primero: es lo que el usuario necesita ya y usa solo piezas que existen.
- **R2** antes de ampliar lo que se borra: cuanto más se borre, más importa poder volver.
- **R3** es el corazón de la calidad; **R4** y **R10** son pequeños y lo completan.
- **R5, R7** son comodidad; **R6** es lo más grande y lo más arriesgado; **R8, R9** son herramientas
  aparte.

## Criterios de aceptación globales

1. Con la app de prueba, un clic + una confirmación: sin app, sin restos, y «Devolver» la trae de vuelta.
2. Comparación con Revo en una VM: misma app, mismo escaneo Avanzado. Purifex encuentra al menos
   lo que encuentra Revo, y lo que Purifex marca por defecto nunca incluye nada que Revo deje fuera
   por ser compartido.
3. Ninguna prueba toca las reglas duras (`Windows`, `System32`, claves de Microsoft, compartidos).
4. Cada bloque cierra con su sección «Hecho» aquí, con fecha y commit.

## Decisiones abiertas (necesito respuesta antes de R3, R6 y R9)

1. **App de prueba para el borrado real:** ¿qué app del PC aceptas quitar (por ejemplo Paint o
   una que ya no uses) para probar el flujo de punta a punta? Mientras tanto uso el instalador
   de juguete.
2. **R6, tiempo real:** ¿instantáneas bastan, o quieres también el monitor en vivo con ETW (pide
   administrador siempre)? Recomiendo empezar solo con instantáneas.
3. **R9:** ¿entran los limpiadores de navegador y el borrado irrecuperable? Son lo más
   delicado (datos de usuario, sin deshacer). Recomiendo dejar el borrado irrecuperable para
   el final y con una confirmación doble.
4. **Cazador (R5):** ¿mira flotante siempre visible, o solo desde un botón de la ventana?
