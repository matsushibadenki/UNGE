# Architecture / 設計

```text
WebView toolbar / inspector (en, ja, zh-CN)
       │ commands + expected_revision / small summaries
       ▼
unge-tauri Engine (application-owned)
       ├ Editor → Document → Graph
       ├ Viewport / selection per view
       ├ SceneIndex (BVH) → visible Scene
       └ SurfaceRenderer → retained GPU buffers → native surface

Document snapshot → Registry → Scheduler → trusted NodeExecutor
                         └ host resource store (CPU/GPU objects)
```

## 所有権と契約

Graphは論理モデル、DocumentはGraphと保存する表示座標を持ちます。
Viewportと選択はDocumentに保存せず、EngineのViewに置きます。
ウインドウが所有するのはOS上の表示面だけで、可変DocumentとRendererはアプリ共有Engineが所有します。
core::Workspaceは複数Document、資産参照、言語設定を保持します。
Tauriアダプターは1つのDocumentを共有する最小構成です。

GPU座標のオーバーフローを避けるため、保存する矩形とViewportのワールド座標は絶対値10億以内に制限します。
Graphのノード・エッジ・Groupは外部から可変アクセスできません。
CommandだけがEditorのDocumentを更新します。逆命令で履歴を保持し、変更のたびに全Documentを複製しません。
Batchの途中や最後の検証に失敗した場合、逆順に戻してrevisionと履歴を維持します。
Undo履歴は件数制限です。バイト数での制限やディスク退避は未実装です。

保存順序を安定させるため主データはBTreeMapです。設計書のHashMap推奨からの意図的な差分です。
依存探索にはHashMapのGraphIndex、表示探索にはBVHを使います。
Document編集完了後に全体検証とSceneIndex再構築を行うため、連続ドラッグは描画のプレビューと確定Commandを分けて統合してください。
細粒度のIndex差分更新は今後の最適化項目です。

## 実行

Graphの接続・型・循環を検証した後、登録DefinitionとNodeのPortスキーマを照合します。
必須入力を検査し、決定的なトポロジカル階層を生成します。
各階層内を `buffer_unordered(concurrency)` で実行し、完了を待って次の階層へ進みます。
後続ノードは、依存ノードが失敗・出力欠損した場合Blockedになります。無関係なノードは続行します。
階層をまたぐ即時スケジューリング、進捗イベント、Streaming、周期実行は未実装です。

pureノードのキャッシュキーはtype_id・definition version・入力・propertiesの決定的なJSONです。
キャッシュは容量を件数で制限したFIFOです。キー探索は線形で、大容量ワークフローではハッシュ化・バイト容量制限が次の改善点です。
入力が変わるとそのノードと出力が変わる下流だけが再実行されます。
同じ結果が出た下流の既存キャッシュは利用できます。GraphIndex::downstreamは明示的なDirty集合にも使えます。

## GPU

ネイティブではwgpuがMetal/Vulkan/DX12等を使います。「WebGPU向け」はwgpu/WGSLによるWebGPU系APIを指し、TauriのWebView内でGPU処理を実行する意味ではありません。
現在の検証対象はwgpu 27.0.1です。将来のメジャーバージョン追従は別途検証してください。

ノード・Port・選択枠・曲線をQuadのインスタンスに変換し、1 draw callで描画します。
Bezier曲線はズームに応じて8/24区間へ分割し、線分を同じインスタンスバッファに入れます。
Node BVHとEdge BVHを別々に使い、接続線が画面内を横切る場合は端点ノードが画面外でも描画します。
遠景はPortを省略します。文字・Group描画・Minimapは未実装です。

GpuRendererは容量に余裕を持ったバッファを再利用し、不足時のみ拡張します。
表示Sceneの更新時には可視インスタンスをCPU→GPUへアップロードします。
JavaScriptを経由するコピーやGPU→CPUの毎フレームreadbackはありません。
テストのみ検証用にピクセルをreadbackします。

SurfaceRendererはゼロサイズ、Resize、Timeout、Lost/Outdatedを扱います。
GPU Device LostとOutOfMemoryはエラーとしてホストに返します。ホストは必要に応じてRendererを再生成します。
サンプルのネイティブWindowBuilderはTauri 2の `unstable` featureを使います。このfeatureはサンプル側だけで有効化し、コアライブラリには要求していません。
同一Tauriウインドウ内でWebViewとネイティブ描画面を重ねる処理はOS別の実装が必要なため、デスクトップ例では別ウインドウを使っています。

## IPC

Tauriのwindow引数から呼び出し元を特定し、Rustで登録済みのViewだけを受け付けます。
リクエスト上限は256KiB、Batch深さ16、命令数4096、選択数10000です。
この上限はdeserialize後のAPI上限です。通信のストリーム段階のサイズ制限ではありません。
Document全体の保存・読み込みと実行はRustホスト側APIを使います。
変更通知はSummaryだけを配信します。イベント欠損時も編集時のrevisionチェックで競合を検出します。
エラーは安定したcodeとログ向けmessageで返し、UI側はcodeを3言語で翻訳します。
TauriのCapabilityはホストで設定し、操作ウインドウへ外部Webページを読み込まない構成にしてください。

## 参照した公式仕様

- [Tauri 2: State management](https://v2.tauri.app/develop/state-management/)
- [Tauri 2: Calling Rust from the frontend](https://v2.tauri.app/develop/calling-rust/)
- [wgpu 27.0.1 API](https://docs.rs/wgpu/27.0.1/wgpu/)

原設計の機能ごとの対応状況はROADMAP.mdを参照してください。

## ACX接続

`unge-acx` はcore/executorだけに依存する任意アダプターです。UI/GPU依存はありません。
ProviderはGraphHostを介して共有Documentにアクセスします。Tauri側のtrait実装は `acx` featureで有効化します。
この経路も同じEditor・Undo履歴・revisionを使い、AI編集完了後に既存のSceneIndexを再構築します。

Preflightはコピー上で命令を組み立てて検証します。Commit後は保存済みの命令だけを実行します。
実行直前のrevision照合と編集をホストの同一ロック内で行います。Runは保存時点のsnapshotを使い、描画側のロックを保持しません。
失敗を含む実行結果とReceiptを保持し、同じcommitIdの再送では再実行しません。回復時は編集直後のrevisionとUndoの利用可否を照合します。

正確なJSON文字列をハッシュ材料として返し、Rustのf32と他言語の浮動小数点シリアライズの差を避けます。
RegistryやPolicyは信頼済みホストが提供し、AIから変更できません。能力の既定値は読み取り専用です。
詳細は [ACX_INTEGRATION.md](ACX_INTEGRATION.md) を参照してください。
