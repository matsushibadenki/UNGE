/** Definition metadata returned by ACX observe/definitions. This is UNGE's typed
 * property schema, not arbitrary JSON Schema. Rust performs authoritative validation.
 * Int bounds use i64 in Rust; JS callers must keep integer values/bounds in the safe
 * integer range or use a transport with lossless integer handling. */
import type { Json, Port } from './index';
export interface LocalizedText { en: string; ja: string; zh_cn: string }
export type PropertyType =
  | { kind: 'bool' | 'json' }
  | { kind: 'int' | 'float'; minimum: number | null; maximum: number | null }
  | { kind: 'string'; min_length: number; max_length: number | null; choices: string[] | null };
export interface PropertyDefinition {
  name: LocalizedText;
  description: LocalizedText;
  value_type: PropertyType;
  required: boolean;
  default: Json;
}
export interface PropertySchema { fields: Record<string, PropertyDefinition>; additional_properties: boolean }
export interface Definition {
  type_id: string; version: string; name: LocalizedText; description: LocalizedText;
  inputs: Port[]; outputs: Port[]; pure: boolean; property_schema: PropertySchema;
}
