# キャンバスのコンテキストメニュー / Canvas context menus

| 対象 / Target / 对象 | 日本語 | English | 简体中文 |
|---|---|---|---|
| Node | プロパティ、選択、座標、グループ作成、削除 | Properties, select, coordinates, create group, delete | 属性、选择、坐标、创建组、删除 |
| Port | 接続の開始／確定、表示中の接続解除、元ノードのプロパティ | Start/finish a connection, disconnect visible edges, edit the owning node | 开始或完成连接、断开可见连线、编辑所属节点 |
| Edge | 接続元・接続先の選択、接続解除 | Select source/destination, disconnect | 选中源节点或目标节点、断开连接 |
| Group | 展開・折りたたみ、改名、所属編集、削除 | Expand/collapse, rename, edit membership, delete | 展开或折叠、重命名、编辑成员、删除 |
| Stage | クリック位置に追加、選択・履歴・実行、ズーム、表示・言語・パネル、ノード／グループ一覧 | Add at the cursor, selection, history, execution, zoom, appearance, language, docking, node/group lists | 在光标处添加、选择、历史、执行、缩放、外观、语言、停靠、节点及组列表 |

ポートのメニューで「接続を開始／確定」を選び、反対側の入出力ポートでも同じ項目を選びます。折りたたんだカードのポートにも使えます。右ドラッグはパンです。座標・名前・プロパティなどの入力は、ドッキング中はクリック位置の近くに表示します。独立した設定ウインドウでは、そのウインドウで入力します。

Choose **Start / finish connection** on two ports of opposite directions, including ports on folded group cards. Right-drag pans. Editors open near the click in the docked workspace or in the settings window if it is separate.

在方向相反的两个端口上依次选择「开始或完成连接」，也适用于折叠组的端口。右键拖动可平移。设置面板停靠时，编辑器会显示在点击位置附近；面板独立时，在设置窗口中编辑。

## ホストに取り込む

- `Engine::canvas_context(view, position)` は論理ピクセルをワールド座標に変換し、ポート、グループ見出し／カード、ノード、エッジ、ステージの対象を読み取ります。接続線は描画と同じベジェ曲線と折りたたみ時のアンカーで判定します。
- `examples/tauri-host/src/context_menu.rs` がOSメニューを構成し、選ばれた対象と操作だけをWebViewに渡します。永続編集には既存のCommandと取得時のrevisionを使います。接続の型・占有・循環の検証はRust側で行います。
- `ui/context.js` は既存のフォームと実行コントローラーを再利用します。ドッキング時も同じWebViewを一時的に配置し直すため、Documentの複製は作りません。右側パネルは一覧操作とキーボード利用のために残しています。
