# UNGE — Universal Node Graph Engine

Tauri 2・Rust・WebGPU向けの、ディレクトリごと再利用できるノードグラフ基盤です。
設計書の中核を実装した v0.1 です。仕様全体の完成版ではありません。

[English](docs/README.en.md) · [简体中文](docs/README.zh-CN.md) · [AIへの引き継ぎ](docs/AI_INTEGRATION.md) · [ロードマップ](docs/ROADMAP.md)

## 構成

| ディレクトリ | 責務 | 依存先 |
|---|---|---|
| `crates/unge-core` | 型付きグラフ、Document、Workspace、命令、履歴、JSON、BVH、レイアウト | UI/GPU依存なし |
| `crates/unge-executor` | ノード登録、非同期DAG実行、並列数制限、キャッシュ、キャンセル | core |
| `crates/unge-interaction` | ポインター操作、選択、ドラッグ、Port接続、一時プレビュー | core |
| `crates/unge-render` | 描画シーン、WGSL、インスタンシング、ネイティブSurface | core、interaction、wgpu 27 |
| `crates/unge-acx` | ACX能力公開、事前確認・承認・実行・Receipt・回復 | core、executor |
| `crates/unge-tauri` | Rust共有状態、Tauri 2命令、リビジョン競合検出 | core、executor、render、Tauri 2 |
| `crates/unge-contracts` | Rustから型・Schemaを生成するビルド用ツール | schema feature付きの既存クレート |
| `bindings/typescript` | 型付きIPCクライアント、英語・日本語・简体中文のメッセージ | invokeを外から注入 |
| `examples/headless` | `20 + 22 = 42` のGUI不要な実行例 | core、executor |
| `examples/tauri-host` | WebView操作画面＋ネイティブwgpuウインドウ | ACXを含む5クレート |

Document・表示状態・RendererはRustで所有します。WebViewは命令と小さなメタデータをやり取りします。
画像、動画、Tensor、GPUバッファをフレームごとにJavaScriptへ渡しません。

## 実行

