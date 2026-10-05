/** Compile-time contract checks; no runtime or framework dependency. */
import type { Command, PointerEvent, Value } from './index';
import type { Definition, RunSummary } from './generated-output';
const deletion: Command = { kind: 'set_property', id: 'id', key: 'value', value: null };
const defaults: Command = { kind: 'set_property', id: 'id', key: 'value' };
const pointer: PointerEvent = { kind: 'down', pointer: 1, button: 'primary', position: [0, 0] };
const resource: Value = { kind: 'resource', value: { id: 'id', data_type: { kind: 'image' } } };
// @ts-expect-error position must contain exactly two coordinates
const invalidPointer: PointerEvent = { kind: 'move', pointer: 1, position: [0] };
// @ts-expect-error custom types require a name
const invalidResource: Value = { kind: 'resource', value: { id: 'id', data_type: { kind: 'custom' } } };
function outputFields(definition: Definition, run: RunSummary) {
  const fields = definition.property_schema.fields;
  const reason: string | null = run.reason;
  return { fields, reason };
}
void [deletion, defaults, pointer, resource, invalidPointer, invalidResource, outputFields];
