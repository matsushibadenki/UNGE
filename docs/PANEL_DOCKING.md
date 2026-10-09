# 設定パネルのドッキング / Panel docking / 面板停靠

## 日本語

macOSの統合サンプルで「設定パネルを切り離す」を押すと、右の設定WebViewを浮動ウインドウへ移します。「設定パネルをドッキング」、または浮動ウインドウの閉じる操作で右側へ戻します。グラフは空いた幅を使用します。

これは1つの設定パネルをボタンで移動する実装です。任意の位置へのドラッグによるドッキング、複数パネルのタブ化は未実装です。Windows/Linuxと `--separate-windows` では操作を表示しません。

### 所有権と再利用

- `examples/tauri-host/src/workspace.rs` が配置状態と移動中の排他制御をRustで所有します。Document・View・Rendererは既存Engineに保持します。配置変更はDocumentの履歴やrevisionを増やしません。
- `Webview::reparent` で同じ `controls` WebViewを `workspace` / `inspector` 間で移します。再読込や複製をしないため、フォームの未保存値・言語・スクロール位置を保持します。
- 浮動Windowは再ドッキング後に非表示で保持します。次に切り離すときは同じWindowを使うので、同じ起動中はOS上の位置・サイズを保持します。通常終了後は、次回起動時にも配置を復元します。
- 親の変更前にグラフのPointer操作をCancelし、移動中の描画は保留します。完了後にRustのLayoutで描画・入力範囲を更新します。
- ネイティブ移動はblocking workerから行い、main threadでの描画を予約します。OS呼び出しを待つ間に配置のMutexを保持しません。
- Capabilityは `controls` WebViewのlabelに限定したままです。移動先のWindow名をEngineの認可labelとして使いません。実行イベントとACXも同じ共有Engine／WebViewへ接続されます。
- 浮動Windowの閉じる要求は再ドッキングへ変換します。メインWindow終了時は浮動Windowを破棄し、EngineのView/Rendererを解放します。移動中のメインWindow閉じる要求は保留されるため、移動終了後にもう一度閉じてください。

Tauriのunstable APIを使用し、サンプルで固定したバージョンを対象にしています。ネイティブのreparent失敗時は元の親への復帰を試みますが、OS/runtimeの故障後の回復を保証しません。移動後のresize/focusが失敗した場合は実際の配置を通知し、UIで再取得します。Windowの強制破棄やプロセス終了に対する下書きの永続保証はありません。

### 配置の保存と復元

`panel_preferences.rs` は通常終了時に `app.path().app_config_dir()/panel-layout.json` へドッキング状態、メイン／浮動ウインドウの通常時の位置とサイズを保存します。読み込み上限は16KiB、JSONのversionは1です。移動・リサイズの通知ではRust内の値だけを更新し、終了時に同じディレクトリの一時ファイルへ書き、sync後にrenameします。未保存のフォーム、Document、選択、履歴はこのファイルに含みません。

位置はモニター作業領域からの論理座標、サイズはクライアント領域の論理サイズです。現在のモニター名／作業領域の原点で復元先を選び、見つからなければプライマリモニターを使います。現在の倍率とウインドウ枠を考慮し、作業領域内に収めます。極端に狭い画面では最小サイズも縮めます。最大化・全画面・最小化の状態は復元せず、最後に観測した通常サイズを使います。

壊れたJSON、未知のversion／フィールド、不正値、読み込み失敗では標準配置で起動し、その起動中は元ファイルへ上書きしません。技術ログに原因を出します。保存失敗でもアプリは終了し、既存ファイルを維持します。設定をリセットする場合はアプリ終了後にこのファイルを退避してください。互換の別ウインドウモードでは読み書きしません。強制終了・電源断・同時起動間の競合解決は対象外です（最後に正常終了したプロセスの配置を使います）。

### サンプル専用のIPC

| 名前 | 引数 | 応答 |
|---|---|---|
| panel_layout | なし | `{ supported: boolean, floating: boolean }` |
| set_panel_floating | `{ floating: boolean }` | 同上 |

`unge://panel-layout` は同じ小さな状態を通知します。エラーは既存の `{ code, message }` 形式で、codeはunknown_view / panel_busy / panel_unavailable / panel_errorです。ユーザー表示は3言語に翻訳し、messageは技術情報として扱います。

