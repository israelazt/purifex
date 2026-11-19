# Purifex · Actualizador de drivers y papel de la IA (plan, 2026-11-18)

## Decisión de fondo

La IA **encuentra e identifica**; nunca **decide ni instala**. Todo lo que llegue a instalarse pasa por comprobaciones deterministas que la IA no puede saltarse.

Por qué: una página web puede contener texto que manipule a un modelo (inyección de instrucciones), y un modelo puede equivocarse de variante de hardware. Un driver equivocado o falso en el kernel es lo peor que puede hacer esta app. Por eso la confianza sale del código (firma, ID de hardware, dominio, hash), no de la respuesta del modelo.

## Tres niveles por dispositivo

| Nivel | Fuente | Quién decide | Estado |
|---|---|---|---|
| 1. Windows lo ofrece | Windows Update / Catálogo de Microsoft, buscando por ID de hardware | Código (sin IA) | Primero, solo lectura |
| 2. Fabricante con fuente fiable y estructurada | NVIDIA, AMD, Intel (sus catálogos/APIs oficiales) | Código (sin IA) | Después |
| 3. Cola larga (Realtek, chipset de portátil, periféricos, OEM) | Búsqueda web | IA propone, código verifica | Último |

La mayoría de los dispositivos se resuelven en 1 y 2. La IA solo entra donde de verdad hace falta, y así el escáner es rápido, barato y reproducible.

## Cómo entra la IA: «trae tu propia IA» (recomendado)

Purifex **no tiene clave de API ni acceso a internet para buscar**. Expone el escáner por el **servidor MCP de solo lectura** ya previsto (bloque 8): el cliente de IA del usuario (Claude u otro) pide la lista de dispositivos sin fuente de nivel 1/2, busca en la web y devuelve **candidatos** (URL + versión + motivo).

Ventajas: no se envía nada a un servicio nuestro; respeta «Sin telemetría»; cero costes; el usuario ya confía en su cliente de IA. Alternativa (más cómoda, peor para la privacidad): llamada directa a una API desde la app, solo con consentimiento explícito y mostrando exactamente qué se envía (únicamente `VEN_xxxx&DEV_xxxx&SUBSYS` y versión actual; nunca nombre de equipo, usuario, serie ni rutas).

## Verificación determinista de un candidato (obligatoria)

Un candidato propuesto por la IA solo se muestra como «instalable» si pasa **todo**:

1. **Dominio** en la lista permitida por fabricante (descarga solo `https`, redirecciones revisadas).
2. **Hash** SHA-256 publicado por el fabricante, o catálogo firmado.
3. **Firma Authenticode** válida y cadena hasta Microsoft/fabricante; catálogo `.cat` del paquete verificado.
4. **El INF contiene el ID de hardware** del dispositivo (coincidencia exacta, con SUBSYS si existe).
5. **Versión y fecha** mayores que las instaladas (nunca un retroceso silencioso).
6. Sin nada de lo anterior: se enseña como «enlace sugerido, no verificado» y **no se instala desde Purifex**.

La respuesta de la IA nunca se interpreta como instrucción: es datos (`Candidate`) que se validan.

## Seguridad al instalar (heredada de los arreglos)

Instalación solo con confirmación, **punto de restauración** y copia del driver actual (export), lectura posterior del estado, historial y deshacer. Los dispositivos críticos (pantalla, red, almacenamiento de arranque) quedan marcados «no tocar» salvo petición explícita. Instalaciones probadas solo en una máquina virtual, nunca en el PC principal por defecto. Requiere un ayudante elevado con permisos mínimos.

## Fases (atómicas, cada una con commit + CI)

1. **Dominio:** tipos `Candidate`, `Source`, `Risk`, `Verdict`; reglas de verificación puras (dominio, versión, ID) con tests; el veredicto es una función pura.
2. **Nivel 1, solo lectura:** consultar el Catálogo de Microsoft por ID de hardware; mostrar en la página Drivers qué hay disponible y qué riesgo tiene. Puerto `DriverSource`, adaptador en `platform`, falso en tests.
3. **Reparto:** clasificar cada dispositivo en nivel 1 / 2 / 3 / «sin candidato».
4. **MCP solo lectura:** herramientas `list_unresolved_devices` y `submit_candidate` (la segunda solo guarda; no instala).
5. **Verificador:** descarga a carpeta aislada, hash, Authenticode, INF; resultado visible en la UI con cada comprobación.
6. **Nivel 2** (NVIDIA primero: es la tarjeta de este PC).
7. **Instalación** con restauración y deshacer, en VM.

## Decisiones (2026-11-18)

- La IA se conecta **solo por MCP** (trae tu propia IA). Purifex no llama a ninguna API de IA.
- Un candidato de nivel 3 (`Source::Suggested`) **nunca se instala desde Purifex**: como mucho se abre su página oficial, y solo si pasa las reglas de dirección (https, dominio de la lista, sin usuario/puerto/letras parecidas). Implementado en `domain::driver_candidate::judge`.

## Avance

- Fase 1 hecha: `crates/purifex-domain/src/driver_candidate.rs` (`Candidate`, `Source`, `Checks`, `Verdict`, `Risk`, `judge`, `risk_of`), 9 pruebas.
- Fase 2 hecha: `domain::driver_offers` (cruce de ofertas con dispositivos), puerto `DriverOffers`, adaptador `WindowsUpdateOffers` (Windows Update Agent, solo lista), búsqueda en segundo plano (`app::Offering`), orden `purifex updates` y panel + etiqueta por fila en la página Drivers. Solo se consulta cuando el usuario pulsa el botón. Probado en este PC: 17 s, Windows ofrece 1 componente de software (Voice Clarity) que no corresponde a un dispositivo físico, así que no se muestra.
- Fase 3 hecha: `domain::driver_tiers` reparte cada dispositivo (Windows ofrece / catálogo del fabricante por VEN_ de NVIDIA, AMD, Intel / necesita búsqueda / nada que hacer). Necesita búsqueda: sin driver, con fallo, o driver de un fabricante (no Microsoft) de 2 años o más. Probado en este PC: 36 con catálogo del fabricante, 1 sin fuente (Realtek Bluetooth, fallo 31).
- Fase 4 hecha: servidor MCP de solo lectura (`purifex mcp`, JSON-RPC por entrada/salida estándar). Herramientas: `list_unresolved_devices` (busca en Windows Update y devuelve lo que nadie cubre, con IDs de hardware), `submit_candidate` (juzga la propuesta con `judge`; solo guarda las que no se rechazan, en `%LOCALAPPDATA%\Purifex\candidates.jsonl`) y `list_candidates`. Ninguna herramienta instala, descarga ni borra; un test lo exige. Los títulos que vienen de fuera se limpian y solo se muestran.
- Para conectarlo a un cliente de IA (por ejemplo Claude Code): `claude mcp add purifex -- C:uta\purifex.exe mcp`.
