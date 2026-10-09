# Pointer input / ポインター操作 / 指针操作

## 日本語

`unge-interaction` はcoreだけに依存するRustクレートです。Documentの変更命令を返す状態機械と、描画用の一時的な座標を提供します。Tauri、wgpu、WebViewへ依存しないため、別のホストにも取り込めます。

### 操作

| 操作 | 動作 |
|---|---|
| ノードをクリック | 選択。既に選択されたノードでは複数選択を維持 |
| Shift＋クリック | 選択の追加・解除 |
| ノードをドラッグ | 選択したノードをまとめて移動 |
| 余白をドラッグ | 矩形と交差するノードを選択。Shiftで既存選択へ追加 |
| Portをドラッグ | 出力と入力を接続。入力側から開始してもよい |
| 右/中ボタンのドラッグ | 表示範囲を移動 |
| ホイール | カーソル位置を中心に拡大縮小 |
| Esc | 操作開始前の選択・表示状態に戻して中止 |

移動開始の閾値は論理ピクセルで3px、Portの判定半径は8pxです。zoomが0.3未満なら描画と入力の両方でPortを省略します。同一画面では1つのpointer IDが操作を所有します。選択数は最大10000です。

Tauriサンプルは11個の数値ノードと1個の加算ノード、2本の接続から始まります。「加算追加」で新しい入力Portを作り、数値ノードの出力から接続を試せます。新しい加算ノードを実行する前には2つの必須入力を接続してください。

### 状態と確定

Downで開始時のrevision・表示範囲・対象座標・選択を記録します。MoveはRust内のPreviewだけを更新し、DocumentやUndo履歴を変更しません。Upの座標も計算に含め、移動は1つのBatchとして確定します。小さなクリックの揺れ、移動後に元の位置へ戻った操作、空白への接続は編集履歴を増やしません。

Port接続のプレビューは型・Single入力の占有・重複・循環を確認します。有効な接続先は緑、未接続/不正な接続先は橙で表示します。型が違うPortや占有済みPortへの確定は `invalid_connection` になります。最終的なグラフの検証はEditorでも実行します。

途中でAI・別ウインドウ・Undo/RedoがDocumentを更新するとプレビューを消し、古いrevisionの操作を拒否します。最新revisionに置き換えて同じUpを再送しないでください。画面サイズ/倍率の変更、フォーカス喪失、画面外への移動ではサンプルがCancelを送ります。

### Tauriへの組み込み

EngineがViewごとにInteractionを持ちます。ネイティブ入力もWebView経由の入力も同じ `Request::Pointer` を使います。

```rust,ignore
use unge_interaction::{PointerButton, PointerEvent};
use unge_tauri::Request;

// Down時に取得したrevisionを、この操作のMove/Upまで保持する。
let revision = engine.dispatch("controls", Request::Summary)?.revision;
engine.dispatch("controls", Request::Pointer {
    expected_revision: revision,
    event: PointerEvent::Down {
        pointer: 0, position: [100.0, 80.0],
        button: PointerButton::Primary, additive: false,
    },
})?;
// 同じrevisionでMove/Upを順に送る。
engine.dispatch("controls", Request::Pointer {
    expected_revision: revision,
    event: PointerEvent::Up { pointer: 0, position: [140.0, 120.0] },
})?;
```

入力座標は描画面の左上を原点とする**論理ピクセル**です。ネイティブの物理座標はscale factorで割ります。CSS座標は描画面のオフセットを除きます。ワールド座標への変換はRust側に任せます。

描画は `engine.draw_scaled(view, [physical_width, physical_height], scale_factor)` を使います。これによりRetinaの2倍ピクセルでも入力と表示が一致します。既存の `draw` はscale factor 1の互換APIです。独自ホストはDPI変更時にもCancelを送ってください。

`examples/tauri-host/src/input.rs` はTaoからネイティブWindowの入力を受け取ります。連続したMoveをイベントループの区切りまでまとめ、描画前に最新位置を反映します。プレビューのフレームをJavaScriptへ転送しません。確定時には既存の `unge://changed`、失敗時には `unge://interaction-error` を配信します。

