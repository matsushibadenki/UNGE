# 実行進捗・期限・キャンセル

`unge-executor` は特定のUI・タイマーランタイムに依存せず、Rustホストへ小さな進捗を通知します。既存の `Scheduler::run` はReportを返す署名を維持します。

```rust
use unge_executor::{Cancellation, Scheduler, StopReason};

let token = Cancellation::default(); // runごとに新しいtoken
let outcome = scheduler.run_with_progress(
    snapshot.graph(), &registry, token.clone(),
    |event| { /* 非ブロッキングのチャネル送信、ホスト状態の更新等 */ },
).await?;
if outcome.reason == StopReason::Completed {
    // 遍歴が終了。成功判定はreport内の各statusを確認する。
}
let report = outcome.report;
```

実行サンプル `examples/headless` はイベントをstderrへJSONで出力し、従来のReportはstdoutへ返します。

## イベント契約

| kind | 内容 |
|---|---|
| started | total（snapshotの全ノード数） |
| node_started | node。並列枠に入り実行を開始する時点。待ち行列への追加時には送らない |
| node_finished | node、status、finished、total |
| finished | reason、finished、total |

Graph/Registryの事前検証に失敗するとErrを返し、イベントを送りません。検証成功時はstartedとfinishedを各1回、各ノードのnode_finishedを1回送ります。finished件数は終端状態に達した数で、Cached/Failed/Blocked/Cancelledも含み、単調増加します。空グラフはtotal=0の開始/終了です。

Cached/Blocked/未開始のCancelledにはnode_startedを送りません。node_startedのcallbackでキャンセルした場合、executorを呼ばずに終了することがあります。並列処理の終了順は完了順です。callbackはSchedulerタスク上で同期的に呼ばれるため、短く非ブロッキングにし、panicさせないでください。通知はノード境界であり、ノード内部の割合は提供しません。

イベントには出力、properties、大容量リソース、技術的エラーを含めません。TypeScriptの `ProgressEvent` と3言語の `executionStatusMessages` / `executionStopMessages` を同梱しています。Tauriへ転送する場合はホストがrun IDとexecution revisionを付け、共有Documentの編集状態と実行snapshotを区別し、通知キューに容量上限を設けます。[EXECUTION_SERVICE.md](EXECUTION_SERVICE.md) の共有サービスを設定すると、Tauriの集約通知とACXのホストobserverへ接続できます。

## run全体の期限

`run_with_deadline(graph, registry, token, deadline_future, observer)` は、ホストが渡す `Future<Output=()>` がReadyになったとき `DeadlineExceeded` を返します。ホストの非同期タイマーやジョブ制御Futureを使えるため、executorへTokio等の依存を追加しません。期限は各ノード単位ではなくrun全体です。Futureは検証後に最初にpollされます。実際の起算時点はホストのFutureが決めます。

- `Completed` は通常の遍歴終了であり、ノードの失敗があってもこのreasonになります。
- `Cancelled` は外部tokenのキャンセル、`DeadlineExceeded` は期限Futureの完了です。同時にReadyならキャンセルを優先します。
- 期限切れでも共有tokenをキャンセルし、実行中・未開始の未完了ノードをCancelledとしてReportへ記録します。すでに記録した結果とキャッシュを保持し、未完了結果は保存しません。
- キャッシュヒット前と並列Futureのpoll前に停止を確認します。準備処理や長い同期callback、yieldしないexecutorを割り込んで停止する機能ではありません。

タイマーを含まない実例として、テストはoneshot通知を期限Futureへ渡し、1ノードが完了したあとPendingのexecutorが破棄されることを検証します。期限のない実行は `run_with_progress`、通知の不要な実行は `run` を使います。ノードごとのHTTP/RPCタイムアウトはNodeExecutor内で設定してください。

## キャンセルと副作用

`Cancellation::cancelled().await` はキャンセル時に全waiterを起こします。`cancel()` は複数回呼べ、キャンセルは解除できません。同じtokenを共有したrunは一緒に停止するため、独立した実行には新しいtokenを使います。

