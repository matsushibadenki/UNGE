# AIによる取り込み手順

## そのまま渡せる依頼文

> このUNGEディレクトリのAGENTS.mdとdocs/AI_INTEGRATION.mdを読み、既存のRustクレートをpath依存で取り込んでください。
> Document/State/RendererはRust側に置き、Tauri 2のWebViewは操作UIに限定してください。
> 大容量データはRust/GPUに保持し、IPCには命令・リビジョン・リソースIDだけを渡してください。
> アプリ固有ノードはRegistryへ追加し、まずheadlessテストで動作を確認してください。
> ROADMAPの未実装項目を前提にしないでください。必要な拡張だけを実装して検証してください。

## 最小の統合順序

1. `cargo run -p unge-headless` を実行し、42が返ることを確認する。
2. `examples/headless/src/main.rs` を基に、アプリ固有のグラフをCommandで組み立てる。
3. `Definition` に安定したtype_id、意味を説明する3言語の文言、version、Port、PropertySchemaを定義する。
4. `NodeExecutor` を実装し、`Registry::register` で登録する。
5. `Scheduler::run` にDocumentのグラフを渡す。実行中の編集を許すなら、Rust内で取得したsnapshotを実行する。
6. Tauriホストで `Engine::from_registry(document, 256, registry.clone())?` を作り、`app.manage(engine)` する。WebViewのlabelは `register_view` で登録する。
7. `invoke_handler(unge_tauri::handler())` を接続する。既存命令と共存するときは `generate_handler![unge_tauri::dispatch, unge_tauri::inspect, unge_tauri::node_properties, unge_tauri::selection_summary, unge_tauri::appearance, unge_tauri::accessible_nodes, unge_tauri::groups, unge_tauri::start_execution, unge_tauri::current_execution, unge_tauri::execution_status, unge_tauri::cancel_execution, your_command]` を使う。
8. `createClient(invoke)` でWebViewから操作する。描画フレームを返す命令を追加しない。
9. Rustネイティブウインドウから `SurfaceRenderer` を作り、Engineに登録する。HiDPIでは `draw_scaled` を使う。実装例は `examples/tauri-host/src/main.rs`。
10. [POINTER_INPUT.md](POINTER_INPUT.md) に従い、論理座標のDown/Move/Up/Cancelを順に送る。1操作のrevisionを固定し、Rust内のプレビューと確定Commandを分ける。
11. [GPU_TEXT.md](GPU_TEXT.md) に従い、LabelCatalogで型・Portの表示名を登録する。配布環境の日本語・简体中文フォントを確認し、必要ならFontSystemを注入する。言語はViewに置く。
12. [THEMES.md](THEMES.md) に従い、ViewのテーマとRustから取得したCSS配色を使う。独自invoke handlerにはappearanceも登録する。
13. ノードを選んだときだけinspectを呼び、`unge://changed`イベントで小さなSummaryを受け取る。

## インターフェース

```ts
import { invoke } from '@tauri-apps/api/core';
import { createClient } from './unge/index';

const engine = createClient(invoke);
const summary = await engine.summary();
await engine.apply({ kind: 'move_node', id: selectedId,
  rect: { x: 200, y: 120, width: 180, height: 90 } }, summary.revision);
```

`revision_conflict` が返ったらSummary/対象ノードを再取得し、ユーザーの編集意図を再確認する。
古い編集を新しいrevisionで自動的に再送しない。ズーム・選択はView単位で、Documentのrevisionを増やさない。
`set_property.value: null` はプロパティ削除を意味する。JSON null自体の保存は現APIでは扱わない。

## Rustのノード拡張

[PROPERTY_VALIDATION.md](PROPERTY_VALIDATION.md) にプロパティ制約・初期値・編集時検証・Undo容量の組み込み例があります。RegistryをArcに包み、UI・ACXで同じ定義を共有してください。

[EXECUTION_CACHE.md](EXECUTION_CACHE.md) に `CacheLimits`・使用量照会・上限変更・pureの契約を記載しています。キャッシュはRust内に保持し、Report/IPC/ACXの形式は変わりません。

