/** Definition metadata returned by ACX observe/definitions. This is UNGE's typed
 * property schema, not arbitrary JSON Schema. Rust performs authoritative validation.
 * Int bounds use i64 in Rust; JS callers must keep integer values/bounds in the safe
 * integer range or use a transport with lossless integer handling. */
export type { Definition, LocalizedText, PropertyDefinition, PropertySchema, PropertyType } from './generated-output';
