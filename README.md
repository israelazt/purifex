# Purifex

Una app de Windows que mide qué limita el rendimiento de tu PC (y, más adelante, arregla lo que se pueda con evidencia y sin dejar nada sin deshacer). Gratuita, sin telemetría, sin proceso residente.

## Compilar y ejecutar

```
cargo build --release
target\release\purifex.exe help
```

Antes de cada commit: `powershell -File tools/check.ps1` (formato, clippy sin avisos y tests, guardas incluidas). `tools/ship.ps1 -Message "..."` hace el commit y el push solo si todo está en verde. Para contribuir, lee `CONTRIBUTING.md`.

## Órdenes

| Orden | Hace |
|---|---|
| `purifex scan [--json] [--all]` | Lista los dispositivos y sus drivers (solo lectura, ~200 ms) |
| `purifex watch [--now] [--seconds N]` | Espera a `League of Legends.exe`, graba una muestra por segundo y resume con pistas |
| `purifex sessions [--json]` | Lista las grabaciones, la más nueva primero |
| `purifex analyze [nombre] [--json]` | Analiza una grabación (por defecto, la última) |
| `purifex compare [antes después] [--json]` | Compara dos partidas (por defecto, las dos últimas): qué mejoró, qué empeoró y qué quedó igual |
| `purifex fixes` · `purifex fix apply ID [--yes]` · `purifex fix undo N [--yes]` · `purifex history` | Arreglos: se proponen solo con una partida medida que los justifique, se explican, se anotan y se deshacen. Sin `--yes` solo dicen qué harían |
| `purifex ui [--screenshot ARCHIVO.png]` | Abre la interfaz (o la guarda como imagen) |
| `purifex help` | La ayuda |

Las grabaciones quedan en `%LOCALAPPDATA%\Purifex\sessions\` (un CSV por partida).

## Más

- `ARCHITECTURE.md`: las capas y las reglas.
- `CONTRIBUTING.md`: qué debe pasar antes de un cambio y las reglas de diseño (KISS, DRY, SOLID).
- `docs/FASE1_PLAN.md`: el plan de la fase actual.
- `docs/DESIGN.md`: el diseño de la interfaz (referencia en `design/screen.png`, iconos Lucide).

## La interfaz

`purifex ui` abre una ventana **nativa** (Win32 + Direct2D, iconos Lucide): un solo proceso, ~48 MB y 0 % de CPU cuando no la tocas. No usa navegador ni servidor local. Con `--screenshot` guarda la misma pantalla como PNG, que es como se comprueba el diseño.
