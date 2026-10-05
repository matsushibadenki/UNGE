# 共有実行サービス

`unge-executor::RunService` はRustホストが所有する、snapshot実行・進捗照会・キャンセル・記録保持のサービスです。Tauri、GPU、ACXへ依存しません。1サービスにつき同時実行は1件です。UIとAIには同じサービスを渡してください。

## ホスト設定

```rust
use std::sync::Arc;
use unge_executor::{RunLimits, RunService, Scheduler};
use unge_tauri::Engine;

let registry = Arc::new(my_registry());
let editor = unge_core::Editor::new(document, 256)?
    .with_validator(registry.clone())?;
let execution = RunService::new(
    registry.clone(), Scheduler::new(4, 128), RunLimits::default(),
);
let engine = Engine::from_editor(editor).with_execution(execution.clone());
// with_executionはEngineをclone/manageする前に呼ぶ。
// View登録、Renderer接続、app.manage(engine)は従来どおり。
```

既定の上限は実行snapshotの10,000ノード・30,000エッジ・シリアライズ相当16 MiBです。これは性能目標の達成を意味しません。完了記録は直近32件、設定値0でも最低1件保持します。実行中の1件は完了記録とは別に保持します。保持するのは小さなRunSummaryとtokenであり、全Reportやイベント履歴は保持しません。完了済みsnapshotと出力はworkerの戻り値をホストが処理したあと解放できます。Schedulerのキャッシュ容量、ResourceStoreの寿命、プロセス全体のメモリ量は別の契約です。

## Rustでの実行

```rust
let prepared = execution.prepare(&snapshot, execution_revision)?;
let run_id = prepared.id();
// ホストworkerで呼ぶ。executeは非同期ノードの終了までブロックする。
let outcome = prepared.execute(|summary| {
    // 小さな状態の通知。短く非ブロッキングにする。
})?;
```

- prepareはグラフ・登録型・入力・容量を検証して1件の予約を作り、snapshotをRust内で凍結します。呼び出し元がdocument ID/revisionを照合する責務を持ちます。
- 実行IDは不変です。現在の実行はcurrent、既知IDはinspectで取得します。cancelは冪等で、完了済み記録を変更しません。破棄された古い記録はunknown_runです。
- PreparedRunを実行せずdropすると予約を解放し、execution_abortedを記録します。unwindする実行中のpanicを捕捉した場合も予約を解放してexecution_panickedを記録し、キャッシュを破棄します。外部副作用は巻き戻しません。
- execute_untilにホストのdeadline Futureを渡せます。Tauri/ACXの標準呼び出しは期限なしです。ノード内のHTTP/workerタイムアウトは別途設定します。
- 終了通知前にSchedulerを解放します。通知関数のpanicは通常実行中なら実行を中断します。実行結果の確定後の通知はadvisoryで、通知panicが確定済み結果を上書きしません。

## RunSummaryと整合性

id、document_id、revisionは開始snapshotを指し、Documentを編集しても変わりません。sequenceは1実行内の更新ごとに増えます。stateはqueued→running→finished、またはホスト障害時のfailedです。

startedは開始したノード数、finishedは全終端状態の件数です。completed/cached/failed/blocked/cancelledに内訳を持ちます。Cached等はstartedへ加算しないため、started−finishedを実行中ノード数として使わないでください。ホスト障害時は未完了ノードが残る場合があります。

state=finished、reason=completedでもノードが失敗していることがあります。成功はreasonがcompletedで、failed/blocked/cancelledが0で、finished=totalであることを確認します。cancel_requestedは明示的なキャンセル要求を示します。実行期限超過はreason=deadline_exceededで確認します。技術的なmessageはログに使い、error_codeはUIで翻訳します。

## Tauri API

`unge_tauri::handler()` は次の4命令を含みます。呼び出し元Viewを検証し、実行開始のexpected_revision照合とsnapshot取得はEngineの同じlock内で行います。実行はworkerへ渡し、実行中はDocument lockを保持しません。

| 命令 | TypeScriptクライアント | 応答 |
|---|---|---|
| start_execution | startExecution(revision) | queued時点のRunSummary |
| current_execution | currentExecution() | アクティブなRunSummaryまたはnull |
| execution_status | executionStatus(id) | 保持中のRunSummary |
| cancel_execution | cancelExecution(id) | キャンセル要求時点のRunSummary |

開始後のDocument編集を許します。キャンセルは実行IDで行い、現在のDocument revisionへの一致は要求しません。同じDocumentの登録済みViewから操作でき、Documentと履歴を更新しません。別Documentの実行は拒否します。実行未設定のEngineはexecution_unavailableです。

開始したウインドウへ `unge://execution` を送ります。ノードごとのイベントをRunSummaryへ集約し、通常通知は最大約10回/秒、終端通知は即時送信します。イベント配送・WebViewの消費速度を保証するものではありません。Rust内では最新版のみ保持し、通知欠損は照会APIで回復します。他ViewはcurrentExecution/既知IDのexecutionStatusを使います。

```ts
const queued = await client.startExecution(displayedRevision);
let latest = await client.executionStatus(queued.id);
// listenでRunSummaryを受け取り、同じIDかつsequenceが大きい時だけ更新する。
// 通知はstartExecutionの応答より先に届くことがあるので、応答後に照会する。
await client.cancelExecution(latest.id);
latest = await client.executionStatus(latest.id); // 停止完了はfinished/failedまで確認
```

