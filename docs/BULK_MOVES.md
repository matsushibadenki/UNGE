# 一括移動後のBVH更新

## 日本語

既存の `SceneChanges::from_command` → 編集成功 → `SceneIndex::update` の手順で利用できます。通常UI、ポインター、ACX、Undo／Redoは同じRustの更新経路を使います。Document／revision／履歴の検証はEditorで行います。

小さな編集は従来の128 IDまでの差分を使用します。ノードの移動だけで対象ノードまたは関連Edgeが128件を超えた場合は、既存BVHの葉と祖先の境界を更新（refit）します。全ノードの表示メタデータと全接続線を作り直さず、対象と関連Edgeの最終形状を読み直します。接続の所属は変わらないため再登録しません。

| 条件 | 処理 |
|---|---|
| ノード／Edgeの更新がそれぞれ128件以下 | 従来の差分。累積差分の上限超過時は全体再構築 |
| 構造を変えず、移動ノードが1,024件以下、関連Edgeが4,096件以下。上の件数を超える一括編集 | BVHの境界更新 |
| 追加・削除・接続・切断を含む混在Batch | 従来の構造編集経路。128件を超えれば全体再構築 |
| BVHの保持上限超過、基底にないID、対象形状の欠落、無効な境界 | 全体再構築。更新を切り捨てない |

SetPropertyは現在形状を変えないため、移動との混在でも境界更新を利用できます。SetGroupはGroup専用Indexを再集計します。ノード変更時のGroup境界再集計も維持します。入れ子Batch・同じIDの反復移動は最終Documentを読みます。形状に影響する新しいCommandを追加する場合は分類を更新してください。

### SpatialIndexを直接取り込む場合

```rust,ignore
let changes: std::collections::BTreeMap<unge_core::Id, unge_core::Rect> = moved_bounds;
if !index.refit_rects(&changes) {
    index = unge_core::SpatialIndex::from_rects(all_current_bounds);
}
```

`refit_rects` は既存の基底BVH内にあるIDだけを更新します。木の分割は変えません。1つのSpatialIndexが再構築までにrefitする異なるIDは最大4,096件で、同じIDの反復は容量を増やしません。空変更は成功。未知ID（差分だけに追加されたIDも含む）、無効矩形、件数／累積ID超過はfalseを返し、一切変更しません。

成功すると対象IDの既存差分を取り除き、基底の矩形を更新します。対象が削除マーカーだった場合は復活します。無関係な差分と削除マーカーは保持します。追加／削除は引き続き `update_entries` を使います。SceneIndexでは一方のSpatialIndexが更新を拒否すると両方を最終Documentから再構築します。

検索結果は従来どおりID順で、Hit Testの重なり順を保持します。変更IDの保持量は有界ですが、refit自体は既存木を走査します。木の分割を保持するため、遠距離の移動や多数の並べ替えで境界の重なりが増えると検索費用も増えます。任意の移動で検索時間を保証するAPIではありません。ホストは `SceneIndex::new` で明示的に再構築できます。自動的な分割品質の評価は未実装です。

IPC／TypeScript／保存形式の変更はありません。GPUは従来どおりRust所有のSceneを受け取り、画像データをWebViewへ転送しません。

### 再現と検証

```sh
cargo run --release --locked -p unge-render --example bulk_move_benchmark
```

10,000ノード／30,000接続、1／100／1,000ノードを96×36の距離だけ移動し、各5回予熱＋30回計測します。同じ最終状態でIndex更新と全体再構築を計時し、その後の3つの近景Viewportの検索とScene生成も別に測ります。毎回、全体再構築とQuadバイト列・文字・可視件数・検索・Hit Testを比較し、Undo後のDocumentと描画も確認します。履歴は64ステップ／16MiB。Command適用、検証、履歴、GPU、IPC、共有ロック待ちは計時外です。Groupを含む回帰テストはありますが、この性能fixtureにはGroupがありません。[数値と生データ](PERFORMANCE.md)。

## English

Existing SceneChanges capture and successful-edit update calls automatically use bounds refitting for bulk moves: up to 1,024 moved nodes and 4,096 incident edges when either exceeds the 128-ID sparse limit. Small edits retain overlays; structural edits retain the old rebuild rules. Properties currently do not change geometry and can coexist with moves; group changes still rebuild the dedicated GroupIndex. Nested batches and history read final committed geometry. Over-limit or missing geometry triggers full reconstruction without truncation.

`SpatialIndex::refit_rects` updates existing base leaves and ancestor bounds without repartitioning, tracking at most 4,096 distinct refitted IDs until reconstruction. Repeated IDs consume no extra capacity. Empty changes succeed. Unknown IDs (including overlay-only insertions), invalid rectangles and capacity overflow fail atomically. Success removes overlays for affected IDs, including deletion markers, and retains unrelated overlays. Use `update_entries` for insertions/deletions. Queries remain sorted by ID and hit tests retain stacking order. Refitting traverses the tree; widely displaced/reordered nodes may increase bounds overlap and query cost. Hosts can explicitly rebuild; automatic tree-quality evaluation remains unfinished. IPC, TypeScript and serialization are unchanged.

The release example compares retained updates with full reconstruction on the same final 10,000-node / 30,000-edge document for 1/100/1,000-node moves of 96×36, five warm-ups and 30 samples. It separately times three nearby viewport queries/scenes and verifies quads, labels, visible counts, queries and hit tests against reconstruction before and after undo. History is enabled at 64 steps / 16 MiB. Edit/validation/history, GPU, IPC and lock costs are excluded. The performance fixture has no groups; group correctness is covered by tests. See [results](PERFORMANCE.md).

## 简体中文

沿用SceneChanges采集及编辑成功后的Index更新即可使用批量移动边界更新：移动节点最多1,024个，相关Edge最多4,096条，且任一超过128个ID的小差分上限。小编辑仍用原差分；结构修改仍按原规则重建。属性目前不影响形状，可与移动混用；Group变化仍重建专用Index。嵌套Batch及历史读取最终状态。超限或形状缺失时完整重建，不截断数据。

`SpatialIndex::refit_rects` 只更新基底BVH已有叶子及祖先边界，不重新划分。每次重建前最多记录4,096个不同ID，重复ID不增加容量，空修改成功。未知ID（包括仅在差分中新增的ID）、无效矩形及超限原子地返回false。成功后移除目标ID的差分（删除标记也会被恢复），保留其他差分。新增／删除继续使用 `update_entries`。查询仍按ID排序，Hit Test重叠顺序不变。refit会遍历已有树；大距离移动及重排可能增加边界重叠和查询成本。宿主可显式重建，自动分割质量评估尚未实现。IPC、TypeScript和保存格式不变。

上方release示例在同一最终10,000节点／30,000连线文档上比较差分更新与完整重建：移动1／100／1,000节点，距离96×36，每组预热5次、计测30次。另测三个近景视口的查询／场景生成，并在编辑及Undo后比较Quad字节、文字、可见数量、查询和Hit Test。历史限制为64步／16MiB。计时不含编辑／验证／历史、GPU、IPC或锁。性能数据集无Group，Group正确性由测试覆盖。参见[结果](PERFORMANCE.md)。
