# Shared Engine persistence / 共有Engineの永続化 / 共享Engine持久化

## 日本語

`unge-tauri`の`acx` featureで、共有Engineを任意の永続Editorとして開けます。Command、Undo／Redo、Group編集、PointerのUp確定、ACXのapply／recoverが同じEditorとログを使います。各ウインドウにDocumentの複製を持たせません。

```toml
unge-tauri = { path = "../vendor/unge/crates/unge-tauri", features = ["acx"] }
unge-acx = { path = "../vendor/unge/crates/unge-acx" }
```

```rust,ignore
// 起動設定中に実行する。seedのIDとRegistry、履歴上限は再起動後も維持する。
let engine = unge_tauri::Engine::open_persistent(
    "private/state.jsonl", seed, registry.clone(),
    unge_acx::PersistentHostOptions::default(),
).map_err(|error| format!("{}: {}", error.code, error.message))?;
// RunServiceを使う場合はここでengine.with_execution(service)を設定する。
let engine = std::sync::Arc::new(engine);
let provider = unge_acx::Provider::new(engine.clone(), registry, policy)
    .with_journal("private/replay.jsonl", Default::default())?;
// 両ログの検証が成功してからView登録・app.manage・イベント受信を開始する。
// engine.register_view("controls", viewport)?;
```

### サンプルで有効化

```sh
# 保存先の親ディレクトリを先に用意する。新規導入では両ログを新しいパスにする。
cargo run --locked -p unge-headless -- /tmp/unge-seed.json
cargo run --locked -p unge-tauri-host -- --acx-stdio \
  --document /tmp/unge-seed.json \
  --state-log /tmp/unge-state.jsonl --journal /tmp/unge-replay.jsonl
```

JSON Linesクライアントをstdin/stdoutへ接続します。接続中も通常のUI操作が同じ状態を編集します。EOFで終了します。サンプルでは永続化オプション3つと`--acx-stdio`をすべて指定してください。重複指定・引数欠落・片側ログ欠損を拒否し、ログの相互prefixを検証してから画面を公開します。通常起動と、保存オプションのない`--acx-stdio`は従来のメモリー内動作です。seedファイルは書き換えません。

### 保存契約と費用

- Engineの一つのmutex内でexpected_revisionを検査し、候補Editorで検証、ログ追記とsync_all、共有Editor公開、SceneIndex更新の順に確定します。公開後も同じRustサービスがDocumentと描画を所有します。大容量データをIPCへ追加しません。
- `PersistentEditor`は既存PersistentHostのログ処理を共有する埋め込み用Rust型です。専用mutexや別Documentサービスを作らず、Engineが所有します。公開Editor参照は読み取り専用の最後に公開した状態です。独自ホストは同じ排他領域で`check()`に成功してから読み取りを提供し、全変更をexecute／undo／redoに通します。Engineは取得用共通guardでこの確認を行います。
- 候補検証・容量超過・revision競合の失敗ではDocument、履歴、revisionと描画Indexを公開せず、メモリー内モードへ切り替えません。I/O失敗で保存結果が不明な場合は読み取り／新規編集も停止し、再起動とログ検証を必要とします。最新revisionに置き換える自動再試行は行いません。
- 変更ごとにEditor／Document／保持履歴を一時コピーし、同期I/Oを共有ロック内で行います。永続モードでのDrag確定や大規模編集の応答時間は未計測です。通常のEngineは従来のEditorを一つ保持し、このコピー／ディスク同期を追加しません。
- 復元するのはDocument・revision・同じ上限のUndo／Redoです。選択・Viewport・言語・テーマ・Renderer・GPU資源・実行中Futureは復元しません。パネル配置／テーマの既存設定復元は独立した仕組みです。
- EngineのGraphHostはpersistence／verify_persistence／acknowledge_replayも転送します。状態／Receiptは引き続き2ログです。編集保存後・Receipt確定前の中断では同commitを`execution_uncertain`として再実行しない契約を維持し、単一トランザクションや外部副作用のexactly-onceを保証しません。

予算・単一writer・破損保全・Registry互換性・ログ形式・バックアップの境界は、[編集保存](ACX_EDITOR_PERSISTENCE.md)、[相互prefix](ACX_LOG_PAIRING.md)、[バックアップ世代](ACX_BACKUP.md)を引き継ぎます。APIのエラーは既存のstorage codeを維持し、TypeScript辞書とサンプルの表示に3言語の文言を追加しています。Document／IPCのwire型は変更していません。

### 検証

Rustで2 View間のUI／AI／Group／Pointer／Undo／Redoの復元、同revision同時編集の勝者だけの保存、失敗時の無変更、writer排他、破損保全、元Receipt再送、片側巻き戻し拒否を検証します。サンプルの引数不備もunit testで検証します。

