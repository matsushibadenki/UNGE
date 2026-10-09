# GPU / Surfaceの描画計測

## 用途

通常描画を変更せず、明示的な診断呼び出しでCPU準備・GPU描画・Surface取得の時間を分離します。Document・Scene・描画バッファはRust/GPUに保持し、IPCにフレーム画像を送りません。タイムスタンプのreadbackは1フレーム16バイトだけです。

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

GPU完了を毎回待つためCPU/GPUを直列化した診断です。通常描画のスループット、アニメーションFPS、画面走査までの遅延を測るものではありません。OSのVSyncやcompositor、DPI、キャッシュ、表示領域で結果が変わります。60FPS達成の判定には、今後、通常の非同期描画と連続操作を含む計測が必要です。

## English

Opt-in native profiling separates CPU preparation, encoding/submission, completion waits, Surface acquisition and the CPU present call. Supported devices additionally report render-pass GPU timestamps; unavailable values remain None. Normal rendering does not acquire queries or wait for completion. Only 16 timestamp bytes are read back, with no frame transfer through IPC. Query resolution uses a separate submission after render completion because same-submission resolution returned stale end counters in the tested Metal environment. The additional submission and wait are included in completion_wait_ms.

Run the Tauri host with `--benchmark-surface` after a release build. It opens a dedicated native window, measures 12 combinations of theme/language/zoom on a shared 10,000-node / 30,000-edge fixture, outputs JSON and exits. Each case uses five successful warm-ups and 30 measured frames. Skipped frames are excluded and counted. The diagnostic waits for completion each frame, so it measures neither asynchronous animation throughput nor compositor/display scanout latency. See PERFORMANCE.md for measured conditions and results.

## 简体中文

可选的原生计测分别记录CPU准备、编码／提交、完成等待、Surface获取和CPU的present调用。支持的设备还报告描画Pass的GPU时间戳；不可用值为None。普通描画不新增查询或完成等待。仅回读16字节时间戳，不通过IPC传输帧图像。测试中的Metal环境在同一次提交内解析查询时返回过旧的结束值，因此在渲染完成后另行提交查询解析。额外提交和等待的开销计入completion_wait_ms。

release构建后，以 `--benchmark-surface` 启动Tauri示例。它打开专用原生窗口，在10,000节点／30,000连线的共用数据集上计测主题／语言／缩放的12种组合，输出JSON并退出。每组预热5个成功帧，计测30帧；跳过帧单独计数。此诊断逐帧等待GPU完成，不能代表异步动画吞吐量或compositor／屏幕扫描延迟。条件和结果见PERFORMANCE.md。
