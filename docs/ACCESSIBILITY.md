# キーボード操作と読み上げ用のノード概要

## 対応範囲

GPU描画はRustに保持し、WebViewにHTMLのノード一覧と選択・座標移動・削除の操作を提供します。サンプルでは通常のTab、矢印キー、Enterで操作できます。入力には表示ラベル、操作説明、フォーカス枠、状態通知を付けています。日本語・英語・简体中文に対応します。

これはアクセシビリティの初期対応です。GPUネイティブSurface自体のOSアクセシビリティツリー、Port接続、Group枠、複数Nodeの個別追加選択、プロパティ編集のキーボード操作、VoiceOver/NVDAによる読み上げの実機確認、WCAG適合評価は未実装または未検証です。

## ホストへの取り込み

`unge_tauri::handler()` に `accessible_nodes` を追加済みです。独自 `generate_handler!` を使うホストは `unge_tauri::accessible_nodes` も登録してください。呼び出し元の登録済みViewだけを参照し、RustのLabelCatalogとViewのlocale/selectionを使用します。

```ts
const summary = await client.summary();
const page = await client.accessibleNodes(summary.revision, null, 50);
// page.nodes: { id, title, rect, selected }[]
// next: 排他的な次ページcursor。nullなら最後。
await client.select([page.nodes[0].id]);
await client.apply({ kind: 'move_node', id: page.nodes[0].id,
  rect: { ...page.nodes[0].rect, x: 300, y: 120 } }, page.revision);
```

空ページでは `page.nodes[0]` を参照しないでください。読み取りAPIは `Engine::accessible_nodes(view, expected_revision, after, limit)`。`limit` は1〜100、ID昇順、cursorより大きいIDを返します。ノード名は256文字まで、制御文字を空白に置換し、空の翻訳はtype_idへフォールバックします。名称と座標・選択だけを返し、プロパティ、資産、画像、Document全体を返しません。総件数・revision・nextは同じRustロックの中で取得します。ノード概要は生成するSchema/TypeScript出力型にも含めています。

ページ取得時にrevisionが違えば `revision_conflict` を返します。取得したpage.revisionを編集時に使い、競合時に新revisionで自動再送しないでください。削除後やUndo/Redo後はSummaryとページを取り直します。選択はView状態のためDocument revisionを増やしません。選択の変更は同じViewのGPU表示へ反映します。

`examples/tauri-host/ui/accessibility.js` は最大50件を表示し、次ページ・先頭・更新のボタンを用意しています。任意のホストへ移すときは `invoke`、既存dispatchの `send`、操作直列化とエラー表示を行う `enqueue`、現在言語の `locale` を注入します。`index.html` のフォームと `style.css` を合わせて取り込んでください。

一覧は明示的に更新します。外部編集イベントでフォームやフォーカスを置き換えないため、入力中の座標が失われません。古いページに基づく編集はRustが拒否します。「一覧を更新」で再取得してください。入力中の移動要求はクリック/submit時のノード・座標・page.revisionを保存し、キュー待ち中に他の操作が入っても意図を変更しません。

HTML要素はtextContentで作成し、ノード名をHTMLとして挿入しません。通常のselect/input/buttonの意味と明示的なlabelを使う方針は [W3Cのフォームラベル説明](https://www.w3.org/WAI/tutorials/forms/labels/) を参考にしています。

## 検証

- Rust: 上限・ページ境界・3言語・View分離・競合・座標更新・削除・Undo・文字数上限・フォールバック。
- `node scripts/test_accessibility_ui.cjs`: 3言語、移動値の保存、入力保持、競合時に再送しないこと、ページ移動。
- ChromiumのIPC mock: 960px/320px、3言語、キーボードによる選択・移動、ページ移動・削除、ラベルの関連付け、アクセシビリティ情報、横スクロールなし、console errorなしを確認。実Tauri IPC/読み上げの確認とは区別します。
- GPU処理の変更はありません。今回の実GPUテストは再実行していません。

## English

The Rust-owned `accessible_nodes` API returns bounded semantic pages with revision, total, IDs, localized titles, rectangles, selection and an exclusive next cursor. Limits are 1–100; titles are capped at 256 characters. Register it in custom Tauri handlers and use `createClient().accessibleNodes(revision, after, limit)`. Edits use the displayed page revision and reject conflicts without retries. The example provides native HTML controls for selection, coordinate movement and deletion, with explicit refresh to preserve in-progress input and focus. Native surface accessibility, ports/group frames, screen-reader device testing and WCAG conformance remain unfinished or unverified.

## 简体中文

Rust拥有的 `accessible_nodes` API返回分页语义信息：版本、总数、ID、本地化名称、矩形、选择状态和排他性下一页游标。每页1〜100个节点，名称最多256个字符。自定义Tauri handler需注册此命令，可通过 `createClient().accessibleNodes(revision, after, limit)` 调用。编辑使用页面版本，发生冲突时不自动重试。示例使用原生HTML控件选择、移动、删除节点，并通过手动刷新保护输入和焦点。原生绘图窗口的无障碍树、端口/组框操作、屏幕阅读器实机测试和WCAG评估仍未完成或未验证。

Groupの名前/所属編集と所属Node一括選択のHTML操作は [GROUP_EDITING.md](GROUP_EDITING.md) を参照してください。