macOSの実Tauriを2回起動し、1ノード／revision 1／値42と元Receiptを照合しました。CUA画面でも再起動後のGPUノードとWebViewを確認しました。実プロセス検証は`cargo build --locked -p unge-tauri-host`後に`python3 scripts/test_tauri_persistence.py`で再現できます。GUIを開く任意テストのためCIに自動追加していません。テストは一時的なseed・両ログ・パネル設定を使います。

⭕️ [Pending] Windows／Linux実機、実電源断、最低Rust版、巨大Documentの速度／メモリー、永続モードでの実画面ドラッグ・ドッキングと長時間運用。2ログ単一トランザクション・旧ログ移行・圧縮は未実装です。

## English

Enable `unge-tauri/acx` and call `Engine::open_persistent(path, seed, registry, options)` before exposing the Engine. It owns one embedded `PersistentEditor`, sharing the existing PersistentHost log format. UI commands, groups, pointer commit, Undo/Redo and ACX apply/recover all use that same editor under the Engine mutex. Validate the Provider replay journal before accepting view edits; forward all three persistence methods in any custom GraphHost wrapper. No per-window Document or frame-data IPC is introduced.

The sample enables this through `--acx-stdio --document SEED --state-log STATE --journal REPLAY`. All four options are required together; duplicate/missing arguments or one missing log are refused. Pair validation precedes view exposure. The seed remains unchanged. Ordinary launch and stdio without storage flags remain in memory. Connect a JSON Lines agent to stdin/stdout; EOF exits the app.

Durable edits clone a candidate Editor/history, validate, append and sync, publish, then update the SceneIndex. Invalid edits/conflicts/capacity failures publish nothing; I/O ambiguity stops reads/new edits without fallback or automatic retry. Sync I/O and candidate cloning occur inside the shared lock, and large-document latency/memory are unmeasured. Memory-only Engine does not add that work. Restore Document/revision/bounded history, not selection, viewport, locale, theme, renderers, GPU resources or running futures. Existing presentation preferences remain separate. State and receipt remain two logs; interruption between state save and receipt completion remains uncertain and cannot be reexecuted. This is not an atomic external-effect transaction.

Embedded-host users must call `PersistentEditor::check()` under their own mutex before serving reads; its editor reference is read-only last-published state. Mutation goes only through execute/undo/redo. Existing log budgets, writer exclusion, corruption preservation, compatible deterministic validators and paired backups still apply. Stable storage codes have EN/JA/简体中文 UI messages; wire types are unchanged. Rust tests cover restoration, concurrent views, failure atomicity, original receipt replay and rollback rejection. A real macOS restart and CUA screenshot confirm the restored node/WebView; the optional script above opens native windows. Other platforms, power loss, minimum toolchains, huge documents, persistent-mode GUI drag/docking and long-duration operation remain pending.

## 简体中文

启用`unge-tauri/acx`，在公开Engine前调用`Engine::open_persistent(path, seed, registry, options)`。Engine持有唯一的嵌入式PersistentEditor，复用PersistentHost日志格式。UI命令、Group、Pointer提交、撤销／重做及ACX apply／recover均在同一mutex下修改该Editor。接受View编辑前验证Provider重放日志；自定义GraphHost封装须转发全部三个持久化方法。各窗口不复制Document，不增加帧数据IPC。

示例通过`--acx-stdio --document SEED --state-log STATE --journal REPLAY`启用，四项须一起提供；拒绝重复／缺失参数和单侧日志缺失，配对验证成功后才公开View。seed不被改写。普通启动及不含存储参数的stdio仍使用内存模式。JSON Lines客户端连接stdin／stdout，EOF退出。

持久化编辑在共享锁内复制候选Editor／历史、验证、追加并同步、公开状态、更新SceneIndex。无效编辑／冲突／容量错误不公开更改；I/O结果不明时停止读取和新编辑，不回退或自动重试。大Document的同步延迟与内存尚未测量；内存模式不增加此复制／同步工作。恢复Document、revision和有界历史，不恢复选择、Viewport、语言、主题、Renderer、GPU资源或运行中Future，既有界面偏好恢复独立处理。状态与Receipt仍为两份日志；两者之间中断的commit不重做，不提供外部副作用的单一事务保证。

自行嵌入PersistentEditor时，在宿主mutex内先调用check再提供读取；editor引用是只读的最后公开状态，修改必须通过execute／undo／redo。继承原日志预算、writer排他、损坏保全、确定性兼容validator和成对备份约定。稳定storage code具备三语UI文案，wire类型不变。Rust测试覆盖恢复、并发View、失败原子性、原Receipt重放及回退拒绝。macOS真实重启与CUA画面确认恢复的节点／WebView，上述可选脚本会打开原生窗口。其他平台、断电、最低工具链、大Document、持久模式的实画面拖动／停靠及长期运行待验证。

状態ログの保持量照会と、メモリー内Editor／PersistentEditorの比較は [PERSISTENCE_PROFILING.md](PERSISTENCE_PROFILING.md) を参照。Storage counters and reproducible profiling are available through this guide. 状态日志用量与可复现比较见该指南。
