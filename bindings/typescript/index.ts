import type { RunSummary } from './execution';
export type { ExecutionStatus, ExecutionStopReason, ProgressEvent, RunSummary, RunState } from './execution';
export { executionStatusMessages, executionStopMessages, runStateMessages } from './execution';
export type { Definition, LocalizedText, PropertyDefinition, PropertySchema, PropertyType } from './definitions';
/** No graph mirror or frame data. Inject Tauri's invoke from the host application. */
import type { Id, Node, Command, PointerEvent, Theme, Locale, Viewport, Request, GroupAction } from './generated';
import type { Summary, Appearance, AccessiblePage, GroupPage } from './generated-output';
export type { Id, Json, DataType, Cardinality, Port, Node, Rect, Endpoint, Edge, Group, Command, Viewport, PointerEvent, Theme, Locale, Request, GroupAction, Value, Document } from './generated';
export type { Summary, ApiError, Appearance, AccessiblePage, AccessibleNode, GroupPage, GroupSummary } from './generated-output';
export type ThemeToken = 'background' | 'surface' | 'border' | 'accent' | 'text' | 'muted' | 'hover';
export type Invoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;
/** Pass the revision displayed when the user started the edit. On conflict, refresh and ask the user to retry. */
export function createClient(invoke: Invoke) {
  const dispatch = (request: Request) => invoke<Summary>('dispatch', { request });
  return {
    startExecution: (revision: number) => invoke<RunSummary>('start_execution', { expectedRevision: revision }),
    currentExecution: () => invoke<RunSummary | null>('current_execution'),
    executionStatus: (id: Id) => invoke<RunSummary>('execution_status', { id }),
    cancelExecution: (id: Id) => invoke<RunSummary>('cancel_execution', { id }),
    pointer: (event: PointerEvent, revision: number) => dispatch({ kind: 'pointer', event, expected_revision: revision }),
    groups: (revision: number, after: Id | null = null, limit = 50) => invoke<GroupPage>('groups', { expectedRevision: revision, after, limit }),
    group: (action: GroupAction, revision: number) => dispatch({ kind: 'group', expected_revision: revision, action }),
    accessibleNodes: (revision: number, after: Id | null = null, limit = 50) => invoke<AccessiblePage>('accessible_nodes', { expectedRevision: revision, after, limit }),
    appearance: () => invoke<Appearance>('appearance'),
    theme: (theme: Theme) => dispatch({ kind: 'set_theme', theme }),
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
