# GPU / Surfaceの描画計測

## 用途

通常描画のCPU計測と、GPU完了待ちを含む直列診断の2経路を提供します。明示的な診断呼び出しでCPU準備・GPU描画・Surface取得の時間を分離します。Document・Scene・描画バッファはRust/GPUに保持し、IPCにフレーム画像を送りません。タイムスタンプのreadbackは1フレーム16バイトだけです。

### Native Surface

```rust,ignore
let mut surface = SurfaceRenderer::new_profiled(window, physical_size).await?;
if let Some(sample) = surface.draw_profiled(&scene, viewport)? {
    // 保存先・集計・計測の頻度はホストが管理する。
    println!("GPU pass: {:?} ms", sample.frame.gpu_pass_ms);
}
```

`new_profiled` はAdapterが対応する場合にのみTIMESTAMP_QUERYを要求します。非対応Deviceでも完了待ち付き計測を続け、GPUパス時間はNoneです。通常の `new` / `draw` はクエリー取得や完了待ちを追加しません。通常コンストラクターで `draw_profiled` を呼ぶと設定エラーを返します。

最小化・Surface Lost/Outdated/TimeoutではNoneを返します。計測サンプルに0msとして混ぜないでください。Lost/Outdatedは従来どおり再設定し、後のフレームで再試行します。Device Pollの完了待ちは5秒を上限とし、エラー時もreadbackのmappingを解除します。

### オフスクリーン／ホスト所有のTextureView

`FrameProfiler::new(&device)` と `measure` を使用します。Device生成時に、利用可能であればTIMESTAMP_QUERYを有効にしてください。Device・Queue・GpuRenderer・TextureView・Profilerは同じDevice由来である必要があります。Profilerは2要素のQuerySet、resolve buffer、16バイトreadback bufferを再利用します。描画完了を待った後に別submitでクエリーをresolveして読み込みます。今回のMetal環境では同じsubmit内のresolveで古い終了値を観測したため、診断経路ではこの同期を行います。追加submitと待機の費用もcompletion_wait_msに含みます。

`GpuRenderer::render_with_timestamps` はホストが提供するtimestamp writesを受け付けます。通常の `render` はNoneを指定する既存経路です。計測APIはネイティブ用で、ブラウザーWebGPUの非同期イベントループに対応するAPIではありません。

## 各数値の範囲

| フィールド | 測定内容 |
|---|---|
| prepare_ms | CPU側の形状転送要求、文字整形、Glyph Atlas準備 |
| encode_submit_ms | コマンド記録とQueueへのsubmit |
| completion_wait_ms | CPUがGPU完了／readbackを待つ時間。GPU実行時間そのものではない |
| gpu_pass_ms | GPUタイムスタンプの差×Queueのtimestamp period。描画パスのみ。非対応・無効な差はNone |
| completed_frame_ms | prepare開始から完了待ち／timestamp読込終了まで |
| acquire_ms | Surfaceから次のTextureを取得するCPU時間 |
| present_call_ms | CPUのpresent呼び出し時間。OS compositorや画面走査の完了時間ではない |
| total_ms | Surface取得から完了待ち、present呼び出しまで |

GPUパス時間は他のCPU時間に加算して合計を作る値ではありません。CPUとGPUは一部並行します。Queueに先行処理があれば完了待ちに影響します。GPUパスにはシーン生成・文字準備・転送・presentを含みません。

## 再現用のTauri起動モード

```sh
cargo build --release --locked -p unge-tauri-host
./target/release/unge-tauri-host --benchmark-surface > /tmp/unge-surface.json
```

このモードは専用ネイティブウインドウだけを作り、計測終了後に終了します。通常の操作WebView・ACX・実行サービスを起動しません。CPUベンチマークと共通のfixtureで10,000ノード・30,000接続を作ります。各ノードは180×90の互換サイズ、LabelCatalogに青の役割色と3言語タイトルを設定します。大きなカードのアイコン／説明文や画像プレビューの負荷はこのfixtureに含みません。

Dark/Light × 英語/日本語/简体中文 × 近景/全体表示の12条件で、5成功フレームをウォームアップし、30成功フレームを計測します。失敗・スキップは件数を別記し、45回の試行内で必要数に達しない場合は失敗にします。missing glyphとGPU Validationエラーも失敗扱いです。

JSONはAdapter名、Backend、present mode、実際の物理解像度、DPI倍率、可視Node/Edge/Quad/文字数、各工程の中央値・p95、最初のフレーム、Surface計測の生データを含みます。`scene` はScene生成、`total` はScene生成からSurfaceのpresent呼び出しまでの実測です。統計・JSON生成、Sceneの破棄はその計測区間に含みません。先頭フレームは各条件の最初の成功フレームであり、後の条件では既存キャッシュが残っています。

