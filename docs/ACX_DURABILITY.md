# ACX durable replay / 永続再送 / 持久重放

## 日本語

Rustホストが`Provider::with_journal`を指定すると、実行開始と完了結果・Receiptを追記型ジャーナルへ保存します。同じ`commitId`のexecute再送は、再起動後も保存済みの元の結果を返し、編集やexecutorをもう一度実行しません。成功・失敗の両方が対象です。記録済みのRecover結果も元のauthorizationを検証して返し、Undoを重ねません。未指定時は従来のメモリー内モードです。discoveryの`extensions.org.unge.node-graph.replay.durable`で区別できます。

### 開始・完了・途中停止

1. ホストのDocument/revision/ポリシーを確認し、開始記録を`sync_all`で確定してから編集・実行・Undoへ進みます。
2. 完了結果の正確な`resultJson`とReceiptを一つの完了レコードに保存・同期してから応答します。Receipt ID、provider、requestHash、resultHash、結果JSONは再送時も不変です。
3. 開始だけが残ったcommitは`execution_uncertain`です。実行前の停止か、実行済みだが結果保存前の停止かを推測せず、再実行しません。新しいcommitを自動発行して再試行することもしないでください。

これは同じcommitの**試行を重ねない**保証です。任意のGraphHost、外部API、ファイルやDBの効果とログを単一トランザクションにする仕組みではなく、途中停止した効果の成否を確定できるexactly-once処理ではありません。完了応答を受信した結果は保存済みですが、途中停止時にはReceiptが未確定の場合があります。

Document、revision、Undo履歴、GPU資源をこのログから復元しません。ホストが同じDocument IDの保存状態を復元してから接続してください。異なるDocument IDへの接続は拒否します。再起動／切り替え前の未実行grantは復元せず、新しい観測・事前確認・承認が必要です。過去epochの編集を新たにRecoverすることも拒否します。既に完了したRecoverの再送のみ可能です。

### Rustホストでの有効化と切り替え

```rust,ignore
use unge_acx::{Provider, JournalOptions};

let mut provider = Provider::new(host, registry, policy)
    .with_execution(shared_run_service, observer)?
    .with_journal("private/acx-replay.jsonl", JournalOptions::default())?;

// 同じ所有workerで、実行中jobがなくなってから呼ぶ。
let next_provider_id = provider.rotate_session()?;
let durable_counts = provider.journal_usage();
```

保存先の親ディレクトリを事前に用意し、AIのリクエストからパスや容量を指定させないでください。Tauriでは復元済みの共有Engineを同じGraphHostとして渡し、Documentを別に複製しません。`with_journal`は最初のリクエスト前に指定します。

`rotate_session`は完了待ちjobを先に確定し、実行中／未確定のローカル操作があれば`execution_busy`で拒否します。新しいprovider IDをログへ確定してからメモリー内セッション・grant・jobをクリアします。Document/revisionは変更せず、旧commitの結果・Receipt・確定済みjob概要は同じログから取得できます。切り替え後のdiscoveryは新しいprovider IDを返しますが、過去Receiptのproviderは変更しません。メモリー内モードでの切り替えは`persistence_required`です。

別プロセスへの引き継ぎは、旧所有者を終了してから同じログを開きます。ファイルの単一writerロックが二重所有を拒否します。古いProviderを稼働させたまま新しいログへ移動して実行を継続する運用にはしないでください。実行中の切り替えには上記APIを使います。

### stdio例

```sh
cargo build --locked -p unge-acx-provider
# project.graph.jsonはホストが保存した同じDocument IDのファイル。
target/debug/unge-acx-provider --document project.graph.json --journal private/replay.jsonl
# 旧プロセス終了後、新しい受付epochへ切り替える。
target/debug/unge-acx-provider --document project.graph.json --journal private/replay.jsonl --rotate-session
```

この例は`--document`を読み込みますが、編集後のDocumentを自動保存しません。Documentの保存を別途実装したホストへ取り込むための例です。`--journal`は`--document`が必須で、`--journal-max-bytes N`で上限を指定できます。Pythonの`Agent(..., extra_args=[...])`から同じホスト引数を渡せます。rotateやストレージ管理のACXメソッドは追加していません。

### 容量と保存障害

