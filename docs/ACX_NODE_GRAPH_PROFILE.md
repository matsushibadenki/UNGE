# ACX Node Graph Profile 0.1 — experimental

[English](#english) · [日本語](#日本語) · [简体中文](#简体中文)

## English

This optional profile connects agents to a Rust-owned node graph. It uses the existing ACX 0.1 Manifest and Receipt schemas without adding mandatory core vocabulary. The reference implementation is UNGE's `unge-acx` crate. It is **not MCP** and does not change the existing experimental print HTTP demo.

### Capabilities and authority

| Capability | Risk | Scope | Meaning |
|---|---|---|---|
| `org.unge.graph.observe` | observational | `graph:read` | Read summaries, definitions, nodes, edges and groups |
| `org.unge.graph.edit` | reversible | `graph:edit` | Apply one atomic edit transaction with conditional undo |
| `org.unge.graph.run` | consequential | `graph:run` | Execute an immutable, revision-bound graph snapshot |

Publish profile information under `capability.extensions["org.unge.node-graph"]`. This namespace identifies the draft implementation; production providers should use a namespace they control. Declare policy approval, effects, free local execution and plain unsigned receipts. Policy comes from the trusted host, not request parameters. The UNGE reference starts read-only by default; its demo explicitly enables arithmetic editing/execution. Run requires both a host type allowlist and trusted definitions marked pure. It does not authorize paid APIs, process execution or external side effects. Additional execution backends need their own effects, economics and approval terms.

The parent grants access by launching the provider and handing its dedicated stdin/stdout to the agent. There is no network listener. Sharing this pipe grants the configured capabilities; it is not an authenticated multi-user service. Bearer authorization tokens and commit IDs are bound to one provider process, preflight, document and scope. They must not be logged or reused in another session.

### Binding and messages

Advertise `bindings[].type: "other"`, `profile: "experimental-node-graph-stdio-v1"`, `framing: "json-lines"`. Discovery is a method, not an HTTP route. Requests are one UTF-8 JSON object per line:

```json
{"id":"1","method":"observe","params":{"query":"summary"}}
```

Success: `{"id":"1","result":{...}}`. Failure: `{"id":"1","error":{"code":"revision_conflict","message":"..."}}`. A malformed envelope returns `id: null`. Unknown operations and top-level fields in typed lifecycle arguments are rejected. Nested geometry/edge/group objects use the host's normalized UNGE representation. Implementations must cap line allocation before deserialization; the reference limit is 256 KiB. The reference handles one request at a time.

| Method | Params | Result |
|---|---|---|
| `discover` | `{}` | Schema-valid ACX Manifest with embedded intent schema |
| `observe` | `query`, optional `id`, `after`, `limit`, `expected_revision` | Document ID/revision and bounded metadata |
| `preflight` | `input` matching Node Graph Intent schema | Normalized input, preview, document hash, request/preflight digest artifacts, expiry, policy and recovery terms |
| `authorize` | `preflightId`, `preflightDigest` | Host-policy grant: `authorization`, provider, scope, expiry |
| `commit` | `preflightId`, `preflightDigest`, `authorization`, `input` | `commitId`, `preflightId`; exact normalized intent is bound |
| `execute` | `commitId` | `result`, `resultJson`, `receiptId` |
| `receipt` | `receiptId` | ACX 0.1 Receipt |
| `recover` | `commitId`, original `authorization`, `expectedRevision` | Recovery result, exact result bytes and a separate recovery receipt |

Observation queries: `summary`, `definitions`, `nodes`, `node` (requires `id`), `edges`, `groups`. Pages use lexicographically ordered IDs/type IDs; `nextCursor` is the last returned key, or null. Set `expected_revision` when continuing pages to detect changes. Limits are 1–100 items per page and 128 KiB of observation metadata. Reduce the page size on `limit_exceeded`.

### Node Graph Intent

[`acx-node-graph-intent.schema.json`](../schemas/profiles/acx-node-graph-intent.schema.json) defines `edit` and `run`. Every intent names `document_id` and `expected_revision`. Editing accepts 1–256 sequential operations:

`create_node`, `delete_node`, `move_node`, `connect`, `disconnect`, `set_property`, `create_group`, `delete_group`, `auto_layout`.

Client-assigned UUIDs let later operations reference newly created nodes within the same transaction. Node creation names a registered type; the host supplies its Port schema. Clients do not supply executable code or Port definitions. Connection/type/cycle validation applies during preview and at the final atomic command. Required-input/property validation happens before execution in the host executor. `set_property.value` null (or omitted) deletes the property; storing JSON null itself is not supported by this draft adapter.

Preflight MUST NOT mutate the graph, execute nodes, consume graph history or create an externally visible effect. It previews edits on an isolated document. The provider checks document identity/revision/content at authorization, commit and execution. The host applies revision checking and mutation under the same lock. Invalid edits roll back as one transaction. Run executes the captured revision; subsequent UI edits do not alter that snapshot.

### Digest artifacts: `provider-json-utf8-v1`

The original Python HTTP demo does not specify cross-language floating-point canonicalization. This profile instead returns the **exact JSON strings whose UTF-8 bytes were hashed**:

- `requestJson`: provider-normalized intent. `requestHash = "sha256:" + lowercase_hex(SHA256(UTF8(requestJson)))`.
- `preflightJson`: the preflight object excluding `preflightJson` and `preflightDigest`. `preflightDigest` hashes these exact bytes.
- `resultJson`: execution/recovery result. Receipt `resultHash` hashes these exact bytes.

Clients verify each byte hash, decode each string and compare it with the corresponding structured object, including provider/capability, document/revision, effects, cost, policy and expiry. Do not reserialize the objects in another language to calculate these hashes. Commit sends the structured normalized input, which the provider normalizes with the same typed encoding and compares to the stored request hash. No text normalization is applied. This profile is not RFC 8785/JCS and does not claim cryptographic origin authentication. The receipt is plain, not signed.

### Replay, failure and recovery

Preflight expiry is integer Unix seconds (default 120 seconds). Expiry includes the boundary `now >= expiresAt`. Authorize/commit/new execute/recover reject expired grants. Duplicate commits fail with `already_committed`. Successful or failed execute retries return the original result and receipt, even after expiry; failures must not rerun a partially completed executor. An interrupted in-memory attempt may return `execution_uncertain`.

Recovery requires a successful edit, the original authorization, an unexpired grant, the exact revision created by that edit and retained undo history. Any intervening UI/agent edit or undo/redo prevents recovery; the provider must never blindly undo someone else's latest operation. Run is not recoverable. Recovery itself advances the document revision and creates a new receipt with `effects: ["graph-edit-rollback"]`, `recovery.state: "recovered"` and a result naming `recoveredCommitId`. Its request hash links to the original edit intent; the new result hash covers the rollback result. The original receipt remains immutable. A retry returns the same recovery result and does not undo twice.

The reference retains at most 128 preflights and their outcomes per provider process; when full it rejects new preflights, preserving existing replay evidence. State is in memory only. Restart loses authorizations, receipts and replay records. This is not durable exactly-once execution. Host deployments need persistent transactions before enabling durable side effects. Large images/tensors/frames remain in Rust/GPU stores; only resource IDs, bounded scalar previews and hashes cross the pipe.

### Interoperability impact and required vectors

Core Manifest/Receipt validators remain unchanged. Consumers that do not support the `other` binding or extension can skip these capabilities. Export the intent schema alongside adapters so a copied UNGE directory does not depend on an ACX symlink. A Python agent example independently verifies Rust-emitted hashes and receipts. This remains experimental until independently maintained providers and governance review establish interoperability.

Required vectors: discover/schema validation; one edit creates/connects arithmetic nodes; result 42; preflight has no mutation; foreign token; changed intent/digest; stale revision at authorize/commit/execute; invalid edge/cycle; duplicate commit; replay after expiry; failed execution replay; denied executor types; successful recovery and replay; recovery after a UI edit is refused; bounded malformed/oversized messages.

- [Done] Experimental intent schema, example and profile validation vectors.
- [Done] UNGE Rust adapter and Python client, including shared Tauri Document integration.
- [Next] Independent provider interoperability and durable policy/receipt storage.
- [Later] MCP/A2A bindings, signatures, paid/external executors and multi-user authentication.

## 日本語

この実験的Profileは、AIからRustが所有するノードグラフを照会・編集・実行するための任意拡張です。既存のACX Manifest/Receiptをそのまま使い、標準必須機能は増やしません。MCPではなく、専用の標準入出力へJSONを1行ずつ送るbindingです。

`discover → observe → preflight → authorize → commit → execute → receipt` の順に操作します。編集は `document_id` と `expected_revision` に固定した単一トランザクションです。作成・削除・移動・接続・切断・プロパティ変更・グループ・自動配置に対応します。Port定義はホストの登録ノード定義から取得し、AIが実行コードやPortを持ち込む形式にはしません。

承認ポリシーはホストが設定します。既定では読み取りのみで、UNGEの例では算術ノードだけを明示的に許可します。実行には型の許可リストとpureな信頼済み定義の両方が必要です。API課金や外部への副作用はこのProfileの無料ローカル実行権限には含めません。

事前確認はDocumentを変更しません。承認・確定・実行のたびに同じDocument/リビジョン/内容であることを確認し、実際の編集は同一ロック内でリビジョン検査とCommand適用を行います。繰り返しexecuteを送っても保存済み結果とReceiptを返し、処理を重複させません。失敗した実行も再実行しません。

言語間の数値表現の違いに対応するため、`requestJson`、`preflightJson`、`resultJson` の正確なUTF-8バイト列をSHA-256で照合します。別言語で再シリアライズしたJSONをハッシュに使いません。Receiptは署名なしで、整合性の確認と発行元の認証は別です。

`recover` は元の承認と、編集直後のリビジョンが保たれている場合だけUndoします。他の画面やAIによる変更が入ったら拒否します。回復後は別Receiptを発行し、元のReceiptを書き換えません。実行処理の巻き戻しは対象外です。

期限は既定120秒、リクエスト256KiB、操作256件、一覧100件、保持するpreflight128件です。期限切れや容量超過は明示的に拒否します。状態はメモリ上にあり再起動をまたぐ保証はありません。大容量データはRust/GPU側に保持します。詳細なwire定義・拒否ケースは上の英語節が規定します。

## 简体中文

此实验性Profile是可选扩展，使AI能够查询、编辑和执行由Rust持有的节点图。它沿用现有ACX Manifest/Receipt，不增加核心必选字段。它不是MCP，而是通过专用标准输入输出逐行传递JSON的binding。

流程为 `discover → observe → preflight → authorize → commit → execute → receipt`。编辑绑定 `document_id` 和 `expected_revision`，作为单一事务执行。支持创建、删除、移动、连接、断开、属性修改、分组和自动布局。Port定义来自宿主注册表，代理不能提交可执行代码或伪造Port。

授权策略由宿主配置，默认只读。UNGE示例仅明确允许算术节点。执行必须同时满足类型白名单和可信pure定义；免费本地执行授权不包含付费API及外部副作用。

预检不会修改文档。授权、提交和执行均核验文档、版本和内容；版本检查与Command应用在同一锁内完成。重复execute返回原结果和回执，不重复执行，失败结果也不会再次执行。

通过对 `requestJson`、`preflightJson`、`resultJson` 的原始UTF-8字节计算SHA-256，避免不同语言的数字序列化差异。不要用客户端重新序列化的JSON计算摘要。回执未签名，一致性验证不等于来源认证。

`recover` 需要原授权，且文档必须保持在该编辑生成的版本。其他界面或代理修改后，恢复会被拒绝。恢复生成独立回执，不修改原回执，也不撤销节点执行的外部效果。

默认有效期120秒、请求上限256KiB、操作256项、分页100项、保留128次预检。状态仅在内存中，重启后失效；大容量数据保留在Rust/GPU端。完整wire定义和拒绝场景以上方英文部分为准。
