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

## 実行・キャンセルUI（2026-10-05）

対象フロー: 操作画面→グラフ実行→終端件数とsnapshot revision表示。通知欠損時は1秒ごとの照会で回復します。

| 確認 | 結果 |
|---|---|
| Browser環境 | Browser plugin not available。対象はTauriのネイティブWebViewで、UI操作をCUAへ限定する環境指示に従いCUAを使用。Playwrightは使用していません |
| ページ識別 / 空白 / overlay | macOSのUNGE Controls、tauri://localhost、380×720。実行操作画面が描画され、空白やframeworkエラーoverlayなし |
| 実操作 | 実行後12/12、実行ボタン再有効化、キャンセル無効化、Document revision不変をAXと画像で確認。未接続加算ノード追加後のエラー、snapshotと編集後revisionの区別、Undo後の再実行成功を確認 |
| 最終ビルド | 再起動して再実行し、日本語の「実行終了 · 12/12 · リビジョン 0」を確認 |
| スクリーンショット | CUAで最終ビルドの配置、文字、進捗バーを確認。検証画像は /tmp/unge-execution-ui.png（配布ソース外） |
| UIロジック | `node scripts/test_execution_ui.cjs` の英語・日本語・简体中文で、cancel要求、古いsequence拒否、終端通知欠損時のポーリング回復、入力エラー、ボタン状態を確認 |
| Rust / clippy / fmt | ワークスペース95テスト成功、clippy全feature/all-targets -D warnings、fmt成功 |
| ACX / TS / JS | ACX4件成功、TypeScript型チェック、app.jsとexecution.js構文チェック成功 |

ネイティブWebViewの開発者コンソールは取得していません。キャンセル操作と通知欠損はロジックテストで検証し、長時間実行の実画面では未検証です。Windows/Linux、英語/简体中文の実画面、320px幅は未検証です。表示が縦に長い場合は通常スクロールを使います。GPU実装の変更はなく実GPUテストは再実行していません。ACXの非同期job/wire照会・cancelは次の項目です。

## 任意ACX非同期ジョブ（2026-10-05）

| 確認 | 結果 |
|---|---|
| `cargo test --workspace --all-features --locked --offline` | Rust100件成功。追加5件でPending executor中の照会・cancel、開始再送、Receipt確定と同期execute再送、状態保持の独立性、未承認/編集/stale commit拒否、busy時の未消費、引数検証、Provider破棄時のcancelを確認 |
| clippy / fmt / diff | all-features/all-targets -D warnings、fmt、git diff --check成功 |
| `python3 scripts/test_acx.py -v` | 5件成功。実stdioの非同期開始・status、終端cancelの冪等性、同期executeとの結果同一性、元Receipt・正確なJSONハッシュ・Schemaを確認 |
| `python3 -m unittest discover -s acx/tests -v` | acx適合テスト9件成功。新job Schemaのpending/terminal、有効状態、必須ID、追加引数拒否を含む |
| TypeScript / Python | 型チェック、agent.py構文確認成功。startRun/runStatus/cancelRunと3言語エラー文言を追加 |
| macOS Tauri `agent.py --binary target/debug/unge-tauri-host --desktop --async-run` | 非同期開始・照会・終端再送、42の計算、Receipt照合、元Documentへの回復成功 |

Pending executorの実行中キャンセルはRust Providerテストで検証しました。Python/Tauri実stdio例は高速算術ノードを使い、終了後のcancel再送を確認しています。Windows/Linux、非yield処理の強制停止、再起動後の永続Receipt、他Providerとの相互運用は未検証または未実装です。今回GPU実装は変更せず、実GPUテストは再実行していません。

acx側へ任意の実験Profile、Schema、適合テストを追加しました。UNGEにもProfile文書とSchemaを同梱し、シンボリックリンクなしで配布できます。既存の同期execute/Receiptは維持します。開始前のrun_start busyはcommit未消費ですが、開始後は再送で実行を増やしません。結果とReceiptは後続dispatchで確定するため、executionが非nullになるまで照会します。仕様は [ACX_ASYNC_JOBS.md](ACX_ASYNC_JOBS.md)。

## 2026-10-05: Rust契約から型・Schema生成

- `cargo test --workspace --all-features --locked`: 102件成功、既存GPU3件ignored。GPU処理の変更はなく今回は実GPUテストを再実行していない。
- `cargo clippy --workspace --all-features --all-targets --locked -- -D warnings`、fmt、diff check成功。既存依存block 0.1.6のfuture compatibility警告は継続。
- 10種類のroot JSON SchemaとTypeScript入力/出力型を生成。`generate_contracts.py --check` 成功。
- `test_contracts.py -v`: 5件成功。実際のRust JSON、誤ったtag/欠損revision/座標tuple/未知pointer field、Serdeの省略規則、生成器の拒否、プロパティ判定72例を検証。
- TypeScript strict型検査成功。固定tuple、custom型のname必須、出力フィールドの非省略をcompile-time checksで確認。
- ACX実stdioテスト5件と実行UIの3言語ロジックテスト成功。UI表示・IPC実行・GPU処理は今回変更していない。
- Schemaは入力構造契約。Port値マップ、ACX規範Schema生成、JSON数値表現とRust i64の完全な同値性は実装済みとしない。

## 2026-10-05: 大規模CPU計測とBVH構築

- `cargo test --workspace --all-features --locked`: 104件成功、GPU3件ignored。新規2件は2,112矩形の総当たり/逆順検索照合、同一座標、境界、無効/空Indexを検証。
- fmt、diff check、`cargo clippy --workspace --all-features --all-targets --locked -- -D warnings` 成功。既存block 0.1.6のfuture compatibility警告は継続。
- `cargo test --locked -p unge-render -- --ignored`: サンドボックス内ではMetalアダプターが見つからず、デスクトップ環境で再実行して実GPU3件成功（ピクセル、3言語文字とDPI、テーマ）。
- Schema再生成差分チェックと契約Pythonテスト5件成功。公開型/IPC/Schemaの変更はない。
- release exampleで10,000ノード/30,000エッジを構築・検証。改善前後30回ずつの可視ノード/エッジ/Quad/ラベル件数一致。生JSONと中央値/p95はPERFORMANCE.md参照。
- ノードBVH構築中央値3.880→2.563ms、SceneIndex構築40.857→30.356ms。この固定データ・単一機械の比較で、一般的な改善率や60FPSを保証しない。
- 細粒度Index更新、GPU/Surfaceを含むフレーム時間、Windows/Linux性能は未検証/未実装のまま。

