# 検証結果

検証日: 2026-09-26。macOS / Apple Metal / rustc 1.98.1。

| 確認 | 結果 |
|---|---|
| `cargo test --workspace --locked --offline` | 通常テスト20件成功。GPUテスト1件は通常実行ではignore |
| `cargo test --locked --offline -p unge-render --test render -- --ignored` | Metal実GPUで1件成功。シェーダー検証と描画ピクセルを確認 |
| `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` | プロジェクトのClippy診断なし |
| `cargo fmt --all -- --check` | 成功 |
| TypeScript `tsc -p bindings/typescript` | 成功 |
| `cargo run -p unge-headless` | 20 + 22 = 42。グラフJSONを書き出し |
| Tauriデスクトップ | WebView表示とネイティブwgpu描画を確認。追加で13ノード/revision 1、Undoで12/revision 2、Redoで13/revision 3 |

GPUはサンドボックス内ではMetalアダプターを取得できず、デスクトップ側でテストを実行しました。
依存クレート `block 0.1.6` にRustのfuture-incompatibility警告が残っています。現在のコンパイル・テストは成功しています。

## 検証対象の詳細

- Batch途中のエラーと最終検証エラーのロールバック
- 型不一致、入力の多重接続、循環の拒否
- 削除したノード・エッジ・グループ・座標のUndo復元
- Copy/Pasteの新IDと内部接続、履歴上限とRedo破棄
- JSON、スキーマ版、Workspaceの複数Document
- BVH検索、階層レイアウト、ズーム中心、座標オーバーフロー拒否
- DAG実行、pureキャッシュ、入力変更による再実行
- 失敗ノードの下流Blocked、独立分岐の継続、協調キャンセル
- 並列数の上限と実際のFuture実行の重なり
- 画面外ノードのカリング、端点が画面外の接続線
- Tauri共有Document、stale revision拒否、View認可、Batch深さ制限

## 未検証の範囲

Windows/Linux実機、ブラウザーWASM、Surfaceの同一ウインドウ合成、長時間のGPU復帰、大規模グラフのFPSは未検証です。
アプリ固有のResourceStore、Remote Backend、未実装機能は対象外です。
デスクトップのZoomと多言語辞書は実装済みですが、全言語・全DPIの操作試験は行っていません。
