# ACX Node Graph Async Jobs v1 — experimental optional extension

## English

### Problem and compatibility

The synchronous Node Graph `execute` method blocks a serial JSON Lines pipe, preventing an agent from inspecting or cancelling a pending run. This optional extension starts an already-approved `run` commit on a worker while the pipe continues accepting requests. It changes no mandatory ACX Core vocabulary, existing Intent, synchronous execute result or Receipt schema. It applies only to trusted, allowlisted pure local nodes authorized by the Node Graph Profile. It does not authorize paid APIs, external effects, arbitrary executables or new transports.

Advertise only under the run capability's `extensions["org.unge.node-graph"].asyncJobs`:

```json
{"profile":"experimental-node-graph-jobs-v1","methods":["run_start","run_status","run_cancel"],"key":"commitId"}
```

Providers without a configured host execution service omit this object or use null. Clients MUST check discovery before using the extension. Method parameters use the existing JSON Lines envelope and exactly `{ "commitId": "..." }`; extra parameter fields MUST be rejected. Schema: `acx-node-graph-job.schema.json`, with `$defs.params` for arguments.

### Methods

| Method | Semantics |
|---|---|
| run_start | Validate an existing approved run commit, freshness and expiry, then reserve and spawn execution. Return job metadata immediately |
| run_status | Return current metadata and, once finalized, the immutable execution/Receipt reference |
| run_cancel | Request cancellation of this commit's job and return metadata. Idempotent; terminal jobs are unchanged |

Response: `{ "commitId": "...", "run": RunSummary, "execution": null | Execution }`. Execution is the existing `{ result, resultJson, receiptId }` object. RunSummary includes UUID id/document_id, snapshot revision, monotonic sequence, state queued/running/finished/failed, total/started/finished counts, completed/cached/failed/blocked/cancelled counts, cancel_requested, nullable reason and error_code. Reason is completed/cancelled/deadline_exceeded or null. Details contain no output payload, properties or secrets; final results use the existing bounded scalar/resource-handle representation.

`state=finished` is not a guarantee of success or final receipt availability. Workers may still serialize/hash results. Clients MUST continue polling until `execution != null`, then verify exact resultJson bytes, resultHash and request binding through the Receipt as in the base profile. A complete traversal can contain failed or blocked nodes. Cancelled runs use the base profile's failed execution Receipt, retaining their node status and reason in job metadata; a cancel response is never an execution Receipt.

### Approval, replay and failure

Before first start, the Provider MUST require a committed, authorized Run intent and validate document identity, revision, hash and expiry. Edit commits are rejected. Agents cannot supply node code, an alternative snapshot, execution policy or arbitrary run IDs. Successful start binds one immutable run ID to the commit. Subsequent starts return that same job/result without executing again, even after expiry or document edits. Status and cancel address the same provider-owned commit, not any other host run.

The reference permits one unfinished job worker, including result finalization. Busy/admission failures before launch consume no execution and create no job; the commit can be retried only while still fresh/unexpired. Once a job starts, synchronous execute MUST NOT execute it again: pending execution returns execution_uncertain, and finalized execution returns the original result/Receipt. Retrieval and cancellation do not recheck current document revision because the run owns the original snapshot. Cancellation does not mutate the Document, revoke approval, undo edits or roll back external effects.

Completion produces exactly one immutable Receipt per commit, including execution errors and cancellation. JSON Lines requests remain serial; workers compute results, while the Provider finalizes received outcomes during subsequent dispatch. No unsolicited response frames are introduced. Call run_status until finalized. Restart invalidates in-memory jobs, grants and receipts; the extension provides no persistent exactly-once guarantee. Never retry effects after uncertain execution.

### Bounds and security/privacy

The dedicated provider pipe is the authority boundary inherited from the base profile. Commit IDs are bearer references scoped to that provider instance and must not be logged or shared. No network listener or multi-user authentication is added. Cancellation is limited to a job this Provider admitted under the original approved capability, using host cooperative cancellation; it does not terminate arbitrary processes.

Job count is bounded by existing retained sessions; one worker and one bounded outcome channel per unfinished job prevent an unbounded task/result queue. Host snapshot/cache limits still apply. Job metadata and completed execution references survive host RunService summary eviction while their Provider session is retained. Provider teardown cancels its owned unfinished jobs; detached external work is outside this guarantee. Polling clients should use a bounded interval and a client deadline, request cancellation if needed, and continue until the execution outcome is received. Receipt hashing confirms byte consistency, not authenticated origin.

### Example and conformance

After discover/observe/preflight/authorize/commit:

```json
{"id":"10","method":"run_start","params":{"commitId":"approved-run-commit"}}
{"id":"11","method":"run_status","params":{"commitId":"approved-run-commit"}}
{"id":"12","method":"run_cancel","params":{"commitId":"approved-run-commit"}}
```

Conformance tests MUST reject unapproved/unknown/edit/stale commits and extra arguments; verify repeat start yields one execution; query while an executor is Pending; cancel that executor; finalize a failed Receipt without document mutation; verify terminal result/Receipt replay; verify metadata after host summary eviction and slot release on Provider teardown. The companion schema tests valid pending/terminal frames and reject missing identity or unsupported states.

## 日本語

同期execute中にもAIが照会・キャンセルできるよう、承認済みrun commitの任意拡張 `experimental-node-graph-jobs-v1` を定義します。発見情報のasyncJobsを確認し、同じcommitIdでrun_start/run_status/run_cancelを使います。既存Intent・execute・ReceiptとACX Coreは変更しません。

初回開始時は承認・commit・Document/revision/hash・期限を検証し、編集commitを拒否します。開始後の再送は同じ実行を返し、再実行しません。キャンセルは自分のProviderが開始したjobだけを対象にし、Documentや履歴を変更しません。外部副作用を巻き戻す保証はありません。

応答はcommitId、snapshotのrun概要、nullまたは既存executionです。finishedだけではReceipt確定を意味せず、executionがnullでなくなるまで照会します。resultJsonとReceiptを既存手順で検証してください。キャンセル・失敗も失敗Receiptとして一度だけ記録します。未完了workerは1件、保持件数は既存session上限で制限します。再起動をまたぐ保証はありません。規定の詳細は英語節、機械検証は付属Schemaを使います。

## 简体中文

可选扩展experimental-node-graph-jobs-v1允许AI在执行期间查询和取消已获批的run commit。先检查发现信息asyncJobs，再使用同一commitId调用run_start/run_status/run_cancel。既有Intent、execute、Receipt及ACX Core不变。

首次启动验证授权、commit、文档/revision/hash及有效期，拒绝编辑commit。启动后重放返回同一执行，不会重跑。取消只针对本Provider启动的job，不修改文档、历史或撤销外部副作用。

响应包含commitId、snapshot执行概要和null或既有execution。finished不保证Receipt已生成，应轮询至execution非null，并按原约定验证resultJson和Receipt。失败及取消只记录一次失败Receipt。最多一个未完成worker，保留条目受session上限约束；重启后不提供持久保证。完整约定以英文部分为准，并使用附带Schema验证。