## 2026-10-05: 小さな編集のIndex差分更新

- `cargo test --workspace --all-features --locked`: 109件成功、通常実行ではGPU3件ignored。追加5件はbounded upsertの原子性、旧境界除外、反復移動/容量再構築、Batch最終座標、プロパティ/Group、構造変更、高接続ノード、共有Engineの競合・Undo/Redo・失敗Batch・ACX経路を検証。
- 差分Indexと全体再構築のQuadバイト列・文字ラベル・可視件数・spatial queryを複数Viewportで照合。一時プレビューも照合。
- fmt、diff check、workspace/all-features/all-targets clippy -D warnings成功。既存block 0.1.6警告は継続。
- macOSデスクトップ環境の `cargo test --locked -p unge-render -- --ignored`: ピクセル、3言語文字/DPI、テーマの実GPU3件成功。新しい差分描画の等価性はCPU側のScene比較で検証している。
- Schema再生成差分チェックと契約Python5件成功。実stdio ACX5件成功。IPC/Schema変更はない。
- release benchmark version 2で10,000ノード/30,000エッジ、30回計測。生データは docs/benchmarks/2026-10-05-delta.json。1ノード/接続4本の定常更新と全体再構築を別々に計測し、Document検証・Tauri待ちを差分更新時間へ含めていない。
- 曲線生成量、トポロジー/Undo/Redoの差分更新、GPU/Surfaceを含む60FPS、他OS性能は引き続き未実装/未検証。

## 2026-10-05: Bezier適応分割

- workspace/all-features Rust 112件成功、通常実行ではGPU3件ignored。追加3件で直線/退化/逆向き/端点一致、zoom0.02〜16、解析曲線4,097サンプルの誤差、極端座標の有限性/連続性/128線分上限を確認。
- 既存の差分Indexと全体再構築のScene照合も成功。
- workspace/all-features/all-targets clippy -D warnings、fmt、diff check成功。既存block 0.1.6警告は継続。
- macOS実GPU ignoredテスト3件成功。描画テストに接続プレビュー曲線の中点ピクセルを追加して確認。
- Schema生成差分チェック成功。Graph/Document/IPC契約、TypeScript型、ACXライフサイクルの変更はない。
- 同一の10,000ノード/30,000エッジ、旧/新releaseバイナリを30回ずつ計測。可視ノード/Edge/ラベル件数一致。全体表示Quad260,001→55,347、CPU中央値4.181→2.578ms。データと限界はPERFORMANCE.md参照。
- 上限に達する曲線やf32丸めでは誤差目標を保証しない。HiDPI物理誤差、極端曲線のGPU品質、GPU/Surfaceを含む60FPS、他OS性能は新たに検証していない。

## 2026-10-05: GroupのGPU描画

- workspace/all-features Rust 115件成功、通常実行ではGPU3件ignored。新規3件で所属ノードからの枠算出、空Group、画面内を囲む枠のカリング、描画順、低ズーム、ドラッグプレビュー、名前変更/所属変更/削除、重複所属、テーマ、極端座標を確認。保持Indexと全体再構築のSceneを照合。
- workspace/all-features/all-targets clippy -D warnings、fmt、diff check成功。既存block 0.1.6のfuture compatibility警告は継続。
- macOS実GPU ignoredテスト3件成功。既存ピクセルテストにGroup背景色と見出し文字の確認を追加。
- Schema生成差分チェック、契約Python5件、ACX実stdio5件成功。TauriホストとACX Providerのビルド成功。IPC/Schemaの変更はない。
- RustのSceneにvisible_groupsを追加。Sceneを全フィールド指定で構築するホストは新フィールドを初期化するか、Scene::default()を併用する。
- Group編集/ノード移動時はGroupIndexを再構築する。大規模Groupの性能、ネイティブ画面操作、Windows/Linux、Group選択/編集UI、アクセシビリティは今回検証または実装していない。

## 2026-10-05: ノード概要APIとキーボード操作

- workspace/all-features Rust117件成功、GPU3件ignored。追加2件で有界ページ・排他的cursor・3言語・View別選択・古いrevisionの照会/編集拒否・移動/削除/Undo・ラベル上限とフォールバックを確認。
- workspace/all-features/all-targets clippy -D warnings、fmt、diff check成功。既存block 0.1.6のfuture compatibility警告は継続。GPUコードは変更せず、今回実GPUテストは再実行していない。
- Schema生成差分チェック（13 artifacts）、契約Python5件、ACX実stdio5件、TypeScript strict型検査、JS構文検査成功。Tauriホスト/ACX Providerのビルド成功。
- 新しいノード操作UIロジックテストと既存実行UIテストはそれぞれ3言語で成功。移動要求の座標/revision保持、入力中の座標保持、競合時の再送なし、ページ切り替えを確認。CIに追加。
- Browser plugin not availableのため既存Playwright/Chromiumを使用。http://127.0.0.1:8769/index.html の静的ソースをrouteで配信し、Tauri IPC mockで検証。サーバー待受や新規依存の導入は不要。英語/日本語/简体中文、960×850・320×850の6条件でpage title/lang/content、keyboard Tab/Enterによる選択と移動、50/1件ページ切り替え、削除、フォームラベル、アクセシビリティ情報、横はみ出しなし、console/page errorsなしを確認。日本語のスクリーンショットを確認。
- Tauri実画面での新操作、VoiceOver/NVDA、他OS、ネイティブGPU Surfaceのアクセシビリティツリー、WCAG適合は確認していない。Group/Portのキーボード操作は未実装。

## 2026-10-05: Groupの選択と編集