Rust stableと、デスクトップ例には[TauriのOS別前提条件](https://v2.tauri.app/start/prerequisites/)が必要です。
検証に使ったRustは1.98.1です。依存バージョンは `Cargo.lock` に固定しています。

```sh
# GUI不要のテストとサンプル
cargo test --locked
cargo run --locked -p unge-headless

# グラフJSONも保存
cargo run --locked -p unge-headless -- /tmp/example.graph.json

# デスクトップ統合例（Node.jsによるビルドは不要）
cargo run --locked -p unge-tauri-host

# Tauriを含む全体の検証
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all -- --check

# GPUで描画し、ピクセルを検証
cargo test --locked -p unge-render --test render -- --ignored
```

デスクトップ例は機能確認用です。別ウインドウにグラフを描画し、操作画面で数値/加算ノード追加・Undo/Redoを試せます。「ダーク」「ライト」で操作画面と描画画面を切り替え、設定を再起動後にも復元します。[テーマの取り込み](docs/THEMES.md)を参照してください。グラフ上ではノードDrag、範囲選択、Port接続、右ドラッグPan、ホイールZoomが使えます。
この例のネイティブウインドウ生成にはTauriの `unstable` featureを使用します。
ノード名・Port名をRust/GPU側で描画し、3言語の表示名をホストから登録できます。[GPU文字の取り込み](docs/GPU_TEXT.md)を参照してください。同一ウインドウ内のネイティブSurface合成は今後の実装項目です。入力の取り込み方とOS別の検証範囲は [POINTER_INPUT.md](docs/POINTER_INPUT.md) を参照してください。

## 別プロジェクトに取り込む

このディレクトリを `vendor/unge/` にコピーし、必要なクレートだけをpath依存で参照します。
各クレートのCargo.tomlはルートのworkspace依存定義を使っていないため、`crates/` をまとめて移すこともできます。

```toml
[dependencies]
unge-core = { path = "../vendor/unge/crates/unge-core" }
unge-executor = { path = "../vendor/unge/crates/unge-executor" }
unge-render = { path = "../vendor/unge/crates/unge-render" }
unge-tauri = { path = "../vendor/unge/crates/unge-tauri" }
```

パスは取り込み先のCargo.tomlからの相対位置に調整してください。
GUI不要ならcoreとexecutorだけで動作します。
AIに渡す場合は、ルートの `AGENTS.md` と [AI_INTEGRATION.md](docs/AI_INTEGRATION.md) から読むよう依頼してください。

## 実装上の境界

- グラフの編集は `Editor::execute(Command)` を通します。Batch全体が1回のUndoになります。
- 保存にはスキーマバージョン付きJSONを使います。未知のバージョンは拒否します。
- `NodeDefinition` 相当はexecutorの `Definition` です。実行前に登録された入出力スキーマと照合します。
- 実行はDAGのみです。独立ノードを同じ階層で並列実行し、失敗したノードの下流をBlockedにします。
- 自動型変換は行いません。変換処理を明示的なノードとして登録してください。
- `pure: true` のノードだけをキャッシュします。外部API、時刻、乱数、可変リソースを使うノードは原則falseにします。
- 描画はノード・ポート・Bezier接続線・選択枠・グリッドに対応します。文字はGlyph Atlasで描画します。CPUフォールバックは未実装です。
- 10,000ノード・30,000エッジ・60FPSは設計上の目標です。この実装で達成済みとはしていません。

詳細は [ARCHITECTURE.md](docs/ARCHITECTURE.md) と [ROADMAP.md](docs/ROADMAP.md) を参照してください。

## AIへ渡すソース一式を書き出す

```sh
python3 scripts/export.py
```

`dist/unge-0.1.0.zip` を生成します。展開したディレクトリをそのまま渡せます。
ビルド済みのtarget、node_modules、Tauri生成スキーマを除外し、設計書・コード・テスト・統合例を含めます。
Python 3.9以降が必要です。

## ACX対応AIからノードを操作する

```sh
cargo build --locked -p unge-acx-provider
python3 examples/acx-provider/agent.py
```

ACXの発見・事前確認・ポリシー承認・確定を経て、3ノードと2接続を作り、42を計算し、Receiptを検証して元に戻します。
Tauriの画面と同じDocumentを操作する起動方法、ホストの権限設定、JSON Linesの契約は [ACX_INTEGRATION.md](docs/ACX_INTEGRATION.md) にまとめています。
`acx` 側に追加したNode Graph ProfileとSchemaを同梱しているため、シンボリックリンクを含めずに再利用できます。

## プロパティ検証と履歴容量

プロパティの型・範囲・選択肢・初期値をDefinitionで宣言し、共有Editorで編集・Undo/Redoを検証できます。履歴は件数とバイト容量で制限します。全サンプルで編集時検証を有効にしました。取り込み方法と互換性は [PROPERTY_VALIDATION.md](docs/PROPERTY_VALIDATION.md) を参照してください。

実行キャッシュはハッシュ検索と件数・バイト容量制限に対応します。ホスト設定とpureの契約は [EXECUTION_CACHE.md](docs/EXECUTION_CACHE.md)。

Rustホストへ実行進捗を通知し、ホストが提供するFutureでrun全体の期限を設定できます。[実行進捗とキャンセル](docs/EXECUTION_PROGRESS.md) を参照してください。

共有RunServiceをTauriとACXへ接続し、実行ID・snapshot revisionで進捗とキャンセルを管理できます。[共有実行サービス](docs/EXECUTION_SERVICE.md)。

操作画面に実行・キャンセルと進捗表示を追加しました。実行後に編集したグラフとsnapshotの違いも表示します。

ACXの非同期実行例は `python3 examples/acx-provider/agent.py --async-run`。AIが同じpipeで実行中の照会・キャンセルを行えます。[任意job Profile](docs/ACX_ASYNC_JOBS.md)。

Rust契約からTypeScript型・JSON Schemaを生成できます。プロパティ値制約の公開とCI差分チェックは [CONTRACT_GENERATION.md](docs/CONTRACT_GENERATION.md)。

10,000ノード・30,000エッジのCPU計測例とBVH構築の改善を追加しました。[PERFORMANCE.md](docs/PERFORMANCE.md)。60FPSは未検証です。

小さなノード移動では描画Indexを差分更新します。UI・ポインター・ACX共通の取り込み契約は [INDEX_UPDATES.md](docs/INDEX_UPDATES.md)。

接続線を形状・ズームに応じて適応分割し、全体表示時のQuad生成量を減らしました。[CURVE_TESSELLATION.md](docs/CURVE_TESSELLATION.md)。

Groupの枠と見出しをGPUで描画します。既存Command/ACXで設定でき、ドラッグにも追従します。[GROUP_RENDERING.md](docs/GROUP_RENDERING.md)。

キーボードからノードを選択・座標移動・削除するHTML操作と、有界なRustノード概要APIを追加しました。 [ACCESSIBILITY.md](docs/ACCESSIBILITY.md)。

Groupの作成・名前/所属編集・削除と所属ノード選択を3言語UIから操作できます。 [GROUP_EDITING.md](docs/GROUP_EDITING.md)。

Definitionごとの入力/出力値マップSchemaとRust検証APIを追加しました。 [PORT_VALUE_SCHEMAS.md](docs/PORT_VALUE_SCHEMAS.md)。
