# Durable editor host / 編集状態の永続化 / 编辑状态持久化

## 日本語

`unge_acx::PersistentHost`は、CLI／サーバー向けの任意のRust GraphHostです。初期Documentと成功したCommand・Undo・Redoを保存し、再起動時にDocument、revision、履歴と履歴容量制限を復元します。実行済みcommit・結果・Receiptは引き続き[Providerのジャーナル](ACX_DURABILITY.md)で管理します。2つを組み合わせると、保存済み編集と元Receiptを復元し、同じcommitの再送で編集を重ねません。

```rust,ignore
use std::sync::Arc;
use unge_acx::{PersistentHost, PersistentHostOptions, Provider, JournalOptions};

let host = Arc::new(PersistentHost::open(
    "private/editor.jsonl", initial_document, registry.clone(),
    PersistentHostOptions::default(),
)?);
let provider = Provider::new(host.clone(), registry, policy)
    .with_journal("private/replay.jsonl", JournalOptions::default())?;
// 通常のRust編集も同じhostのapply/undo/redoを通す。
```

```sh
cargo build --locked -p unge-acx-provider
target/debug/unge-acx-provider --document project.graph.json \
  --state-log private/editor.jsonl --journal private/replay.jsonl
```

`--document`は新しい状態ログの初期値です。状態ログが既にあれば、ログ内の初期Documentと編集履歴が正本で、同じDocument IDのseedファイルにある後からの変更を取り込みません。seedファイルは上書きしません。`--state-log`は`--journal`と`--document`を必須とし、片方のログしか存在しない場合は作り直さず起動を拒否します。最初の導入では新しい2ファイルを使います。既存のReceiptログだけに後付けする移行は未実装です。

### 保存と復元の順序

1. 単一mutex内でDocument IDとexpected_revisionを検査する。
2. Editorの一時コピーでCommand／Undo／Redoとホストvalidatorを検証する。
3. 成功する操作だけをversion／sequence／hash chain付きレコードへ追記し、`sync_all`する。
4. 保存に成功した候補を唯一の共有Editorへ公開し、結果を返す。

不正なCommand・競合・容量不足では共有Document、revision、履歴、ログを変更しません。書込み／同期失敗では共有状態を更新せず、当該ホストの読み書きを`storage_unavailable`で停止します。同期結果が不明な場合に、新しいrevisionで自動再試行しないでください。再起動時はログを検証・再同期してから復元した状態を公開します。途中で切れた末尾、未知version、順序／hash不一致、再生できないCommandは`storage_corrupt`として保全し、自動修復やメモリー内へのフォールバックはしません。

復元はcoreのCommandだけを再生します。NodeExecutorや外部APIを呼びません。初期Documentと各遷移に同じ互換なRegistry／決定的validatorを適用します。Definitionの変更や非決定的validatorは復元を拒否する可能性があり、スキーマ移行は別途必要です。Undo/Redoは元の件数・バイト上限と同じ設定で復元します。予算を変えて開くことは`invalid_policy`です。巨大な履歴ステップによる履歴消去や、Undo後の新規編集によるRedo破棄も復元します。

### Receiptとの境界

Providerの開始guard → 状態ログの同期と編集公開 → Receipt完了記録の同期、の順序です。状態の保存後・Receipt確定前で停止した場合、再起動後のDocumentには編集が残り、同じcommitは`execution_uncertain`として再実行しません。保存済みのReceiptがある場合は元の値を返します。信頼済みホストによるUndoは再起動後も可能ですが、旧epochからの**新規ACX Recover**は従来どおり拒否します。完了済みRecoverの再送契約は維持します。

2ファイルを単一トランザクションにするexactly-once処理ではありません。ログの片方だけを巻き戻したり、別の同一Document IDのログと組み替えたりしないでください。CLIは片側の欠損を検出します。[相互prefix検証](ACX_LOG_PAIRING.md)は、確認済み地点より古い片側のログや別ペアのログも拒否します。ただし、整合した一組全体の古さや未確認の直接編集の新しさまでは証明しません。両writerを止めて、seed・両ログ・ホスト資産を一組としてバックアップ／復元します。Rust APIで独自ホストを組む場合、この組合せ管理もホストの責務です。