- workspace/all-features Rust120件成功、GPU3件ignored。追加3件でGroup作成/改名/所属追加・除外/削除/所属Node選択、View分離、空Group、無変更の履歴維持、5編集のUndo/Redo、競合/重複ID/欠落ID/名称/空選択の原子的拒否、105Groupのページ境界と概要の上限を確認。
- workspace/all-features/all-targets clippy -D warnings、fmt、diff check成功。既存block 0.1.6のfuture compatibility警告は継続。GPU実装は変更せず、実GPUテストは再実行していない。
- Schema生成差分チェック14 artifacts、契約Python5件、ACX実stdio5件、TypeScript strict型検査、JS構文検査成功。TauriホストとACX Providerのビルド成功。ACXの変更なし。
- 新Group UI/既存ノードUI/実行UIのロジックテストはそれぞれ3言語で成功。クリック時点のGroup ID/名称/revision保持、入力名保持、競合時に再送しないことを確認。CIに追加。
- Browser plugin not availableのため既存Playwright/Chromiumを使用。http://127.0.0.1:8769/index.html の静的ソースをrouteで配信、IPC mockで検証。960×850/320×850、3言語の6条件でtitle/lang/content、キーボード作成/改名、所属選択/除外/追加/削除、50/1件ページ切り替え、フォームラベル/アクセシビリティ情報、横はみ出しなし、console/page errorsなしを確認。日本語スクリーンショットを確認。
- Tauri実画面での新操作、読み上げソフト、他OS、10,000超の所属操作の実負荷は未検証。Group枠のHit Test・枠ドラッグ・折りたたみ・OSアクセシビリティツリーは未実装。

## 2026-10-06: Port値マップSchema

- workspace/all-features Rust121件成功、GPU3件ignored。追加Rustテストで必須/空/多重/未知入力、非有限Float、重複Portを確認。schema featureなしのexecutorテストも成功。
- workspace/all-features/all-targets clippy -D warnings、fmt、diff check、TypeScript strict型検査成功。既存block 0.1.6のfuture compatibility警告は継続。
- 契約Python6件成功。13 DataType×Single/Multiple×必須/任意の52定義、3,068件の実Rust Value入力/出力をDraft7Validatorと照合。Resourceの型一致、未知Port、空/欠損、件数、Custom不一致を確認。Schema生成差分チェック15 artifacts成功。
- Definitionへfeatureなしのvalidate_inputs/validate_outputs、任意schema featureのinput_values_schema/output_values_schemaを追加。Valueのwire形はSchemarsから生成。math-port-values.jsonを生成対象に追加し、既存CIの差分/契約テストで検証。
- Scheduler、IPC、ACX、UI、GPU実装の変更はない。実GPU・ネイティブ画面・ACX実stdioテストは今回再実行していない。Schemaは資源の存在/内容/権限を検証しない。任意精度JSONとRust/JavaScriptの数値表現の完全同値性も保証しない。

## 2026-10-07: ノードの表示デザイン

- workspace/all-features Rust123件成功、通常実行でGPU3件ignored。追加2件で役割色と接続線の一致、Portリングの中心と既存port_anchorの一致、320/375/414/768幅の低LOD、密なPort名とヘッダーの重なり回避を確認。
- workspace/all-features/all-targets clippy -D warnings、fmt、diff check成功。既存block 0.1.6のfuture compatibility警告は継続。
- macOSの実GPU ignoredテスト3件成功。ピクセル、Group、接続プレビュー、文字クリップ/重なり、CJK/1x・2x、テーマ往復を検証。格子線テストをドット交点の検証へ更新。
- Galleryを実GPUのsRGB textureで生成し、Dark/Light×英語/日本語/简体中文の6画像で欠損Glyphなし。英語・日本語・简体中文の画像を視認し、曲線の継ぎ目を修正後に再生成。日本語の2テーマをdocs/imagesへ保存。
- Rust契約生成差分チェック15 artifacts、契約Python6件成功。NodeLabelsへホスト用tone/symbol/caption追加。Tauri/ACX/Document/TypeScriptのwire変更なし。
- Tauriネイティブ画面での新しい配色/配置の実操作、他OS、新デザインのGPU/Surfaceフレーム時間は未検証。近景の影・Portリング等でQuad数が増える。Galleryの画像ノードは描画fixtureであり、画像処理Executorの実装ではない。

## 2026-10-07: ノードの設定インターフェイス

- workspace/all-features Rust126件成功、GPU3件は通常実行でignored。新規APIテスト2件で定義/値の同時取得、View認可、revision競合、応答容量、未設定Registry、Batch検証失敗のロールバック、ウインドウ間のUndoを確認。ホストテスト1件で倍率/加算/丸め/バイパス・範囲検証を確認。
- workspace/all-features/all-targets clippy -D warnings、fmt、diff check、TypeScript strict型検査成功。既存block 0.1.6のfuture compatibility警告は継続。
- Schema生成・差分チェック16 artifacts、契約Python6件成功。追加APIはNodeProperties / node_properties。from_registryで設定取得と編集検証に同じRegistryを使用。
- macOS実GPU ignoredテスト3件成功。色を重ねたヘッダーを含むGalleryを2テーマ×3言語で生成、日本語のLight/Dark画像を更新。Port座標とHit Testの契約は維持。
- Browser plugin not availableのため既存Playwright/Chromiumを使用。scripts/test_properties_ui.cjsで、静的UI＋IPC mockの保存、型/範囲/JSON/null/整数精度エラー、任意値の削除、外部更新の下書き保持、言語変更時の入力保持、保存・再読込失敗、スイッチ/選択肢/整数の単一Batch、初期値ボタンを3言語で検証。2テーマ×320/380/768pxで横はみ出しなし、page errorsなし。日本語の両テーマ画像を視認し、数値欄とスイッチのスタイルを修正して再検証。
- 既存のノード操作/Group/実行UIロジックテストも3言語で成功。CIに新しいJSの構文チェックを追加。新規ブラウザーテストは外部のPlaywright環境で実行する。
- 新しいプロパティUIのTauri実プロセス操作、スクリーンリーダー、他OS、GPUフレーム時間は未検証。GPUノードへのHTML入力の埋め込み、画像プレビュー、パネルドッキングは未実装。サンプルは実行可能な6ノード・5接続の数値グラフ。

## 2026-10-09: 構造変更とUndo/RedoのIndex差分更新

