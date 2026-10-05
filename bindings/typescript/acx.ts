import type { RunSummary } from './execution';
/** Experimental ACX JSON Lines profile. The transport is a host-owned process/pipe. */
import type { Definition } from './definitions';
import type { Edge, Group, Id, Json, Rect } from './index';
export type AcxOperation =
  | { kind: 'create_node'; id: Id; type_id: string; properties?: Record<string, Json>; rect: Rect }
  | { kind: 'delete_node'; id: Id }
  | { kind: 'move_node'; id: Id; rect: Rect }
  | { kind: 'connect'; edge: Edge }
  | { kind: 'disconnect'; id: Id }
  | { kind: 'set_property'; id: Id; key: string; value?: Json }
  | { kind: 'create_group'; group: Group }
  | { kind: 'delete_group'; id: Id }
  | { kind: 'auto_layout'; gap: [number, number] };
export type AcxIntent =
  | { kind: 'edit'; document_id: Id; expected_revision: number; operations: AcxOperation[] }
  | { kind: 'run'; document_id: Id; expected_revision: number };
export interface AcxError { code: string; message: string }
export interface AcxEnvelope { id: string; method: string; params: unknown }
export type AcxResponse = { id: string | null; result: unknown; error?: never } | { id: string | null; error: AcxError; result?: never };
export type AcxTransport = (request: AcxEnvelope) => Promise<AcxResponse>;
export interface Preflight {
  preflightId: string; provider: string; capability: string; version: string;
  input: AcxIntent; requestJson: string; requestHash: string;
  preflightJson: string; preflightDigest: string; documentHash: string;
  expiresAt: number; effects: string[]; preview: Record<string, unknown>;
  cost: { currency: string; estimated: string; max: string };
  approval: { type: 'policy'; scope: string };
  recovery: { reversible: boolean; condition: string; expiresAt: number };
}
export interface Grant { authorization: string; provider: string; scope: string; expiresAt: number }
export interface Commit { commitId: string; preflightId: string }
export interface Execution { result: Record<string, unknown>; resultJson: string; receiptId: string }
export interface Receipt {
  acx: '0.1'; receiptId: string; capability: string; provider: string;
  status: 'accepted' | 'succeeded' | 'failed' | 'cancelled'; issuedAt: string;
  requestHash: string; resultHash?: string; effects?: string[]; recovery?: Record<string, unknown>;
}
export interface RunJob { commitId: string; run: RunSummary; execution: Execution | null }
export interface Observation { query: 'summary' | 'definitions' | 'nodes' | 'node' | 'edges' | 'groups'; id?: Id; after?: string; limit?: number; expected_revision?: number }
export interface DefinitionPage { documentId: Id; revision: number; nodes: number; edges: number; groups: number; items: Definition[]; nextCursor: string | null }
/** No implicit authorization or retry. Inspect/hash-check a preflight before calling authorize. */
export function createAcxClient(transport: AcxTransport) {
  let sequence = 0;
  async function call<T>(method: string, params: unknown = {}): Promise<T> {
    const id = String(++sequence);
    const response = await transport({ id, method, params });
    if (response.id !== id) throw new Error('ACX response correlation mismatch');
    if (response.error) throw response.error;
    return response.result as T;
  }
  const binding = (p: Preflight) => ({ preflightId: p.preflightId, preflightDigest: p.preflightDigest });
  return {
    discover: () => call<Record<string, unknown>>('discover'),
    observe: (params: Observation) => call<Record<string, unknown>>('observe', params),
    definitions: (params: Omit<Observation, 'query' | 'id'> = {}) => call<DefinitionPage>('observe', { ...params, query: 'definitions' }),
    preflight: (input: AcxIntent) => call<Preflight>('preflight', { input }),
    authorize: (preflight: Preflight) => call<Grant>('authorize', binding(preflight)),
    commit: (preflight: Preflight, grant: Grant) => call<Commit>('commit', { ...binding(preflight), authorization: grant.authorization, input: preflight.input }),
    startRun: (commitId: string) => call<RunJob>('run_start', { commitId }),
    runStatus: (commitId: string) => call<RunJob>('run_status', { commitId }),
    cancelRun: (commitId: string) => call<RunJob>('run_cancel', { commitId }),
    execute: (commitId: string) => call<Execution>('execute', { commitId }),
    receipt: (receiptId: string) => call<Receipt>('receipt', { receiptId }),
    recover: (commitId: string, grant: Grant, expectedRevision: number) => call<Execution>('recover', { commitId, authorization: grant.authorization, expectedRevision }),
  };
}
/** Verify exact provider-emitted bytes; never hash a client JSON reserialization. */
export async function verifyAcxHash(jsonText: string, expected: string): Promise<boolean> {
  const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(jsonText));
  const hex = Array.from(new Uint8Array(digest), b => b.toString(16).padStart(2, '0')).join('');
  return expected === `sha256:${hex}`;
}
