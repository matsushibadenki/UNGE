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

## 編集時検証・履歴容量の追加（2026-09-28）

| 確認 | 結果 |
|---|---|
| `cargo test --workspace --all-features --locked --offline` | Rust 52件成功。GPUテスト1件はignore |
| `cargo clippy --workspace --all-features --all-targets --locked --offline -- -D warnings` | 成功 |
| `cargo fmt --all -- --check` / `git diff --check` | 成功 |
| TypeScript `tsc -p bindings/typescript` | 新しいDefinition型・定義照会・3言語辞書を含め成功 |
| `python3 scripts/test_acx.py -v` | 4件成功。実Rustプロセスのプロパティ定義照会、不正値拒否、初期値適用と既存ライフサイクル |
| acxのNode Graph Profileテスト | 5件成功。既存のIntent Schema互換性を確認 |
| headlessサンプル | Registry検証を有効にしたグラフで42を確認 |
| Tauri `--acx-stdio` | 実Tauri/Metalへ接続し、3ノード・2接続追加、42を実行、Receiptを確認、元の12ノードへ回復 |

追加したRustテスト13件は、型・範囲・選択肢・Unicode長・初期値・必須値、旧Definition読込、登録時の不正制約拒否、最終Batchの検証、失敗時のDocument/revision/両履歴保持、Undo/Redo合算容量、逆命令の容量増加、巨大な編集による履歴破棄、UIとAIの共通検証、容量不足時のACX回復拒否を確認します。

今回レンダラーやシェーダーは変更していません。GPUピクセルテストは再実行せず、Tauri実プロセスの統合経路を確認しました。履歴バイト数はシリアライズした命令ペイロードの容量であり、プロセス全体のメモリ上限ではありません。大規模グラフのFPS測定は今回も対象外です。

## ポインター操作の追加（2026-09-28）

| 確認 | 結果 |
|---|---|
| `cargo test --workspace --all-features --locked --offline` | Rust 62件成功。GPU1件は通常実行ではignore |
| `cargo test --locked --offline -p unge-render --test render -- --ignored` | Metal実GPUで1件成功。移動プレビューの位置にNodeを描き、元の位置が背景になったこととGPU検証エラーなしを確認 |
| `cargo clippy --workspace --all-features --all-targets --locked --offline -- -D warnings` | 成功 |
| `cargo fmt --all -- --check` / `git diff --check` | 成功 |
| TypeScript型チェック / `node --check examples/tauri-host/ui/app.js` | 成功。JS構文チェックをCIにも追加 |
| `python3 scripts/test_acx.py -v` | 4件成功 |
| Tauri実画面 | 単一NodeのDrag、Undo、加算Node追加、Port接続、Box Select、2Node同時Drag、ホイール操作を確認 |
| 同じ実プロセスのACX照会 | Drag後の座標100/90とrevision 1、Undo後revision 2、追加後3、接続数3/revision 4、2Node同時Drag後の座標320/60とrevision 5を確認 |
| Tauri `--acx-stdio` のPythonクライアント | 3Node/2Edge追加、42を実行、Receipt照合、元の12Node/2Edgeへ回復 |

追加したテストは、確定前にDocument/revisionが変わらないこと、1回のUndoで複数Nodeを復元できること、Box/Shift選択、クリックの揺れ、キャンセル、同一pointerの所有権、Zoom変換、双方向接続、占有/循環/同側Portの拒否、古いrevisionの拒否、ビュー更新時のキャンセル、移動Node/Edgeのカリングと残像防止を確認します。

macOSの実画面でドラッグした距離と、ACXで照会した論理座標の変化が一致することを確認しました。日本語パネルのボタン配置・意味単位の改行も確認しました。英語・简体中文の文言は追加済みですが、今回は全言語・全画面サイズの実操作試験は行っていません。Windows/Linux実機、同一ウインドウ合成、タッチ、キーボードだけの編集、GPU文字、大規模FPSは未検証または未実装です。

## GPU文字の追加（2026-09-28）