- workspace/all-features Rust130件成功、通常実行ではGPU3件ignored。追加4件で削除マーカーのHit Test、削除・再挿入・容量超過時の原子的拒否、ノード削除による接続/Group変更とUndo/Redo、同じIDの再作成によるPort/文字/形状の変更、接続先の差し替え後の移動、追加削除の繰り返し、失敗Batchを検証。既存の多接続移動・128件超の再構築も検証。
- 保持IndexのQuad全バイト・文字・可視件数・空間検索・移動プレビューを全体再構築と照合。Tauri UIとACXの共有Editor・revision・Undo後の空間検索の既存テストも成功。
- workspace/all-features/all-targets clippy -D warnings、fmt、diff check成功。既存block 0.1.6のfuture compatibility警告は継続。変更はRust内のAPIのみで、Schema/TypeScript/ACX wire形式は不変。生成差分チェック16 artifacts成功。
- macOSの実GPU ignoredテスト3件成功。GPU文字、テーマ切替、接続・Groupの描画ピクセルを確認。WGSLや見た目の指定は変更していない。
- release benchmarkを10,000ノード／30,000接続・30回で実行。追加・削除・接続・逆命令のIndex更新を全体再構築と比較。ノード削除の中央値は差分0.001833ms、全体再構築30.000459ms。描画結果の一致もベンチマーク中に検証。条件と生データはPERFORMANCE.md参照。
- ⭕️ [Pending] Windows/LinuxのSurface/入力実機検証は環境がないため保留。GPU/Surfaceのフレーム時間は今回未計測で、次の検証項目として維持。CPU Indexの値を編集全体の応答時間やFPSへ換算しない。


## 2026-10-09: GPU / Surface診断計測

- workspace/all-features Rust130件成功、通常実行でGPU4件ignored。macOS実GPUでignored4件を実行し成功。新規テストはTIMESTAMP_QUERYなしの完了待ち、対応Deviceで同じQuerySetを8フレーム再利用した有効な時間取得、無効Viewport拒否と次回の回復、Validationエラーなしを確認。
- workspace/all-features/all-targets clippy -D warnings、fmt、diff check成功。既存block 0.1.6のfuture compatibility警告は継続。Rust内の診断APIのみ追加し、IPC/Schema/TypeScript wire形式は不変。生成差分チェック16 artifacts成功。
- Tauriのreleaseビルドと専用ネイティブSurface起動モードが成功。Apple M4／Metal、1280×720、DPI倍率1、Fifo、10,000ノード／30,000接続でDark/Light×3言語×近景/全体表示の12条件を実測。各条件5フレームのウォームアップ後30フレーム、計360フレームでスキップ・タイムスタンプ欠損・missing glyph・GPU Validationエラーなし。生データはdocs/benchmarks/2026-10-09-surface.json。
- 同一submit内のquery resolveでは古い終了値が返るケースを観測。描画完了待ち後に別submitでresolveする実装に変更し、debug/releaseの両ネイティブ実行で有効な360サンプルを確認。通常draw/renderはqueryも完了待ちも追加しない。
- 計測には毎フレームの同期と16バイト読込が含まれる。通常の非同期アニメーションFPSやcompositor完了時間は測っていない。大きなカードのアイコン・画像プレビュー、Group、連続操作、同一ウインドウ合成も今回のfixtureには含まない。
- ⭕️ [Pending] Windows/LinuxのGPU/Surface検証は実行環境がないため保留。


## 2026-10-09: macOSの同一ウインドウ合成

- workspace/all-features Rust131件成功、通常実行でGPU4件ignored。新規ホストテストでDPI1/1.25/2と複数の幅における描画領域・入力判定の境界一致を確認。実GPU ignored4件成功。既存のピクセルテストを拡張し、左半分への領域描画が全面描画と一致し、右側が背景色のみになることを全ピクセルで確認。
- workspace/all-features/all-targets clippy -D warnings、fmt、diff check成功。既存block 0.1.6のfuture compatibility警告は継続。契約生成差分チェック16 artifacts成功。IPC JSONは変更なし。Rustのcommand引数はWebviewWindowからWebviewへ変更し、子WebViewのlabelを認可に使用。execution_observerは両方を受け付ける。
- macOSのTauri debugアプリを実際に起動し、Computer Useで操作。1440×800の初期画面とウインドウ拡大後に、ネイティブグラフと右の設定WebViewが合成されることを視認。ノードDragでrevision 0→1、Undoで2、設定の倍率1→2の保存で3を確認。パネルにまたがるDragはrevisionを増やさず取消。パネルのスクロール中にグラフが移動しないことを確認。
- 子WebViewから6ノードの実行が完了し、進捗6/6とrevision 3を表示。Dark/Light切替と英語・日本語・简体中文でGPU名とHTMLの表示を確認。別ウインドウモードでもノード数6→7→6とrevision 0→1→2（追加／Undo）を確認。
- JS構文検査、既存ノード操作／Group／実行のUIロジックテストが3言語で成功。今回はネイティブ合成の検証にComputer Useを使用し、ブラウザーIPC mockによる新しい検証は行っていない。ブラウザーconsoleログの完全取得、スクリーンリーダー、最小化復帰、実モニター間のDPI移動は未検証。
- ⭕️ [Pending] Windows/Linuxの同一ウインドウ合成は実機環境がないため保留。サンプルでは従来の別ウインドウ構成を既定に維持。ドッキング／フローティング、配置保存、インスペクターの選択自動追従は未実装。通常の非同期描画FPSは今回計測していない。

## 2026-10-09: 設定パネルの切り離しと再ドッキング

- workspace/all-features Rust132件成功、通常実行でGPU4件ignored。追加ホストテストで、浮動時のグラフ領域拡張と入力境界、配置変更の排他制御と解除、再ドッキング時の領域復帰、controls WebViewの認可を確認。
- workspace/all-features/all-targets clippy -D warnings、fmt、diff check成功。既存block 0.1.6のfuture compatibility警告は継続。契約生成差分チェック16 artifacts、追加TypeScript宣言のstrict型検査、JS構文検査も成功。今回の変更はホスト/UIで、GPU実装は変更せず、実GPU ignoredテストは再実行していない。
- 新しいパネルUIロジックテストが英語・日本語・简体中文で成功。二重操作の抑止、切り離し成功、ネイティブからの再ドッキング通知、移動後のエラー時の配置再取得、非対応モードの操作非表示を確認。既存ノード操作／Group／実行UIテストも3言語で成功。
- macOSのTauri debugアプリをComputer Useで操作。revision 0で数値を22から77へ変更し、未保存のまま切り離して値と下書き表示が残ることを確認。浮動パネルから保存するとrevision 1になり、閉じるボタンによる再ドッキング後も保存値とスクロール位置を保持。再度切り離し、6ノードの実行完了（6/6）、ボタンによる再ドッキングを確認。
- 最終ビルドで日本語の浮動ウインドウタイトル「UNGE · 設定」を確認。切り離し中はグラフが1440×800のウインドウ全幅へ拡張することを視認。メインウインドウを閉じた後、浮動パネルを含むアプリが終了することを確認。
- 今回の完成範囲は1枚の設定パネルのボタンによる切り離し／再ドッキング。任意のドラッグ配置、複数パネルのタブ化、配置の永続保存は未実装。OS呼び出し失敗の注入テストは行っておらず、その場合の完全な復旧は保証しない。
- ⭕️ [Pending] Windows/Linux、複数モニター間のDPI移動、スクリーンリーダーによる実機検証は環境がないため保留。

