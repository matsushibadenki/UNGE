# Paired log prefixes / ログ世代の相互検証 / 日志前缀相互验证

## 日本語

`PersistentHost`と`Provider::with_journal`を組み合わせると、状態ログとReceiptログが互いの確定済みhash-chain prefixを記録します。Document IDが同じでも、相互確認済みの地点より古い片側のログや、別ペアのログへの差し替えを`storage_corrupt`で拒否します。ファイルを修復・切り詰めたり、メモリーホストへ戻したりしません。CLIの既存`--state-log`で有効になり、新しいRPC／フラグは不要です。[基本の有効化](ACX_EDITOR_PERSISTENCE.md)。

### 記録順序と復元

- 初接続時は、状態ログにReceiptログの初期prefixを同期保存し、Receiptログに状態prefixを記録した後、その確認も状態ログへ同期します。以後は同じ組合せだけを使います。
- Providerは開始guardをReceiptログへ同期保存し、そのprefixを状態ログへ確認保存してから編集／executor／Undoへ進みます。
- 完了レコードには、元の結果・Receiptと同じフレーム内に状態prefixを含めます。次に状態ログへReceiptログの完了prefixを確認保存してから応答します。
- 再起動時と各dispatchの前に両方向を検証します。確認保存が失敗していた場合は、再送応答より先に確認を確定します。状態／Receiptの容量を増やして再開することはできますが、失敗を理由に新しいcommitを自動発行しません。
- Provider epochの切り替えも確認保存するため、確認済み切り替え前へReceiptログだけを戻すことも拒否します。

`PersistentHost.apply/undo/redo`自体が容量不足なら編集前に拒否します。一方、Providerの**完了確認**が容量不足になった場合は、編集とReceiptが既に保存されていることがあります。エラーを「編集が無かった」と解釈せず、容量確保後に同じcommitで結果を再取得してください。保存の成否が不明な開始は従来どおり`execution_uncertain`で、再実行しません。

### 保証範囲と移行

これは相互確認済みprefixの結合であり、2ファイルの単一トランザクションや完全なバックアップ世代manifestではありません。両ログを古い整合した一組へ戻した場合に、それが最新かをローカルログだけで証明することはできません。Providerがまだ確認していない信頼済みホストの直接編集についても、別ログに新しさの証拠があるとは限りません。バックアップは両writerを停止し、seed・両ログ・外部資産を一組として取得／復元します。任意外部API／課金／GPU資源のexactly-once処理は保証しません。

相互確認のない**実行済みReceiptログ**をPersistentHostへ接続することは`persistence_required`で拒否します。既存記録が正しい組合せか証明できないため、自動で結び直しません。記録を保全し、明示的なオフライン移行を待ってください。新規ペアは自動で結合します。まだcommitのないログは開始時結合が可能です。既存のMemoryHost／Tauriホストと非結合Receiptログは従来どおりです。結合済みログをMemoryHostへ接続することは拒否します。PersistentHostを使うProviderは、Receiptログ未接続のままdispatchすることも`persistence_required`で拒否します。新しい結合レコードを理解しない旧バイナリーへ戻すこともできません。

Rustで永続ホストを包む独自GraphHostは`persistence`・`verify_persistence`・`acknowledge_replay`の3メソッドを転送してください。既定の`None`は非結合ホストの宣言です。`LogCheckpoint`／`HostPersistence`は信頼済みRustアダプター向けで、AIから指定させるwire型ではありません。状態ログとReceiptログはprefix hashのインデックスをメモリーに保持します。上限はログのバイト数で、ヒープ使用量の直接制限ではありません。

hash chainは改変検出用であり、保存先を自由に書き換えられる管理者に対する署名／認証ではありません。信頼済みローカルfilesystem、単一writer、機能するlock／fsyncという前提は変わりません。

## English

Combining PersistentHost with the Provider journal now cross-records confirmed hash-chain prefixes. A one-sided rollback behind a witnessed boundary, or a log from another pair sharing the same Document ID, fails closed with `storage_corrupt`. No files are repaired, truncated or replaced with memory-only storage. Existing CLI `--state-log` enables this without new RPCs or flags.

The state log acknowledges the durable start guard before effects. Completion stores the state prefix in the same frame as the original result/receipt, then the state log fsyncs acknowledgement of that completion before replying. Startup and every dispatch verify both directions. Replays must finish a previously failed acknowledgement before returning a result. Epoch rotation is acknowledged too. An acknowledgement-capacity failure can occur after edits/receipts are saved: provision capacity and retrieve the same commit; never assume no effect or automatically mint a new commit. Interrupted starts remain `execution_uncertain`.

This binds witnessed prefixes, not two-file atomicity or a full backup-generation manifest. Restoring both files to an older consistent pair cannot prove freshness; trusted direct edits not yet witnessed by the Provider may lack a cross-log freshness proof. Stop both writers and back up/restore seed, both logs and resources together. External effects/GPU resources are not covered by exactly-once guarantees.

