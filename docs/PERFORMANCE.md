# 大規模グラフの計測 / Performance / 性能测量

## 日本語

```sh
cargo run --release --locked -p unge-render --example benchmark -- 20 > benchmark.json
```

引数は計測回数（1〜1000、既定20）です。各項目はウォームアップ1回を除き、中央値・p95・最小値・最大値をJSONで返します。
毎回10,000ノードと30,000エッジのDAGをCommand::Batchで作り、件数とDocumentの妥当性を検証します。
ID、座標、Port、接続は固定です。100×100の格子に隣接ノードへの前方向接続を置きます。
入力PortはMultiple、ノード処理・PropertySchema検証は含めません。

| 項目 | 計測範囲 |
|---|---|
| fixture_batch_ms | 1回の初期生成・Command適用・検証。比較の対象にはしない |
| document_validation | Documentの構造・接続・DAG・placement検証 |
| node_index_build | ノードBVH構築。戻り値の破棄を含む |
| scene_index_build | 描画用ノード/エッジ/接続Indexと両BVH構築。戻り値の破棄を含む |
| node_query | 通常表示範囲の検索と結果の整列 |
| normal_scene | 1400×900、zoom=1でのカリング・曲線・Port・文字ラベルのCPU生成 |
| overview_scene | 同じサイズ、zoom=0.02で全体表示。低LODで文字・Portを省略 |
| move_command_validation | 1ノード移動のCommand適用と全体検証。SceneIndex再構築は含まない |

通常表示と全体表示は可視件数、Quad数、文字ラベル数も記録します。
CPU側のシーン生成・割り当て・破棄を含み、文字の整形/Glyph Atlas、GPUへのアップロード、
GPUの処理時間、Surface present、WebView、Tauriロック待ちは計測しません。
FPS、総ヒープ容量、他OSの性能の検証には使えません。性能目標60FPSは引き続き未検証です。

## 最適化

