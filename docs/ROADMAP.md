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
- [Next] 実行進捗イベント、タイムアウト方針、キャッシュのハッシュ検索とバイト容量制限
- [Next] 入出力型・プロパティ・IPC型のスキーマ自動生成
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
- [Next] Group描画、テーマAPI、アクセシビリティ
- [Next] 10000ノード/30000エッジの実測、Index差分更新、曲線テッセレーションの改善
- [Done] Tauri/Taoのネイティブ入力ブリッジ、HiDPIの論理座標、ホイール拡大縮小（macOS実操作確認）
- [Next] Windows/LinuxのネイティブSurface/入力検証、同一ウインドウ合成
- [Later] GPU不可時のフォールバック、ブラウザーWASMホスト
- [Later] Minimap、検索、Smart Connection、CRDT共同編集、Git連携

設計書の「10000 Nodes / 30000 Edges / 60 FPS」は測定前の目標です。

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

## 今回の優先順位（2026-09-28）

- [Done] AIと通常UIの双方に効く編集時検証、履歴容量制限を先行。統合手順は [PROPERTY_VALIDATION.md](PROPERTY_VALIDATION.md)。
- [Done] Pointer状態機械とDrag/接続操作。取り込み手順は [POINTER_INPUT.md](POINTER_INPUT.md)。
- [Done] GPU文字描画。型名・Port名の3言語表示、ホストのフォント注入。取り込み手順は [GPU_TEXT.md](GPU_TEXT.md)。
- [Next] 実行進捗とキャッシュ容量/検索の改善。処理状況の可視化と大きなグラフの再実行を改善する。