### 範囲・容量・検証

- 状態ログ上限は既定256MiB、最小1KiB。CLIの`--state-max-bytes N`で設定できます。1フレーム上限16MiB。履歴は既定256ステップ／16MiB。上限超過時は`storage_full`で保存前に拒否します。ログは圧縮／切り詰めません。
- 各編集はDocumentと保持履歴を一時コピーします。通常のMemoryHost／Tauri Engineの編集方式は変えず、この任意のホストだけが保存前の検証にコピーを使います。巨大Documentでの速度やメモリー改善は未達成です。起動時間は保存済みCommand数に比例します。
- 単一writerのファイルロック、Unix新規0600、ファイルと作成時の親ディレクトリ同期を使います。信頼済みローカルfilesystem向けで、署名・暗号化・ネットワークfilesystem・複数マシンwriterは提供しません。
- 新しいRPCやwire型は追加していません。既存のストレージ／競合エラーcodeと3言語辞書を使います。redoはRustホスト専用です。
- Tauri共有Engine、GPU資源、外部資産／API副作用、再生中のexecutor実行状態をこのホストへコピー／復元しません。TauriのDocumentをこれと二重所有しないでください。Tauriへのトランザクション保存接続は次の段階です。

再起動後のDocument／revision／Undo／Redo／履歴上限、同時編集、破損・書込み失敗・容量不足・Document不一致をRustで検証します。実プロセスを「編集保存後・Receipt前」で終了し、状態が残りcommitは再実行されないことも確認します。実stdioでは再起動・Provider切り替え後の状態・元Receipt・hashを照合し、片側ログ欠損も拒否します。Windows/Linux、実電源断、最低Rust版、巨大Document性能は検証待ちです。

## English

`PersistentHost::open(path, initial_document, validator, options)` is an optional standalone Rust GraphHost. It logs the initial Document and successful Commands/Undo/Redo, restoring Document, revision and bounded history on restart without invoking NodeExecutors. Use it with the Provider replay journal. The CLI enables it through `--document seed.json --state-log editor.jsonl --journal replay.jsonl`. Existing editor logs own the restored state; the seed only initializes new logs and checks Document ID. It is not overwritten. Missing one of the two logs is rejected; attaching this option to a pre-existing receipt-only setup is not a migration path.

Under one lock, edits validate a private Editor/history clone, append and fsync a versioned/hash-chained event, then publish the candidate. Invalid edits/conflicts/capacity errors do not publish or append. I/O failure stops subsequent reads and writes. Restart validates, replays and syncs before exposing state; corruption, truncation, incompatible validators or unknown versions fail closed without repair. Keep a compatible deterministic Registry/validator and the original history budgets. Restored eviction, Undo/Redo and redo invalidation match the original editor.

Provider start guards precede editor saves, which precede receipt completion. A crash after the editor save but before receipt completion restores the edit and returns `execution_uncertain` for that commit, never repeats it. Completed results/receipts replay unchanged. Trusted-host Undo works after restart; new ACX Recover attempts from archived epochs remain prohibited. These two files are not one atomic exactly-once transaction. Stop both writers and back up/restore seed, both logs and resources together. The CLI detects missing files. [Cross-log prefix verification](ACX_LOG_PAIRING.md) also rejects one-sided rollback behind witnessed boundaries and logs from another pair; freshness of an entire consistent older pair or unwitnessed direct edits is not proven.

Defaults: 256MiB log (CLI `--state-max-bytes N`), 16MiB frame, 256 history steps/16MiB history. Each edit clones the live Document/history; startup replays all events. Compaction and large-document performance are unfinished. Single writer, fsync and Unix mode 0600 target trusted local storage. No new RPC/wire types; existing localized errors apply. This host does not own a copy of the Tauri Engine, GPU resources or external effects. Tauri persistence integration remains future work. Regression tests cover restart/history, contention, storage failures, corruption and a real-process crash between editor save and receipt, plus stdio restart/rotation and missing-log rejection. Windows/Linux, power loss, minimum toolchains and large-document performance remain unverified.