### 解釈上の制限

GPU完了を毎回待つためCPU/GPUを直列化した診断です。通常描画のスループット、アニメーションFPS、画面走査までの遅延を測るものではありません。OSのVSyncやcompositor、DPI、キャッシュ、表示領域で結果が変わります。通常経路の連続操作計測は以下を参照してください。どちらの経路も表示完了や60FPS達成を保証しません。

## English

Opt-in native profiling separates CPU preparation, encoding/submission, completion waits, Surface acquisition and the CPU present call. Supported devices additionally report render-pass GPU timestamps; unavailable values remain None. Normal rendering does not acquire queries or wait for completion. Only 16 timestamp bytes are read back, with no frame transfer through IPC. Query resolution uses a separate submission after render completion because same-submission resolution returned stale end counters in the tested Metal environment. The additional submission and wait are included in completion_wait_ms.

Run the Tauri host with `--benchmark-surface` after a release build. It opens a dedicated native window, measures 12 combinations of theme/language/zoom on a shared 10,000-node / 30,000-edge fixture, outputs JSON and exits. Each case uses five successful warm-ups and 30 measured frames. Skipped frames are excluded and counted. The diagnostic waits for completion each frame, so it measures neither asynchronous animation throughput nor compositor/display scanout latency. See PERFORMANCE.md for measured conditions and results.

## 简体中文

可选的原生计测分别记录CPU准备、编码／提交、完成等待、Surface获取和CPU的present调用。支持的设备还报告描画Pass的GPU时间戳；不可用值为None。普通描画不新增查询或完成等待。仅回读16字节时间戳，不通过IPC传输帧图像。测试中的Metal环境在同一次提交内解析查询时返回过旧的结束值，因此在渲染完成后另行提交查询解析。额外提交和等待的开销计入completion_wait_ms。

release构建后，以 `--benchmark-surface` 启动Tauri示例。它打开专用原生窗口，在10,000节点／30,000连线的共用数据集上计测主题／语言／缩放的12种组合，输出JSON并退出。每组预热5个成功帧，计测30帧；跳过帧单独计数。此诊断逐帧等待GPU完成，不能代表异步动画吞吐量或compositor／屏幕扫描延迟。条件和结果见PERFORMANCE.md。


## 通常描画と連続操作のCPU計測（2026-10-11）

```rust,ignore
let mut surface = SurfaceRenderer::new(window, physical_size).await?;
if let Some(sample) = surface.draw_timed(&scene, viewport)? {
    // Rustホスト側で記録。保存件数・集計頻度はホストが制限する。
    println!("CPU prepare: {} ms", sample.prepare_ms);
}
// 同一ウインドウの左側だけを描画するホスト用。
let sample = surface.draw_region_timed(&scene, viewport, graph_size)?;
```

`CpuSurfaceFrameTimings` は acquire_ms / prepare_ms / encode_submit_ms / present_call_ms / total_ms を返します。`draw` / `draw_region` と同じ内部経路を使用し、通常APIでは時計取得も無効になります。計測APIもGPU完了待ち・timestamp query・readbackを追加しません。Surface取得、submit、present内部でドライバーやVSyncによる待機が生じることはあります。total_msはSurface取得前からCPUのpresent終了までで、GPU処理終了を意味しません。

最小化・空の描画領域・Surface Lost/Outdated/TimeoutはNoneです。範囲外の描画領域や致命的なSurfaceエラーはErrです。スキップを0msの成功サンプルにしないでください。既存のblocking診断APIは引き続き利用できます。

### 再現する

```sh
cargo build --release --locked -p unge-tauri-host
./target/release/unge-tauri-host --benchmark-animation > /tmp/unge-animation.json
```

ネイティブイベントループのMainEventsClearedごとに入力を1ステップ進め、1回描画します。次のターンを起こすイベントを1件だけ送信して戻り、各フレームのGPU完了は待ちません。独立したRustの計測セッションがDocument・View・Rendererを所有します。WebView・ACX・実行サービス・既存のパネル配置ファイルは使用しません。

