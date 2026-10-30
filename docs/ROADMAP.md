# Roadmap

- 🟢 [Done] implemented in the current codebase
- 🟠 [Next] high-priority unfinished work
- 🔴 [Later] planned, but not the closest next step
- ⭕️ [Pending] cannot be tested in the current dev environment; on hold

## コアと編集

- 🟢 [Done] UUID、型付きPort、Single/Multiple入力、明示的接続、DAG検証
- 🟢 [Done] 論理Graphと表示座標の分離、複数DocumentのWorkspace、資産参照
- 🟢 [Done] Command、Batchの原子性、逆命令によるUndo/Redo、履歴件数制限
- 🟢 [Done] ノード削除時のEdge/Group整合性、プロパティ更新、Groupデータ
- 🟢 [Done] IDを再発行するコピー＆ペースト、Left-to-right階層レイアウト
- 🟢 [Done] BVHの範囲検索・Hit Test、Viewport Pan/Zoomの座標変換
- 🟢 [Done] スキーマバージョン付きJSON、未知バージョンの拒否、安定した保存順序
- 🟢 [Done] 独立したPointer状態機械、選択/複数Node Drag/Box Select/双方向Port接続、Pan
- 🟢 [Done] Rust内の一時プレビュー、Up時の単一Command確定、競合・Cancel・ポインター所有権検証
- 🟢 [Done] Undo/Redo合算のバイト容量制限、容量超過時の履歴連続性維持、保持量照会
- 🟢 [Done] PropertySchemaの型・範囲・選択肢・必須値・初期値、Registryによる編集/Undo/Redo時の原子的検証
- 🔴 [Later] JSONバージョンMigration、Binary形式、Semantic Diff
- 🔴 [Later] Subgraph、Frame、Comment、他の自動レイアウトとEdge Routing

## 実行

- 🟢 [Done] Definition/Instance分離、意味情報の3言語対応、信頼済みExecutor登録
- 🟢 [Done] プロパティ定義の妥当性検証、3言語メタデータ、生成時の初期値適用
- 🟢 [Done] 入出力スキーマ検証、必須入力検証、非同期DAGの階層並列実行、並列数制限
- 🟢 [Done] Completed/Cached/Failed/Blocked/Cancelledレポート、独立分岐の継続
- 🟢 [Done] pureノードのFIFOキャッシュ、変更入力からの再実行、下流探索
- 🟢 [Done] 協調キャンセル、ホストのリソースストア向け型付きID
- 🟢 [Done] 完全キーのハッシュ検索、FIFOの件数＋バイト容量制限、使用量照会、実行時の上限変更、executor識別子によるキャッシュ分離
- 🟢 [Done] Rustホスト向け開始/終了/ノード進捗イベント、ホストFutureによるrun期限、待機中のキャンセル通知、未完了結果のCancelled記録
- 🟢 [Done] run ID/revision付き共有実行サービス、1実行の予約、snapshot/概要保持上限、破棄・panic時の予約解放
- 🟢 [Done] Tauriの開始/現在実行/進捗照会/キャンセルAPI、集約通知、TypeScript型と3言語文言
- 🟢 [Done] ACX承認済み実行の共有サービス接続、ホスト向け進捗通知、UIとの二重実行防止、既存Receipt再送契約維持
- 🟢 [Done] 3言語の実行/キャンセル操作UI、進捗表示、snapshot revisionの区別、通知欠損時の照会回復
- 🟢 [Done] 任意ACX非同期job Profile、承認済みcommitの開始/進捗照会/キャンセル、単一worker、終端Receiptと再送、Provider終了時のキャンセル
- 🟢 [Done] Rustのグラフ・実行値・定義・進捗・Tauri IPCからTypeScript入力/出力型とJSON Schema生成、プロパティ値制約のSchema公開、CI差分チェック
- 🟢 [Done] DefinitionごとのPort名/型/必須性/入力個数を反映した値マップSchema、Rust検証API、3,068例の判定照合
- 🔴 [Later] Converter Registry、Remote/Python/AI/GPU Backendの標準アダプター
- 🔴 [Later] Streaming、Reactive、Simulation、Subgraph実行
- 🔴 [Later] 外部Plugin manifest、WASMサンドボックス、権限モデル

