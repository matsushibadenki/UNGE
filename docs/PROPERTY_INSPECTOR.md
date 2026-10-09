# ノードとプロパティのインターフェイス

## 実装した構成

参考画像の「色付きヘッダー → 端子 → 設定」という情報の分け方を採用します。GPUノードは役割色を薄く重ねたヘッダー、記号、タイトル、Port、説明文で構成します。近景のみ装飾を描画し、Port座標と接続・選択操作の契約を維持します。

設定の編集にはWebViewのプロパティパネルを使います。グラフでノードを1つ選択すると自動で表示します。ノード一覧の「ノードを選択」や「選択候補のノードを編集」からも対象を選べます。ヘッダーに名前・型・定義バージョン・読み込んだrevisionを表示し、値・説明・初期値ボタンを項目ごとに配置します。Dark/Lightと日本語・英語・简体中文に対応します。幅320pxでは縦スクロールし、スライダーと数値欄が横にはみ出さない構成です。

macOSのTauriサンプルは[同一ウインドウ](WINDOW_COMPOSITION.md)に操作WebViewとネイティブGPU領域を配置します。**GPUノード内へのHTML入力の重ね合わせ、画像サムネイルは今回の実装には含みません。** 今後、選択ノードの主要プロパティをインライン化する設計です。画像はRust/GPUの資産IDで参照し、フレーム画像をIPC転送する方式は採りません。

## PropertySchemaから作るコントロール

| 定義 | コントロール | 検証 |
| --- | --- | --- |
| Bool | キーボード操作できるスイッチ | 真偽値 |
| Int | 数値入力、上下限が揃えばスライダー | 整数・範囲・JS安全整数 |
| Float | 数値入力、上下限が揃えばスライダー | 有限値・範囲 |
| String + choices | 選択メニュー | 選択肢・文字数 |
| String | 複数行テキスト | Unicode scalar数 |
| Json | JSONテキスト | 構文・安全でない整数の拒否 |

任意項目は「値を設定」を外すと削除を予約します。初期値ボタンも下書きに反映するだけで、適用前にはDocumentを変更しません。`SetProperty.value: null` は削除のため、JSONのliteral nullを書き込む操作は拒否します。既存の未知プロパティは保持し、フォームは定義済みフィールドだけを編集します。選択肢の文字列は保存用の値そのものです。意味の説明を3言語のdescriptionに記載してください。

初期の並びはPropertySchemaのキー順です。アプリ固有のグループ分け、色・ファイル・資産選択、条件表示を行う表示ヒントは今後の拡張とし、コアへ用途固有の設定を埋め込みません。

## Rustへの取り込み

```rust
let registry = std::sync::Arc::new(host_registry());
let engine = unge_tauri::Engine::from_registry(document, 256, registry.clone())?;
// 必要に応じて同じregistryで作ったRunServiceをwith_executionで設定する。
```

`from_registry` は同じRegistryをEditorのバリデーターと設定メタデータに使用します。既存の `new` / `from_editor` は後方互換のため残り、その場合のプロパティ取得は `properties_unavailable` です。既存のホスト固有バリデーターを使う構成は、そのまま既存inspect/dispatchで独自フォームを作れます。

独自invoke handlerには `unge_tauri::node_properties` と `unge_tauri::selection_summary` を追加します。標準 `unge_tauri::handler()` には登録済みです。`bindings/typescript/createClient` の `nodeProperties(id, revision)` は、対象ウインドウの登録・revision・ノード存在を確認して、名前・説明・スキーマ・現在値を一つのスナップショットで返します。最大128フィールド・シリアライズ256KiBです。グラフ全体やGPUデータは返しません。

`examples/tauri-host/ui/properties.js` を、対応するHTML・CSSとともに取り込めます。ホストから `invoke` / `send` / `locale` / `selectedId` を注入し、Summaryイベントで `observe(revision)`、選択変更の通知で `refreshSelection()` を呼びます。参照するDocumentの複製は作らず、一つのノードの編集下書きだけを保持します。

## 選択への自動追従

`selection_summary` / `createClient(invoke).selectionSummary()` は呼び出し元Viewを認可し、同じRustロック内で `{ revision, count, single }` を返します。`single` はちょうど1ノードを選択したときだけUUID、それ以外はnullです。ノード一覧のページ外でも取得でき、選択ID列やDocument全体を送りません。選択の変更自体はDocumentのrevisionを増やしません。

- 下書きがない場合: 単一選択を `node_properties(id, revision)` で読み込みます。空選択ではカードを閉じ、複数選択では1つ選ぶよう案内します。外部の文書更新も読み直します。
- 未保存の入力がある場合: 表示中のノードと入力を固定し、選択の変更を案内します。保存は表示中のノードIDと読込時revisionへ行い、成功後に最新の選択へ追従します。無効入力・競合・保存失敗では切り替えません。
- 明示的な破棄: 「変更を破棄して選択に追従」で現在の選択へ切り替えます。単一ノードの取得に失敗すると下書きを残します。選択が空／複数なら明示的な破棄後にカードを閉じます。
- 一覧の「選択候補のノードを編集」は、下書きがないときに候補をRustのViewで選択します。下書き中の同ボタンは候補一覧ではなく、現在のRust選択への明示的な追従になります。

サンプルのInputBridgeは描画更新時に小さなSelectionSummaryを比較し、変更時だけ `controls` WebViewへ `unge://selection-changed` を通知します。HTML操作・Group選択・削除は既存の `unge://changed` から選択を再取得します。イベント内容を編集先として信用せず、APIから読み直します。フォーカス復帰と可視ページの2秒間隔の照会で通知欠落を補います。同じ選択・revisionならプロパティを再取得しません。

