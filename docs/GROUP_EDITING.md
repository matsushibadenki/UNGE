# Groupの選択と編集

## 実装した操作

- Group一覧を最大100件のページで取得。サンプルは50件ずつ表示。
- Viewで選択中のノードからGroupを作成。
- 名前変更、選択中のノードを追加／除外、Groupだけを削除。
- 所属ノードをまとめて選択し、既存の複数Node Dragで移動。

操作画面は英語・日本語・简体中文に対応します。通常のselect/input/buttonを使い、Tab/Enterでも操作できます。GPU/Documentの複製や所属IDの全件転送はありません。

## Rust / TypeScriptへの取り込み

`unge_tauri::handler()` は `groups` を含みます。独自handlerには `unge_tauri::groups` を追加してください。

```ts
const summary = await client.summary();
const page = await client.groups(summary.revision, null, 50);
// page.groups: { id, label, members }[]
// page.selected_nodes: このViewで選択中のノード数
await client.group({ kind: 'create', id: crypto.randomUUID(), label: '計算' }, page.revision);
```

Groupを操作するRequestは `{ kind: 'group', expected_revision, action }`。actionは以下です。

| kind | 引数 | 結果 |
|---|---|---|
| create | id, label | 現在のView選択から作成。空選択、既存IDを拒否 |
| rename | id, label | 所属を保持して名前変更 |
| add_selection | id | 現在のView選択を追加 |
| remove_selection | id | 現在のView選択を除外。空Groupも保持 |
| delete | id | Groupのみ削除。Node/Edgeは維持 |
| select_members | id | 所属を現在のViewのNode選択へ置換。空Groupなら選択解除 |

名称は空白のみを拒否し、1〜256 Unicode文字まで。任意の保存済みGroup名の照会は256文字まで、制御文字は空白にします。名前編集欄は新しい名前を明示的に入力する方式で、短縮した照会結果を保存名へ自動的に書き戻しません。サンプルHTMLのmaxlengthはUTF-16単位なので、絵文字などはRust上限より入力可能な個数が少なくなります。

選択・所属編集は最大10,000ノードです。これより大きいGroupがcore/ACXで保存されていても名前変更と削除は可能です。既存の `Command::SetGroup` / ACX契約は変更していません。

Rustは同一ロック内でrevision、Groupの存在、呼び出し元Viewを検査し、現在のView選択から所属を決めます。編集はSetGroupへ変換し、1回のUndo対象になります。同一内容の名前/所属更新ではrevisionと履歴を増やしません。SelectMembersはDocumentを編集せず、既存ポインター操作をCancelしてView選択を設定します。他Viewの選択は変えません。

一覧取得のlimitは1〜100、ID昇順、afterは排他的cursor、next=nullで終端です。revision・総件数・選択件数・概要は同じRustロックから取得します。ページ取得と全Group操作でexpected_revisionを検査し、競合時は編集を再送せず一覧を更新します。`missing_group` / `group_exists` / `invalid_group_label` を3言語で翻訳してください。TypeScriptクライアントと辞書、Rust生成Schema/型を更新済みです。

## サンプルUI

`examples/tauri-host/ui/groups.js` の `createGroupControls` にinvoke/send/enqueue/localeを注入します。`index.html` と `style.css` も取り込んでください。

1. グラフ（Shiftで追加選択）またはノード操作画面でノードを選ぶ。
2. Group一覧を更新し、選択件数を確認する。
3. 新しい名前を入力し「選択から作成」を押す。
4. 一覧でGroupを選び、所属追加/除外・名前変更・削除を実行する。
5. 「所属ノードを選択」後はグラフでまとめてドラッグできる。

一覧と名前欄は外部通知で置き換えません。外部編集後は明示的に更新します。名前とGroup IDとrevisionはボタンを押した時点で固定します。create/add/removeの対象ノードはRequestをRustで処理する時点のView選択です。選択はDocument revisionを増やさないため、別の入力経路で選択を変更すると新しい選択が対象になります。

大規模一覧で新しいGroupが最初のページに入らない場合は次ページから探してください。グループ同士の選択対象は独立した枠ではなく、所属Nodeです。枠のHit Test・枠からのドラッグ・折りたたみ・OSアクセシビリティツリーは未実装です。

## 検証と限界

Rustで作成・重複/欠落ID・名称・空選択・所属追加/除外・View分離・空Group選択・競合・無変更時の履歴維持・Undo/Redo・ページ境界を確認。UIロジックは3言語で名前の入力保持、クリック後の値固定、競合時の再送なし、ページ移動を確認。ChromiumのIPC mockで960px/320pxの3言語、キーボード作成/改名、所属選択/除外/追加/削除、ページ移動、ラベル、横はみ出しとconsole errorなしを確認しています。実Tauri画面/読み上げソフトでの新操作は未検証です。GPU実装は変更していません。

## English

Bounded group pages and `Request::Group` support creating from the invoking Rust view's selection, renaming, adding/removing selected nodes, deleting only the group, and selecting all members for existing multi-node dragging. Memberships stay in Rust. Every operation checks the displayed revision; persistent edits use SetGroup and Undo/Redo, with no-op edits preserving history. Labels accept 1–256 Unicode characters, selections/membership edits are capped at 10,000. The sample has native HTML controls in three languages and explicit refresh. Selection-based actions use the view selection at Rust dispatch time. Frame hit testing/dragging, collapsing and native accessibility remain unfinished.

## 简体中文

有界组分页及 `Request::Group` 支持从Rust视图选择创建组、改名、添加/移除所选节点、仅删除组以及选中全部成员进行已有多节点拖动。成员信息保留在Rust。所有操作检查页面版本，持久修改通过SetGroup及Undo/Redo执行，相同内容不增加历史记录。名称为1〜256个Unicode字符，选择/成员编辑最多10,000个节点。示例使用三语言原生HTML控件及手动刷新。基于选择的操作使用Rust处理请求时的视图选择。组框命中测试/拖动、折叠和原生无障碍树尚未实现。