## 描画と統合

- 🟢 [Done] Rust所有のSceneIndex、GPUインスタンスバッファ、WGSLパイプライン
- 🟢 [Done] Node/Port/選択枠/Grid/Bezier Edge描画、NodeとEdgeのBVHカリング、簡易LOD
- 🟢 [Done] TextureViewへの描画、ネイティブSurface、Resize/Timeout/Lostへの処理
- 🟢 [Done] Tauri 2共有Engine、呼び出し元View検査、revision競合検出、変更Summaryイベント
- 🟢 [Done] TypeScriptクライアント、3言語のエラー辞書、CLIとTauri実行例
- 🟢 [Done] GPU文字描画（Glyph Atlas）、3言語のホスト表示名、View単位の言語、HiDPI・クリップ・重なり順・容量制限
- 🟢 [Done] 参考画像に基づくドット背景・役割色・ノード記号/説明文・Portリング・選択枠・SDF影、3言語実GPU Gallery
- 🟢 [Done] ダーク/ライトのテーマAPI、Rust View単位の設定、GPU/CSS共通配色、3言語の切り替えUIと設定復元
- 🟢 [Done] Group枠・見出しのGPU描画、BVHカリング、ドラッグ追従、テーマ/LOD、編集後の整合性
- 🟢 [Done] 役割色のヘッダー、PropertySchemaから生成する3言語の設定パネル、有界な設定取得API、Batch保存・検証・競合時の下書き保持、ホスト拡張の数値調整サンプル
- 🔴 [Later] ネイティブノード内の入力コントロール、画像資産プレビュー、設定項目の表示ヒント
- 🟢 [Done] 有界なノード概要API、3言語のHTML一覧、キーボードで選択/座標移動/削除、revision競合拒否
- 🟢 [Done] Group概要のページ取得、選択から作成/改名/所属追加・除外/削除、所属Nodeの一括選択、3言語HTML操作とUndo/Redo・競合検証
- 🔴 [Later] Group枠のHit Test・枠ドラッグ・折りたたみ
- 🔴 [Later] ネイティブSurfaceのアクセシビリティツリー、Port/Groupのキーボード操作、読み上げ実機検証とWCAG評価
- 🔴 [Later] 任意のテーマ登録、OS配色/高コントラストへの自動追従
- 🟢 [Done] 10000ノード/30000エッジの再現可能なCPU計測、BVH中央値分割による構築改善、総当たり検索との回帰検証
- 🟢 [Done] 小さな移動のノード/接続Edge Index差分更新、プロパティ編集時の形状Index維持、Group編集時のGroup専用Index更新、128件上限と全体再構築への切り替え、UI/Pointer/ACXへの共通適用
- 🟢 [Done] 形状/ズームに応じたBezier適応分割、誤差目標と分割上限、逆向き/端点一致の検証、実GPU曲線ピクセル検証
- 🟢 [Done] 小さなノード/接続の追加・削除・再接続、Undo/Redo・ACX RecoverのIndex差分更新。容量超過は全体再構築
- 🟢 [Done] 明示的なGPU/Surface診断APIと12条件のネイティブ計測。通常描画の完了待ちは追加しない
- 🟢 [Done] 通常描画のCPU計測APIとPan／Zoom／Dragの18条件計測。[結果と範囲](PERFORMANCE.md)
- 🟢 [Done] Tauri/Taoのネイティブ入力ブリッジ、HiDPIの論理座標、ホイール拡大縮小（macOS実操作確認）
- ⭕️ [Pending] Windows/LinuxのネイティブSurface/入力検証（現在はmacOS開発環境のみ）
- 🟢 [Done] macOSの同一ウインドウ合成、子WebViewの呼び出し元認可、領域描画・入力境界・DPI計算。別ウインドウ互換経路を維持
- ⭕️ [Pending] Windows/Linuxの同一ウインドウ合成と実DPI切り替え検証
- 🟢 [Done] 設定パネルのボタンによるドッキング／フローティング、同じWebViewの再配置、Rustの配置状態と排他制御、閉じる操作で再ドッキング（macOS）
- 🟢 [Done] 通常終了時のパネル配置保存と次回起動時の復元、画面内への補正、不正な設定の保全（macOS）
- 🟢 [Done] 選択ノードへのインスペクター追従、未保存の入力保持、空／複数選択案内、古い応答の除外と通知欠落回復
- 🔴 [Later] ドラッグによる任意位置へのドッキング、複数パネルのタブ化
- ⭕️ [Pending] Windows/Linuxのパネル再配置と複数モニターでの実機検証
- 🔴 [Later] GPU不可時のフォールバック、ブラウザーWASMホスト
- 🔴 [Later] Minimap、検索、Smart Connection、CRDT共同編集、Git連携

