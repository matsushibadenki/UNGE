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

小さな移動は [INDEX_UPDATES.md](INDEX_UPDATES.md) の差分更新に対応しました。全体検証は残り、小さなトポロジー編集・Undo/Redoも差分更新し、容量超過時にはSceneIndexを再構築します。
曲線の適応分割、構造変更の差分更新、GPU/Surface計測の結果は以下の各節を参照してください。

## English

Run the release example above to measure CPU geometry with 10,000 nodes and 30,000 edges.
Each metric excludes one warm-up and reports median/p95/min/max. IDs, geometry and DAG topology are fixed.
Node and scene index builds include destruction; scene generation includes allocations and culling.
GPU uploads, rendering, text shaping, presentation and IPC are excluded. This is not an FPS benchmark.
BVH construction partitions at the median instead of sorting every subtree. Small moves, topology changes and undo/redo use bounded deltas; capacity overflow still rebuilds.

## 简体中文

运行上述release示例可测量10,000节点、30,000连线的CPU几何处理。
每项排除一次预热，输出中位数、p95及最小/最大值。ID、坐标和DAG拓扑固定。
Index构建包含析构，场景生成包含分配及裁剪。GPU上传、绘制、文字整形、present和IPC不在计测范围。
此结果不能证明60FPS目标。BVH构建改为按中位数分区，小规模移动、拓扑修改及Undo/Redo已支持有界差分更新；容量超限仍使用完整重建。

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

## 構造変更とUndoのIndex更新（2026-10-09）

macOS/aarch64、release、10,000ノード／30,000接続で各操作をウォームアップ後30回計測しました。同じIDを操作前後に往復させる定常ケースです。削除対象は接続4本のノードです。Undoの差分はEditorの実際の逆命令から取得します。

| 操作 | 差分中央値 (ms) | 差分p95 (ms) | 全体再構築中央値 (ms) | Undo差分中央値 (ms) |
|---|---:|---:|---:|---:|
| ノード追加 | 0.000500 | 0.000750 | 29.066875 | 0.000458 |
| ノード削除 | 0.001833 | 0.005916 | 30.000459 | 0.002709 |
| 接続追加 | 0.000667 | 0.001000 | 35.626125 | 0.000708 |
| 接続削除 | 0.000541 | 0.000709 | 38.297084 | 0.000584 |

[生JSON](benchmarks/2026-10-09-topology.json)。`cargo run --release --locked -p unge-render --example benchmark -- 30` で再現できます。各計測は正しい直前状態から始め、逆方向の更新を計測区間外で行います。先頭・末尾の描画Quadとラベルは全体再構築と照合しています。

差分時間にはCommand適用、履歴の複製、Document検証、選択解除、IPC、GPU、Surface presentを含みません。全体再構築側は生成と破棄を含みます。同じ実行内の比較であり、過去版のバイナリとの比較ではありません。計測対象にGroupはなく、多数のGroupの再集計費用は含まれません。128 IDの保持上限を越える操作、多接続ノードの削除、異なるIDへの連続操作では全体再構築も発生します。微小な差分時間は計時ノイズの影響を受けるため、アプリ全体の高速化率には換算しません。

English: On a 10,000-node / 30,000-edge fixture, small topology edits and their actual history inverses update retained indexes in bounded time. The table compares delta updates with full reconstruction in one release run (30 samples). Timings exclude edit application, validation, history cloning, selection cleanup, IPC and GPU work. The fixture has no groups, and repeated IDs do not exhaust the overlay. This is not an application latency or FPS measurement.

简体中文：在10,000节点／30,000连线的数据集上，测量小规模拓扑修改及实际历史逆命令的Index差分更新。表中比较同一次release运行的差分更新和完整重建（30次）。不含编辑应用、验证、历史复制、选择清理、IPC或GPU；数据集无Group，重复ID不会耗尽更新容量。不能将结果视为应用整体延迟或FPS。


## GPU / Native Surface（2026-10-09）