PersistentHost refuses automatic enrollment of executed legacy receipt logs without pairing (`persistence_required`); retain the evidence until explicit offline migration exists. Fresh pairs enroll automatically; non-persistent hosts with unpaired journals retain prior behavior. Paired journals require their persistent adapter and cannot be read by older binaries lacking pairing records. A Provider using PersistentHost also refuses dispatch until its replay journal is attached. Custom GraphHost wrappers must forward persistence, verify_persistence and acknowledge_replay; default None declares an unpaired adapter. Checkpoint types are trusted Rust storage metadata, not agent inputs. Prefix indexes use memory in addition to the bounded log files. Hash chains are not authentication against a malicious storage administrator. Trusted local storage, working locks/fsync and one writer remain required.

## 简体中文

PersistentHost与Provider日志结合后，相互保存已确认的hash-chain前缀。单侧回退至已见证边界之前，或换入相同Document ID的另一配对日志，将以`storage_corrupt`拒绝。不修复、截断或退回内存模式。现有CLI `--state-log`自动启用，无新增RPC或参数。

执行前，状态日志先确认已落盘的开始guard。完成时，将状态前缀与原结果／Receipt保存在同一记录，再由状态日志同步确认完成前缀，之后才响应。启动及每次dispatch均双向验证。之前确认保存失败时，重放结果前必须先完成确认；epoch切换也保存确认。确认阶段容量不足时，编辑／Receipt可能已保存，应扩容后查询同一commit，不能假定无效果或自动创建新commit。中断的开始仍返回`execution_uncertain`。

这是已见证前缀绑定，不是双文件原子事务或完整备份世代manifest。两个文件一起恢复到较旧但一致的配对，无法由本地日志证明其最新性；尚未被Provider见证的可信宿主直接编辑也可能没有跨日志的新鲜度证据。停止两个writer，将seed、两个日志及资源一起备份／恢复。不保证外部效果／GPU资源的exactly-once。

对于已有执行记录、但无配对证据的旧Receipt日志，PersistentHost返回`persistence_required`，不自动重新绑定。保留证据，等待明确的离线迁移。新配对自动绑定；非持久宿主及未配对日志保持原行为。已配对日志必须使用对应持久适配器，不能降级到不识别新记录的旧程序。使用PersistentHost的Provider未接入Receipt日志时也拒绝dispatch。GraphHost包装器必须转发persistence、verify_persistence和acknowledge_replay；默认None表示非配对适配器。Checkpoint类型仅供可信Rust存储层使用，不是AI输入。前缀索引额外占用内存，不构成直接heap上限。hash chain不是针对恶意存储管理员的认证，仍要求可信本地存储、有效锁／fsync及单writer。

## 2026-11-02 verification

- 🟢 [Done] workspace標準／all-features各181件、両構成all-targets Clippy（-D warnings）、fmt／fmt check、TypeScript 7.0.2 strict、17生成契約一致、Schema6件、実stdio7件、headlessの42が成功。新しいRust回帰は5件。
- 🟢 [Done] 両方向の片側巻き戻し・同一Document ID別ペア差し替えを無修復で拒否。正しい一組の復元、epoch巻き戻し、完了確認の容量不足と再送、旧実行済みログの自動結合拒否、両方向の設定抜け拒否を検証。既存の実プロセス中断テストも新しい結合の下で成功。
- 🟠 [Next] 旧実行済みログのオフライン移行、完全な世代manifest、単一トランザクション、圧縮・Tauri接続。
- ⭕️ [Pending] Windows/Linux、実電源断、最低Rust版、長期容量／性能。GPUコードは変更しておらずignored GPU4件は今回未実行。残るignored3件は親テストが実プロセスとして起動する補助入口。既知のblock 0.1.6将来Rust警告は継続。

English: Both workspace configurations pass 181 tests and strict all-targets Clippy. Formatting, strict TypeScript, 17 unchanged contracts, six schema tests, seven real-stdio tests and headless output 42 pass. Five new Rust regressions verify paired-prefix rollback/swap detection, whole-pair restore, rotation rollback, acknowledgement failure/replay and unsafe configurations; existing process-crash tests also pass. Offline legacy migration, full-generation manifests, atomic transactions, compaction and Tauri integration remain unfinished. Other-platform/power-loss/minimum-toolchain/long-running performance verification remains pending.

简体中文：两种workspace配置各181项测试及严格all-targets Clippy、格式、strict TypeScript、17项契约无差异、6项Schema、7项实际stdio及headless输出42通过。新增5项Rust回归覆盖前缀回退／替换、整组恢复、epoch回退、确认失败后重放及不安全配置；既有真实进程中断测试也通过。旧日志离线迁移、完整世代manifest、原子事务、压缩和Tauri接入仍未完成。其他平台／断电／最低工具链／长期性能验证待完成。