TypeScript宣言は `examples/tauri-host/ui/panels.d.ts`。UIは `panels.js`、状態遷移の回帰テストは `scripts/test_panels_ui.cjs` です。ホスト固有機能なので、再利用ライブラリのEngine IPCやACX Profileへドッキング命令を追加していません。独自invoke handlerへ取り込む際は既存Engine命令と、この2つのホスト命令を合わせて登録してください。

## English

The macOS integrated sample can move its settings WebView into a floating window and dock it back using buttons. Closing the floating window docks it back. The graph expands into the available space. The same WebView is reparented, preserving unsaved form values, language and scroll position. Document, View and Renderer remain in the shared Rust Engine; layout changes do not add document history.

The host owns placement and a transition guard. Native moves run on a blocking worker without holding layout mutexes across OS calls; drawing resumes afterward. Authorization remains tied to the `controls` WebView label. Floating window position/size survives another detach within the same process because the window is hidden and reused. Main-window close during a transition is prevented; close again after the move finishes.

Host commands `panel_layout` and `set_panel_floating({ floating })` return `{ supported, floating }`; `unge://panel-layout` carries the same state. See the sample's panels.d.ts for types. These are host-specific commands, outside Engine/ACX contracts. Native reparent failure attempts rollback; recovery after an OS/runtime failure is not guaranteed. UI errors refresh actual placement. Arbitrary drag docking, multiple tabbed panels and other operating systems remain unfinished or unverified. The separate-window compatibility mode does not offer docking.

Normal exit saves docking mode and normal geometry for the main and floating windows to `app.path().app_config_dir()/panel-layout.json` (version 1, maximum read size 16 KiB). Rust caches move/resize events and replaces the file via a synced temporary file on exit. Restart restores logical client size and monitor-relative logical position, using current DPI and frame dimensions. Missing displays fall back to the primary display; bounds are clamped to its work area. Minimized, maximized and fullscreen states are not restored. Invalid, newer or unreadable files are preserved and defaults are used with a technical log. Move the file aside after exiting to reset. Separate-window mode does not read or write it. Document, drafts and selection are not saved here. Forced termination, power loss and concurrent-instance coordination are outside this feature; the last successful exit wins.

## 简体中文

macOS集成示例支持用按钮将设置WebView移到浮动窗口，再停靠回右侧。关闭浮动窗口也会停靠回去，节点图使用空出的宽度。通过reparent移动同一个WebView，保留未保存表单、语言和滚动位置。Document、View和Renderer仍由共享Rust Engine持有；布局变化不会增加文档历史。

宿主在Rust中持有布局和迁移排他状态。原生迁移在blocking worker执行，等待OS时不持有布局Mutex；完成后恢复绘图。权限始终绑定controls WebView的label。浮动窗口停靠后隐藏并复用，所以同一次运行中可保持其位置和大小。迁移期间禁止关闭主窗口，完成后可再次关闭。

宿主命令panel_layout和set_panel_floating返回 `{ supported, floating }`，unge://panel-layout事件使用相同状态。类型见示例panels.d.ts，不属于Engine或ACX契约。原生迁移失败时尝试返回原父窗口，但不保证OS/runtime故障后的恢复。UI在错误后重新读取实际布局。拖拽到任意位置停靠、多面板标签及其他操作系统仍待实现或验证。双窗口兼容模式不提供停靠操作。

正常退出时将停靠状态、主窗口和浮动窗口的普通位置及大小保存到 `app.path().app_config_dir()/panel-layout.json`（version 1，读取上限16 KiB）。Rust在内存中记录移动和缩放事件，退出时写入临时文件、sync并rename。重启时按当前DPI和窗口边框恢复逻辑尺寸及相对显示器工作区的位置。原显示器不可用时回到主显示器，并限制在当前工作区内。不恢复最小化、最大化或全屏状态。损坏、较新版本、未知字段或无法读取的文件会保留，使用默认布局并输出技术日志。重置时请先退出，再移走该文件。双窗口模式不读写此文件。此处不保存Document、草稿或选择。强制退出、断电及多进程冲突协调不在范围内；以最后正常退出的进程为准。
