# ノードとプロパティのインターフェイス

## 実装した構成

参考画像の「色付きヘッダー → 端子 → 設定」という情報の分け方を採用します。GPUノードは役割色を薄く重ねたヘッダー、記号、タイトル、Port、説明文で構成します。近景のみ装飾を描画し、Port座標と接続・選択操作の契約を維持します。

設定の編集にはWebViewのプロパティパネルを使います。ノード一覧で対象を選び、「選択候補のノードを編集」を押してください。ヘッダーに名前・型・定義バージョン・読み込んだrevisionを表示し、値・説明・初期値ボタンを項目ごとに配置します。Dark/Lightと日本語・英語・简体中文に対応します。幅320pxでは縦スクロールし、スライダーと数値欄が横にはみ出さない構成です。

現在のTauriサンプルは操作WebViewとネイティブGPUウインドウが分かれています。**GPUノード内へのHTML入力の重ね合わせ、画像サムネイル、ドッキングパネルは今回の実装には含みません。** 同一ウインドウ合成の後に、選択ノードの主要プロパティをインライン化する設計です。画像はRust/GPUの資産IDで参照し、フレーム画像をIPC転送する方式は採りません。

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

独自invoke handlerには `unge_tauri::node_properties` を追加します。標準 `unge_tauri::handler()` には登録済みです。`bindings/typescript/createClient` の `nodeProperties(id, revision)` は、対象ウインドウの登録・revision・ノード存在を確認して、名前・説明・スキーマ・現在値を一つのスナップショットで返します。最大128フィールド・シリアライズ256KiBです。グラフ全体やGPUデータは返しません。

`examples/tauri-host/ui/properties.js` を、対応するHTML・CSSとともに取り込めます。ホストから `invoke` / `send` / `locale` / `selectedId` を注入し、Summaryイベントで `observe(revision)` を呼びます。参照するDocumentの複製は作らず、一つのノードの編集下書きだけを保持します。

## 保存と競合

1. 読み込んだrevisionを下書きの基準にする。
2. 「変更を適用」で差分だけを一つの `Command::Batch` にする。
3. RustがrevisionとPropertySchemaを検証し、全件適用か全件拒否にする。
4. 一度のUndoでその適用を取り消す。外部更新・失敗時には下書きを保持する。
5. 更新後の自動再送は行わず、「変更を破棄して再読込」で明示的に現在値へ戻る。

処理中はフォームを無効にします。空・読込失敗・保存失敗・検証エラー・外部更新の状態を3言語で通知します。入力にはlabel、説明、エラーを紐付け、色だけに依存しません。文言と任意キーはtextContent／DOM APIで扱います。

## 実行できるサンプル

`examples/tauri-host/src/property_demo.rs` の `example.adjust` はホスト登録の実装です。入力を `input × gain + offset` で変換し、roundingを適用します。enabledがfalseなら入力を通します。noteは保存される任意メモです。既定の6ノード・5接続は最終的に50を出力します。core・共通math Registryの定義は変更しません。

## English

GPU cards now use tinted role headers. A reusable WebView inspector generates switches, number inputs, bounded sliders, text areas, choices and JSON fields from PropertySchema. Configure `Engine::from_registry(document, history_capacity, registry)` and call `nodeProperties(id, revision)`. Read responses are limited to 128 fields / 256 KiB. Apply sends one revision-checked Batch; Rust validates all changes atomically, and Undo restores the batch. Conflicts preserve drafts and require explicit reload. Optional fields can be removed; literal JSON null cannot be written with SetProperty. The executable host sample adds scale/offset/rounding/bypass controls. Native inline controls, image previews and docking remain planned.

## 简体中文

GPU节点采用按角色着色的标题栏。可复用的WebView属性面板根据PropertySchema生成开关、数值、有限范围滑块、文本、选项和JSON编辑器。通过 `Engine::from_registry(document, history_capacity, registry)` 配置，并调用 `nodeProperties(id, revision)`。读取限制为128个字段／256KiB。应用时发送带版本检查的单个Batch，Rust原子验证，撤销可恢复整批修改。冲突时保留草稿，需明确重新加载。可删除可选属性；SetProperty不支持写入literal JSON null。可执行示例提供倍率、偏移、取整和旁路。节点内嵌控件、图像预览及停靠仍属后续计划。

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
