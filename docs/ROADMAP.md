# Roadmap

- [Done] implemented in the current codebase
- [Next] high-priority unfinished work
- [Later] planned, but not the closest next step

## コアと編集

- [Done] UUID、型付きPort、Single/Multiple入力、明示的接続、DAG検証
- [Done] 論理Graphと表示座標の分離、複数DocumentのWorkspace、資産参照
- [Done] Command、Batchの原子性、逆命令によるUndo/Redo、履歴件数制限
- [Done] ノード削除時のEdge/Group整合性、プロパティ更新、Groupデータ
- [Done] IDを再発行するコピー＆ペースト、Left-to-right階層レイアウト
- [Done] BVHの範囲検索・Hit Test、Viewport Pan/Zoomの座標変換
- [Done] スキーマバージョン付きJSON、未知バージョンの拒否、安定した保存順序
- [Done] 独立したPointer状態機械、選択/複数Node Drag/Box Select/双方向Port接続、Pan
- [Done] Rust内の一時プレビュー、Up時の単一Command確定、競合・Cancel・ポインター所有権検証
- [Done] Undo/Redo合算のバイト容量制限、容量超過時の履歴連続性維持、保持量照会
- [Done] PropertySchemaの型・範囲・選択肢・必須値・初期値、Registryによる編集/Undo/Redo時の原子的検証
- [Later] JSONバージョンMigration、Binary形式、Semantic Diff
- [Later] Subgraph、Frame、Comment、他の自動レイアウトとEdge Routing

## 実行

- [Done] Definition/Instance分離、意味情報の3言語対応、信頼済みExecutor登録
- [Done] プロパティ定義の妥当性検証、3言語メタデータ、生成時の初期値適用
- [Done] 入出力スキーマ検証、必須入力検証、非同期DAGの階層並列実行、並列数制限
- [Done] Completed/Cached/Failed/Blocked/Cancelledレポート、独立分岐の継続
- [Done] pureノードのFIFOキャッシュ、変更入力からの再実行、下流探索
- [Done] 協調キャンセル、ホストのリソースストア向け型付きID
- [Done] 完全キーのハッシュ検索、FIFOの件数＋バイト容量制限、使用量照会、実行時の上限変更、executor識別子によるキャッシュ分離
- [Done] Rustホスト向け開始/終了/ノード進捗イベント、ホストFutureによるrun期限、待機中のキャンセル通知、未完了結果のCancelled記録
- [Done] run ID/revision付き共有実行サービス、1実行の予約、snapshot/概要保持上限、破棄・panic時の予約解放
- [Done] Tauriの開始/現在実行/進捗照会/キャンセルAPI、集約通知、TypeScript型と3言語文言
- [Done] ACX承認済み実行の共有サービス接続、ホスト向け進捗通知、UIとの二重実行防止、既存Receipt再送契約維持
- [Done] 3言語の実行/キャンセル操作UI、進捗表示、snapshot revisionの区別、通知欠損時の照会回復
- [Done] 任意ACX非同期job Profile、承認済みcommitの開始/進捗照会/キャンセル、単一worker、終端Receiptと再送、Provider終了時のキャンセル
- [Done] Rustのグラフ・実行値・定義・進捗・Tauri IPCからTypeScript入力/出力型とJSON Schema生成、プロパティ値制約のSchema公開、CI差分チェック
- [Done] DefinitionごとのPort名/型/必須性/入力個数を反映した値マップSchema、Rust検証API、3,068例の判定照合
- [Later] Converter Registry、Remote/Python/AI/GPU Backendの標準アダプター
- [Later] Streaming、Reactive、Simulation、Subgraph実行
- [Later] 外部Plugin manifest、WASMサンドボックス、権限モデル

## 描画と統合

- [Done] Rust所有のSceneIndex、GPUインスタンスバッファ、WGSLパイプライン
- [Done] Node/Port/選択枠/Grid/Bezier Edge描画、NodeとEdgeのBVHカリング、簡易LOD
- [Done] TextureViewへの描画、ネイティブSurface、Resize/Timeout/Lostへの処理
- [Done] Tauri 2共有Engine、呼び出し元View検査、revision競合検出、変更Summaryイベント
- [Done] TypeScriptクライアント、3言語のエラー辞書、CLIとTauri実行例
- [Done] GPU文字描画（Glyph Atlas）、3言語のホスト表示名、View単位の言語、HiDPI・クリップ・重なり順・容量制限
- [Done] 参考画像に基づくドット背景・役割色・ノード記号/説明文・Portリング・選択枠・SDF影、3言語実GPU Gallery
- [Done] ダーク/ライトのテーマAPI、Rust View単位の設定、GPU/CSS共通配色、3言語の切り替えUIと設定復元
- [Done] Group枠・見出しのGPU描画、BVHカリング、ドラッグ追従、テーマ/LOD、編集後の整合性
- [Done] 役割色のヘッダー、PropertySchemaから生成する3言語の設定パネル、有界な設定取得API、Batch保存・検証・競合時の下書き保持、ホスト拡張の数値調整サンプル
- [Later] ネイティブノード内の入力コントロール、画像資産プレビュー、設定項目の表示ヒント（同一ウインドウ合成後）
- [Done] 有界なノード概要API、3言語のHTML一覧、キーボードで選択/座標移動/削除、revision競合拒否
- [Done] Group概要のページ取得、選択から作成/改名/所属追加・除外/削除、所属Nodeの一括選択、3言語HTML操作とUndo/Redo・競合検証
- [Later] Group枠のHit Test・枠ドラッグ・折りたたみ
- [Later] ネイティブSurfaceのアクセシビリティツリー、Port/Groupのキーボード操作、読み上げ実機検証とWCAG評価
- [Later] 任意のテーマ登録、OS配色/高コントラストへの自動追従
- [Done] 10000ノード/30000エッジの再現可能なCPU計測、BVH中央値分割による構築改善、総当たり検索との回帰検証
- [Done] 小さな移動のノード/接続Edge Index差分更新、プロパティ編集時の形状Index維持、Group編集時のGroup専用Index更新、128件上限と全体再構築への切り替え、UI/Pointer/ACXへの共通適用
- [Done] 形状/ズームに応じたBezier適応分割、誤差目標と分割上限、逆向き/端点一致の検証、実GPU曲線ピクセル検証
- [Next] トポロジー/Undo/RedoのIndex差分更新、実GPU/Surfaceを含むフレーム時間の計測
- [Done] Tauri/Taoのネイティブ入力ブリッジ、HiDPIの論理座標、ホイール拡大縮小（macOS実操作確認）
- [Next] Windows/LinuxのネイティブSurface/入力検証、同一ウインドウ合成
- [Later] GPU不可時のフォールバック、ブラウザーWASMホスト
- [Later] Minimap、検索、Smart Connection、CRDT共同編集、Git連携