## 2026-10-09: パネル配置の保存と復元

- workspace/all-features Rust135件成功、通常実行でGPU4件ignored。追加3件で保存／読込／置換、初期化未完了時の保存抑止、壊れたJSON・未知version・容量超過ファイルの保全、無効な数値、負座標のモニター、DPI変更、モニター消失時のプライマリへの復帰、作業領域内へのサイズ・位置補正を確認。
- workspace/all-features/all-targets clippy -D warnings、fmt、diff check、Tauri debugビルド成功。既存block 0.1.6のfuture compatibility警告は継続。契約生成差分チェック16 artifacts成功。Engine／ACX／TypeScriptのIPC契約は変更なし。GPU実装は今回変更せず、実GPU ignoredテストは再実行していない。
- パネル／ノード操作／Group／実行の既存UIロジックテストはそれぞれ3言語で成功。新規のユーザー向け文言はなく、保存形式と復元・リセット方法をPANEL_DOCKING.mdの3言語で説明。
- macOSのTauri実アプリをComputer Useで起動。浮動パネルを操作し、通常終了後にfloating=true、メイン1440×800／浮動380×800の論理サイズとモニター相対位置をJSONで確認。最終ビルドで再起動し、浮動パネルとドッキングボタンを視認。復元したパネルから6ノードの実行（6/6、revision 0）が成功。再終了後のJSONでメインと浮動の位置・サイズが前回と完全一致。
- 次にボタンでドッキングしてメインウインドウを閉じ、floating=falseと浮動ウインドウの保存サイズ維持を確認。再起動時に同一ウインドウの右パネルと切り離しボタンが表示されることをスクリーンショットで確認し、検証アプリを終了。
- ⭕️ [Pending] Windows/Linux、実際の複数モニター間のDPI変更／接続解除、最小化・最大化・全画面からの復帰は今回の実機検証対象外。強制終了・電源断の耐久性、複数プロセス間の配置競合解決は提供しない。選択ノードへのインスペクター追従は次の実装項目。

## 2026-10-09: 選択ノードへのインスペクター追従

- workspace/all-features Rust136件成功、GPU4件は通常実行でignored。追加テストで未知Viewの拒否、View間の選択分離、一覧の最初の50件に含まれないノードの選択、105件選択時の固定サイズ応答、選択ではrevisionが変わらないこと、他Viewからの削除による選択除去、Undo後の整合性を確認。
- workspace/all-features/all-targets clippy -D warnings、fmt、diff check、ホストdebugビルド成功。既存block 0.1.6のfuture compatibility警告は継続。SelectionSummaryのSchema／TypeScriptを生成し、17 artifactsの差分検査、TypeScript strict型検査、契約Python6件成功。Python検証には既存uvキャッシュのjsonschemaをofflineで使用。ACX契約とGPU実装は変更せず、実GPU ignoredテストは再実行していない。
- Browser plugin not availableのため、既存Playwright／Chromiumでhttp://unge.test/へ静的ソースをroute配信し、IPC mockを使用。新規ブラウザー依存の導入なし。3言語・2テーマ・幅320/380/768pxでページURL/title、内容表示、横はみ出しなし、console/page errorsなしを確認。日本語の下書き保持画面を視認し、案内を文単位の改行へ調整。
- ブラウザーで単一選択の自動読込、ページ外の選択、空／複数選択、未保存値の保持、表示中ノードへの保存後の追従、保存失敗と取得失敗、明示的な切り替え待ち中の入力／保存抑止、保存待ち中の選択変更、古いプロパティ応答の破棄、通知欠落の定期照会による回復、同一選択の再読込抑止、外部revision更新時の下書き保持を検証。既存の型・JSON・任意値削除・Batch・言語変更テストも維持。
- macOSネイティブアプリでGPU上の数値ノードをクリックし、値20／revision 0のプロパティが自動表示されることを確認。未保存の77を入力後、HTMLの選択操作で別の数値調整ノードを選択。元ノードのIDと77、下書き保持の案内が残り、保存するとrevision 1となって数値調整ノードへ表示が追従することを確認。検証アプリを終了。
- 既存パネル／ノード操作／Group／実行UIロジックテストも3言語で成功。新しい選択取得は登録Viewに限定し、IPCにはrevision・選択数・単一IDだけを送る。イベントは再照会の契機とし、保存には既存revision検査を使う。
- ⭕️ [Pending] Windows/Linux、スクリーンリーダーの実機操作、別ウインドウモードでの今回の追従操作は未検証。未保存フォームのプロセスをまたぐ永続化、複数ノードへの一括プロパティ編集は未実装。


## 2026-10-11: 通常描画と連続操作のCPU計測

- workspace/all-features Rust137件成功、通常実行でGPU4件ignored。新規の10,000ノード／30,000接続の連続入力テストで、Pan／ZoomがDocumentを変更しないこと、Zoom範囲、Drag途中のプレビューと13ステップごとの単一Command確定、5回の編集後の座標、各確定後の描画Indexと全体再構築のQuad一致を確認。
- 実GPU ignored4件成功。通常サンドボックスではMetal Adapter取得不可だったため、許可されたデスクトップGPU環境で再実行。既存のピクセル・領域描画・テーマ／文字・GPU診断テストを確認。WGSL・描画形状は変更していない。
- workspace/all-features/all-targets clippy -D warnings、fmt、diff check成功。17件の生成契約チェックで差分なし。既存block 0.1.6のfuture compatibility警告は継続。IPC／TypeScript／ACXのwire形式、UI文言は変更なし。取り込み文書には日本語・英語・简体中文の説明を追加。
- Tauri releaseビルド（thin LTO）と `--benchmark-animation` が成功。Apple M4／Metal、macOS/aarch64、1280×720、倍率1、Fifo。Dark/Light×3言語×Pan/Zoom/Dragの18条件で、各5成功フレームの予熱後60フレーム、計1,080サンプル。Surfaceスキップ0、欠字0、GPU Validationエラーなし。専用プロセスは計測後にexit 0で終了。
- 実Surface上でゼロサイズの通常／計測drawと空の描画領域がスキップし、範囲外領域がエラーとなり、サイズ復元後に通常経路の描画が回復することを確認。OSからLost/Outdated/Timeoutを強制発生させるテストは行っていない。
- 生JSONの18条件の一意性、各60サンプル、非負かつ有限の全時間、工程の時間と総時間の大小関係、開始頻度の再計算、Drag各5回／他の操作0回のCommand確定を確認。Dragの更新時間上位5件はすべてUp時。CPU更新p95は12.747〜20.665ms、ZoomのScene生成p95は6.034〜10.841ms。[結果とJSON](PERFORMANCE.md)。
- 通常APIは時計取得を省略し、計測APIもGPU完了待ち・timestamp query・readbackを追加しない。タイミングはCPUのSurface取得／準備／encode-submit／presentと、入力・編集・Index／Scene生成に分離。成功開始間隔にはイベントループとスキップした試行を含める。サンプル数・試行数・ターン間の期限を制限し、JSONは条件末尾で集計。
- ネイティブ合成入力を直接Interactionへ送る計測で、実マウス入力やWebView／共有Engine競合は対象外。画面表示のFPS、GPU完了、OS compositor、入力から表示までの遅延、長時間安定性を確認したものではない。HTMLは未変更のためブラウザーUIテストは今回は再実行していない。
- ⭕️ [Pending] Windows/Linux、実モニターでの表示完了・入力から表示までの遅延の検証は環境がないため保留。