| 確認 | 結果 |
|---|---|
| `cargo test --workspace --all-features --locked --offline` | Rust 66件成功。GPU2件は通常実行ではignore |
| `cargo test --locked --offline -p unge-render --test render -- --ignored` | Metal実GPUで2件成功。英語・日本語・简体中文の欠落グリフなし、1倍/2倍解像度、文字クリップ、前面ノードによる遮蔽、アトラス再利用、移動プレビューのピクセルを確認 |
| `cargo clippy --workspace --all-features --all-targets --locked --offline -- -D warnings` | 成功 |
| TypeScript型チェック / JavaScript構文チェック | 成功。`set_locale` のRust/TypeScript/wire値を同期 |
| `python3 scripts/test_acx.py -v` | 4件成功 |
| 最終Tauriビルドの `--acx-stdio` | 3Node/2Edge作成、再送時の重複防止、42の実行、Receipt照合、元のグラフへの回復が成功 |
| macOS実画面 | 日本語と英語のノード名/Port名、言語変更後revision 0維持を確認 |

追加の通常テストはAtlas配置上限、表示名の翻訳/識別子フォールバック/長さ制限、LOD、Port位置と移動プレビュー、描画順序、Viewごとの言語分離とDocument不変性を確認します。文字の描画順を保つためノード単位でバッチを分けています。大規模FPSは未測定です。フォントは検証端末のシステムフォントを使用しました。フォントのない環境、全DPI、Windows/Linux実画面、全言語の操作パネルは未検証です。配布先にCJKフォントを用意する方法と容量上限は [GPU_TEXT.md](GPU_TEXT.md) を参照してください。

## ダーク/ライトの追加（2026-10-03）

| 確認 | 結果 |
|---|---|
| `cargo test --workspace --all-features --locked --offline` | Rust 70件成功。GPU3件は通常実行ではignore |
| `cargo test --locked --offline -p unge-render -- --ignored` | Metal実GPUで3件成功。Dark→Light→Darkの背景clear・グリッド・Node・Port・文字のピクセル、Atlas再利用、既存のCJK/HiDPI/クリップ/プレビューを確認 |
| `cargo clippy --workspace --all-features --all-targets --locked --offline -- -D warnings` | 成功 |
| `cargo fmt --all -- --check` / `git diff --check` | 成功 |
| TypeScript型チェック / JavaScript構文チェック | 成功。SetTheme・AppearanceをRustと同期 |
| `python3 scripts/test_acx.py -v` | 4件成功 |
| 最終TauriビルドのACX連携 | 3Node/2Edge作成・再送時の重複防止・42の実行・Receipt照合・元のグラフへの回復が成功 |
| 3言語のJSロジック確認 | 一時的なNode VMテストで、Dark/Lightの翻訳、保存選択の再適用、変更失敗時の設定維持、無効な保存値、ストレージ利用不可時を確認。実画面テストとは別のロジック検証 |
| macOS実画面 | 日本語のDark→Light→Dark、操作画面/ネイティブ枠/GPUの連動、revision 0維持、Lightを保存して再起動後の復元を確認 |
| 操作画面の配置 | 380×720の日本語表示で、テーマ選択・既存ボタン・ヘルプ・状態表示が収まることを確認 |

配色のテストは、文字4.5以上・操作色3以上のコントラスト、テーマごとの同一形状とラベル、仮接続と選択矩形、View分離、Document/Undo/Redo維持、操作中の切り替え、未知Viewと無効テーマの拒否を含みます。パレットのsRGB→線形変換はScene生成前にまとめて計算し、ノードごとの変換を避けています。

対象はネイティブTauriの `tauri://localhost` 操作画面とMetal描画面です。Browserプラグインは未提供で、埋め込みWebViewとネイティブ描画をCUAで操作しました。単独ブラウザーやPlaywrightは使用していません。表示は空白やエラー画面にならず、操作後のAX状態とスクリーンショットで配色・選択状態・revisionを確認しました。WebViewの開発者コンソールは取得していません。Windows/Linux実画面、全言語の画面配置、最小幅320pxの実画面は未検証です。実装/API/ネイティブ枠のOS制約と設定保存範囲は [THEMES.md](THEMES.md)。

## 実行キャッシュの追加（2026-10-03）

| 確認 | 結果 |
|---|---|
| `cargo test --workspace --all-features --locked --offline` | Rust 76件成功。追加6件で件数・バイトの境界、即時縮小、保存無効化、FIFO、重複保存、巨大キー/出力、executor分離・version、Arc解放、失敗・無効出力・キャンセルを確認 |
| `cargo clippy --workspace --all-features --all-targets --locked --offline -- -D warnings` | 成功 |
| `cargo fmt --all -- --check` / `git diff --check` | 成功 |
| TypeScript型チェック | 成功。今回Report/IPC/ACXのwire形式は変更なし |
| `python3 scripts/test_acx.py -v` | 4件成功 |
| `cargo run --locked --offline -p unge-headless` | 20+22=42、再実行で全3ノードCached、保持量3件/313シリアライズバイトを確認。executorアドレスの桁数でバイト数は変わり得る |