このブリッジはTauriの [wry_plugin（unstable API）](https://docs.rs/tauri/2.11.6/tauri/struct.App.html#method.wry_plugin) を使い、サンプルのTauriを2.11.6、`tauri-runtime-wry` を2.11.4に固定しています。ホスト部分を更新する際はイベント型とウインドウIDの対応を再検証してください。core/interaction/renderはこのAPIに依存しません。macOSで実操作を確認し、同一ウインドウの入力境界もmacOSで確認しました。[構成方法](WINDOW_COMPOSITION.md)。Windows/Linux実機は引き続き未検証です。

TypeScriptは `createClient(invoke).pointer(event, revision)` を提供します。ホストはDown/Move/Upを順序通りに送信し、Moveはフレームごとにまとめてください。WebViewで直接受ける場合はpointer capture、lost capture時のCancelもホストで設定します。WebViewはDocumentやプレビュー座標を所有しません。

### 描画と再利用

SceneIndexのBVHは確定編集時に作ります。プレビュー時は、移動中のノードとその接続だけに一時座標を適用し、変更した境界で可視判定します。元の位置に残像を描かず、画面外から入ってくる接続も描画します。全Documentの複製や毎フレームのBVH再構築は行いません。

独立ホストでは `SpatialIndex` と `Interaction` を保持し、`Interaction::handle` が返したCommandをEditorに適用します。revisionの確認からCommand適用までを同じロック内で行い、編集後にSpatialIndex/SceneIndexを更新してください。他の編集で確定状態が変わったら `invalidate_preview()` で一時描画を消します。検証と描画のテストは各クレートのtestsに含めています。

GPU文字は [GPU_TEXT.md](GPU_TEXT.md) に実装・取り込み手順があります。キーボードだけでのノード編集、タッチの複数指操作、接続の差し替え、自動スクロールは未実装です。

## English

`unge-interaction` is a portable Rust state machine depending only on core. It supports selection, Shift toggling/addition, group dragging, intersecting box selection, port connections in either direction and view panning. Move events update ephemeral geometry; release returns one Command/Batch. Jitter and gestures returning to their starting position do not add history. The host must apply commands under the same lock as the revision check.

Tauri Engine owns one Interaction per view. Send `Request::Pointer` in order with the revision captured at pointer-down; do not retry a stale release with a new revision. AI/UI edits invalidate previews. Escape cancels; the native example also cancels on focus loss, resize, DPI change or cursor exit. Occupied single inputs, incompatible types, duplicates and cycles cannot be connected. Ports are hidden and non-interactive below zoom 0.3.

Positions use logical surface pixels. Divide native physical coordinates by the scale factor and use `Engine::draw_scaled` for rendering. The sample's Tao bridge coalesces cursor motion before drawing, keeping frames in Rust/GPU memory. It uses Tauri's unstable Wry plugin API, pinned at the host boundary. macOS native dragging, box selection, connections and undo were tested; combined WebView/native input boundaries were also tested on macOS. Windows/Linux remain unverified. See [window composition](WINDOW_COMPOSITION.md).

The renderer reuses committed BVHs and adjusts only moved nodes and incident edges for preview culling. TypeScript includes `client.pointer(event, revision)` and translated errors. A WebView host must serialize events, coalesce moves and manage pointer capture/cancel. See [GPU text](GPU_TEXT.md#english) for labels. Keyboard-only graph editing, multi-touch, rewiring and edge auto-scroll remain planned.

## 简体中文

`unge-interaction` 是仅依赖core的Rust状态机，支持点击选择、Shift追加/取消选择、多节点拖动、矩形框选、双向拖动端口建立连接及视图平移。Move仅改变临时预览，松开时返回一个Command/Batch；点击抖动或回到原位不会增加历史。宿主须在同一锁内检查版本并应用命令。

Tauri Engine为每个视图持有Interaction。Down/Move/Up按顺序发送，并保持按下时的revision。AI或其他界面修改文档后，旧操作会被拒绝，请勿用新版本重试旧Up。Esc取消；示例在失焦、尺寸/DPI变化或指针离开窗口时也取消。拒绝类型不符、占用的单输入、重复或循环连接。缩放小于0.3时隐藏端口并停止端口命中测试。

输入使用绘图区域内的逻辑像素。物理坐标除以scale factor，绘图使用 `Engine::draw_scaled`。Tao适配器合并连续移动后绘图，帧保留在Rust/GPU端。示例使用固定版本的Tauri unstable Wry API，core/interaction/render不依赖该API。已在macOS验证实际拖动、框选、连接和撤销；macOS单窗口的输入边界也已验证；Windows/Linux仍待验证。

渲染复用已提交状态的BVH，仅调整移动节点及关联连线的预览几何。TypeScript提供pointer调用与三语错误文案。WebView宿主需自行保证事件顺序、合并移动、捕获指针并在丢失捕获时取消。GPU文字见[集成说明](GPU_TEXT.md#简体中文)。仅键盘编辑、多点触控、重新接线及自动滚动尚未实现。
