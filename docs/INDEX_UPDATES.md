# 編集後の描画Index更新

## 日本語

通常UI、ポインター操作、ACXのCommand確定後に、小さな変更だけを描画Indexへ反映します。
Documentの検証、revision照合、履歴は従来のEditorで行います。描画IndexはRust内の派生データです。

```rust,ignore
let changes = unge_render::SceneChanges::from_command(&command);
editor.execute(command)?;
let update = scene_index.update(editor.document(), &changes);
```

変更情報はCommandを消費する前に取得し、編集成功後にだけ適用してください。
SceneIndexは編集直前の同じDocumentに対応している必要があります。
直接Documentを置き換えた場合は `SceneIndex::new()` を使います。
`SceneUpdate` はRustホスト用の `Unchanged / Incremental / Rebuilt` で、IPC契約を追加しません。

- MoveNodeとそのBatch: 最終placementを使い、変更ノードと接続するEdgeの曲線・境界だけを更新。
- SetProperty: 現在の描画形状に影響しないためIndexを維持。
- SetGroup: Group専用Indexを更新し、Node/Edge Indexを保持。MoveNode時もGroup境界を再集計。[GROUP_RENDERING.md](GROUP_RENDERING.md)。
- ノード/Edgeの追加・削除を含むBatch、Undo/Redo: 全体再構築。
- 大きな移動、接続数が多い移動、更新保持量超過: 全体再構築へ切り替え。

SpatialIndexは既存BVHと最大128個の更新矩形を保持します。
検索時に旧矩形の結果を除外し、更新矩形を検査してID順で返すため、
画面外から入る移動、画面外へ出る移動、重なり時のhit_testが正しく動きます。
同一IDの繰り返し移動は保持量を増やしません。ノードとEdgeのIndexはそれぞれ128件が上限です。
`update_rects()` は矩形をupsertする汎用APIです。無効矩形や容量超過ではfalseを返し、一切更新しません。
falseの場合は呼び出し側で現在の全矩形から再構築してください。削除APIではありません。

各Viewの一時プレビューの無効化と、削除済みノードの選択解除は差分更新時にも行います。
失敗したBatch、古いrevisionの編集ではIndexを変更しません。
GPUの保持場所、毎フレームの転送方式、ACXの承認/Receipt契約は従来どおりです。

### 制限

トポロジーの差分更新とUndo/Redoの差分更新は未実装です。曲線は [CURVE_TESSELLATION.md](CURVE_TESSELLATION.md) の適応分割に対応しました。
Document全体の検証と選択整合性のチェックは残っています。
曲線の全Quad、ラベル、可視Sceneは表示ごとに生成します。60FPSは引き続き未検証です。
将来プロパティから表示形状を計算する場合は、SceneChangesの分類も更新してください。

## English

Capture `SceneChanges::from_command()` before consuming a Command and apply it only after a successful edit.
The retained SceneIndex must represent the immediately preceding state of the same Document.
Small moves update node rectangles and incident edge curves using bounded BVH overlays (128 rectangles per index).
Properties leave geometry unchanged; group edits rebuild the group index while retaining node/edge indexes. Topology edits, undo/redo and capacity overflow rebuild.
Editor validation, revision checks, preview invalidation and selection cleanup remain authoritative.
On `SpatialIndex::update_rects()` returning false, rebuild from current rectangles; the failed call is atomic.
This optimization does not cover topology deltas or prove 60 FPS.

## 简体中文

消费Command之前获取 `SceneChanges::from_command()`，仅在编辑成功后更新Index。
SceneIndex必须对应同一Document编辑前的状态。
小规模移动通过有容量限制的BVH更新集合修改节点矩形及相连Edge曲线，每个Index最多128项。
属性目前不改变几何；Group编辑更新专用Group Index；拓扑修改、Undo/Redo和容量超限使用完整重建。
Editor验证、revision检查、预览失效处理与选择清理仍然执行。
`SpatialIndex::update_rects()` 返回false时不修改Index，调用方应使用当前矩形完整重建。
拓扑差分更新和60FPS目标尚未完成。
