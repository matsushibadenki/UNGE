# 編集後の描画Index更新

## 日本語

通常UI、ポインター操作、ACXのCommand確定後に、小さな変更だけを描画Indexへ反映します。追加・削除・接続・移動とUndo/Redoに対応します。Documentの検証、revision照合、履歴はEditorで行います。描画IndexはRust内の派生データです。

```rust,ignore
let changes = unge_render::SceneChanges::from_command(&command);
editor.execute(command)?;
let update = scene_index.update(editor.document(), &changes);

// Undoの逆命令を参照するだけで、Documentも履歴も変更しない。
let undo_changes = editor.undo_command().map(unge_render::SceneChanges::from_command);
if editor.undo()? {
    scene_index.update(editor.document(), &undo_changes.unwrap());
}
// Redoはredo_command()から取得し、redo()成功後に同様に更新する。
```

変更情報はCommandを消費する前に取得し、編集成功後にだけ適用してください。取得と編集の間に同じEditorへ別の編集を挟まないでください。Tauriの共有Engineは同じロック内でこの手順を行い、ACX RecoverもUndoの差分を使用します。

SceneIndexは編集直前の同じDocumentに対応している必要があります。直接Documentを置き換えた場合は `SceneIndex::new()` を使います。`SceneUpdate` はRustホスト用の `Unchanged / Incremental / Rebuilt` で、IPC契約を追加しません。

- AddNode / RemoveNode / MoveNode: 最終Documentから対象の形状・Port・表示名を更新。削除ノードの接続線は既存の接続一覧から取得し、暗黙の削除も反映。
- Connect / Disconnect: 対象Edgeと両端の接続一覧を更新。同じIDの接続先変更では古い所属を外してから新しい所属を登録。
- Batch: 中間状態ではなく最終状態を読む。同じIDの削除・再作成や、一時的な追加・削除にも対応。
- SetProperty: 現在は描画形状に影響しないためIndexを維持。プロパティだけのUndo/Redoも同様。
- SetGroup: Group専用Indexを再構築。Node変更時も境界を再集計。[GROUP_RENDERING.md](GROUP_RENDERING.md)。
- 変更Node、明示的／関連Edge、または累積更新IDが上限を超える場合: 全体再構築。巨大Batchと多接続ノードは差分を切り捨てない。

SpatialIndexは既存BVHと最大128個の更新を保持します。更新は矩形か削除マーカーです。検索時には更新IDの旧矩形を除外し、更新後の矩形だけを調べ、ID順で返します。削除・再追加・画面外への移動後もHit Testと重なり順を維持します。同じIDの繰り返し更新は保持量を増やしません。

`update_entries(&BTreeMap<Id, Option<Rect>>)` は矩形をSomeで追加／置換、Noneで削除します。削除マーカーも、元々存在しないIDの削除も128件の容量に数えます。無効矩形や容量超過ではfalseを返し、一切更新しません。既存 `update_rects()` は矩形upsertの互換APIです。falseなら現在の全矩形から再構築してください。SceneIndexはNode/Edgeどちらの更新が失敗しても両方を再構築します。

### 保持する契約と制限

各Viewの一時プレビュー無効化と、削除ノードの選択解除は差分更新でも行います。失敗したBatch、古いrevision、失敗したUndo/RedoではIndexを変更しません。Historyの参照APIは読み取り専用で、残っていない履歴はNoneです。履歴容量やエントリー破棄の規則は変えません。

Document全体の検証・ノード削除に伴う接続探索・Group Index再集計・選択整合性チェックは残っています。今回の差分は描画用Node/Edge Indexの更新費用を減らします。毎フレームの可視Quadと文字は従来どおり生成し、GPU/Surfaceを含む60FPSは未検証です。実測と条件は [PERFORMANCE.md](PERFORMANCE.md)。将来プロパティから形状を計算する場合はSceneChangesの分類も更新してください。

## English

Capture `SceneChanges::from_command()` before committing an edit. Update the retained SceneIndex only after success, using the final Document. Small node/edge additions, removals, moves, rewiring and nested Batches now use bounded deltas. Removing a node includes its implicit edge deletions. Recreating an ID reads the final ports and labels. Groups still rebuild their dedicated index when nodes or groups change.

Use `Editor::undo_command()` / `redo_command()` to borrow the upcoming history command without cloning payloads; capture changes and apply them after a successful undo/redo without intervening edits. Tauri UI and ACX recovery use this under the shared lock. These Rust APIs do not change IPC, TypeScript wire types or receipts.

`SpatialIndex::update_entries()` accepts `Some(rect)` for upserts and `None` for deletions. Both count toward a 128-ID overlay limit. Invalid rectangles or capacity overflow return false atomically; rebuild from authoritative geometry. Existing `update_rects()` remains supported. Large changes and high-degree edits rebuild completely. Validation, selection cleanup and group aggregation remain; this is not an end-to-end frame-rate claim.

## 简体中文

提交Command前获取 `SceneChanges::from_command()`，仅在成功后用最终Document更新SceneIndex。小规模节点／Edge增删、移动、重新连接和嵌套Batch使用差分。删除节点时包含隐式删除的Edge；同ID重建读取最终端口和文字。节点或Group变化仍重建Group专用Index。

通过 `Editor::undo_command()`／`redo_command()` 只读借用下一条历史命令，不复制属性数据。获取差分后，不插入其他编辑，仅在Undo/Redo成功时应用。Tauri UI与ACX恢复在共享锁内执行此流程。IPC、TypeScript通信类型和Receipt不变。

`SpatialIndex::update_entries()` 用Some(rect)新增／替换，用None删除。两者都计入128个ID的上限。无效矩形或容量超限返回false且不改变Index，应根据当前几何完整重建。原有update_rects仍可用。大批量修改和高连接度修改完整重建。Document验证、选择清理及Group汇总仍存在，不代表已经达到整体帧率目标。