設計書の「10000 Nodes / 30000 Edges / 60 FPS」のうちCPU処理、GPU/Surfaceの直列診断、通常描画での合成連続操作を計測しました。画面表示の60FPSは未検証です。詳細は [PERFORMANCE.md](PERFORMANCE.md)。

## ACXによるAI操作

- 🟢 [Done] 実験的Node Graph Profile、ACX Manifest/Receiptに適合するRust Provider
- 🟢 [Done] プロパティ制約付きDefinition照会、生成時の初期値、Preflightでの編集後プロパティ検証
- 🟢 [Done] ページ付きノード定義・グラフ照会、型付きノード生成、接続・削除・移動・プロパティ・Group・レイアウト操作
- 🟢 [Done] Preflightの無変更プレビュー、ホストポリシー承認、digestとrevisionに固定したCommit/Execute
- 🟢 [Done] 同一ロック内の編集、失敗を含む結果の再送、条件付きRecover、元Receiptを維持した回復証明
- 🟢 [Done] Rust/Python間の正確なハッシュ材料照合、型付きTypeScriptクライアント、3言語エラー辞書
- 🟢 [Done] Headless JSON Lines Provider、Tauriの共有Documentへ接続する `--acx-stdio` モード
- 🟢 [Done] acx側へのProfile仕様・Schema・例・テスト追加、UNGEへの持ち運び可能なSchema同梱
- 🟢 [Done] 期限切れ未実行preflightの自動／明示的回収、Rustホストの保持件数照会。実行済み結果・Receipt・jobの再送を維持。[保持運用](ACX_RETENTION.md)。
- 🟢 [Done] 任意の永続ジャーナルによる開始guard・結果/Receipt保存と再起動後の再送、単一writer排他、実行中を拒否するProvider epoch切り替え。[有効化と復元範囲](ACX_DURABILITY.md)。
- 🟠 [Next] 別実装Providerとの相互運用、Document/Undo保存とのトランザクション連携、長期ログの圧縮・退避運用
- 🔴 [Later] MCP/A2A binding、署名、多ユーザー認証、外部副作用や課金を伴う実行Profile

## 今回の優先順位（2026-10-05）

- 🟢 [Done] AIと通常UIの双方に効く編集時検証、履歴容量制限を先行。統合手順は [PROPERTY_VALIDATION.md](PROPERTY_VALIDATION.md)。
- 🟢 [Done] Pointer状態機械とDrag/接続操作。取り込み手順は [POINTER_INPUT.md](POINTER_INPUT.md)。
- 🟢 [Done] GPU文字描画。型名・Port名の3言語表示、ホストのフォント注入。取り込み手順は [GPU_TEXT.md](GPU_TEXT.md)。
- 🟢 [Done] ダーク/ライト切り替え。取り込み手順は [THEMES.md](THEMES.md)。
- 🟢 [Done] 実行キャッシュのハッシュ検索とバイト容量制限。大きすぎる結果を保持せず、件数と容量の両方でFIFOを管理する。
- 🟢 [Done] Rustホストへの進捗通知とrun期限。取り込み手順は [EXECUTION_PROGRESS.md](EXECUTION_PROGRESS.md)。
- 🟢 [Done] run ID/revision付き実行サービスとTauri操作API、ACXのホスト接続。取り込み手順は [EXECUTION_SERVICE.md](EXECUTION_SERVICE.md)。
- 🟢 [Done] 実行ボタンと進捗UI。通常操作と実行状態を分け、通知欠損時は照会で回復する。
- 🟢 [Done] ACX非同期job Profile。取り込み仕様は [ACX_ASYNC_JOBS.md](ACX_ASYNC_JOBS.md)。
- 🟢 [Done] Rust契約から型・Schema生成。[取り込み手順](CONTRACT_GENERATION.md)。
- 🟢 [Done] 大規模グラフのCPU実測とBVH構築改善。[PERFORMANCE.md](PERFORMANCE.md)。
- 🟢 [Done] 小さな編集のIndex差分更新。[取り込み手順](INDEX_UPDATES.md)。
- 🟢 [Done] 全体表示時の曲線生成量を改善。[CURVE_TESSELLATION.md](CURVE_TESSELLATION.md)。
- 🟢 [Done] Group描画。[取り込み手順](GROUP_RENDERING.md)。
- 🟢 [Done] アクセシビリティの初期対応。[ノード概要とHTML操作](ACCESSIBILITY.md)。
- 🟢 [Done] Groupの所属Node選択と編集。[取り込み手順](GROUP_EDITING.md)。
- 🟢 [Done] DefinitionごとのPort名/型/必須性/入力個数を反映した値マップSchema、Rust検証API、3,068例の判定照合。

