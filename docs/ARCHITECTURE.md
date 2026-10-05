# Architecture / 設計

```text
WebView toolbar / inspector (en, ja, zh-CN)
       │ commands + expected_revision / small summaries
       ▼
unge-tauri Engine (application-owned)
       ├ Editor → Document → Graph
       ├ RunService → frozen snapshot / bounded run summaries / cancellation
       ├ Viewport / selection / Interaction preview per view
       ├ SceneIndex (BVH) → visible Scene
       └ SurfaceRenderer → retained GPU buffers → native surface

Document snapshot + revision → RunService → Registry → Scheduler → trusted NodeExecutor
                         └ host resource store (CPU/GPU objects)
```

## 所有権と契約

Graphは論理モデル、DocumentはGraphと保存する表示座標を持ちます。
Viewport・選択・言語・テーマはDocumentに保存せず、EngineのViewに置きます。
ウインドウが所有するのはOS上の表示面だけで、可変DocumentとRendererはアプリ共有Engineが所有します。
core::Workspaceは複数Document、資産参照、言語設定を保持します。
Tauriアダプターは1つのDocumentを共有する最小構成です。

ポインター入力とViewportのsizeは論理ピクセルです。draw_scaledは物理Surfaceサイズとscale factorから論理サイズを求めます。ネイティブ入力は物理座標をscale factorで割って渡します。

GPU座標のオーバーフローを避けるため、保存する矩形とViewportのワールド座標は絶対値10億以内に制限します。
Graphのノード・エッジ・Groupは外部から可変アクセスできません。
CommandだけがEditorのDocumentを更新します。逆命令で履歴を保持し、変更のたびに全Documentを複製しません。
Batchの途中や最後の検証に失敗した場合、逆順に戻してrevisionと履歴を維持します。
Undo/Redo履歴は合算の件数とシリアライズ後ペイロード容量で制限します（既定256件・16MiB）。巨大な逆命令を保持できなければ履歴を破棄し、途中の編集を飛ばしたUndoを防ぎます。総ヒープ使用量の制限やディスク退避は未実装です。

coreのDocumentValidatorをEditorへ設定すると、ロード時と編集・Undo/Redo確定時にホスト検証を適用できます。Registryがこのtraitを実装し、Port一致とPropertySchemaを検証します。Batchの最終状態だけを検証し、失敗時はDocument・revision・履歴を維持します。基本のnewコンストラクターは構造検証だけを維持し、サンプルはwith_validatorとfrom_editorで有効化しています。詳細は [PROPERTY_VALIDATION.md](PROPERTY_VALIDATION.md)。

保存順序を安定させるため主データはBTreeMapです。設計書のHashMap推奨からの意図的な差分です。
依存探索にはHashMapのGraphIndex、表示探索にはBVHを使います。BVH構築は各階層の中央値分割を使い、部分木ごとの全件ソートを避けます。
Document編集完了後に全体検証を行います。小さな移動ではノードと接続Edgeの描画Indexだけを更新し、プロパティ編集では現在の形状Indexを維持し、Group編集とノード移動ではGroup専用Indexを更新します。構造変更・Undo/Redo・更新保持量の超過時はSceneIndexを再構築します。unge-interactionはドラッグ中の一時座標を保持し、Upだけを確定Commandにします。描画は同じBVHと移動ノードの接続Indexを使い、一時座標で可視性を再評価します。
変更矩形を各Index最大128件の集合で保持し、検索時に旧境界を除外して新境界を検査します。取り込み契約は [INDEX_UPDATES.md](INDEX_UPDATES.md)。トポロジーとUndo/Redoの差分更新は今後の項目です。

## 実行

Graphの接続・型・循環を検証した後、登録DefinitionとNodeのPortスキーマ・プロパティの型/制約を照合します。
必須入力を検査し、決定的なトポロジカル階層を生成します。
各階層内を `buffer_unordered(concurrency)` で実行し、完了を待って次の階層へ進みます。
後続ノードは、依存ノードが失敗・出力欠損した場合Blockedになります。無関係なノードは続行します。
Rustホストへ開始/終了/ノード進捗を通知し、ホストのdeadline Futureと協調キャンセルで未完了Futureを破棄できます。結果・キャンセル・外部副作用の契約は [EXECUTION_PROGRESS.md](EXECUTION_PROGRESS.md)。階層をまたぐ即時スケジューリング、Streaming、周期実行、ACXは任意の非同期job操作に対応し、Provider所有のcommitで開始・照会・キャンセルします。仕様は [ACX_ASYNC_JOBS.md](ACX_ASYNC_JOBS.md)。共有RunServiceとTauri集約通知・ACXホストobserverは [EXECUTION_SERVICE.md](EXECUTION_SERVICE.md)。