`math_registry()` が最小の実例です。`NodeExecutor::execute` はBoxFutureを返すためdyn traitとして登録できます。
`Inputs` はPort名→Value列です。Single入力も1要素の列として受け取ります。
Multiple入力の値の順序はEdge ID順です。意味のある順序が必要なアプリは明示的な順序プロパティを設計してください。
出力名・型・必須出力は実行後に検証され、違反はFailedになります。

CPU処理でasync executorを塞がないよう、ホストの `spawn_blocking` / Rayonなどへ処理を逃がしてください。
進捗は `run_with_progress`、run全体の期限は `run_with_deadline` でホストへ組み込めます。[EXECUTION_PROGRESS.md](EXECUTION_PROGRESS.md) のイベント契約・Future破棄・通知の制限を参照してください。
キャンセルは協調式です。長時間ノードは `context.cancellation.is_cancelled()` をチェックし、HTTPタイムアウト等も実装します。
Schedulerは実行中の外部副作用を巻き戻しません。GPU処理はホスト所有のDevice/Queue/ResourceStoreをexecutorにArcで注入します。

## 大容量リソース

`Value::Resource { id, data_type }` はホストのリソースストアを指します。
バイト列、画像、音声、Tensor等を `Value::Json` に埋め込まないでください。
リソースIDは内容・バージョンごとに不変である必要があります。内容を書き換えたら新しいIDを発行します。
キャッシュが参照するIDの解放・容量制限・永続化はホストの責務です。
不変性を保証できないリソースを使うノードは `pure: false` にします。

## 複数Document / ウインドウ

coreのWorkspaceは複数Documentと各Documentの履歴を保持できます。
TauriのEngineは1つのDocumentを複数Viewで共有するアダプターです。
複数Documentを同時表示するアプリでは、ホストサービスにDocument ID→Engineの対応を設け、各ウインドウのアクセス先をRustで固定してください。
WorkspaceとEngineに同じDocumentの可変コピーを二重に持たせないでください。

## 取り込み時の検証

- ドメイン固有ノードで、正常系・不正入力・出力型違反・キャンセル・再実行を検証する。
- Undo/Redo後のJSONが元に戻ることを確認する。
- 2ウインドウから同じrevisionで編集し、後の編集が拒否されることを確認する。
- GPUテストを動かす。SurfaceはOSごとに、縮小・復帰・DPI変更・終了時も確認する。
- Rust契約変更後は `python3 scripts/generate_contracts.py` で型・Schemaを更新し、`--check` と `scripts/test_contracts.py` を実行する。[CONTRACT_GENERATION.md](CONTRACT_GENERATION.md) を参照。ACXライフサイクル型は引き続き手書き。

## 移植範囲

- core/executor/interaction: TauriなしでCLI・サーバーに使用可能。
- render: wgpuのRust Device/TextureViewを受け取る。ホストのネイティブウインドウと組み合わせる。
- tauri: Tauri 2専用。グラフ変更はworkerへ移し、Surfaceの生成・描画スレッドはホストで調整する。
- ブラウザー単独版: wgpuのWebGPUバックエンドを利用するWASMホストは未提供。UUIDのWASM設定、Canvas初期化、非Send future等の対応が必要。

## ACXエージェントから実行中のグラフを操作する

コード生成の引き継ぎに加え、実行中のノード操作には `unge-acx` を利用できます。
[ACX_INTEGRATION.md](ACX_INTEGRATION.md) を読み、`examples/acx-provider/agent.py` と同じライフサイクルを使ってください。
通常UIと同じRustのEngineに接続し、別Documentを作って同期する設計にしないでください。
事前確認のdigest・内容・revisionを照合し、競合時には新しく観測してpreflightを作り直します。

## 実行サービスを接続する