## 2026-10-11: 移動確定時の全体検証を限定

- workspace/all-features Rust140件成功、通常実行でGPU4件ignored。追加3件で、移動のみの入れ子Batch・同一IDの複数回移動・履歴上限・Undo／Redoを、毎回全体検証する参照Editorと比較。Document・revision・両履歴の件数と容量、最終状態の全体検証が一致。
- 不正なNaN／範囲外座標、存在しないノード、循環接続、未知ノードを含むGroupを混在させたBatchが原子的に失敗し、Redoも失わないことを確認。プロパティを含むBatchは全体検証へ戻ることも確認。
- 位置依存のホスト検証と拒否ゲートを使用し、通常／計測execute、Undo、Redoすべてで検証が実行され、拒否時にはDocument・revision・履歴を保持することを確認。任意ホスト検証とRegistry検証は省略しない。
- `execute_timed` の工程が有限・非負で、合計が総時間を超えないことを確認。通常APIでは時計取得を省略。APIはRust内に追加し、Command／Tauri／ACXのwire形式は変更なし。
- workspace/all-features/all-targets clippy -D warnings、fmt、diff check成功。生成契約17件に差分なし。既存block 0.1.6のfuture compatibility警告は継続。GPU実装とHTMLは未変更のため、GPU ignoredテスト・ブラウザーUIテストは今回は再実行していない。
- 10,000ノード／30,000接続、1／100ノード移動Batchについて、同じ工程計測を追加した最適化前後のrelease CPU診断を実行。各5回予熱＋30回計測。毎回Undoして元のDocumentに一致することを確認し、全体検証を測定区間外で実行。1ノード確定中央値7.878584→0.002333ms、100ノード8.037000→0.016417ms。ホスト検証未登録のfixtureであり、Registryを使う実アプリ全体の値ではない。[全工程と生データ](PERFORMANCE.md)。

- 最終releaseホストで `--benchmark-animation` を再実行しexit 0で終了。Apple M4／Metal、1280×720、倍率1、Fifo、18条件×60＝1,080サンプルでスキップ・欠字・GPU Validationエラーなし。全条件の一意性、試行と成功件数、有限かつ非負の各工程、総時間との整合性、Drag各5回／Pan・Zoom各0回のCommand確定をJSONで確認。
- 通常Surface計測のDrag更新p95は前回12.747〜20.665msから0.030583〜0.035125msへ短縮。これはRegistry未登録の専用fixtureでのInteraction・編集・履歴・Index更新の値であり、実アプリ全体の応答時間や表示FPSではない。各工程の範囲と残るRegistry／多数ノードのIndex再構築費用を3言語で文書化。
- ⭕️ [Pending] Windows/Linux、実モニターの表示完了・入力から表示までの遅延の検証は環境がないため保留。

## 2026-10-11: Registryの遷移検証

- 🟢 [Done] `cargo test --workspace --all-features --locked`：144件成功、失敗0。compile-fail doctestを含む。通常実行でGPU4件ignored。今回はcore／executorの検証経路のみ変更し、GPUテストは再実行していません。
- 🟢 [Done] 追加3件の実行テストと1件のcompile-fail doctest。Editorだけが作る `ValidatedEdit` の外部生成拒否、core検証後の通知、移動／混在Batchの分類、拒否時のexecute／Undo／RedoのDocument・revision・履歴保持を確認。
- 🟢 [Done] 最適化Registryと全体検証ラッパーに同じ操作列を適用し、結果・Document・revision・履歴を照合。入れ子移動、不正プロパティ、Batch内での修復、未知型、Port不一致、必須値削除、空Batch、Undo／Redoを含む。直接呼び出しで循環Graphを拒否し、履歴のないEditorへのvalidator設定でも不正な読込プロパティを拒否。
- 🟢 [Done] `cargo fmt --all -- --check`、`cargo clippy --workspace --all-features --all-targets --locked -- -D warnings`、生成契約17件の `--check` 成功。Rust内のcallback追加であり、IPC／TypeScript／ACXのwire契約変更なし。
- 🟢 [Done] Registry付きrelease CPU比較6条件×30サンプル。各5回予熱、毎回Undoと独立全体検証。180サンプルの有限値・非負値、工程合計、core検証分類、統計の再集計を確認。[生JSON](benchmarks/2026-10-11-registry.json)。1ノード移動中央値10.767083→0.003625ms、100ノード12.124834→0.029959ms、1プロパティ28.937375→15.279250ms。Index／Scene／GPU／IPC／ロック／ACX Preflightは含まない。
- 🟢 [Done] 3言語の統合契約とROADMAPを更新。前節のRegistry費用は今回の別計測で改善を確認。多数移動のIndex再構築、ZoomのScene生成は未対応。
- English: 144 Rust tests including one compile-fail doctest passed; four GPU tests were ignored and were not rerun for this CPU-only change. Formatting, strict workspace Clippy and 17 generated contracts passed. Six benchmark cases retain 180 validated samples. Registry-enabled timings exclude rendering, Index, IPC, locks and ACX preflight; they are not application latency or FPS.
- 简体中文：144项Rust测试通过（含1项编译失败验证），4项GPU测试默认忽略，本次仅修改CPU验证路径，未重跑GPU。格式、严格workspace Clippy及17项生成契约检查通过。6组基准共180个样本已校验。Registry计时不含渲染、Index、IPC、锁或ACX预检，不代表应用延迟或FPS。

