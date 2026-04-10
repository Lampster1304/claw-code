# Interactive terminal thinking visibility design

## Problem
En modo interactivo, el usuario percibe silencio (“no responde nada”) porque el thinking se colapsa por defecto y la zona de entrada no queda claramente delimitada tras la respuesta.

## Goals
1. Mostrar thinking en terminal interactiva de forma visible y entendible.
2. Eliminar ambiguedad de “donde escribir” con una separacion visual consistente.
3. Dar feedback explicito cuando el modelo tarda en responder.

## Non-goals
1. Cambiar el formato de `--json` o `--compact`.
2. Implementar una TUI compleja con paneles colapsables.
3. Alterar el contrato de proveedores o el routing two-mode.

## Approved behavior
### 1) Interactive thinking output
- En modo interactivo normal, el thinking se imprime en vivo (texto completo).
- El estado inicial se muestra como `Thinking...` (sin emoji).
- `--hide-thinking` desactiva esta salida y mantiene comportamiento colapsado.

### 2) Clear input boundary
- Al finalizar cada turno del asistente, se imprime una linea divisoria horizontal.
- Debajo de la linea se muestra `› Tu mensaje` para marcar claramente la zona de escritura.

### 3) No-response feedback
- Si no llega contenido visible en 10 segundos, se imprime:
  - `Aún esperando respuesta del modelo...`
- Si hay error de conexion/proveedor, se muestra error explicito con causa (sin fallback silencioso).

### 4) Mode boundaries
- `--json` y `--compact` conservan su comportamiento actual.
- Los cambios de thinking visible y separador aplican solo al flujo interactivo normal.

## Technical design
### Runtime output toggles
- Introducir/propagar un flag `show_thinking` para el runtime interactivo.
- Valor por defecto: `true` en interactivo.
- `--hide-thinking` lo fuerza a `false`.

### Stream handling changes
- En `ContentBlockDelta::ThinkingDelta`, cuando `show_thinking=true`, renderizar texto thinking incremental en vivo.
- Mantener aviso breve para `RedactedThinking` cuando el proveedor oculta bloques.
- Evitar mostrar solo “Thinking hidden” en el camino interactivo por defecto.

### Prompt boundary rendering
- Agregar helper dedicado para renderizar:
  - linea divisoria
  - etiqueta `› Tu mensaje`
- Invocarlo al cierre del turno para mantener consistencia visual.

### Delayed-response notice
- Agregar temporizador de 10s desde inicio del turno hasta primer contenido visible.
- Si vence, imprimir aviso de espera una sola vez por turno.

## Error handling
- Sin catches amplios ni retornos silenciosos.
- Los errores de transporte/conexion deben propagarse y mostrarse con mensaje accionable.

## Testing plan
Agregar/actualizar pruebas en `rusty-claude-cli` para cubrir:
1. Thinking visible en modo interactivo por defecto.
2. `--hide-thinking` mantiene salida colapsada.
3. Aviso de 10s cuando no hay contenido.
4. Render de linea divisoria + `› Tu mensaje` al final del turno.
5. Invariancia de `--json` y `--compact`.

## Acceptance criteria
1. El usuario ve thinking en vivo en modo interactivo sin activar flags.
2. El estado inicial aparece como `Thinking...` sin emoji.
3. Tras cada respuesta queda clara la zona de escritura mediante separador + `› Tu mensaje`.
4. Si no hay respuesta inicial en 10s, aparece aviso explicito.
5. `--json` y `--compact` no cambian.
