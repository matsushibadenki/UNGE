# Group描画 / Group rendering / Group绘制

## 日本語

既存の `Command::SetGroup` で所属ノードと見出しを設定すると、Rust/GPUで枠と文字を描きます。
追加のIPCやJavaScriptのグラフ複製は不要です。サンプルには `20 + 22 = 42` のGroupがあります。

```rust,ignore
let command = Command::SetGroup {
    id: group_id,
    group: Some(Group {
        id: group_id,
        label: "計算".into(),
        nodes: [number_a, number_b, add].into(),
    }),
};
let changes = SceneChanges::from_command(&command);
editor.execute(command)?;
scene_index.update(editor.document(), &changes);
```

- 所属ノードの外接矩形へ16 world unitsの余白と28 unitsの見出し領域を付けます。境界は保存しません。
- 描画順はGrid→Group枠/見出し→Edge→Node。Group同士はID順で、入れ子の意味付けはありません。
- Group専用のBVHで可視範囲を検索します。所属ノードが画面外でも枠が画面内を囲めば描きます。
- ドラッグ中は変更ノードが所属するGroupの境界を再評価し、元の場所の枠を残しません。
- SetGroupはGroup Indexだけを再構築し、Node/Edge Indexを保持します。ノード移動時もGroup境界を更新します。
- ノード削除とUndo/RedoでもGroup Indexを再集計し、所属・見出し・境界を復元します。Node/Edge Indexは小さな変更なら差分更新します。
- 空Groupと、外接矩形がエンジンの有効座標範囲を超えるGroupは描きません。ノード自体は従来どおり描きます。
- 見出しはzoom≥0.6で描き、枠の幅でクリップします。Dark/Lightは既存ThemePaletteを使います。

`Group.label` はユーザーの保存テキストで、勝手に翻訳しません。英語・日本語・简体中文を既存Glyph Atlasで描けます。
既存ACXのGroup編集でも同じ経路を使います。承認・revision・Receiptの契約は変更していません。
`Scene::visible_groups` はRust側の可視件数で、IPCには追加していません。

### 境界

Groupは視覚的な分類です。Group専用のHit Test、枠ドラッグ、折りたたみは未実装です。所属Nodeの選択と見出し/所属編集UIは [GROUP_EDITING.md](GROUP_EDITING.md) を使います。
Nodeのポインター入力と実行DAGには影響しません。GPU画面のスクリーンリーダー対応も未実装です。
Group IndexはGroup全体を再構築します。大きな重複所属Groupでは、編集時の境界集計とプレビュー中の所属集計の費用が増えます。
大規模Groupを含む性能、極端座標での見た目、OS別のGroup実操作はまだ計測/検証していません。

## English

Existing SetGroup commands now draw GPU frames and titles around member node bounds.
Frames use 16 world units of padding and a 28-unit title area. Empty/out-of-range frames are omitted.
A group BVH handles culling; drag previews recompute affected group bounds, including off-screen moves.
Group edits rebuild only the group index. Small topology edits and undo/redo retain node/edge indexes while recomputing affected group bounds.
Titles preserve user text, support the existing English/Japanese/Simplified Chinese font pipeline, and are shown at zoom≥0.6.
Member selection and HTML editing are available via [GROUP_EDITING.md](GROUP_EDITING.md). Frame dragging, collapsing and native screen-reader support remain unfinished.
Large overlapping memberships are not performance-tested. No IPC/ACX protocol change is required.

## 简体中文

已有SetGroup命令可通过GPU绘制成员节点的外框和标题。
外接矩形加16 world units边距及28 units标题区域；空Group和超出有效坐标范围的外框不绘制。
Group BVH负责裁剪，拖动预览会重新计算受影响Group的边界，包括从画面外移入的情况。
Group编辑仅重建Group Index。小规模拓扑修改及Undo/Redo差分更新节点／Edge Index，并重新汇总Group边界。
标题保留用户文本，通过已有英/日/简体中文字体管线绘制，zoom≥0.6时显示。
成员选择及HTML编辑见 [GROUP_EDITING.md](GROUP_EDITING.md)。框拖动、折叠及原生屏幕阅读器支持尚未实现。大量重叠成员的性能尚未计测，IPC/ACX协议不变。