Schedulerは停止時にtokenをキャンセルしたあと未完了executor Futureを破棄します。Rust内でdetachしたworker、実行済みGPUコマンド、外部HTTP処理、課金等が自動的に停止・ロールバックされる保証はありません。workerへtokenを渡し、結果公開前に確認し、キャンセル時の資源解放と外部操作の回復をホストで設計してください。実行Future自体をホストがdropする場合はReport/finishedイベントを返せないので、結果が必要ならtokenをキャンセルしてrunをawaitしてください。

ReportとACXのwire形式は維持します。DeadlineExceededは新しいNode statusではなくRustのExecutionOutcomeとfinishedイベントのreasonです。同期ACX executeに期限設定は追加していません。任意の [非同期job拡張](ACX_ASYNC_JOBS.md) はcommit IDで照会・キャンセルできます。

## English

`run_with_progress` returns `ExecutionOutcome { report, reason }` and synchronously calls a short, nonblocking observer. `run` retains its Report return type. After successful validation, events are started, node_started when a concurrency slot begins work, one node_finished per node, and finished. Validation errors emit nothing. Cached, blocked and never-started cancelled nodes do not emit node_started. Terminal counts include all statuses and increase monotonically. Completion order may vary. Empty graphs emit start/finish with zero counts.

`run_with_deadline` accepts a host `Future<Output=()>` for the entire run, avoiding timer-runtime dependencies. A ready deadline produces deadline_exceeded; cancellation takes priority when both are ready. The token is cancelled before pending executor futures are dropped. Remaining nodes become Cancelled; recorded results and cache entries remain. Completed means traversal ended, including runs with failed nodes. Deadline timing is defined by the host future, first polled after validation. Synchronous preparation, callbacks and executors that do not yield cannot be preempted.

`Cancellation::cancelled().await` wakes all waiters, including a scheduler blocked on an executor that never wakes. Cancellation is permanent; use fresh tokens for independent runs. Detached workers, submitted GPU commands and external side effects may continue after future drop. Hosts must propagate cancellation and manage resource cleanup and recovery. Cancel the token and await the run to receive a terminal report; directly dropping the run cannot emit its final event.

Events contain metadata only. TypeScript types and English/Japanese/Simplified Chinese status labels are included. When forwarding events, hosts should add a run ID and snapshot revision and bound notification queues. See [execution service](EXECUTION_SERVICE.md#english) for Tauri aggregate notifications and cancellation commands and the ACX host observer. Existing Report and ACX wire formats remain unchanged.

## 简体中文

`run_with_progress` 返回 `ExecutionOutcome { report, reason }`，并同步调用简短、非阻塞的通知函数。`run` 仍返回Report。验证通过后发送started，在进入并发执行槽时发送node_started，每个节点发送一次node_finished，最后发送finished。验证错误不发送事件。缓存命中、阻塞和未开始的取消节点不发送node_started。终端计数包含所有状态，单调增加；并发完成顺序可能变化。空图发送计数为0的开始与结束事件。

`run_with_deadline` 接收主机提供的 `Future<Output=()>`，控制整个run的期限，不依赖特定计时运行时。期限Future就绪时reason为deadline_exceeded，同时取消就绪时优先取消。先取消token，再丢弃未完成的executor Future；剩余节点记为Cancelled，已记录结果与缓存保留。Completed只表示遍历结束，也可能包含失败节点。期限起算由主机Future决定，首次poll在验证之后。同步准备、通知函数和不yield的executor无法被抢占。

`Cancellation::cancelled().await` 会唤醒全部等待者，也能停止正在等待永不唤醒executor的Scheduler。取消不能撤销，独立run应使用新token。独立worker、已提交GPU命令及外部副作用可能继续执行，主机必须传递取消信号并管理资源清理和恢复。需要终端Report时应取消token后await run；直接丢弃run无法发送结束事件。

事件只包含元数据，并附带TypeScript类型及英、日、简体中文状态文案。主机转发时应添加run ID、执行snapshot的revision，并限制通知队列容量。Tauri概要通知、取消命令及ACX主机通知见[共享服务](EXECUTION_SERVICE.md#简体中文)。既有Report与ACX通信格式保持不变。