custom invoke handlerにはdispatch/inspect/appearanceに加えstart_execution/current_execution/execution_status/cancel_executionを登録します。RunSummary、RunState、3言語の表示文言・エラー辞書をTypeScriptへ追加しました。デスクトップ例は実行・キャンセルボタンと進捗表示を備えます。1秒ごととfocus時に状態を照会し、通知欠損から回復します。実行snapshotと現在のrevisionが異なる場合は編集後であることを表示します。

## ACXとの共有

```rust
let provider = unge_acx::Provider::new(
    Arc::new(engine.clone()), registry.clone(), policy,
).with_execution(execution.clone(), observer)?;
```

RegistryはサービスとProviderで同じArcを渡します。異なるArcはregistry_mismatchです。ホストのobserverにRunSummaryを渡し、デスクトップ例ではcontrolsウインドウへ同じ集約通知を送ります。Tauriの `execution_observer(window)` を再利用できます。

ACXの承認済みrunは同じ予約・Schedulerを使うため、UIとの二重実行を防ぎます。既存の同期execute/Report/Receipt形式を維持します。キャンセル結果やexecution_busyも失敗Receiptに記録し、同じcommitを再送しても再実行しません。busy解消後に同じcommitを自動再試行してはいけません。新しい実行には再観測・新しいpreflight/承認/commitが必要です。

同期executeは維持します。任意の [ACX_ASYNC_JOBS.md](ACX_ASYNC_JOBS.md) 拡張を使うと、AIが同じpipeからrun_start/run_status/run_cancelで操作できます。必要な仕様・Schema・適合テストはacx側にも追加しています。

## English

RunService is owned by the Rust host, independent of Tauri/GPU/ACX. Share one service between UI and AI. It accepts one active run, retaining 32 completed metadata records by default (at least one even when configured as zero). Default snapshot limits are 10,000 nodes, 30,000 edges and 16 MiB of serialized data; these are admission limits, not verified performance claims or process memory limits. Reports and event histories are not retained.

prepare validates and freezes a snapshot; the host checks its document identity and revision. PreparedRun owns the reservation and is executed on a worker. Dropping unused work releases it. Panic handling releases the slot and clears cached outputs; external effects are not rolled back. current/inspect/cancel expose shared state. Cancellation is idempotent and never edits document history. RunSummary identifies the frozen document/revision, with per-run monotonic sequence and terminal counters. Finished traversal can contain failed nodes; check reason and counters for success. Deadline support is available through execute_until; built-in Tauri/ACX calls do not set a deadline.

Tauri provides start_execution, current_execution, execution_status and cancel_execution, with matching TypeScript methods. Registered views of the same document can inspect and cancel a run after the document is edited. The initiating window receives aggregate unge://execution notifications, throttled to about 10 per second with immediate terminal delivery. Events are advisory; query status after start and after lost notifications, and accept only newer sequences for the same ID. Rust retains latest state rather than a notification backlog. The sample includes run/cancel controls and progress, polls every second and on focus, and labels runs whose snapshot revision differs from the edited document.

Provider::with_execution shares the exact Registry Arc and service. Approved ACX runs preserve existing synchronous execute and Receipt formats; failed/busy/cancelled outcomes are recorded and commit replay does not rerun work. Host/Tauri cancellation is supported. Synchronous execute still blocks the pipe; the optional [async job extension](ACX_ASYNC_JOBS.md#english) enables run_start/run_status/run_cancel without changing existing execute/Receipt forms.

## 简体中文

RunService由Rust主机拥有，不依赖Tauri/GPU/ACX。UI与AI应共享同一服务，最多同时运行1个任务，默认保留最近32条完成概要（配置为0时仍至少保留1条）。默认snapshot限制为10,000节点、30,000边、16 MiB序列化数据；这些是接收限制，不是性能保证或进程内存上限。不保留完整Report及事件历史。

prepare验证并冻结snapshot，由主机核对文档ID与revision。PreparedRun持有执行预约，在worker中执行；未执行就drop会释放预约。panic处理释放执行槽并清除缓存，外部副作用不会回滚。current/inspect/cancel提供共享状态，取消操作幂等且不修改文档历史。RunSummary保存原文档/revision、单调sequence及终端计数。遍历结束可能包含失败节点，应检查reason和计数判断成功。execute_until支持主机期限Future；默认Tauri/ACX调用不设置期限。

Tauri提供start_execution、current_execution、execution_status、cancel_execution及对应TypeScript方法。同一文档的注册View可以在文档修改后继续查询、取消原run。启动窗口收到unge://execution概要通知，通常最多约每秒10次，终端状态立即发送。事件只作提示；启动返回后及通知丢失后应查询状态，只接受同一ID的更高sequence。Rust只保存最新状态，不保存通知队列。样例包含执行、取消按钮和进度，每秒及focus时查询状态，文档修改后显示snapshot版本差异。

Provider::with_execution使用相同Registry Arc与服务。获批ACX执行保持既有同步execute及Receipt格式，失败、busy、取消结果写入Receipt，重放commit不会重跑。主机/Tauri可以取消共享任务。同步execute仍会阻塞pipe；可选[异步任务扩展](ACX_ASYNC_JOBS.md#简体中文)支持run_start/run_status/run_cancel，保持原execute/Receipt格式。
