# Purifex · Desinstalador de apps y borrado de rastros (plan, 2026-11-18)

## Qué se pidió

Poder desinstalar apps y eliminar cualquier rastro de ellas.

## Decisión de fondo

«Cualquier rastro» no se puede prometer sin riesgo: borrar de más rompe otras apps o Windows, y borrar de menos deja basura. Por eso Purifex borra **solo lo que puede atribuir a esa app con evidencia**, enseña la lista completa antes y deja deshacer. Todo lo dudoso se muestra como «posible rastro, no se toca por defecto».

## Cómo se desinstala

1. **El desinstalador oficial primero.** Se lee la entrada de registro de desinstalación (`HKLM/HKCU\...\Uninstall`, `QuietUninstallString` o `UninstallString`) y para apps de la Tienda (MSIX) la API de paquetes. Purifex no reimplementa el desinstalador: lo lanza y espera. Pedirá permisos de administrador solo si la app lo necesita.
2. **Después, los restos.** Con la app ya desinstalada, se buscan rastros y se proponen.

## Qué cuenta como rastro (y con qué evidencia)

| Rastro | Evidencia exigida |
|---|---|
| Carpeta de instalación (la de `InstallLocation`) | Era la ruta registrada de esa app |
| `%AppData%`, `%LocalAppData%`, `%ProgramData%`, `Documentos` con el nombre del fabricante/app | Nombre exacto del editor o de la app **y** carpeta no usada por otra app instalada. Datos del usuario (Documentos, partidas guardadas) nunca se marcan por defecto |
| Claves de registro bajo `Software\<Editor>\<App>` | Coinciden con editor y app; no hay otra app del mismo editor instalada |
| Servicios, tareas programadas, entradas de inicio, reglas de firewall | Su ruta apunta a una carpeta de la app |
| Accesos directos, entradas del menú contextual | Apuntan a un ejecutable de la app |

Reglas duras (código, no opinión): **nunca** se toca `C:\Windows`, `System32`, `Program Files` fuera de la carpeta propia de la app, claves de `Microsoft`/`Windows`, ni nada compartido por más de una app. Si el editor tiene otra app instalada, sus carpetas comunes se conservan.

## Seguridad (heredada del marco de arreglos)

- Siempre **vista previa**: lista de lo que se borraría, con tamaño y la evidencia de cada elemento. Nada se borra sin confirmación.
- **Punto de restauración** antes de desinstalar y **copia** de las claves de registro borradas (`.reg`) y de las carpetas pequeñas; las grandes van a una cuarentena (mover, no borrar) durante 7 días y luego se eliminan.
- Historial append-only y **deshacer** mientras la copia exista.
- Las aplicaciones del propio sistema y las protegidas (antivirus, controladores gráficos, runtimes que otras apps usan) se marcan «no tocar».
- Dos modos: «Desinstalar» (solo el oficial) y «Desinstalar y limpiar» (con los restos).

## Fases

1. **Dominio** (sin sistema): `InstalledApp`, `Trace`, `Evidence`, reglas de qué es borrable y qué está protegido, vista previa. Pruebas con casos tramposos (editor compartido, ruta con `..`, enlaces simbólicos).
2. **Lectura**: puerto `AppInventory` (registro de desinstalación **y** paquetes MSIX); página «Apps» con lista, tamaño y editor, solo lectura.
3. **Rastros en seco**: `purifex apps traces NOMBRE` busca y muestra, sin borrar.
4. **Desinstalar con el oficial** (con confirmación) y registrar en el historial.
5. **Limpieza con cuarentena y deshacer**, primero en máquina virtual con apps de prueba.
6. **Interfaz**: acciones en la página, confirmación en línea, resultado y deshacer.

Las pruebas con borrado real se hacen en una VM o con una app de prueba creada para ello, nunca con apps reales del PC principal.

## Decisiones (2026-11-18)

- **Todo, sin rastro:** «Desinstalar y limpiar» incluye por defecto los datos del usuario de esa app (configuración, partidas, cachés). Contrapeso: la vista previa los lista aparte, marcados «datos tuyos», y se confirma todo junto; y nada se borra al instante: va a cuarentena 7 días, así que hasta entonces se puede recuperar.
- **Cobertura total:** apps de escritorio y de la Tienda (MSIX) desde el principio, y también servicios, tareas, entradas de inicio, firewall, menú contextual y asociaciones de archivos.
- Siguen en pie las reglas duras (nunca `Windows`, `System32`, claves de Microsoft ni nada compartido con otra app): «ningún rastro» significa ningún rastro **de esa app**.

