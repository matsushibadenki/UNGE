# コンパクトな設定パネル

## 日本語

サンプルのWebViewを、32pxのSVG操作ボタン、24pxの情報ボタン、上部のプロパティ設定へ更新しました。追加・Undo／Redo・実行・Node／Group・dock/floatは同じ既存コントローラーを使います。操作名は各コントローラーの3言語テキストからaria-labelとtitleへ反映するため、切り離し状態や未保存下書きによる名称変更も維持します。

常設の操作説明、プロパティ説明、範囲、型・version・revisionは情報ボタンのhover／focus／clickで表示します。Escで閉じ、再クリックで開けます。ウインドウの縮小や説明内容の更新時にも位置を補正します。説明は画面端から16px内側に収めます。エラー・競合・未保存状態は通常表示のまま残します。日本語・简体中文の説明は短い意味単位で改行します。

未使用の任意項目は値の入力欄を畳みます。DOM上の入力値を消さず、既存の有効化チェックボックスで再表示します。プロパティ設定を実行・Node操作より前へ移し、ノード選択時にスクロール量を減らします。左右16pxの余白、共有のDark/Light配色、フォームの保存・検証・競合・下書き保持を維持します。

### 取り込み

`ui/index.html`、`style.css`、既存の各コントローラー、`compact.js` を一緒に取り込んでください。compact.jsはapp.jsの後に読み込みます。SVGはコード内にあり、画像サービスや追加のnpm依存はありません。Node／Document／GPUの処理を移動する変更ではなく、IPCとTypeScriptの形式も変更しません。

```js
// サンプルでは読み込み時にwindow.compactControlsへ初期化済み。
window.compactControls.refresh();
window.compactControls.disconnect(); // Observerと全体のkey/resize listenerを停止
```

別ホストで手動初期化する場合は `createCompactControls(root)` を使います。既存のコントロールIDを持つ単一のパネルDOMが前提です。異なるIDにはcompact.jsのcontrols／説明対象マッピングを合わせてください。disconnect後も装飾したDOMは残り、各ボタンの通常動作を破棄しません。パネルDOMを置換する前にdisconnectしてください。Documentの複製や独自の保存ルートを追加しないでください。

### 確認した範囲

Browser pluginは未提供のため、既存Playwright＋専用headless Chrome＋IPCモックを使用しました。http://unge.test/、320／380／768×900px、英語・日本語・简体中文、Dark/Lightを確認。SVGの操作名／title、dock/float名称変更、hover／focus／Esc／再クリック、説明を開いたまま768→320pxへの縮小、ポップアップの画面内配置、保存・検証・競合・下書き・JSON・選択回復が成功しました。

macOSの実Tauri WebViewでも初期表示、アイコンからの追加・Undo、切り離し・再ドッキング、選択追従、数値22→24の保存→Undoで22へ復元、情報表示とEscを確認。検証アプリは一時設定を使い、確認後に終了しました。Windows/Linuxと実スクリーンリーダーの検証はPendingです。

既存デザインの部品更新として、配色・文字階層・ボタン寸法／ストローク・プロパティの位置・説明の改行・狭い幅を画像で比較しました。既存の配色と部品を継承しています。意図したコピー変更はGraph／Nodes見出しの短縮、空選択／保存状態の短縮、説明のポップアップへの移動です。エラーや危険な操作の意味は変更しません。

## English

The sample panel uses 32px SVG action buttons, 24px information buttons and properties near the top. Existing controllers still own actions/drafts; localized button captions become aria-labels and titles, including changing dock/discard labels. Information opens on hover, focus or click, closes with Escape and reopens on click. Position updates on resize/content changes and stays within 16px viewport margins. Validation/conflict/dirty messages remain visible. Optional inactive value controls collapse without deleting draft inputs.

Copy the entire UI folder; compact.js loads after app.js and adds no image service or package dependency. `createCompactControls(root)` expects the sample's control IDs; adapt its mappings for other hosts. The auto-created `window.compactControls` exposes refresh/disconnect. Disconnect stops observation and global key/resize listeners while leaving decorated DOM intact. Keep commands/revisions in the shared Rust Engine; IPC and TypeScript formats are unchanged.

Playwright with an isolated headless Chrome and IPC mock verifies three languages, both themes and 320/380/768px widths, tooltip positioning/open-resize/Escape, dock labels and all existing property workflows. Native macOS Tauri confirms rendering, edits/undo, docking, selection and a 22→24→22 property round trip. Windows/Linux and real screen-reader testing remain Pending. This updates existing design-system components; deliberate copy changes shorten headings/status and move explanations into popups.

## 简体中文

示例面板改为32px SVG操作按钮、24px信息按钮，属性设置位于上方。操作／草稿仍由原控制器管理，三语按钮文本映射至aria-label和title，支持浮动／停靠／放弃草稿时的名称变化。信息可通过hover、focus或click打开，Esc关闭，再点击重开。缩放窗口或文字更新时重新定位，保持距视口边缘16px。验证／冲突／未保存消息仍可见。未启用的可选值控件折叠，不删除草稿输入。

整体复制UI目录，compact.js在app.js之后加载，无额外图片服务或npm依赖。`createCompactControls(root)`要求示例控件ID，不同宿主需调整映射。自动创建的window.compactControls提供refresh／disconnect；disconnect停止观察及全局key／resize监听，保留装饰后的DOM。共享Rust Engine继续管理命令及revision，IPC／TypeScript格式不变。

通过独立headless Chrome的Playwright＋IPC mock验证三语、双主题、320／380／768px、信息定位／开着信息缩小／Esc、停靠名称及已有属性流程。macOS实际Tauri确认显示、编辑／Undo、停靠、选中追踪及属性22→24→22。Windows/Linux与实际屏幕阅读器保持Pending。沿用现有设计系统，明确缩短标题／状态并把说明移入信息弹层。
