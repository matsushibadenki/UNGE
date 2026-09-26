# UNGE — Universal Node Graph Engine

Tauri 2・Rust・WebGPU向けの、ディレクトリごと再利用できるノードグラフ基盤です。
設計書の中核を実装した v0.1 です。仕様全体の完成版ではありません。

[English](docs/README.en.md) · [简体中文](docs/README.zh-CN.md) · [AIへの引き継ぎ](docs/AI_INTEGRATION.md) · [ロードマップ](docs/ROADMAP.md)

## 構成

| ディレクトリ | 責務 | 依存先 |
|---|---|---|
| `crates/unge-core` | 型付きグラフ、Document、Workspace、命令、履歴、JSON、BVH、レイアウト | UI/GPU依存なし |
| `crates/unge-executor` | ノード登録、非同期DAG実行、並列数制限、キャッシュ、キャンセル | core |
| `crates/unge-render` | 描画シーン、WGSL、インスタンシング、ネイティブSurface | core、wgpu 27 |
| `crates/unge-acx` | ACX能力公開、事前確認・承認・実行・Receipt・回復 | core、executor |
| `crates/unge-tauri` | Rust共有状態、Tauri 2命令、リビジョン競合検出 | core、render、Tauri 2 |
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

デスクトップ例は機能確認用です。別ウインドウにグラフを描画し、操作画面でノード追加・Undo/Redo・ズームを試せます。
この例のネイティブウインドウ生成にはTauriの `unstable` featureを使用します。
ノード上の文字、マウスでのノード編集、同一ウインドウ内のネイティブSurface合成は今後の実装項目です。

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
- 描画はノード・ポート・Bezier接続線・選択枠・グリッドに対応します。文字描画とCPUフォールバックは未実装です。
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