## Avance (2026-11-18)

Funciona hoy, con pruebas y probado de punta a punta con una app de mentira (registro, carpetas y un desinstalador de juguete que deja restos):

- Lectura de apps instaladas (registro de desinstalación: HKLM 64 y 32 bits y HKCU); las que otras apps o Windows necesitan se marcan «no tocar» y no se desinstalan desde aquí.
- Lanzar el **desinstalador oficial** de la app (con confirmación). No espera: la app muestra su propia ventana y se termina allí. El comando se rechaza si encadena órdenes.
- **Buscar restos** (vista previa, no toca nada) y **quitarlos**: solo si la app ya no está instalada. Las reglas del dominio (`app_traces`) rechazan Windows, System32, Program Files y carpetas amplias, y carpetas de un editor que tenga otra app instalada.
- **Cuarentena**: carpetas movidas (no borradas) a `%LOCALAPPDATA%\Purifex\quarantine\N`; claves de registro exportadas a `.reg` antes de borrarlas; todo anotado en `cleanup.jsonl`; **devolver** cualquier eliminación (nunca sobrescribe).
- Línea de órdenes: `purifex apps [traces|uninstall|clean|restore|history] NOMBRE [--yes] [--json]`. Pantalla: página **Apps**.

Pendiente (actualizado 2026-11-20):

- **Servicios, tareas programadas, entradas de inicio, reglas de firewall, menú contextual y asociaciones de archivos**: no se buscan aún (la limpieza profunda de `DEEP_CLEAN_PLAN.md` cubre archivos, accesos directos y claves del registro, no estos).
- **Punto de restauración** del sistema antes de desinstalar: no se crea (pediría administrador; ahora que existe el reinicio como administrador se puede ofrecer); la cuarentena y la copia de las claves son la red de seguridad.
- **Deshacer desde la pantalla**: el historial se devuelve con `purifex apps restore N`; falta el botón en la página de Apps.
- Probado con una app de mentira y con Kdenlive real (solo lectura), **no desinstalando apps reales** de este PC.

Hecho desde entonces: vaciar y caducar la cuarentena a los 7 días (`DEEP_CLEAN_PLAN.md`, fase 7), buscador de la página de Apps, y las apps de la Tienda (abajo).

### Apps de la Tienda (MSIX) (2026-11-20)

- **Lista:** `StorePackages` (WinRT `PackageManager`, solo lectura, sin administrador) las suma a las clásicas con `AllApps`; si una fuente falla, las otras se siguen viendo y el fallo va al registro. En este PC: 54 paquetes.
- **Desinstalar:** `StoreUninstaller` quita el paquete del usuario con `RemovePackageWithOptionsAsync` en su propio hilo (la ventana no se congela); `UninstallByKind` elige entre el desinstalador registrado y este según el tipo de app. Después se vigila que desaparezca de la lista, igual que con las clásicas.
- **Protegidos («es parte de Windows»):** por el nombre del paquete, no por lo que se ve en pantalla: Inicio, shell, Configuración, Seguridad de Windows, la Tienda, el instalador de paquetes, Xbox Identity, Gaming Services, Widgets, winget, paquetes de idioma, runtimes (VCLibs, UI.Xaml, .NET, Windows App Runtime) y los códecs y formatos de imagen (`Microsoft.*Extension*`). Calculadora, Bloc de notas, Paint, etc. sí se pueden quitar.
- **Probado:** la lista real, la protección con nombres reales de este PC y la tubería de borrado con un paquete que no existe (falla con el motivo de Windows y no toca nada). **No se ha quitado ningún paquete real**: falta probarlo con una app de prueba que la persona elija.

## Como Revo Uninstaller (2026-11-18)

- Al pulsar «Desinstalar» y confirmar en el diálogo, se abre el desinstalador oficial y Purifex **espera a que termine**: lee la lista cada 1,5 s (hasta 30 min) y, cuando la app desaparece, busca sus restos y **abre solo la ventana con la lista**, todo marcado.
- En esa ventana: casillas para elegir, «Eliminar marcados (N)» y **«Eliminar todo rastro»** (un solo paso, sin elegir, con confirmación). Hasta 9 restos se listan; si hay más, solo «Eliminar todo rastro» se lleva los demás. Lo quitado va a la cuarentena y se puede devolver.
- Funciona también si la app se desinstala por fuera de Purifex (al volver a la ventana se ve que desapareció) o si se escribe su nombre en el buscador de Apps.

> Plan ampliado de limpieza profunda (igualar a Revo Uninstaller): ver `docs/DEEP_CLEAN_PLAN.md`.