- 上限は既定256MiB、設定の最小値は1KiBです。開始前に完了レコード用の空きを予約し、足りなければ`storage_full`として効果を開始しません。epoch切り替えは履歴を削除せず、ディスク容量不足の解消にはなりません。
- `journal_usage`は同期済みログのバイト数／上限と全epochのcommit・Receipt件数です。`retention_usage`は現在epochのメモリー内記録です。再起動時は全ログを検証・再同期してインデックスを再構築するため、長期運用でのメモリー／起動時間や圧縮・退避は別途設計が必要です。ヒープのバイト数制限ではありません。
- 1レコードのpayload上限は1MiBです。バージョン、順序、hash chain、結果とReceiptの整合性を検証し、未知形式・改変・切れた末尾は`storage_corrupt`で拒否します。勝手な切り詰めやメモリー内モードへのフォールバックはしません。hash chainは署名や悪意ある保存先管理者への認証ではありません。
- 書き込み・同期失敗は`storage_unavailable`。そのProviderで以後の操作を停止し、既存記録を保全します。保存先を確認してから再開してください。途中停止のcommitは効果を自動再試行しません。
- Unixでは新規ファイルを0600にし、新規ファイルと親ディレクトリを同期します。保存ディレクトリと既存ファイルのアクセス権はホストが管理します。ロック／同期が機能する信頼済みローカルfilesystem向けです。NFSや複数マシンの協調writerは対応範囲外です。

新しいエラーcodeは`storage_locked`、`storage_corrupt`、`storage_unavailable`、`storage_full`、`persistence_required`。英語・日本語・简体中文のUI文言を`acx-i18n.ts`へ追加しています。新しいRPCや必須wireフィールドはなく、discoveryに任意のreplay情報を加えています。

### 検証

Rustの実プロセスを開始記録後／編集適用後／完了記録後／非同期job開始後／Undo適用後で強制終了し、再開後に同じ操作を繰り返さないことを検証します。保存済み結果・Receipt・Recover・job概要の再送、失敗executorの呼び出し回数、別プロセスのロック競合、容量不足、書込失敗、checksum・未知version・壊れた末尾・Document不一致も確認します。Pythonの実stdioテストでも再起動とepoch切り替え後のReceipt/hash/schema一致を確認します。補助プロセステストは親テストから実際に起動します。

標準ライブラリのFileロックAPIを使うためRust 1.89以上が必要です（[公式API](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock)）。実際の最小ツールチェーン試験は未実施です。Windows/Linux、電源断、ネットワークfilesystemはこのmacOS環境で未検証です。GPUコードは今回変更していません。

## English

Enable `Provider::with_journal(path, JournalOptions::default())` before serving requests. The host must restore the same Document ID. Completed execute results and original receipts survive restart and `rotate_session()`, including failed executions. Completed Recover replies require the original authorization and never undo again. The optional discovery replay extension advertises durability; the default remains in-memory mode.

Every attempt is fsynced before effects, and a completion record containing exact result bytes and its receipt is fsynced before replying. A start without a confirmed finish returns `execution_uncertain`; never retry it automatically under the same or a new commit. This prevents repeated attempts by commit ID, not an atomic exactly-once transaction with arbitrary external effects. An interrupted attempt may have no confirmed Receipt. Document state, revision, Undo and GPU resources are not checkpointed. Unstarted approvals are not restored; new rollback attempts from archived epochs are rejected. Completed rollback replays remain available.

Host-only rotation finalizes completed jobs, refuses active/unfinalized operations, persists a new provider ID and clears hot sessions only. Historical receipts keep their original IDs, provider and hashes. A single-writer file lock prevents concurrent owners. Handover reopens the same journal after the previous owner stops; never bypass it by switching to an empty log. The stdio example accepts `--document`, `--journal`, `--journal-max-bytes` and `--rotate-session`. It does not automatically save edited Documents. Journal/rotation control is not an agent RPC.

Default file budget is 256MiB (minimum 1KiB). Completion space is reserved before effects. Rotation never erases history and does not cure disk exhaustion. The entire journal is validated, synced and indexed in memory at open; compaction/archiving and long-running memory/startup budgets remain future work. Frames are size-limited, versioned and hash-chained; malformed/truncated storage fails closed without repair or volatile fallback. Hash chains are integrity checks, not authentication. I/O failures stop further operations. Unix creation uses mode 0600 and syncs the file and directory; trusted local filesystems with working locks/fsync are required. Network filesystems and multi-machine writers are not supported.