今回の変更対象はexecutorと取り込み文書・headless例です。GPUの変更はなく、実GPUテストは再実行していません。検索は標準HashMapの平均計算量に基づく改善で、10kノードの実測性能やプロセス全体のメモリ上限は未検証です。容量の意味とホスト資源管理の範囲は [EXECUTION_CACHE.md](EXECUTION_CACHE.md)。

## 実行進捗・期限・待機中キャンセルの追加（2026-10-03）

| 確認 | 結果 |
|---|---|
| `cargo test --workspace --all-features --locked --offline` | Rust 85件成功。追加9件で終端イベントとReportの一致、Cached/Blocked/Failed、事前キャンセル、即時期限、Pending executorの破棄、完了済み結果の保持、callbackからのキャンセル、空/不正グラフ、Send、複数waiterの起床とdrop時の解除を確認 |
| `cargo clippy --workspace --all-features --all-targets --locked --offline -- -D warnings` | 成功 |
| `cargo fmt --all -- --check` / `git diff --check` | 成功 |
| TypeScript型チェック | 成功。ProgressEvent、停止理由、3言語の表示文言を追加 |
| ACX Providerの再ビルド後 `python3 scripts/test_acx.py -v` | 4件成功。従来のReport/ACX wire形式とライフサイクルを維持 |
| headless実行例 | 20+22=42、stderrにstarted→node_started/node_finished→finished、再実行の全ノードCachedを確認 |

期限テストはoneshot Futureを使い、実時間のsleepや特定ランタイムへ依存せず停止の順序と結果を確認します。ホストのタイマー精度、外部worker/GPU/HTTP処理の停止、Tauri/ACXの進捗転送とキャンセル命令は今回の検証対象ではありません。今回GPU変更はなく、実GPUテストは再実行していません。ホストが守る契約とAPIは [EXECUTION_PROGRESS.md](EXECUTION_PROGRESS.md)。

## 共有実行サービス・Tauri操作API・ACXホスト接続（2026-10-03）

| 確認 | 結果 |
|---|---|
| `cargo test --workspace --all-features --locked --offline` | Rust 95件成功。追加10件でsnapshot/revisionの固定、共有cache、単一予約、drop・panic後の復旧、容量と保持上限、別スレッドのcancel、View/revision/Document検証、編集・Undoと実行の分離、ACXのcancel/Receipt再送・busy・Registry不一致を確認 |
| `cargo clippy --workspace --all-features --all-targets --locked --offline -- -D warnings` | 成功 |
| `cargo build --workspace --locked --offline` | 全サンプルとTauri命令登録のビルド成功 |
| `cargo fmt --all -- --check` / `git diff --check` | 成功 |
| TypeScript型チェック | 成功。RunSummary、操作クライアント、3言語文言・エラー辞書を追加 |
| 新ビルド後 `python3 scripts/test_acx.py -v` | 4件成功。headless Providerも共有サービス経由で検証 |
| macOS Tauri `agent.py --binary target/debug/unge-tauri-host --desktop` | 発見・事前確認・承認・commit、3Node/2Edge編集、Receiptと再送、42の共有サービス実行、元Documentへの回復が成功 |

Tauriの4実行命令は登録・ビルドとEngine側の直接APIテストで検証しました。実WebViewからのinvoke、通知購読の実画面、Windows/Linuxは未検証です。HTML操作画面への実行ボタンと進捗表示は今回追加していません。今回GPU変更はなく実GPUテストは再実行していません。

サービスはlatest RunSummaryのみ保持し、Tauriへ集約・レート制限したadvisory通知を送ります。配送欠損や応答前の通知はsequenceと照会で回復します。ACXの承認済みexecute/Receiptのwire形式は維持し、同期JSON Lines処理中のAI照会・cancelは未実装です。ホスト設定、保持上限、失敗時の再送、今後のwire仕様との境界は [EXECUTION_SERVICE.md](EXECUTION_SERVICE.md)。