Apple M4、Metal、macOS/aarch64、release（thin LTO）、1280×720物理px、DPI倍率1、Fifoで計測しました。再現方法とAPIは [RENDER_PROFILING.md](RENDER_PROFILING.md)。10,000ノード／30,000接続の共通fixtureを使い、各条件で5成功フレームを除外して30フレームを計測しています。

| テーマ | 言語 | 表示 | Scene CPU中央値 (ms) | GPUパス中央値 (ms) | 診断全体 中央値 / p95 (ms) |
|---|---|---|---:|---:|---:|
| dark | en | near | 0.057 | 0.687 | 8.361 / 9.513 |
| dark | en | overview | 4.539 | 2.516 | 10.442 / 11.063 |
| dark | ja | near | 0.049 | 0.684 | 8.253 / 9.262 |
| dark | ja | overview | 4.536 | 2.522 | 10.644 / 11.393 |
| dark | zh-cn | near | 0.134 | 0.687 | 8.326 / 9.206 |
| dark | zh-cn | overview | 4.504 | 2.525 | 10.642 / 11.791 |
| light | en | near | 0.058 | 0.685 | 8.375 / 8.850 |
| light | en | overview | 4.376 | 2.517 | 10.039 / 10.748 |
| light | ja | near | 0.063 | 0.684 | 8.375 / 9.467 |
| light | ja | overview | 4.395 | 2.519 | 10.511 / 11.149 |
| light | zh-cn | near | 0.117 | 0.686 | 8.361 / 9.124 |
| light | zh-cn | overview | 4.478 | 2.523 | 10.604 / 11.688 |

[生JSONと各工程の詳細](benchmarks/2026-10-09-surface.json)。12条件の計360フレームでSurfaceスキップ0、GPUタイムスタンプ欠損0、missing glyph 0、GPU Validationエラーなし。近景は36ノード／110接続／1,173 Quad／108ラベル、全体表示は10,000ノード／30,000接続／55,347 Quad／ラベルなしです。

診断全体はScene生成開始からSurface取得、CPU準備、描画submit、GPU完了待ち、別submitでのクエリーresolveと16バイト読込、CPUのpresent呼び出しまでです。GPUパスの時間にはCPU準備や転送、Surface待機、presentを含みません。両者は足し合わせません。描画完了後にクエリーを別submitでresolveし、今回のMetal環境で観測した古い終了値の読込を回避しています。

これは1台での単一実行であり、通常の非同期描画FPSを示しません。毎回のGPU完了待ちと追加submitに診断固有の費用があり、VSync、compositor、他アプリの負荷にも左右されます。全体表示では低LODにより文字がなく、言語ごとの差は文字描画の性能差ではありません。180×90の基本カードのfixtureなので、大きなカードのアイコン／説明文、画像プレビュー、Group、ドラッグ、WebViewと共有Engineの競合は含みません。今後は通常の非同期描画と連続操作、同一ウインドウ合成を検証します。Windows/Linuxは実行環境がないため保留です。

English: The release native diagnostic completed 12 theme/language/zoom cases × 30 measured frames on Apple M4 / Metal at 1280×720, scale 1, Fifo. All 360 GPU timestamp samples were available, with no skipped Surface frames, missing glyphs or validation errors. Median GPU pass time was 0.684–0.687 ms nearby and 2.516–2.525 ms for the overview. Total diagnostic time includes scene generation, CPU work, completion waits, a separate query-resolution submission and the CPU present call. This serialized diagnostic does not establish animation FPS or display latency. The overview omits text; language differences there are measurement variation. Larger cards, previews, groups, interaction, WebView contention and other operating systems are outside this measurement.

简体中文：在Apple M4／Metal、1280×720、缩放倍率1、Fifo下，release原生诊断完成12种主题／语言／缩放组合，每组30个计测帧。360个GPU时间戳均可用，Surface跳过、缺字和验证错误均为0。近景GPU Pass中位数为0.684–0.687ms，全景为2.516–2.525ms。诊断总时间包含场景生成、CPU工作、完成等待、单独提交的查询解析及CPU的present调用。该串行诊断不能证明动画FPS或屏幕显示延迟。全景省略文字，语言间的差异属于测量波动。大卡片、预览、Group、交互、WebView竞争及其他操作系统不在此次测量范围内。
