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

## ACX追加後の検証（2026-09-26）

| 確認 | 結果 |
|---|---|
| `cargo test --workspace --all-features --locked --offline` | Rust 39件成功。既存GPUテスト1件はignore |
| `cargo clippy --workspace --all-features --all-targets --locked --offline -- -D warnings` | 成功 |
| TypeScript `tsc -p bindings/typescript` | ACX型・3言語辞書を含め成功 |
| `python3 scripts/test_acx.py -v` | 3件成功。Rustの実プロセスにPythonから接続、ACX Schema検証、digest照合、作成・実行・回復 |
| acxのProfileテスト | 8件成功（新規Node Graph 5件＋既存Print 3件） |
| acxの既存HTTPライフサイクル | 7件成功。loopback利用のためサンドボックス外で実施 |
| Tauri `--acx-stdio` | 実際のMetal/TauriプロセスへPythonから接続。初期12ノードに3ノード・2接続追加、42を実行、Receipt照合、元のグラフに回復 |

新規Rustテストには事前確認の無変更、改変入力、他preflightの承認、重複Commit、期限境界、失敗結果の再送、容量制限、型の許可リスト、UI編集との競合、回復時の競合、過大な通信行の拒否、0.1等のf32小数のハッシュ整合を含みます。

今回GPUシェーダー/レンダラーのコードは変更していません。実GPUの独立ピクセルテストは前回の結果を維持し、今回はTauriプロセスを使うACX統合経路を確認しました。