## 次の優先項目（2026-10-06）

- 🟢 [Done] Port値マップSchema。[取り込み手順](PORT_VALUE_SCHEMAS.md)。
- 🟢 [Done] トポロジー/Undo/RedoのIndex更新費用を削減し、CPU計測で検証。2026-10-09の結果を参照。

## 2026-10-07: 表示デザイン

- 🟢 [Done] ノードの表示を更新。[ホスト指定と見本生成](NODE_APPEARANCE.md)。
- 🟢 [Done] 追加・削除・Undo/RedoのIndex更新費用を削減。
- 🟢 [Done] 新デザインの基本カードを用いた実GPU/Surface診断計測。[方法](RENDER_PROFILING.md)。大きなカードや画像プレビューの負荷は含まない。

- 🟢 [Done] [ノード設定インターフェイス](PROPERTY_INSPECTOR.md)を追加。読み込んだrevisionで適用し、1回のUndoで復元する。

## 2026-10-09: 編集と履歴の描画更新

- 🟢 [Done] 削除マーカー付きBVH差分、関連Edgeの更新、最終Batch状態への追従、Undo/Redo逆命令の参照API。
- 🟢 [Done] 全体再構築と描画/Hit Testの照合、削除・再追加・再接続・履歴・容量超過の回帰検証。
- 🟢 [Done] 10,000ノード/30,000接続で小さな構造変更とUndoのCPU Index更新を30回計測。[条件と数値](PERFORMANCE.md)。
- 🟢 [Done] GPU/Surfaceを含む直列診断計測。[再利用APIと計測手順](RENDER_PROFILING.md)。
- 🟢 [Done] macOSの同一ウインドウ合成。[取り込み方法](WINDOW_COMPOSITION.md)。
- 🟢 [Done] 通常描画・連続操作のCPU計測。2026-10-11の結果を参照。

## 2026-10-09: 設定パネルの切り離し

- 🟢 [Done] 未保存の値を保持した切り離し／再ドッキングと、浮動パネルからの保存・実行。[取り込み手順](PANEL_DOCKING.md)。
- 🟢 [Done] 3言語の操作・エラー表示、ホスト専用IPCのTypeScript宣言、二重操作と失敗後の状態再取得テスト。

## 2026-10-09: パネル配置の保存と復元

- 🟢 [Done] Rustホストでドッキング状態とメイン／浮動ウインドウの通常配置を保存。論理サイズとモニター相対位置を復元。[仕様](PANEL_DOCKING.md)。
- 🟢 [Done] サイズ上限・version・数値の検証、不正ファイルの保全、一時ファイル経由の置換、モニター消失／倍率変更時の補正ロジック。
- ⭕️ [Pending] 複数モニターとWindows/Linuxでの配置復元実機検証。
- 🟢 [Done] 未保存の入力を保護した選択ノードへのインスペクター追従。[契約](PROPERTY_INSPECTOR.md)。

## 2026-10-09: 選択と設定表示の連携