[EXECUTION_SERVICE.md](EXECUTION_SERVICE.md) のRunServiceをEngineのclone/manage前にwith_executionで設定します。Registry ArcとサービスをACX Providerにも共有し、実行IDとsnapshot revisionを使って進捗を管理してください。Tauriには開始・現在実行・照会・キャンセル命令があります。既存executeは同期です。AIの実行中照会・キャンセルには [ACX_ASYNC_JOBS.md](ACX_ASYNC_JOBS.md) の任意Profileを発見してから使います。

## 大規模グラフを取り込むとき

[PERFORMANCE.md](PERFORMANCE.md) のrelease計測例を使って、取り込み先でも検証・Index構築・シーン生成を確認してください。公開値は単一のmacOS環境でのCPU計測とGPU/Surfaceの直列診断です。60FPSやOS間の性能を保証しません。編集後の全体検証は残ります。小さな移動・構造変更・Undo/Redoは [INDEX_UPDATES.md](INDEX_UPDATES.md) の差分更新を使い、プロパティだけの編集は形状Indexを維持します。容量超過ではSceneIndexを再構築します。履歴の差分はundo_command/redo_commandから編集前に取得し、成功後に反映してください。

Group表示には既存SetGroupを使います。境界をDocumentやWebViewへ保存せず、[GROUP_RENDERING.md](GROUP_RENDERING.md) の派生描画と更新契約を使ってください。Groupの所属Node選択・名前/所属編集は [GROUP_EDITING.md](GROUP_EDITING.md) を使います。枠ドラッグは未実装です。

GPU表示に対応するHTML操作には [ACCESSIBILITY.md](ACCESSIBILITY.md) の有界ノード概要を利用してください。独自handlerへaccessible_nodesを追加し、編集にはページのrevisionを付けます。WebViewにDocumentの可変コピーを持たせず、競合時に自動再送しません。

実行値の事前検査にはDefinitionのvalidate_inputs/validate_outputsを使い、schema featureでPort名ごとのSchemaを生成できます。[PORT_VALUE_SCHEMAS.md](PORT_VALUE_SCHEMAS.md)。大容量ResourceはIDだけで表し、存在・内容・権限をホストで検証してください。

ノードの役割色・記号・補足文は [NODE_APPEARANCE.md](NODE_APPEARANCE.md) のNodeLabelsを使い、表示メタデータとしてホストから指定してください。既存の構造体リテラルには新フィールドまたはDefaultが必要です。

プロパティ編集UIを取り込むときは [PROPERTY_INSPECTOR.md](PROPERTY_INSPECTOR.md) を参照してください。定義と現在値を同じrevisionで取得し、差分を単一Batchで保存します。

描画のボトルネック確認には [RENDER_PROFILING.md](RENDER_PROFILING.md) のFrameProfiler / SurfaceRenderer::new_profiledを明示的に使用します。通常描画へ完了待ちを混ぜず、GPUパス・CPU待ち・present呼び出しを区別してください。

同一ウインドウへ組み込む場合は [WINDOW_COMPOSITION.md](WINDOW_COMPOSITION.md) を使用します。子WebViewのlabelを認可し、親WindowのSurfaceサイズとグラフ領域を分けます。macOSで検証済み、Windows/Linuxは保留です。

設定パネルを切り離す場合は [PANEL_DOCKING.md](PANEL_DOCKING.md) のホスト専用命令とWebview::reparentを使用します。同じWebViewのlabelと下書きを維持し、DocumentやRendererを複製しないでください。

配置の永続化はサンプルの `panel_preferences.rs` に分離しています。DocumentやEngine IPCを変更せず、Rustでネイティブの通常配置を記録し、通常終了時にホストの設定ファイルへ保存します。復元時の画面内補正、設定のversion検証と破損ファイル保全は [PANEL_DOCKING.md](PANEL_DOCKING.md) を参照してください。

選択へのプロパティ追従には `selection_summary` を登録し、単一選択IDとrevisionから `node_properties` を取得します。選択ではrevisionが増えないため、非同期応答の順序は通知の世代でも検証します。未保存の値があれば表示中ノードへの下書きを維持し、保存成功または明示的な破棄後に切り替えてください。[詳細](PROPERTY_INSPECTOR.md)。
