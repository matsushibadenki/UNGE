export type { Definition, LocalizedText, PropertyDefinition, PropertySchema, PropertyType } from './definitions';
/** No graph mirror or frame data. Inject Tauri's invoke from the host application. */
export type Id = string;
export type Json = null | boolean | number | string | Json[] | { [key: string]: Json };
export type DataType = { kind: 'bool' | 'int' | 'float' | 'string' | 'json' | 'bytes' | 'image' | 'audio' | 'video' | 'tensor' | 'event' | 'any' } | { kind: 'custom'; name: string };
export interface Port { name: string; data_type: DataType; cardinality: 'single' | 'multiple'; required: boolean }
export interface Node { id: Id; type_id: string; inputs: Port[]; outputs: Port[]; properties: Record<string, Json> }
export interface Rect { x: number; y: number; width: number; height: number }
export interface Endpoint { node: Id; port: string }
export interface Edge { id: Id; from: Endpoint; to: Endpoint }
export interface Group { id: Id; label: string; nodes: Id[] }
export type Command =
  | { kind: 'add_node'; node: Node; rect: Rect }
  | { kind: 'remove_node'; id: Id }
  | { kind: 'move_node'; id: Id; rect: Rect }
  | { kind: 'connect'; edge: Edge }
  | { kind: 'disconnect'; id: Id }
  | { kind: 'set_property'; id: Id; key: string; value: Json }
  | { kind: 'set_group'; id: Id; group: Group | null }
  | { kind: 'batch'; commands: Command[] };
export interface Viewport { origin: [number, number]; zoom: number; size: [number, number] }
export interface Summary { revision: number; nodes: number; edges: number }
export interface ApiError { code: string; message: string }
export type PointerEvent =
  | { kind: 'down'; pointer: number; position: [number, number]; button: 'primary' | 'pan'; additive?: boolean }
  | { kind: 'move' | 'up'; pointer: number; position: [number, number] }
  | { kind: 'cancel' };
export type Locale = 'en' | 'ja' | 'zh-cn';
export type Request =
  | { kind: 'set_locale'; locale: Locale }
  | { kind: 'pointer'; expected_revision: number; event: PointerEvent }
  | { kind: 'apply'; expected_revision: number; command: Command }
  | { kind: 'undo' | 'redo'; expected_revision: number }
  | { kind: 'set_viewport'; viewport: Viewport }
  | { kind: 'select'; ids: Id[] }
  | { kind: 'summary' };
export type Invoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;
/** Pass the revision displayed when the user started the edit. On conflict, refresh and ask the user to retry. */
export function createClient(invoke: Invoke) {
  const dispatch = (request: Request) => invoke<Summary>('dispatch', { request });
  return {
    pointer: (event: PointerEvent, revision: number) => dispatch({ kind: 'pointer', event, expected_revision: revision }),
    locale: (locale: Locale) => dispatch({ kind: 'set_locale', locale }),
    summary: () => dispatch({ kind: 'summary' }),
    apply: (command: Command, revision: number) => dispatch({ kind: 'apply', command, expected_revision: revision }),
    undo: (revision: number) => dispatch({ kind: 'undo', expected_revision: revision }),
    redo: (revision: number) => dispatch({ kind: 'redo', expected_revision: revision }),
    viewport: (viewport: Viewport) => dispatch({ kind: 'set_viewport', viewport }),
    select: (ids: Id[]) => dispatch({ kind: 'select', ids }),
    inspect: (id: Id) => invoke<Node>('inspect', { id }),
  };
}