- 🟢 [Done] Viewごとの有界なSelectionSummary API、生成Schema／TypeScriptクライアント、ネイティブ選択通知。
- 🟢 [Done] 単一選択への自動追従、下書きの保存先保持、保存後／明示的な破棄後の切り替え、古い取得結果の除外、通知欠落時の再照会。[取り込み手順](PROPERTY_INSPECTOR.md)。
- ⭕️ [Pending] Windows/Linuxでのネイティブ選択通知と設定パネルの実機検証。


## 2026-10-11: 通常描画と連続操作の計測

- 🟢 [Done] `draw_timed` / `draw_region_timed` によるCPU計測。通常と同じ経路でGPU完了待ち・query・readbackを追加せず、スキップを成功サンプルから除外。[取り込み方法](RENDER_PROFILING.md)。
- 🟢 [Done] イベントループでPan／Zoom／Dragを再現するTauri起動モード。Command確定とIndex更新を含み、予熱・件数・試行・時間の上限、工程別統計と生データを保持。
- 🟢 [Done] macOS／Metalで18条件・1,080サンプルを計測。スキップ・欠字・GPU Validationエラーなし。[結果](PERFORMANCE.md)。CPU開始頻度を画面FPSとは扱わない。
- 🟢 [Done] ドラッグ確定時の編集・検証・履歴を工程別に計測し、移動だけのcore検証を最適化。[契約](PLACEMENT_EDITS.md)。
- 🔴 [Later] WebView合成・共有Engineの競合を含む長時間の連続操作計測。
- ⭕️ [Pending] Windows/Linux、実モニターの表示完了・入力から表示までの遅延の実機検証。


## 2026-10-11: 移動確定時の検証費用

- 🟢 [Done] MoveNode／移動のみの入れ子BatchとUndo／Redoで対象ノード・矩形を検査し、Graphの全体再検証を省略。混在Batch、読込／保存、ホスト独自検証は維持。
- 🟢 [Done] `execute_timed` と工程別CPU計測例。変更前後の同一条件比較、毎回のUndo／Document一致／全体検証で確認。[結果](PERFORMANCE.md)。
- 🟢 [Done] 入れ子・同一IDの繰り返し・履歴上限・不正座標・存在しないノード・循環／Group違反を含む混在Batchの回帰テスト。ホスト検証が拒否する編集／Undo／RedoでDocument・revision・履歴を保持。
- 🟢 [Done] macOSの通常Surfaceで18条件・1,080フレームを再計測。Registry未登録のfixtureでDrag更新p95が0.030583〜0.035125msに短縮。表示FPSの達成とは扱わない。
- 🟢 [Done] Registryの重複構造検証と移動だけのDefinition走査を削減。独自validatorの互換性、混在Batch、読込、Undo／Redoを検証。[契約とCPU比較](REGISTRY_VALIDATION.md)。
- 🟢 [Done] 一括移動のIndex再構築をBVH境界更新へ変更。最大1,024ノード／4,096関連Edge、差分との併用、Group、Undo／Redo、上限超過の再構築を検証。[契約と比較](BULK_MOVES.md)。
- 🔴 [Later] 遠距離移動・並べ替え後のBVH分割品質の評価と自動再構築。
- 🟢 [Done] Scene生成の可視ID再変換を省略し、6段階のZoomで出力一致とCPU費用を確認。[計測と範囲](SCENE_VISIBILITY.md)。
- 🟢 [Done] Sceneの工程別計測API、Edge境界／接続元型の保持で形状参照を削減。6段階の描画一致、通常Surface18条件・1,080サンプルで確認。[保持量と計測](SCENE_TIMINGS.md)。
- 🔴 [Later] 曲線分割自体の保持・再利用。Zoom精度、プレビュー、保持容量を別途設計して検証する。

## 2026-10-12: stable依存の定期更新

