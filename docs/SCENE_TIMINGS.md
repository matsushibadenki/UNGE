# Scene生成の工程計測

## 日本語

`SceneIndex::scene_with_theme_timed` は通常の `scene_with_theme` と同じ入力を受け、成功時に `(Scene, SceneTimings)` を返します。不正なViewport／PreviewはErrを返します。通常のScene APIは時計を読みません。診断APIにもGPU待機やreadbackを追加しません。

```rust,ignore
let (scene, timings) = index.scene_with_theme_timed(
    viewport, &selection, &preview, &catalog, locale, theme,
)?;
```

| 値 (ms) | 測定範囲 |
|---|---|
| groups_ms | Groupの可視検索、形状・見出し生成 |
| edge_query_ms | EdgeのBVH検索、プレビュー候補との和集合 |
| edge_geometry_ms | Edge参照、形状／境界、役割色、曲線分割、Quad生成 |
| node_query_ms | NodeのBVH検索、プレビュー候補との和集合 |
| node_geometry_ms | Node参照、選択・形状・Port・文字生成 |
| overlays_ms | 新規接続プレビュー、選択矩形 |
| total_ms | 検証・背景・全工程を含むScene生成全体 |

各工程に計時計数があり、totalには工程外の検証・背景処理も含みます。統計の中央値やp95を加算してもtotalの統計値にはなりません。Sceneの破棄・GPU準備／描画・共有ロック待ち・IPCは含みません。診断結果の記録量はホストで制限してください。構造はRustホスト用で、IPC／TypeScriptの契約は変更しません。

### 保持するEdge情報

編集確定後のRenderEdgeに曲線の境界と接続元のtype_idを保持します。毎フレームの境界再計算と、役割色の解決に必要だったNodeマップの再検索を省きます。役割色はその時点のCatalogとThemeから取得するので、色を固定しません。移動・接続変更・ノード再作成・Undo／Redoで関連Edgeを最終Documentから作り直し、一時移動中は一時座標から曲線と境界を再計算します。直接Documentを置換した場合は引き続きIndex全体を再構築してください。

追加保持量は64bit環境で1 EdgeあたりRect 16バイト＋String本体24バイト＋type_idのUTF-8バイト列です。30,000 Edge／9バイトのtype_idでは約1.47MBの追加ペイロードで、アロケーター／BTreeMapのオーバーヘッドは含みません。SceneIndexの構築や編集時にも型文字列の複製費用が増えます。Viewごとの巨大フレームやIPCキャッシュは追加しません。曲線の分割精度・上限、描画順、保存形式は変更しません。

### 再現

```sh
cargo run --release --locked -p unge-render --example zoom_benchmark
```

6段階Zoomで各5回予熱＋30回計測。通常SceneのCPU時間とは別に、同じ状態の診断Sceneを各回生成してphase_rawを記録します。計測順は通常→診断で、キャッシュ／負荷の影響があります。出力指紋は暗号学的な証明ではなく変更前後の確認用です。[測定結果](PERFORMANCE.md)。連続Surfaceは既存 `--benchmark-animation` で別に検証します。

## English

`scene_with_theme_timed` accepts the same inputs as `scene_with_theme` and returns `(Scene, SceneTimings)` on success. Invalid views/previews return Err. Normal Scene APIs do not read the clock; diagnostics add no GPU wait/readback. Timings separate groups, edge queries, edge geometry (including lookup, color, tessellation and quads), node queries, node geometry and overlays. Total also includes validation/background and clock overhead. Phase percentiles are not additive. Scene destruction, GPU, locks and IPC are excluded; hosts must bound sample retention. These are Rust host diagnostics, with no IPC/TypeScript change.

Committed RenderEdges now retain bounds and source type IDs. Preview movement recomputes geometry/bounds; edits, rewiring, node recreation and history reconstruct affected edges from the final Document. Colors still resolve against the current Catalog/Theme. On 64-bit hosts the extra payload is 40 bytes plus UTF-8 type bytes per edge: about 1.47 MB for 30,000 edges with 9-byte IDs, excluding allocator/map overhead. Construction/edit costs include string cloning. Tessellation, draw order and serialization remain unchanged.

The release zoom example records 30 ordinary samples and 30 separate phase samples after five warm-ups at each of six zoom levels. Ordinary then diagnostic ordering can affect caches. Native animation is verified separately; see [results](PERFORMANCE.md).

## 简体中文

`scene_with_theme_timed` 与普通接口输入相同，成功时返回 `(Scene, SceneTimings)`，无效视口／预览返回Err。普通Scene接口不读取时钟，诊断不增加GPU等待或回读。分别测量Group、Edge查询、Edge形状（含查找、颜色、曲线细分、Quad）、Node查询、Node形状及覆盖层；total还含验证／背景／计时成本。分位数不能相加。不含Scene销毁、GPU、锁或IPC，宿主需限制样本保存量。仅新增Rust诊断，IPC／TypeScript不变。

已提交的RenderEdge保存边界及源type_id。临时移动重算曲线／边界；编辑、重连、节点重建及历史从最终Document更新相关Edge。颜色仍使用当前Catalog／Theme。64位环境额外载荷为每Edge 40字节加type_id的UTF-8字节：30,000 Edge、9字节type_id约1.47MB，不含分配器／Map开销。构建及编辑增加字符串复制成本。细分精度、绘制顺序及保存格式不变。

release示例对六档Zoom分别预热5次，记录30个普通样本及30个独立阶段样本。先普通后诊断，缓存和负载可能影响比较。原生动画另行验证。参见[结果](PERFORMANCE.md)。
