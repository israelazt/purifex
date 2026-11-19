//! The text of `purifex help`.

pub const HELP: &str = "Purifex: lo que hay en tu PC, rápido y sin tocar nada.

Uso:
  purifex scan [--json] [--all]   Lista los dispositivos y sus drivers
  purifex watch [--now] [--seconds N]
                                  Graba el rendimiento mientras juegas a LoL
  purifex sessions [--json]       Lista las grabaciones, la más nueva primero
  purifex analyze [NOMBRE] [--json]
                                  Analiza una grabación (por defecto, la última)
  purifex compare [ANTES DESPUÉS] [--json]
                                  Compara dos partidas (por defecto, las dos últimas)
  purifex apps [--json]           Apps instaladas (solo lista)
  purifex mcp                     Servidor MCP para tu IA (lo lanza tu cliente de IA, no se usa a mano)
  purifex updates [--json]        Drivers que ofrece Windows Update (solo lista; no instala)
  purifex fixes [--json]          Arreglos que se proponen, con la medición que los justifica
  purifex fix apply ID [--dry-run] [--yes]
                                  Aplica un arreglo (sin --yes solo dice qué haría)
  purifex fix undo N [--yes]      Deshace el cambio N del historial
  purifex history [--json]        Lo que se ha cambiado
  purifex ui [--screenshot ARCHIVO.png [--page NOMBRE] [--size ANCHOxALTO] [--scale N]]
                                  Abre la interfaz (o la guarda como imagen)
  purifex help                    Esta ayuda

Opciones de scan:
  --json   Salida en JSON, para scripts
  --all    Incluye también los dispositivos que el sistema inventa (raíz, software)

Opciones de watch (sin opciones: espera a que abras la partida y para al cerrarla):
  --now        Graba ya, sin esperar al juego
  --seconds N  Graba N segundos (implica --now)
";
