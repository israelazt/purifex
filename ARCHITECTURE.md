# Arquitectura de Purifex

Misma disciplina que Luminifex (`dockview/ARCHITECTURE.md`): núcleo puro y probable, Windows detrás de puertos, el binario solo conecta, y el compilador y unas guardas impiden romper las reglas. Plan completo en `dockview/docs/PURIFEX_PLAN.md`; fase actual en `docs/FASE1_PLAN.md`.

Las convenciones completas de la familia (capas, estilo, tests, código de Windows, CLI y MCP, planes) están en la skill **`fexarium-rust`** (`~/.claude/skills/fexarium-rust/`): no hace falta abrir dockview para consultarlas.

| Crate | Es | Contiene | No puede |
|---|---|---|---|
| `purifex-domain` | El **qué** | Datos y reglas puras: `Device`, `DriverVersion`, `Date`, `ScanReport` (salud, edad, resumen), el texto del informe y los **puertos** (`SystemInventory`, `Clock`), el log | Windows, disco, red, hilos |
| `purifex-platform` | El **cómo** en Windows | Adaptadores que implementan los puertos: inventario con SetupAPI/CfgMgr32, reloj, log a stderr. `decode` (bytes de Windows → datos) es código puro | Conocer la app |
| `purifex-app` | El **cuándo** | Servicios (`Scanner`) y qué hace cada orden de la línea de comandos (`Command`). Solo contra puertos; se prueba con dobles | Windows |
| `purifex` (binario) | La **raíz de composición** | Crea los adaptadores, se los da a `app`, imprime la salida | Lógica |

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
- [ ] Limitaciones conocidas: sin tiempos de fotograma (haría falta ETW/PresentMon); la frecuencia del CPU que da `CallNtPowerInformation` suele marcar siempre el máximo; solo GPU NVIDIA.
- [ ] Bloque 2 en adelante: ver el plan.