設計書の「10000 Nodes / 30000 Edges / 60 FPS」のうちCPU処理は計測しました。GPU/Surfaceを含む60FPSは未検証です。詳細は [PERFORMANCE.md](PERFORMANCE.md)。

## ACXによるAI操作

- [Done] 実験的Node Graph Profile、ACX Manifest/Receiptに適合するRust Provider
- [Done] プロパティ制約付きDefinition照会、生成時の初期値、Preflightでの編集後プロパティ検証
- [Done] ページ付きノード定義・グラフ照会、型付きノード生成、接続・削除・移動・プロパティ・Group・レイアウト操作
- [Done] Preflightの無変更プレビュー、ホストポリシー承認、digestとrevisionに固定したCommit/Execute
- [Done] 同一ロック内の編集、失敗を含む結果の再送、条件付きRecover、元Receiptを維持した回復証明
- [Done] Rust/Python間の正確なハッシュ材料照合、型付きTypeScriptクライアント、3言語エラー辞書
- [Done] Headless JSON Lines Provider、Tauriの共有Documentへ接続する `--acx-stdio` モード
- [Done] acx側へのProfile仕様・Schema・例・テスト追加、UNGEへの持ち運び可能なSchema同梱
- [Next] 別実装Providerとの相互運用、承認・Receipt・再送記録の永続化、セッション保持上限の運用
- [Later] MCP/A2A binding、署名、多ユーザー認証、外部副作用や課金を伴う実行Profile

## 今回の優先順位（2026-10-05）

- [Done] AIと通常UIの双方に効く編集時検証、履歴容量制限を先行。統合手順は [PROPERTY_VALIDATION.md](PROPERTY_VALIDATION.md)。
- [Done] Pointer状態機械とDrag/接続操作。取り込み手順は [POINTER_INPUT.md](POINTER_INPUT.md)。
- [Done] GPU文字描画。型名・Port名の3言語表示、ホストのフォント注入。取り込み手順は [GPU_TEXT.md](GPU_TEXT.md)。
- [Done] ダーク/ライト切り替え。取り込み手順は [THEMES.md](THEMES.md)。
- [Done] 実行キャッシュのハッシュ検索とバイト容量制限。大きすぎる結果を保持せず、件数と容量の両方でFIFOを管理する。
- [Done] Rustホストへの進捗通知とrun期限。取り込み手順は [EXECUTION_PROGRESS.md](EXECUTION_PROGRESS.md)。
- [Done] run ID/revision付き実行サービスとTauri操作API、ACXのホスト接続。取り込み手順は [EXECUTION_SERVICE.md](EXECUTION_SERVICE.md)。
- [Done] 実行ボタンと進捗UI。通常操作と実行状態を分け、通知欠損時は照会で回復する。
- [Done] ACX非同期job Profile。取り込み仕様は [ACX_ASYNC_JOBS.md](ACX_ASYNC_JOBS.md)。
- [Done] Rust契約から型・Schema生成。[取り込み手順](CONTRACT_GENERATION.md)。
- [Done] 大規模グラフのCPU実測とBVH構築改善。[PERFORMANCE.md](PERFORMANCE.md)。
- [Done] 小さな編集のIndex差分更新。[取り込み手順](INDEX_UPDATES.md)。
- [Done] 全体表示時の曲線生成量を改善。[CURVE_TESSELLATION.md](CURVE_TESSELLATION.md)。
- [Done] Group描画。[取り込み手順](GROUP_RENDERING.md)。
- [Done] アクセシビリティの初期対応。[ノード概要とHTML操作](ACCESSIBILITY.md)。
- [Done] Groupの所属Node選択と編集。[取り込み手順](GROUP_EDITING.md)。
- [Done] DefinitionごとのPort名/型/必須性/入力個数を反映した値マップSchema、Rust検証API、3,068例の判定照合。

## 次の優先項目（2026-10-06）

- [Done] Port値マップSchema。[取り込み手順](PORT_VALUE_SCHEMAS.md)。
- [Next] トポロジー/Undo/RedoのIndex更新費用を削減し、CPU計測で検証する。

## 2026-10-07: 表示デザイン

- [Done] ノードの表示を更新。[ホスト指定と見本生成](NODE_APPEARANCE.md)。
- [Next] 既存のIndex更新費用削減と、新デザインの実GPU/Surfaceフレーム時間計測。

- [Done] [ノード設定インターフェイス](PROPERTY_INSPECTOR.md)を追加。読み込んだrevisionで適用し、1回のUndoで復元する。