BVHの各階層で全要素を並べ替える代わりに、中央値で左右を分割します。
平衡木と検索結果のID順、重なり時のhit_test規則は維持します。
検索結果は独立した総当たり検索、入力順の反転、同一座標、境界、無効矩形で検証します。
Rust標準ライブラリの [select_nth_unstable_by](https://doc.rust-lang.org/std/primitive.slice.html#method.select_nth_unstable_by) を使用します。

小さな移動は [INDEX_UPDATES.md](INDEX_UPDATES.md) の差分更新に対応しました。全体検証は残り、トポロジー編集・Undo/Redo・容量超過時にはSceneIndexを再構築します。
大規模な全体表示では曲線のCPU生成量も増えるため、次は曲線生成量の改善を検討します。

## English

Run the release example above to measure CPU geometry with 10,000 nodes and 30,000 edges.
Each metric excludes one warm-up and reports median/p95/min/max. IDs, geometry and DAG topology are fixed.
Node and scene index builds include destruction; scene generation includes allocations and culling.
GPU uploads, rendering, text shaping, presentation and IPC are excluded. This is not an FPS benchmark.
BVH construction partitions at the median instead of sorting every subtree. Small move deltas are now supported; topology changes, undo/redo and capacity overflow still rebuild.

## 简体中文

运行上述release示例可测量10,000节点、30,000连线的CPU几何处理。
每项排除一次预热，输出中位数、p95及最小/最大值。ID、坐标和DAG拓扑固定。
Index构建包含析构，场景生成包含分配及裁剪。GPU上传、绘制、文字整形、present和IPC不在计测范围。
此结果不能证明60FPS目标。BVH构建改为按中位数分区，小规模移动已支持差分更新；拓扑修改、Undo/Redo和容量超限仍使用完整重建。

## 2026-10-05 macOSでの結果

Rust 1.98.1、macOS/aarch64、release（thin LTO）、同じ機械で改善前→改善後を順に30回ずつ計測しました。CPU型番とメモリ量は取得していません。

結果はこの固定データでの単一比較です。稼働中の他アプリ、メモリ圧力、温度による変動を含み、全環境での改善率を保証しません。初回20回の結果はビルド直後で変動が大きかったため、ビルドと検証終了後の30回比較を記録しています。

| CPU処理 | 改善前 中央値 / p95 (ms) | 改善後 中央値 / p95 (ms) |
|---|---:|---:|
| node_index_build | 3.880 / 4.354 | 2.563 / 2.675 |
| scene_index_build | 40.857 / 64.048 | 30.356 / 81.962 |
| node_query | 0.001 / 0.004 | 0.001 / 0.003 |
| normal_scene | 0.086 / 0.089 | 0.085 / 0.089 |
| overview_scene | 12.092 / 14.742 | 10.178 / 12.743 |
| document_validation | 19.851 / 24.099 | 19.777 / 24.209 |
| move_command_validation | 24.751 / 29.934 | 22.015 / 38.088 |

BVH構築の中央値は約34%、SceneIndex構築は約26%短縮しました。変更していない検証やシーン生成の値も変動しています。クエリ時間は同程度でした。

通常表示は42ノード・174エッジ・4,345 Quad・126ラベル。全体表示は10,000ノード・30,000エッジ・260,001 Quad・文字なしです。改善前後で件数は一致しました。

[改善前JSON](benchmarks/2026-10-05-before.json) · [改善後JSON](benchmarks/2026-10-05-after.json)

English: On this single macOS/aarch64 fixture, median node BVH build time fell from 3.880 to 2.563 ms and SceneIndex build from 40.857 to 30.356 ms. Other timing changes include measurement noise. GPU frame time remains unmeasured.

简体中文：在此单一macOS/aarch64数据集上，节点BVH构建中位数由3.880降至2.563ms，SceneIndex构建由40.857降至30.356ms。其他计时变化包含测量波动，GPU帧时间仍未计测。

## 小さな移動のIndex更新（2026-10-05）

同じ10,000ノード/30,000エッジのrelease例を30回計測しました。今回は同一ノード（接続4本）を繰り返し更新する定常ケースです。ウォームアップ後の更新集合は1ノード・4エッジで、容量超過による再構築は含みません。

差分計測にはCommand適用・Document検証・選択整合性・Tauriロック待ちを含みません。比較の全体再構築は構築と破棄を含みます。過去の計測との性能比較ではなく、今回の同一実行内の比較です。

| CPU処理 | 中央値 (ms) | p95 (ms) |
|---|---:|---:|
| move_index_delta | 0.000500 | 0.000666 |
| move_index_full_rebuild | 12.670958 | 12.783625 |
| document_validation | 7.768834 | 7.967750 |
| delta_normal_scene | 0.038875 | 0.040542 |
| delta_overview_scene | 4.397625 | 4.523000 |

[生JSON](benchmarks/2026-10-05-delta.json)。変更IDが増えるケース、容量上限を越えた再構築、接続の多いノードでは費用が異なります。

English: This steady-state benchmark repeats one moved node with four incident edges. Delta timing excludes document validation, edit application, selection cleanup and IPC. Full rebuild timing includes construction and destruction. Capacity overflow and high-degree moves use full rebuilds.

简体中文：本次定常计测重复更新一个节点及其四条相连Edge。差分计时不含Document验证、编辑应用、选择清理和IPC；完整重建包含构建与析构。容量超限及连接较多的节点使用完整重建。

## 適応曲線の比較（2026-10-05）

同じmacOS/aarch64、Rust 1.98.1、releaseで、固定8/24分割の旧バイナリと適応分割の新バイナリを順に30回ずつ計測しました。単一データでの比較で、GPUアップロード/描画/present、FPSは含みません。

| 表示 | 旧Quad数 | 新Quad数 | 旧CPU中央値 / p95 (ms) | 新CPU中央値 / p95 (ms) |
|---|---:|---:|---:|---:|
| normal | 4,345 | 1,225 | 0.038 / 0.041 | 0.025 / 0.030 |
| overview | 260,001 | 55,347 | 4.181 / 4.249 | 2.578 / 2.945 |

全体表示のQuad数は約78.7%、CPUシーン生成の中央値は約38.3%減少しました。主に同じ高さの接続を持つ格子データの結果です。複雑な曲線では生成量が増える場合もあります。

ノード・Edge・ラベルの件数は改善前後で一致しました。GPUへのQuadペイロード量もQuad数に比例して減りますが、GPU処理時間の短縮率は計測していません。

[旧方式JSON](benchmarks/2026-10-05-curves-before.json) · [適応分割JSON](benchmarks/2026-10-05-curves-after.json)

English: This single grid fixture reduced overview quads from 260,001 to 55,347 and median CPU scene time from 4.181 to 2.578 ms. Visible node, edge and label counts match. Complex curves may increase geometry; GPU frame time remains unmeasured.

简体中文：此单一格子数据集的全景Quad由260,001降至55,347，CPU场景生成中位数由4.181降至2.578ms。可见节点、Edge和文字数量一致。复杂曲线可能增加几何量，GPU帧时间仍未计测。