Tests terminate subprocesses at start/effect/finish, async start and rollback boundaries, reopen and verify no repeated edit/executor/Undo. They cover immutable replay, active-job rotation refusal, independent writer contention, disk limits, write failures, invalid history and wrong Documents. Python stdio tests verify restart/rotation, schema and hashes. New storage errors have English/Japanese/Simplified Chinese messages. The standard File locking API requires Rust 1.89 or later ([official API](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock)); the minimum toolchain has not been tested. Windows/Linux, power-loss and network-filesystem validation remain unverified.

## 简体中文

在接收请求前，通过`Provider::with_journal(path, JournalOptions::default())`启用持久日志。宿主必须恢复相同Document ID。已完成的execute结果及原Receipt（包括失败结果）在重启和`rotate_session()`后仍可重放。已完成Recover需要原授权且不会再次Undo。discovery的可选replay扩展标明持久模式；默认仍为内存模式。

开始记录先同步落盘，再执行操作；完成记录包含准确的结果JSON和Receipt，先同步再响应。仅有开始记录时返回`execution_uncertain`，不能用原commit或自动创建新commit重试。这保证同一commit不重复尝试，并非与任意外部效果组成exactly-once事务；中断时可能没有确定的Receipt。日志不保存Document／revision／Undo／GPU资源。未执行授权不会恢复，旧epoch的新回滚请求被拒绝；已完成回滚的重放仍可使用。

切换仅由宿主发起：先收集已完成job，拒绝执行中／未完成的本地操作，持久化新provider ID后才清理当前会话。历史Receipt保留原ID、provider和hash。单writer文件锁拒绝并发所有者；旧所有者停止后打开同一日志进行交接，不能通过切到空日志绕过旧记录。stdio示例支持`--document`、`--journal`、`--journal-max-bytes`及`--rotate-session`，但不自动保存编辑后的Document。日志管理和切换不是AI可调用的RPC。

默认文件上限256MiB，最小配置1KiB。执行前预留完成记录空间；切换不删除历史，也不能解决磁盘不足。启动时验证并重新同步日志，在内存重建全日志索引，压缩／归档及长期内存／启动时间管理仍待实现。记录有大小限制、版本和hash chain；损坏或截断时拒绝打开，不擅自修复、不退回内存模式。hash chain不是身份认证。I/O失败后停止后续操作。Unix新文件权限0600并同步文件和目录，要求可信本地filesystem支持锁及fsync；不支持网络filesystem或多机writer。

测试在开始／效果应用／完成记录／异步开始／Undo应用后结束子进程，重启验证不重复操作；也覆盖原结果和Receipt重放、活跃job切换拒绝、独立进程锁冲突、容量不足、写入失败、损坏日志及Document不匹配。Python实际stdio检查重启／切换后的Schema和hash。新存储错误提供英／日／简中消息。标准File锁API需要Rust 1.89以上（[官方API](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock)），最低工具链尚未实测。Windows/Linux、断电及网络filesystem尚未验证。


### 2026-10-25 verification

- 🟢 [Done] macOS / Rust 1.99.1: 標準/all-featuresのworkspace各169件、厳格Clippy（両構成・全target）、fmt、TypeScript 7.0.2 strictが成功。
- 🟢 [Done] 17契約の生成差分なし、Python Schema6件、実stdio6件が成功。ACX単体は46件成功。追加した永続化回帰は11件。2つのignored補助関数は親テストから実プロセスとして自動実行するfixtureで、未検証項目ではない。
- 🟠 [Next] Document/Undo保存とのトランザクション連携、長期ログの圧縮／退避、独立Providerとの相互運用。
- ⭕️ [Pending] Windows/Linux、実電源断、最低Rust版、filesystem別の実機検証。NFS／複数マシンwriterは未対応。

English: macOS/Rust 1.99.1 passes 169 workspace tests in each configuration, strict Clippy, fmt, strict TypeScript, 17 unchanged generated contracts, six schema tests and six real-stdio tests. The ACX package passes 46 tests, including 11 new durability regressions. Two ignored fixture entry points run as subprocesses under their parent tests. Checkpoint transactions, archive/compaction and independent-provider interop remain unfinished; other-platform/power-loss/minimum-toolchain validation remains pending.

简体中文：macOS/Rust 1.99.1下，两种配置各169项workspace测试、严格Clippy、fmt、strict TypeScript、17种生成契约无差异、6项Schema及6项实际stdio测试通过。ACX共46项测试成功，其中11项为新增持久化回归。两个ignored辅助入口由父测试自动启动为子进程。checkpoint事务、归档／压缩、独立实现互操作尚未完成；其他平台／断电／最低工具链验证待完成。
