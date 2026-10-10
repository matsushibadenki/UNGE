# Registryの編集時検証

## 日本語

`Arc<Registry>` を `Editor::with_validator` または `Engine::from_registry` へ渡す既存の設定で有効になります。UIとACXは同じ共有Editorを使用してください。

| 経路 | 実施する検証 |
|---|---|
| Editor生成・validator設定 | Documentの構造、登録Definition・Port一致・PropertySchema |
| MoveNode／再帰的に移動だけのBatch／そのUndo・Redo | 各対象ノードと矩形。Graphとプロパティは直前の検証結果を利用 |
| その他のCommand／混在Batch／そのUndo・Redo | coreでDocument全体、Registryで全ノードのDefinition・Port・プロパティ。Graph構造を二重走査しない |
| `Registry::validate_edit` の直接呼び出し | 従来どおりGraph全体とDefinition・Port・プロパティ |
| `Registry::validate` の直接呼び出し／実行前検証 | 上記に加え、必須入力の接続 |

空Batchも移動だけの範囲です。混在Batchは最終状態を検証し、途中だけ不正な値があっても最終的に修復されていれば許可します。未知の型、Port不一致、不正なプロパティは引き続き拒否します。失敗時の巻き戻し、revision、履歴の件数・容量制限は変わりません。

### Rust拡張点と互換性

`DocumentValidator::validate_transition(ValidatedEdit<'_>)` を追加しました。Editorはcoreの不変条件を確認してから、最終状態への不変参照と `placement_only()` を渡します。`ValidatedEdit` はフィールド非公開で、公開コンストラクターもDeserializeもありません。任意の読込Documentから作ることはできません。初期状態は同じvalidatorの `validate` を通過しています。

既存の `validate` だけを実装するホストは修正不要です。既定の `validate_transition` が常に `validate` を呼ぶため、位置依存の独自ルールも移動・Undo・Redoごとに実行します。Registry自身は共有中にDefinitionを変更できず、位置に依存しないので、この範囲で以前の結果を再利用します。

独自アダプターで最適化を転送する場合、**初期のvalidateとすべてのtransitionを同じRegistryへ転送し、検証ルールを途中で交換しない**でください。別Registryへトークンを転送しても、以前の状態がそのRegistryで検証済みとは限りません。独自の追加ルールは各transitionでも実行します。ルール変更には新しいEditorと明示的な移行を使用してください。これは信頼済みRustコード向けの契約であり、ACXの承認・revision検査を代替しません。

coreからexecutorへの依存、IPC、TypeScript型、ACXの形式は変更しません。ACXのPreflightや任意Graphの検査には引き続き完全な公開validatorを使います。

### 再現

```sh
cargo run --release --locked -p unge-executor --example registry_benchmark
```

10,000ノード／30,000接続で、1／100ノード移動と1ノードのプロパティ変更を計測します。同じバイナリー内で従来相当の全体検証ラッパーとRegistry直接登録を比較し、各条件5回予熱＋30回計測します。毎回Undoで復元し、Document一致と独立した全体検証を計測外で確認します。履歴は64ステップ／16MiB。生データには確定処理の各工程と通常Undoを含みます。Scene／Index／GPU／IPC／ロック待ち／ACX Preflightは測りません。[結果と制限](PERFORMANCE.md)。

## English

Install the existing shared `Arc<Registry>` through `Editor::with_validator` or `Engine::from_registry`. Initial validation and standalone `Registry::validate_edit` still check the full graph, registered definitions, ports and properties. Execution additionally checks required connections. After core checks a transition, Registry reuses prior results for move-only edits (including nested/empty batches and their undo/redo). Other edits retain full definition/port/property validation on the final state, without repeating core graph validation. Atomic rollback, revision and bounded history remain unchanged.

The new default `DocumentValidator::validate_transition(ValidatedEdit<'_>)` calls the existing `validate` method every time. Existing validators, including placement-dependent rules, require no changes. Only Editor constructs the borrowed token after checking core invariants; it has private fields and no public constructor or deserialization. The previous state passed the same installed validator.

Custom adapters may forward this optimization only if initial validation and every transition use the same immutable Registry/rules. Forwarding a token to another Registry does not prove that Registry validated the previous state. Always execute additional host rules on transitions. Changing schemas requires a new Editor and explicit migration. This trusted Rust extension is not an authorization mechanism. Core dependencies, IPC, TypeScript and ACX wire contracts are unchanged; ACX preflight and arbitrary graph validation retain complete checks.

Run the command above for a same-binary comparison of legacy full validation and direct Registry installation on 10,000 nodes / 30,000 edges: 1/100-node moves and one property change, five warm-ups and 30 samples per case. Undo restores and independently validates each document outside timing. History is enabled at 64 steps / 16 MiB. Measurements exclude Index, scene, GPU, IPC, locks and ACX preflight; see [performance results](PERFORMANCE.md).

## 简体中文

继续通过 `Editor::with_validator` 或 `Engine::from_registry` 安装共享的 `Arc<Registry>`。初次验证及直接调用 `Registry::validate_edit` 仍完整检查图、注册类型、端口和属性；执行前还检查必需连接。core确认状态转换后，Registry对纯移动（包括嵌套／空Batch及其Undo／Redo）复用上次结果。其他编辑仍检查最终状态中所有节点的Definition、端口和属性，仅省略重复的图结构检查。原子回滚、revision和历史容量规则不变。

新增的 `DocumentValidator::validate_transition(ValidatedEdit<'_>)` 默认每次调用原有 `validate`，已有验证器及位置相关规则无需修改。只有Editor在确认core约束后才能创建借用令牌；其字段私有，无公开构造函数或反序列化接口。先前状态已通过同一验证器。

自定义适配器只有在初次验证和每次转换都使用同一个不变Registry／规则时，才能转发此优化。把令牌转给其他Registry不能证明旧状态曾通过该Registry。额外宿主规则必须每次执行。更换schema需创建新Editor并显式迁移。这是可信Rust扩展契约，不代替授权。core依赖、IPC、TypeScript及ACX格式不变；ACX预检和任意图验证仍使用完整检查。

上方命令在同一二进制内比较旧式全量验证与直接Registry配置：10,000节点／30,000连线，1／100节点移动及单节点属性修改，每组预热5次、计测30次。每次Undo还原并在计时外比较文档和独立全量验证。历史限制为64步／16MiB。不含Index、场景、GPU、IPC、锁等待及ACX预检。参见[性能结果](PERFORMANCE.md)。