## 2026-10-11: 一括移動のBVH境界更新

- 🟢 [Done] `cargo test --workspace --all-features --locked`：148件成功、失敗0、GPU4件は通常実行でignored。最終のSpatialIndexテスト5件も成功。`cargo fmt --all -- --check`、workspace/all-features/all-targetsのClippy `-D warnings`、生成契約17件の確認に成功。
- 🟢 [Done] 実GPUテスト4件を明示実行して成功。ピクセル／曲線／文字のクリップ・重なり・2倍率のCJK／テーマ切り替えを確認。Surface連続操作の計測は今回は再実行していない。
- 🟢 [Done] 追加4件のCPU回帰テスト。refitの原子的失敗、未知／差分だけのID、不正矩形、4,096累積ID上限、削除マーカーとの併用、検索とHit Testを総当たり結果と比較。Group・サイズ変更・遠距離移動・入れ子Batch・同一ID反復・Undo／Redoを全体再構築のQuad／文字／検索と照合。
- 🟢 [Done] 1,024ノードで境界更新、1,025ノードと構造編集混在は全体再構築。4,299接続を持つノードは移動／Undoとも全体再構築し、接続を切り捨てない。既存の小変更・構造変更・容量超過・失敗Batchの回帰テストも成功。
- 🟢 [Done] releaseのCPU比較3条件×30サンプル、各5回予熱。同じ最終Documentに対するIndex更新と全体再構築を計時し、3つの近景の検索／Sceneも別に計測。毎回Undo・Document一致・全体検証・Quad／文字／可視件数／検索／Hit Testを照合。90サンプルの有限・非負値と統計の再集計を確認。[生JSON](benchmarks/2026-10-11-bulk-moves.json)。
- 🟢 [Done] 100ノード更新中央値1.916625ms（同状態の全体再構築18.191417ms）、1,000ノード4.056542ms（再構築16.647125ms）。近景検索は再構築より数µs、Sceneは約0.03ms長い結果も記録。性能fixtureはGroupなし、移動距離96×36。Command・検証・履歴・Registry・GPU／Surface・IPC・共有ロック待ちは計時外。全体の表示FPSと任意の並べ替え性能は未検証。
- 🟢 [Done] 3言語の取り込み契約、既存Index更新手順、性能文書、ROADMAPを更新。Rustの派生Index内の変更であり、IPC／TypeScript／保存形式は変更なし。
- English: 148 workspace Rust tests and four explicitly run GPU tests passed. Formatting, strict workspace Clippy and 17 generated contracts passed. Ninety CPU samples compare bounded bulk refitting with same-state reconstruction and independently check rendering/query/history correctness. Query cost after arbitrary rearrangement and display FPS remain unverified.
- 简体中文：148项workspace Rust测试及4项显式GPU测试通过。格式、严格Clippy及17项生成契约检查通过。90个CPU样本比较有界批量边界更新与同状态重建，并独立验证渲染／查询／历史正确性。任意重排的查询成本及屏幕FPS尚未验证。

## 2026-10-13: Sceneの可視ID処理

- 🟢 [Done] workspace/all-features Rust150件、明示実行した実GPU4件が成功。fmt、workspace/all-features/all-targets Clippy `-D warnings`、生成契約17件の確認に成功。
- 🟢 [Done] 可視IDとプレビュー候補の和集合を従来のBTreeSetと比較し、空・重複・逆順・10,000 IDで描画順と重複除去を確認。既存の移動・構造変更・Group・プレビュー・Undo／Redoの描画比較も成功。
- 🟢 [Done] 6段階Zoom、変更前後それぞれ5回予熱＋30回計測。Quad／文字のFNV指紋・Quad数・可視Node／Edge数が一致。360サンプルの有限値・非負値と中央値／p95の再集計を確認。[生データ](benchmarks/2026-10-13-zoom.json)。全体表示中央値3.527250→3.275500ms。静的ZoomのCPU Scene生成のみで、連続Surface／GPU／IPC／共有ロックは含まない。
- 🟢 [Done] 3言語の取り込み文書、性能記録、ROADMAPを更新。IPC／TypeScript／GPUの形式変更なし。連続Zoomと残る曲線・形状参照の工程計測はNext。
- English: 150 Rust tests, four explicit GPU tests, formatting, strict Clippy and 17 contracts passed. Six zoom cases preserve output fingerprints and visible counts; 360 before/after CPU samples were checked. These static scenes do not establish continuous Surface performance or FPS.
- 简体中文：150项Rust测试、4项显式GPU测试、格式、严格Clippy及17项契约检查通过。六档Zoom输出指纹及可见数量一致，已检查修改前后360个CPU样本。静态Scene不代表连续Surface性能或FPS。

## 2026-10-14: Scene工程計測とEdge情報の保持

- 🟢 [Done] workspace/all-features Rust152件、明示実行した実GPU4件、fmt、厳格Clippy、生成契約17件の確認に成功。
- 🟢 [Done] 追加2件で計測／通常SceneのQuad・文字・可視件数を2テーマ×3言語×3Zoomで比較。複数移動プレビューと選択矩形を含み、不正Viewport拒否、時間の有限／非負値・工程合計を確認。接続元ノード再作成、Undo／Redo、Catalog変更後のEdge表示を全体再構築と照合。
- 🟢 [Done] 静的Zoom6条件で通常CPU時間と別の工程診断を各30回（予熱5回）実行。変更前後のQuad／文字指紋、Quad数、可視Node／Edge数が一致。通常360＋工程360サンプルの統計・有限／非負値・工程合計を確認。[生データ](benchmarks/2026-10-14-scene-phases.json)。全体表示中央値4.830667→2.717875ms。GPU／IPC／共有Engineは計時外。
- 🟢 [Done] release Tauriホストの通常Surfaceで18条件×60＝1,080サンプルを確認。M4／Metal、1280×720、倍率1、Fifo。スキップ・欠字・GPU Validationエラーなし。連続ZoomのScene p95は4.445667〜6.336584ms。[生データ](benchmarks/2026-10-14-animation-scene.json)。前回は依存／実行時点が異なるため高速化率は算出しない。
- 🟢 [Done] Rust用SceneTimings APIと通常APIの時計なし経路、Edge保持データの追加メモリー・構築費用、計測範囲を3言語で文書化。IPC／TypeScript／保存形式変更なし。
- ⭕️ [Pending] Windows/Linuxの実GPU／Surface、表示完了と入力から表示までの遅延の検証は環境がないため保留。
- English: 152 Rust and four explicit GPU tests passed, along with formatting, strict Clippy and 17 contracts. Before/after static scenes preserve fingerprints/counts. Native queued rendering completed 1,080 samples with no skips, missing glyphs or validation errors. Scene CPU timings exclude display latency and shared Engine contention; additional retained-edge memory and construction cost are documented.
- 简体中文：152项Rust及4项显式GPU测试、格式、严格Clippy与17项契约检查通过。静态Scene修改前后指纹／数量一致；原生普通渲染完成1,080样本，无跳过、缺字或验证错误。Scene CPU计时不含屏幕延迟及共享Engine竞争；已记录Edge保留信息新增内存和构建成本。