## 简体中文

`PersistentHost::open(path, initial_document, validator, options)`是可选的独立Rust GraphHost，保存初始Document及成功的Command／Undo／Redo，重启恢复Document、revision和有界历史，不调用NodeExecutor。与Provider重放日志一起使用。CLI参数为`--document seed.json --state-log editor.jsonl --journal replay.jsonl`。已有状态日志是正本；seed只用于初始化新日志及核对Document ID，不会被覆盖。缺少任一日志时拒绝启动；不支持直接为旧Receipt日志补加状态日志。

单锁内先在Editor／历史的临时副本中验证，再追加带version／hash chain的记录并fsync，最后发布候选状态。无效编辑、冲突或容量不足不会发布或追加。I/O失败后停止读取和修改。重启先验证、重放、同步，再提供状态；损坏、截断、不兼容validator或未知版本均拒绝，不自动修复。必须使用兼容且确定性的Registry／validator及原历史预算；保留历史淘汰、Undo／Redo和新编辑使Redo失效的行为。

顺序为Provider开始guard→编辑日志保存→Receipt完成记录。若编辑保存后、Receipt确定前中断，重启保留编辑，同commit返回`execution_uncertain`且不会重做。已完成结果／Receipt原样重放。可信宿主可在重启后Undo，但旧epoch的新ACX Recover仍拒绝。两个文件不是单一exactly-once事务。停止两个writer，将seed、两个日志及资源一起备份／恢复。CLI检测文件缺失，[相互前缀验证](ACX_LOG_PAIRING.md)还拒绝单侧回退至已见证边界之前或换入其他配对日志；无法证明整组一致旧备份或未见证直接编辑的新鲜度。

默认日志256MiB（CLI `--state-max-bytes N`）、单帧16MiB、历史256步／16MiB。每次编辑复制Document及历史，启动重放全部记录；压缩及大Document性能尚未完成。单writer、fsync和Unix新文件0600面向可信本地存储。没有新增RPC／wire类型，使用既有三语错误码。本宿主不持有Tauri Engine的副本，不恢复GPU资源或外部效果；Tauri持久化接入仍待实现。回归覆盖重启／历史、并发、存储失败、损坏、编辑保存与Receipt之间的真实进程中断，以及stdio重启／切换／缺失日志拒绝。Windows/Linux、断电、最低工具链及大Document性能尚未验证。

## 2026-11-02 verification

- 🟢 [Done] workspace標準／all-features各176件、両構成all-targets Clippy（-D warnings）、fmt、17契約一致、Schema6件、実stdio7件、headlessの42が成功。新規Rust回帰7件。ignoredの補助入口は親テストから実プロセスとして実行する。
- 🟠 [Next] Tauri共有Engineの保存接続、状態／Receiptの単一トランザクション・完全なバックアップ世代manifest、ログ圧縮。
- ⭕️ [Pending] Windows/Linux、実電源断、最低Rust版、大Document性能。

English: Both workspace configurations pass 176 tests and strict all-targets Clippy; formatting, 17 unchanged contracts, six schema tests, seven real-stdio tests and headless output 42 pass. Seven new Rust regressions include a real-process crash fixture. Tauri integration, a single state/receipt transaction, a full backup-generation manifest and compaction remain unfinished; other-platform/power-loss/minimum-toolchain/performance validation is pending.

简体中文：两种workspace配置各176项测试及严格all-targets Clippy、格式、17项契约无差异、6项Schema、7项实际stdio测试及headless输出42均通过。新增7项Rust回归包含真实进程中断。Tauri接入、状态／Receipt单一事务、完整备份世代manifest和压缩尚未完成；其他平台／断电／最低工具链／性能验证待完成。
