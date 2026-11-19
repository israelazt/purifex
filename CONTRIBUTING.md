# Contribuir a Purifex

Gracias por querer ayudar. Antes de abrir un cambio:

1. `powershell -File tools/check.ps1` debe pasar (formato, clippy sin avisos, pruebas y
   las pruebas de arquitectura).
2. Un cambio por commit, y la aplicación funcionando después de cada uno. El mensaje dice
   qué cambia, como lo leería una persona.
3. Lee `ARCHITECTURE.md`: dónde va cada cosa. La lógica va en `purifex-domain` (con
   pruebas), lo que toca el sistema detrás de un puerto, y el binario solo conecta.

## Reglas de diseño

- **KISS**: funciones de menos de 50 líneas, con retornos tempranos; sin generalidad que
  nadie pidió.
- **DRY**: un dato, una regla o un formato se escriben una vez. Un tipo que se construye
  en más de tres sitios tiene `Default`.
- **SOLID**: módulos con una sola razón de cambio, puertos pequeños, implementaciones que
  cumplen todo el contrato, y dependencias (rutas, tiempo, registro) como parámetros.
- Nada de `unwrap()` fuera de las pruebas; los errores dicen qué hacer.
- `unsafe` solo en `purifex-platform`, cada bloque con su línea `// SAFETY:`.

## Refactorizar

Un refactor no cambia el comportamiento: las pruebas que ya había pasan sin tocarlas, y si
es de dibujo o de formato de salida se compara antes y después.

## Lo que toca el sistema del usuario

Propone con evidencia, explica, muestra una vista previa, guarda una copia y permite
deshacer. Las pruebas que escriben (registro, archivos) usan una rama o carpeta de prueba
y la dejan vacía.