## 2026-10-14: Tauri関連依存の一式更新

- 🟢 [Done] Tauri 2.12.3、runtime／runtime-wry 2.12.1、build／macros／codegen 2.7.2、utils 2.10.1、Wry 0.57.0、Tao 0.37.1の組み合わせでoffline解決とホストビルドが成功。以前のtrait／生成API不整合を解消。[lock差分と検証範囲](DEPENDENCY_UPDATES.md)。
- 🟢 [Done] 最終ソースでworkspace/all-features Rust152件成功、GPU4件は通常実行でignored。GPU実装は変更せず、新ホストの実Surfaceで別途検証。fmt、厳格workspace Clippy、生成契約17件、Panel／Group／Accessible／Executionの各3言語UI回帰も成功。
- 🟢 [Done] M4／Metal、1280×720、倍率1、Fifoでdebug Surface12条件×30＝360サンプル成功、スキップ0、欠字／GPU Validationエラーなし。性能認定や前後速度比較には使わない。[生データ](dependencies/2026-10-14-tauri-surface.json)。
- 🟢 [Done] CUAでmacOS実アプリを確認。tauri://localhostの非空画面、同一ウインドウのGPUグラフとWebView、切り離し、浮動パネルからのノード追加、Undo、再ドッキング、グラフ上の40×30のドラッグ、設定表示の追従が成功。6→7→6ノード、revision 0→1→2→3で共有編集を確認。
- 🟢 [Done] 一時保存先へfloating=trueを書き出して通常終了、別プロセスで再起動し浮動ウインドウ「UNGE · 設定」とドッキングボタンを確認。検証アプリは終了。一時 .app のmacOS sandbox-extension警告は依存記録に残した。Browser pluginは未提供、対象がnative Tauriアプリのためブラウザー代替ではなくCUAを使用した。
- 🟢 [Done] 任意のUNGE_PANEL_PREFERENCES保存先、3言語の移行記録、ROADMAP、GUIのアイコン／コンパクト配置方針を取り込み規約へ反映。
- ⭕️ [Pending] Windows/Linux、複数モニター、全Window寸法、正式署名付き配布バンドル、WASM、最低Rust版の検証。
- English: 152 Rust tests, strict Clippy, formatting, 17 contracts and three-language UI checks pass. Updated Tauri renders 360 native Surface samples without skips. CUA confirms shared edits, floating/docking, dragging and restored floating mode. Temporary-bundle warning and unverified platforms/distribution cases are documented.
- 简体中文：152项Rust测试、严格Clippy、格式、17项契约及三语UI检查通过。更新Tauri完成360个原生Surface样本，无跳过。CUA验证共享编辑、浮动／停靠、拖动及浮动模式恢复。临时bundle警告与未验证平台／分发情况已记录。

## 2026-10-14: コンパクトなWebView設定パネル

- 🟢 [Done] 既存のパネルを32pxのSVGアイコン操作へ整理し、プロパティを実行・ノード一覧より上へ配置。説明と型／版／revisionは情報アイコンで表示。13px文字、16px左右余白、意味単位の改行、ダーク／ライトの視認性を画面で確認。任意の未設定値は入力欄を畳み、draftを保持。
- 🟢 [Done] 3言語×2テーマ×320／380／768pxのPlaywright検証成功。横方向のはみ出し、アイコンのaria-label／title、hover／focus／click、Escと再表示、開いた説明のリサイズ追従、切り離し／再ドッキングのアイコン変化を確認。既存の保存・不正値・競合・draft・JSON・選択変更／race／復旧の回帰も成功。Browser pluginが提供されていないため、既存Playwrightとインストール済みChromeを使用。
- 🟢 [Done] macOSのTauri実画面でアイコンから追加／Undo（6→7→6ノード）、切り離し／再ドッキング、数値プロパティ22→24の保存とUndoによる22への復帰、revision更新、情報表示とEscによる閉鎖を確認。GPUグラフとWebViewが同じウインドウに表示。リサイズ追従はブラウザーで検証。
- 🟢 [Done] workspace/all-features Rust152件成功、fmt、厳格workspace/all-targets Clippy、生成契約17件確認、Panel／Group／Accessible／Executionの3言語回帰が成功。今回はUIのみ変更し、GPU4件は通常実行でignored、明示再実行なし。Clippyの依存block 0.1.6の将来互換警告は残る。
- 🟢 [Done] 見出し・保存済み・選択案内を3言語で短く更新。エラー／競合表示とCommand／expected_revisionを維持。再利用するDOM補助APIと破棄手順を[COMPACT_PANEL.md](COMPACT_PANEL.md)に記載。
- ⭕️ [Pending] Windows／Linux実画面、実スクリーンリーダーは環境がないため保留。WCAG適合認定や性能改善の数値は今回の検証範囲に含めない。
- English: Compact SVG controls, priority property placement and bounded information popups pass three-language, two-theme, three-width browser checks. macOS Tauri confirms property save/Undo and floating/docking. All 152 Rust tests, formatting, strict Clippy and 17 contracts pass. Windows/Linux and real screen readers remain unverified.
- 简体中文：紧凑SVG控件、优先属性布局及信息弹窗通过三语、双主题、三宽度浏览器检查。macOS Tauri验证属性保存／撤销与浮动／停靠。152项Rust测试、格式、严格Clippy及17项契约检查通过。Windows／Linux及真实屏幕阅读器尚未验证。