- 10,000ノード／30,000接続、180×90の基本カード、1280×720論理pxのウインドウ。実際の物理サイズとDPIをJSONに記録。
- Dark/Light × 英語/日本語/简体中文 × Pan/Zoom/Dragの18条件。それぞれ成功5フレームの予熱後、60成功フレームを保持。
- PanとDragはDown→11回Move→Upの13ステップを繰り返し、96×36論理pxを交互方向へ移動。Dragは既存Interactionを通じ、UpでEditorのCommandを適用してSceneIndexを差分更新。スキップなしの場合は各条件5回の編集を確定。
- Zoomは原点を固定して1.0→0.02→1.0を60ステップで往復。全体表示の低LODと文字が出る近景を通過。
- 各条件でDocument・Viewをリセット。Rendererと文字キャッシュは再利用。条件切り替えの構築費用は次の測定に混ぜない。
- 1条件75試行、全体180秒をターン間で確認。必要数不足・missing glyph・GPU Validationエラーは失敗。OSやドライバー内で停止した呼び出しを強制中断する機能ではない。

### JSONの読み方

`update` は入力処理・Command検証と確定・履歴・Index更新、`scene` は可視Scene生成、`total` は入力開始からCPU present終了までです。`interval` は成功フレームの入力処理開始同士の間隔。スキップした試行とイベントループの費用も次の成功間隔に含みます。予熱の最後の成功開始から最初の計測開始までを含む60間隔です。統計・JSON出力は条件末尾で行います。Scene破棄やサンプル保存の費用はtotalには含まず、次のintervalに含みます。

`start_rate_hz` は60間隔の合計から求めるCPU側の開始頻度です。`intervals_over_16_667_ms` は60Hz相当の間隔を超えた件数で、画面の欠落フレーム数ではありません。生データにはstep・zoom・可視件数・全工程の時間を保存します。`committed_edits` は予熱を含む条件全体のrevision増加、`skipped` は成功計測に含めなかった試行数です。

これは決定的な合成入力であり、OSからの実マウス入力、入力デバイスの遅延、共有Engineのロック競合、WebView合成、画像プレビュー、Group、大きなカードは含みません。1秒程度の条件別サンプルで、長時間の安定性も判定しません。FPSや入力から画面表示までの遅延には換算しないでください。

### English — normal rendering

Use `SurfaceRenderer::new` with `draw_timed` / `draw_region_timed` to obtain `CpuSurfaceFrameTimings` from the normal rendering path. No GPU completion wait, timestamp query or readback is added; acquisition, submission or presentation may still block inside the driver. Untimed draw calls do not read the clock. Skipped frames return None; invalid regions and fatal errors return Err.

`--benchmark-animation` advances one deterministic input step per native event-loop turn using the normal queued Surface path. It measures 18 theme/language/motion combinations, five successful warm-ups and 60 samples each, on 10,000 nodes / 30,000 edges. Pan/drag repeat 13-step gestures; drag commits through Editor and updates SceneIndex on release. Zoom sweeps 1.0–0.02. Cases reset Document/View while retaining GPU/font caches. Limits are 75 attempts per case and a 180-second deadline checked between turns.

The report separates input/edit/index, scene, acquire, prepare, encode/submit and present CPU costs. Successful-start intervals include event-loop overhead and skipped attempts. Start rate is CPU cadence, and intervals above 16.667 ms are not dropped display frames. This short synthetic workload excludes real OS input latency, shared Engine contention, WebView composition, previews, groups and larger cards. It does not measure GPU completion, display FPS or input-to-display latency.

### 简体中文 — 普通渲染

使用 `SurfaceRenderer::new` 和 `draw_timed` / `draw_region_timed`，通过普通渲染路径获取 `CpuSurfaceFrameTimings`。不增加GPU完成等待、时间戳查询或回读；驱动内部的获取、提交或present仍可能阻塞。未计测的draw不读取时钟。跳过帧返回None，非法区域和致命错误返回Err。

`--benchmark-animation` 在每次原生事件循环中推进一步确定性的输入，使用普通Surface提交路径。在10,000节点／30,000连线上测试18种主题／语言／操作组合，每组预热5个成功帧后记录60帧。Pan／Drag重复13步手势，Drag释放时通过Editor提交并更新SceneIndex；Zoom在1.0–0.02间往返。各组重置Document／View，保留GPU与字体缓存。每组上限75次尝试，全局180秒期限在循环之间检查。

报告分离输入／编辑／Index、场景生成、获取、准备、编码／提交和present的CPU成本。成功开始间隔包含事件循环与跳过尝试的耗时。开始频率仅代表CPU节奏，超过16.667ms的间隔不等于屏幕丢帧。短时间合成输入不包含真实OS输入延迟、共享Engine竞争、WebView合成、图片预览、Group或大卡片，也不测量GPU完成、屏幕FPS或输入到显示的延迟。
