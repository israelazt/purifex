# Purifex

Una app de Windows que mide qué limita el rendimiento de tu PC (y, más adelante, arregla lo que se pueda con evidencia y sin dejar nada sin deshacer). Gratuita, sin telemetría, sin proceso residente.

## Compilar y ejecutar

```
cargo build --release
target\release\purifex.exe help
```

Antes de cada commit: `powershell -File tools/check.ps1` (formato, clippy sin avisos y tests, guardas incluidas).

## Órdenes

| Orden | Hace |
|---|---|
| `purifex scan [--json] [--all]` | Lista los dispositivos y sus drivers (solo lectura, ~200 ms) |
| `purifex watch [--now] [--seconds N]` | Espera a `League of Legends.exe`, graba una muestra por segundo y resume con pistas |
| `purifex sessions [--json]` | Lista las grabaciones, la más nueva primero |
| `purifex analyze [nombre] [--json]` | Analiza una grabación (por defecto, la última) |
| `purifex ui [--no-open]` | Abre la interfaz en una ventana (Edge en modo aplicación) |
| `purifex help` | La ayuda |

Las grabaciones quedan en `%LOCALAPPDATA%\Purifex\sessions\` (un CSV por partida).

## Más

- `ARCHITECTURE.md`: las capas y las reglas.
- `docs/FASE1_PLAN.md`: el plan de la fase actual.
- `docs/DESIGN.md`: el diseño de la interfaz (referencia en `design/screen.png`, iconos Lucide).

## La interfaz

`purifex ui` arranca un servidor mínimo **solo en `127.0.0.1`** (sin dependencias) y abre una ventana de Edge sin barra de direcciones. La página va incluida en el `.exe`; la API exige un token distinto en cada ejecución, valida `Host` y es de solo lectura. Cuando cierras la ventana, el programa se apaga solo: no queda nada residente.