非同期取得は直列化し、通知の世代を比較して古い応答を捨てます。定期照会は既存取得や保存と重ねません。編集中の再読込や保存済み命令の自動再送はしません。ホストが独自のRust操作で選択を変更する場合も、この通知または `refreshSelection()` を接続してください。

## 保存と競合

1. 読み込んだrevisionを下書きの基準にする。
2. 「変更を適用」で差分だけを一つの `Command::Batch` にする。
3. RustがrevisionとPropertySchemaを検証し、全件適用か全件拒否にする。
4. 一度のUndoでその適用を取り消す。外部更新・失敗時には下書きを保持する。
5. 更新後の自動再送は行わず、「変更を破棄して選択に追従」で、現在の選択を読み直す。取得に失敗した場合は下書きを保持する。

処理中はフォームを無効にします。空・読込失敗・保存失敗・検証エラー・外部更新の状態を3言語で通知します。入力にはlabel、説明、エラーを紐付け、色だけに依存しません。文言と任意キーはtextContent／DOM APIで扱います。

## 実行できるサンプル

`examples/tauri-host/src/property_demo.rs` の `example.adjust` はホスト登録の実装です。入力を `input × gain + offset` で変換し、roundingを適用します。enabledがfalseなら入力を通します。noteは保存される任意メモです。既定の6ノード・5接続は最終的に50を出力します。core・共通math Registryの定義は変更しません。

## English

GPU cards now use tinted role headers. A reusable WebView inspector generates switches, number inputs, bounded sliders, text areas, choices and JSON fields from PropertySchema. Configure `Engine::from_registry(document, history_capacity, registry)` and call `nodeProperties(id, revision)`. Read responses are limited to 128 fields / 256 KiB. Apply sends one revision-checked Batch; Rust validates all changes atomically, and Undo restores the batch. Conflicts preserve drafts and require explicit reload. Optional fields can be removed; literal JSON null cannot be written with SetProperty. The executable host sample adds scale/offset/rounding/bypass controls. Native inline controls and image previews remain planned. The settings panel supports [button docking and floating](PANEL_DOCKING.md).

The inspector follows the calling Rust view's single selection, including nodes outside the current list page. `selection_summary` returns `{ revision, count, single }`; selection alone does not increment revision. Clean forms reload or close for empty/multiple selection. Dirty forms remain attached to the displayed node: applying saves that node at the loaded revision, then follows the latest selection. Explicit discard switches only after a successful read; failures preserve the draft. Native input emits `unge://selection-changed`, HTML commands emit `unge://changed`. Serialize reads and invalidate old responses on events. Focus and a visible-page 2-second query recover missed events without refetching unchanged properties. Register `selection_summary` in custom handlers and call `selectionSummary()` from the TypeScript client.

## 简体中文

GPU节点采用按角色着色的标题栏。可复用的WebView属性面板根据PropertySchema生成开关、数值、有限范围滑块、文本、选项和JSON编辑器。通过 `Engine::from_registry(document, history_capacity, registry)` 配置，并调用 `nodeProperties(id, revision)`。读取限制为128个字段／256KiB。应用时发送带版本检查的单个Batch，Rust原子验证，撤销可恢复整批修改。冲突时保留草稿，需明确重新加载。可删除可选属性；SetProperty不支持写入literal JSON null。可执行示例提供倍率、偏移、取整和旁路。节点内嵌控件和图像预览仍属后续计划。设置面板支持[按钮停靠与浮动](PANEL_DOCKING.md)。

属性面板自动跟随Rust View的单个选择，包括列表当前页之外的节点。`selection_summary` 返回 `{ revision, count, single }`；选择本身不增加文档版本。没有草稿时自动加载，空选择或多选时关闭卡片。有草稿时保持当前显示的节点；应用会保存该节点及其读取版本，成功后才跟随最新选择。明确放弃草稿时，成功读取新节点后再切换；失败仍保留输入。原生输入发送 `unge://selection-changed`，HTML命令发送 `unge://changed`。读取串行执行，并丢弃过期响应。恢复焦点及可见页面每2秒的查询可恢复遗漏通知；相同选择和版本不重复加载属性。自定义handler需注册selection_summary；TypeScript客户端使用selectionSummary()。

<!-- Design review: preserve role colors and port anchors; keep form values readable at 320px.
Reference images guide the separation of headers, connections and controls. GPU image previews
must wait for host resource integration and native composition, not simulated IPC image frames. -->

## 画面と検証

ブラウザーで実装したHTML/CSS/JSを動かした見本（Tauri IPCはmock）: [Dark](images/properties-dark.png) / [Light](images/properties-light.png)。GPUノードの画像は [NODE_APPEARANCE.md](NODE_APPEARANCE.md) の実GPU出力を参照してください。

ブラウザーの回帰テストは既存のPlaywright環境を使って実行できます。

```sh
NODE_PATH=/path/to/node_modules \
PLAYWRIGHT_EXECUTABLE_PATH=/path/to/chromium \
node scripts/test_properties_ui.cjs
```

`UNGE_SCREENSHOTS=/tmp/unge-properties` を指定すると、3言語×2テーマの画像を保存します。Rustの設定API・原子的保存・Undoは `cargo test -p unge-tauri --test properties`、ホストの計算処理は `cargo test -p unge-tauri-host` で検証します。
