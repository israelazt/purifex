# Arquitectura de Purifex

Misma disciplina que Luminifex (`dockview/ARCHITECTURE.md`): núcleo puro y probable, Windows detrás de puertos, el binario solo conecta, y el compilador y unas guardas impiden romper las reglas. Plan completo en `dockview/docs/PURIFEX_PLAN.md`; fase actual en `docs/FASE1_PLAN.md`.

Las convenciones completas de la familia (capas, estilo, tests, código de Windows, CLI y MCP, planes) están en la skill **`fexarium-rust`** (`~/.claude/skills/fexarium-rust/`): no hace falta abrir dockview para consultarlas.

| Crate | Es | Contiene | No puede |
|---|---|---|---|
| `purifex-domain` | El **qué** | Datos y reglas puras: `Device`, `DriverVersion`, `Date`, `ScanReport` (salud, edad, resumen), el texto del informe y los **puertos** (`SystemInventory`, `Clock`), el log | Windows, disco, red, hilos |
| `purifex-platform` | El **cómo** en Windows | Adaptadores que implementan los puertos: inventario con SetupAPI/CfgMgr32, reloj, log, muestreador, y la ventana Win32 con Direct2D/DirectWrite. `decode` y `nt_layout` (bytes de Windows → datos) son código puro | Conocer la app |
| `purifex-app` | El **cuándo** | Servicios (`Scanner`) y qué hace cada orden de la línea de comandos (`Command`). Solo contra puertos; se prueba con dobles | Windows |
| `purifex` (binario) | La **raíz de composición** | Crea los adaptadores reales, se los da a `app`, imprime la salida; `shell/` conecta `Core`, `Ui` y la ventana | Lógica |

```
              purifex (binario)
              /        |       \
   purifex-app         |   purifex-platform
              \        |       /
              purifex-domain
```

Las flechas solo van hacia abajo. `app` y `platform` no se ven entre sí.

## Reglas

1. Lo que toca hardware, disco o red va detrás de un puerto de `domain`; el adaptador real vive en `platform`.
2. Receta de una función nueva: reglas en `domain` con tests → puerto → servicio en `app` con un doble → adaptador en `platform` → cableado en el binario.
3. Guardas (`crates/purifex-domain/tests/architecture.rs`): dependencias permitidas por crate, ninguna fuente de más de 400 líneas, y nada de `println!` salvo excepciones escritas con su motivo (usar `log_info!`/`log_warn!`/`log_error!`).
4. El informe da **hechos**, no órdenes: nunca dice «actualiza esto». Decidir si conviene es una capa posterior (riesgo y canal, Bloque 0 del plan).
5. Solo lectura: el analizador no cambia nada del sistema.

## Estado

- [x] Bloque 0 (parcial): esqueleto, guardas, `Device`/`DriverVersion`/`Date`/`ScanReport`. Falta: nivel de riesgo, canal estable/última, decisión «no tocar».
- [x] Bloque 1: inventario con SetupAPI (`purifex scan [--json] [--all]`).
- [x] Grabación de rendimiento (`purifex watch`): `PerformanceSampler` y `SessionFile` como puertos; `Recorder` espera a `League of Legends.exe`, graba un CSV por segundo y resume con pistas (`session`, `diagnosis`). Muestreo: tabla de procesos y tiempos por núcleo con `NtQuerySystemInformation`, GPU NVIDIA con NVML (cargado en caliente; temperatura y throttling cada 5 s porque esa llamada tarda ~20 ms). Coste medido: ~12 ms por muestra.
- [x] Bloque 1 de la fase 1 (2026-11-18): frecuencia real del CPU con PDH (`% Processor Performance` sobre la base; en este PC ~4070 MHz con base 3801), familias de procesos (`process_group`: el cliente de League son tres procesos y se suma el total), cabecera versionada del CSV (`session_header`) y resumen guardado junto al CSV (`<nombre>.summary.json`).
- [x] Interfaz nativa (2026-11-18, `docs/DESIGN.md`): `purifex ui` con Direct2D. Lo que se muestra se decide en el dominio (`overview`, `drivers_view`, `Analysis::stat_rows`, `Analysis::is_game`, `Model::visible_drivers`); `purifex-ui` solo dibuja y traduce entradas; `Core` aplica las acciones. Sustituye a una primera versión web (descartada por peso: 16 procesos, 936 MB). Medido: 1 proceso, 48 MB, 0 % de CPU en reposo.
- [x] Bloque 2 completo: `sessions`, `analyze`, `compare` y el lector de grabaciones (`session_csv`, también las antiguas). `compare` (dominio `compare.rs`) juzga cada medida con su sentido y su ruido: una diferencia dentro del ruido es «igual», y avisa si la comparación puede no ser justa. La ventana muestra cada partida contra la anterior (`Core::compare_with_previous`).
- [x] Arreglos (2026-11-18): `Fixer` (app) es la única vía por la que el programa cambia algo: propone solo con medición, aplica solo con confirmación y sin bloqueos, lee de vuelta y revierte si no se quedó, y no cambia nada sin anotarlo. Primer arreglo: cerrar el cliente de League durante la partida (`LCULocalPreferences.yaml`, copia previa y escritura atómica, nunca con el cliente abierto). La ventana tiene la página Arreglos con confirmación en línea. Falta probarlo con el cliente de League cerrado en el PC del autor.
- [ ] Limitaciones conocidas: sin tiempos de fotograma (haría falta ETW/PresentMon); solo GPU NVIDIA.
- [ ] Bloque 2 en adelante: ver el plan.