- 🟢 [Done] sha2 0.11／pollster 1.0／互換な推移依存とCIのTypeScript 7へ更新。Rust149件（標準／all-featuresの各構成）、Clippy、実GPU4件、Surface360フレーム、Schema／ACX／3言語UIを検証。[採用版・互換性記録](DEPENDENCY_UPDATES.md)。
- 🟢 [Done] Tauri 2.12.3／runtime-wry 2.12.1／build・macros・codegen 2.7.2へ一式移行。macOSのSurface、入力、子WebView合成、dock/float、浮動状態の再起動復元を検証。[移行記録](DEPENDENCY_UPDATES.md)。
- 🟢 [Done] cosmic-text 0.19へ移行し、遅延整形API・再公開型の取り込み手順・実GPU・Surfaceを検証。2026-10-19の記録を参照。
- 🟠 [Next] wgpu 30、直接依存schemars 1.2.3のAPI／描画／Schema移行と回帰検証。wgpu/Metal経由のblock 0.1.6の将来Rust互換性警告も確認する。現行pinを無条件に解除しない。
- ⭕️ [Pending] Windows/LinuxのSurface・入力・dock/float、WASM、最低Rust版での依存更新検証。

## 2026-10-14: GUIの配置方針

- 🟢 [Done] サンプルWebViewをSVGアイコン中心のコンパクトな配置へ更新。プロパティを上部へ移し、説明・補助情報をhover／focusへ集約。3言語の操作名、Esc、狭い幅、保存／競合／下書き、macOS実操作を確認。[取り込み](COMPACT_PANEL.md)。
- ⭕️ [Pending] Windows/Linuxと実スクリーンリーダーでの新パネル操作確認。

## 2026-10-19: 文字描画とstableパッチ更新

- 🟢 [Done] cosmic-text 0.19、sha2 0.11.1、Tauri 2.12.4一式と互換な推移依存を更新。両Rust構成各152件、Clippy、実GPU4件、Surface360フレーム、17契約、ACX、3言語UIを確認。[記録](DEPENDENCY_UPDATES.md)。
- ⭕️ [Pending] Windows/Linux、WASM、最低Rust版、署名配布と多モニターでの検証。

## 2026-10-25: ACXの継続利用と保持運用

- 🟢 [Done] 新規preflightの受付前に期限切れ・未実行セッションだけを回収。承認／commit済みの未実行確認も対象とし、期限前の確認・実行中job・実行/回復Receiptを保護。
- 🟢 [Done] Rust専用の保持量照会と明示的prune API。期限境界・混在grant・時計の巻き戻り・256サイクルの受付・失敗/回復/非同期jobの再送を検証。両Rust構成各158件、Clippy、17契約とACX stdioが成功。[取り込みと範囲](ACX_RETENTION.md)。
- 🟢 [Done] 実行済み記録の任意永続化と再送保証を保つProvider切り替え。以下の永続再送を参照。独立Providerとの相互運用は未完了。

## 2026-10-25: ACXの永続再送とProvider切り替え

- 🟢 [Done] 開始前/完了応答前の同期保存、同じcommitの結果とReceiptの再取得、成否不明commitの再実行拒否、完了済みRecover/job再送、writer排他と容量予約。任意のRust APIとstdioオプションで有効化。[設計と限界](ACX_DURABILITY.md)。
- 🟢 [Done] 実行中のepoch切り替え拒否、旧Receiptを残す切り替え、別プロセスでの中断・再起動、破損・I/O失敗・容量不足・Document不一致の検証。両Rust構成各169件、Clippy、17契約、stdio再起動検証が成功。
- 🟠 [Next] Document/Undo保存とのトランザクション連携と長期ログの圧縮／退避。永続化ジャーナルはDocument自動保存ではない。
- ⭕️ [Pending] Windows/Linux、実電源断、最低Rust版、filesystem別の実機検証。独立Providerとの相互運用は別途必要。

## 2026-10-26: UUID stable更新

- 🟢 [Done] uuid 1.28.0→1.29.0、455種のCargo依存のstable候補／MSRV監査。両Rust構成各169件、Clippy、17契約、Schema6件、ACX再起動を含むstdio6件、strict TypeScript、headlessを確認。[互換性・保留記録](DEPENDENCY_UPDATES.md#2026-10-26-uuidの互換更新と全依存監査)。
- 🟠 [Next] Tauri 2.12.5一式／Wry 0.57.1、wgpu 30、直接schemars 1.2.3、他の推移依存候補を関連回帰検証とともに分割移行。exact pinは維持。
- ⭕️ [Pending] 他OS、WASM、最低Rust版と実電源断の実機検証。
