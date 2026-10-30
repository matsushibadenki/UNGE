# ACX retention / 保持運用 / 保留管理

## 日本語

期限切れの未実行preflightが保持枠を埋め続ける問題を解消しました。新しい`preflight`の受付前に、`now >= expiresAt`かつ実行を一度も開始していないセッションだけを回収します。承認済み・commit済みでも実行を開始していなければ対象です。`Policy::max_sessions`（既定128）の上限は維持します。期限前の確認は追い出しません。

一度でも実行を開始したセッションは、成功・失敗・結果不明・非同期実行中・結果確定待ち・実行完了・回復済みのいずれでも残します。元の結果、実行/回復Receipt、commit IDとjobを期限後も保持し、再送で実行やUndoを重ねません。実行済み記録で満杯の場合は従来どおり`capacity_exceeded`を返します。実行済み記録の永続化と安全なProvider切り替えは、[任意の永続ジャーナル](ACX_DURABILITY.md)で有効にできます。ログの圧縮・退避運用は未実装です。

回収したpreflightへのauthorize/commitは`not_found`、古いcommitへのexecuteは`unauthorized`になります。未回収の期限切れgrantは従来の`expired`です。クライアントは新しい観測・事前確認・承認を取得してください。破棄した承認を新しい確認へ流用しません。時計が巻き戻っても破棄したIDは復活しません。

### Rustホストで使う

```rust,ignore
// 同じProviderを所有するホストのworkerで呼ぶ。JSON Lines命令ではない。
let usage = provider.retention_usage();
if usage.expired_unstarted > 0 {
    let removed = provider.prune_expired_preflights();
    // removedをホストの監視へ記録できる。
}
```

`RetentionUsage`の`sessions`/`max_sessions`は現在の保持件数/設定上限、`expired_unstarted`はその時点で回収できる件数、`started`は実行開始済みの保持件数、`receipts`/`jobs`は各記録数です。`active_jobs`はworker実行中だけでなく、workerが終了して結果をProviderがまだ回収していないjobも含みます。これはヒープのバイト容量計測ではありません。

照会は記録やDocumentを変更せず、job結果も回収しません。明示的なpruneは回収件数を返し、繰り返しても追加削除しません。Document/revision/Undoを変更せず、実行やキャンセルを発行しません。idle時に回収したいホストは同じProviderの所有スレッドで呼びます。バックグラウンドのタイマーは追加していません。`dispatch`は従来どおり先にjob結果を確定します。

Rustホスト専用の追加APIです。Graph/Document、Tauri IPC、ACXメソッド・JSON Schema・TypeScript wire型を変更していません。新しいUIエラーcodeも追加していません。外部プラグインやAIにPolicyの変更権限を渡しません。

### 検証範囲

期限直前/境界、有効な確認との混在、未承認・承認済み・未実行commit、古い承認の流用拒否、時計の巻き戻り、模擬時計で256回の期限切れ再受付、元Document/revisionの維持、実行/回復の結果とReceipt再送、期限をまたいだ実行中jobとキャンセル/終了後の再送をRustで検証します。プロセス再起動後の永続保証や別実装Providerとの相互運用は今回の完成範囲に含みません。

### 2026-10-25の結果

macOS、Rust/Cargo 1.99.0で検証。`cargo test --workspace --locked`とall-features版で各158件成功（ACXは35件）。`cargo clippy --workspace --all-targets --locked -- -D warnings`とall-features版、fmt/check、TypeScript strict検査が成功しました。17契約の生成差分はなく、PythonのSchema6件と実stdioのACX相互運用5件が成功しています。追加ケースは6件で、既存の失敗実行再送ケースも期限後の保持検証へ拡張しました。

今回GPUコードは変更しておらず、通常テストのGPU4件はignoredのままです。既存の依存更新差分を保持しています。Cargoのblock 0.1.6将来Rust互換性警告は依存更新の残項目として残ります。

## English

Before admitting a new preflight, the provider reclaims only sessions whose grant has expired (`now >= expiresAt`) and whose execution never started. This includes authorized/committed but never executed grants. Live grants remain retained. The configured maximum session count (default 128) still applies. Sessions that started execution, including failed/uncertain attempts, active or finalized jobs, and recovered edits, retain their replay results and receipts for the provider lifetime. Saturation by started sessions still returns `capacity_exceeded`.

Removed preflight IDs return `not_found` on authorize/commit; removed commit IDs return `unauthorized` on execute. An expired grant that has not been reclaimed still returns `expired`. Acquire a fresh observation/preflight/approval; never transfer old authority. Clock rollback cannot revive removed IDs.

The Rust-only `retention_usage()` reports session limits/counts, expired unstarted grants, started sessions, receipts, jobs and active jobs. Active includes terminal results not yet collected. It does not collect results or mutate records. `prune_expired_preflights()` returns the number removed and is idempotent. Neither method edits the Document/history/revision, starts work or cancels jobs. Hosts may invoke pruning on the same owner worker while idle; no timer was added. These are record counts, not heap-byte limits. IPC, JSON Schemas and wire types are unchanged. Validation on macOS/Rust 1.99.0 passes 158 workspace tests in each feature configuration (35 ACX tests), strict Clippy, fmt, strict TypeScript, 17 generated contracts, six Python schema tests and five real-stdio interop tests. Six new regression cases cover reclamation and replay; the existing failed-run case also checks post-expiry retention. Durable replay and safe host rotation are now available through the optional [journal](ACX_DURABILITY.md). Compaction/archiving and independent-provider interoperability remain unfinished.

## 简体中文

Provider在接受新preflight前，仅回收已过期（`now >= expiresAt`）且从未开始执行的会话，包括已授权／已commit但未执行的grant。未过期grant不被逐出。最大保留数量（默认128）不变。已开始执行的会话，包括失败／结果不明、执行中或已完成job、已恢复编辑，均保留结果和Receipt用于重放。若已执行记录占满容量，仍返回`capacity_exceeded`。

已删除preflight的authorize/commit返回`not_found`，旧commit的execute返回`unauthorized`；尚未回收的过期grant仍返回`expired`。客户端必须重新观察、预检并获取批准，不能复用旧授权。时钟回退不会恢复已删除ID。

仅Rust宿主新增`retention_usage()`和`prune_expired_preflights()`。前者报告会话／上限、已过期未开始、已开始、Receipt、job及active job数量；active包括worker已结束但结果尚未收集的job。查询不修改记录、不收集结果；prune返回删除数量且可重复调用。两者均不修改Document／revision／Undo，不启动或取消任务。宿主可在同一Provider所属worker空闲时调用；未新增后台计时器。这些是记录数量，不是堆内存字节上限。IPC、JSON Schema及wire类型保持不变。macOS/Rust 1.99.0下，两种feature配置各158项workspace测试（含35项ACX测试）、严格Clippy、fmt、strict TypeScript、17种生成契约、6项Python Schema及5项实际stdio互操作测试通过。新增6项回归覆盖回收与重放，原失败执行用例也扩展至过期后保留验证。持久重放和安全切换已由可选[日志](ACX_DURABILITY.md)提供，压缩／归档及独立实现互操作仍待实现。


`retention_usage`は現在epochのメモリー内記録、`journal_usage`は全epochの永続記録を数えます。既定のメモリー内Providerは切り替えられません。[永続再送の有効化と復元範囲](ACX_DURABILITY.md)。
