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
| move_command_validation | 1ノード移動のCommand適用と検証。現在は移動のみの局所検査。SceneIndex再構築は含まない |

通常表示と全体表示は可視件数、Quad数、文字ラベル数も記録します。
CPU側のシーン生成・割り当て・破棄を含み、文字の整形/Glyph Atlas、GPUへのアップロード、
GPUの処理時間、Surface present、WebView、Tauriロック待ちは計測しません。
FPS、総ヒープ容量、他OSの性能の検証には使えません。性能目標60FPSは引き続き未検証です。

## 最適化

BVHの各階層で全要素を並べ替える代わりに、中央値で左右を分割します。
平衡木と検索結果のID順、重なり時のhit_test規則は維持します。
検索結果は独立した総当たり検索、入力順の反転、同一座標、境界、無効矩形で検証します。
Rust標準ライブラリの [select_nth_unstable_by](https://doc.rust-lang.org/std/primitive.slice.html#method.select_nth_unstable_by) を使用します。

小さな移動は [INDEX_UPDATES.md](INDEX_UPDATES.md) の差分更新に対応しました。移動以外の全体検証とホスト検証は残り、小さなトポロジー編集・Undo/Redoも差分更新し、容量超過時にはSceneIndexを再構築します。
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


## 通常描画と連続操作（2026-10-11）

Apple M4／Metal、macOS/aarch64、release（thin LTO）、1280×720物理px、DPI倍率1、Fifoで計測。10,000ノード／30,000接続に対して、ネイティブイベントループから合成Pan／Zoom／Dragを送ります。各条件は5成功フレームの予熱後、60成功フレーム。通常描画経路を使用し、GPU完了待ちやreadbackを追加しません。[再現コマンドとAPI](RENDER_PROFILING.md)。

| テーマ | 言語 | 操作 | 更新p95 (ms) | Scene p95 (ms) | 開始間隔 中央値 / p95 (ms) |
|---|---|---|---:|---:|---:|
| dark | en | pan | 0.002 | 0.095 | 16.674 / 17.187 |
| dark | en | zoom | 0.001 | 6.472 | 16.680 / 16.873 |
| dark | en | drag | 14.192 | 0.095 | 16.647 / 16.835 |
| dark | ja | pan | 0.001 | 0.081 | 16.657 / 16.835 |
| dark | ja | zoom | 0.001 | 6.034 | 16.684 / 16.969 |
| dark | ja | drag | 12.747 | 0.090 | 16.671 / 16.847 |
| dark | zh-cn | pan | 0.002 | 0.093 | 16.666 / 16.924 |
| dark | zh-cn | zoom | 0.001 | 7.237 | 16.663 / 16.874 |
| dark | zh-cn | drag | 15.954 | 0.099 | 16.683 / 16.797 |
| light | en | pan | 0.002 | 0.096 | 16.671 / 16.792 |
| light | en | zoom | 0.001 | 7.220 | 16.667 / 16.865 |
| light | en | drag | 15.017 | 0.100 | 16.674 / 16.788 |
| light | ja | pan | 0.002 | 0.092 | 16.667 / 16.813 |
| light | ja | zoom | 0.001 | 7.266 | 16.679 / 16.955 |
| light | ja | drag | 14.367 | 0.097 | 16.680 / 16.898 |
| light | zh-cn | pan | 0.002 | 0.095 | 16.666 / 16.877 |
| light | zh-cn | zoom | 0.001 | 10.841 | 16.690 / 17.015 |
| light | zh-cn | drag | 20.665 | 0.142 | 16.687 / 21.377 |

[全工程の統計と生JSON](benchmarks/2026-10-11-animation.json)。18条件・1,080サンプル、Surfaceスキップ0、missing glyph 0、GPU Validationエラーなし。起動時には最小化相当のゼロサイズ・空領域でのスキップ、範囲外領域の拒否、サイズ復元後の描画回復も確認しました。

Panでは25〜36ノード、Zoomでは36〜10,000ノード、Dragでは36ノードが可視。Zoomは1.0〜0.02の往復により全体表示と近景を通過し、Scene生成のp95は6.034〜10.841msでした。Dragの各条件で確定5回を含み、更新処理のp95は12.747〜20.665ms。更新時間が長い上位5サンプルはすべてUp時の確定に一致しました。更新にはInteraction、Command・Documentの検証、履歴、Index更新を含むため、この計測だけで個別の処理を原因と断定しません。次は確定処理の内訳を測り、編集の正しさを保ちながら費用を削減します。

成功開始頻度は各条件59.974〜60.040Hzですが、これはCPUが入力処理を開始した頻度です。取得処理の中央値は約15.1〜16.3msと大きく、Surface側の待機も総時間に含まれます。最も長い開始間隔p95は21.377msでした。取得待ち、GPUの並行動作、イベントループの影響があるため、個別工程の中央値やp95を足し合わせて総時間にしません。JSONの16.667ms超過件数も画面の欠落フレーム数ではありません。

1台での単一実行・各条件約1秒の合成入力です。実マウス入力、WebViewとの合成や共有Engineの競合、画像プレビュー、Group、大きなカード、長時間の安定性は含みません。GPU完了・OS compositor・画面走査を観測していないため、表示60FPS達成や入力から表示までの遅延を示しません。Windows/Linuxは実行環境がないため保留です。

English: Normal queued rendering completed 18 cases × 60 samples on Apple M4 / Metal, 1280×720, scale 1, Fifo, without explicit GPU waits or readback. All 1,080 samples succeeded without skips, missing glyphs or validation errors. Synthetic zoom traversed 36–10,000 visible nodes; scene-generation p95 was 6.034–10.841 ms. Drag included five committed edits per case; input/edit/index p95 was 12.747–20.665 ms, and the five slowest updates in each case were releases. Further profiling must separate validation, history and index costs. CPU start cadence of 59.974–60.040 Hz does not establish display FPS; maximum case interval p95 was 21.377 ms. These short, single-machine cases exclude OS input latency, shared Engine/WebView contention, previews, groups and larger cards.

简体中文：Apple M4／Metal、1280×720、倍率1、Fifo下，普通提交渲染完成18组×60个样本，无显式GPU等待或回读。1,080个样本均成功，无跳过、缺字或验证错误。合成Zoom覆盖36–10,000个可见节点，场景生成p95为6.034–10.841ms。每组Drag提交5次编辑，输入／编辑／Index更新p95为12.747–20.665ms，最慢的5次更新均对应释放提交。后续需进一步区分验证、历史和Index成本。CPU开始频率59.974–60.040Hz不代表屏幕FPS，组间最高间隔p95为21.377ms。短时间单机测试不含真实OS输入延迟、共享Engine／WebView竞争、预览、Group或大卡片。


## 移動確定の工程分離と検証最適化（2026-10-11）

通常描画計測で重かったDragの確定を、Command適用・core検証・ホスト検証・履歴・Index更新に分けました。`Editor::execute_timed` と同じ計測コードを使い、coreが毎回全体検証する変更前と、移動だけの検証を限定する変更後を順にrelease実行しました。Apple M4／macOS/aarch64、10,000ノード／30,000接続、各5回の予熱＋30回の計測。詳細な契約と再現方法は [PLACEMENT_EDITS.md](PLACEMENT_EDITS.md)。

| 移動数 | 工程 | 変更前 中央値 / p95 (ms) | 変更後 中央値 / p95 (ms) |
|---:|---|---:|---:|
| 1 | 命令適用・局所検査 | 0.000708 / 0.001333 | 0.000792 / 0.001750 |
| 1 | core全体検証／省略分岐 | 7.876333 / 8.715500 | 0.000000 / 0.000042 |
| 1 | 履歴更新 | 0.001542 / 0.002625 | 0.001292 / 0.003084 |
| 1 | 確定全体 | 7.878584 / 8.718792 | 0.002333 / 0.005291 |
| 1 | 通常Undo | 7.852292 / 9.187625 | 0.000625 / 0.001833 |
| 1 | Index更新 | 0.009250 / 0.017667 | 0.009500 / 0.021958 |
| 100 | 命令適用・局所検査 | 0.009666 / 0.011291 | 0.008875 / 0.010667 |
| 100 | core全体検証／省略分岐 | 8.019208 / 10.482834 | 0.000041 / 0.000083 |
| 100 | 履歴更新 | 0.008333 / 0.009542 | 0.007459 / 0.008333 |
| 100 | 確定全体 | 8.037000 / 10.503125 | 0.016417 / 0.019375 |
| 100 | 通常Undo | 8.181750 / 9.049459 | 0.016417 / 0.021125 |
| 100 | Index更新 | 14.518583 / 18.251000 | 14.466333 / 15.036792 |

[変更前後の工程別生JSON](benchmarks/2026-10-11-placement.json)。単一ノードの確定中央値7.878584msのうち、core全体検証が7.876333msを占めていました。移動だけのCommand／Batchは対象ノードと矩形を検査し、Graphが変わらないことを利用して全体再検証を省略します。これにより確定中央値は0.002333ms、100ノードでは8.037000msから0.016417msになりました。ホスト検証の登録はなく、その区間は分岐と計時のみです。

毎回Undoで元のDocumentに戻し、Documentの一致と全体検証を測定区間外で確認しています。通常Undoも同じ最適化を使います。履歴は64ステップ／16MiBで有効。Undo後の次のexecuteにはRedo破棄も含まれます。1ノードでもポインター確定と同じBatchを使います。Index更新は別区間で、100ノードでは関連Edgeが差分更新の128件上限を超えるため、全体再構築の約14.5msが残りました。この段階ではIndex処理を変更していません。後述の「一括移動後のBVH境界更新」で別に改善と比較を行いました。

これは2つの順次実行を比べたCPU診断です。微小値をアプリ全体の高速化率やFPSへ換算しません。Graph全体を変更する操作、ホスト独自のDocumentValidator、RegistryのGraph／Definition照合は維持しています。この測定時点ではRegistryの検証費用が残っていました。後述のRegistry最適化は別の比較で計測しています。この数値が実アプリへそのまま当てはまるわけではありません。Scene生成・GPU・IPC・共有ロック待ちも確定全体の値には含みません。

English: Matching release instrumentation isolated committed move costs before and after the placement-only optimization on 10,000 nodes / 30,000 edges (five warm-ups, 30 samples per case). Single-node execute median fell from 7.878584 to 0.002333 ms; the previous full core validation consumed 7.876333 ms. The 100-node median fell from 8.037000 to 0.016417 ms. Each move was undone and independently validated outside execute timing. Host validators were not installed; the subsequent Registry optimization is measured separately below. At that stage the 100-node Index rebuild remained about 14.5 ms; the subsequent bulk-refit comparison appears below. These sequential CPU runs do not establish application speedup or FPS.

简体中文：对10,000节点／30,000连线使用相同release计时，比较纯移动优化前后的确定位移，每组预热5次、计测30次。单节点execute中位数从7.878584降为0.002333ms，原core全量验证占7.876333ms；100节点从8.037000降为0.016417ms。每次Undo并在execute计时外进行独立全量验证。此次未注册宿主验证器，后续Registry优化见下方独立比较。当时100节点Index重建约14.5ms，后续批量边界更新的比较见下文。顺序运行的CPU诊断不能换算为应用整体加速比或FPS。


### 通常Surfaceでの再計測

同じApple M4／Metal、1280×720、倍率1、Fifoで `--benchmark-animation` を再実行しました。18条件×60サンプル＝1,080フレームで、Surfaceスキップ・欠字・GPU Validationエラーはありません。[最適化後の生JSON](benchmarks/2026-10-11-animation-placement.json)。

| テーマ | 言語 | Drag更新 p95 変更前 (ms) | 変更後 (ms) | 変更後の開始間隔 p95 (ms) |
|---|---|---:|---:|---:|
| dark | en | 14.192167 | 0.030583 | 16.794542 |
| dark | ja | 12.747458 | 0.034792 | 16.826333 |
| dark | zh-cn | 15.953666 | 0.035125 | 16.942459 |
| light | en | 15.017000 | 0.031750 | 16.771958 |
| light | ja | 14.366791 | 0.031250 | 16.745667 |
| light | zh-cn | 20.664584 | 0.033458 | 16.782583 |

更新にはInteraction、Command確定、履歴、SceneIndex更新を含みます。Drag各条件の確定5回を維持し、操作と描画の計測コードは前回と同一です。開始間隔はSurface待機の影響を受けるため、更新の短縮をそのままFPS改善に換算しません。この専用セッションはRegistryを登録しておらず、WebView・共有Engineの競合や実マウス入力も含みません。

English: The unchanged native animation workload completed another 1,080 samples without skips, missing glyphs or GPU validation errors. Drag input/edit/history/index p95 fell from 12.747–20.665 ms to 0.030583–0.035125 ms across the six theme/language cases, retaining five committed edits each. This dedicated fixture has no Registry, WebView or shared Engine contention, and the result is not a display FPS claim.

简体中文：同一原生动画计测再次完成1,080个样本，无跳过、缺字或GPU验证错误。六组主题／语言条件的Drag输入／编辑／历史／Index更新p95从12.747–20.665ms降至0.030583–0.035125ms，每组仍提交5次。专用数据集未注册Registry，不含WebView或共享Engine竞争，结果不代表屏幕FPS。


## Registry付きEditorの重複検証削減（2026-10-11）

同一releaseバイナリー内で、従来と同じ全体検証を行うホストラッパーと、遷移検証を使うRegistryを比較しました。Apple M4／macOS/aarch64、共通の10,000ノード／30,000接続、各条件5回予熱＋30回計測。実行コードは `crates/unge-executor/examples/registry_benchmark.rs`、[契約と再現コマンド](REGISTRY_VALIDATION.md)、[生JSON](benchmarks/2026-10-11-registry.json)。

| 操作 | 工程 | 全体検証 中央値 / p95 (ms) | 遷移検証 中央値 / p95 (ms) |
|---|---|---:|---:|
| 1ノード移動 | 確定全体 | 10.767083 / 11.340250 | 0.003625 / 0.005625 |
| 1ノード移動 | Registry検証 | 10.763916 / 11.336542 | 0.000125 / 0.000291 |
| 1ノード移動 | 通常Undo | 10.927958 / 11.378708 | 0.003750 / 0.006375 |
| 100ノード移動 | 確定全体 | 12.124834 / 14.313500 | 0.029959 / 0.050792 |
| 100ノード移動 | Registry検証 | 12.097500 / 14.285167 | 0.000166 / 0.000292 |
| 100ノード移動 | 通常Undo | 12.251167 / 15.608333 | 0.029833 / 0.054000 |
| 1プロパティ変更 | 確定全体 | 28.937375 / 33.804292 | 15.279250 / 18.708584 |
| 1プロパティ変更 | Registry検証 | 14.279042 / 16.627792 | 0.289625 / 0.427042 |
| 1プロパティ変更 | 通常Undo | 29.546500 / 32.930458 | 14.721042 / 20.818041 |

移動ではGraph／プロパティが変わらないためRegistryの再走査を省きます。プロパティ変更ではcoreの全体検証とRegistryの全ノードのDefinition／Port／PropertySchema照合を維持し、Graphの構造検証だけを二重に行わなくなりました。各組は同じEditor実装を使うため、前節のcore最適化は両方に適用されています。

履歴は64ステップ／16MiBで有効。毎回Undoして元Documentへの一致と完全なRegistry検証を計測外で確認します。確定は `execute_timed`、Undoは通常APIの外側で計時。各中央値／p95は独立集計なので、各工程の統計値を加算しても全体の統計値にはなりません。1台での順次計測であり、負荷変動や微小区間の計時計数を含みます。Index・Scene・GPU・IPC・共有ロック・ACXのPreflightは除外され、アプリ全体の応答時間や表示FPSを示しません。多数移動のIndex再構築とZoomのScene生成は引き続き改善対象です。

English: A same-release-binary comparison on Apple M4 / macOS uses the shared 10,000-node / 30,000-edge fixture, five warm-ups and 30 samples per case. Legacy full host validation versus direct Registry transitions reduced one-node move execute median from 10.767083 to 0.003625 ms, 100-node moves from 12.124834 to 0.029959 ms, and one property change from 28.937375 to 15.279250 ms. Property edits still run core validation and scan every definition/port/property, avoiding only duplicate graph checks. Each edit is undone, compared and independently validated outside timing. Both paths already include the earlier core optimization. These sequential CPU measurements exclude Index/scene/GPU/IPC/locks/ACX preflight and do not establish application latency or display FPS. Independent phase percentiles are not additive.

简体中文：同一release二进制在Apple M4／macOS上比较旧式完整宿主验证与Registry状态转换验证，使用10,000节点／30,000连线，每组预热5次、计测30次。单节点移动execute中位数从10.767083降至0.003625ms，100节点从12.124834降至0.029959ms，单属性修改从28.937375降至15.279250ms。属性修改仍执行core全量验证及全部Definition／端口／属性检查，仅避免重复图结构验证。每次Undo后在计时外比较文档并独立完整验证。两种路径均包含前文core优化。该顺序CPU比较不含Index／场景／GPU／IPC／锁／ACX预检，不代表应用整体延迟或屏幕FPS；各阶段独立统计的分位数不能相加。


## 一括移動後のBVH境界更新（2026-10-11）

Apple M4／macOS/aarch64で `bulk_move_benchmark` をrelease実行しました。共通の10,000ノード／30,000接続、1／100／1,000ノードを96×36だけ移動し、各5回予熱＋30回計測。同じ最終Documentの保持Index更新と全体再構築を比較します。[再現と契約](BULK_MOVES.md)、[生JSON](benchmarks/2026-10-11-bulk-moves.json)。

| 移動ノード数 | 保持Index更新 中央値 / p95 (ms) | 全体再構築 中央値 / p95 (ms) | Undo後のIndex更新 中央値 / p95 (ms) |
|---:|---:|---:|---:|
| 1 | 0.013041 / 0.039500 | 16.699541 / 26.175833 | 0.006334 / 0.009958 |
| 100 | 1.916625 / 3.100834 | 18.191417 / 23.483250 | 1.880750 / 3.000208 |
| 1000 | 4.056542 / 4.804084 | 16.647125 / 19.745458 | 4.063667 / 4.781875 |

1ノードは従来の差分、100／1,000ノードはrefitを使いました。以前は100ノードの関連Edgeが128件を超えて全体再構築していました。今回の同じバイナリーでの比較では、100ノードで18.191417msに対して1.916625ms、1,000ノードで16.647125msに対して4.056542msでした。単一ノードの比較は既存差分と全体再構築の参考値で、今回の変更による高速化ではありません。

木の分割を保持するので、更新後の検索も計測しました。3つの近景Viewportの合計です。

| 移動ノード数 | Node検索 保持 / 再構築 中央値 (ms) | Scene生成 保持 / 再構築 中央値 (ms) |
|---:|---:|---:|
| 1 | 0.009375 / 0.004541 | 0.143833 / 0.108584 |
| 100 | 0.009917 / 0.006083 | 0.154042 / 0.122084 |
| 1000 | 0.007959 / 0.005083 | 0.134958 / 0.108292 |

毎回、全体再構築した同じ状態とQuadのバイト列・文字・可視Node／Edge数・範囲検索・先頭1,100ノードのHit Testを照合しました。Undo後の元Document一致・全体検証・描画一致も計時外で確認しています。履歴は64ステップ／16MiB。Index更新はGroupの再集計を含みますが、このfixtureにGroupはありません。Group・遠距離移動・サイズ変更・上限超過・入れ子Batch・差分との併用は回帰テストで確認しました。

これは1台での順次CPU計測です。保持Index更新を先に計測し、同じ状態を全体再構築した後に、保持／再構築の順で検索・Sceneを計測するため、キャッシュと実行順序の影響があります。refit後の近景検索中央値は完全再構築より数µs長く、Sceneも約0.03ms長い結果でした。移動距離と配置を限定した短い計測で、任意の大規模並べ替えの検索性能は保証しません。Command／検証／履歴／Registry、GPU／Surface、IPC／ロック待ちは含みません。表示FPSの達成は未検証です。ZoomのScene生成と、遠距離移動後の自動的な分割品質評価は今後の対象です。

English: On Apple M4 / macOS, the same release binary compared retained index updates with full reconstruction on the same 10,000-node / 30,000-edge final document (five warm-ups and 30 samples for moves of 1/100/1,000 nodes by 96×36). For 100 nodes, update median was 1.916625 ms versus 18.191417 ms reconstruction; for 1,000 nodes, 4.056542 versus 16.647125 ms. Each sample matches reconstructed quads, labels, visible counts, range queries and hit tests; undo restores the document and matching geometry. Three nearby viewport queries/scenes were measured separately. Retained queries were several microseconds slower and scenes about 0.03 ms slower in this fixture. Measurement order/cache state can affect comparisons. The single-node path is the existing overlay, not a new improvement. The fixture has no groups; group and limit correctness are tested separately. Timings exclude edit/validation/history/Registry, GPU/Surface, IPC and locks, and do not establish FPS or arbitrary rearrangement performance.

简体中文：Apple M4／macOS上的同一release二进制比较相同最终10,000节点／30,000连线文档的保持Index更新与完整重建。移动1／100／1,000节点，距离96×36，每组预热5次、计测30次。100节点更新中位数1.916625ms，重建18.191417ms；1,000节点4.056542ms，重建16.647125ms。每个样本与重建结果比较Quad、文字、可见数量、范围查询及Hit Test，Undo后恢复并比较文档和形状。另测三个近景视口的查询／场景。保持Index查询在此次数据集上慢数µs，场景约慢0.03ms；顺序和缓存可能影响比较。单节点仍用旧差分，并非本次新增优化。数据集无Group，Group及超限正确性由测试覆盖。计时不含编辑／验证／历史／Registry、GPU／Surface、IPC及锁，不代表FPS或任意重排的性能。


## 可視ID処理の削減（2026-10-13）

Apple M4／macOSで同じrelease計測例を変更前後に順次実行しました。10,000ノード／30,000接続、各5回予熱＋30回計測。[再現方法](SCENE_VISIBILITY.md)、[生JSON](benchmarks/2026-10-13-zoom.json)。

| Zoom | 中央値 変更前 → 後 (ms) | p95 変更前 → 後 (ms) | 可視Node数 |
|---:|---:|---:|---:|
| 1.00 | 0.023292 → 0.022375 | 0.029583 → 0.024875 | 36 |
| 0.50 | 0.044834 → 0.041958 | 0.050458 → 0.042958 | 121 |
| 0.20 | 0.235042 → 0.226833 | 0.293792 → 0.271417 | 702 |
| 0.10 | 0.974833 → 0.924500 | 1.015667 → 0.984291 | 2808 |
| 0.05 | 3.494375 → 3.312125 | 3.623583 → 3.422291 | 10000 |
| 0.02 | 3.527250 → 3.275500 | 3.838542 → 3.361542 | 10000 |

6条件ともQuad／文字の指紋、Quad数、可視Node／Edge数が一致しました。全体表示の中央値は3.527250→3.275500ms（約7%短縮）。これは静的ZoomのCPU Scene生成で、連続Surface操作・表示FPS・GPU・IPC・共有Engine競合を測りません。順次実行の負荷変動も含みます。Group／選択／プレビューは性能fixtureの範囲外です。

English: All six static-zoom cases retain matching quad/text fingerprints and visible counts. Overview scene median decreased from 3.527250 to 3.275500 ms (about 7%). Sequential CPU-only runs exclude GPU/Surface, IPC, shared Engine contention, groups, selection and previews; they do not establish display FPS.

简体中文：六档静态Zoom的Quad／文字指纹及可见数量均一致。全景Scene中位数从3.527250降至3.275500ms，约减少7%。顺序CPU计测不含GPU／Surface、IPC、共享Engine竞争、Group、选择或预览，不代表屏幕FPS。


## Edge情報の保持とScene工程計測（2026-10-14）

Apple M4／macOS、同じrelease例で変更前後を順次比較。各Zoomで5回予熱＋通常30回・別の診断30回。前節とは異なる実行で、絶対値を横断して高速化率を計算しません。[計測契約と追加メモリー](SCENE_TIMINGS.md)、[生JSON](benchmarks/2026-10-14-scene-phases.json)。

| Zoom | 通常Scene中央値 前 → 後 (ms) | Edge工程中央値 前 → 後 (ms) |
|---:|---:|---:|
| 1.00 | 0.029542 → 0.018500 | 0.018583 → 0.011292 |
| 0.50 | 0.056958 → 0.032250 | 0.041875 → 0.022708 |
| 0.20 | 0.362250 → 0.177584 | 0.249875 → 0.114083 |
| 0.10 | 1.459625 → 0.747375 | 1.018833 → 0.421250 |
| 0.05 | 5.283667 → 2.694125 | 3.442958 → 1.511958 |
| 0.02 | 4.830667 → 2.717875 | 3.290500 → 1.549541 |

6条件でQuad／文字の指紋、Quad数、可視Node／Edge数が一致。全体表示の通常Scene中央値4.830667→2.717875ms。Edge工程は曲線分割だけでなく、Map参照・境界・色・Quadも含むので、曲線単体の時間とは扱いません。保持情報により境界計算と接続元Nodeの再参照を省き、曲線分割は従来どおり実行します。Group／選択／Previewなしの静的fixtureで、GPU／IPC／共有Engine競合は計時外です。順次計測の負荷変動があります。

English: Six cases retain identical output fingerprints/counts. Overview ordinary Scene median fell from 4.830667 to 2.717875 ms. The Edge phase includes lookup, bounds, colors, tessellation and quads; it is not isolated curve time. Extra retained data avoids bounds/source-node lookup; subdivision is unchanged. Sequential static CPU runs exclude groups/selection/previews, GPU, IPC and shared Engine contention.

简体中文：六档输出指纹／数量一致。全景普通Scene中位数从4.830667降至2.717875ms。Edge阶段包括查找、边界、颜色、细分及Quad，并非单独曲线时间。额外保留数据省去边界计算和源节点查找，细分保持原规则。顺序静态CPU计测不含Group／选择／预览、GPU、IPC及共享Engine竞争。


### 通常Surfaceの連続操作確認

Apple M4／Metal、1280×720、倍率1、Fifoで既存 `--benchmark-animation` を実行。18条件×60＝1,080成功サンプル、Surfaceスキップ0。[生JSON](benchmarks/2026-10-14-animation-scene.json)。

| テーマ | 言語 | 連続Zoom Scene 中央値 / p95 (ms) |
|---|---|---:|
| dark | en | 0.892333 / 4.552792 |
| dark | ja | 1.405459 / 5.043291 |
| dark | zh-cn | 1.708625 / 5.063500 |
| light | en | 1.289833 / 6.336584 |
| light | ja | 1.444417 / 6.232625 |
| light | zh-cn | 1.635583 / 4.445667 |

GPU完了待ちを追加しない通常提出経路です。合成Pan／Zoom／Dragであり、WebView・Registry・共有Engine競合・実マウス入力・Groupは含みません。前回のSurface結果とは依存更新や時点が異なるので高速化率を計算しません。表示完了と入力から表示までの遅延は未測定です。

English: Native queued rendering completed 18 cases / 1,080 samples on M4/Metal with zero Surface skips. Continuous zoom Scene p95 was 4.445667–6.336584 ms. This synthetic fixture excludes WebView/Registry/shared Engine contention, real input and groups. No explicit GPU wait or display-latency measurement was added. Earlier native runs have different dependencies/conditions and are not a controlled speedup comparison.

简体中文：M4／Metal普通提交渲染完成18组／1,080个样本，Surface跳过为0。连续Zoom的Scene p95为4.445667–6.336584ms。合成数据集不含WebView／Registry／共享Engine竞争、真实输入或Group。不增加GPU等待，也不测量屏幕延迟。此前原生计测的依赖／条件不同，不据此计算加速比。
