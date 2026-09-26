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
3. `Definition` に安定したtype_id、意味を説明する3言語の文言、version、Portを定義する。
4. `NodeExecutor` を実装し、`Registry::register` で登録する。
5. `Scheduler::run` にDocumentのグラフを渡す。実行中の編集を許すなら、Rust内で取得したsnapshotを実行する。
6. Tauriホストで `Engine::new(document)` を作り、`app.manage(engine)` する。ウインドウは `register_view` で登録する。
7. `invoke_handler(unge_tauri::handler())` を接続する。既存命令と共存するときは `generate_handler![unge_tauri::dispatch, unge_tauri::inspect, your_command]` を使う。
8. `createClient(invoke)` でWebViewから操作する。描画フレームを返す命令を追加しない。
9. Rustネイティブウインドウから `SurfaceRenderer` を作り、Engineに登録する。実装例は `examples/tauri-host/src/main.rs`。
10. ノードを選んだときだけinspectを呼び、`unge://changed`イベントで小さなSummaryを受け取る。

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

`math_registry()` が最小の実例です。`NodeExecutor::execute` はBoxFutureを返すためdyn traitとして登録できます。
`Inputs` はPort名→Value列です。Single入力も1要素の列として受け取ります。
Multiple入力の値の順序はEdge ID順です。意味のある順序が必要なアプリは明示的な順序プロパティを設計してください。
出力名・型・必須出力は実行後に検証され、違反はFailedになります。

CPU処理でasync executorを塞がないよう、ホストの `spawn_blocking` / Rayonなどへ処理を逃がしてください。
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
- IPCのTypeScript型は手書きのため、Rust enum変更時に必ず同期する。

## 移植範囲

- core/executor: TauriなしでCLI・サーバーに使用可能。
- render: wgpuのRust Device/TextureViewを受け取る。ホストのネイティブウインドウと組み合わせる。
- tauri: Tauri 2専用。グラフ変更はworkerへ移し、Surfaceの生成・描画スレッドはホストで調整する。
- ブラウザー単独版: wgpuのWebGPUバックエンドを利用するWASMホストは未提供。UUIDのWASM設定、Canvas初期化、非Send future等の対応が必要。

## ACXエージェントから実行中のグラフを操作する

コード生成の引き継ぎに加え、実行中のノード操作には `unge-acx` を利用できます。
[ACX_INTEGRATION.md](ACX_INTEGRATION.md) を読み、`examples/acx-provider/agent.py` と同じライフサイクルを使ってください。
通常UIと同じRustのEngineに接続し、別Documentを作って同期する設計にしないでください。
事前確認のdigest・内容・revisionを照合し、競合時には新しく観測してpreflightを作り直します。