pureノードのキャッシュキーはtype_id・definition version・executorのプロセス内識別子・入力・propertiesの決定的なJSONです。
標準HashMapで完全なキーを照合し、ハッシュ衝突でも異なる入力を取り違えません。FIFOを件数とシリアライズしたキー＋出力のバイト数で制限します。既定のバイト上限は16 MiBです。ヒットで挿入順を変えず、同じキーの重複保存を避けます。大きすぎるキーや出力は保存しません。詳細とホスト設定は [EXECUTION_CACHE.md](EXECUTION_CACHE.md)。
入力が変わるとそのノードと出力が変わる下流だけが再実行されます。
同じ結果が出た下流の既存キャッシュは利用できます。GraphIndex::downstreamは明示的なDirty集合にも使えます。

## GPU

ネイティブではwgpuがMetal/Vulkan/DX12等を使います。「WebGPU向け」はwgpu/WGSLによるWebGPU系APIを指し、TauriのWebView内でGPU処理を実行する意味ではありません。
現在の検証対象はwgpu 27.0.1です。将来のメジャーバージョン追従は別途検証してください。

ノード・Port・選択枠・曲線をQuadのインスタンスに変換します。文字のないSceneは1 draw callで描き、文字がある場合は各ノードの形状とGlyphを交互に描いて重なり順を守ります。
Bezier曲線は形状とズームに応じて適応分割し、線分を同じインスタンスバッファに入れます。目標誤差0.75論理px、最大128区間で処理量を制限します。上限と数値精度の制限は [CURVE_TESSELLATION.md](CURVE_TESSELLATION.md)。
Node BVHとEdge BVHを別々に使い、接続線が画面内を横切る場合は端点ノードが画面外でも描画します。
遠景はPortと文字を段階的に省略します。文字はcosmic-textで整形し、固定容量のGlyph Atlasに保持します。表示名はホストのLabelCatalog、言語はRustのView状態に置きます。実装・フォント注入・容量制限は [GPU_TEXT.md](GPU_TEXT.md)。Groupは専用BVH、所属ノードの外接矩形、テーマに合う枠とGPU見出しに対応します。[GROUP_RENDERING.md](GROUP_RENDERING.md)。Minimapは未実装です。

GpuRendererは容量に余裕を持ったバッファを再利用し、不足時のみ拡張します。
表示Sceneの更新時には可視インスタンスをCPU→GPUへアップロードします。
JavaScriptを経由するコピーやGPU→CPUの毎フレームreadbackはありません。
テストのみ検証用にピクセルをreadbackします。

Dark/LightのsRGB配色はRustのThemePaletteに集約し、GPUには線形RGB、WebViewにはCSS tokensを渡します。Sceneの背景clear・グリッド・形状・文字を同時に更新します。テーマ変更でSceneIndexやGlyph Atlasを作り直しません。操作画面の小さなappearance命令とView別通知、ネイティブ枠のOS制約は [THEMES.md](THEMES.md)。

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
この経路も同じEditor・Undo履歴・revisionを使い、AI編集完了後にも同じSceneChangesによる差分更新またはSceneIndex再構築を行います。

Preflightはコピー上で命令を組み立てて検証します。Commit後は保存済みの命令だけを実行します。
実行直前のrevision照合と編集をホストの同一ロック内で行います。Runは保存時点のsnapshotを使い、描画側のロックを保持しません。
失敗を含む実行結果とReceiptを保持し、同じcommitIdの再送では再実行しません。回復時は編集直後のrevisionとUndoの利用可否を照合します。

正確なJSON文字列をハッシュ材料として返し、Rustのf32と他言語の浮動小数点シリアライズの差を避けます。
RegistryやPolicyは信頼済みホストが提供し、AIから変更できません。能力の既定値は読み取り専用です。
詳細は [ACX_INTEGRATION.md](ACX_INTEGRATION.md) を参照してください。

## ポインター入力

`unge-interaction` はcoreにだけ依存します。TauriはViewごとに操作状態を持ち、Down時のrevisionをMove/Upでも照合します。MoveはDocumentを更新せず、Up時に同一ロック内でCommandを実行します。外部編集時は一時描画を消して古い操作を拒否します。接続プレビューは型・占有・重複・循環を検証します。

サンプルの `input.rs` だけがunstableなWry/Tao入力APIへ依存します。OS入力の取得、論理座標への変換、連続Moveの集約、Cancelと小さな通知を担当します。ホストへの実装手順と未対応項目は [POINTER_INPUT.md](POINTER_INPUT.md)。

グラフ・定義・実行値/進捗・Tauri IPCの構造契約を、任意schema featureとビルド用unge-contractsから生成します。入力の省略規則と出力の全フィールドを区別し、CIで生成差分を検出します。意味上の検証はRustに保持します。[CONTRACT_GENERATION.md](CONTRACT_GENERATION.md)。

キーボード操作用のノード概要は登録済みViewの言語/選択からRustが生成し、1〜100件のページで返します。HTMLからの座標移動・削除もCommandと表示revisionを使用します。ネイティブSurfaceのOSアクセシビリティツリーは未実装です。[ACCESSIBILITY.md](ACCESSIBILITY.md)。

Group UIはページ付き概要だけを受け取り、Rust Viewの選択から作成/所属編集します。Group Requestは同じロック内でrevisionを検査し、SetGroupで履歴とSceneIndexを更新します。所属Node選択はViewだけを更新します。[GROUP_EDITING.md](GROUP_EDITING.md)。
